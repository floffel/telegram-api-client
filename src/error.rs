//! Typed errors with source chains and an explicit retryable/terminal split.

use std::time::Duration;

use serde_json::Value;
use thiserror::Error;

/// Failures from verification, request building, parsing or the Bot API.
///
/// The enum is [`non_exhaustive`](https://doc.rust-lang.org/reference/attributes/type_system.html#the-non_exhaustive-attribute):
/// match with a `_` arm so new variants cannot break you.
///
/// ```
/// use telegram_api_client::Error;
///
/// let error = Error::Api { code: 400, description: "Bad Request: chat not found".into() };
/// assert!(!error.is_retryable());
/// assert!(!error.is_ambiguous());
/// assert_eq!(error.retry_after(), None);
///
/// let limited = Error::RateLimited { retry_after: Some(3), code: 429, description: "Too Many Requests".into() };
/// assert!(limited.is_retryable());
/// assert_eq!(limited.retry_after(), Some(std::time::Duration::from_secs(3)));
///
/// let unknown = Error::ambiguous("sendMessage", "timeout after the request was written");
/// assert!(unknown.is_ambiguous());
/// assert!(!unknown.is_retryable(), "retrying could duplicate the message");
/// ```
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// The configured secret/token is empty.
    #[error("secret must not be empty")]
    EmptySecret,
    /// Secret token header does not match.
    #[error("secret token mismatch")]
    SecretMismatch,
    /// Webhook path does not match the derived path.
    #[error("webhook path mismatch")]
    PathMismatch,
    /// A request or response body could not be parsed.
    #[error("invalid payload: {message}")]
    Payload {
        /// What failed to parse, in prose.
        message: String,
        /// The underlying `serde_json` failure, when there is one.
        #[source]
        source: Option<serde_json::Error>,
    },
    /// The request never produced a usable response.
    ///
    /// This variant is the *undecided* case: it says a transport failed without
    /// saying whether anything was written. A non-idempotent method is
    /// therefore treated as [`Error::Ambiguous`] by [`Error::classify_for`] —
    /// transports should report pre-write failures as [`Error::NotSent`] and
    /// post-write failures as [`Error::Ambiguous`] instead of using this.
    #[error("transport error: {message}")]
    Transport {
        /// What went wrong, in prose.
        message: String,
        /// The underlying HTTP/client failure, when there is one.
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
    },
    /// The request was never written: connecting failed, DNS failed, TLS
    /// failed — nothing reached Telegram.
    ///
    /// Safe to repeat for *any* method, including a send, which is why it is
    /// retryable. The counterpart of [`Error::Ambiguous`]: the two together
    /// cover what [`Error::Transport`] used to blur.
    #[error("request not sent: {message}")]
    NotSent {
        /// What went wrong before the request was written.
        message: String,
        /// The underlying HTTP/client failure, when there is one.
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
    },
    /// The Bot API answered with `ok: false`.
    #[error("telegram api error {code}: {description}")]
    Api {
        /// Numeric error code from Telegram (or the HTTP status).
        code: i64,
        /// Human readable description.
        description: String,
    },
    /// The Bot API asked us to slow down.
    #[error("rate limited (429): {}", retry_hint(*retry_after))]
    RateLimited {
        /// Seconds Telegram asked us to wait, when it said so.
        retry_after: Option<u64>,
        /// Numeric error code, normally `429`.
        code: i64,
        /// Human readable description.
        description: String,
    },
    /// A request was rejected before it hit the wire.
    #[error("invalid request field `{field}`: {reason}")]
    InvalidRequest {
        /// The offending field name as it appears in the Bot API.
        field: &'static str,
        /// Why it was rejected.
        reason: String,
    },
    /// The request may or may not have been accepted: the failure happened after
    /// it was written (a timeout, a dropped connection, or a 5xx on a
    /// non-idempotent method).
    ///
    /// Retrying duplicates the effect — for a message, the recipient gets it
    /// twice, and the Bot API has no idempotency key to prevent that. Compare
    /// against your own record of what you sent before deciding.
    #[error("ambiguous delivery for {method}: {detail}")]
    Ambiguous {
        /// Bot API method whose delivery is unknown.
        method: String,
        /// What was observed, in prose.
        detail: String,
        /// The underlying failure, when there is one.
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
    },
    /// Telegram answered with a shape this crate cannot interpret.
    #[error("unexpected response from {method}: {detail}")]
    Unexpected {
        /// Bot API method that was called.
        method: String,
        /// What was missing or malformed.
        detail: String,
    },
}

