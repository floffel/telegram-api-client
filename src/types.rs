//! Typed building blocks shared by requests and updates.
//!
//! Every type here is permissive on the way in: unknown enum variants are
//! carried as `Unknown(..)` payloads, unknown JSON fields are ignored, and both
//! `snake_case` and `camelCase` field spellings are accepted where Telegram
//! has used both over time.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::send::{MAX_PHOTO_BYTES, MAX_UPLOAD_BYTES};
use crate::update::User;

/// Generates a string-backed enum that keeps unknown wire values instead of
/// failing to deserialize them.
macro_rules! string_enum {
    (
        $(#[$outer:meta])*
        pub enum $name:ident {
            $(
                $(#[$vmeta:meta])*
                $variant:ident => $literal:literal,
            )*
        }
    ) => {
        $(#[$outer])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        pub enum $name {
            $(
                $(#[$vmeta])*
                $variant,
            )*
            /// A value this crate does not know yet — carried verbatim.
            Unknown(String),
        }

        impl $name {
            /// The exact wire representation.
            #[must_use]
            pub fn as_str(&self) -> &str {
                match self {
                    $(Self::$variant => $literal,)*
                    Self::Unknown(value) => value.as_str(),
                }
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::Unknown(String::new())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                match value {
                    $($literal => Self::$variant,)*
                    other => Self::Unknown(other.to_string()),
                }
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                match value.as_str() {
                    $($literal => Self::$variant,)*
                    _ => Self::Unknown(value),
                }
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                Ok(Self::from(String::deserialize(deserializer)?))
            }
        }
    };
}

string_enum! {
    /// How `text`/`caption` should be parsed by Telegram.
    ///
    /// ```
    /// use telegram_api_client::ParseMode;
    ///
    /// assert_eq!(ParseMode::Html.to_string(), "HTML");
    /// assert_eq!(ParseMode::from("MarkdownV2"), ParseMode::MarkdownV2);
    /// // Unknown values survive a round trip instead of failing.
    /// assert_eq!(ParseMode::from("MarkdownV3"), ParseMode::Unknown("MarkdownV3".into()));
    /// ```
    pub enum ParseMode {
        /// Legacy Markdown.
        Markdown => "Markdown",
        /// Markdown with the 2.0 escape rules.
        MarkdownV2 => "MarkdownV2",
        /// HTML subset.
        Html => "HTML",
    }
}

string_enum! {
    /// Kinds of [`MessageEntity`].
    ///
    /// ```
    /// use telegram_api_client::EntityKind;
    ///
    /// assert_eq!(EntityKind::from("text_link").as_str(), "text_link");
    /// assert_eq!(EntityKind::from("brand_new").as_str(), "brand_new");
    /// ```
    pub enum EntityKind {
        /// Bold text.
        Bold => "bold",
        /// Italic text.
        Italic => "italic",
        /// Underlined text.
        Underline => "underline",
        /// Struck-through text.
        Strikethrough => "strikethrough",
        /// Spoiler.
        Spoiler => "spoiler",
        /// Block quotation.
        Blockquote => "blockquote",
        /// Expandable block quotation.
        ExpandableBlockquote => "expandable_blockquote",
        /// Monospaced inline text.
        Code => "code",
        /// Monospaced block.
        Pre => "pre",
        /// Clickable link.
        TextLink => "text_link",
        /// Mention of a user without a username.
        TextMention => "text_mention",
        /// Custom emoji.
        CustomEmoji => "custom_emoji",
    }
}

string_enum! {
    /// Type of a chat.
    ///
    /// ```
    /// use telegram_api_client::ChatType;
    ///
    /// assert_eq!(ChatType::from("supergroup"), ChatType::Supergroup);
    /// ```
    pub enum ChatType {
        /// One-to-one chat with a user.
        Private => "private",
        /// Small group.
        Group => "group",
        /// Large group.
        Supergroup => "supergroup",
        /// Channel.
        Channel => "channel",
    }
}

string_enum! {
    /// `chatAction` values for [`crate::SendChatAction`].
    ///
    /// ```
    /// use telegram_api_client::ChatAction;
    ///
    /// assert_eq!(ChatAction::Typing.as_str(), "typing");
    /// ```
    pub enum ChatAction {
        /// Typing text.
        Typing => "typing",
        /// Uploading a photo.
        UploadPhoto => "upload_photo",
        /// Recording a video.
        RecordVideo => "record_video",
        /// Uploading a video.
        UploadVideo => "upload_video",
        /// Recording a voice note.
        RecordVoice => "record_voice",
        /// Uploading a voice note.
        UploadVoice => "upload_voice",
        /// Uploading a document.
        UploadDocument => "upload_document",
        /// Picking a sticker.
        ChooseSticker => "choose_sticker",
        /// Sharing a location.
        FindLocation => "find_location",
        /// Recording a video note.
        RecordVideoNote => "record_video_note",
        /// Uploading a video note.
        UploadVideoNote => "upload_video_note",
    }
}

string_enum! {
    /// Membership status of a chat member.
    ///
    /// ```
    /// use telegram_api_client::ChatMemberStatus;
    ///
    /// assert_eq!(ChatMemberStatus::from("kicked"), ChatMemberStatus::Kicked);
    /// ```
    pub enum ChatMemberStatus {
        /// Creator/owner.
        Creator => "creator",
        /// Administrator.
        Administrator => "administrator",
        /// Normal member.
        Member => "member",
        /// Restricted member.
        Restricted => "restricted",
        /// Left the chat.
        Left => "left",
        /// Banned from the chat.
        Kicked => "kicked",
    }
}

string_enum! {
    /// Update kinds accepted by `getUpdates` and `setWebhook`.
    ///
    /// ```
    /// use telegram_api_client::UpdateKind;
    ///
    /// let kinds = [UpdateKind::Message, UpdateKind::CallbackQuery];
    /// assert_eq!(kinds[1].as_str(), "callback_query");
    /// ```
    pub enum UpdateKind {
        /// New message.
        Message => "message",
        /// Edited message.
        EditedMessage => "edited_message",
        /// Channel post.
        ChannelPost => "channel_post",
        /// Edited channel post.
        EditedChannelPost => "edited_channel_post",
        /// Inline keyboard press.
        CallbackQuery => "callback_query",
        /// Inline query.
        InlineQuery => "inline_query",
        /// Chosen inline result.
        ChosenInlineResult => "chosen_inline_result",
        /// Poll state changed.
        Poll => "poll",
        /// Poll answer.
        PollAnswer => "poll_answer",
        /// Bot membership changed.
        MyChatMember => "my_chat_member",
        /// Chat member changed.
        ChatMember => "chat_member",
        /// Pre-checkout query.
        PreCheckoutQuery => "pre_checkout_query",
        /// Shipping query.
        ShippingQuery => "shipping_query",
    }
}

string_enum! {
    /// `type` of a poll.
    ///
    /// ```
    /// use telegram_api_client::PollType;
    ///
    /// assert_eq!(PollType::Quiz.as_str(), "quiz");
    /// ```
    pub enum PollType {
        /// Regular poll.
        Regular => "regular",
        /// Quiz with one correct answer.
        Quiz => "quiz",
    }
}

string_enum! {
    /// `type` of an [`InputMedia`] group item.
    ///
    /// ```
    /// use telegram_api_client::InputMediaKind;
    ///
    /// assert_eq!(InputMediaKind::Photo.as_str(), "photo");
    /// ```
    pub enum InputMediaKind {
        /// Photo.
        Photo => "photo",
        /// Video.
        Video => "video",
        /// Animation (GIF or soundless MP4).
        Animation => "animation",
        /// Audio file.
        Audio => "audio",
        /// Document.
        Document => "document",
    }
}

impl InputMediaKind {
    /// MIME families Telegram accepts for this kind of media.
    ///
    /// ```rust
    /// use telegram_api_client::InputMediaKind;
    ///
    /// assert_eq!(InputMediaKind::Photo.mime_families(), ["image/"]);
    /// assert!(InputMediaKind::Document.mime_families().is_empty());
    /// ```
    #[must_use]
    pub fn mime_families(&self) -> &'static [&'static str] {
        match self {
            Self::Photo => &["image/"],
            Self::Video => &["video/"],
            Self::Animation => &["image/", "video/"],
            Self::Audio => &["audio/"],
            Self::Document | Self::Unknown(_) => &[],
        }
    }

    /// Bytes this kind of media may carry in a multipart upload.
    ///
    /// Photos are capped at 10 MB and everything else at 50 MB (Bot API,
    /// "Sending files": "10 MB max size for photos, 50 MB for other files").
    /// Unknown kinds get the general budget: the crate does not know what they
    /// carry, and refusing them would reject payloads Telegram accepts.
    ///
    /// ```rust
    /// use telegram_api_client::{InputMediaKind, MAX_PHOTO_BYTES, MAX_UPLOAD_BYTES};
    ///
    /// assert_eq!(InputMediaKind::Photo.upload_limit_bytes(), MAX_PHOTO_BYTES);
    /// assert_eq!(InputMediaKind::Video.upload_limit_bytes(), MAX_UPLOAD_BYTES);
    /// assert_eq!(InputMediaKind::from("sticker").upload_limit_bytes(), MAX_UPLOAD_BYTES);
    /// ```
    #[must_use]
    pub fn upload_limit_bytes(&self) -> usize {
        match self {
            Self::Photo => MAX_PHOTO_BYTES,
            Self::Video | Self::Animation | Self::Audio | Self::Document | Self::Unknown(_) => {
                MAX_UPLOAD_BYTES
            }
        }
    }

    /// Whether `mime` fits this kind of media.
    ///
    /// Unknown kinds accept anything: the crate does not know what they carry.
    ///
    /// ```rust
    /// use telegram_api_client::InputMediaKind;
    ///
    /// assert!(InputMediaKind::Photo.accepts_mime("image/png"));
    /// assert!(!InputMediaKind::Photo.accepts_mime("video/mp4"));
    /// assert!(InputMediaKind::from("sticker").accepts_mime("image/webp"));
    /// ```
    #[must_use]
    pub fn accepts_mime(&self, mime: &str) -> bool {
        let families = self.mime_families();
        families.is_empty() || families.iter().any(|family| mime.starts_with(family))
    }
}

/// A chat reference: numeric id or `@username`.
///
/// ```
/// use telegram_api_client::ChatId;
///
/// assert_eq!(ChatId::id(-100123).to_string(), "-100123");
/// assert_eq!(ChatId::username("@floffel"), ChatId::Username("floffel".into()));
/// // Serializes as the bare value, exactly as the Bot API expects it.
/// assert_eq!(serde_json::to_string(&ChatId::id(7)).unwrap(), "7");
/// assert_eq!(serde_json::to_string(&ChatId::username("floffel")).unwrap(), "\"floffel\"");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChatId {
    /// Numeric chat id, negative for groups and channels.
    Id(i64),
    /// Public username without the leading `@`.
    Username(String),
}

impl ChatId {
    /// Numeric chat id.
    #[must_use]
    pub fn id(id: i64) -> Self {
        Self::Id(id)
    }

    /// Public `@username`; a single leading `@` is stripped.
    #[must_use]
    pub fn username(username: impl Into<String>) -> Self {
        let username = username.into();
        Self::Username(username.trim_start_matches('@').to_string())
    }

    /// The numeric id, if this is not a username.
    #[must_use]
    pub fn as_id(&self) -> Option<i64> {
        match self {
            Self::Id(id) => Some(*id),
            Self::Username(_) => None,
        }
    }

    /// The username, if this is one.
    #[must_use]
    pub fn as_username(&self) -> Option<&str> {
        match self {
            Self::Id(_) => None,
            Self::Username(name) => Some(name),
        }
    }
}

impl Default for ChatId {
    /// The zero id — only useful as a struct default, never as a request.
    fn default() -> Self {
        Self::Id(0)
    }
}

impl fmt::Display for ChatId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Id(id) => write!(f, "{id}"),
            Self::Username(name) => f.write_str(name),
        }
    }
}

