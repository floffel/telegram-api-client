//! Delivery classification, backoff decisions and the sleeper seam.
//!
//! Everything in this module is pure or timer-based: the crate never depends on
//! a specific async runtime, and a caller can always swap the timer.
//!
//! # What is delegated, and what is not (issue #15)
//!
//! The audit asked for the backoff engine and the sleeping loop to come from
//! maintained crates instead of being written here. The two halves get different
//! answers, and both were measured rather than assumed.
//!
//! **The wait is delegated.** [`ThreadSleeper`] is built on `futures-timer`
//! (3.0.4, released 2026-05): its `Delay` completes on whatever executor drives
//! the future, with no tokio/async-std/smol feature and no runtime activation,
//! which was confirmed by driving it from a bare `futures::executor::block_on`.
//! That replaced a hand-written future with its own waker slot, an atomic flag
//! and one spawned thread per wait.
//!
//! **The curve is not.** Every candidate was measured against the three things
//! this policy has to do: exact integer delays, `retry_after` precedence and
//! idempotency gating.
//!
//! * `backoff` 0.4, the crate the audit named, has had no release since
//!   2021-12, and its `next_backoff` takes `&mut self`, so a pure `&self`
//!   decision function cannot hold one.
//! * `backon` 1.6 is an `Iterator` over `f32` seconds: a 100 ms base yields
//!   `100 ms, 200.000_003 ms, 400.000_006 ms, 800.000_012 ms, 1.600_000_024 s`,
//!   and entry `n` costs `n` iterator steps, so a `u32::MAX` attempt budget is
//!   unbounded work.
//! * `retry-policies` 0.5 answers with a `SystemTime` (a wall-clock correction
//!   moves the wait), takes no server hint at all, and models no idempotency.
//!
//! What remains is `u32` shifts on `Duration` plus the two rules only this crate
//! knows: Telegram hints are authoritative, and a send is never retried after an
//! ambiguous failure.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use crate::error::Error;

/// Whether a Bot API method is safe to repeat.
///
/// The read-only methods are the `get*` family (`getMe`, `getUpdates`,
/// `getChat`, `getFile`, …); every method that sends, edits, deletes or
/// administers something is state-changing, and repeating it can duplicate or
/// double-apply the effect.
///
/// ```rust
/// use telegram_api_client::is_idempotent;
///
/// assert!(is_idempotent("getMe"));
/// assert!(is_idempotent("getChatMember"));
/// assert!(!is_idempotent("sendMessage"));
/// assert!(!is_idempotent("editMessageText"));
/// assert!(!is_idempotent("get"));       // not a method name
/// assert!(!is_idempotent("getchat"));   // Bot API is camelCase
/// ```
#[must_use]
pub fn is_idempotent(method: &str) -> bool {
    method
        .strip_prefix("get")
        .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_uppercase()))
}

/// Waits between retry attempts.
///
/// The client asks its sleeper instead of calling a runtime's timer, so the
/// crate stays usable on tokio, async-std, smol or a hand-rolled executor. The
/// default is [`ThreadSleeper`]; tests pass a recording or zero-delay sleeper.
///
/// ```rust
/// use std::time::Duration;
/// use telegram_api_client::Sleeper;
///
/// /// Records every delay it was asked to wait.
/// #[derive(Default)]
/// struct Recorder(std::sync::Mutex<Vec<Duration>>);
///
/// impl Sleeper for Recorder {
///     fn sleep(&self, delay: Duration) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
///         self.0.lock().unwrap().push(delay);
///         Box::pin(std::future::ready(()))
///     }
/// }
///
/// # async fn demo() {
/// let sleeper = Recorder::default();
/// sleeper.sleep(Duration::from_secs(3)).await;
/// assert_eq!(sleeper.0.lock().unwrap().as_slice(), [Duration::from_secs(3)]);
/// # }
/// ```
pub trait Sleeper: Send + Sync {
    /// Completes after at least `delay`.
    fn sleep(&self, delay: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}

/// A sleeper that returns immediately, without waiting.
///
/// For tests and for callers that would rather not pause at all; production
/// code should keep [`ThreadSleeper`] or install its runtime's timer so
/// Telegram's `retry_after` is respected.
///
/// ```rust
/// use std::time::Duration;
/// use telegram_api_client::{NoDelay, Sleeper};
///
/// # async fn demo() {
/// NoDelay.sleep(Duration::from_secs(3600)).await; // returns at once
/// # }
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct NoDelay;

impl Sleeper for NoDelay {
    fn sleep(&self, _delay: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(std::future::ready(()))
    }
}

/// The default sleeper: a timer that works with any executor.
///
/// Waiting is delegated to `futures-timer`: its `Delay` is driven by whichever
/// executor polls the future and counts down on one shared timer thread, so the
/// crate still depends on no specific runtime and a retry no longer spawns a
/// thread of its own.
///
/// ```rust
/// use std::time::{Duration, Instant};
/// use telegram_api_client::{Sleeper, ThreadSleeper};
///
/// # async fn demo() {
/// let start = Instant::now();
/// ThreadSleeper.sleep(Duration::from_millis(20)).await;
/// assert!(start.elapsed() >= Duration::from_millis(20), "never completes early");
/// # }
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct ThreadSleeper;

impl Sleeper for ThreadSleeper {
    fn sleep(&self, delay: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        if delay.is_zero() {
            return Box::pin(std::future::ready(()));
        }
        Box::pin(futures_timer::Delay::new(delay))
    }
}

/// Exponential backoff with a hard cap, honouring Telegram's `retry_after`.
///
/// The policy never sleeps — it only answers "retry?" and "how long?" — so the
/// caller stays in control of its runtime and can abort on shutdown.
///
/// ```
/// use std::time::Duration;
/// use telegram_api_client::{Error, RetryPolicy};
///
/// let policy = RetryPolicy::default();
/// assert_eq!(policy.delay_for(0), Duration::from_millis(500));
/// assert_eq!(policy.delay_for(1), Duration::from_secs(1));
/// assert!(policy.delay_for(30) <= policy.max_delay);
///
/// let limited = Error::RateLimited {
///     retry_after: Some(9),
///     code: 429,
///     description: "Too Many Requests".into(),
/// };
/// // Telegram's hint wins over our own curve.
/// assert_eq!(policy.delay_for_error(0, &limited), Duration::from_secs(9));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Total attempts, including the first one. `1` disables retries.
    pub max_attempts: u32,
    /// Delay before the second attempt; doubles per attempt.
    pub base_delay: Duration,
    /// Ceiling for the computed delay.
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
        }
    }
}

