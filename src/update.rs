//! Typed subset of the `Update` object.

use serde::{Deserialize, Serialize};

/// One incoming update.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Update {
    /// Monotonic update id.
    pub update_id: i64,
    /// Present for new messages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<Message>,
}

/// A chat message.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Message {
    /// Message id inside the chat.
    pub message_id: i64,
    /// Sender, absent for channel posts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<User>,
    /// Chat the message belongs to.
    pub chat: Chat,
    /// Unix time.
    #[serde(default)]
    pub date: i64,
    /// Text content, absent for media-only messages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// A user or bot.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct User {
    /// Telegram user id.
    pub id: i64,
    /// First name.
    #[serde(default)]
    pub first_name: String,
    /// Optional username without `@`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
}

/// The chat a message belongs to.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Chat {
    /// Chat id — negative for groups.
    pub id: i64,
    /// `private`, `group`, `supergroup` or `channel`.
    #[serde(rename = "type", default)]
    pub chat_type: String,
}

impl Update {
    /// Returns the message text if this update is a text message.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        self.message.as_ref()?.text.as_deref()
    }

    /// Returns the chat id of the message, if any.
    #[must_use]
    pub fn chat_id(&self) -> Option<i64> {
        Some(self.message.as_ref()?.chat.id)
    }
}
