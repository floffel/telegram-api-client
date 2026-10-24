//! Webhook lifecycle: register, delete, inspect.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::types::UpdateKind;

/// Longest secret token Telegram accepts.
pub const MAX_SECRET_TOKEN: usize = 256;
/// Upper bound for `max_connections` per the Bot API.
pub const MAX_WEBHOOK_CONNECTIONS: u16 = 100;

/// Rejects secrets Telegram would reject, and secrets that are trivially weak.
fn validate_secret(secret: &str) -> Result<()> {
    let length = secret.chars().count();
    if length == 0 {
        return Err(Error::InvalidRequest {
            field: "secret_token",
            reason: "must not be empty".to_string(),
        });
    }
    if length > MAX_SECRET_TOKEN {
        return Err(Error::InvalidRequest {
            field: "secret_token",
            reason: format!("must be at most {MAX_SECRET_TOKEN} characters, got {length}"),
        });
    }
    if !secret
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(Error::InvalidRequest {
            field: "secret_token",
            reason: "may only contain A-Z, a-z, 0-9, '_' and '-'".to_string(),
        });
    }
    Ok(())
}

/// `setWebhook` body.
///
/// The secret token is never rendered by `Debug`.
///
/// ```
/// use telegram_api_client::SetWebhook;
///
/// let request = SetWebhook::new("https://example.com/hook")
///     .with_secret("s3cret")
///     .max_connections(40);
/// let json = serde_json::to_value(&request).unwrap();
/// assert_eq!(json["max_connections"], 40);
///
/// // The token must not leak into logs.
/// assert!(!format!("{request:?}").contains("s3cret"));
/// ```
#[derive(Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct SetWebhook {
    /// HTTPS URL Telegram should POST updates to.
    pub url: String,
    /// Value sent back in `X-Telegram-Bot-Api-Secret-Token`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_token: Option<String>,
    /// Fixed IP address or hostname to resolve against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip_address: Option<String>,
    /// Simultaneous connections Telegram may open, `1..=100`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_connections: Option<u16>,
    /// Update kinds to deliver; empty means all.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_updates: Vec<UpdateKind>,
    /// Drop queued updates when switching the webhook.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drop_pending_updates: Option<bool>,
}

impl fmt::Debug for SetWebhook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SetWebhook")
            .field("url", &self.url)
            .field(
                "secret_token",
                &self.secret_token.as_ref().map(|_| "<redacted>"),
            )
            .field("ip_address", &self.ip_address)
            .field("max_connections", &self.max_connections)
            .field("allowed_updates", &self.allowed_updates)
            .field("drop_pending_updates", &self.drop_pending_updates)
            .finish()
    }
}

impl SetWebhook {
    /// Registers `url`; everything else stays at Telegram's defaults.
    #[must_use]
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            ..Self::default()
        }
    }

    /// Sets the secret token echoed back in the header.
    #[must_use]
    pub fn with_secret(mut self, secret: impl Into<String>) -> Self {
        self.secret_token = Some(secret.into());
        self
    }

    /// Limits the connection pool Telegram uses.
    #[must_use]
    pub fn max_connections(mut self, connections: u16) -> Self {
        self.max_connections = Some(connections);
        self
    }

    /// Restricts delivered update kinds.
    #[must_use]
    pub fn allowed_updates(mut self, kinds: Vec<UpdateKind>) -> Self {
        self.allowed_updates = kinds;
        self
    }

    /// Drops updates queued for the previous webhook.
    #[must_use]
    pub fn dropping_pending(mut self) -> Self {
        self.drop_pending_updates = Some(true);
        self
    }

    /// Checks the URL, secret, connection count and update kinds.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for a non-HTTPS or empty URL, a secret that
    /// is empty, too long or has illegal characters, a connection count of `0`
    /// or above [`MAX_WEBHOOK_CONNECTIONS`], or an empty update kind.
    pub fn validate(&self) -> Result<()> {
        if !self.url.starts_with("https://") || self.url.len() <= "https://".len() {
            return Err(Error::InvalidRequest {
                field: "url",
                reason: "must be an absolute https:// URL".to_string(),
            });
        }
        if let Some(secret) = self.secret_token.as_deref() {
            validate_secret(secret)?;
        }
        if self
            .max_connections
            .is_some_and(|count| count == 0 || count > MAX_WEBHOOK_CONNECTIONS)
        {
            return Err(Error::InvalidRequest {
                field: "max_connections",
                reason: format!("must be within 1..={MAX_WEBHOOK_CONNECTIONS}"),
            });
        }
        if self
            .allowed_updates
            .iter()
            .any(|kind| kind.as_str().is_empty())
        {
            return Err(Error::InvalidRequest {
                field: "allowed_updates",
                reason: "update kinds must not be empty".to_string(),
            });
        }
        Ok(())
    }
}