impl From<i64> for ChatId {
    fn from(id: i64) -> Self {
        Self::Id(id)
    }
}

impl From<&str> for ChatId {
    fn from(name: &str) -> Self {
        Self::Username(name.trim_start_matches('@').to_string())
    }
}

impl From<String> for ChatId {
    fn from(name: String) -> Self {
        Self::Username(name.trim_start_matches('@').to_string())
    }
}

/// One formatting entity inside `text` or `caption`.
///
/// Offsets and lengths are in UTF-16 code units, as defined by the Bot API.
///
/// ```
/// use telegram_api_client::{EntityKind, MessageEntity};
///
/// let entity = MessageEntity::bold(0, 5);
/// assert_eq!(entity.kind, EntityKind::Bold);
/// let json = serde_json::to_value(&entity).unwrap();
/// assert_eq!(json["type"], "bold");
/// assert_eq!(json["length"], 5);
/// // Unknown entity kinds are preserved.
/// let parsed: MessageEntity =
///     serde_json::from_str(r#"{"type":"future_kind","offset":1,"length":2}"#).unwrap();
/// assert_eq!(parsed.kind, EntityKind::Unknown("future_kind".into()));
/// ```
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
pub struct MessageEntity {
    /// What kind of formatting this is.
    #[serde(rename = "type")]
    pub kind: EntityKind,
    /// Start offset in UTF-16 code units.
    #[serde(default)]
    pub offset: i64,
    /// Length in UTF-16 code units.
    #[serde(default)]
    pub length: i64,
    /// Target of a `text_link` entity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Target user of a `text_mention` entity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
    /// Programming language of a `pre` entity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Identifier of a `custom_emoji` entity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_emoji_id: Option<String>,
}

