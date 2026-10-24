//! Pluggable HTTP transport, plus an in-memory mock for tests and doctests.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

use serde_json::{json, Value};

use crate::error::{Error, Result};

/// "POST JSON with no auth header" — Telegram authenticates via the bot token
/// embedded in the URL path.
///
/// Implement this to plug in your own HTTP stack, a proxy, or a recorder:
///
/// ```
/// use serde_json::Value;
/// use telegram_api_client::{Result, Transport};
///
/// struct Fixed;
///
/// impl Transport for Fixed {
///     fn post_json(
///         &self,
///         _url: &str,
///         _body: &Value,
///     ) -> impl std::future::Future<Output = Result<Value>> {
///         std::future::ready(Ok(serde_json::json!({"ok": true, "result": true})))
///     }
/// }
///
/// # async fn demo() -> Result<()> {
/// let response = Fixed
///     .post_json("https://api.telegram.org/botx/getMe", &serde_json::json!({}))
///     .await?;
/// assert_eq!(response["ok"], true);
/// # Ok(()) }
/// ```
#[allow(async_fn_in_trait)]
pub trait Transport {
    /// POSTs `body` as JSON and returns the decoded response.
    ///
    /// # Errors
    ///
    /// Whatever the implementation produces: network failures, non-JSON
    /// bodies, and Bot API `ok: false` answers.
    fn post_json(
        &self,
        url: &str,
        body: &serde_json::Value,
    ) -> impl std::future::Future<Output = Result<serde_json::Value>>;
}

/// `reqwest` transport behind the `http` feature.
///
/// Maps `429` answers to [`crate::Error::RateLimited`] (keeping
/// `parameters.retry_after`), 5xx to a retryable [`crate::Error::Api`], and
/// non-JSON bodies to [`crate::Error::Payload`].
#[cfg(feature = "http")]
#[cfg_attr(
    feature = "http",
    doc = r#"
```
# use telegram_api_client::{ReqwestTransport, TelegramClient};
# async fn demo() -> telegram_api_client::Result<()> {
let client = TelegramClient::new("123:ABC", ReqwestTransport::new());
client.get_me().await?;
# Ok(()) }
```
"#
)]
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
            .map_err(|error| Error::Transport {
                message: error.to_string(),
                source: Some(Box::new(error)),
            })?;

        let status = response.status();
        let text = response.text().await.map_err(|error| Error::Transport {
            message: error.to_string(),
            source: Some(Box::new(error)),
        })?;

        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            // A proxy or overload page may answer 429/5xx without JSON; keep the
            // status meaningful instead of reporting an opaque parse error.
            return Err(
                Error::from_api_body(&Value::Null, Some(status.as_u16())).unwrap_or_else(|| {
                    Error::Payload {
                        message: format!(
                            "non-JSON response (status {}): {}",
                            status.as_u16(),
                            truncate(&text, 200)
                        ),
                        source: serde_json::from_str::<Value>(&text).err(),
                    }
                }),
            );
        };

        if let Some(error) = Error::from_api_body(&value, Some(status.as_u16())) {
            return Err(error);
        }
        Ok(value)
    }
}

/// Truncates on a character boundary so error messages stay readable.
#[cfg(feature = "http")]
fn truncate(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let mut out: String = text.chars().take(limit).collect();
    out.push('…');
    out
}

/// One recorded request.
///
/// ```
/// use telegram_api_client::{Call, MockTransport, Transport};
///
/// # async fn demo() {
/// let transport = MockTransport::new();
/// transport.post_json("https://x/bott/sendMessage", &serde_json::json!({"chat_id": 1})).await.unwrap();
/// let call = transport.last_call().unwrap();
/// assert_eq!(call.method(), "sendMessage");
/// assert_eq!(call.body["chat_id"], 1);
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    /// Full URL the client posted to.
    pub url: String,
    /// JSON body that was posted.
    pub body: Value,
}

impl Call {
    /// The Bot API method name, i.e. the last URL segment.
    #[must_use]
    pub fn method(&self) -> &str {
        self.url.rsplit('/').next().unwrap_or_default()
    }
}

/// Canned answers for [`MockTransport`].
///
/// ```
/// use telegram_api_client::MockReply;
///
/// let reply = MockReply::result(serde_json::json!({"message_id": 5}));
/// assert_eq!(reply.clone().into_result().unwrap()["result"]["message_id"], 5);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MockReply {
    /// `{"ok": true, "result": <value>}`.
    Result(Value),
    /// A body returned verbatim.
    Raw(Value),
    /// `{"ok": false, "error_code": code, "description": ...}`.
    Api {
        /// Error code Telegram would send.
        code: i64,
        /// Error description Telegram would send.
        description: String,
    },
    /// A 429 body with an optional `parameters.retry_after`.
    RateLimited {
        /// Seconds Telegram would ask us to wait.
        retry_after: Option<u64>,
        /// Error description Telegram would send.
        description: String,
    },
    /// A transport-level failure (no response at all).
    Transport(String),
}

