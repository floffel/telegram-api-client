//! Chat administration: actions, lookups, bans and pins.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::error::{Error, Result};
use crate::send::validate_chat;
use crate::types::{ChatAction, ChatId, ChatMemberStatus};
use crate::update::User;

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

/// The owner of a chat — Telegram's `ChatMemberOwner`, `status: "creator"`.
///
/// ```
/// use telegram_api_client::ChatMemberOwner;
///
/// let owner: ChatMemberOwner =
///     serde_json::from_str(r#"{"user":{"id":1},"custom_title":"Boss"}"#).unwrap();
/// assert_eq!(owner.custom_title.as_deref(), Some("Boss"));
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ChatMemberOwner {
    /// The user who owns the chat.
    pub user: User,
    /// Whether the owner hides behind the chat identity.
    ///
    /// Absent stays `None`: the crate never turns a missing flag into
    /// "not anonymous".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_anonymous: Option<bool>,
    /// Custom title of the owner, when one was set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_title: Option<String>,
    /// Fields this crate does not model, forwarded unchanged.
    #[serde(flatten, default, skip_serializing_if = "Map::is_empty")]
    pub extra: Map<String, Value>,
}

/// An administrator — Telegram's `ChatMemberAdministrator`,
/// `status: "administrator"`.
///
/// The `can_*` rights are the member's actual permissions, not a request: an
/// absent mandatory right decodes to `false` (the safe direction — a truncated
/// payload under-reports privileges instead of inventing them), and an absent
/// optional one stays `None`, which is "not applicable to this chat type".
///
/// ```
/// use telegram_api_client::ChatMemberAdministrator;
///
/// let admin: ChatMemberAdministrator =
///     serde_json::from_str(r#"{"user":{"id":1},"can_pin_messages":true}"#).unwrap();
/// assert_eq!(admin.can_pin_messages, Some(true));
/// assert!(!admin.can_manage_chat, "an absent right is not granted");
/// ```
#[allow(clippy::struct_excessive_bools)] // a permission set is booleans, by definition
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ChatMemberAdministrator {
    /// The administrator.
    pub user: User,
    /// Whether the bot may edit this administrator's privileges.
    #[serde(default)]
    pub can_be_edited: bool,
    /// Whether the administrator hides behind the chat identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_anonymous: Option<bool>,
    /// Access the event log, the boost list and hidden members; implied by every
    /// other privilege.
    #[serde(default)]
    pub can_manage_chat: bool,
    /// Delete other users' messages.
    #[serde(default)]
    pub can_delete_messages: bool,
    /// Manage video chats.
    #[serde(default)]
    pub can_manage_video_chats: bool,
    /// Restrict, ban and unban members.
    #[serde(default)]
    pub can_restrict_members: bool,
    /// Add and demote administrators.
    #[serde(default)]
    pub can_promote_members: bool,
    /// Change the chat title, photo and other settings.
    #[serde(default)]
    pub can_change_info: bool,
    /// Invite new users.
    #[serde(default)]
    pub can_invite_users: bool,
    /// Post stories.
    #[serde(default)]
    pub can_post_stories: bool,
    /// Edit other users' stories.
    #[serde(default)]
    pub can_edit_stories: bool,
    /// Delete other users' stories.
    #[serde(default)]
    pub can_delete_stories: bool,
    /// Post in the channel; channels only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub can_post_messages: Option<bool>,
    /// Edit other users' messages; channels only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub can_edit_messages: Option<bool>,
    /// Pin messages; groups and supergroups only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub can_pin_messages: Option<bool>,
    /// Create, rename, close and reopen forum topics; supergroups only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub can_manage_topics: Option<bool>,
    /// Manage the channel's direct messages; channels only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub can_manage_direct_messages: Option<bool>,
    /// Edit the tags of regular members; groups and supergroups only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub can_manage_tags: Option<bool>,
    /// Manage chat welcome messages.
    #[serde(default)]
    pub can_send_welcome_messages: bool,
    /// Custom title of the administrator, when one was set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_title: Option<String>,
    /// Fields this crate does not model, forwarded unchanged.
    #[serde(flatten, default, skip_serializing_if = "Map::is_empty")]
    pub extra: Map<String, Value>,
}