impl MessageEntity {
    /// A `bold` entity.
    #[must_use]
    pub fn bold(offset: i64, length: i64) -> Self {
        Self::kind(EntityKind::Bold, offset, length)
    }

    /// An entity of the given kind.
    #[must_use]
    pub fn kind(kind: EntityKind, offset: i64, length: i64) -> Self {
        Self {
            kind,
            offset,
            length,
            ..Self::default()
        }
    }

    /// Turns this into a `text_link` entity pointing at `url`.
    #[must_use]
    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Attaches a language hint (for `pre` entities).
    #[must_use]
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }
}

/// Controls the link preview Telegram renders for a message.
///
/// ```
/// use telegram_api_client::LinkPreviewOptions;
///
/// let options = LinkPreviewOptions::disabled();
/// assert_eq!(serde_json::to_value(options).unwrap()["is_disabled"], true);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct LinkPreviewOptions {
    /// Disables previews entirely.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_disabled: Option<bool>,
    /// URL to preview instead of the first link in the text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Prefer a small preview.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefer_small_media: Option<bool>,
    /// Prefer a large preview.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefer_large_media: Option<bool>,
    /// Render the preview above the text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_above_text: Option<bool>,
}

impl LinkPreviewOptions {
    /// No preview at all.
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            is_disabled: Some(true),
            ..Self::default()
        }
    }
}

