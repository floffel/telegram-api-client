//! Pluggable HTTP transport, plus an in-memory mock for tests and doctests.

use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};

use serde_json::{json, Map, Value};

use crate::error::{Error, Result};

/// One binary part of a multipart upload.
///
/// Built by [`crate::UploadFile`] and named after the Bot API field it fills
/// (`photo`, `document`, `file0`, …).
///
/// ```rust
/// use telegram_api_client::MultipartFile;
///
/// let part = MultipartFile::new("photo", "cat.png", "image/png", b"PNG".to_vec());
/// assert_eq!(part.field, "photo");
/// assert_eq!(part.len(), 3);
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct MultipartFile {
    /// Form field name, i.e. the Bot API parameter.
    pub field: String,
    /// File name sent in `Content-Disposition`; Unicode is fine.
    pub filename: String,
    /// Content type, `type/subtype`.
    pub mime: String,
    /// The bytes to upload.
    pub bytes: Vec<u8>,
}

impl MultipartFile {
    /// A file part.
    #[must_use]
    pub fn new(
        field: impl Into<String>,
        filename: impl Into<String>,
        mime: impl Into<String>,
        bytes: Vec<u8>,
    ) -> Self {
        Self {
            field: field.into(),
            filename: filename.into(),
            mime: mime.into(),
            bytes,
        }
    }

    /// Number of bytes in the part.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Whether the part carries no bytes. The Bot API rejects such uploads.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

/// Byte counts are noisy in `Debug`; the length is what matters when a test
/// fails on a mismatch.
impl fmt::Debug for MultipartFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MultipartFile")
            .field("field", &self.field)
            .field("filename", &self.filename)
            .field("mime", &self.mime)
            .field("bytes", &format_args!("{} bytes", self.bytes.len()))
            .finish()
    }
}

/// A `multipart/form-data` body: text fields plus binary parts.
///
/// Text fields are strings because that is all multipart can carry — nested
/// JSON is passed as its serialized form, which is what the Bot API expects.
/// The encoding is deterministic, so the same form always produces the same
/// boundary and the same bytes; that makes it testable without a socket.
///
/// ```rust
/// use telegram_api_client::MultipartForm;
///
/// let form = MultipartForm::new()
///     .field("chat_id", "-1001")
///     .field("caption", "Grüße");
/// assert_eq!(form.fields().len(), 2);
/// assert!(form.files().is_empty());
///
/// let (boundary, body) = form.to_http_body();
/// let text = String::from_utf8(body).unwrap();
/// assert!(text.starts_with(&format!("--{boundary}\r\n")));
/// assert!(text.ends_with(&format!("--{boundary}--\r\n")));
/// assert!(text.contains("name=\"caption\""));
/// ```
#[derive(Clone, Default, PartialEq, Eq)]
pub struct MultipartForm {
    fields: Vec<(String, String)>,
    files: Vec<MultipartFile>,
}

impl MultipartForm {
    /// An empty form.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a text field.
    ///
    /// ```rust
    /// use telegram_api_client::MultipartForm;
    ///
    /// let form = MultipartForm::new().field("chat_id", 42);
    /// assert_eq!(form.fields(), [("chat_id".to_string(), "42".to_string())]);
    /// ```
    #[must_use]
    pub fn field(mut self, name: impl Into<String>, value: impl fmt::Display) -> Self {
        self.fields.push((name.into(), value.to_string()));
        self
    }

    /// Adds a file part.
    ///
    /// ```rust
    /// use telegram_api_client::{MultipartFile, MultipartForm};
    ///
    /// let form = MultipartForm::new()
    ///     .field("chat_id", 1)
    ///     .with_file(MultipartFile::new("photo", "a.png", "image/png", vec![1, 2, 3]));
    /// assert_eq!(form.files()[0].len(), 3);
    /// ```
    #[must_use]
    pub fn with_file(mut self, file: MultipartFile) -> Self {
        self.files.push(file);
        self
    }