/// A plain member — Telegram's `ChatMemberMember`, `status: "member"`.
///
/// ```
/// use telegram_api_client::ChatMemberMember;
///
/// let member: ChatMemberMember =
///     serde_json::from_str(r#"{"user":{"id":1},"tag":"vip"}"#).unwrap();
/// assert_eq!(member.tag.as_deref(), Some("vip"));
/// assert_eq!(member.until_date, None, "no subscription, no expiry");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ChatMemberMember {
    /// The member.
    pub user: User,
    /// Tag of the member, when the chat uses tags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// Unix time the member's subscription expires.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until_date: Option<i64>,
    /// Fields this crate does not model, forwarded unchanged.
    #[serde(flatten, default, skip_serializing_if = "Map::is_empty")]
    pub extra: Map<String, Value>,
}

/// A restricted member — Telegram's `ChatMemberRestricted`,
/// `status: "restricted"`.
///
/// `is_member` is the bit that decides whether the user is in the chat at all,
/// so [`ChatMember::is_present`] reads it instead of trusting the status alone.
/// `until_date` is a Unix time, and `0` means the restrictions never expire.
///
/// ```
/// use telegram_api_client::ChatMemberRestricted;
///
/// let restricted: ChatMemberRestricted = serde_json::from_str(
///     r#"{"user":{"id":1},"is_member":true,"can_send_messages":false,"until_date":0}"#,
/// )
/// .unwrap();
/// assert!(restricted.is_member);
/// assert!(!restricted.can_send_messages);
/// assert_eq!(restricted.until_date, 0, "0 is forever");
/// ```
#[allow(clippy::struct_excessive_bools)] // a permission set is booleans, by definition
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ChatMemberRestricted {
    /// The restricted user.
    pub user: User,
    /// Tag of the member, when the chat uses tags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// Whether the user is a member of the chat right now.
    #[serde(default)]
    pub is_member: bool,
    /// Send text messages and other plain content.
    #[serde(default)]
    pub can_send_messages: bool,
    /// Send audios.
    #[serde(default)]
    pub can_send_audios: bool,
    /// Send documents.
    #[serde(default)]
    pub can_send_documents: bool,
    /// Send photos.
    #[serde(default)]
    pub can_send_photos: bool,
    /// Send videos.
    #[serde(default)]
    pub can_send_videos: bool,
    /// Send video notes.
    #[serde(default)]
    pub can_send_video_notes: bool,
    /// Send voice notes.
    #[serde(default)]
    pub can_send_voice_notes: bool,
    /// Send polls and checklists.
    #[serde(default)]
    pub can_send_polls: bool,
    /// Send animations, games, stickers and use inline bots.
    #[serde(default)]
    pub can_send_other_messages: bool,
    /// Add web page previews to messages.
    #[serde(default)]
    pub can_add_web_page_previews: bool,
    /// React to messages.
    #[serde(default)]
    pub can_react_to_messages: bool,
    /// Edit their own tag.
    #[serde(default)]
    pub can_edit_tag: bool,
    /// Change the chat title, photo and other settings.
    #[serde(default)]
    pub can_change_info: bool,
    /// Invite new users.
    #[serde(default)]
    pub can_invite_users: bool,
    /// Pin messages.
    #[serde(default)]
    pub can_pin_messages: bool,
    /// Create forum topics.
    #[serde(default)]
    pub can_manage_topics: bool,
    /// Unix time the restrictions are lifted; `0` means never.
    #[serde(default)]
    pub until_date: i64,
    /// Fields this crate does not model, forwarded unchanged.
    #[serde(flatten, default, skip_serializing_if = "Map::is_empty")]
    pub extra: Map<String, Value>,
}

/// A member who left — Telegram's `ChatMemberLeft`, `status: "left"`.
///
/// ```
/// use telegram_api_client::ChatMemberLeft;
///
/// let left: ChatMemberLeft = serde_json::from_str(r#"{"user":{"id":1}}"#).unwrap();
/// assert_eq!(left.user.id, 1);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ChatMemberLeft {
    /// The user, who may join again on their own.
    pub user: User,
    /// Fields this crate does not model, forwarded unchanged.
    #[serde(flatten, default, skip_serializing_if = "Map::is_empty")]
    pub extra: Map<String, Value>,
}

