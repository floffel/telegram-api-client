//! Typed update objects for the pinned Bot API version.
//!
//! Every field of Telegram's `Update` object is modelled (see
//! [`BOT_API_VERSION`](crate::BOT_API_VERSION) and `docs/contracts.md`), one
//! `Option` per kind, at most one of them present per update. Unknown JSON
//! fields are ignored and unknown enum variants are carried through, so a newer
//! Bot API cannot break parsing.

use serde::{Deserialize, Serialize};

use crate::events::{
    BotSubscriptionUpdated, BusinessConnection, BusinessMessagesDeleted, ChatBoostRemoved,
    ChatBoostUpdated, ChatJoinRequest, ChatMemberUpdated, ChosenInlineResult, ManagedBotUpdated,
    MessageGenerationStopped, MessageReactionCountUpdated, MessageReactionUpdated,
    PaidMediaPurchased, Poll, PollAnswer, PreCheckoutQuery, ShippingQuery,
};
use crate::types::{ChatType, UpdateKind};

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
    pub message: Option<Box<Message>>,
    /// Present for edited messages.
    #[serde(
        default,
        alias = "editedMessage",
        skip_serializing_if = "Option::is_none"
    )]
    pub edited_message: Option<Box<Message>>,
    /// Present for new channel posts.
    #[serde(
        default,
        alias = "channelPost",
        skip_serializing_if = "Option::is_none"
    )]
    pub channel_post: Option<Box<Message>>,
    /// Present for edited channel posts.
    #[serde(
        default,
        alias = "editedChannelPost",
        skip_serializing_if = "Option::is_none"
    )]
    pub edited_channel_post: Option<Box<Message>>,
    /// The bot was connected to, disconnected from or re-configured for a
    /// business account.
    #[serde(
        default,
        alias = "businessConnection",
        skip_serializing_if = "Option::is_none"
    )]
    pub business_connection: Option<Box<BusinessConnection>>,
    /// New message from a connected business account.
    #[serde(
        default,
        alias = "businessMessage",
        skip_serializing_if = "Option::is_none"
    )]
    pub business_message: Option<Box<Message>>,
    /// Edited message from a connected business account.
    #[serde(
        default,
        alias = "editedBusinessMessage",
        skip_serializing_if = "Option::is_none"
    )]
    pub edited_business_message: Option<Box<Message>>,
    /// Messages were deleted from a connected business account.
    #[serde(
        default,
        alias = "deletedBusinessMessages",
        skip_serializing_if = "Option::is_none"
    )]
    pub deleted_business_messages: Option<Box<BusinessMessagesDeleted>>,
    /// New guest message; answer it through `guest_query_id`.
    #[serde(
        default,
        alias = "guestMessage",
        skip_serializing_if = "Option::is_none"
    )]
    pub guest_message: Option<Box<Message>>,
    /// A reaction on a message was changed by a user.
    #[serde(
        default,
        alias = "messageReaction",
        skip_serializing_if = "Option::is_none"
    )]
    pub message_reaction: Option<Box<MessageReactionUpdated>>,
    /// Reaction counts on a message with anonymous reactions changed.
    #[serde(
        default,
        alias = "messageReactionCount",
        skip_serializing_if = "Option::is_none"
    )]
    pub message_reaction_count: Option<Box<MessageReactionCountUpdated>>,
    /// Present for inline mode queries.
    #[serde(
        default,
        alias = "inlineQuery",
        skip_serializing_if = "Option::is_none"
    )]
    pub inline_query: Option<Box<InlineQuery>>,
    /// An inline query result was chosen and sent.
    #[serde(
        default,
        alias = "chosenInlineResult",
        skip_serializing_if = "Option::is_none"
    )]
    pub chosen_inline_result: Option<Box<ChosenInlineResult>>,
    /// Present when an inline keyboard button was pressed.
    #[serde(
        default,
        alias = "callbackQuery",
        skip_serializing_if = "Option::is_none"
    )]
    pub callback_query: Option<Box<CallbackQuery>>,
    /// Incoming shipping query, for invoices with a flexible price.
    #[serde(
        default,
        alias = "shippingQuery",
        skip_serializing_if = "Option::is_none"
    )]
    pub shipping_query: Option<Box<ShippingQuery>>,
    /// Incoming pre-checkout query.
    #[serde(
        default,
        alias = "preCheckoutQuery",
        skip_serializing_if = "Option::is_none"
    )]
    pub pre_checkout_query: Option<Box<PreCheckoutQuery>>,
    /// A user bought paid media from the bot.
    #[serde(
        default,
        alias = "purchasedPaidMedia",
        skip_serializing_if = "Option::is_none"
    )]
    pub purchased_paid_media: Option<Box<PaidMediaPurchased>>,
    /// New poll state for a stopped poll or one the bot sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poll: Option<Box<Poll>>,
    /// A user changed their answer in a non-anonymous poll.
    #[serde(default, alias = "pollAnswer", skip_serializing_if = "Option::is_none")]
    pub poll_answer: Option<Box<PollAnswer>>,
    /// The bot's own membership changed.
    #[serde(
        default,
        alias = "myChatMember",
        skip_serializing_if = "Option::is_none"
    )]
    pub my_chat_member: Option<Box<ChatMemberUpdated>>,
    /// Another member's membership changed.
    #[serde(default, alias = "chatMember", skip_serializing_if = "Option::is_none")]
    pub chat_member: Option<Box<ChatMemberUpdated>>,
    /// Someone asked to join the chat.
    #[serde(
        default,
        alias = "chatJoinRequest",
        skip_serializing_if = "Option::is_none"
    )]
    pub chat_join_request: Option<Box<ChatJoinRequest>>,
    /// A chat boost was added or changed.
    #[serde(default, alias = "chatBoost", skip_serializing_if = "Option::is_none")]
    pub chat_boost: Option<Box<ChatBoostUpdated>>,
    /// A boost was removed from a chat.
    #[serde(
        default,
        alias = "removedChatBoost",
        skip_serializing_if = "Option::is_none"
    )]
    pub removed_chat_boost: Option<Box<ChatBoostRemoved>>,
    /// A bot managed by this bot was created or changed.
    #[serde(default, alias = "managedBot", skip_serializing_if = "Option::is_none")]
    pub managed_bot: Option<Box<ManagedBotUpdated>>,
    /// A user's payment subscription towards the bot changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription: Option<Box<BotSubscriptionUpdated>>,
    /// A user asked the bot to stop generating a message.
    #[serde(
        default,
        alias = "stoppedMessageGeneration",
        skip_serializing_if = "Option::is_none"
    )]
    pub stopped_message_generation: Option<Box<MessageGenerationStopped>>,
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
    pub message: Option<Box<Message>>,
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

    /// The message this update carries: new, edited, channel, business or guest
    /// message.
    ///
    /// ```
    /// use telegram_api_client::Update;
    ///
    /// let update: Update = serde_json::from_str(
    ///     r#"{"update_id":1,"channel_post":{"message_id":2,"chat":{"id":-4}}}"#,
    /// ).unwrap();
    /// assert_eq!(update.primary_message().unwrap().message_id, 2);
    ///
    /// let business: Update = serde_json::from_str(
    ///     r#"{"update_id":2,"business_message":{"message_id":3,"chat":{"id":5},"text":"hi"}}"#,
    /// ).unwrap();
    /// assert_eq!(business.text(), Some("hi"));
    /// ```
    #[must_use]
    pub fn primary_message(&self) -> Option<&Message> {
        self.message
            .as_deref()
            .or(self.edited_message.as_deref())
            .or(self.channel_post.as_deref())
            .or(self.edited_channel_post.as_deref())
            .or(self.business_message.as_deref())
            .or(self.edited_business_message.as_deref())
            .or(self.guest_message.as_deref())
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

    /// Which of the 27 update kinds this update carries.
    ///
    /// The Bot API sends at most one payload field per update, so this is the
    /// single dispatch point for a handler. `None` means the payload is missing
    /// or of a kind newer than [`crate::BOT_API_VERSION`].
    ///
    /// ```
    /// use telegram_api_client::{Update, UpdateKind};
    ///
    /// let update: Update = serde_json::from_str(
    ///     r#"{"update_id":1,"chat_join_request":{"chat":{"id":-1},"from":{"id":2}}}"#,
    /// ).unwrap();
    /// assert_eq!(update.kind(), Some(UpdateKind::ChatJoinRequest));
    ///
    /// let empty = Update::default();
    /// assert_eq!(empty.kind(), None);
    /// ```
    #[must_use]
    pub fn kind(&self) -> Option<UpdateKind> {
        if self.message.is_some() {
            Some(UpdateKind::Message)
        } else if self.edited_message.is_some() {
            Some(UpdateKind::EditedMessage)
        } else if self.channel_post.is_some() {
            Some(UpdateKind::ChannelPost)
        } else if self.edited_channel_post.is_some() {
            Some(UpdateKind::EditedChannelPost)
        } else if self.business_connection.is_some() {
            Some(UpdateKind::BusinessConnection)
        } else if self.business_message.is_some() {
            Some(UpdateKind::BusinessMessage)
        } else if self.edited_business_message.is_some() {
            Some(UpdateKind::EditedBusinessMessage)
        } else if self.deleted_business_messages.is_some() {
            Some(UpdateKind::DeletedBusinessMessages)
        } else if self.guest_message.is_some() {
            Some(UpdateKind::GuestMessage)
        } else if self.message_reaction.is_some() {
            Some(UpdateKind::MessageReaction)
        } else if self.message_reaction_count.is_some() {
            Some(UpdateKind::MessageReactionCount)
        } else if self.inline_query.is_some() {
            Some(UpdateKind::InlineQuery)
        } else if self.chosen_inline_result.is_some() {
            Some(UpdateKind::ChosenInlineResult)
        } else if self.callback_query.is_some() {
            Some(UpdateKind::CallbackQuery)
        } else if self.shipping_query.is_some() {
            Some(UpdateKind::ShippingQuery)
        } else if self.pre_checkout_query.is_some() {
            Some(UpdateKind::PreCheckoutQuery)
        } else if self.purchased_paid_media.is_some() {
            Some(UpdateKind::PurchasedPaidMedia)
        } else if self.poll.is_some() {
            Some(UpdateKind::Poll)
        } else if self.poll_answer.is_some() {
            Some(UpdateKind::PollAnswer)
        } else if self.my_chat_member.is_some() {
            Some(UpdateKind::MyChatMember)
        } else if self.chat_member.is_some() {
            Some(UpdateKind::ChatMember)
        } else if self.chat_join_request.is_some() {
            Some(UpdateKind::ChatJoinRequest)
        } else if self.chat_boost.is_some() {
            Some(UpdateKind::ChatBoost)
        } else if self.removed_chat_boost.is_some() {
            Some(UpdateKind::RemovedChatBoost)
        } else if self.managed_bot.is_some() {
            Some(UpdateKind::ManagedBot)
        } else if self.subscription.is_some() {
            Some(UpdateKind::Subscription)
        } else {
            self.stopped_message_generation
                .as_ref()
                .map(|_| UpdateKind::StoppedMessageGeneration)
        }
    }
    /// The membership change this update carries, for `my_chat_member` and
    /// `chat_member`.
    ///
    /// ```
    /// use telegram_api_client::{ChatMemberStatus, Update};
    ///
    /// let update: Update = serde_json::from_str(
    ///     r#"{"update_id":1,"my_chat_member":{"chat":{"id":-1},"date":1,
    ///         "new_chat_member":{"status":"member","user":{"id":2}}}}"#,
    /// ).unwrap();
    /// let changed = update.chat_member_update().unwrap();
    /// assert_eq!(changed.new_chat_member.status(), ChatMemberStatus::Member);
    /// assert!(update.chat_member.is_none());
    /// ```
    #[must_use]
    pub fn chat_member_update(&self) -> Option<&ChatMemberUpdated> {
        self.my_chat_member
            .as_deref()
            .or(self.chat_member.as_deref())
    }
}