    /// The text fields, in insertion order.
    #[must_use]
    pub fn fields(&self) -> &[(String, String)] {
        &self.fields
    }

    /// The file parts, in insertion order.
    #[must_use]
    pub fn files(&self) -> &[MultipartFile] {
        &self.files
    }

    /// Whether the form carries neither fields nor files.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty() && self.files.is_empty()
    }

    /// The text fields as a JSON object — handy for logging and for the mock
    /// transport's [`Call::body`].
    ///
    /// ```rust
    /// use telegram_api_client::MultipartForm;
    ///
    /// let json = MultipartForm::new().field("chat_id", "-1001").json_fields();
    /// assert_eq!(json["chat_id"], "-1001");
    /// ```
    #[must_use]
    pub fn json_fields(&self) -> Value {
        let mut map = Map::new();
        for (name, value) in &self.fields {
            map.insert(name.clone(), Value::String(value.clone()));
        }
        Value::Object(map)
    }

    /// Boundary derived from the content, so the encoding is deterministic.
    ///
    /// A hash is used instead of randomness: no `rand` dependency, and the value
    /// cannot collide with the payload it delimits in practice.
    ///
    /// ```rust
    /// use telegram_api_client::MultipartForm;
    ///
    /// let a = MultipartForm::new().field("chat_id", 1);
    /// let b = MultipartForm::new().field("chat_id", 2);
    /// assert_ne!(a.boundary(), b.boundary());
    /// assert_eq!(a.boundary(), a.boundary());
    /// ```
    #[must_use]
    pub fn boundary(&self) -> String {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        for (name, value) in &self.fields {
            hasher.update(name.as_bytes());
            hasher.update([0]);
            hasher.update(value.as_bytes());
            hasher.update([0xff]);
        }
        for file in &self.files {
            hasher.update(file.field.as_bytes());
            hasher.update([0]);
            hasher.update(file.filename.as_bytes());
            hasher.update([0]);
            hasher.update(file.mime.as_bytes());
            hasher.update([0]);
            hasher.update(&file.bytes);
            hasher.update([0xff]);
        }
        let digest = hasher.finalize();
        format!("telegram-api-client-{}", hex::encode(&digest[..16]))
    }

    /// Encodes the body for the given boundary.
    ///
    /// Header values are escaped: quotes and line breaks in a file name cannot
    /// smuggle extra headers into the request.
    ///
    /// ```rust
    /// use telegram_api_client::{MultipartFile, MultipartForm};
    ///
    /// let form = MultipartForm::new()
    ///     .field("chat_id", 1)
    ///     .with_file(MultipartFile::new("photo", "a.png", "image/png", b"PNG".to_vec()));
    /// let body = form.encode("BOUNDARY");
    /// let text = String::from_utf8_lossy(&body);
    /// assert!(text.contains("Content-Disposition: form-data; name=\"photo\"; filename=\"a.png\""));
    /// assert!(text.contains("Content-Type: image/png"));
    /// assert!(text.ends_with("--BOUNDARY--\r\n"));
    /// ```
    #[must_use]
    pub fn encode(&self, boundary: &str) -> Vec<u8> {
        let mut out = Vec::new();
        for (name, value) in &self.fields {
            out.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            out.extend_from_slice(
                format!(
                    "Content-Disposition: form-data; name=\"{}\"\r\n\r\n",
                    escape_header(name)
                )
                .as_bytes(),
            );
            out.extend_from_slice(value.as_bytes());
            out.extend_from_slice(b"\r\n");
        }
        for file in &self.files {
            out.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            out.extend_from_slice(
                format!(
                    "Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\n",
                    escape_header(&file.field),
                    escape_header(&file.filename)
                )
                .as_bytes(),
            );
            out.extend_from_slice(format!("Content-Type: {}\r\n\r\n", file.mime).as_bytes());
            out.extend_from_slice(&file.bytes);
            out.extend_from_slice(b"\r\n");
        }
        out.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        out
    }

    /// The boundary and the encoded body, ready to be posted.
    ///
    /// ```rust
    /// use telegram_api_client::{MultipartFile, MultipartForm};
    ///
    /// let form = MultipartForm::new()
    ///     .with_file(MultipartFile::new("photo", "a.png", "image/png", vec![0x89]));
    /// let (boundary, body) = form.to_http_body();
    /// assert!(!boundary.is_empty());
    /// assert!(body.contains(&0x89), "file bytes are in the encoded body");
    /// ```
    #[must_use]
    pub fn to_http_body(&self) -> (String, Vec<u8>) {
        let boundary = self.boundary();
        let body = self.encode(&boundary);
        (boundary, body)
    }
}

