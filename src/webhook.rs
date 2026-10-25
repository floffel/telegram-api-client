//! Verification of webhook authenticity.
//!
//! Telegram supports two independent mechanisms: a secret token header and a
//! webhook URL that contains a value only the bot knows. This module checks
//! both, in constant time.

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::error::{Error, Result};
use crate::webhook_api::validate_secret;

/// Verifies secret token and derived path for incoming webhook calls.
///
/// Both checks are constant time, and the configured secret is kept only as a
/// SHA-256 digest, so [`Debug`](std::fmt::Debug) has nothing to redact and the
/// comparison cannot leak the secret's length: a 3-character and a 200-character
/// token are both 32 bytes on the wire and in memory.
///
/// The secret must be one Telegram accepts (`1..=256` characters of
/// `A-Z a-z 0-9 _ -`), because a secret Telegram rejects can never arrive in the
/// header — a guard built from one could not verify anything.
///
/// ```
/// use telegram_api_client::WebhookGuard;
///
/// let guard = WebhookGuard::new("s3cret").unwrap();
/// // Register `guard.derived_path()` with setWebhook and verify both checks
/// // before trusting an incoming request.
/// assert!(guard.verify_secret(Some("s3cret")).is_ok());
/// assert!(guard.verify_path(guard.derived_path()).is_ok());
///
/// // Values Telegram would never let you register are refused here, not
/// // silently accepted and then never matched.
/// assert!(WebhookGuard::new("has space").is_err());
/// assert!(WebhookGuard::new("ümlaut").is_err());
/// assert!(WebhookGuard::new("a".repeat(257)).is_err());
/// ```
#[derive(Clone)]
pub struct WebhookGuard {
    /// `sha256(secret)` — the only form the secret takes inside the guard.
    secret_digest: [u8; 32],
    derived_path: String,
}

impl std::fmt::Debug for WebhookGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebhookGuard").finish_non_exhaustive()
    }
}

impl WebhookGuard {
    /// Derives the expected path from `secret`:
    /// `/<sha256(secret) hex>`.
    ///
    /// # Errors
    ///
    /// Rejects an empty secret with [`Error::EmptySecret`] — an empty
    /// configured token would accept an empty header — and otherwise applies
    /// Telegram's own secret rules through the same validator `setWebhook`
    /// uses: at most
    /// [`MAX_SECRET_TOKEN`](crate::webhook_api::MAX_SECRET_TOKEN) characters
    /// of `A-Z a-z 0-9 _ -`.
    ///
    /// ```
    /// use telegram_api_client::WebhookGuard;
    ///
    /// assert!(WebhookGuard::new("").is_err());
    /// let guard = WebhookGuard::new("s3cret").unwrap();
    /// assert_eq!(guard.derived_path().len(), 65);
    ///
    /// // Telegram's documented alphabet and limit.
    /// assert!(WebhookGuard::new("aZ0_-").is_ok());
    /// assert!(WebhookGuard::new("a".repeat(256)).is_ok());
    /// assert!(WebhookGuard::new("a".repeat(257)).is_err());
    /// ```
    pub fn new(secret: impl Into<String>) -> crate::error::Result<Self> {
        let secret = secret.into();
        if secret.is_empty() {
            return Err(Error::EmptySecret);
        }
        // Same rules `setWebhook` applies: rejecting here means a guard can
        // never be built from a token Telegram would refuse to register.
        validate_secret(&secret)?;
        let secret_digest: [u8; 32] = Sha256::digest(secret.as_bytes()).into();
        let derived_path = format!("/{}", hex::encode(secret_digest));
        Ok(Self {
            secret_digest,
            derived_path,
        })
    }

    /// The path that must appear in `setWebhook`.
    ///
    /// ```
    /// use telegram_api_client::WebhookGuard;
    ///
    /// let guard = WebhookGuard::new("s3cret").unwrap();
    /// assert!(guard.derived_path().starts_with('/'));
    /// ```
    #[must_use]
    pub fn derived_path(&self) -> &str {
        &self.derived_path
    }

    /// Checks the `X-Telegram-Bot-Api-Secret-Token` header.
    ///
    /// The header is hashed and the two digests are compared in constant time,
    /// so the check reveals neither the secret's length nor how much of a
    /// candidate matched. A missing header, a prefix or a NUL-suffixed value
    /// never matches.
    ///
    /// # Errors
    ///
    /// Returns [`Error::SecretMismatch`] when the header is absent or differs.
    ///
    /// ```
    /// use telegram_api_client::WebhookGuard;
    ///
    /// let guard = WebhookGuard::new("s3cret").unwrap();
    /// assert!(guard.verify_secret(Some("s3cret")).is_ok());
    /// assert!(guard.verify_secret(Some("s3cre")).is_err());
    /// assert!(guard.verify_secret(Some("s3crett")).is_err());
    /// assert!(guard.verify_secret(None).is_err());
    /// ```
    pub fn verify_secret(&self, header: Option<&str>) -> Result<()> {
        let Some(value) = header else {
            return Err(Error::SecretMismatch);
        };
        // Both sides are fixed-size digests, so the comparison is
        // length-oblivious: `ct_eq` on the raw secret returned as soon as the
        // lengths differed, which leaked the configured token's length.
        let candidate: [u8; 32] = Sha256::digest(value.as_bytes()).into();
        if bool::from(self.secret_digest.ct_eq(&candidate)) {
            Ok(())
        } else {
            Err(Error::SecretMismatch)
        }
    }

    /// Checks the request path against the derived path.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PathMismatch`] when it differs.
    ///
    /// ```
    /// use telegram_api_client::WebhookGuard;
    ///
    /// let guard = WebhookGuard::new("s3cret").unwrap();
    /// assert!(guard.verify_path(guard.derived_path()).is_ok());
    /// assert!(guard.verify_path("/elsewhere").is_err());
    /// ```
    pub fn verify_path(&self, path: &str) -> Result<()> {
        if self.derived_path.as_bytes().ct_eq(path.as_bytes()).into() {
            Ok(())
        } else {
            Err(Error::PathMismatch)
        }
    }

    /// Verifies both and parses the update.
    ///
    /// # Errors
    ///
    /// Propagates verification errors and maps JSON failures to
    /// [`Error::Payload`] with the parser error as
    /// [`source`](std::error::Error::source).
    ///
    /// ```
    /// use telegram_api_client::WebhookGuard;
    ///
    /// let guard = WebhookGuard::new("s3cret").unwrap();
    /// let body = br#"{"update_id":1,"message":{"message_id":2,"chat":{"id":-3},"text":"hi"}}"#;
    ///
    /// let update = guard
    ///     .verify_and_parse(Some("s3cret"), guard.derived_path(), body)
    ///     .unwrap();
    /// assert_eq!(update.text(), Some("hi"));
    ///
    /// // A wrong secret never reaches the parser.
    /// assert!(guard
    ///     .verify_and_parse(Some("nope"), guard.derived_path(), body)
    ///     .is_err());
    /// ```
    pub fn verify_and_parse(
        &self,
        secret_header: Option<&str>,
        path: &str,
        body: &[u8],
    ) -> Result<crate::update::Update> {
        self.verify_secret(secret_header)?;
        self.verify_path(path)?;
        serde_json::from_slice(body).map_err(|error| Error::Payload {
            message: error.to_string(),
            source: Some(error),
        })
    }
}