/// Renders the retry hint for `RateLimited`'s `Display` impl.
fn retry_hint(retry_after: Option<u64>) -> String {
    match retry_after {
        Some(seconds) => format!("retry after {seconds}s"),
        None => "retry later".to_string(),
    }
}

impl Error {
    /// Whether re-sending the exact same request can plausibly succeed without
    /// risking a duplicate.
    ///
    /// `true` for rate limits (the server rejected before processing), 5xx
    /// answers (the caller is expected to have classified the error for its
    /// method first, see [`Self::classify_for`]) and [`Error::NotSent`], where
    /// the transport proved nothing was written.
    ///
    /// `false` for everything a retry cannot fix, for [`Error::Ambiguous`],
    /// where a retry may duplicate the message, and for the unphased
    /// [`Error::Transport`] — that one is only a statement that *something*
    /// failed, so it must be classified for its method before it is retried.
    ///
    /// ```rust
    /// use telegram_api_client::Error;
    ///
    /// assert!(Error::Api { code: 502, description: "Bad Gateway".into() }.is_retryable());
    /// assert!(Error::not_sent("dns: name resolution failed").is_retryable());
    /// assert!(!Error::Api { code: 403, description: "Forbidden".into() }.is_retryable());
    ///
    /// // A 5xx whose request was a send: classified as ambiguous, not retryable.
    /// let send = Error::Api { code: 502, description: "Bad Gateway".into() }.classify_for("sendMessage");
    /// assert!(!send.is_retryable());
    ///
    /// // An unphased transport failure is never retryable on its own: the
    /// // caller has to say what happened, and `classify_for` does.
    /// let unphased = || Error::Transport { message: "boom".into(), source: None };
    /// assert!(!unphased().is_retryable());
    /// assert!(unphased().classify_for("getMe").is_retryable());
    /// assert!(unphased().classify_for("sendMessage").is_ambiguous());
    /// ```
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::RateLimited { .. } | Self::NotSent { .. } => true,
            Self::Api { code, .. } => *code == 429 || *code >= 500,
            Self::Transport { .. }
            | Self::EmptySecret
            | Self::SecretMismatch
            | Self::PathMismatch
            | Self::Payload { .. }
            | Self::InvalidRequest { .. }
            | Self::Ambiguous { .. }
            | Self::Unexpected { .. } => false,
        }
    }

    /// Whether the request may have been accepted even though the call failed.
    ///
    /// `true` only for [`Error::Ambiguous`]: the caller must decide, from its own
    /// ledger, whether re-sending is acceptable. `false` for everything else —
    /// a 429 or a connect error provably did not reach the bot, and the
    /// remaining errors are terminal.
    ///
    /// ```rust
    /// use telegram_api_client::Error;
    ///
    /// let unknown = Error::ambiguous("sendPhoto", "connection reset after the body was written");
    /// assert!(unknown.is_ambiguous());
    ///
    /// let limited = Error::RateLimited { retry_after: None, code: 429, description: String::new() };
    /// assert!(!limited.is_ambiguous());
    /// ```
    #[must_use]
    pub fn is_ambiguous(&self) -> bool {
        matches!(self, Self::Ambiguous { .. })
    }

    /// Re-classifies a failure for `method`.
    ///
    /// Two failures carry no phase information and have to be resolved by the
    /// method's idempotency:
    ///
    /// * a 5xx on a state-changing method becomes [`Error::Ambiguous`] — the
    ///   Bot API answers 5xx both before and after it accepted a message, so
    ///   delivery is unknown;
    /// * an unphased [`Error::Transport`] becomes [`Error::Ambiguous`] for a
    ///   state-changing method and [`Error::NotSent`] for a read.
    ///
    /// Read calls stay retryable either way; a send is never retried on an
    /// unknown outcome.
    ///
    /// ```rust
    /// use telegram_api_client::Error;
    ///
    /// let send = Error::Api { code: 502, description: "Bad Gateway".into() }
    ///     .classify_for("sendMessage");
    /// assert!(send.is_ambiguous());
    ///
    /// let read = Error::Api { code: 502, description: "Bad Gateway".into() }
    ///     .classify_for("getChat");
    /// assert!(read.is_retryable() && !read.is_ambiguous());
    ///
    /// // A 400 is terminal either way, and a 429 stays retryable.
    /// let bad = Error::Api { code: 400, description: "Bad Request".into() }.classify_for("sendMessage");
    /// assert!(!bad.is_ambiguous() && !bad.is_retryable());
    ///
    /// // A transport that could not say when it failed is resolved the same
    /// // way: ambiguous for a send, safely repeatable for a read.
    /// let unphased = || Error::Transport { message: "reset".into(), source: None };
    /// assert!(unphased().classify_for("sendMessage").is_ambiguous());
    /// assert!(unphased().classify_for("getChat").is_retryable());
    /// ```
    #[must_use]
    pub fn classify_for(self, method: &str) -> Self {
        match self {
            Self::Api { code, description }
                if code >= 500 && !crate::retry::is_idempotent(method) =>
            {
                Self::Ambiguous {
                    method: method.to_string(),
                    detail: format!("HTTP {code} after a state-changing request: {description}"),
                    source: None,
                }
            }
            Self::Transport { message, source } if crate::retry::is_idempotent(method) => {
                Self::NotSent { message, source }
            }
            Self::Transport { message, source } => Self::Ambiguous {
                method: method.to_string(),
                detail: format!(
                    "transport failed without saying whether the request was written: {message}"
                ),
                source,
            },
            Self::Ambiguous {
                method: current,
                detail,
                source,
            } if current.is_empty() => Self::Ambiguous {
                method: method.to_string(),
                detail,
                source,
            },
            other => other,
        }
    }

    /// Reports a failure that happened after the request was written.
    ///
    /// Transports call this when they cannot tell whether the bot received the
    /// request — a timeout while waiting for the answer, a connection reset
    /// mid-body, a truncated response. The result is never
    /// [retryable](Self::is_retryable).
    ///
    /// ```rust
    /// use telegram_api_client::Error;
    ///
    /// let error = Error::ambiguous("sendMessage", "timed out waiting for the response");
    /// assert!(matches!(error, Error::Ambiguous { .. }));
    /// assert!(error.to_string().contains("timed out"));
    /// ```
    #[must_use]
    pub fn ambiguous(method: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Ambiguous {
            method: method.into(),
            detail: detail.into(),
            source: None,
        }
    }

    /// Reports a failure that provably happened before the request was written.
    ///
    /// Transports call this when connecting, DNS or the TLS handshake failed,
    /// so nothing reached Telegram and repeating the request cannot duplicate
    /// anything. The result is [retryable](Self::is_retryable) for every
    /// method, sends included.
    ///
    /// ```rust
    /// use telegram_api_client::Error;
    ///
    /// let error = Error::not_sent("dns: name resolution failed");
    /// assert!(error.is_retryable());
    /// assert!(!error.is_ambiguous());
    /// assert!(error.to_string().contains("name resolution"));
    /// ```
    #[must_use]
    pub fn not_sent(message: impl Into<String>) -> Self {
        Self::NotSent {
            message: message.into(),
            source: None,
        }
    }

    /// Attaches the underlying failure, keeping any cause that is already there.
    ///
    /// [`Error::Ambiguous`], [`Error::NotSent`] and [`Error::Transport`] carry a
    /// source slot and keep the failure; every other variant has nowhere to put
    /// it and is returned unchanged. Attaching twice does not drop the first
    /// cause: the newest failure becomes the visible one and the earlier
    /// attachment stays behind it in the chain.
    ///
    /// A source is diagnosis, never routing: attaching one does not change
    /// [`is_retryable`](Self::is_retryable),
    /// [`is_ambiguous`](Self::is_ambiguous), [`retry_after`](Self::retry_after)
    /// or the `Display` text.
    ///
    /// ```rust
    /// use telegram_api_client::Error;
    ///
    /// // The retryable case: a connect failure, with its DNS cause kept so a
    /// // log line says why the request never left the machine.
    /// let error = Error::not_sent("dns: name resolution failed")
    ///     .with_source(std::io::Error::other("no such host"));
    /// assert!(error.is_retryable(), "nothing was written, a send may repeat");
    /// let cause = std::error::Error::source(&error).expect("cause kept");
    /// assert_eq!(cause.to_string(), "no such host");
    ///
    /// // A variant without a source slot is unchanged.
    /// let api = Error::Api { code: 400, description: "Bad Request: chat not found".into() }
    ///     .with_source(std::io::Error::other("no such host"));
    /// assert!(matches!(api, Error::Api { .. }));
    /// assert!(std::error::Error::source(&api).is_none());
    ///
    /// // Attaching twice keeps both causes, newest first.
    /// let twice = Error::ambiguous("sendMessage", "timed out")
    ///     .with_source(std::io::Error::other("first"))
    ///     .with_source(std::io::Error::other("second"));
    /// let newest = std::error::Error::source(&twice).expect("newest cause");
    /// assert_eq!(newest.to_string(), "second");
    /// let earlier = std::error::Error::source(newest).expect("earlier cause");
    /// assert_eq!(earlier.to_string(), "first");
    /// ```
    #[must_use]
    pub fn with_source(self, source: impl std::error::Error + Send + Sync + 'static) -> Self {
        self.attach(Box::new(source))
    }

    /// Puts `source` into the source slot of the error, chaining an earlier cause.
    ///
    /// Private half of [`Self::with_source`]: the public method takes an
    /// `impl Error`, transports hand in an already boxed failure, and both paths
    /// must behave identically.
    fn attach(self, source: Box<dyn std::error::Error + Send + Sync + 'static>) -> Self {
        fn slot(
            earlier: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
            latest: Box<dyn std::error::Error + Send + Sync + 'static>,
        ) -> Box<dyn std::error::Error + Send + Sync + 'static> {
            match earlier {
                Some(earlier) => Box::new(SourceChain { latest, earlier }),
                None => latest,
            }
        }

        match self {
            Self::Ambiguous {
                method,
                detail,
                source: earlier,
            } => Self::Ambiguous {
                method,
                detail,
                source: Some(slot(earlier, source)),
            },
            Self::NotSent {
                message,
                source: earlier,
            } => Self::NotSent {
                message,
                source: Some(slot(earlier, source)),
            },
            Self::Transport {
                message,
                source: earlier,
            } => Self::Transport {
                message,
                source: Some(slot(earlier, source)),
            },
            other => other,
        }
    }

    /// How long Telegram asked us to wait, if it said so explicitly.
    ///
    /// The number is the one the server wrote, unclamped: a hint longer than
    /// [`crate::RetryPolicy::max_delay`] stays that long, and it is the policy -
    /// not this accessor - that decides not to retry. A `0` never gets here; see
    /// [`parse_retry_after`] for why a zero wait is treated as no hint.
    ///
    /// ```
    /// use telegram_api_client::Error;
    ///
    /// let error = Error::RateLimited { retry_after: Some(7), code: 429, description: String::new() };
    /// assert_eq!(error.retry_after().map(|d| d.as_secs()), Some(7));
    /// ```
    #[must_use]
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::RateLimited { retry_after, .. } => retry_after.map(Duration::from_secs),
            _ => None,
        }
    }

    /// Classifies a Bot API response body (and optional HTTP status) into an error.
    ///
    /// Transports use this so that every implementation maps `429`,
    /// `ok: false` and 5xx identically. Returns `None` for a successful body.
    ///
    /// ```
    /// use telegram_api_client::Error;
    /// use serde_json::json;
    ///
    /// let body = json!({
    ///     "ok": false,
    ///     "error_code": 429,
    ///     "description": "Too Many Requests: retry after 4",
    ///     "parameters": { "retry_after": 4 }
    /// });
    /// let error = Error::from_api_body(&body, Some(429)).unwrap();
    /// assert_eq!(error.retry_after(), Some(std::time::Duration::from_secs(4)));
    ///
    /// assert!(Error::from_api_body(&json!({"ok": true, "result": true}), Some(200)).is_none());
    /// ```
    #[must_use]
    pub fn from_api_body(body: &Value, http_status: Option<u16>) -> Option<Self> {
        let code = body
            .get("error_code")
            .and_then(Value::as_i64)
            .or_else(|| http_status.map(i64::from));
        let description = body
            .get("description")
            .and_then(Value::as_str)
            .map(ToString::to_string);

        let is_rate_limited = code == Some(429) || http_status == Some(429);
        if is_rate_limited {
            // A zero hint is no hint. It is the shape that says "the budget is
            // exhausted", and retrying with no wait is exactly what burns the
            // attempt budget. Both hint sources go through the same filter, so an
            // explicit `0` falls back to the description just like an absent
            // field does (issue #42).
            let explicit = body
                .get("parameters")
                .and_then(|parameters| parameters.get("retry_after"))
                .and_then(Value::as_u64)
                .and_then(usable_retry_after);
            let retry_after =
                explicit.or_else(|| description.as_deref().and_then(parse_retry_after));
            return Some(Self::RateLimited {
                retry_after,
                code: code.unwrap_or(429),
                description: description.unwrap_or_else(|| "Too Many Requests".to_string()),
            });
        }

        let failed = body.get("ok").and_then(Value::as_bool) == Some(false)
            || http_status.is_some_and(|status| status >= 400);
        if !failed {
            return None;
        }

        Some(Self::Api {
            code: code.unwrap_or(0),
            description: description.unwrap_or_else(|| "unknown error".to_string()),
        })
    }
}