impl RetryPolicy {
    /// Builds a policy, clamping to something sane.
    ///
    /// `max_attempts` is at least `1`; `max_delay` is at least `base_delay`.
    ///
    /// ```
    /// use std::time::Duration;
    /// use telegram_api_client::RetryPolicy;
    ///
    /// let policy = RetryPolicy::new(0, Duration::from_secs(5), Duration::from_secs(1));
    /// assert_eq!(policy.max_attempts, 1);
    /// assert_eq!(policy.max_delay, Duration::from_secs(5));
    /// ```
    #[must_use]
    pub fn new(max_attempts: u32, base_delay: Duration, max_delay: Duration) -> Self {
        Self {
            max_attempts: max_attempts.max(1),
            base_delay,
            max_delay: max_delay.max(base_delay),
        }
    }

    /// Whether `attempt` (1-based: 1 is the first try) may be followed by another.
    ///
    /// Combines the attempt budget with [`Error::is_retryable`], and refuses a
    /// retry when Telegram's `retry_after` is longer than [`Self::max_delay`].
    /// The hint is not shortened: waiting half of what the server demanded only
    /// burns an attempt on a second 429, so the error is handed back instead and
    /// the caller decides whether it can wait that long.
    ///
    /// Classify the error for its method first ([`Error::classify_for`]) — that
    /// is what turns a 5xx or an unphased transport failure on a send into
    /// [`Error::Ambiguous`] and therefore "do not retry".
    ///
    /// ```rust
    /// use telegram_api_client::{Error, RetryPolicy};
    ///
    /// let policy = RetryPolicy::new(2, std::time::Duration::from_millis(1), std::time::Duration::from_millis(2));
    /// let terminal = Error::Api { code: 400, description: "Bad Request".into() };
    /// // A 5xx on a read stays retryable; the same 5xx on a send does not.
    /// let read = Error::Api { code: 503, description: "Service Unavailable".into() }.classify_for("getMe");
    /// let send = Error::Api { code: 503, description: "Service Unavailable".into() }.classify_for("sendMessage");
    ///
    /// assert!(policy.should_retry(1, &read));
    /// assert!(!policy.should_retry(2, &read));
    /// assert!(!policy.should_retry(1, &send));
    /// assert!(!policy.should_retry(1, &terminal));
    ///
    /// // A hint longer than the ceiling means "do not retry", not "retry early".
    /// let long = Error::RateLimited { retry_after: Some(3), code: 429, description: String::new() };
    /// assert!(!policy.should_retry(1, &long));
    /// ```
    #[must_use]
    pub fn should_retry(&self, attempt: u32, error: &Error) -> bool {
        attempt < self.max_attempts
            && error.is_retryable()
            && error
                .retry_after()
                .is_none_or(|hint| hint <= self.max_delay)
    }

