//! Bot API client.

use serde_json::json;

use crate::error::Result;
use crate::transport::Transport;

/// Client bound to one bot token.
pub struct TelegramClient<T: Transport> {
    token: String,
    base_url: String,
    transport: T,
}

impl<T: Transport> TelegramClient<T> {
    /// Creates a client for `token`.
    pub fn new(token: impl Into<String>, transport: T) -> Self {
        Self {
            token: token.into(),
            base_url: "https://api.telegram.org".to_string(),
            transport,
        }
    }

    /// Overrides the base URL (tests, proxies).
    #[must_use]
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Endpoint for a Bot API method.
    #[must_use]
    pub fn method_url(&self, method: &str) -> String {
        format!(
            "{}/bot{}/{}",
            self.base_url.trim_end_matches('/'),
            self.token,
            method
        )
    }

    /// Sends a plain text message.
    ///
    /// # Errors
    ///
    /// Propagates transport and Bot API errors.
    pub async fn send_message(&self, chat_id: i64, text: &str) -> Result<serde_json::Value> {
        let payload = json!({ "chat_id": chat_id, "text": text });
        self.transport
            .post_json(&self.method_url("sendMessage"), &payload)
            .await
    }

    /// Fetches bot identity — the cheapest call to validate a token.
    ///
    /// # Errors
    ///
    /// Propagates transport and Bot API errors.
    pub async fn get_me(&self) -> Result<serde_json::Value> {
        self.transport
            .post_json(&self.method_url("getMe"), &json!({}))
            .await
    }
}

impl<T: Transport> std::fmt::Debug for TelegramClient<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print the bot token.
        f.debug_struct("TelegramClient")
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}
