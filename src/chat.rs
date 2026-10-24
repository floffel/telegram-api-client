//! Chat administration: actions, lookups, bans and pins.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::send::validate_chat;
use crate::types::{ChatAction, ChatId, ChatMemberStatus};

/// `sendChatAction` body — the "typing…" indicator.
///
/// ```
/// use telegram_api_client::{ChatAction, SendChatAction};
///
/// let action = SendChatAction::new(-1001, ChatAction::Typing);
/// assert_eq!(serde_json::to_value(&action).unwrap()["action"], "typing");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct SendChatAction {
    /// Target chat.
    pub chat_id: ChatId,
    /// What the bot pretends to be doing.
    pub action: ChatAction,
    /// Forum topic the action belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_thread_id: Option<i64>,
}

impl SendChatAction {
    /// Shows `action` in `chat_id`.
    #[must_use]
    pub fn new(chat_id: impl Into<ChatId>, action: ChatAction) -> Self {
        Self {
            chat_id: chat_id.into(),
            action,
            message_thread_id: None,
        }
    }

    /// The `typing…` indicator.
    #[must_use]
    pub fn typing(chat_id: impl Into<ChatId>) -> Self {
        Self::new(chat_id, ChatAction::Typing)
    }

    /// Checks the chat reference and the action.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an unusable chat or an empty action.
    pub fn validate(&self) -> Result<()> {
        validate_chat("chat_id", &self.chat_id)?;
        if self.action.as_str().is_empty() {
            return Err(Error::InvalidRequest {
                field: "action",
                reason: "must not be empty".to_string(),
            });
        }
        Ok(())
    }
}

/// `getChat` body.
///
/// ```
/// use telegram_api_client::GetChat;
///
/// let request = GetChat::new("@floffel");
/// assert_eq!(serde_json::to_value(&request).unwrap()["chat_id"], "floffel");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct GetChat {
    /// Chat to look up.
    pub chat_id: ChatId,
}

impl GetChat {
    /// Looks up `chat_id`.
    #[must_use]
    pub fn new(chat_id: impl Into<ChatId>) -> Self {
        Self {
            chat_id: chat_id.into(),
        }
    }

    /// Checks the chat reference.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an unusable chat reference.
    pub fn validate(&self) -> Result<()> {
        validate_chat("chat_id", &self.chat_id)
    }
}

/// `getChatMember` body.
///
/// ```
/// use telegram_api_client::GetChatMember;
///
/// let request = GetChatMember::new(-1001, 42);
/// assert_eq!(serde_json::to_value(&request).unwrap()["user_id"], 42);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct GetChatMember {
    /// Chat to inspect.
    pub chat_id: ChatId,
    /// User to look up.
    pub user_id: i64,
}

impl GetChatMember {
    /// Looks up `user_id` in `chat_id`.
    #[must_use]
    pub fn new(chat_id: impl Into<ChatId>, user_id: i64) -> Self {
        Self {
            chat_id: chat_id.into(),
            user_id,
        }
    }

    /// Checks chat and user id.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an unusable chat or a non-positive user id.
    pub fn validate(&self) -> Result<()> {
        validate_chat("chat_id", &self.chat_id)?;
        if self.user_id <= 0 {
            return Err(Error::InvalidRequest {
                field: "user_id",
                reason: format!("must be positive, got {}", self.user_id),
            });
        }
        Ok(())
    }
}

/// `banChatMember` body.
///
/// ```
/// use telegram_api_client::BanChatMember;
///
/// let ban = BanChatMember::new(-1001, 42).until(0);
/// let json = serde_json::to_value(&ban).unwrap();
/// assert_eq!(json["user_id"], 42);
/// assert_eq!(json["until_date"], 0);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct BanChatMember {
    /// Chat to act on.
    pub chat_id: ChatId,
    /// User to ban.
    pub user_id: i64,
    /// Unix time until the ban expires; `0` means forever.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until_date: Option<i64>,
    /// Also delete the user's recent messages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoke_messages: Option<bool>,
}

impl BanChatMember {
    /// Bans `user_id` from `chat_id` until unbanned.
    #[must_use]
    pub fn new(chat_id: impl Into<ChatId>, user_id: i64) -> Self {
        Self {
            chat_id: chat_id.into(),
            user_id,
            until_date: None,
            revoke_messages: None,
        }
    }

    /// Sets the expiry (`0` = forever).
    #[must_use]
    pub fn until(mut self, unix_seconds: i64) -> Self {
        self.until_date = Some(unix_seconds);
        self
    }

