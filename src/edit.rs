//! Editing, deleting, forwarding and copying messages.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, Result};
use crate::send::{text_field, validate_chat, validate_text, MAX_CAPTION, MAX_MESSAGE_TEXT};
use crate::transport::MultipartForm;
use crate::types::{ChatId, InputMedia, LinkPreviewOptions, MessageEntity, ParseMode, ReplyMarkup};

/// Identifies the message an edit applies to: either `chat_id` + `message_id`,
/// or the id of an inline message.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct EditTarget {
    /// Chat containing the message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_id: Option<ChatId>,
    /// Message to edit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<i64>,
    /// Inline message id, for messages sent via the bot inline mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inline_message_id: Option<String>,
}

impl EditTarget {
    /// A message in a chat.
    #[must_use]
    pub fn chat(chat_id: impl Into<ChatId>, message_id: i64) -> Self {
        Self {
            chat_id: Some(chat_id.into()),
            message_id: Some(message_id),
            inline_message_id: None,
        }
    }

    /// An inline message.
    #[must_use]
    pub fn inline(inline_message_id: impl Into<String>) -> Self {
        Self {
            chat_id: None,
            message_id: None,
            inline_message_id: Some(inline_message_id.into()),
        }
    }

    /// Exactly one addressing form must be present, and a chat id needs a
    /// message id.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] when both forms are given, when neither is,
    /// when only one half of `chat_id`/`message_id` is set, or when the
    /// message id is not positive.
    pub fn validate(&self) -> Result<()> {
        let inline = self
            .inline_message_id
            .as_deref()
            .is_some_and(|id| !id.trim().is_empty());
        let chat = self.chat_id.is_some() || self.message_id.is_some();

        if inline && chat {
            return Err(Error::InvalidRequest {
                field: "inline_message_id",
                reason: "use either chat_id + message_id or inline_message_id, not both"
                    .to_string(),
            });
        }
        if inline {
            return Ok(());
        }
        match (self.chat_id.as_ref(), self.message_id) {
            (Some(chat_id), Some(message_id)) => {
                validate_chat("chat_id", chat_id)?;
                if message_id <= 0 {
                    return Err(Error::InvalidRequest {
                        field: "message_id",
                        reason: format!("must be positive, got {message_id}"),
                    });
                }
                Ok(())
            }
            _ => Err(Error::InvalidRequest {
                field: "message_id",
                reason: "chat_id + message_id or inline_message_id is required".to_string(),
            }),
        }
    }
}

/// `editMessageText` body.
///
/// ```
/// use telegram_api_client::{EditMessageText, ParseMode};
///
/// let edit = EditMessageText::new(-1001, 7, "corrected")
///     .with_parse_mode(ParseMode::MarkdownV2);
/// let json = serde_json::to_value(&edit).unwrap();
/// assert_eq!(json["message_id"], 7);
/// assert_eq!(json["text"], "corrected");
/// assert_eq!(json["parse_mode"], "MarkdownV2");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct EditMessageText {
    /// Message to edit.
    #[serde(flatten)]
    pub target: EditTarget,
    /// Replacement text.
    pub text: String,
    /// How Telegram should parse the text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parse_mode: Option<ParseMode>,
    /// Pre-computed formatting entities.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entities: Vec<MessageEntity>,
    /// Link preview behaviour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_preview_options: Option<LinkPreviewOptions>,
    /// Replacement keyboard.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_markup: Option<ReplyMarkup>,
}

impl EditMessageText {
    /// Edits the text of a message in a chat.
    #[must_use]
    pub fn new(chat_id: impl Into<ChatId>, message_id: i64, text: impl Into<String>) -> Self {
        Self {
            target: EditTarget::chat(chat_id, message_id),
            text: text.into(),
            ..Self::default()
        }
    }

