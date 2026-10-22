//! Error type.

use thiserror::Error;

/// Failures from verification, parsing or the Bot API.
#[derive(Debug, Error)]
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
    /// Payload could not be parsed.
    #[error("invalid payload: {0}")]
    Payload(String),
    /// Transport failure.
    #[error("transport error: {0}")]
    Transport(String),
    /// The Bot API answered with `ok: false`.
    #[error("telegram api error {code}: {description}")]
    Api {
        /// Numeric error code from Telegram.
        code: i64,
        /// Human readable description.
        description: String,
    },
}

/// Convenience alias.
pub type Result<T> = std::result::Result<T, Error>;
