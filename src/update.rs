//! Typed subset of the `Update` object.
//!
//! Only the fields this crate models are listed; Telegram's unknown fields are
//! ignored, so a newer Bot API cannot break parsing.

use serde::{Deserialize, Serialize};

use crate::types::ChatType;

/// One incoming update.
///
/// ```
/// use telegram_api_client::{ChatType, Update};
///
/// let update: Update = serde_json::from_str(
///     r#"{"update_id":1,"message":{"message_id":2,"chat":{"id":-3,"type":"supergroup"},"text":"hi"}}"#,
/// ).unwrap();
/// assert_eq!(update.update_id, 1);
/// assert_eq!(update.text(), Some("hi"));
/// assert_eq!(update.chat_id(), Some(-3));
/// assert_eq!(update.message.unwrap().chat.chat_type, ChatType::Supergroup);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Update {
    /// Monotonic update id.
    #[serde(default, alias = "updateId")]
    pub update_id: i64,
    /// Present for new messages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<Message>,
    /// Present for edited messages.
    #[serde(
        default,
        alias = "editedMessage",
        skip_serializing_if = "Option::is_none"
    )]
    pub edited_message: Option<Message>,
    /// Present for new channel posts.
    #[serde(
        default,
        alias = "channelPost",
        skip_serializing_if = "Option::is_none"
    )]
    pub channel_post: Option<Message>,
    /// Present when an inline keyboard button was pressed.
    #[serde(
        default,
        alias = "callbackQuery",
        skip_serializing_if = "Option::is_none"
    )]
    pub callback_query: Option<CallbackQuery>,
    /// Present for inline mode queries.
    #[serde(
        default,
        alias = "inlineQuery",
        skip_serializing_if = "Option::is_none"
    )]
    pub inline_query: Option<InlineQuery>,
}

/// A chat message.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Message {
    /// Message id inside the chat.
    #[serde(default, alias = "messageId")]
    pub message_id: i64,
    /// Sender, absent for channel posts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<User>,
    /// Chat the message belongs to.
    #[serde(default)]
    pub chat: Chat,
    /// Unix time.
    #[serde(default)]
    pub date: i64,
    /// Text content, absent for media-only messages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Caption of a media message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    /// Forum topic the message belongs to.
    #[serde(
        default,
        alias = "messageThreadId",
        skip_serializing_if = "Option::is_none"
    )]
    pub message_thread_id: Option<i64>,
}

/// A user or bot.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct User {
    /// Telegram user id.
    #[serde(default)]
    pub id: i64,
    /// First name.
    #[serde(default, alias = "firstName")]
    pub first_name: String,
    /// Last name.
    #[serde(default, alias = "lastName", skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
    /// Optional username without `@`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// Whether this is a bot account.
    #[serde(default, alias = "isBot", skip_serializing_if = "Option::is_none")]
    pub is_bot: Option<bool>,
}

impl User {
    /// The display name, falling back to the username and then the id.
    ///
    /// ```
    /// use telegram_api_client::User;
    ///
    /// let user = User { id: 7, first_name: "Flo".into(), ..User::default() };
    /// assert_eq!(user.display_name(), "Flo");
    /// assert_eq!(User::default().display_name(), "0");
    /// ```
    #[must_use]
    pub fn display_name(&self) -> String {
        if !self.first_name.is_empty() {
            return self.first_name.clone();
        }
        if let Some(username) = self.username.as_deref() {
            if !username.is_empty() {
                return username.to_string();
            }
        }
        self.id.to_string()
    }
}

/// The chat a message belongs to.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Chat {
    /// Chat id — negative for groups.
    #[serde(default)]
    pub id: i64,
    /// `private`, `group`, `supergroup` or `channel`.
    #[serde(rename = "type", alias = "chat_type", default)]
    pub chat_type: ChatType,
    /// Title of a group or channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Public username without `@`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// First name of a private chat.
    #[serde(default, alias = "firstName", skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    /// Last name of a private chat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
}

impl Chat {
    /// Human readable chat label: title, then name, then username, then id.
    ///
    /// ```
    /// use telegram_api_client::Chat;
    ///
    /// let chat = Chat { id: -1, title: Some("Team".into()), ..Chat::default() };
    /// assert_eq!(chat.label(), "Team");
    /// ```
    #[must_use]
    pub fn label(&self) -> String {
        if let Some(title) = self.title.as_deref() {
            if !title.is_empty() {
                return title.to_string();
            }
        }
        if let Some(first_name) = self.first_name.as_deref() {
            if !first_name.is_empty() {
                return first_name.to_string();
            }
        }
        match self.username.as_deref() {
            Some(username) if !username.is_empty() => format!("@{username}"),
            _ => self.id.to_string(),
        }
    }
}