/// One button of an [`InlineKeyboardMarkup`].
///
/// ```
/// use telegram_api_client::InlineKeyboardButton;
///
/// let button = InlineKeyboardButton::callback("Yes", "vote:yes");
/// let json = serde_json::to_value(&button).unwrap();
/// assert_eq!(json["callback_data"], "vote:yes");
/// assert!(json.get("url").is_none());
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct InlineKeyboardButton {
    /// Label shown on the button.
    pub text: String,
    /// URL opened on press.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Data echoed back in the callback query (1–64 bytes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub callback_data: Option<String>,
    /// `web_app` payload for Mini Apps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_app: Option<WebAppInfo>,
    /// Prompts the user to pick a chat and query it inline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub switch_inline_query: Option<String>,
    /// Same, but stays in the current chat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub switch_inline_query_current_chat: Option<String>,
    /// Pay button for invoices.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pay: Option<bool>,
}

impl InlineKeyboardButton {
    /// A `callback_data` button.
    #[must_use]
    pub fn callback(text: impl Into<String>, data: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            callback_data: Some(data.into()),
            ..Self::default()
        }
    }

    /// A URL button.
    #[must_use]
    pub fn url(text: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            url: Some(url.into()),
            ..Self::default()
        }
    }
}

/// A `web_app` descriptor.
///
/// ```
/// use telegram_api_client::WebAppInfo;
///
/// let info = WebAppInfo::new("https://example.com/app");
/// assert_eq!(serde_json::to_value(info).unwrap()["url"], "https://example.com/app");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct WebAppInfo {
    /// HTTPS URL of the Mini App.
    pub url: String,
}

