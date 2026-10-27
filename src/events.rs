//! Payload types for the updates that carry more than a message.
//!
//! The `Update` object gained a field per feature since 2021, and this module
//! models every one of them for the pinned Bot API version
//! ([`BOT_API_VERSION`], 10.3). Each struct names the wire field it binds and
//! ignores fields it does not know, so a newer Bot API cannot break parsing.
//!
//! The wire shapes are pinned as fixtures in `docs/contracts.md`, taken from
//! <https://core.telegram.org/bots/api>.

use serde::{Deserialize, Serialize};

use crate::chat::ChatMember;
use crate::types::MessageEntity;
use crate::update::{Chat, User};

/// The Bot API version whose update surface this crate implements.
///
/// Every field of `Update`, `Message` and the payload types in this module is
/// checked against this revision; `docs/contracts.md` records the source URL and
/// the field table it was read from.
///
/// ```
/// use telegram_api_client::BOT_API_VERSION;
///
/// assert_eq!(BOT_API_VERSION, "10.3");
/// ```
pub const BOT_API_VERSION: &str = "10.3";

/// Serde default for the `rights` booleans Telegram omits when they are on.
fn default_true() -> bool {
    true
}

/// One emoji, custom emoji or paid reaction on a message.
///
/// ```
/// use telegram_api_client::ReactionType;
///
/// let reaction: ReactionType = serde_json::from_str(r#"{"type":"emoji","emoji":"👍"}"#).unwrap();
/// assert_eq!(reaction.glyph(), Some("👍"));
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ReactionType {
    /// A plain unicode emoji.
    Emoji {
        /// The emoji itself, e.g. `👍`.
        emoji: String,
    },
    /// One of Telegram's custom (premium) emoji.
    CustomEmoji {
        /// Identifier of the custom emoji; resolve it with `getCustomEmojiStickers`.
        #[serde(alias = "customEmojiId")]
        custom_emoji_id: String,
    },
    /// A paid reaction, bought with Telegram Stars.
    Paid,
    /// A reaction kind this crate does not know yet.
    #[default]
    #[serde(other)]
    Unknown,
}

impl ReactionType {
    /// The emoji a user picked, when this is a plain emoji reaction.
    ///
    /// ```
    /// use telegram_api_client::ReactionType;
    ///
    /// let reaction: ReactionType = serde_json::from_str(r#"{"type":"paid"}"#).unwrap();
    /// assert_eq!(reaction.glyph(), None);
    /// ```
    #[must_use]
    pub fn glyph(&self) -> Option<&str> {
        match self {
            Self::Emoji { emoji } => Some(emoji.as_str()),
            _ => None,
        }
    }

    /// The custom emoji id, when this is a custom emoji reaction.
    #[must_use]
    pub fn custom_emoji_id(&self) -> Option<&str> {
        match self {
            Self::CustomEmoji { custom_emoji_id } => Some(custom_emoji_id.as_str()),
            _ => None,
        }
    }
}

/// A reaction and how many users set it.
///
/// ```
/// use telegram_api_client::ReactionCount;
///
/// let count: ReactionCount =
///     serde_json::from_str(r#"{"type":{"type":"emoji","emoji":"🔥"},"total_count":7}"#).unwrap();
/// assert_eq!(count.total_count, 7);
/// assert_eq!(count.r#type.glyph(), Some("🔥"));
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReactionCount {
    /// The reaction that was counted.
    #[serde(rename = "type", alias = "reactionType")]
    pub r#type: ReactionType,
    /// How many users set this reaction.
    #[serde(default, alias = "totalCount")]
    pub total_count: i64,
}

/// A reaction on a message was changed by a user.
///
/// ```
/// use telegram_api_client::MessageReactionUpdated;
///
/// let update: MessageReactionUpdated = serde_json::from_str(
///     r#"{"chat":{"id":-1},"message_id":5,"date":1700000000,
///         "old_reaction":[{"type":"emoji","emoji":"👍"}],
///         "new_reaction":[{"type":"emoji","emoji":"👎"}]}"#,
/// ).unwrap();
/// assert_eq!(update.old_reaction.len(), 1);
/// assert_eq!(update.new_reaction[0].glyph(), Some("👎"));
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MessageReactionUpdated {
    /// Chat holding the message.
    #[serde(default)]
    pub chat: Chat,
    /// Message that was reacted to.
    #[serde(default, alias = "messageId")]
    pub message_id: i64,
    /// User who changed the reaction, absent for anonymous reactors.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
    /// Chat on whose behalf an anonymous reaction was changed.
    #[serde(default, alias = "actorChat", skip_serializing_if = "Option::is_none")]
    pub actor_chat: Option<Chat>,
    /// Unix time of the change.
    #[serde(default)]
    pub date: i64,
    /// Reactions that were set before the change.
    #[serde(default, alias = "oldReaction")]
    pub old_reaction: Vec<ReactionType>,
    /// Reactions that are set after the change; empty means removed.
    #[serde(default, alias = "newReaction")]
    pub new_reaction: Vec<ReactionType>,
}

