//! Delivery classification, backoff decisions and the sleeper seam.
//!
//! Everything in this module is pure or thread-based: the crate never depends on
//! a specific async runtime, and a caller can always swap the timer.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
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

/// The default sleeper: waits on a dedicated thread, so it works with any
/// executor and never blocks the reactor.
///
/// One short-lived thread is started per wait, and only when a retry actually
/// happens — in normal operation this costs nothing.
///
/// ```rust
/// use std::time::{Duration, Instant};
/// use telegram_api_client::{Sleeper, ThreadSleeper};
///
/// # async fn demo() {
/// let start = Instant::now();
/// ThreadSleeper.sleep(Duration::from_millis(20)).await;
/// assert!(start.elapsed() >= Duration::from_millis(20));
/// # }
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct ThreadSleeper;

impl Sleeper for ThreadSleeper {
    fn sleep(&self, delay: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(ThreadWait {
            delay,
            started: false,
        })
    }
}

/// Future that becomes ready once a helper thread has slept long enough.
struct ThreadWait {
    delay: Duration,
    started: bool,
}

impl Future for ThreadWait {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let this = self.get_mut();
        if this.started || this.delay.is_zero() {
            return Poll::Ready(());
        }
        this.started = true;
        let waker = cx.waker().clone();
        let delay = this.delay;
        let spawned = std::thread::Builder::new()
            .name("telegram-api-client-sleep".to_string())
            .spawn(move || {
                std::thread::sleep(delay);
                waker.wake();
            });
        if spawned.is_err() {
            // No thread available: wake ourselves so the next poll completes
            // instead of leaving the task parked forever.
            cx.waker().wake_by_ref();
        }
        Poll::Pending
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
    /// Combines the attempt budget with [`Error::is_retryable`]. Classify the
    /// error for its method first ([`Error::classify_for`]) — that is what turns
    /// a 5xx on a send into [`Error::Ambiguous`] and therefore "do not retry".
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
    /// ```
    #[must_use]
    pub fn should_retry(&self, attempt: u32, error: &Error) -> bool {
        attempt < self.max_attempts && error.is_retryable()
    }

    /// Backoff for a 1-based attempt number, capped at [`Self::max_delay`].
    ///
    /// ```
    /// use std::time::Duration;
    /// use telegram_api_client::RetryPolicy;
    ///
    /// let policy = RetryPolicy::new(10, Duration::from_millis(100), Duration::from_millis(400));
    /// assert_eq!(policy.delay_for(0), Duration::from_millis(100));
    /// assert_eq!(policy.delay_for(u32::MAX), Duration::from_millis(400));
    /// ```
    #[must_use]
    pub fn delay_for(&self, attempt: u32) -> Duration {
        let factor = 1_u32.checked_shl(attempt.min(31)).unwrap_or(u32::MAX);
        self.base_delay.saturating_mul(factor).min(self.max_delay)
    }

    /// The delay to honour before retrying after `error`.
    ///
    /// Telegram's explicit `retry_after` wins; otherwise the backoff curve
    /// applies, capped at [`Self::max_delay`].
    ///
    /// ```
    /// use std::time::Duration;
    /// use telegram_api_client::{Error, RetryPolicy};
    ///
    /// let policy = RetryPolicy::default();
    /// let error = Error::RateLimited { retry_after: Some(2), code: 429, description: String::new() };
    /// assert_eq!(policy.delay_for_error(0, &error), Duration::from_secs(2));
    /// ```
    #[must_use]
    pub fn delay_for_error(&self, attempt: u32, error: &Error) -> Duration {
        error
            .retry_after()
            .map_or_else(|| self.delay_for(attempt), |hint| hint.min(self.max_delay))
    }
}
