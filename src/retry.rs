//! Backoff decisions, kept pure so they can be tested without a clock.

use std::time::Duration;

use crate::error::Error;

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
    /// ```
    /// use telegram_api_client::{Error, RetryPolicy};
    ///
    /// let policy = RetryPolicy::new(2, std::time::Duration::from_millis(1), std::time::Duration::from_millis(2));
    /// let terminal = Error::Api { code: 400, description: "Bad Request".into() };
    /// let transient = Error::Api { code: 503, description: "Service Unavailable".into() };
    ///
    /// assert!(policy.should_retry(1, &transient));
    /// assert!(!policy.should_retry(2, &transient));
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