impl WebAppInfo {
    /// Wraps `url`.
    #[must_use]
    pub fn new(url: impl Into<String>) -> Self {
        Self { url: url.into() }
    }
}

/// An inline keyboard: rows of buttons.
///
/// ```
/// use telegram_api_client::{InlineKeyboardButton, InlineKeyboardMarkup};
///
/// let markup = InlineKeyboardMarkup::new(vec![vec![
///     InlineKeyboardButton::callback("Yes", "y"),
///     InlineKeyboardButton::callback("No", "n"),
/// ]]);
/// let json = serde_json::to_value(markup).unwrap();
/// assert_eq!(json["inline_keyboard"][0][1]["text"], "No");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct InlineKeyboardMarkup {
    /// Rows of buttons.
    pub inline_keyboard: Vec<Vec<InlineKeyboardButton>>,
}

impl InlineKeyboardMarkup {
    /// Builds a keyboard from rows.
    #[must_use]
    pub fn new(inline_keyboard: Vec<Vec<InlineKeyboardButton>>) -> Self {
        Self { inline_keyboard }
    }
}

/// One button of a [`ReplyKeyboardMarkup`].
///
/// ```
/// use telegram_api_client::KeyboardButton;
///
/// let button = KeyboardButton::text("Share location").requesting_location();
/// assert_eq!(serde_json::to_value(button).unwrap()["request_location"], true);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct KeyboardButton {
    /// Label shown on the button, sent back as the message text.
    pub text: String,
    /// Ask for the user's phone number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_contact: Option<bool>,
    /// Ask for the user's location.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_location: Option<bool>,
    /// Open a Mini App.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_app: Option<WebAppInfo>,
}

impl KeyboardButton {
    /// A plain text button.
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }

    /// Turns this into a contact request button.
    #[must_use]
    pub fn requesting_contact(mut self) -> Self {
        self.request_contact = Some(true);
        self
    }

    /// Turns this into a location request button.
    #[must_use]
    pub fn requesting_location(mut self) -> Self {
        self.request_location = Some(true);
        self
    }
}

/// A custom reply keyboard.
///
/// ```
/// use telegram_api_client::{KeyboardButton, ReplyKeyboardMarkup};
///
/// let markup = ReplyKeyboardMarkup::new(vec![vec![KeyboardButton::text("A")]])
///     .resized();
/// let json = serde_json::to_value(markup).unwrap();
/// assert_eq!(json["resize_keyboard"], true);
/// assert_eq!(json["keyboard"][0][0]["text"], "A");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReplyKeyboardMarkup {
    /// Rows of buttons.
    pub keyboard: Vec<Vec<KeyboardButton>>,
    /// Ask clients to fit the keyboard to the available space.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resize_keyboard: Option<bool>,
    /// Hide the keyboard after one use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub one_time_keyboard: Option<bool>,
    /// Placeholder for the input field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_field_placeholder: Option<String>,
    /// Show the keyboard only to selected users.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selective: Option<bool>,
    /// Whether the keyboard is persistent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_persistent: Option<bool>,
}

impl ReplyKeyboardMarkup {
    /// Builds a keyboard from rows.
    #[must_use]
    pub fn new(keyboard: Vec<Vec<KeyboardButton>>) -> Self {
        Self {
            keyboard,
            ..Self::default()
        }
    }