impl MockReply {
    /// A successful answer wrapping `result`.
    #[must_use]
    pub fn result(result: Value) -> Self {
        Self::Result(result)
    }

    /// A successful answer with an empty object result.
    ///
    /// ```
    /// use telegram_api_client::MockReply;
    ///
    /// assert_eq!(MockReply::ok().into_result().unwrap()["result"], serde_json::json!({}));
    /// ```
    #[must_use]
    pub fn ok() -> Self {
        Self::Result(json!({}))
    }

    /// A Bot API error.
    #[must_use]
    pub fn api(code: i64, description: impl Into<String>) -> Self {
        Self::Api {
            code,
            description: description.into(),
        }
    }

    /// A 429 with `retry_after` seconds.
    #[must_use]
    pub fn rate_limited(retry_after: u64) -> Self {
        Self::RateLimited {
            retry_after: Some(retry_after),
            description: "Too Many Requests: retry later".to_string(),
        }
    }

    /// The value this reply resolves to, using the same classification the
    /// HTTP transport applies.
    ///
    /// # Errors
    ///
    /// The error encoded by this reply, if it is not a success.
    ///
    /// ```
    /// use telegram_api_client::MockReply;
    ///
    /// let error = MockReply::api(400, "Bad Request").into_result().unwrap_err();
    /// assert!(error.to_string().contains("400"));
    /// ```
    pub fn into_result(self) -> Result<Value> {
        match self {
            Self::Result(result) => Ok(json!({ "ok": true, "result": result })),
            Self::Raw(value) => match Error::from_api_body(&value, None) {
                Some(error) => Err(error),
                None => Ok(value),
            },
            Self::Api { code, description } => Err(Error::Api { code, description }),
            Self::RateLimited {
                retry_after,
                description,
            } => Err(Error::RateLimited {
                retry_after,
                code: 429,
                description,
            }),
            Self::Transport(message) => Err(Error::Transport {
                message,
                source: None,
            }),
        }
    }
}

#[derive(Debug, Default)]
struct Inner {
    calls: Vec<Call>,
    queue: VecDeque<MockReply>,
}

/// In-memory [`Transport`] that records every call and replays canned replies.
///
/// Cheap to clone and `Send + Sync`, so the same transport can be hammered from
/// many tasks at once.
///
/// ```
/// use telegram_api_client::{MockReply, MockTransport, TelegramClient};
///
/// # async fn demo() -> telegram_api_client::Result<()> {
/// let transport = MockTransport::new();
/// transport.push(MockReply::result(serde_json::json!({"message_id": 1})));
/// let client = TelegramClient::new("123:ABC", transport.clone());
///
/// let _ = client.send_message(42, "hi").await?;
/// assert_eq!(transport.calls().len(), 1);
/// # Ok(()) }
/// ```
#[derive(Debug, Clone, Default)]
pub struct MockTransport {
    inner: Arc<Mutex<Inner>>,
}

impl MockTransport {
    /// A transport with an empty queue; every call answers `{"ok":true,"result":{}}`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Queues one reply.
    ///
    /// ```
    /// use telegram_api_client::{MockReply, MockTransport};
    ///
    /// let transport = MockTransport::new();
    /// transport.push(MockReply::result(serde_json::json!({"message_id": 1})));
    /// assert!(transport.is_empty(), "queueing is not sending");
    /// ```
    pub fn push(&self, reply: impl Into<MockReply>) {
        self.lock().queue.push_back(reply.into());
    }

    /// Appends every reply in `replies`.
    ///
    /// ```
    /// use telegram_api_client::{MockReply, MockTransport};
    ///
    /// let transport = MockTransport::new();
    /// transport.extend([MockReply::ok(), MockReply::api(400, "Bad Request")]);
    /// ```
    pub fn extend(&self, replies: impl IntoIterator<Item = MockReply>) {
        self.lock().queue.extend(replies);
    }

    /// Every request recorded so far, oldest first.
    #[must_use]
    pub fn calls(&self) -> Vec<Call> {
        self.lock().calls.clone()
    }

    /// The most recent request, if any.
    #[must_use]
    pub fn last_call(&self) -> Option<Call> {
        self.lock().calls.last().cloned()
    }

    /// Number of recorded requests.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lock().calls.len()
    }

    /// Whether nothing has been sent yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        // A poisoned lock still holds intact data; recovering keeps the mock
        // usable after a panicking test without `unwrap`.
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl From<Value> for MockReply {
    fn from(value: Value) -> Self {
        Self::result(value)
    }
}

impl Transport for MockTransport {
    fn post_json(
        &self,
        url: &str,
        body: &serde_json::Value,
    ) -> impl std::future::Future<Output = Result<serde_json::Value>> {
        let reply = {
            let mut inner = self.lock();
            inner.calls.push(Call {
                url: url.to_string(),
                body: body.clone(),
            });
            inner.queue.pop_front()
        };
        std::future::ready(reply.unwrap_or_else(MockReply::ok).into_result())
    }
}