impl fmt::Debug for MultipartForm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MultipartForm")
            .field("fields", &self.fields)
            .field("files", &self.files)
            .finish()
    }
}

/// Neutralises anything that would break out of a `Content-Disposition` line.
fn escape_header(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            '"' => "%22".to_string(),
            '\\' => "%5C".to_string(),
            c if c.is_control() => "_".to_string(),
            c => c.to_string(),
        })
        .collect()
}

/// The Bot API method named by the last URL segment, for error reporting.
#[cfg(feature = "http")]
fn method_of(url: &str) -> String {
    url.rsplit('/').next().unwrap_or_default().to_string()
}

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
///
/// [`post_multipart`](Transport::post_multipart) has a default implementation
/// that returns an error, so a JSON-only transport keeps compiling.
#[allow(async_fn_in_trait)]
pub trait Transport {
    /// POSTs `body` as JSON and returns the decoded response.
    ///
    /// # Errors
    ///
    /// Whatever the implementation produces: network failures, non-JSON
    /// bodies, and Bot API `ok: false` answers. A failure observed *after* the
    /// request was written must be [`Error::Ambiguous`], a failure before it
    /// [`Error::Transport`] — the client bases its retry decision on that split.
    fn post_json(
        &self,
        url: &str,
        body: &serde_json::Value,
    ) -> impl std::future::Future<Output = Result<serde_json::Value>>;

    /// POSTs a multipart form and returns the decoded response.
    ///
    /// The default implementation reports the transport as unable to upload,
    /// which is the honest answer for a JSON-only implementation:
    ///
    /// ```
    /// use telegram_api_client::{Error, MultipartForm, Result, Transport};
    ///
    /// struct JsonOnly;
    ///
    /// impl Transport for JsonOnly {
    ///     fn post_json(
    ///         &self,
    ///         _url: &str,
    ///         _body: &serde_json::Value,
    ///     ) -> impl std::future::Future<Output = Result<serde_json::Value>> {
    ///         std::future::ready(Ok(serde_json::json!({"ok": true})))
    ///     }
    /// }
    ///
    /// # async fn demo() {
    /// let form = MultipartForm::new().field("chat_id", 1);
    /// let error = JsonOnly
    ///     .post_multipart("https://api.telegram.org/botx/sendPhoto", &form)
    ///     .await
    ///     .unwrap_err();
    /// assert!(matches!(error, Error::InvalidRequest { field: "multipart", .. }));
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Same contract as [`post_json`](Transport::post_json).
    fn post_multipart(
        &self,
        _url: &str,
        _form: &MultipartForm,
    ) -> impl std::future::Future<Output = Result<serde_json::Value>> {
        std::future::ready(Err(Error::InvalidRequest {
            field: "multipart",
            reason: "this Transport does not implement post_multipart; use ReqwestTransport or provide one"
                .to_string(),
        }))
    }
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
        let request = self.client.post(url).json(body);
        execute(request, url).await
    }

    async fn post_multipart(&self, url: &str, form: &MultipartForm) -> Result<serde_json::Value> {
        let (boundary, bytes) = form.to_http_body();
        let request = self
            .client
            .post(url)
            .header(
                reqwest::header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(bytes);
        execute(request, url).await
    }
}

