//! Delivery classification, backoff decisions and the sleeper seam.
//!
//! Everything in this module is pure or thread-based: the crate never depends on
//! a specific async runtime, and a caller can always swap the timer.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
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
            state: Arc::new(WaitState::default()),
            spawned: false,
        })
    }
}

/// Shared between the future and its sleeping thread.
#[derive(Default)]
struct WaitState {
    /// Set by the thread once it has actually slept for the whole delay.
    finished: AtomicBool,
    /// The task to wake; replaced on every poll, taken by the thread.
    waker: Mutex<Option<Waker>>,
}

/// Future that becomes ready once a helper thread has slept long enough.
struct ThreadWait {
    delay: Duration,
    state: Arc<WaitState>,
    spawned: bool,
}

impl Future for ThreadWait {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let this = self.get_mut();
        if this.delay.is_zero() || this.state.finished.load(Ordering::Acquire) {
            return Poll::Ready(());
        }
        // Register the waker on *every* poll: an executor is free to poll a
        // future more than once before the thread finishes, and dropping the
        // newest waker would park the task forever.
        {
            let mut slot = this
                .state
                .waker
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *slot = Some(cx.waker().clone());
        }
        if this.spawned {
            // Already waiting: a second poll must stay pending until the thread
            // says otherwise. Completing here would skip the whole delay.
            return Poll::Pending;
        }
        this.spawned = true;
        let state = Arc::clone(&this.state);
        let delay = this.delay;
        let spawned = std::thread::Builder::new()
            .name("telegram-api-client-sleep".to_string())
            .spawn(move || {
                std::thread::sleep(delay);
                state.finished.store(true, Ordering::Release);
                let waker = state
                    .waker
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .take();
                if let Some(waker) = waker {
                    waker.wake();
                }
            });
        if spawned.is_err() {
            // No thread available. Sleep on the current thread instead of
            // completing early: returning `Ready` here would silently turn a
            // rate-limit delay into no delay at all.
            std::thread::sleep(this.delay);
            return Poll::Ready(());
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
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use std::task::{Wake, Waker};
    use std::time::Instant;

    use super::*;

    /// A waker that counts how often the task was woken.
    #[derive(Default)]
    struct CountingWaker(AtomicUsize);

    impl Wake for CountingWaker {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, AtomicOrdering::SeqCst);
        }

        fn wake_by_ref(self: &Arc<Self>) {
            self.0.fetch_add(1, AtomicOrdering::SeqCst);
        }
    }

    fn counting() -> (Waker, Arc<CountingWaker>) {
        let counter = Arc::new(CountingWaker::default());
        (Waker::from(Arc::clone(&counter)), counter)
    }

    /// The default sleeper used to treat "already started" as "already done",
    /// so a single spurious poll completed the wait immediately. A second poll
    /// before the thread has slept must stay pending.
    #[test]
    fn a_second_poll_does_not_skip_the_wait() {
        let (waker, counter) = counting();
        let mut context = Context::from_waker(&waker);
        let mut future = ThreadSleeper.sleep(Duration::from_millis(80));

        let start = Instant::now();
        assert!(
            future.as_mut().poll(&mut context).is_pending(),
            "the first poll starts the wait"
        );
        // The poll the old `started` flag mistook for completion.
        assert!(
            future.as_mut().poll(&mut context).is_pending(),
            "a spurious poll must not complete the delay"
        );
        assert!(
            future.as_mut().poll(&mut context).is_pending(),
            "and neither must a third"
        );
        assert!(
            start.elapsed() < Duration::from_millis(80),
            "the wait had not finished yet"
        );

        // The thread wakes the task when, and only when, it has slept it out.
        // Wait for that wake instead of for a fixed wall-clock margin: on a
        // loaded machine (a full parallel test run) the helper thread can be
        // scheduled later than the margin, and a `sleep(120ms)` here then polls
        // before `finished` is set. The wake happens after the store, so once
        // the counter moves the poll is ready by construction.
        let deadline = start + Duration::from_secs(10);
        while counter.0.load(AtomicOrdering::SeqCst) == 0 {
            assert!(Instant::now() < deadline, "the sleeper never woke the task");
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(future.as_mut().poll(&mut context), Poll::Ready(()));
        assert!(start.elapsed() >= Duration::from_millis(80));
        assert!(
            counter.0.load(AtomicOrdering::SeqCst) >= 1,
            "the sleeper woke the task instead of letting it spin"
        );
    }

    /// A poll after the thread finished completes, and the zero-delay case
    /// never spawns anything.
    #[test]
    fn completion_and_zero_delay_behave() {
        let (waker, _counter) = counting();
        let mut context = Context::from_waker(&waker);

        let mut zero = NoDelay.sleep(Duration::from_secs(3600));
        assert_eq!(zero.as_mut().poll(&mut context), Poll::Ready(()));

        let mut immediate = ThreadSleeper.sleep(Duration::ZERO);
        assert_eq!(
            immediate.as_mut().poll(&mut context),
            Poll::Ready(()),
            "a zero delay needs no thread"
        );

        let mut short = ThreadSleeper.sleep(Duration::from_millis(10));
        assert!(short.as_mut().poll(&mut context).is_pending());
        std::thread::sleep(Duration::from_millis(40));
        assert_eq!(short.as_mut().poll(&mut context), Poll::Ready(()));
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