impl MessageReactionUpdated {
    /// Whether the user removed every reaction from the message.
    ///
    /// ```
    /// use telegram_api_client::MessageReactionUpdated;
    ///
    /// let update: MessageReactionUpdated =
    ///     serde_json::from_str(r#"{"message_id":1,"new_reaction":[]}"#).unwrap();
    /// assert!(update.removed_all());
    /// ```
    #[must_use]
    pub fn removed_all(&self) -> bool {
        self.new_reaction.is_empty()
    }
}

/// Reaction counts on a message with anonymous reactions.
///
/// ```
/// use telegram_api_client::MessageReactionCountUpdated;
///
/// let update: MessageReactionCountUpdated = serde_json::from_str(
///     r#"{"chat":{"id":-1},"message_id":9,"date":1700000000,
///         "reactions":[{"type":{"type":"emoji","emoji":"🎉"},"total_count":3}]}"#,
/// ).unwrap();
/// assert_eq!(update.total(), 3);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MessageReactionCountUpdated {
    /// Chat holding the message.
    #[serde(default)]
    pub chat: Chat,
    /// Message whose reactions changed.
    #[serde(default, alias = "messageId")]
    pub message_id: i64,
    /// Unix time of the change.
    #[serde(default)]
    pub date: i64,
    /// All reactions currently on the message.
    #[serde(default)]
    pub reactions: Vec<ReactionCount>,
}

impl MessageReactionCountUpdated {
    /// Sum of all reaction counts.
    ///
    /// ```
    /// use telegram_api_client::MessageReactionCountUpdated;
    ///
    /// let update = MessageReactionCountUpdated::default();
    /// assert_eq!(update.total(), 0);
    /// ```
    #[must_use]
    pub fn total(&self) -> i64 {
        self.reactions.iter().map(|count| count.total_count).sum()
    }
}

/// What a business bot connected to a business account may do.
///
/// Every flag is optional on the wire and defaults to `true`, which is how
/// Telegram encodes "this right was granted".
///
/// ```
/// use telegram_api_client::BusinessBotRights;
///
/// let rights: BusinessBotRights = serde_json::from_str(r#"{"can_reply":false}"#).unwrap();
/// assert!(!rights.can_reply);
/// assert!(rights.can_read_messages);
/// ```
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
// The wire object is a flat set of optional flags, each defaulting to `true`;
// nesting them would misrepresent what Telegram sends.
#[allow(clippy::struct_excessive_bools)]
pub struct BusinessBotRights {
    /// Reply to messages.
    #[serde(default = "default_true")]
    pub can_reply: bool,
    /// Read incoming messages.
    #[serde(default = "default_true")]
    pub can_read_messages: bool,
    /// Delete messages the bot sent.
    #[serde(default = "default_true")]
    pub can_delete_sent_messages: bool,
    /// Delete any message in the chat.
    #[serde(default = "default_true")]
    pub can_delete_all_messages: bool,
    /// Edit the business account's name.
    #[serde(default = "default_true")]
    pub can_edit_name: bool,
    /// Edit the business account's bio.
    #[serde(default = "default_true")]
    pub can_edit_bio: bool,
    /// Change the business account's profile photo.
    #[serde(default = "default_true")]
    pub can_edit_profile_photo: bool,
    /// Change the business account's username.
    #[serde(default = "default_true")]
    pub can_edit_username: bool,
    /// Change the account's gift settings.
    #[serde(default = "default_true")]
    pub can_change_gift_settings: bool,
    /// See the account's gifts and Stars.
    #[serde(default = "default_true")]
    pub can_view_gifts_and_stars: bool,
    /// Convert gifts into Stars.
    #[serde(default = "default_true")]
    pub can_convert_gifts_to_stars: bool,
    /// Transfer and upgrade gifts.
    #[serde(default = "default_true")]
    pub can_transfer_and_upgrade_gifts: bool,
    /// Transfer Stars.
    #[serde(default = "default_true")]
    pub can_transfer_stars: bool,
    /// Manage the account's stories.
    #[serde(default = "default_true")]
    pub can_manage_stories: bool,
}

impl Default for BusinessBotRights {
    fn default() -> Self {
        Self {
            can_reply: true,
            can_read_messages: true,
            can_delete_sent_messages: true,
            can_delete_all_messages: true,
            can_edit_name: true,
            can_edit_bio: true,
            can_edit_profile_photo: true,
            can_edit_username: true,
            can_change_gift_settings: true,
            can_view_gifts_and_stars: true,
            can_convert_gifts_to_stars: true,
            can_transfer_and_upgrade_gifts: true,
            can_transfer_stars: true,
            can_manage_stories: true,
        }
    }
}