/// Longest description prefix [`parse_retry_after`] will look at.
///
/// The description arrives from a remote endpoint, so its length is not ours to
/// trust: capping the scan turns it into a fixed work budget. The Bot API writes
/// its hint into a single short sentence, so nothing real lives beyond this
/// window, and a marker after byte 512 is not treated as a hint.
///
/// ```
/// use telegram_api_client::{parse_retry_after, MAX_RETRY_HINT_SCAN_BYTES};
///
/// // A marker that starts inside the window is read ...
/// let inside = "x".repeat(MAX_RETRY_HINT_SCAN_BYTES - "retry after 5".len());
/// assert_eq!(parse_retry_after(&format!("{inside}retry after 5")), Some(5));
///
/// // ... one that starts past it is not, however long the description gets.
/// let outside = "x".repeat(MAX_RETRY_HINT_SCAN_BYTES);
/// assert_eq!(parse_retry_after(&format!("{outside}retry after 5")), None);
/// ```
pub const MAX_RETRY_HINT_SCAN_BYTES: usize = 512;

/// Longest run of digits a hint may have.
///
/// `u64` holds 20 decimal digits. A longer run cannot be a wait in seconds, so it
/// is rejected instead of being accumulated with an unbounded amount of work.
const MAX_RETRY_HINT_DIGITS: usize = 20;

