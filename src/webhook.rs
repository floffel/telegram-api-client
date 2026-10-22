//! Verification of webhook authenticity.
//!
//! Telegram supports two independent mechanisms: a secret token header and a
//! webhook URL that contains a value only the bot knows. This module checks
//! both, in constant time.

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::error::{Error, Result};

/// Verifies secret token and derived path for incoming webhook calls.
#[derive(Clone)]
pub struct WebhookGuard {
    secret: String,
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
    /// Rejects an empty secret: an empty configured token would accept an
    /// empty header.
    pub fn new(secret: impl Into<String>) -> crate::error::Result<Self> {
        let secret = secret.into();
        if secret.is_empty() {
            return Err(crate::error::Error::EmptySecret);
        }
        let digest = Sha256::digest(secret.as_bytes());
        let derived_path = format!("/{}", hex::encode(digest));
        Ok(Self {
            secret,
            derived_path,
        })
    }

    /// The path that must appear in `setWebhook`.
    #[must_use]
    pub fn derived_path(&self) -> &str {
        &self.derived_path
    }

    /// Checks the `X-Telegram-Bot-Api-Secret-Token` header.
    ///
    /// # Errors
    ///
    /// Returns [`Error::SecretMismatch`] when the header is absent or differs.
    pub fn verify_secret(&self, header: Option<&str>) -> Result<()> {
        let Some(value) = header else {
            return Err(Error::SecretMismatch);
        };
        if self.secret.as_bytes().ct_eq(value.as_bytes()).into() {
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
    /// Propagates verification errors and maps JSON failures to [`Error::Payload`].
    pub fn verify_and_parse(
        &self,
        secret_header: Option<&str>,
        path: &str,
        body: &[u8],
    ) -> Result<crate::update::Update> {
        self.verify_secret(secret_header)?;
        self.verify_path(path)?;
        serde_json::from_slice(body).map_err(|e| Error::Payload(e.to_string()))
    }
}