    /// Backoff before retry number `retry` (0-based: `0` is the first retry),
    /// capped at [`Self::max_delay`].
    ///
    /// The first retry therefore waits exactly [`Self::base_delay`], the second
    /// twice that, and so on.
    ///
    /// ```
    /// use std::time::Duration;
    /// use telegram_api_client::RetryPolicy;
    ///
    /// let policy = RetryPolicy::new(10, Duration::from_millis(100), Duration::from_millis(400));
    /// assert_eq!(policy.delay_for(0), Duration::from_millis(100));
    /// assert_eq!(policy.delay_for(1), Duration::from_millis(200));
    /// assert_eq!(policy.delay_for(u32::MAX), Duration::from_millis(400));
    /// ```
    #[must_use]
    pub fn delay_for(&self, retry: u32) -> Duration {
        let factor = 1_u32.checked_shl(retry.min(31)).unwrap_or(u32::MAX);
        self.base_delay.saturating_mul(factor).min(self.max_delay)
    }

    /// The delay to honour before retrying after `error`, which came from the
    /// `attempt`th try (1-based).
    ///
    /// Telegram's explicit `retry_after` is authoritative and returned
    /// unchanged — [`Self::should_retry`] is what refuses to retry at all when
    /// the hint is longer than [`Self::max_delay`], so shortening it here would
    /// only hide a hint the server meant literally. Without a hint the backoff
    /// curve applies, starting at [`Self::base_delay`] for the first retry.
    ///
    /// ```
    /// use std::time::Duration;
    /// use telegram_api_client::{Error, RetryPolicy};
    ///
    /// let policy = RetryPolicy::default();
    ///
    /// // A hint longer than `max_delay` is not clamped: it is the real wait.
    /// let long = Error::RateLimited { retry_after: Some(60), code: 429, description: String::new() };
    /// assert_eq!(policy.delay_for_error(1, &long), Duration::from_secs(60));
    /// assert!(!policy.should_retry(1, &long), "the caller is handed the 429 instead");
    ///
    /// // No hint: the curve starts at `base_delay`, not twice it.
    /// let refused = Error::NotSent { message: "connect refused".into(), source: None };
    /// assert_eq!(policy.delay_for_error(1, &refused), Duration::from_millis(500));
    /// assert_eq!(policy.delay_for_error(2, &refused), Duration::from_secs(1));
    /// ```
    #[must_use]
    pub fn delay_for_error(&self, attempt: u32, error: &Error) -> Duration {
        match error.retry_after() {
            Some(hint) => hint,
            None => self.delay_for(attempt.saturating_sub(1)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    /// The sleeper is what the client stores behind an `Arc<dyn Sleeper>`.
    #[test]
    fn the_sleepers_are_shareable_across_tasks() {
        fn assert_static<T: Send + Sync + 'static>() {}
        assert_static::<ThreadSleeper>();
        assert_static::<NoDelay>();
    }

    /// The delegated timer must not complete early: a retry that fires before the
    /// delay is exactly the double 429 the policy exists to avoid.
    #[test]
    fn the_thread_sleeper_waits_at_least_the_delay() {
        let start = Instant::now();
        futures::executor::block_on(ThreadSleeper.sleep(Duration::from_millis(60)));
        let elapsed = start.elapsed();
        assert!(
            elapsed >= Duration::from_millis(60),
            "completed after {elapsed:?}, less than the 60 ms asked for",
        );
    }

    /// Zero and `NoDelay` return at once, whatever the executor.
    #[test]
    fn zero_delay_and_no_delay_return_immediately() {
        let start = Instant::now();
        futures::executor::block_on(ThreadSleeper.sleep(Duration::ZERO));
        futures::executor::block_on(NoDelay.sleep(Duration::from_secs(3600)));
        let elapsed = start.elapsed();
        assert!(elapsed < Duration::from_secs(1), "no wait was requested");
    }

    /// Many waits share one timer thread: interleaved delays must all complete,
    /// and must not serialise into the sum of their durations.
    #[test]
    fn interleaved_waits_share_the_timer() {
        let start = Instant::now();
        let waits = (0..32).map(|_| ThreadSleeper.sleep(Duration::from_millis(40)));
        futures::executor::block_on(futures::future::join_all(waits));
        let elapsed = start.elapsed();
        assert!(elapsed >= Duration::from_millis(40), "waited {elapsed:?}");
        assert!(
            elapsed < Duration::from_millis(600),
            "32 waits of 40 ms took {elapsed:?}, which is serialised, not shared"
        );
    }

    #[test]
    fn is_idempotent_is_exactly_the_get_family() {
        assert!(is_idempotent("getMe"));
        assert!(is_idempotent("getUpdates"));
        assert!(!is_idempotent("sendMessage"));
        assert!(!is_idempotent("get"));
        assert!(!is_idempotent("GetMe"));
        assert!(!is_idempotent(""));
    }

    #[test]
    fn the_policy_clamps_its_own_construction() {
        let policy = RetryPolicy::new(0, Duration::from_secs(5), Duration::from_secs(1));
        assert_eq!(policy.max_attempts, 1);
        assert_eq!(policy.max_delay, Duration::from_secs(5));
        assert_eq!(
            policy.delay_for(3),
            Duration::from_secs(5),
            "capped at max_delay"
        );
    }
}