/// Sends a prepared request and classifies whatever comes back.
#[cfg(feature = "http")]
async fn execute(request: reqwest::RequestBuilder, url: &str) -> Result<serde_json::Value> {
    let response = request
        .send()
        .await
        .map_err(|error| classify_send_error(url, error))?;

    let status = response.status().as_u16();
    let text = response.text().await.map_err(|error| {
        // The request was accepted (or not) — we just lost the answer. That is
        // the textbook ambiguous case: never silently retried.
        Error::ambiguous(
            method_of(url),
            format!("connection broke while reading the response: {error}"),
        )
        .with_source(error)
    })?;

    decode_response(status, &text)
}

/// Splits a connection failure into "nothing was written" and "we cannot tell".
#[cfg(feature = "http")]
fn classify_send_error(url: &str, error: reqwest::Error) -> Error {
    if error.is_connect() {
        // DNS, TCP connect, TLS handshake: the body never left the machine.
        Error::Transport {
            message: error.to_string(),
            source: Some(Box::new(error)),
        }
    } else {
        Error::Ambiguous {
            method: method_of(url),
            detail: format!("request failed after it was written: {error}"),
            source: Some(Box::new(error)),
        }
    }
}

/// Turns an HTTP status plus body into a value or a typed error.
#[cfg(feature = "http")]
fn decode_response(status: u16, text: &str) -> Result<serde_json::Value> {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        // A proxy or overload page may answer 429/5xx without JSON; keep the
        // status meaningful instead of reporting an opaque parse error.
        return Err(
            Error::from_api_body(&Value::Null, Some(status)).unwrap_or_else(|| Error::Payload {
                message: format!(
                    "non-JSON response (status {status}): {}",
                    truncate(text, 200)
                ),
                source: serde_json::from_str::<Value>(text).err(),
            }),
        );
    };

    if let Some(error) = Error::from_api_body(&value, Some(status)) {
        return Err(error);
    }
    Ok(value)
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
/// For a multipart upload `body` holds the text fields and [`Call::multipart`]
/// the file parts; for a JSON call `multipart` is `None`.
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
/// assert!(!call.is_multipart());
/// assert!(call.files().is_empty());
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    /// Full URL the client posted to.
    pub url: String,
    /// JSON body that was posted, or the text fields of a multipart upload.
    pub body: Value,
    /// File parts, when the request was a multipart upload.
    pub multipart: Option<MultipartForm>,
}

impl Call {
    /// The Bot API method name, i.e. the last URL segment.
    #[must_use]
    pub fn method(&self) -> &str {
        self.url.rsplit('/').next().unwrap_or_default()
    }

    /// Whether this request was a multipart upload.
    #[must_use]
    pub fn is_multipart(&self) -> bool {
        self.multipart.is_some()
    }

    /// The uploaded file parts, empty for JSON calls.
    #[must_use]
    pub fn files(&self) -> &[MultipartFile] {
        self.multipart.as_ref().map_or(&[], MultipartForm::files)
    }