/// The bot was connected to, disconnected from or re-configured for a business
/// account.
///
/// ```
/// use telegram_api_client::BusinessConnection;
///
/// let connection: BusinessConnection = serde_json::from_str(
///     r#"{"id":"bc-1","user":{"id":7},"user_chat_id":7,"date":1700000000,"is_enabled":true}"#,
/// ).unwrap();
/// assert!(connection.is_enabled);
/// assert_eq!(connection.user_chat_id, 7);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct BusinessConnection {
    /// Identifier to use as `business_connection_id`.
    #[serde(default)]
    pub id: String,
    /// The business account's user.
    #[serde(default)]
    pub user: User,
    /// Private chat with that user.
    #[serde(default, alias = "userChatId")]
    pub user_chat_id: i64,
    /// Unix time the connection was established.
    #[serde(default)]
    pub date: i64,
    /// Rights granted to the bot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rights: Option<BusinessBotRights>,
    /// Whether the connection is currently active.
    #[serde(default, alias = "isEnabled")]
    pub is_enabled: bool,
}

/// Messages were deleted from a connected business account.
///
/// ```
/// use telegram_api_client::BusinessMessagesDeleted;
///
/// let deleted: BusinessMessagesDeleted = serde_json::from_str(
///     r#"{"business_connection_id":"bc-1","chat":{"id":7},"message_ids":[3,4]}"#,
/// ).unwrap();
/// assert_eq!(deleted.message_ids, vec![3, 4]);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct BusinessMessagesDeleted {
    /// Business connection the messages belonged to.
    #[serde(default, alias = "businessConnectionId")]
    pub business_connection_id: String,
    /// Chat in the business account; the bot may not be able to read it.
    #[serde(default)]
    pub chat: Chat,
    /// Ids of the deleted messages.
    #[serde(default, alias = "messageIds")]
    pub message_ids: Vec<i64>,
}

/// An invite link of a chat.
///
/// ```
/// use telegram_api_client::ChatInviteLink;
///
/// let link: ChatInviteLink = serde_json::from_str(
///     r#"{"invite_link":"https://t.me/+abc","creator":{"id":1},
///         "creates_join_request":false,"is_primary":true,"is_revoked":false}"#,
/// ).unwrap();
/// assert!(link.is_primary);
/// assert_eq!(link.url(), "https://t.me/+abc");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ChatInviteLink {
    /// The link itself.
    #[serde(default, alias = "inviteLink")]
    pub invite_link: String,
    /// User who created the link.
    #[serde(default)]
    pub creator: User,
    /// Whether joining needs admin approval.
    #[serde(default, alias = "createsJoinRequest")]
    pub creates_join_request: bool,
    /// Whether this is the chat's primary link.
    #[serde(default, alias = "isPrimary")]
    pub is_primary: bool,
    /// Whether the link was revoked.
    #[serde(default, alias = "isRevoked")]
    pub is_revoked: bool,
    /// Name shown when a user joins through the link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Unix time the link expires.
    #[serde(default, alias = "expireDate", skip_serializing_if = "Option::is_none")]
    pub expire_date: Option<i64>,
    /// Maximum number of members that may join through the link.
    #[serde(
        default,
        alias = "memberLimit",
        skip_serializing_if = "Option::is_none"
    )]
    pub member_limit: Option<i64>,
    /// Pending join requests for this link.
    #[serde(
        default,
        alias = "pendingJoinRequestCount",
        skip_serializing_if = "Option::is_none"
    )]
    pub pending_join_request_count: Option<i64>,
    /// Subscription period in seconds, for subscription links.
    #[serde(
        default,
        alias = "subscriptionPeriod",
        skip_serializing_if = "Option::is_none"
    )]
    pub subscription_period: Option<i64>,
    /// Stars a subscription link costs.
    #[serde(
        default,
        alias = "subscriptionPrice",
        skip_serializing_if = "Option::is_none"
    )]
    pub subscription_price: Option<i64>,
}

impl ChatInviteLink {
    /// The invite URL.
    ///
    /// ```
    /// use telegram_api_client::ChatInviteLink;
    ///
    /// assert_eq!(ChatInviteLink::default().url(), "");
    /// ```
    #[must_use]
    pub fn url(&self) -> &str {
        self.invite_link.as_str()
    }
}

