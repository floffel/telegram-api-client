//! Chat administration: actions, lookups, bans and pins.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::send::{validate_chat, validate_thread};
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
    ///
    /// The id of the topic's opening message; must be positive when set.
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

    /// Checks the chat reference, the action and the forum-topic id.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an unusable chat, an empty action, or a
    /// `message_thread_id` that is set but not positive.
    ///
    /// ```
    /// use telegram_api_client::{ChatAction, Error, SendChatAction};
    ///
    /// // Outside a forum topic: nothing to check.
    /// assert!(SendChatAction::typing(-1001).validate().is_ok());
    ///
    /// // A topic's opening message id, so the smallest usable value is 1.
    /// let mut action = SendChatAction::new(-1001, ChatAction::Typing);
    /// action.message_thread_id = Some(1);
    /// assert!(action.validate().is_ok());
    ///
    /// // 0 names no message, so it names no thread either.
    /// action.message_thread_id = Some(0);
    /// assert!(matches!(
    ///     action.validate(),
    ///     Err(Error::InvalidRequest { field: "message_thread_id", .. })
    /// ));
    /// ```
    pub fn validate(&self) -> Result<()> {
        validate_chat("chat_id", &self.chat_id)?;
        if self.action.as_str().is_empty() {
            return Err(Error::InvalidRequest {
                field: "action",
                reason: "must not be empty".to_string(),
            });
        }
        validate_thread("message_thread_id", self.message_thread_id)
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

/// Where a [`ChatMember`] stands with respect to its chat.
///
/// `getChatMember` does not answer with a plain yes/no. A `restricted` member
/// carries an `is_member` flag that can be `false` for a user who is currently
/// outside the chat, and a status a newer Bot API introduced says nothing about
/// presence at all. Keeping "not there" and "not said" apart is what keeps a
/// presence check from turning into a false positive.
///
/// ```
/// use telegram_api_client::{ChatMember, ChatMemberPresence, ChatMemberStatus};
///
/// let mut member = ChatMember::default();
/// member.status = ChatMemberStatus::Restricted;
/// member.is_member = Some(false);
/// assert_eq!(member.presence(), ChatMemberPresence::Absent);
///
/// member.status = ChatMemberStatus::from("ghost");
/// assert_eq!(member.presence(), ChatMemberPresence::Unknown);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ChatMemberPresence {
    /// The user is in the chat.
    Present,
    /// The user is not in the chat: they left, were banned, or are restricted
    /// without membership.
    Absent,
    /// The payload does not answer the question, because its status is one this
    /// crate does not know yet. Neither present nor absent.
    Unknown,
}

/// One member of a chat, as returned by `getChatMember`.
///
/// `status` and `user` are mandatory in the Bot API, and they are mandatory
/// here too: a payload missing either one fails to decode instead of turning
/// into a default member, so a truncated response can never masquerade as a
/// real identity. A `status` this crate does not know yet is still carried as
/// [`ChatMemberStatus::Unknown`] — that is a new value, not a missing one.
///
/// ```
/// use telegram_api_client::{ChatMember, ChatMemberStatus};
///
/// let member: ChatMember = serde_json::from_str(
///     r#"{"status":"creator","user":{"id":42,"first_name":"Flo"},"is_anonymous":false}"#,
/// ).unwrap();
/// assert_eq!(member.status, ChatMemberStatus::Creator);
/// assert_eq!(member.user.id, 42);
///
/// // A restricted member says whether they are still in the chat.
/// let restricted: ChatMember = serde_json::from_str(
///     r#"{"status":"restricted","user":{"id":42},"is_member":false}"#,
/// ).unwrap();
/// assert_eq!(restricted.is_member, Some(false));
///
/// // Unknown status values are carried, not rejected.
/// let fresh: ChatMember = serde_json::from_str(
///     r#"{"status":"ghost","user":{"id":42,"first_name":"Flo"}}"#,
/// ).unwrap();
/// assert_eq!(fresh.status, ChatMemberStatus::Unknown("ghost".to_string()));
///
/// // Without a user there is no member to report: the payload is rejected.
/// assert!(serde_json::from_str::<ChatMember>(r#"{"status":"creator"}"#).is_err());
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ChatMember {
    /// Membership status. Required: its absence is a decode error, never
    /// [`ChatMemberStatus::Unknown`].
    pub status: ChatMemberStatus,
    /// The user behind the membership. Required: its absence is a decode
    /// error, never a fabricated default user.
    pub user: crate::update::User,
    /// Whether the member hides behind the chat identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_anonymous: Option<bool>,
    /// Whether a *restricted* member is still in the chat.
    ///
    /// Telegram sends this flag, and only this flag, inside
    /// `ChatMemberRestricted`, where it is required. It stays `None` for every
    /// other status and for a payload that left it out. Only
    /// [`ChatMemberStatus::Restricted`] consults it: a `creator` carrying
    /// `is_member: false` is still the owner, so a crafted flag cannot make a
    /// present member disappear.
    ///
    /// ```
    /// use telegram_api_client::ChatMember;
    ///
    /// let member: ChatMember = serde_json::from_str(
    ///     r#"{"status":"restricted","user":{"id":42},"isMember":true}"#,
    /// ).unwrap();
    /// assert_eq!(member.is_member, Some(true));
    ///
    /// // Every other status has no such flag, so it stays unset.
    /// let creator: ChatMember =
    ///     serde_json::from_str(r#"{"status":"creator","user":{"id":42}}"#).unwrap();
    /// assert_eq!(creator.is_member, None);
    /// ```
    #[serde(default, alias = "isMember", skip_serializing_if = "Option::is_none")]
    pub is_member: Option<bool>,
}