    /// Edits the text of an inline message.
    #[must_use]
    pub fn for_inline(inline_message_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            target: EditTarget::inline(inline_message_id),
            text: text.into(),
            ..Self::default()
        }
    }

    /// Sets the parse mode.
    #[must_use]
    pub fn with_parse_mode(mut self, mode: ParseMode) -> Self {
        self.parse_mode = Some(mode);
        self
    }

    /// Attaches formatting entities.
    #[must_use]
    pub fn with_entities(mut self, entities: Vec<MessageEntity>) -> Self {
        self.entities = entities;
        self
    }

    /// Overrides link previews.
    #[must_use]
    pub fn with_link_preview(mut self, options: LinkPreviewOptions) -> Self {
        self.link_preview_options = Some(options);
        self
    }

    /// Replaces the keyboard.
    #[must_use]
    pub fn with_reply_markup(mut self, markup: ReplyMarkup) -> Self {
        self.reply_markup = Some(markup);
        self
    }

    /// Checks the target and the text (empty text deletes nothing — Telegram
    /// rejects it).
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an invalid target, empty text, or text
    /// over [`MAX_MESSAGE_TEXT`].
    pub fn validate(&self) -> Result<()> {
        self.target.validate()?;
        validate_text("text", &self.text, MAX_MESSAGE_TEXT)
    }
}

/// `editMessageCaption` body.
///
/// ```
/// use telegram_api_client::EditMessageCaption;
///
/// let edit = EditMessageCaption::new(-1001, 7).with_caption("new caption");
/// let json = serde_json::to_value(&edit).unwrap();
/// assert_eq!(json["caption"], "new caption");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct EditMessageCaption {
    /// Message to edit.
    #[serde(flatten)]
    pub target: EditTarget,
    /// Replacement caption; omit to remove it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    /// How Telegram should parse the caption.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parse_mode: Option<ParseMode>,
    /// Pre-computed caption entities.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub caption_entities: Vec<MessageEntity>,
    /// Put the caption above the media.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_caption_above_media: Option<bool>,
    /// Replacement keyboard.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_markup: Option<ReplyMarkup>,
}

impl EditMessageCaption {
    /// Edits the caption of a message in a chat.
    #[must_use]
    pub fn new(chat_id: impl Into<ChatId>, message_id: i64) -> Self {
        Self {
            target: EditTarget::chat(chat_id, message_id),
            ..Self::default()
        }
    }

    /// Edits the caption of an inline message.
    #[must_use]
    pub fn for_inline(inline_message_id: impl Into<String>) -> Self {
        Self {
            target: EditTarget::inline(inline_message_id),
            ..Self::default()
        }
    }

    /// Sets the caption.
    #[must_use]
    pub fn with_caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = Some(caption.into());
        self
    }

    /// Sets the caption parse mode.
    #[must_use]
    pub fn with_parse_mode(mut self, mode: ParseMode) -> Self {
        self.parse_mode = Some(mode);
        self
    }

    /// Replaces the keyboard.
    #[must_use]
    pub fn with_reply_markup(mut self, markup: ReplyMarkup) -> Self {
        self.reply_markup = Some(markup);
        self
    }

    /// Checks the target and the caption length.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an invalid target, or a caption that is
    /// empty when set or longer than [`MAX_CAPTION`].
    pub fn validate(&self) -> Result<()> {
        self.target.validate()?;
        match self.caption.as_deref() {
            None => Ok(()),
            Some(caption) => validate_text("caption", caption, MAX_CAPTION),
        }
    }
}

/// `editMessageMedia` body: replaces the media of an existing message.
///
/// ```rust
/// use telegram_api_client::{EditMessageMedia, InputMedia};
///
/// let edit = EditMessageMedia::new(1, 7, InputMedia::video("DQACAgI"));
/// let json = serde_json::to_value(&edit).unwrap();
/// assert_eq!(json["media"]["type"], "video");
/// assert_eq!(json["message_id"], 7);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct EditMessageMedia {
    /// Message to edit.
    #[serde(flatten)]
    pub target: EditTarget,
    /// Replacement media.
    pub media: InputMedia,
    /// Replacement keyboard.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_markup: Option<ReplyMarkup>,
}