/// A chat member's status changed — the bot's own (`my_chat_member`) or
/// another user's (`chat_member`).
///
/// ```
/// use telegram_api_client::{ChatMemberStatus, ChatMemberUpdated};
///
/// let updated: ChatMemberUpdated = serde_json::from_str(
///     r#"{"chat":{"id":-1},"from":{"id":2},"date":1700000000,
///         "old_chat_member":{"status":"left","user":{"id":2}},
///         "new_chat_member":{"status":"member","user":{"id":2}}}"#,
/// ).unwrap();
/// assert_eq!(updated.old_chat_member.status, ChatMemberStatus::Left);
/// assert!(updated.joined());
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ChatMemberUpdated {
    /// Chat the membership belongs to.
    #[serde(default)]
    pub chat: Chat,
    /// Who performed the change.
    #[serde(default)]
    pub from: Option<User>,
    /// Unix time of the change.
    #[serde(default)]
    pub date: i64,
    /// Status before the change.
    #[serde(default, alias = "oldChatMember")]
    pub old_chat_member: ChatMember,
    /// Status after the change.
    #[serde(default, alias = "newChatMember")]
    pub new_chat_member: ChatMember,
    /// Invite link used to join, for link joins.
    #[serde(default, alias = "inviteLink", skip_serializing_if = "Option::is_none")]
    pub invite_link: Option<ChatInviteLink>,
    /// Joined after an approved direct join request.
    #[serde(
        default,
        alias = "viaJoinRequest",
        skip_serializing_if = "Option::is_none"
    )]
    pub via_join_request: Option<bool>,
    /// Joined through a chat folder invite link.
    #[serde(
        default,
        alias = "viaChatFolderInviteLink",
        skip_serializing_if = "Option::is_none"
    )]
    pub via_chat_folder_invite_link: Option<bool>,
}

impl ChatMemberUpdated {
    /// Whether the member is present after the change but was not before.
    ///
    /// ```
    /// use telegram_api_client::{ChatMember, ChatMemberStatus, ChatMemberUpdated};
    ///
    /// let mut updated = ChatMemberUpdated::default();
    /// updated.old_chat_member.status = ChatMemberStatus::Left;
    /// updated.new_chat_member.status = ChatMemberStatus::Member;
    /// assert!(updated.joined());
    ///
    /// updated.new_chat_member = ChatMember::default();
    /// assert!(!updated.joined());
    /// ```
    #[must_use]
    pub fn joined(&self) -> bool {
        !self.old_chat_member.is_present() && self.new_chat_member.is_present()
    }

    /// Whether the member is gone after the change but was present before.
    #[must_use]
    pub fn left(&self) -> bool {
        self.old_chat_member.is_present() && !self.new_chat_member.is_present()
    }
}

/// A request to join a chat that an admin has to approve.
///
/// ```
/// use telegram_api_client::ChatJoinRequest;
///
/// let request: ChatJoinRequest = serde_json::from_str(
///     r#"{"chat":{"id":-1},"from":{"id":3},"user_chat_id":3,"date":1700000000,"bio":"hi"}"#,
/// ).unwrap();
/// assert_eq!(request.user_chat_id, 3);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ChatJoinRequest {
    /// Chat the request targets.
    #[serde(default)]
    pub chat: Chat,
    /// User asking to join.
    #[serde(default)]
    pub from: Option<User>,
    /// Private chat with that user, usable for five minutes.
    #[serde(default, alias = "userChatId")]
    pub user_chat_id: i64,
    /// Unix time of the request.
    #[serde(default)]
    pub date: i64,
    /// The user's bio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bio: Option<String>,
    /// Invite link the request came through.
    #[serde(default, alias = "inviteLink", skip_serializing_if = "Option::is_none")]
    pub invite_link: Option<ChatInviteLink>,
    /// Id of the join request query, for bots that process requests only.
    #[serde(default, alias = "queryId", skip_serializing_if = "Option::is_none")]
    pub query_id: Option<String>,
}

/// Where a chat boost came from.
///
/// ```
/// use telegram_api_client::ChatBoostSource;
///
/// let source: ChatBoostSource =
///     serde_json::from_str(r#"{"source":"premium","user":{"id":1}}"#).unwrap();
/// assert_eq!(source.source_kind(), "premium");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "source", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ChatBoostSource {
    /// A premium subscription boosted the chat.
    Premium {
        /// The premium user.
        #[serde(default)]
        user: Option<User>,
    },
    /// A gifted premium subscription boosted the chat.
    GiftCode {
        /// The user who received the gift.
        #[serde(default)]
        user: Option<User>,
    },
    /// A giveaway prize boosted the chat.
    Giveaway {
        /// Message with the giveaway.
        #[serde(default, alias = "giveawayMessageId")]
        giveaway_message_id: i64,
        /// The user who won, if they are not anonymous.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        user: Option<User>,
        /// Stars that came with the prize.
        #[serde(
            default,
            alias = "prizeStarCount",
            skip_serializing_if = "Option::is_none"
        )]
        prize_star_count: Option<i64>,
        /// Whether the prize is still unclaimed.
        #[serde(
            default,
            alias = "isUnclaimed",
            skip_serializing_if = "Option::is_none"
        )]
        is_unclaimed: Option<bool>,
    },
    /// A source this crate does not know yet.
    #[default]
    #[serde(other)]
    Unknown,
}