/// A banned member — Telegram's `ChatMemberBanned`, `status: "kicked"`.
///
/// ```
/// use telegram_api_client::ChatMemberBanned;
///
/// let banned: ChatMemberBanned =
///     serde_json::from_str(r#"{"user":{"id":1},"until_date":0}"#).unwrap();
/// assert_eq!(banned.until_date, 0, "0 means banned forever");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ChatMemberBanned {
    /// The banned user.
    pub user: User,
    /// Unix time the ban is lifted; `0` means never.
    #[serde(default)]
    pub until_date: i64,
    /// Fields this crate does not model, forwarded unchanged.
    #[serde(flatten, default, skip_serializing_if = "Map::is_empty")]
    pub extra: Map<String, Value>,
}

/// One member of a chat, as returned by `getChatMember` and carried by
/// [`ChatMemberUpdated`](crate::ChatMemberUpdated).
///
/// The Bot API's `ChatMember` is a union of six shapes that share only `status`
/// and `user`. The crate models it as a status-tagged enum with one variant per
/// shape, so the data that comes with a status stays reachable: match on the
/// variant to read an administrator's `can_*` right, a restricted member's
/// `is_member` bit or a ban's `until_date` — or use the accessors below, which
/// answer the same questions without a match.
///
/// Decoding keeps the crate's contract. `status` and `user` are mandatory: a
/// payload without either is a decode error, never a default member (that is
/// what #26 was about). A `status` this crate does not model yet is carried as
/// [`ChatMember::Unknown`] *with* its user and every other member, and a field no
/// variant models lands in that shape's `extra` map, so nothing the server said
/// is dropped on the way in or on the way out. `status` is written first, then
/// the shape's fields in wire order, then `extra`.
///
/// ```
/// use telegram_api_client::{ChatMember, ChatMemberStatus};
///
/// let member: ChatMember = serde_json::from_str(
///     r#"{"status":"administrator","user":{"id":42,"first_name":"Flo"},
///         "can_pin_messages":true,"custom_title":"Ops"}"#,
/// )
/// .unwrap();
///
/// // Status-specific data survives the decode.
/// let ChatMember::Administrator(admin) = &member else {
///     panic!("expected the administrator shape");
/// };
/// assert_eq!(admin.can_pin_messages, Some(true));
/// assert_eq!(admin.custom_title.as_deref(), Some("Ops"));
/// assert_eq!(member.status(), ChatMemberStatus::Administrator);
/// assert_eq!(member.custom_title(), Some("Ops"));
///
/// // An unmodelled status keeps its user and its unmodelled fields.
/// let future: ChatMember = serde_json::from_str(
///     r#"{"status":"ghost","user":{"id":7},"wears_a_sheet":true}"#,
/// )
/// .unwrap();
/// assert_eq!(future.status(), ChatMemberStatus::Unknown("ghost".to_string()));
/// assert_eq!(future.user().id, 7);
/// assert_eq!(serde_json::to_value(&future).unwrap()["wears_a_sheet"], true);
///
/// // Without a status or a user there is no member to report.
/// assert!(serde_json::from_str::<ChatMember>(r#"{"status":"member"}"#).is_err());
/// assert!(serde_json::from_str::<ChatMember>(r#"{"user":{"id":1}}"#).is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ChatMember {
    /// `status: "creator"` — the chat owner.
    Creator(ChatMemberOwner),
    /// `status: "administrator"` — a member with extra rights.
    Administrator(ChatMemberAdministrator),
    /// `status: "member"` — no extra rights, no restrictions.
    Member(ChatMemberMember),
    /// `status: "restricted"` — a member behind a permission set.
    Restricted(ChatMemberRestricted),
    /// `status: "left"` — not in the chat, may come back.
    Left(ChatMemberLeft),
    /// `status: "kicked"` — banned, cannot come back before `until_date`.
    Banned(ChatMemberBanned),
    /// A status this crate does not model yet — carried with its payload.
    ///
    /// Decoding fills this in for a `status` word this crate has no variant for,
    /// and [`ChatMember::status`] returns it verbatim. A hand-built value should
    /// keep the same meaning: `status` names a shape this crate does not model,
    /// and `extra` holds that shape's other fields. `status` and `user` are
    /// written from the fields below when serialising, so a copy inside `extra`
    /// is ignored rather than repeated.
    Unknown {
        /// The status word, exactly as Telegram sent it.
        status: ChatMemberStatus,
        /// The user behind the membership. Required, like everywhere else.
        user: User,
        /// Every other field of the object, unchanged.
        extra: Map<String, Value>,
    },
}