impl EditMessageMedia {
    /// Replaces the media of a message in a chat.
    ///
    /// ```rust
    /// use telegram_api_client::{EditMessageMedia, InputMedia};
    ///
    /// let edit = EditMessageMedia::new(-1001, 7, InputMedia::photo("AgACAgI"))
    ///     .with_caption("new picture");
    /// assert!(edit.validate().is_ok());
    /// ```
    #[must_use]
    pub fn new(chat_id: impl Into<ChatId>, message_id: i64, media: InputMedia) -> Self {
        Self {
            target: EditTarget::chat(chat_id, message_id),
            media,
            reply_markup: None,
        }
    }

    /// Replaces the media of an inline message.
    ///
    /// ```rust
    /// use telegram_api_client::{EditMessageMedia, InputMedia};
    ///
    /// let edit = EditMessageMedia::for_inline("inline-1", InputMedia::photo("AgACAgI"));
    /// assert_eq!(edit.target.inline_message_id.as_deref(), Some("inline-1"));
    /// ```
    #[must_use]
    pub fn for_inline(inline_message_id: impl Into<String>, media: InputMedia) -> Self {
        Self {
            target: EditTarget::inline(inline_message_id),
            media,
            reply_markup: None,
        }
    }

    /// Adds or replaces the caption of the new media.
    #[must_use]
    pub fn with_caption(mut self, caption: impl Into<String>) -> Self {
        self.media = self.media.with_caption(caption);
        self
    }

    /// Replaces the keyboard.
    #[must_use]
    pub fn with_reply_markup(mut self, markup: ReplyMarkup) -> Self {
        self.reply_markup = Some(markup);
        self
    }

    /// Checks the target, the media reference or upload, and the caption.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an invalid target, an empty media
    /// reference, an upload that is empty, too large, misnamed or of the wrong
    /// MIME family for the item kind, or a caption that is empty or too long.
    pub fn validate(&self) -> Result<()> {
        self.target.validate()?;
        if self.media.media.trim().is_empty() {
            return Err(Error::InvalidRequest {
                field: "media",
                reason: "media reference must not be empty".to_string(),
            });
        }
        if let Some(upload) = &self.media.upload {
            upload.validate_with_limit(
                "media",
                self.media.kind.mime_families(),
                self.media.kind.upload_limit_bytes(),
            )?;
        }
        match self.media.caption.as_deref() {
            None => Ok(()),
            Some(caption) => validate_text("caption", caption, MAX_CAPTION),
        }
    }

    /// The multipart body for this edit, or `None` when the new media is
    /// referenced by `file_id`/URL.
    ///
    /// ```rust
    /// use telegram_api_client::{EditMessageMedia, InputMedia, UploadFile};
    ///
    /// let edit = EditMessageMedia::new(
    ///     1,
    ///     7,
    ///     InputMedia::photo("ignored").with_upload(UploadFile::new("new.png", vec![1, 2])),
    /// );
    /// let form = edit.multipart_form().unwrap();
    /// assert_eq!(form.files()[0].field, "file0");
    /// assert!(form.json_fields()["media"].as_str().unwrap().contains("attach://file0"));
    ///
    /// let json_only = EditMessageMedia::new(1, 7, InputMedia::photo("AgACAgI"));
    /// assert!(json_only.multipart_form().is_none());
    /// ```
    #[must_use]
    pub fn multipart_form(&self) -> Option<MultipartForm> {
        let upload = self.media.upload.as_ref()?;
        let body = serde_json::to_value(self).ok()?;
        let Value::Object(map) = body else {
            return None;
        };

        let mut form = MultipartForm::new();
        for (key, value) in map {
            if key == "media" {
                let Value::Object(mut object) = value else {
                    form = form.field(key, text_field(&value));
                    continue;
                };
                object.insert(
                    "media".to_string(),
                    Value::String("attach://file0".to_string()),
                );
                form = form.field(key, Value::Object(object));
                continue;
            }
            form = form.field(key, text_field(&value));
        }
        Some(form.with_file(upload.to_part("file0")))
    }
}

