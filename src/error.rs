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
/// assert_eq!(error.retry_after(), None);
///
/// let limited = Error::RateLimited { retry_after: Some(3), code: 429, description: "Too Many Requests".into() };
/// assert!(limited.is_retryable());
/// assert_eq!(limited.retry_after(), Some(std::time::Duration::from_secs(3)));
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
    #[error("transport error: {message}")]
    Transport {
        /// What went wrong, in prose.
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
    /// Whether re-sending the exact same request can plausibly succeed.
    ///
    /// `true` for rate limits, 5xx answers and transport failures; `false` for
    /// everything a retry cannot fix (bad requests, auth, parse errors).
    ///
    /// ```
    /// use telegram_api_client::Error;
    ///
    /// assert!(Error::Api { code: 502, description: "Bad Gateway".into() }.is_retryable());
    /// assert!(Error::Transport { message: "dns".into(), source: None }.is_retryable());
    /// assert!(!Error::Api { code: 403, description: "Forbidden".into() }.is_retryable());
    /// ```
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::RateLimited { .. } | Self::Transport { .. } => true,
            Self::Api { code, .. } => *code == 429 || *code >= 500,
            Self::EmptySecret
            | Self::SecretMismatch
            | Self::PathMismatch
            | Self::Payload { .. }
            | Self::InvalidRequest { .. }
            | Self::Unexpected { .. } => false,
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

/// Extracts the number from `"Too Many Requests: retry after 12"`.
///
/// Returns `None` when there is no marker or no positive number after it.
///
/// ```
/// use telegram_api_client::parse_retry_after;
///
/// assert_eq!(parse_retry_after("Too Many Requests: retry after 5"), Some(5));
/// assert_eq!(parse_retry_after("Flood control exceeded. Retry in 9s"), None);
/// assert_eq!(parse_retry_after("no digits here"), None);
/// ```
#[must_use]
pub fn parse_retry_after(description: &str) -> Option<u64> {
    let lower = description.to_ascii_lowercase();
    let marker = ["retry after ", "retry_after "]
        .iter()
        .find_map(|needle| lower.find(needle).map(|index| index + needle.len()))?;
    let digits: String = lower[marker..]
        .chars()
        .skip_while(|c| c.is_whitespace())
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse::<u64>().ok().filter(|seconds| *seconds > 0)
}

/// Convenience alias.
pub type Result<T> = std::result::Result<T, Error>;