impl ChatMember {
    /// Where this member stands: present, absent, or not answered by the
    /// payload.
    ///
    /// `restricted` is the one status whose answer depends on
    /// [`is_member`](Self::is_member): the flag on `false` means the user is
    /// outside the chat, while a flag that is absent is not read as an absence.
    /// `left` and `kicked` are absent; `creator`, `administrator` and `member`
    /// are present. Any other status is [`ChatMemberPresence::Unknown`] — a new
    /// value from a newer Bot API is not the same thing as "not in the chat".
    ///
    /// ```
    /// use telegram_api_client::{ChatMember, ChatMemberPresence, ChatMemberStatus};
    ///
    /// let mut member = ChatMember::default();
    /// member.status = ChatMemberStatus::Member;
    /// assert_eq!(member.presence(), ChatMemberPresence::Present);
    ///
    /// // A restricted user who is not in the chat is absent, not present.
    /// member.status = ChatMemberStatus::Restricted;
    /// member.is_member = Some(false);
    /// assert_eq!(member.presence(), ChatMemberPresence::Absent);
    ///
    /// // A restricted user without the flag is not turned absent on a guess.
    /// member.is_member = None;
    /// assert_eq!(member.presence(), ChatMemberPresence::Present);
    ///
    /// // A status from a newer Bot API answers neither way.
    /// member.status = ChatMemberStatus::from("ghost");
    /// assert_eq!(member.presence(), ChatMemberPresence::Unknown);
    /// ```
    #[must_use]
    pub fn presence(&self) -> ChatMemberPresence {
        match self.status {
            ChatMemberStatus::Creator
            | ChatMemberStatus::Administrator
            | ChatMemberStatus::Member => ChatMemberPresence::Present,
            ChatMemberStatus::Restricted => {
                if self.is_member == Some(false) {
                    ChatMemberPresence::Absent
                } else {
                    ChatMemberPresence::Present
                }
            }
            ChatMemberStatus::Left | ChatMemberStatus::Kicked => ChatMemberPresence::Absent,
            // `Unknown(..)`, plus any status a later Bot API adds: the payload
            // does not say whether the user is in the chat.
            _ => ChatMemberPresence::Unknown,
        }
    }

    /// Whether the user is currently in the chat.
    ///
    /// `true` exactly when [`presence`](Self::presence) is
    /// [`ChatMemberPresence::Present`], so a restricted non-member is not
    /// present and neither is a status this crate does not know yet.
    ///
    /// ```
    /// use telegram_api_client::{ChatMember, ChatMemberPresence, ChatMemberStatus};
    ///
    /// let mut member = ChatMember::default();
    /// assert!(!member.is_present());
    ///
    /// member.status = ChatMemberStatus::Member;
    /// assert!(member.is_present());
    ///
    /// // A restricted user who is not a member of the chat.
    /// member.status = ChatMemberStatus::Restricted;
    /// member.is_member = Some(false);
    /// assert!(!member.is_present());
    /// assert_eq!(member.presence(), ChatMemberPresence::Absent);
    /// ```
    #[must_use]
    pub fn is_present(&self) -> bool {
        self.presence() == ChatMemberPresence::Present
    }
}