impl ChatBoostSource {
    /// The wire value of the source, for logging and metrics.
    ///
    /// ```
    /// use telegram_api_client::ChatBoostSource;
    ///
    /// assert_eq!(ChatBoostSource::default().source_kind(), "unknown");
    /// ```
    #[must_use]
    pub fn source_kind(&self) -> &'static str {
        match self {
            Self::Premium { .. } => "premium",
            Self::GiftCode { .. } => "gift_code",
            Self::Giveaway { .. } => "giveaway",
            Self::Unknown => "unknown",
        }
    }

    /// The user behind the boost, if the API names one.
    #[must_use]
    pub fn user(&self) -> Option<&User> {
        match self {
            Self::Premium { user } | Self::GiftCode { user } | Self::Giveaway { user, .. } => {
                user.as_ref()
            }

            Self::Unknown => None,
        }
    }
}

/// One boost of a chat.
///
/// ```
/// use telegram_api_client::ChatBoost;
///
/// let boost: ChatBoost = serde_json::from_str(
///     r#"{"boost_id":"b1","add_date":1700000000,"expiration_date":1700100000,
///         "source":{"source":"premium","user":{"id":1}}}"#,
/// ).unwrap();
/// assert_eq!(boost.boost_id, "b1");
/// assert_eq!(boost.source.source_kind(), "premium");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ChatBoost {
    /// Identifier of the boost.
    #[serde(default, alias = "boostId")]
    pub boost_id: String,
    /// Unix time the boost was added.
    #[serde(default, alias = "addDate")]
    pub add_date: i64,
    /// Unix time the boost expires.
    #[serde(default, alias = "expirationDate")]
    pub expiration_date: i64,
    /// Where the boost came from.
    #[serde(default)]
    pub source: ChatBoostSource,
}

/// A chat boost was added or changed.
///
/// ```
/// use telegram_api_client::ChatBoostUpdated;
///
/// let updated: ChatBoostUpdated = serde_json::from_str(
///     r#"{"chat":{"id":-1},"boost":{"boost_id":"b1","add_date":1,"expiration_date":2,
///         "source":{"source":"premium","user":{"id":1}}}}"#,
/// ).unwrap();
/// assert_eq!(updated.boost.boost_id, "b1");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ChatBoostUpdated {
    /// Chat that was boosted.
    #[serde(default)]
    pub chat: Chat,
    /// The boost itself.
    #[serde(default)]
    pub boost: ChatBoost,
}

/// A boost was removed from a chat.
///
/// ```
/// use telegram_api_client::ChatBoostRemoved;
///
/// let removed: ChatBoostRemoved = serde_json::from_str(
///     r#"{"chat":{"id":-1},"boost_id":"b1","remove_date":5,
///         "source":{"source":"premium","user":{"id":1}}}"#,
/// ).unwrap();
/// assert_eq!(removed.remove_date, 5);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ChatBoostRemoved {
    /// Chat that lost the boost.
    #[serde(default)]
    pub chat: Chat,
    /// Identifier of the removed boost.
    #[serde(default, alias = "boostId")]
    pub boost_id: String,
    /// Unix time the boost was removed.
    #[serde(default, alias = "removeDate")]
    pub remove_date: i64,
    /// Where the boost came from.
    #[serde(default)]
    pub source: ChatBoostSource,
}

/// A postal address a user entered for shipping.
///
/// ```
/// use telegram_api_client::ShippingAddress;
///
/// let address: ShippingAddress = serde_json::from_str(
///     r#"{"country_code":"DE","state":"SN","city":"Dresden","street_line1":"A Str. 1",
///         "street_line2":"","post_code":"01187"}"#,
/// ).unwrap();
/// assert_eq!(address.city, "Dresden");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ShippingAddress {
    /// ISO 3166-1 alpha-2 country code.
    #[serde(default, alias = "countryCode")]
    pub country_code: String,
    /// State, region or province.
    #[serde(default)]
    pub state: String,
    /// City.
    #[serde(default)]
    pub city: String,
    /// First line of the street address.
    #[serde(default, alias = "streetLine1")]
    pub street_line1: String,
    /// Second line of the street address.
    #[serde(default, alias = "streetLine2")]
    pub street_line2: String,
    /// Postal code.
    #[serde(default, alias = "postCode")]
    pub post_code: String,
}

/// An incoming shipping query for an invoice with a flexible price.
///
/// ```
/// use telegram_api_client::ShippingQuery;
///
/// let query: ShippingQuery = serde_json::from_str(
///     r#"{"id":"sq1","from":{"id":1},"invoice_payload":"order-7",
///         "shipping_address":{"country_code":"DE","city":"Dresden"}}"#,
/// ).unwrap();
/// assert_eq!(query.invoice_payload, "order-7");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ShippingQuery {
    /// Query id, needed to answer it.
    #[serde(default)]
    pub id: String,
    /// User who sent the query.
    #[serde(default)]
    pub from: Option<User>,
    /// Bot-specified invoice payload.
    #[serde(default, alias = "invoicePayload")]
    pub invoice_payload: String,
    /// Address the user entered.
    #[serde(default, alias = "shippingAddress")]
    pub shipping_address: ShippingAddress,
}