/// A press on an inline keyboard button.
///
/// ```
/// use telegram_api_client::CallbackQuery;
///
/// let query: CallbackQuery = serde_json::from_str(
///     r#"{"id":"4382","from":{"id":1},"data":"vote:yes","chat_instance":"-9"}"#,
/// ).unwrap();
/// assert_eq!(query.data.as_deref(), Some("vote:yes"));
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct CallbackQuery {
    /// Id to pass to `answerCallbackQuery`.
    #[serde(default, alias = "callbackQueryId")]
    pub id: String,
    /// User who pressed the button.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<User>,
    /// Message the button belongs to (absent for inline messages).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<Message>,
    /// `callback_data` of the pressed button.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    /// Inline message id, when the button was on an inline message.
    #[serde(
        default,
        alias = "inlineMessageId",
        skip_serializing_if = "Option::is_none"
    )]
    pub inline_message_id: Option<String>,
    /// Opaque identifier of the chat with the bot.
    #[serde(
        default,
        alias = "chatInstance",
        skip_serializing_if = "Option::is_none"
    )]
    pub chat_instance: Option<String>,
}

impl CallbackQuery {
    /// The `callback_data` payload, if the button carried one.
    ///
    /// ```
    /// use telegram_api_client::CallbackQuery;
    ///
    /// let query = CallbackQuery { data: Some("go".into()), ..CallbackQuery::default() };
    /// assert_eq!(query.payload(), Some("go"));
    /// ```
    #[must_use]
    pub fn payload(&self) -> Option<&str> {
        self.data.as_deref()
    }
}

/// An inline mode query.
///
/// ```
/// use telegram_api_client::InlineQuery;
///
/// let query: InlineQuery = serde_json::from_str(
///     r#"{"id":"q1","from":{"id":1},"query":"ping","offset":""}"#,
/// ).unwrap();
/// assert_eq!(query.query, "ping");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct InlineQuery {
    /// Id to pass to `answerInlineQuery`.
    #[serde(default)]
    pub id: String,
    /// User who typed the query.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<User>,
    /// Text typed after the bot's username.
    #[serde(default)]
    pub query: String,
    /// Pagination offset echoed from the previous answer.
    #[serde(default)]
    pub offset: String,
}

impl Update {
    /// Returns the message text if this update is a text message.
    ///
    /// ```
    /// use telegram_api_client::Update;
    ///
    /// let update: Update = serde_json::from_str(r#"{"update_id":1}"#).unwrap();
    /// assert_eq!(update.text(), None);
    /// ```
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        self.primary_message()?.text.as_deref()
    }

    /// Returns the chat id of the message, if any.
    #[must_use]
    pub fn chat_id(&self) -> Option<i64> {
        Some(self.primary_message()?.chat.id)
    }

    /// The message this update carries: new, edited or channel post.
    ///
    /// ```
    /// use telegram_api_client::Update;
    ///
    /// let update: Update = serde_json::from_str(
    ///     r#"{"update_id":1,"channel_post":{"message_id":2,"chat":{"id":-4}}}"#,
    /// ).unwrap();
    /// assert_eq!(update.primary_message().unwrap().message_id, 2);
    /// ```
    #[must_use]
    pub fn primary_message(&self) -> Option<&Message> {
        self.message
            .as_ref()
            .or(self.edited_message.as_ref())
            .or(self.channel_post.as_ref())
    }

    /// The `callback_data` of a button press, if this update is one.
    ///
    /// ```
    /// use telegram_api_client::Update;
    ///
    /// let update: Update = serde_json::from_str(
    ///     r#"{"update_id":1,"callback_query":{"id":"9","data":"ok"}}"#,
    /// ).unwrap();
    /// assert_eq!(update.callback_data(), Some("ok"));
    /// ```
    #[must_use]
    pub fn callback_data(&self) -> Option<&str> {
        self.callback_query.as_ref()?.payload()
    }

    /// The id needed to answer a callback query, if this update is one.
    #[must_use]
    pub fn callback_query_id(&self) -> Option<&str> {
        Some(self.callback_query.as_ref()?.id.as_str())
    }
}
