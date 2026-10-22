//! Pluggable HTTP transport.

use crate::error::Result;

/// "POST JSON with no auth header" — Telegram authenticates via the bot token
/// embedded in the URL path.
#[allow(async_fn_in_trait)]
pub trait Transport {
    /// POSTs `body` as JSON and returns the decoded response.
    fn post_json(
        &self,
        url: &str,
        body: &serde_json::Value,
    ) -> impl std::future::Future<Output = Result<serde_json::Value>>;
}

/// `reqwest` transport behind the `http` feature.
#[cfg(feature = "http")]
#[derive(Debug, Clone, Default)]
pub struct ReqwestTransport {
    client: reqwest::Client,
}

#[cfg(feature = "http")]
impl ReqwestTransport {
    /// Creates a transport with a default client.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(feature = "http")]
impl Transport for ReqwestTransport {
    async fn post_json(&self, url: &str, body: &serde_json::Value) -> Result<serde_json::Value> {
        let response = self
            .client
            .post(url)
            .json(body)
            .send()
            .await
            .map_err(|e| crate::error::Error::Transport(e.to_string()))?;

        let text = response
            .text()
            .await
            .map_err(|e| crate::error::Error::Transport(e.to_string()))?;
        let value: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| crate::error::Error::Payload(e.to_string()))?;

        if value.get("ok") == Some(&serde_json::Value::Bool(false)) {
            return Err(crate::error::Error::Api {
                code: value["error_code"].as_i64().unwrap_or(0),
                description: value["description"]
                    .as_str()
                    .unwrap_or("unknown error")
                    .to_string(),
            });
        }
        Ok(value)
    }
}