/// Order details a user entered in a payment flow.
///
/// ```
/// use telegram_api_client::OrderInfo;
///
/// let order: OrderInfo = serde_json::from_str(r#"{"name":"Flo","email":"a@b.c"}"#).unwrap();
/// assert_eq!(order.email.as_deref(), Some("a@b.c"));
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct OrderInfo {
    /// Recipient name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Phone number.
    #[serde(
        default,
        alias = "phoneNumber",
        skip_serializing_if = "Option::is_none"
    )]
    pub phone_number: Option<String>,
    /// E-mail address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Shipping address.
    #[serde(
        default,
        alias = "shippingAddress",
        skip_serializing_if = "Option::is_none"
    )]
    pub shipping_address: Option<ShippingAddress>,
}

/// An incoming pre-checkout query, the last chance to reject a payment.
///
/// ```
/// use telegram_api_client::PreCheckoutQuery;
///
/// let query: PreCheckoutQuery = serde_json::from_str(
///     r#"{"id":"pc1","from":{"id":1},"currency":"EUR","total_amount":1450,
///         "invoice_payload":"order-7"}"#,
/// ).unwrap();
/// assert_eq!(query.total_amount, 1450);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct PreCheckoutQuery {
    /// Query id, needed to answer it.
    #[serde(default)]
    pub id: String,
    /// User who is paying.
    #[serde(default)]
    pub from: Option<User>,
    /// ISO 4217 currency code, or `XTR` for Telegram Stars.
    #[serde(default)]
    pub currency: String,
    /// Total in the smallest currency unit.
    #[serde(default, alias = "totalAmount")]
    pub total_amount: i64,
    /// Bot-specified invoice payload.
    #[serde(default, alias = "invoicePayload")]
    pub invoice_payload: String,
    /// Shipping option the user picked.
    #[serde(
        default,
        alias = "shippingOptionId",
        skip_serializing_if = "Option::is_none"
    )]
    pub shipping_option_id: Option<String>,
    /// Order details the user entered.
    #[serde(default, alias = "orderInfo", skip_serializing_if = "Option::is_none")]
    pub order_info: Option<OrderInfo>,
}

/// A user bought paid media that the bot offered.
///
/// ```
/// use telegram_api_client::PaidMediaPurchased;
///
/// let purchased: PaidMediaPurchased = serde_json::from_str(
///     r#"{"from":{"id":1},"paid_media_payload":"media-7"}"#,
/// ).unwrap();
/// assert_eq!(purchased.paid_media_payload, "media-7");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct PaidMediaPurchased {
    /// User who paid.
    #[serde(default)]
    pub from: Option<User>,
    /// Bot-specified payload of the paid media.
    #[serde(default, alias = "paidMediaPayload")]
    pub paid_media_payload: String,
}

/// A location on the map.
///
/// ```
/// use telegram_api_client::Location;
///
/// let location: Location = serde_json::from_str(r#"{"latitude":51.05,"longitude":13.74}"#).unwrap();
/// assert!((location.latitude - 51.05).abs() < f64::EPSILON);
/// ```
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]
pub struct Location {
    /// Latitude in degrees.
    #[serde(default)]
    pub latitude: f64,
    /// Longitude in degrees.
    #[serde(default)]
    pub longitude: f64,
    /// Accuracy radius in metres.
    #[serde(
        default,
        alias = "horizontalAccuracy",
        skip_serializing_if = "Option::is_none"
    )]
    pub horizontal_accuracy: Option<f64>,
    /// Period in seconds the location is updated for.
    #[serde(default, alias = "livePeriod", skip_serializing_if = "Option::is_none")]
    pub live_period: Option<i64>,
    /// Direction of movement in degrees.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading: Option<i64>,
    /// Maximum distance for proximity alerts in metres.
    #[serde(
        default,
        alias = "proximityAlertRadius",
        skip_serializing_if = "Option::is_none"
    )]
    pub proximity_alert_radius: Option<i64>,
}

/// Compares the raw bits of both coordinates, so the type is reflexive even if
/// a payload ever carried `NaN` — which is what makes the `Eq` impl sound.
impl PartialEq for Location {
    fn eq(&self, other: &Self) -> bool {
        self.latitude.to_bits() == other.latitude.to_bits()
            && self.longitude.to_bits() == other.longitude.to_bits()
            && self.horizontal_accuracy.map(f64::to_bits)
                == other.horizontal_accuracy.map(f64::to_bits)
            && self.live_period == other.live_period
            && self.heading == other.heading
            && self.proximity_alert_radius == other.proximity_alert_radius
    }
}