/// `editMessageReplyMarkup` body.
///
/// ```
/// use telegram_api_client::{EditMessageReplyMarkup, InlineKeyboardButton, InlineKeyboardMarkup, ReplyMarkup};
///
/// let markup = ReplyMarkup::inline(InlineKeyboardMarkup::new(vec![vec![
///     InlineKeyboardButton::callback("Done", "done"),
/// ]]));
/// let edit = EditMessageReplyMarkup::new(42, 9).with_reply_markup(markup);
/// assert_eq!(serde_json::to_value(&edit).unwrap()["message_id"], 9);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct EditMessageReplyMarkup {
    /// Message to edit.
    #[serde(flatten)]
    pub target: EditTarget,
    /// New keyboard; omit to remove the existing one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_markup: Option<ReplyMarkup>,
}

impl EditMessageReplyMarkup {
    /// Edits the keyboard of a message in a chat.
    #[must_use]
    pub fn new(chat_id: impl Into<ChatId>, message_id: i64) -> Self {
        Self {
            target: EditTarget::chat(chat_id, message_id),
            ..Self::default()
        }
    }

    /// Edits the keyboard of an inline message.
    #[must_use]
    pub fn for_inline(inline_message_id: impl Into<String>) -> Self {
        Self {
            target: EditTarget::inline(inline_message_id),
            ..Self::default()
        }
    }

    /// Replaces the keyboard.
    #[must_use]
    pub fn with_reply_markup(mut self, markup: ReplyMarkup) -> Self {
        self.reply_markup = Some(markup);
        self
    }

    /// Checks the target.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an invalid target.
    pub fn validate(&self) -> Result<()> {
        self.target.validate()
    }
}

/// `deleteMessage` body.
///
/// ```
/// use telegram_api_client::DeleteMessage;
///
/// let delete = DeleteMessage::new(-1001, 7);
/// assert_eq!(serde_json::to_value(&delete).unwrap()["message_id"], 7);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct DeleteMessage {
    /// Chat containing the message.
    pub chat_id: ChatId,
    /// Message to delete.
    pub message_id: i64,
}

impl DeleteMessage {
    /// Deletes `message_id`.
    #[must_use]
    pub fn new(chat_id: impl Into<ChatId>, message_id: i64) -> Self {
        Self {
            chat_id: chat_id.into(),
            message_id,
        }
    }

    /// Checks chat and message id.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an unusable chat or a non-positive id.
    pub fn validate(&self) -> Result<()> {
        validate_chat("chat_id", &self.chat_id)?;
        if self.message_id <= 0 {
            return Err(Error::InvalidRequest {
                field: "message_id",
                reason: format!("must be positive, got {}", self.message_id),
            });
        }
        Ok(())
    }
}

/// `forwardMessage` body.
///
/// ```
/// use telegram_api_client::ForwardMessage;
///
/// let forward = ForwardMessage::new(42, -1001, 7).silent();
/// let json = serde_json::to_value(&forward).unwrap();
/// assert_eq!(json["from_chat_id"], -1001);
/// assert_eq!(json["disable_notification"], true);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct ForwardMessage {
    /// Destination chat.
    pub chat_id: ChatId,
    /// Chat the original message lives in.
    pub from_chat_id: ChatId,
    /// Message to forward.
    pub message_id: i64,
    /// Forum topic to post into.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_thread_id: Option<i64>,
    /// Forward without a notification sound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_notification: Option<bool>,
    /// Prevent forwarding and saving.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protect_content: Option<bool>,
}

impl ForwardMessage {
    /// Forwards `message_id` from `from_chat_id` to `chat_id`.
    #[must_use]
    pub fn new(
        chat_id: impl Into<ChatId>,
        from_chat_id: impl Into<ChatId>,
        message_id: i64,
    ) -> Self {
        Self {
            chat_id: chat_id.into(),
            from_chat_id: from_chat_id.into(),
            message_id,
            ..Self::default()
        }
    }

    /// Forwards silently.
    #[must_use]
    pub fn silent(mut self) -> Self {
        self.disable_notification = Some(true);
        self
    }