/// `deleteWebhook` body.
///
/// ```
/// use telegram_api_client::DeleteWebhook;
///
/// assert_eq!(
///     serde_json::to_string(&DeleteWebhook::dropping_pending()).unwrap(),
///     r#"{"drop_pending_updates":true}"#
/// );
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct DeleteWebhook {
    /// Drop updates queued for the webhook.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drop_pending_updates: Option<bool>,
}

impl DeleteWebhook {
    /// Deletes the webhook, keeping queued updates.
    ///
    /// ```
    /// use telegram_api_client::DeleteWebhook;
    ///
    /// assert_eq!(serde_json::to_string(&DeleteWebhook::new()).unwrap(), "{}");
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Deletes the webhook and drops queued updates.
    #[must_use]
    pub fn dropping_pending() -> Self {
        Self {
            drop_pending_updates: Some(true),
        }
    }

    /// Nothing to validate — all fields are optional.
    ///
    /// # Errors
    ///
    /// Never; the method exists so every request type validates the same way.
    pub fn validate(&self) -> Result<()> {
        Ok(())
    }
}

/// The answer of `getWebhookInfo`.
///
/// ```
/// use telegram_api_client::WebhookInfo;
///
/// let info: WebhookInfo = serde_json::from_str(
///     r#"{"url":"https://example.com/hook","pending_update_count":3,"unknown_future_field":1}"#,
/// ).unwrap();
/// assert_eq!(info.pending_update_count, 3);
/// assert!(!info.has_custom_certificate);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct WebhookInfo {
    /// Current webhook URL; empty when none is set.
    #[serde(default, alias = "URL", alias = "webhookUrl")]
    pub url: String,
    /// Whether a self-signed certificate is in use.
    #[serde(default, alias = "hasCustomCertificate")]
    pub has_custom_certificate: bool,
    /// Updates waiting to be delivered.
    #[serde(default, alias = "pendingUpdateCount")]
    pub pending_update_count: u32,
    /// Configured IP address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip_address: Option<String>,
    /// Unix time of the last delivery error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error_date: Option<i64>,
    /// Description of the last delivery error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error_message: Option<String>,
    /// Unix time of the last error while syncing updates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_synchronization_error_date: Option<i64>,
    /// Negotiated connection pool size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_connections: Option<u16>,
    /// Update kinds being delivered.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_updates: Vec<UpdateKind>,
}

impl WebhookInfo {
    /// Whether a webhook is currently registered.
    ///
    /// ```
    /// use telegram_api_client::WebhookInfo;
    ///
    /// assert!(!WebhookInfo::default().is_registered());
    /// let info = WebhookInfo { url: "https://example.com/hook".into(), ..Default::default() };
    /// assert!(info.is_registered());
    /// ```
    #[must_use]
    pub fn is_registered(&self) -> bool {
        !self.url.is_empty()
    }

    /// Whether the most recent delivery failed.
    ///
    /// ```
    /// use telegram_api_client::WebhookInfo;
    ///
    /// let info = WebhookInfo { last_error_message: Some("timeout".into()), ..Default::default() };
    /// assert!(info.has_error());
    /// ```
    #[must_use]
    pub fn has_error(&self) -> bool {
        self.last_error_message.is_some()
    }
}
