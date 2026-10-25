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

    /// Attaches the underlying failure to an ambiguous error.
    #[must_use]
    pub fn with_source(self, source: impl std::error::Error + Send + Sync + 'static) -> Self {
        match self {
            Self::Ambiguous { method, detail, .. } => Self::Ambiguous {
                method,
                detail,
                source: Some(Box::new(source)),
            },
            other => other,
        }
    }

    /// How long Telegram asked us to wait, if it said so explicitly.
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
            let explicit = body
                .get("parameters")
                .and_then(|parameters| parameters.get("retry_after"))
                .and_then(Value::as_u64);
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

/// Extracts the number of seconds from a Bot API description.
///
/// Telegram writes the hint in two shapes: `"Too Many Requests: retry after
/// 12"` / `"retry_after 12"` on the modern path, and `"Flood control exceeded.
/// Retry in 12 seconds"` for flood control. All three markers are recognised;
/// the leftmost one wins, so a description that mentions both is read in the
/// order Telegram wrote it.
///
/// Returns `None` when there is no marker, no positive number after it, or a
/// number that does not fit in `u64`.
///
/// ```rust
/// use telegram_api_client::parse_retry_after;
///
/// assert_eq!(parse_retry_after("Too Many Requests: retry after 5"), Some(5));
/// assert_eq!(parse_retry_after("retry_after 7"), Some(7));
/// assert_eq!(parse_retry_after("Flood control exceeded. Retry in 9s"), Some(9));
/// assert_eq!(parse_retry_after("Flood control exceeded. Retry in 12 seconds"), Some(12));
///
/// // No marker, no number, zero or out of range: no usable hint.
/// assert_eq!(parse_retry_after("no digits here"), None);
/// assert_eq!(parse_retry_after("Retry in "), None);
/// assert_eq!(parse_retry_after("Retry in 0s"), None);
/// assert_eq!(parse_retry_after("Retry in 99999999999999999999s"), None);
/// ```
#[must_use]
pub fn parse_retry_after(description: &str) -> Option<u64> {
    /// Markers Telegram uses, longest-first is irrelevant because the leftmost
    /// match wins.
    const MARKERS: [&str; 3] = ["retry after", "retry_after", "retry in"];

    let lower = description.to_ascii_lowercase();
    let (index, marker) = MARKERS
        .iter()
        .filter_map(|needle| lower.find(needle).map(|index| (index, *needle)))
        .min_by_key(|(index, _)| *index)?;
    let rest = lower[index + marker.len()..].trim_start_matches([' ', '\t', ':', '=']);
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse::<u64>().ok().filter(|seconds| *seconds > 0)
}

/// Convenience alias.
pub type Result<T> = std::result::Result<T, Error>;