    /// Fits the keyboard to the screen.
    #[must_use]
    pub fn resized(mut self) -> Self {
        self.resize_keyboard = Some(true);
        self
    }

    /// Hides the keyboard after a single use.
    #[must_use]
    pub fn one_time(mut self) -> Self {
        self.one_time_keyboard = Some(true);
        self
    }
}

/// Instructions to remove a custom reply keyboard.
///
/// ```
/// use telegram_api_client::ReplyKeyboardRemove;
///
/// assert_eq!(
///     serde_json::to_string(&ReplyKeyboardRemove::default()).unwrap(),
///     r#"{"remove_keyboard":true}"#
/// );
/// ```
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReplyKeyboardRemove {
    /// Telegram expects `true`; the field must be present.
    pub remove_keyboard: bool,
    /// Only affect selected users.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selective: Option<bool>,
}

impl Default for ReplyKeyboardRemove {
    fn default() -> Self {
        Self {
            remove_keyboard: true,
            selective: None,
        }
    }
}

/// Forces the client to show a reply interface.
///
/// ```
/// use telegram_api_client::ForceReply;
///
/// assert_eq!(
///     serde_json::to_string(&ForceReply::default()).unwrap(),
///     r#"{"force_reply":true}"#
/// );
/// ```
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ForceReply {
    /// Telegram expects `true`; the field must be present.
    pub force_reply: bool,
    /// Placeholder for the input field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_field_placeholder: Option<String>,
    /// Only affect selected users.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selective: Option<bool>,
}

impl Default for ForceReply {
    fn default() -> Self {
        Self {
            force_reply: true,
            input_field_placeholder: None,
            selective: None,
        }
    }
}

/// Any `reply_markup` Telegram accepts.
///
/// ```
/// use telegram_api_client::{InlineKeyboardButton, InlineKeyboardMarkup, ReplyMarkup};
///
/// let markup = ReplyMarkup::inline(InlineKeyboardMarkup::new(vec![vec![
///     InlineKeyboardButton::callback("OK", "ok"),
/// ]]));
/// assert_eq!(serde_json::to_value(&markup).unwrap()["inline_keyboard"][0][0]["text"], "OK");
///
/// // Unknown shapes are carried, not dropped.
/// let raw: ReplyMarkup = serde_json::from_str(r#"{"future_markup":{"x":1}}"#).unwrap();
/// assert_eq!(raw, ReplyMarkup::Unknown(serde_json::json!({"future_markup": {"x": 1}})));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReplyMarkup {
    /// Buttons attached to the message.
    InlineKeyboard(InlineKeyboardMarkup),
    /// Replacement for the device keyboard.
    Keyboard(ReplyKeyboardMarkup),
    /// Remove the custom keyboard.
    RemoveKeyboard(ReplyKeyboardRemove),
    /// Ask the user to reply to this message.
    ForceReply(ForceReply),
    /// A shape this crate does not model.
    Unknown(Value),
}

impl ReplyMarkup {
    /// An inline keyboard.
    #[must_use]
    pub fn inline(markup: InlineKeyboardMarkup) -> Self {
        Self::InlineKeyboard(markup)
    }

    /// A custom reply keyboard.
    #[must_use]
    pub fn keyboard(markup: ReplyKeyboardMarkup) -> Self {
        Self::Keyboard(markup)
    }

    /// Remove the custom keyboard.
    #[must_use]
    pub fn remove() -> Self {
        Self::RemoveKeyboard(ReplyKeyboardRemove::default())
    }

    /// Force a reply to the message.
    #[must_use]
    pub fn force_reply() -> Self {
        Self::ForceReply(ForceReply::default())
    }
}