/// Whether a wait in seconds is worth handing to a caller.
///
/// `0` is well formed and means "immediately", which as a hint is
/// indistinguishable from no hint at all - a loop that waits nothing and calls
/// again is exactly what the server asked us not to do. Every other value passes
/// through unchanged: the hint is authoritative and is never clamped here, so one
/// longer than [`crate::RetryPolicy::max_delay`] still reaches the caller.
fn usable_retry_after(seconds: u64) -> Option<u64> {
    (seconds > 0).then_some(seconds)
}

/// Case-insensitive ASCII search that allocates nothing.
///
/// Lowercasing the whole description and searching that copy is what made the old
/// parser allocate in proportion to caller-controlled input (issue #43). ASCII
/// lowercasing changes ASCII bytes only and never changes lengths, so a windowed
/// byte comparison finds exactly the same offset without a copy. The needles are
/// ASCII, so a multi-byte UTF-8 sequence in the haystack can never match one.
fn find_ascii_ignore_case(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle))
}

/// The part of `description` that may carry a hint, cut on a char boundary.
fn scan_window(description: &str) -> &str {
    let mut end = description.len().min(MAX_RETRY_HINT_SCAN_BYTES);
    while !description.is_char_boundary(end) {
        end -= 1;
    }
    &description[..end]
}