    /// Also removes the user's messages.
    #[must_use]
    pub fn revoking_messages(mut self) -> Self {
        self.revoke_messages = Some(true);
        self
    }

    /// Checks chat, user id and expiry.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an unusable chat, a non-positive user id,
    /// or a negative expiry.
    pub fn validate(&self) -> Result<()> {
        validate_chat("chat_id", &self.chat_id)?;
        if self.user_id <= 0 {
            return Err(Error::InvalidRequest {
                field: "user_id",
                reason: format!("must be positive, got {}", self.user_id),
            });
        }
        if self.until_date.is_some_and(|date| date < 0) {
            return Err(Error::InvalidRequest {
                field: "until_date",
                reason: "must be 0 (forever) or a future unix timestamp".to_string(),
            });
        }
        Ok(())
    }
}

/// `pinChatMessage` body.
///
/// ```
/// use telegram_api_client::PinChatMessage;
///
/// let pin = PinChatMessage::new(-1001, 7).silent();
/// assert_eq!(serde_json::to_value(&pin).unwrap()["disable_notification"], true);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct PinChatMessage {
    /// Chat to act on.
    pub chat_id: ChatId,
    /// Message to pin.
    pub message_id: i64,
    /// Pin without a notification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_notification: Option<bool>,
}

impl PinChatMessage {
    /// Pins `message_id` in `chat_id`.
    #[must_use]
    pub fn new(chat_id: impl Into<ChatId>, message_id: i64) -> Self {
        Self {
            chat_id: chat_id.into(),
            message_id,
            disable_notification: None,
        }
    }

    /// Pins without a notification.
    #[must_use]
    pub fn silent(mut self) -> Self {
        self.disable_notification = Some(true);
        self
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

/// `unpinChatMessage` body.
///
/// ```
/// use telegram_api_client::UnpinChatMessage;
///
/// // Without a message id Telegram unpins the most recent pin.
/// assert_eq!(serde_json::to_value(UnpinChatMessage::new(-1001)).unwrap()["chat_id"], -1001);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct UnpinChatMessage {
    /// Chat to act on.
    pub chat_id: ChatId,
    /// Message to unpin; omit to unpin the most recent one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<i64>,
}

impl UnpinChatMessage {
    /// Unpins the most recent pinned message.
    #[must_use]
    pub fn new(chat_id: impl Into<ChatId>) -> Self {
        Self {
            chat_id: chat_id.into(),
            message_id: None,
        }
    }

    /// Unpins a specific message.
    #[must_use]
    pub fn message(mut self, message_id: i64) -> Self {
        self.message_id = Some(message_id);
        self
    }

    /// Checks chat and the optional message id.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an unusable chat or a non-positive id.
    pub fn validate(&self) -> Result<()> {
        validate_chat("chat_id", &self.chat_id)?;
        if self.message_id.is_some_and(|id| id <= 0) {
            return Err(Error::InvalidRequest {
                field: "message_id",
                reason: "must be positive when set".to_string(),
            });
        }
        Ok(())
    }
}

/// One member of a chat, as returned by `getChatMember`.
///
/// ```
/// use telegram_api_client::{ChatMember, ChatMemberStatus};
///
/// let member: ChatMember = serde_json::from_str(
///     r#"{"status":"creator","user":{"id":42,"first_name":"Flo"},"is_anonymous":false}"#,
/// ).unwrap();
/// assert_eq!(member.status, ChatMemberStatus::Creator);
/// assert_eq!(member.user.id, 42);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ChatMember {
    /// Membership status.
    #[serde(default)]
    pub status: ChatMemberStatus,
    /// The user behind the membership.
    #[serde(default)]
    pub user: crate::update::User,
    /// Whether the member hides behind the chat identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_anonymous: Option<bool>,
}

impl ChatMember {
    /// Whether the user is currently in the chat (any non-`left`/`kicked` state).
    ///
    /// ```
    /// use telegram_api_client::{ChatMember, ChatMemberStatus};
    ///
    /// let mut member = ChatMember::default();
    /// assert!(!member.is_present());
    /// member.status = ChatMemberStatus::Member;
    /// assert!(member.is_present());
    /// ```
    #[must_use]
    pub fn is_present(&self) -> bool {
        !matches!(
            self.status,
            ChatMemberStatus::Left | ChatMemberStatus::Kicked | ChatMemberStatus::Unknown(_)
        )
    }
}