impl Serialize for ReplyMarkup {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::InlineKeyboard(markup) => markup.serialize(serializer),
            Self::Keyboard(markup) => markup.serialize(serializer),
            Self::RemoveKeyboard(markup) => markup.serialize(serializer),
            Self::ForceReply(markup) => markup.serialize(serializer),
            Self::Unknown(value) => value.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for ReplyMarkup {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;

        let value = Value::deserialize(deserializer)?;
        let has = |key: &str| value.get(key).is_some();

        if has("inline_keyboard") {
            InlineKeyboardMarkup::deserialize(value)
                .map(Self::InlineKeyboard)
                .map_err(D::Error::custom)
        } else if has("remove_keyboard") {
            ReplyKeyboardRemove::deserialize(value)
                .map(Self::RemoveKeyboard)
                .map_err(D::Error::custom)
        } else if has("force_reply") {
            ForceReply::deserialize(value)
                .map(Self::ForceReply)
                .map_err(D::Error::custom)
        } else if has("keyboard") {
            ReplyKeyboardMarkup::deserialize(value)
                .map(Self::Keyboard)
                .map_err(D::Error::custom)
        } else {
            Ok(Self::Unknown(value))
        }
    }
}

/// One item of a `sendMediaGroup` album.
///
/// Field names match the Bot API, so the JSON is forwarded verbatim. Unknown
/// fields and unknown `type` values are preserved through `extra`/`kind`.
///
/// ```
/// use telegram_api_client::{InputMedia, InputMediaKind};
///
/// let item = InputMedia::photo("file-id").with_caption("A");
/// let json = serde_json::to_value(&item).unwrap();
/// assert_eq!(json["type"], "photo");
/// assert_eq!(json["caption"], "A");
///
/// let item = InputMedia::new(InputMediaKind::from("hologram"), "x");
/// assert_eq!(serde_json::to_value(item).unwrap()["type"], "hologram");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct InputMedia {
    /// Kind of media; serialized as `type`.
    #[serde(rename = "type")]
    pub kind: InputMediaKind,
    /// `file_id` or HTTP URL of the media.
    pub media: String,
    /// Caption shown under the media.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    /// Caption parse mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parse_mode: Option<ParseMode>,
    /// Pre-computed caption entities.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub caption_entities: Vec<MessageEntity>,
    /// Blur the media until tapped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_spoiler: Option<bool>,
    /// Show the caption above the media.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_caption_above_media: Option<bool>,
    /// Fields this crate does not model, forwarded unchanged.
    #[serde(flatten, default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub extra: serde_json::Map<String, Value>,
    /// Bytes to upload instead of referencing the media remotely.
    ///
    /// Skipped on the wire: the enclosing request becomes multipart and the
    /// part is referenced as `attach://…`.
    #[serde(skip)]
    pub upload: Option<crate::send::UploadFile>,
}

impl InputMedia {
    /// A group item of the given kind.
    #[must_use]
    pub fn new(kind: InputMediaKind, media: impl Into<String>) -> Self {
        Self {
            kind,
            media: media.into(),
            ..Self::default()
        }
    }

    /// A photo item.
    #[must_use]
    pub fn photo(media: impl Into<String>) -> Self {
        Self::new(InputMediaKind::Photo, media)
    }

    /// A video item.
    #[must_use]
    pub fn video(media: impl Into<String>) -> Self {
        Self::new(InputMediaKind::Video, media)
    }

    /// A document item.
    #[must_use]
    pub fn document(media: impl Into<String>) -> Self {
        Self::new(InputMediaKind::Document, media)
    }

    /// Adds a caption.
    #[must_use]
    pub fn with_caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = Some(caption.into());
        self
    }

    /// Adds a caption parse mode.
    #[must_use]
    pub fn with_parse_mode(mut self, mode: ParseMode) -> Self {
        self.parse_mode = Some(mode);
        self
    }

    /// Uploads local bytes instead of referencing a `file_id` or URL.
    ///
    /// ```rust
    /// use telegram_api_client::{InputMedia, UploadFile};
    ///
    /// let item = InputMedia::photo("ignored").with_upload(UploadFile::new("b.png", vec![1, 2]));
    /// assert_eq!(item.media, "attach://media");
    /// assert_eq!(item.upload.as_ref().unwrap().len(), 2);
    /// ```
    #[must_use]
    pub fn with_upload(mut self, upload: crate::send::UploadFile) -> Self {
        self.upload = Some(upload);
        self.media = "attach://media".to_string();
        self
    }
}