    /// Checks both chats and the message id.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an unusable chat or a non-positive id.
    pub fn validate(&self) -> Result<()> {
        validate_chat("chat_id", &self.chat_id)?;
        validate_chat("from_chat_id", &self.from_chat_id)?;
        if self.message_id <= 0 {
            return Err(Error::InvalidRequest {
                field: "message_id",
                reason: format!("must be positive, got {}", self.message_id),
            });
        }
        Ok(())
    }
}

/// `copyMessage` body — like forwarding, but without the "forwarded from" label.
///
/// ```
/// use telegram_api_client::CopyMessage;
///
/// let copy = CopyMessage::new(42, -1001, 7).with_caption("look");
/// let json = serde_json::to_value(&copy).unwrap();
/// assert_eq!(json["message_id"], 7);
/// assert_eq!(json["caption"], "look");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct CopyMessage {
    /// Destination chat.
    pub chat_id: ChatId,
    /// Chat the original message lives in.
    pub from_chat_id: ChatId,
    /// Message to copy.
    pub message_id: i64,
    /// Replacement caption; omit to keep the original.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    /// How Telegram should parse the caption.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parse_mode: Option<ParseMode>,
    /// Pre-computed caption entities.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub caption_entities: Vec<MessageEntity>,
    /// Copy without a notification sound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_notification: Option<bool>,
    /// Prevent forwarding and saving of the copy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protect_content: Option<bool>,
    /// Reply to this message id in the destination chat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to_message_id: Option<i64>,
    /// Deliver even when the replied-to message is gone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_sending_without_reply: Option<bool>,
    /// Keyboard attached to the copy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_markup: Option<ReplyMarkup>,
}

impl CopyMessage {
    /// Copies `message_id` from `from_chat_id` to `chat_id`.
    #[must_use]
    pub fn new(
        chat_id: impl Into<ChatId>,
        from_chat_id: impl Into<ChatId>,
        message_id: i64,
    ) -> Self {
        Self {
            chat_id: chat_id.into(),
            from_chat_id: from_chat_id.into(),
            message_id,
            ..Self::default()
        }
    }

    /// Replaces the caption.
    #[must_use]
    pub fn with_caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = Some(caption.into());
        self
    }

    /// Copies silently.
    #[must_use]
    pub fn silent(mut self) -> Self {
        self.disable_notification = Some(true);
        self
    }

    /// Checks both chats, the message id and the caption.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an unusable chat, a non-positive id, or a
    /// caption that is empty when set or longer than [`MAX_CAPTION`].
    pub fn validate(&self) -> Result<()> {
        validate_chat("chat_id", &self.chat_id)?;
        validate_chat("from_chat_id", &self.from_chat_id)?;
        if self.message_id <= 0 {
            return Err(Error::InvalidRequest {
                field: "message_id",
                reason: format!("must be positive, got {}", self.message_id),
            });
        }
        match self.caption.as_deref() {
            None => Ok(()),
            Some(caption) => validate_text("caption", caption, MAX_CAPTION),
        }
    }
}

/// What an edit method returned.
///
/// Telegram answers with the edited [`Message`](crate::Message) for chat
/// messages, but with a bare `true` for inline messages.
///
/// ```
/// use telegram_api_client::EditResult;
///
/// let chat: EditResult = serde_json::from_str(
///     r#"{"message_id":7,"chat":{"id":1,"type":"private"},"text":"x"}"#,
/// ).unwrap();
/// assert!(matches!(chat, EditResult::Message(_)));
///
/// let inline: EditResult = serde_json::from_str("true").unwrap();
/// assert_eq!(inline, EditResult::Inline(true));
/// ```
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum EditResult {
    /// The edited message.
    Message(Box<crate::update::Message>),
    /// `true` for inline messages.
    Inline(bool),
}

/// Response of `copyMessage`: the new message id.
///
/// ```
/// use telegram_api_client::MessageId;
///
/// let created: MessageId = serde_json::from_str(r#"{"message_id": 11}"#).unwrap();
/// assert_eq!(created.message_id, 11);
/// ```
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MessageId {
    /// Identifier of the created message.
    #[serde(default)]
    pub message_id: i64,
}