impl Default for ChatMember {
    /// The empty member: no status this crate knows and the default user, which
    /// is what a [`ChatMemberUpdated`](crate::ChatMemberUpdated) carries when the
    /// payload named no member at all. [`ChatMember::is_present`] is `false`.
    fn default() -> Self {
        Self::Unknown {
            status: ChatMemberStatus::default(),
            user: User::default(),
            extra: Map::new(),
        }
    }
}

impl ChatMember {
    /// A member with `status` and `user`, without the status-specific data.
    ///
    /// ```
    /// use telegram_api_client::{ChatMember, ChatMemberStatus, User};
    ///
    /// let member = ChatMember::new(ChatMemberStatus::Kicked, User::default());
    /// assert_eq!(member.status(), ChatMemberStatus::Kicked);
    /// assert!(!member.is_present());
    /// ```
    #[must_use]
    pub fn new(status: ChatMemberStatus, user: User) -> Self {
        match status {
            ChatMemberStatus::Creator => Self::Creator(ChatMemberOwner {
                user,
                ..ChatMemberOwner::default()
            }),
            ChatMemberStatus::Administrator => Self::Administrator(ChatMemberAdministrator {
                user,
                ..ChatMemberAdministrator::default()
            }),
            ChatMemberStatus::Member => Self::Member(ChatMemberMember {
                user,
                ..ChatMemberMember::default()
            }),
            ChatMemberStatus::Restricted => Self::Restricted(ChatMemberRestricted {
                user,
                ..ChatMemberRestricted::default()
            }),
            ChatMemberStatus::Left => Self::Left(ChatMemberLeft {
                user,
                ..ChatMemberLeft::default()
            }),
            ChatMemberStatus::Kicked => Self::Banned(ChatMemberBanned {
                user,
                ..ChatMemberBanned::default()
            }),
            status @ ChatMemberStatus::Unknown(_) => Self::Unknown {
                status,
                user,
                extra: Map::new(),
            },
        }
    }

    /// The member's status, reconstructed from the variant.
    ///
    /// ```
    /// use telegram_api_client::{ChatMember, ChatMemberStatus};
    ///
    /// let member: ChatMember = serde_json::from_str(r#"{"status":"kicked","user":{"id":1}}"#).unwrap();
    /// assert_eq!(member.status(), ChatMemberStatus::Kicked);
    /// ```
    #[must_use]
    pub fn status(&self) -> ChatMemberStatus {
        match self {
            Self::Creator(_) => ChatMemberStatus::Creator,
            Self::Administrator(_) => ChatMemberStatus::Administrator,
            Self::Member(_) => ChatMemberStatus::Member,
            Self::Restricted(_) => ChatMemberStatus::Restricted,
            Self::Left(_) => ChatMemberStatus::Left,
            Self::Banned(_) => ChatMemberStatus::Kicked,
            Self::Unknown { status, .. } => status.clone(),
        }
    }

    /// The user behind the membership, whatever the status is.
    ///
    /// ```
    /// use telegram_api_client::ChatMember;
    ///
    /// let member: ChatMember =
    ///     serde_json::from_str(r#"{"status":"left","user":{"id":42}}"#).unwrap();
    /// assert_eq!(member.user().id, 42);
    /// ```
    #[must_use]
    pub fn user(&self) -> &User {
        match self {
            Self::Creator(member) => &member.user,
            Self::Administrator(member) => &member.user,
            Self::Member(member) => &member.user,
            Self::Restricted(member) => &member.user,
            Self::Left(member) => &member.user,
            Self::Banned(member) => &member.user,
            Self::Unknown { user, .. } => user,
        }
    }

