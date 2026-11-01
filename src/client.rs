//! Bot API client.
//!
//! Every method validates its request locally, posts it through the
//! [`Transport`], and decodes the `result` field into a typed value.

use std::sync::Arc;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::json;

use crate::callback::{AnswerCallbackQuery, AnswerInlineQuery, SetChatMenuButton};
use crate::chat::{
    BanChatMember, ChatMember, GetChat, GetChatMember, PinChatMessage, SendChatAction,
    UnpinChatMessage,
};
use crate::edit::{
    CopyMessage, DeleteMessage, EditMessageCaption, EditMessageMedia, EditMessageReplyMarkup,
    EditMessageText, EditResult, ForwardMessage, MessageId,
};
use crate::error::{Error, Result};
use crate::file::{File, GetFile};
use crate::polling::GetUpdates;
use crate::retry::{RetryPolicy, Sleeper, ThreadSleeper};
use crate::send::{
    SendAnimation, SendAudio, SendContact, SendDice, SendDocument, SendLocation, SendMediaGroup,
    SendMessage, SendPhoto, SendPoll, SendVenue, SendVideo, SendVoice,
};
use crate::transport::{MultipartForm, Transport};
use crate::types::ChatId;
use crate::update::User;
use crate::update::{Chat, Message, Update};
use crate::webhook_api::{DeleteWebhook, SetWebhook, WebhookInfo};

/// Pulls `result` out of an envelope and deserializes it.
fn take_result<D: DeserializeOwned>(method: &str, response: &serde_json::Value) -> Result<D> {
    if let Some(error) = Error::from_api_body(response, None) {
        return Err(error);
    }
    let result = response
        .get("result")
        .cloned()
        .ok_or_else(|| Error::Unexpected {
            method: method.to_string(),
            detail: "response has no `result` field".to_string(),
        })?;
    serde_json::from_value(result).map_err(|error| Error::Payload {
        message: format!("{method}: could not decode result: {error}"),
        source: Some(error),
    })
}