impl Eq for Location {}

/// An inline query result the user picked and sent.
///
/// ```
/// use telegram_api_client::ChosenInlineResult;
///
/// let result: ChosenInlineResult = serde_json::from_str(
///     r#"{"result_id":"r1","from":{"id":1},"query":"ping"}"#,
/// ).unwrap();
/// assert_eq!(result.query, "ping");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ChosenInlineResult {
    /// Identifier of the chosen result.
    #[serde(default, alias = "resultId")]
    pub result_id: String,
    /// User who chose it.
    #[serde(default)]
    pub from: Option<User>,
    /// Sender location, if the bot requires it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,
    /// Identifier of the inline message, when it carries a keyboard.
    #[serde(
        default,
        alias = "inlineMessageId",
        skip_serializing_if = "Option::is_none"
    )]
    pub inline_message_id: Option<String>,
    /// Query that produced the result.
    #[serde(default)]
    pub query: String,
}

/// One answer option of a poll.
///
/// ```
/// use telegram_api_client::PollOption;
///
/// let option: PollOption = serde_json::from_str(r#"{"text":"Yes","voter_count":2}"#).unwrap();
/// assert_eq!(option.voter_count, 2);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct PollOption {
    /// Persistent identifier of the option, stable across vote updates.
    #[serde(
        default,
        alias = "persistentId",
        skip_serializing_if = "Option::is_none"
    )]
    pub persistent_id: Option<String>,
    /// Option text.
    #[serde(default)]
    pub text: String,
    /// Formatting entities of the option text.
    #[serde(
        default,
        alias = "textEntities",
        skip_serializing_if = "Option::is_none"
    )]
    pub text_entities: Option<Vec<MessageEntity>>,
    /// Votes for this option so far.
    #[serde(default, alias = "voterCount")]
    pub voter_count: i64,
    /// User who added the option, for polls that allow it.
    #[serde(
        default,
        alias = "addedByUser",
        skip_serializing_if = "Option::is_none"
    )]
    pub added_by_user: Option<User>,
    /// Chat that added the option.
    #[serde(
        default,
        alias = "addedByChat",
        skip_serializing_if = "Option::is_none"
    )]
    pub added_by_chat: Option<Chat>,
    /// Unix time the option was added.
    #[serde(
        default,
        alias = "additionDate",
        skip_serializing_if = "Option::is_none"
    )]
    pub addition_date: Option<i64>,
}

/// The state of a poll.
///
/// ```
/// use telegram_api_client::{Poll, PollType};
///
/// let poll: Poll = serde_json::from_str(
///     r#"{"id":"p1","question":"Lunch?","options":[{"text":"Yes","voter_count":1}],
///         "total_voter_count":1,"is_closed":false,"is_anonymous":true,"type":"regular",
///         "allows_multiple_answers":false}"#,
/// ).unwrap();
/// assert_eq!(poll.poll_type, PollType::Regular);
/// assert_eq!(poll.options.len(), 1);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
// Five independent flags, all part of the documented wire shape.
#[allow(clippy::struct_excessive_bools)]
pub struct Poll {
    /// Poll identifier.
    #[serde(default)]
    pub id: String,
    /// The question, 1-300 characters.
    #[serde(default)]
    pub question: String,
    /// Entities in the question.
    #[serde(
        default,
        alias = "questionEntities",
        skip_serializing_if = "Option::is_none"
    )]
    pub question_entities: Option<Vec<MessageEntity>>,
    /// The answer options.
    #[serde(default)]
    pub options: Vec<PollOption>,
    /// Number of users who voted.
    #[serde(default, alias = "totalVoterCount")]
    pub total_voter_count: i64,
    /// Whether the poll is closed.
    #[serde(default, alias = "isClosed")]
    pub is_closed: bool,
    /// Whether votes are anonymous.
    #[serde(default, alias = "isAnonymous")]
    pub is_anonymous: bool,
    /// `regular` or `quiz`.
    #[serde(rename = "type", alias = "pollType", default)]
    pub poll_type: crate::types::PollType,
    /// Whether several answers may be picked.
    #[serde(default, alias = "allowsMultipleAnswers")]
    pub allows_multiple_answers: bool,
    /// Whether voters may change their answer.
    #[serde(default, alias = "allowsRevoting")]
    pub allows_revoting: bool,
    /// Whether voting is limited to long-standing members.
    #[serde(default, alias = "membersOnly")]
    pub members_only: bool,
    /// Countries allowed to vote, ISO 3166-1 alpha-2.
    #[serde(
        default,
        alias = "countryCodes",
        skip_serializing_if = "Option::is_none"
    )]
    pub country_codes: Option<Vec<String>>,
    /// 0-based ids of the correct options, for closed quizzes.
    #[serde(
        default,
        alias = "correctOptionIds",
        skip_serializing_if = "Option::is_none"
    )]
    pub correct_option_ids: Option<Vec<i64>>,
    /// Explanation shown after a wrong quiz answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
    /// Entities in the explanation.
    #[serde(
        default,
        alias = "explanationEntities",
        skip_serializing_if = "Option::is_none"
    )]
    pub explanation_entities: Option<Vec<MessageEntity>>,
    /// Seconds the poll stays open after creation.
    #[serde(default, alias = "openPeriod", skip_serializing_if = "Option::is_none")]
    pub open_period: Option<i64>,
    /// Unix time the poll closes automatically.
    #[serde(default, alias = "closeDate", skip_serializing_if = "Option::is_none")]
    pub close_date: Option<i64>,
    /// Description of the poll.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Entities in the description.
    #[serde(
        default,
        alias = "descriptionEntities",
        skip_serializing_if = "Option::is_none"
    )]
    pub description_entities: Option<Vec<MessageEntity>>,
}