    /// Whether the user hides behind the chat identity, when the shape says so.
    ///
    /// `None` means "this status carries no such flag" or "the payload did not
    /// send it" — never an invented `false`.
    ///
    /// ```
    /// use telegram_api_client::{ChatMember, ChatMemberStatus};
    ///
    /// let owner: ChatMember = serde_json::from_str(
    ///     r#"{"status":"creator","user":{"id":1},"is_anonymous":true}"#,
    /// )
    /// .unwrap();
    /// assert_eq!(owner.is_anonymous(), Some(true));
    /// assert_eq!(ChatMember::new(ChatMemberStatus::Left, Default::default()).is_anonymous(), None);
    /// ```
    #[must_use]
    pub fn is_anonymous(&self) -> Option<bool> {
        match self {
            Self::Creator(member) => member.is_anonymous,
            Self::Administrator(member) => member.is_anonymous,
            Self::Unknown { extra, .. } => extra.get("is_anonymous").and_then(Value::as_bool),
            _ => None,
        }
    }

    /// The custom title Telegram reported for this member, if any.
    ///
    /// Only the owner and administrator shapes carry one.
    ///
    /// ```
    /// use telegram_api_client::ChatMember;
    ///
    /// let admin: ChatMember = serde_json::from_str(
    ///     r#"{"status":"administrator","user":{"id":1},"custom_title":"Ops"}"#,
    /// )
    /// .unwrap();
    /// assert_eq!(admin.custom_title(), Some("Ops"));
    /// ```
    #[must_use]
    pub fn custom_title(&self) -> Option<&str> {
        match self {
            Self::Creator(member) => member.custom_title.as_deref(),
            Self::Administrator(member) => member.custom_title.as_deref(),
            Self::Unknown { extra, .. } => extra.get("custom_title").and_then(Value::as_str),
            _ => None,
        }
    }

    /// The Unix time the membership or the restriction ends, when the shape has
    /// one.
    ///
    /// `0` is the API's "forever", exactly as sent: it is not filtered out,
    /// because "expires at 0" and "sends no date" are different statements.
    ///
    /// ```
    /// use telegram_api_client::ChatMember;
    ///
    /// let banned: ChatMember =
    ///     serde_json::from_str(r#"{"status":"kicked","user":{"id":1},"until_date":0}"#).unwrap();
    /// assert_eq!(banned.until_date(), Some(0));
    ///
    /// let left: ChatMember = serde_json::from_str(r#"{"status":"left","user":{"id":1}}"#).unwrap();
    /// assert_eq!(left.until_date(), None);
    /// ```
    #[must_use]
    pub fn until_date(&self) -> Option<i64> {
        match self {
            Self::Member(member) => member.until_date,
            Self::Restricted(member) => Some(member.until_date),
            Self::Banned(member) => Some(member.until_date),
            Self::Unknown { extra, .. } => extra.get("until_date").and_then(Value::as_i64),
            _ => None,
        }
    }

    /// The `is_member` bit, which only the restricted shape carries.
    ///
    /// ```
    /// use telegram_api_client::ChatMember;
    ///
    /// let restricted: ChatMember = serde_json::from_str(
    ///     r#"{"status":"restricted","user":{"id":1},"is_member":true}"#,
    /// )
    /// .unwrap();
    /// assert_eq!(restricted.is_member(), Some(true));
    /// assert!(restricted.is_present());
    /// ```
    #[must_use]
    pub fn is_member(&self) -> Option<bool> {
        match self {
            Self::Restricted(member) => Some(member.is_member),
            Self::Unknown { extra, .. } => extra.get("is_member").and_then(Value::as_bool),
            _ => None,
        }
    }