    /// The uploaded bytes for `field`, if that part was present.
    ///
    /// ```
    /// use telegram_api_client::{MultipartFile, MultipartForm, MockTransport, Transport};
    ///
    /// # async fn demo() {
    /// let transport = MockTransport::new();
    /// let form = MultipartForm::new()
    ///     .field("chat_id", "-1001")
    ///     .with_file(MultipartFile::new("photo", "a.png", "image/png", b"PNG".to_vec()));
    /// transport.post_multipart("https://x/bott/sendPhoto", &form).await.unwrap();
    ///
    /// let call = transport.last_call().unwrap();
    /// assert_eq!(call.file_bytes("photo"), Some(&b"PNG"[..]));
    /// assert_eq!(call.body["chat_id"], "-1001");
    /// # }
    /// ```
    #[must_use]
    pub fn file_bytes(&self, field: &str) -> Option<&[u8]> {
        self.files()
            .iter()
            .find(|file| file.field == field)
            .map(|file| file.bytes.as_slice())
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
    /// A transport-level failure before anything was written (no response).
    Transport(String),
    /// A transport-level failure after the request was written: delivery is
    /// unknown.
    Ambiguous(String),
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

    /// A failure observed after the request was written (timeout, reset).
    #[must_use]
    pub fn ambiguous(message: impl Into<String>) -> Self {
        Self::Ambiguous(message.into())
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
            // No method in the reply: the client fills it in when it classifies
            // the error for the method it called.
            Self::Ambiguous(detail) => Err(Error::Ambiguous {
                method: String::new(),
                detail,
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
        std::future::ready(self.record(url, body.clone(), None))
    }

    fn post_multipart(
        &self,
        url: &str,
        form: &MultipartForm,
    ) -> impl std::future::Future<Output = Result<serde_json::Value>> {
        std::future::ready(self.record(url, form.json_fields(), Some(form.clone())))
    }
}

impl MockTransport {
    /// Records one call and pops the next canned reply.
    fn record(&self, url: &str, body: Value, multipart: Option<MultipartForm>) -> Result<Value> {
        let reply = {
            let mut inner = self.lock();
            inner.calls.push(Call {
                url: url.to_string(),
                body,
                multipart,
            });
            inner.queue.pop_front()
        };
        reply.unwrap_or_else(MockReply::ok).into_result()
    }
}

/// Unit tests for the response parser the `reqwest` transport delegates to.
/// They live here, not in `tests/`, because the functions are private.
#[cfg(all(test, feature = "http"))]
mod tests {
    use super::{decode_response, method_of, truncate};
    use crate::error::Error;

    #[test]
    fn a_successful_body_decodes() {
        let value = decode_response(200, r#"{"ok":true,"result":{"message_id":7}}"#).unwrap();
        assert_eq!(value["result"]["message_id"], 7);
    }

    #[test]
    fn an_ok_false_body_becomes_an_api_error() {
        let error = decode_response(
            400,
            r#"{"ok":false,"error_code":400,"description":"Bad Request: chat not found"}"#,
        )
        .unwrap_err();
        assert!(matches!(error, Error::Api { code: 400, .. }));
        assert!(!error.is_retryable());
    }

    #[test]
    fn a_429_body_keeps_its_retry_hint() {
        let error = decode_response(
            429,
            r#"{"ok":false,"error_code":429,"description":"Too Many Requests","parameters":{"retry_after":9}}"#,
        )
        .unwrap_err();
        assert_eq!(error.retry_after(), Some(std::time::Duration::from_secs(9)));
    }

    #[test]
    fn a_non_json_5xx_keeps_the_status() {
        let error = decode_response(502, "<html>bad gateway</html>").unwrap_err();
        assert!(matches!(error, Error::Api { code: 502, .. }), "{error:?}");
    }

    #[test]
    fn a_non_json_200_is_a_payload_error() {
        let error = decode_response(200, "not json").unwrap_err();
        assert!(matches!(error, Error::Payload { .. }), "{error:?}");
        assert!(error.to_string().contains("not json"));
    }

    #[test]
    fn the_method_is_the_last_url_segment() {
        assert_eq!(
            method_of("https://api.telegram.org/bot1:a/sendPhoto"),
            "sendPhoto"
        );
        assert_eq!(method_of("sendPhoto"), "sendPhoto");
        assert_eq!(method_of(""), "");
    }

    #[test]
    fn long_bodies_are_truncated_on_a_char_boundary() {
        assert_eq!(truncate("short", 10), "short");
        let long = "ä".repeat(300);
        let cut = truncate(&long, 5);
        assert_eq!(cut.chars().count(), 6, "five chars plus the ellipsis");
        assert!(cut.ends_with('…'));
    }

    #[test]
    fn the_http_feature_keeps_the_status_for_a_boundary_case() {
        // A 500 with a JSON body but no `ok` field: the status decides.
        let error = decode_response(500, r#"{"message":"boom"}"#).unwrap_err();
        assert!(matches!(error, Error::Api { code: 500, .. }), "{error:?}");
    }
}