/// Generates a thin client method: validate, post, decode.
macro_rules! api_method {
    (
        $(#[$meta:meta])*
        $name:ident, $method:literal, $request:ty, $response:ty
    ) => {
        $(#[$meta])*
        ///
        /// # Errors
        ///
        /// [`Error::InvalidRequest`] when the request does not validate, plus
        /// transport, rate-limit and decoding failures from the Bot API.
        pub async fn $name(&self, request: &$request) -> Result<$response> {
            request.validate()?;
            let response = self.call($method, request).await?;
            take_result($method, &response)
        }
    };
}

/// Like [`api_method`], but the request may carry a file to upload.
macro_rules! media_method {
    (
        $(#[$meta:meta])*
        $name:ident, $method:literal, $request:ty, $response:ty
    ) => {
        $(#[$meta])*
        ///
        /// A configured upload turns the request into a `multipart/form-data`
        /// body; without one the same method posts JSON.
        ///
        /// # Errors
        ///
        /// [`Error::InvalidRequest`] when the request or the file does not
        /// validate, plus transport, rate-limit and decoding failures. A 5xx is
        /// reported as [`Error::Ambiguous`] and never retried — the upload may
        /// have been accepted.
        pub async fn $name(&self, request: &$request) -> Result<$response> {
            request.validate()?;
            let form = request.multipart_form();
            let response = self.call_maybe_upload($method, request, form).await?;
            take_result($method, &response)
        }
    };
}

/// How a request is put on the wire.
enum Payload<'a> {
    /// A JSON body.
    Json(serde_json::Value),
    /// A multipart body.
    Multipart(&'a MultipartForm),
}

/// Retry budget and the clock to wait on.
struct RetrySettings {
    policy: RetryPolicy,
    sleeper: Arc<dyn Sleeper>,
}

impl Default for RetrySettings {
    fn default() -> Self {
        Self {
            // Deliberately short: automatic retries only ever cover 429 and
            // pre-write connection failures, both of which resolve quickly.
            policy: RetryPolicy::new(3, Duration::from_millis(500), Duration::from_secs(30)),
            sleeper: Arc::new(ThreadSleeper),
        }
    }
}

/// Client bound to one bot token.
///
/// Cheap to clone (when the transport is) and safe to share across tasks.
///
/// ```
/// use telegram_api_client::{MockTransport, TelegramClient};
///
/// # async fn demo() -> telegram_api_client::Result<()> {
/// let client = TelegramClient::new("123:ABC", MockTransport::new());
/// assert!(client.method_url("getMe").ends_with("/bot123:ABC/getMe"));
/// # Ok(()) }
/// ```
pub struct TelegramClient<T: Transport> {
    token: String,
    base_url: String,
    transport: T,
    retry: RetrySettings,
}

impl<T: Transport + Clone> Clone for TelegramClient<T> {
    fn clone(&self) -> Self {
        Self {
            token: self.token.clone(),
            base_url: self.base_url.clone(),
            transport: self.transport.clone(),
            retry: RetrySettings {
                policy: self.retry.policy.clone(),
                sleeper: Arc::clone(&self.retry.sleeper),
            },
        }
    }
}

impl<T: Transport> TelegramClient<T> {
    /// Creates a client for `token`.
    ///
    /// An empty token is rejected by the Bot API, not here — the first call
    /// will fail with [`Error::Api`].
    ///
    /// ```rust
    /// use telegram_api_client::{MockTransport, TelegramClient};
    ///
    /// let client = TelegramClient::new("123:ABC", MockTransport::new());
    /// assert_eq!(client.base_url(), "https://api.telegram.org");
    /// ```
    pub fn new(token: impl Into<String>, transport: T) -> Self {
        Self {
            token: token.into(),
            base_url: "https://api.telegram.org".to_string(),
            transport,
            retry: RetrySettings::default(),
        }
    }

    /// Replaces the retry budget.
    ///
    /// The budget only ever applies to failures that are safe to repeat: a 429
    /// (honouring `retry_after`) and a connection failure before the request was
    /// written. A 5xx on a state-changing method is
    /// [`Error::Ambiguous`] and is returned immediately, whatever the budget.
    ///
    /// ```rust
    /// use std::time::Duration;
    /// use telegram_api_client::{MockTransport, RetryPolicy, TelegramClient};
    ///
    /// let client = TelegramClient::new("123:ABC", MockTransport::new())
    ///     .with_retry_policy(RetryPolicy::new(1, Duration::from_millis(10), Duration::from_secs(1)));
    /// assert_eq!(client.retry_policy().max_attempts, 1, "no automatic retry");
    /// ```
    #[must_use]
    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry.policy = policy;
        self
    }

    /// Disables automatic retries; the caller keeps the loop.
    ///
    /// ```rust
    /// use telegram_api_client::{MockTransport, TelegramClient};
    ///
    /// let client = TelegramClient::new("123:ABC", MockTransport::new()).without_retry();
    /// assert_eq!(client.retry_policy().max_attempts, 1);
    /// ```
    #[must_use]
    pub fn without_retry(mut self) -> Self {
        self.retry.policy.max_attempts = 1;
        self
    }

    /// Installs a different [`Sleeper`] — a runtime timer, or a recorder that
    /// skips the wait in tests.
    ///
    /// ```rust
    /// use telegram_api_client::{MockTransport, NoDelay, TelegramClient};
    ///
    /// let client = TelegramClient::new("123:ABC", MockTransport::new()).with_sleeper(NoDelay);
    /// # let _ = client;
    /// ```
    #[must_use]
    pub fn with_sleeper(mut self, sleeper: impl Sleeper + 'static) -> Self {
        self.retry.sleeper = Arc::new(sleeper);
        self
    }

    /// The effective retry budget.
    #[must_use]
    pub fn retry_policy(&self) -> &RetryPolicy {
        &self.retry.policy
    }

    /// Overrides the base URL (tests, proxies).
    ///
    /// Trailing slashes are tolerated.
    ///
    /// ```
    /// use telegram_api_client::{MockTransport, TelegramClient};
    ///
    /// let client = TelegramClient::new("1:A", MockTransport::new())
    ///     .with_base_url("https://proxy.test/");
    /// assert_eq!(client.method_url("getMe"), "https://proxy.test/bot1:A/getMe");
    /// ```
    #[must_use]
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// The configured base URL.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Endpoint for a Bot API method.
    ///
    /// ```
    /// use telegram_api_client::{MockTransport, TelegramClient};
    ///
    /// let client = TelegramClient::new("123:ABC", MockTransport::new());
    /// assert_eq!(
    ///     client.method_url("sendMessage"),
    ///     "https://api.telegram.org/bot123:ABC/sendMessage",
    /// );
    /// ```
    #[must_use]
    pub fn method_url(&self, method: &str) -> String {
        format!(
            "{}/bot{}/{}",
            self.base_url.trim_end_matches('/'),
            self.token,
            method
        )
    }

    /// Download URL for a file resolved with [`Self::get_file`].
    ///
    /// # Errors
    ///
    /// [`Error::Unexpected`] when Telegram returned no `file_path` (file too
    /// large for the Bot API, or the handle expired).
    ///
    /// ```
    /// use telegram_api_client::{File, MockTransport, TelegramClient};
    ///
    /// let client = TelegramClient::new("123:ABC", MockTransport::new());
    /// let file = File { file_path: Some("photos/a.jpg".into()), ..File::default() };
    /// assert_eq!(
    ///     client.file_url(&file).unwrap(),
    ///     "https://api.telegram.org/file/bot123:ABC/photos/a.jpg",
    /// );
    /// assert!(client.file_url(&File::default()).is_err());
    /// ```
    pub fn file_url(&self, file: &File) -> Result<String> {
        let path = file.download_path().ok_or_else(|| Error::Unexpected {
            method: "getFile".to_string(),
            detail: "response carried no `file_path` (file too large, or handle expired)"
                .to_string(),
        })?;
        Ok(format!(
            "{}/file/bot{}/{}",
            self.base_url.trim_end_matches('/'),
            self.token,
            path.trim_start_matches('/')
        ))
    }

    /// Serializes `request` and posts it.
    async fn call<R: Serialize + ?Sized>(
        &self,
        method: &str,
        request: &R,
    ) -> Result<serde_json::Value> {
        let body = serde_json::to_value(request).map_err(|error| Error::Payload {
            message: format!("cannot serialize {method} body: {error}"),
            source: Some(error),
        })?;
        self.post(method, &Payload::Json(body)).await
    }

    /// Posts either a JSON or a multipart payload, retrying only what is safe.
    async fn call_maybe_upload<R: Serialize + ?Sized>(
        &self,
        method: &str,
        request: &R,
        form: Option<MultipartForm>,
    ) -> Result<serde_json::Value> {
        match form {
            Some(form) => self.post(method, &Payload::Multipart(&form)).await,
            None => self.call(method, request).await,
        }
    }

    /// The one place that decides whether a failure may be repeated.
    ///
    /// Every error is classified for its method first, so a 5xx on a send turns
    /// into [`Error::Ambiguous`] before the retry decision is taken. That is what
    /// keeps a non-idempotent request from ever being retried after a 5xx.
    async fn post(&self, method: &str, payload: &Payload<'_>) -> Result<serde_json::Value> {
        let url = self.method_url(method);
        let mut attempt = 0_u32;
        loop {
            attempt += 1;
            let outcome = match payload {
                Payload::Json(body) => self.transport.post_json(&url, body).await,
                Payload::Multipart(form) => self.transport.post_multipart(&url, form).await,
            }
            .map_err(|error| error.classify_for(method));

            match outcome {
                Ok(response) => return Ok(response),
                Err(error) if self.retry.policy.should_retry(attempt, &error) => {
                    let delay = self.retry.policy.delay_for_error(attempt, &error);
                    self.retry.sleeper.sleep(delay).await;
                }
                Err(error) => return Err(error),
            }
        }
    }

    // ---- messages -------------------------------------------------------

    /// Sends a plain text message and returns the raw response envelope.
    ///
    /// Deprecated: [`Self::send_text`] returns the typed [`Message`] and
    /// [`Self::send_message_request`] exposes the full request surface.
    ///
    /// The call takes the same road as `send_message_request`: the request is
    /// built as a [`SendMessage`] and validated before anything is posted, and
    /// the answer goes through the same envelope check before it is handed
    /// back. Validation and error handling are therefore identical to the typed
    /// path; only the return type still differs.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an unusable chat, an empty text or one over
    /// [`MAX_MESSAGE_TEXT`](crate::send::MAX_MESSAGE_TEXT) UTF-16 code units, plus
    /// transport, rate-limit, Bot API and decoding failures — a `{"ok": false}`
    /// body or one without `result` is an error, never a successful envelope.
    ///
    /// ```rust
    /// use telegram_api_client::send::MAX_MESSAGE_TEXT;
    /// use telegram_api_client::{Error, MockTransport, TelegramClient};
    ///
    /// # async fn demo() -> telegram_api_client::Result<()> {
    /// // An empty text never reaches the network any more.
    /// let transport = MockTransport::new();
    /// let client = TelegramClient::new("123:ABC", transport.clone());
    /// # #[allow(deprecated)]
    /// let error = client.send_message(-1001, " ").await.unwrap_err();
    /// assert!(matches!(error, Error::InvalidRequest { field: "text", .. }));
    /// assert!(transport.calls().is_empty(), "rejected before the call");
    ///
    /// // The boundary pair: the limit itself passes, one unit more does not.
    /// # #[allow(deprecated)]
    /// let envelope = client
    ///     .send_message(-1001, &"a".repeat(MAX_MESSAGE_TEXT))
    ///     .await?;
    /// assert_eq!(envelope["ok"], true);
    /// # #[allow(deprecated)]
    /// let error = client
    ///     .send_message(-1001, &"a".repeat(MAX_MESSAGE_TEXT + 1))
    ///     .await
    ///     .unwrap_err();
    /// assert!(matches!(error, Error::InvalidRequest { field: "text", .. }));
    /// # Ok(()) }
    /// ```
    #[deprecated(
        since = "0.1.1",
        note = "use `send_text` for a typed Message, or `send_message_request(&SendMessage)`"
    )]
    pub async fn send_message(&self, chat_id: i64, text: &str) -> Result<serde_json::Value> {
        let request = SendMessage::new(chat_id, text);
        request.validate()?;
        let response = self.call("sendMessage", &request).await?;
        // The envelope goes back as it arrived, but it is still checked: a
        // failed body or one without `result` must not read as a success.
        take_result::<serde_json::Value>("sendMessage", &response)?;
        Ok(response)
    }

    /// Sends plain text and returns the created [`Message`].
    ///
    /// The typed replacement for the deprecated `send_message`: one call, one
    /// message back, no raw JSON to walk.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an empty or over-long text, plus transport,
    /// rate-limit and decoding failures.
    ///
    /// ```rust
    /// use telegram_api_client::{MockTransport, TelegramClient};
    ///
    /// # async fn demo() -> telegram_api_client::Result<()> {
    /// let transport = MockTransport::new();
    /// let client = TelegramClient::new("123:ABC", transport.clone());
    /// let message = client.send_text(-1001, "ping").await?;
    ///
    /// assert_eq!(message.message_id, 0);
    /// assert_eq!(transport.last_call().unwrap().method(), "sendMessage");
    /// # Ok(()) }
    /// ```
    pub async fn send_text(
        &self,
        chat_id: impl Into<ChatId>,
        text: impl Into<String>,
    ) -> Result<Message> {
        self.send_message_request(&SendMessage::new(chat_id, text))
            .await
    }

    api_method!(
        /// Sends a text message with formatting, reply markup and threads.
        ///
        /// ```
        /// use telegram_api_client::{MockTransport, SendMessage, TelegramClient};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let transport = MockTransport::new();
        /// let client = TelegramClient::new("123:ABC", transport.clone());
        /// let message = client
        ///     .send_message_request(&SendMessage::new(-1001, "hi").silent())
        ///     .await?;
        /// assert_eq!(message.message_id, 0);
        /// # Ok(()) }
        /// ```
        send_message_request, "sendMessage", SendMessage, Message
    );

    media_method!(
        /// Sends a photo.
        ///
        /// ```
        /// use telegram_api_client::{MockTransport, SendPhoto, TelegramClient};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// client.send_photo(&SendPhoto::new(1, "file-id")).await?;
        /// # Ok(()) }
        /// ```
        send_photo, "sendPhoto", SendPhoto, Message
    );

    media_method!(
        /// Sends a document.
        ///
        /// ```
        /// use telegram_api_client::{MockTransport, SendDocument, TelegramClient};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// client.send_document(&SendDocument::new(1, "file-id")).await?;
        /// # Ok(()) }
        /// ```
        send_document, "sendDocument", SendDocument, Message
    );

    media_method!(
        /// Sends an audio file.
        ///
        /// ```
        /// use telegram_api_client::{MockTransport, SendAudio, TelegramClient};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// client.send_audio(&SendAudio::new(1, "file-id")).await?;
        /// # Ok(()) }
        /// ```
        send_audio, "sendAudio", SendAudio, Message
    );

    media_method!(
        /// Sends a video.
        ///
        /// ```
        /// use telegram_api_client::{MockTransport, SendVideo, TelegramClient};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// client.send_video(&SendVideo::new(1, "file-id")).await?;
        /// # Ok(()) }
        /// ```
        send_video, "sendVideo", SendVideo, Message
    );

    media_method!(
        /// Sends an animation.
        ///
        /// ```
        /// use telegram_api_client::{MockTransport, SendAnimation, TelegramClient};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// client.send_animation(&SendAnimation::new(1, "file-id")).await?;
        /// # Ok(()) }
        /// ```
        send_animation, "sendAnimation", SendAnimation, Message
    );

    media_method!(
        /// Sends a voice note.
        ///
        /// ```
        /// use telegram_api_client::{MockTransport, SendVoice, TelegramClient};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// client.send_voice(&SendVoice::new(1, "file-id")).await?;
        /// # Ok(()) }
        /// ```
        send_voice, "sendVoice", SendVoice, Message
    );

    media_method!(
        /// Sends an album of 2–10 media items.
        ///
        /// ```
        /// use telegram_api_client::{InputMedia, MockTransport, SendMediaGroup, TelegramClient};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// let album = SendMediaGroup::new(1, vec![InputMedia::photo("a"), InputMedia::video("b")]);
        /// client.send_media_group(&album).await?;
        /// # Ok(()) }
        /// ```
        send_media_group, "sendMediaGroup", SendMediaGroup, Vec<Message>
    );

    api_method!(
        /// Sends a location.
        ///
        /// ```
        /// use telegram_api_client::{MockTransport, SendLocation, TelegramClient};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// client.send_location(&SendLocation::new(1, 48.7665, 11.4258)).await?;
        /// # Ok(()) }
        /// ```
        send_location, "sendLocation", SendLocation, Message
    );

    api_method!(
        /// Sends a venue.
        ///
        /// ```
        /// use telegram_api_client::{MockTransport, SendVenue, TelegramClient};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// client.send_venue(&SendVenue::new(1, 48.7, 11.4, "Café", "Am Platz 1")).await?;
        /// # Ok(()) }
        /// ```
        send_venue, "sendVenue", SendVenue, Message
    );

    api_method!(
        /// Sends a contact card.
        ///
        /// ```
        /// use telegram_api_client::{MockTransport, SendContact, TelegramClient};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// client.send_contact(&SendContact::new(1, "+49151", "Flo")).await?;
        /// # Ok(()) }
        /// ```
        send_contact, "sendContact", SendContact, Message
    );

    api_method!(
        /// Sends a poll.
        ///
        /// ```
        /// use telegram_api_client::{MockTransport, SendPoll, TelegramClient};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// let poll = SendPoll::new(1, "Lunch?", vec!["Pizza".into(), "Salad".into()]);
        /// client.send_poll(&poll).await?;
        /// # Ok(()) }
        /// ```
        send_poll, "sendPoll", SendPoll, Message
    );

    api_method!(
        /// Sends a dice throw.
        ///
        /// ```
        /// use telegram_api_client::{MockTransport, SendDice, TelegramClient};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// client.send_dice(&SendDice::new(1)).await?;
        /// # Ok(()) }
        /// ```
        send_dice, "sendDice", SendDice, Message
    );

    // ---- editing --------------------------------------------------------

    api_method!(
        /// Edits the text of a message.
        ///
        /// ```
        /// use telegram_api_client::{EditMessageText, MockTransport, TelegramClient};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// client.edit_message_text(&EditMessageText::new(1, 7, "new")).await?;
        /// # Ok(()) }
        /// ```
        edit_message_text, "editMessageText", EditMessageText, EditResult
    );

    api_method!(
        /// Edits the caption of a media message.
        ///
        /// ```
        /// use telegram_api_client::{EditMessageCaption, MockTransport, TelegramClient};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// client.edit_message_caption(&EditMessageCaption::new(1, 7).with_caption("new")).await?;
        /// # Ok(()) }
        /// ```
        edit_message_caption, "editMessageCaption", EditMessageCaption, EditResult
    );

    api_method!(
        /// Replaces the inline keyboard of a message.
        ///
        /// ```
        /// use telegram_api_client::{EditMessageReplyMarkup, MockTransport, TelegramClient};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// client.edit_message_reply_markup(&EditMessageReplyMarkup::new(1, 7)).await?;
        /// # Ok(()) }
        /// ```
        edit_message_reply_markup, "editMessageReplyMarkup", EditMessageReplyMarkup, EditResult
    );

    media_method!(
        /// Replaces the media of an existing message.
        ///
        /// ```rust
        /// use telegram_api_client::{EditMessageMedia, InputMedia, MockTransport, TelegramClient};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// let edit = EditMessageMedia::new(1, 7, InputMedia::video("DQACAgI"));
        /// client.edit_message_media(&edit).await?;
        /// # Ok(()) }
        /// ```
        edit_message_media, "editMessageMedia", EditMessageMedia, EditResult
    );

    api_method!(
        /// Deletes a message.
        ///
        /// ```
        /// use telegram_api_client::{DeleteMessage, MockTransport, TelegramClient};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// let deleted = client.delete_message(&DeleteMessage::new(1, 7)).await?;
        /// assert!(deleted);
        /// # Ok(()) }
        /// ```
        delete_message, "deleteMessage", DeleteMessage, bool
    );

    api_method!(
        /// Forwards a message, keeping the "forwarded from" label.
        ///
        /// ```
        /// use telegram_api_client::{ForwardMessage, MockTransport, TelegramClient};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// client.forward_message(&ForwardMessage::new(1, -1001, 7)).await?;
        /// # Ok(()) }
        /// ```
        forward_message, "forwardMessage", ForwardMessage, Message
    );

    api_method!(
        /// Copies a message without the forward label.
        ///
        /// ```
        /// use telegram_api_client::{CopyMessage, MockTransport, TelegramClient};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// let created = client.copy_message(&CopyMessage::new(1, -1001, 7)).await?;
        /// assert_eq!(created.message_id, 0);
        /// # Ok(()) }
        /// ```
        copy_message, "copyMessage", CopyMessage, MessageId
    );

    // ---- callbacks and inline -------------------------------------------

    api_method!(
        /// Answers a callback query so the client stops its spinner.
        ///
        /// ```
        /// use telegram_api_client::{AnswerCallbackQuery, MockTransport, TelegramClient};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// client.answer_callback_query(&AnswerCallbackQuery::new("4382").with_text("ok")).await?;
        /// # Ok(()) }
        /// ```
        answer_callback_query, "answerCallbackQuery", AnswerCallbackQuery, bool
    );

    api_method!(
        /// Answers an inline query with typed or raw results.
        ///
        /// ```
        /// use telegram_api_client::{
        ///     AnswerInlineQuery, InlineQueryResult, MockTransport, TelegramClient,
        /// };
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// let results = vec![InlineQueryResult::article("1", "Ping", "Pong")];
        /// client.answer_inline_query(&AnswerInlineQuery::new("q1", results)).await?;
        /// # Ok(()) }
        /// ```
        answer_inline_query, "answerInlineQuery", AnswerInlineQuery, bool
    );

    api_method!(
        /// Sets the menu button, globally or for one chat.
        ///
        /// ```
        /// use telegram_api_client::{MenuButton, MockTransport, SetChatMenuButton, TelegramClient};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// let request = SetChatMenuButton::global(MenuButton::Commands);
        /// client.set_chat_menu_button(&request).await?;
        /// # Ok(()) }
        /// ```
        set_chat_menu_button, "setChatMenuButton", SetChatMenuButton, bool
    );

    // ---- chat administration --------------------------------------------

    api_method!(
        /// Shows a chat action such as `typing`.
        ///
        /// ```
        /// use telegram_api_client::{ChatAction, MockTransport, SendChatAction, TelegramClient};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// client.send_chat_action(&SendChatAction::typing(-1001)).await?;
        /// # let _ = ChatAction::Typing;
        /// # Ok(()) }
        /// ```
        send_chat_action, "sendChatAction", SendChatAction, bool
    );

    api_method!(
        /// Fetches full chat details.
        ///
        /// ```
        /// use telegram_api_client::{GetChat, MockTransport, TelegramClient};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// let chat = client.get_chat(&GetChat::new(-1001)).await?;
        /// assert_eq!(chat.label(), "0");
        /// # Ok(()) }
        /// ```
        get_chat, "getChat", GetChat, Chat
    );

    api_method!(
        /// Fetches one chat member.
        ///
        /// ```
        /// use telegram_api_client::{GetChatMember, MockTransport, TelegramClient};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// let member = client.get_chat_member(&GetChatMember::new(-1001, 42)).await?;
        /// assert!(!member.is_present());
        /// # Ok(()) }
        /// ```
        get_chat_member, "getChatMember", GetChatMember, ChatMember
    );

    api_method!(
        /// Bans a user from a chat.
        ///
        /// ```
        /// use telegram_api_client::{BanChatMember, MockTransport, TelegramClient};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// client.ban_chat_member(&BanChatMember::new(-1001, 42)).await?;
        /// # Ok(()) }
        /// ```
        ban_chat_member, "banChatMember", BanChatMember, bool
    );

    api_method!(
        /// Pins a message.
        ///
        /// ```
        /// use telegram_api_client::{MockTransport, PinChatMessage, TelegramClient};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// client.pin_chat_message(&PinChatMessage::new(-1001, 7)).await?;
        /// # Ok(()) }
        /// ```
        pin_chat_message, "pinChatMessage", PinChatMessage, bool
    );

    api_method!(
        /// Unpins one or the most recent message.
        ///
        /// ```
        /// use telegram_api_client::{MockTransport, TelegramClient, UnpinChatMessage};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// client.unpin_chat_message(&UnpinChatMessage::new(-1001)).await?;
        /// # Ok(()) }
        /// ```
        unpin_chat_message, "unpinChatMessage", UnpinChatMessage, bool
    );

    // ---- webhook lifecycle ----------------------------------------------

    api_method!(
        /// Registers the webhook.
        ///
        /// ```
        /// use telegram_api_client::{MockTransport, SetWebhook, TelegramClient};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// let request = SetWebhook::new("https://example.com/hook").with_secret("s3cret");
        /// client.set_webhook(&request).await?;
        /// # Ok(()) }
        /// ```
        set_webhook, "setWebhook", SetWebhook, bool
    );

    api_method!(
        /// Deletes the webhook, optionally dropping queued updates.
        ///
        /// ```
        /// use telegram_api_client::{DeleteWebhook, MockTransport, TelegramClient};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// client.delete_webhook(&DeleteWebhook::dropping_pending()).await?;
        /// # Ok(()) }
        /// ```
        delete_webhook, "deleteWebhook", DeleteWebhook, bool
    );

    /// Reports the current webhook state.
    ///
    /// # Errors
    ///
    /// Transport, rate-limit and decoding failures.
    ///
    /// ```
    /// use telegram_api_client::{MockTransport, TelegramClient};
    ///
    /// # async fn demo() -> telegram_api_client::Result<()> {
    /// let client = TelegramClient::new("123:ABC", MockTransport::new());
    /// let info = client.get_webhook_info().await?;
    /// assert!(!info.is_registered());
    /// # Ok(()) }
    /// ```
    pub async fn get_webhook_info(&self) -> Result<WebhookInfo> {
        let response = self.call("getWebhookInfo", &json!({})).await?;
        take_result("getWebhookInfo", &response)
    }

    // ---- polling --------------------------------------------------------

    api_method!(
        /// Fetches updates. Pair with [`crate::UpdateCursor`] for exactly-once
        /// processing.
        ///
        /// ```
        /// use telegram_api_client::{GetUpdates, MockTransport, TelegramClient};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// let updates = client.get_updates(&GetUpdates::new().timeout(25)).await?;
        /// assert!(updates.is_empty());
        /// # Ok(()) }
        /// ```
        get_updates, "getUpdates", GetUpdates, Vec<Update>
    );

    // ---- files ----------------------------------------------------------

    api_method!(
        /// Resolves a `file_id` into a [`File`] with a download path.
        ///
        /// ```
        /// use telegram_api_client::{file::File, GetFile, MockTransport, TelegramClient};
        ///
        /// # async fn demo() -> telegram_api_client::Result<()> {
        /// let client = TelegramClient::new("123:ABC", MockTransport::new());
        /// let file: File = client.get_file(&GetFile::new("AgACAgI")).await?;
        /// assert!(!file.is_downloadable());
        /// # Ok(()) }
        /// ```
        get_file, "getFile", GetFile, File
    );

    /// Fetches bot identity — the cheapest call to validate a token.
    ///
    /// Deprecated: [`Self::me`] returns the typed [`User`].
    ///
    /// Returns the raw envelope, but the answer goes through the same envelope
    /// check as `me`: a body-level failure or a reply without `result` is an
    /// error rather than a value handed back as a success.
    ///
    /// # Errors
    ///
    /// [`Error::Api`] / [`Error::RateLimited`] for a failed answer,
    /// [`Error::Unexpected`] when the envelope carries no `result`, plus
    /// transport and decoding failures.
    ///
    /// ```rust
    /// use telegram_api_client::{Error, MockReply, MockTransport, TelegramClient};
    ///
    /// # async fn demo() -> telegram_api_client::Result<()> {
    /// let transport = MockTransport::new();
    /// transport.push(MockReply::result(serde_json::json!({"id": 7, "is_bot": true})));
    /// let client = TelegramClient::new("123:ABC", transport.clone());
    ///
    /// // The whole envelope, exactly as in 0.1 ...
    /// # #[allow(deprecated)]
    /// let envelope = client.get_me().await?;
    /// assert_eq!(envelope["result"]["id"], 7);
    ///
    /// // ... but an answer the transport could not judge, without a `result`,
    /// // is no longer a success.
    /// transport.push(MockReply::Raw(serde_json::json!({"ok": true})));
    /// # #[allow(deprecated)]
    /// let error = client.get_me().await.unwrap_err();
    /// assert!(matches!(error, Error::Unexpected { .. }));
    /// # Ok(()) }
    /// ```
    #[deprecated(since = "0.1.1", note = "use `me` for a typed User")]
    pub async fn get_me(&self) -> Result<serde_json::Value> {
        let response = self.call("getMe", &json!({})).await?;
        take_result::<serde_json::Value>("getMe", &response)?;
        Ok(response)
    }

    /// Like [`Self::get_me`], but typed.
    ///
    /// # Errors
    ///
    /// Transport, rate-limit and decoding failures.
    ///
    /// ```
    /// use telegram_api_client::{MockTransport, TelegramClient};
    ///
    /// # async fn demo() -> telegram_api_client::Result<()> {
    /// let client = TelegramClient::new("123:ABC", MockTransport::new());
    /// let me: telegram_api_client::User = client.me().await?;
    /// assert_eq!(me.id, 0);
    /// # Ok(()) }
    /// ```
    pub async fn me(&self) -> Result<User> {
        let response = self.call("getMe", &json!({})).await?;
        take_result("getMe", &response)
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