    /// Whether the user is currently in the chat.
    ///
    /// A restricted member counts as present only while `is_member` is set, and
    /// an unmodelled status is never presence — the same rule
    /// [`ChatMemberUpdated::joined`](crate::ChatMemberUpdated::joined) builds on.
    ///
    /// ```
    /// use telegram_api_client::{ChatMember, ChatMemberStatus};
    ///
    /// let mut member = ChatMember::default();
    /// assert!(!member.is_present());
    /// member = ChatMember::new(ChatMemberStatus::Member, Default::default());
    /// assert!(member.is_present());
    /// ```
    #[must_use]
    pub fn is_present(&self) -> bool {
        match self {
            Self::Creator(_) | Self::Administrator(_) | Self::Member(_) => true,
            Self::Restricted(member) => member.is_member,
            // Left, banned, and a status this crate cannot classify: none of
            // them is a member of the chat.
            Self::Left(_) | Self::Banned(_) | Self::Unknown { .. } => false,
        }
    }
}

impl Serialize for ChatMember {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::Error as _;

        let (status, payload) = match self {
            Self::Creator(member) => ("creator", serde_json::to_value(member)),
            Self::Administrator(member) => ("administrator", serde_json::to_value(member)),
            Self::Member(member) => ("member", serde_json::to_value(member)),
            Self::Restricted(member) => ("restricted", serde_json::to_value(member)),
            Self::Left(member) => ("left", serde_json::to_value(member)),
            Self::Banned(member) => ("kicked", serde_json::to_value(member)),
            Self::Unknown {
                status,
                user,
                extra,
            } => {
                let mut out = Map::new();
                out.insert(
                    "status".to_string(),
                    Value::String(status.as_str().to_string()),
                );
                out.insert(
                    "user".to_string(),
                    serde_json::to_value(user).map_err(S::Error::custom)?,
                );
                for (key, value) in extra {
                    // `status` and `user` are structural, not payload: the
                    // variant owns them, so a hand-built map cannot disagree
                    // with it.
                    if key != "status" && key != "user" {
                        out.insert(key.clone(), value.clone());
                    }
                }
                return Value::Object(out).serialize(serializer);
            }
        };

        let Value::Object(mut fields) = payload.map_err(S::Error::custom)? else {
            return Err(S::Error::custom(
                "a chat member must serialize as an object",
            ));
        };
        // The variant decides the status word; nothing in the payload can
        // override it.
        let _ = fields.remove("status");

        let mut out = Map::new();
        out.insert("status".to_string(), Value::String(status.to_string()));
        out.extend(fields);
        Value::Object(out).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ChatMember {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        use serde::de::Error as _;

        let value = Value::deserialize(deserializer)?;
        let Value::Object(mut fields) = value else {
            return Err(D::Error::custom("a chat member must be a JSON object"));
        };
        let status = match fields.remove("status") {
            None => return Err(D::Error::missing_field("status")),
            Some(Value::String(status)) => status,
            Some(_) => return Err(D::Error::custom("a chat member `status` must be a string")),
        };

        // The status word selects the shape; the shape itself is decoded by
        // serde from the remaining fields, which keeps one authority for the
        // field list of every status.
        match status.as_str() {
            "creator" => ChatMemberOwner::deserialize(Value::Object(fields))
                .map(Self::Creator)
                .map_err(D::Error::custom),
            "administrator" => ChatMemberAdministrator::deserialize(Value::Object(fields))
                .map(Self::Administrator)
                .map_err(D::Error::custom),
            "member" => ChatMemberMember::deserialize(Value::Object(fields))
                .map(Self::Member)
                .map_err(D::Error::custom),
            "restricted" => ChatMemberRestricted::deserialize(Value::Object(fields))
                .map(Self::Restricted)
                .map_err(D::Error::custom),
            "left" => ChatMemberLeft::deserialize(Value::Object(fields))
                .map(Self::Left)
                .map_err(D::Error::custom),
            "kicked" => ChatMemberBanned::deserialize(Value::Object(fields))
                .map(Self::Banned)
                .map_err(D::Error::custom),
            _ => {
                let user = match fields.remove("user") {
                    None => return Err(D::Error::missing_field("user")),
                    Some(user) => User::deserialize(user).map_err(D::Error::custom)?,
                };
                Ok(Self::Unknown {
                    status: ChatMemberStatus::Unknown(status),
                    user,
                    extra: fields,
                })
            }
        }
    }
}