/// Extracts the number of seconds from a Bot API description.
///
/// Telegram writes the hint in two shapes: `"Too Many Requests: retry after
/// 12"` / `"retry_after 12"` on the modern path, and `"Flood control exceeded.
/// Retry in 12 seconds"` for flood control. All three markers are recognised,
/// case-insensitively; the leftmost one wins, so a description that mentions both
/// is read in the order Telegram wrote it.
///
/// Returns `None` when there is no marker, no number after it, a zero, or a
/// number too wide for `u64`. The scan is bounded twice - at most
/// [`MAX_RETRY_HINT_SCAN_BYTES`] bytes of description and at most 20 digits - and
/// allocates nothing, so a hostile description costs a fixed amount of work per
/// call instead of memory proportional to its length.
///
/// ```rust
/// use telegram_api_client::parse_retry_after;
///
/// assert_eq!(parse_retry_after("Too Many Requests: retry after 5"), Some(5));
/// assert_eq!(parse_retry_after("retry_after 7"), Some(7));
/// assert_eq!(parse_retry_after("Flood control exceeded. Retry in 9s"), Some(9));
/// assert_eq!(parse_retry_after("Flood control exceeded. Retry in 12 seconds"), Some(12));
/// assert_eq!(parse_retry_after("RETRY AFTER: = 3"), Some(3));
///
/// // No marker, no number, zero or out of range: no usable hint.
/// assert_eq!(parse_retry_after("no digits here"), None);
/// assert_eq!(parse_retry_after("Retry in "), None);
/// assert_eq!(parse_retry_after("Retry in 0s"), None);
/// assert_eq!(parse_retry_after("Retry in 99999999999999999999s"), None);
///
/// // The widest hint a `u64` can hold is still a hint.
/// assert_eq!(parse_retry_after("retry after 18446744073709551615"), Some(u64::MAX));
/// ```
#[must_use]
pub fn parse_retry_after(description: &str) -> Option<u64> {
    /// Markers Telegram uses, longest-first is irrelevant because the leftmost
    /// match wins.
    const MARKERS: [&str; 3] = ["retry after", "retry_after", "retry in"];

    let bytes = scan_window(description).as_bytes();
    let (index, marker_len) = MARKERS
        .iter()
        .filter_map(|needle| {
            find_ascii_ignore_case(bytes, needle.as_bytes()).map(|index| (index, needle.len()))
        })
        .min_by_key(|(index, _)| *index)?;

    let mut rest = &bytes[index + marker_len..];
    while let Some((first, tail)) = rest.split_first() {
        if matches!(*first, b' ' | b'\t' | b':' | b'=') {
            rest = tail;
        } else {
            break;
        }
    }

    let width = rest.iter().take_while(|byte| byte.is_ascii_digit()).count();
    if width == 0 {
        return None;
    }

    // Leading zeros are padding, not a larger number: they do not count towards
    // the width, so `0000...5` still reads as 5 instead of being rejected as an
    // over-long run. The run itself stays bounded by the scan window, so a
    // megabyte of zeros costs the same as a short description.
    let mut significant = &rest[..width];
    while let Some((first, tail)) = significant.split_first() {
        if *first == b'0' {
            significant = tail;
        } else {
            break;
        }
    }
    if significant.len() > MAX_RETRY_HINT_DIGITS {
        return None;
    }
    let mut seconds: u64 = 0;
    for byte in significant {
        seconds = seconds
            .checked_mul(10)?
            .checked_add(u64::from(byte - b'0'))?;
    }
    usable_retry_after(seconds)
}

/// Convenience alias for the error type of this crate.
///
/// ```rust
/// use telegram_api_client::Result;
///
/// fn answer() -> Result<u32> {
///     Ok(42)
/// }
///
/// assert_eq!(answer().expect("ok"), 42);
/// ```
pub type Result<T> = std::result::Result<T, Error>;

/// Two attached causes held in one linear chain.
///
/// [`Error::with_source`] builds this when a source is attached to an error that
/// already had one, so the second attachment does not silently erase the first.
/// `Display` and `Debug` show the newest failure; `source()` walks on to the
/// earlier attachment. `std::error::Error` chains are linear, so when the newest
/// failure carries a cause of its own, that cause is walked instead - a chain has
/// one branch, not two.
#[derive(Debug)]
struct SourceChain {
    latest: Box<dyn std::error::Error + Send + Sync + 'static>,
    earlier: Box<dyn std::error::Error + Send + Sync + 'static>,
}

impl std::fmt::Display for SourceChain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.latest)
    }
}

impl std::error::Error for SourceChain {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        std::error::Error::source(self.latest.as_ref()).or(Some(self.earlier.as_ref()))
    }
}