/// A user changed their answer in a non-anonymous poll.
///
/// ```
/// use telegram_api_client::PollAnswer;
///
/// let answer: PollAnswer = serde_json::from_str(
///     r#"{"poll_id":"p1","user":{"id":1},"option_ids":[0,2]}"#,
/// ).unwrap();
/// assert_eq!(answer.option_ids, vec![0, 2]);
/// assert!(answer.is_vote());
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct PollAnswer {
    /// Poll that was answered.
    #[serde(default, alias = "pollId")]
    pub poll_id: String,
    /// Chat that voted, for anonymous voters.
    #[serde(default, alias = "voterChat", skip_serializing_if = "Option::is_none")]
    pub voter_chat: Option<Chat>,
    /// User who voted, unless the vote is anonymous.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
    /// 0-based ids of the chosen options; empty when the vote is retracted.
    #[serde(default, alias = "optionIds")]
    pub option_ids: Vec<i64>,
    /// Persistent ids of the chosen options.
    #[serde(default, alias = "optionPersistentIds")]
    pub option_persistent_ids: Vec<String>,
}

impl PollAnswer {
    /// Whether this update casts a vote rather than retracting one.
    ///
    /// ```
    /// use telegram_api_client::PollAnswer;
    ///
    /// let retracted = PollAnswer::default();
    /// assert!(!retracted.is_vote());
    /// ```
    #[must_use]
    pub fn is_vote(&self) -> bool {
        !self.option_ids.is_empty() || !self.option_persistent_ids.is_empty()
    }
}

/// A bot that the current bot manages was created, or its token or owner
/// changed.
///
/// ```
/// use telegram_api_client::ManagedBotUpdated;
///
/// let update: ManagedBotUpdated = serde_json::from_str(
///     r#"{"user":{"id":1},"bot":{"id":2,"first_name":"Helper","is_bot":true}}"#,
/// ).unwrap();
/// assert_eq!(update.bot.as_ref().map(|bot| bot.first_name.as_str()), Some("Helper"));
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ManagedBotUpdated {
    /// User who created the bot.
    #[serde(default)]
    pub user: Option<User>,
    /// The managed bot itself.
    #[serde(default)]
    pub bot: Option<User>,
}

/// A user's payment subscription towards the bot changed.
///
/// ```
/// use telegram_api_client::BotSubscriptionUpdated;
///
/// let update: BotSubscriptionUpdated = serde_json::from_str(
///     r#"{"user":{"id":1},"invoice_payload":"sub-7","state":"canceled"}"#,
/// ).unwrap();
/// assert_eq!(update.state, "canceled");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct BotSubscriptionUpdated {
    /// The subscriber.
    #[serde(default)]
    pub user: Option<User>,
    /// Bot-specified payload of the subscription.
    #[serde(default, alias = "invoicePayload")]
    pub invoice_payload: String,
    /// New state: `canceled`, `active` or `failed`. Carried verbatim so a new
    /// state does not fail the parse.
    #[serde(default)]
    pub state: String,
}

/// A user asked the bot to stop generating a message.
///
/// ```
/// use telegram_api_client::MessageGenerationStopped;
///
/// let stopped: MessageGenerationStopped = serde_json::from_str(
///     r#"{"chat":{"id":-1},"message_thread_id":4,"draft_id":9}"#,
/// ).unwrap();
/// assert_eq!(stopped.draft_id, 9);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MessageGenerationStopped {
    /// Chat the message is being generated in.
    #[serde(default)]
    pub chat: Chat,
    /// Thread the message is generated in.
    #[serde(
        default,
        alias = "messageThreadId",
        skip_serializing_if = "Option::is_none"
    )]
    pub message_thread_id: Option<i64>,
    /// Identifier of the draft that was stopped.
    #[serde(default, alias = "draftId")]
    pub draft_id: i64,
}
