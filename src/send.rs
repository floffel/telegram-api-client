//! `sendX` request bodies.
//!
//! Every struct serializes to the exact JSON the Bot API expects (`snake_case`,
//! absent optional fields omitted) and validates what can be validated without
//! the network. Media is referenced by `file_id` or HTTP URL — see the README
//! for why multipart upload is out of scope.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::types::{
    ChatId, InputMedia, LinkPreviewOptions, MessageEntity, ParseMode, PollType, ReplyMarkup,
};

/// Longest accepted `text`, in UTF-16 code units (what Telegram counts).
pub const MAX_MESSAGE_TEXT: usize = 4096;
/// Longest accepted `caption`, in UTF-16 code units.
pub const MAX_CAPTION: usize = 1024;
/// Longest accepted poll question, in characters.
pub const MAX_POLL_QUESTION: usize = 300;
/// Longest accepted poll answer option, in characters.
pub const MAX_POLL_OPTION: usize = 100;
/// Most answer options a poll may have.
pub const MAX_POLL_OPTIONS: usize = 12;
/// Fewest answer options a poll may have.
pub const MIN_POLL_OPTIONS: usize = 2;
/// Most items a media group may contain.
pub const MAX_MEDIA_GROUP: usize = 10;
/// Fewest items a media group may contain.
pub const MIN_MEDIA_GROUP: usize = 2;

/// Rejects empty and over-long text, counting UTF-16 code units like Telegram.
pub(crate) fn validate_text(field: &'static str, value: &str, max: usize) -> Result<()> {
    if value.trim().is_empty() {
        return Err(Error::InvalidRequest {
            field,
            reason: "must not be empty".to_string(),
        });
    }
    let units = value.encode_utf16().count();
    if units > max {
        return Err(Error::InvalidRequest {
            field,
            reason: format!("must be at most {max} UTF-16 code units, got {units}"),
        });
    }
    Ok(())
}

/// Rejects an obviously unusable chat reference.
pub(crate) fn validate_chat(field: &'static str, chat: &ChatId) -> Result<()> {
    match chat {
        ChatId::Id(0) => Err(Error::InvalidRequest {
            field,
            reason: "chat id 0 is not a valid chat".to_string(),
        }),
        ChatId::Username(name) if name.is_empty() => Err(Error::InvalidRequest {
            field,
            reason: "username must not be empty".to_string(),
        }),
        ChatId::Id(_) | ChatId::Username(_) => Ok(()),
    }
}

/// Optional flags shared by every `sendX` method.
///
/// ```
/// use telegram_api_client::{DeliveryOptions, ReplyMarkup};
///
/// let options = DeliveryOptions::default().silent().reply_to(7);
/// let json = serde_json::to_value(&options).unwrap();
/// assert_eq!(json["disable_notification"], true);
/// assert_eq!(json["reply_to_message_id"], 7);
/// assert!(json.get("reply_markup").is_none());
/// assert!(ReplyMarkup::remove() != ReplyMarkup::force_reply());
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct DeliveryOptions {
    /// Send without a notification sound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_notification: Option<bool>,
    /// Prevent forwarding and saving.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protect_content: Option<bool>,
    /// Id of the message to reply to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to_message_id: Option<i64>,
    /// Deliver even when the replied-to message is gone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_sending_without_reply: Option<bool>,
    /// Keyboard or force-reply markup.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_markup: Option<ReplyMarkup>,
    /// Forum topic to post into.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_thread_id: Option<i64>,
}

impl DeliveryOptions {
    /// Send silently.
    #[must_use]
    pub fn silent(mut self) -> Self {
        self.disable_notification = Some(true);
        self
    }

    /// Protect the message from forwarding.
    #[must_use]
    pub fn protected(mut self) -> Self {
        self.protect_content = Some(true);
        self
    }

    /// Reply to `message_id`.
    #[must_use]
    pub fn reply_to(mut self, message_id: i64) -> Self {
        self.reply_to_message_id = Some(message_id);
        self
    }

    /// Attach a keyboard.
    #[must_use]
    pub fn with_reply_markup(mut self, markup: ReplyMarkup) -> Self {
        self.reply_markup = Some(markup);
        self
    }
}

/// Optional flags for text messages.
///
/// ```
/// use telegram_api_client::{MessageOptions, ParseMode};
///
/// let options = MessageOptions::default().with_parse_mode(ParseMode::Html).silent();
/// let json = serde_json::to_value(&options).unwrap();
/// assert_eq!(json["parse_mode"], "HTML");
/// assert_eq!(json["disable_notification"], true);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct MessageOptions {
    /// How Telegram should parse the text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parse_mode: Option<ParseMode>,
    /// Pre-computed formatting entities.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entities: Vec<MessageEntity>,
    /// Link preview behaviour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_preview_options: Option<LinkPreviewOptions>,
    /// Delivery flags (notification, reply, keyboard, thread).
    #[serde(flatten)]
    pub delivery: DeliveryOptions,
}

impl MessageOptions {
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

    /// Send silently.
    #[must_use]
    pub fn silent(mut self) -> Self {
        self.delivery = self.delivery.silent();
        self
    }

    /// Reply to `message_id`.
    #[must_use]
    pub fn reply_to(mut self, message_id: i64) -> Self {
        self.delivery = self.delivery.reply_to(message_id);
        self
    }

    /// Attach a keyboard.
    #[must_use]
    pub fn with_reply_markup(mut self, markup: ReplyMarkup) -> Self {
        self.delivery = self.delivery.with_reply_markup(markup);
        self
    }
}

/// Optional flags for caption-bearing media.
///
/// ```
/// use telegram_api_client::{MediaOptions, ParseMode};
///
/// let options = MediaOptions::default()
///     .with_caption("hi")
///     .with_parse_mode(ParseMode::MarkdownV2);
/// let json = serde_json::to_value(&options).unwrap();
/// assert_eq!(json["caption"], "hi");
/// assert_eq!(json["parse_mode"], "MarkdownV2");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct MediaOptions {
    /// Caption shown with the media.
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
    /// Delivery flags (notification, reply, keyboard, thread).
    #[serde(flatten)]
    pub delivery: DeliveryOptions,
}

impl MediaOptions {
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

    /// Send silently.
    #[must_use]
    pub fn silent(mut self) -> Self {
        self.delivery = self.delivery.silent();
        self
    }

    /// Reply to `message_id`.
    #[must_use]
    pub fn reply_to(mut self, message_id: i64) -> Self {
        self.delivery = self.delivery.reply_to(message_id);
        self
    }

    /// Attach a keyboard.
    #[must_use]
    pub fn with_reply_markup(mut self, markup: ReplyMarkup) -> Self {
        self.delivery = self.delivery.with_reply_markup(markup);
        self
    }

    /// Checks the caption length (and rejects an explicitly empty caption).
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] when the caption is empty or longer than
    /// [`MAX_CAPTION`].
    pub fn validate(&self) -> Result<()> {
        match self.caption.as_deref() {
            None => Ok(()),
            Some(caption) => validate_text("caption", caption, MAX_CAPTION),
        }
    }
}

/// `sendMessage` body.
///
/// ```
/// use telegram_api_client::{ParseMode, SendMessage};
///
/// let request = SendMessage::new(42, "Hallo")
///     .with_parse_mode(ParseMode::Html)
///     .silent();
/// let json = serde_json::to_value(&request).unwrap();
/// assert_eq!(json["chat_id"], 42);
/// assert_eq!(json["parse_mode"], "HTML");
/// assert_eq!(json["disable_notification"], true);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct SendMessage {
    /// Target chat.
    pub chat_id: ChatId,
    /// Message text.
    pub text: String,
    /// Text flags.
    #[serde(flatten)]
    pub options: MessageOptions,
}

impl SendMessage {
    /// A plain text message.
    #[must_use]
    pub fn new(chat_id: impl Into<ChatId>, text: impl Into<String>) -> Self {
        Self {
            chat_id: chat_id.into(),
            text: text.into(),
            options: MessageOptions::default(),
        }
    }

    /// Sets the parse mode.
    #[must_use]
    pub fn with_parse_mode(mut self, mode: ParseMode) -> Self {
        self.options = self.options.with_parse_mode(mode);
        self
    }

    /// Attaches formatting entities.
    #[must_use]
    pub fn with_entities(mut self, entities: Vec<MessageEntity>) -> Self {
        self.options = self.options.with_entities(entities);
        self
    }

    /// Overrides link previews.
    #[must_use]
    pub fn with_link_preview(mut self, options: LinkPreviewOptions) -> Self {
        self.options = self.options.with_link_preview(options);
        self
    }

    /// Send silently.
    #[must_use]
    pub fn silent(mut self) -> Self {
        self.options = self.options.silent();
        self
    }

    /// Reply to `message_id`.
    #[must_use]
    pub fn reply_to(mut self, message_id: i64) -> Self {
        self.options = self.options.reply_to(message_id);
        self
    }

    /// Attach a keyboard.
    #[must_use]
    pub fn with_reply_markup(mut self, markup: ReplyMarkup) -> Self {
        self.options = self.options.with_reply_markup(markup);
        self
    }

    /// Checks chat and text.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an unusable chat, empty text, or text over
    /// [`MAX_MESSAGE_TEXT`].
    pub fn validate(&self) -> Result<()> {
        validate_chat("chat_id", &self.chat_id)?;
        validate_text("text", &self.text, MAX_MESSAGE_TEXT)
    }
}

macro_rules! media_send {
    (
        $(#[$meta:meta])*
        pub struct $name:ident {
            $(#[$fmeta:meta])*
            pub $field:ident: String,
        }
        $doc:literal
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
        pub struct $name {
            /// Target chat.
            pub chat_id: ChatId,
            $(#[$fmeta])*
            pub $field: String,
            /// Caption and delivery flags.
            #[serde(flatten)]
            pub options: MediaOptions,
        }

        impl $name {
            #[doc = $doc]
            #[must_use]
            pub fn new(chat_id: impl Into<ChatId>, $field: impl Into<String>) -> Self {
                Self {
                    chat_id: chat_id.into(),
                    $field: $field.into(),
                    options: MediaOptions::default(),
                }
            }

            /// Sets the caption.
            #[must_use]
            pub fn with_caption(mut self, caption: impl Into<String>) -> Self {
                self.options = self.options.with_caption(caption);
                self
            }

            /// Sets the caption parse mode.
            #[must_use]
            pub fn with_parse_mode(mut self, mode: ParseMode) -> Self {
                self.options = self.options.with_parse_mode(mode);
                self
            }

            /// Send silently.
            #[must_use]
            pub fn silent(mut self) -> Self {
                self.options = self.options.silent();
                self
            }

            /// Reply to `message_id`.
            #[must_use]
            pub fn reply_to(mut self, message_id: i64) -> Self {
                self.options = self.options.reply_to(message_id);
                self
            }

            /// Attach a keyboard.
            #[must_use]
            pub fn with_reply_markup(mut self, markup: ReplyMarkup) -> Self {
                self.options = self.options.with_reply_markup(markup);
                self
            }

            /// Checks chat, media reference and caption.
            ///
            /// # Errors
            ///
            /// [`Error::InvalidRequest`] for an unusable chat, an empty media
            /// reference, or a caption that is empty or too long.
            pub fn validate(&self) -> Result<()> {
                validate_chat("chat_id", &self.chat_id)?;
                if self.$field.trim().is_empty() {
                    return Err(Error::InvalidRequest {
                        field: stringify!($field),
                        reason: "must not be empty".to_string(),
                    });
                }
                self.options.validate()
            }
        }
    };
}

media_send! {
    /// `sendPhoto` body.
    ///
    /// ```
    /// use telegram_api_client::SendPhoto;
    ///
    /// let request = SendPhoto::new(-1001, "AgACAgIAAxkBAAI").with_caption("sky");
    /// let json = serde_json::to_value(&request).unwrap();
    /// assert_eq!(json["photo"], "AgACAgIAAxkBAAI");
    /// assert_eq!(json["caption"], "sky");
    /// ```
    pub struct SendPhoto {
        /// `file_id` or HTTP URL of the photo.
        pub photo: String,
    }
    "A photo, referenced by `file_id` or URL."
}

media_send! {
    /// `sendDocument` body.
    ///
    /// ```
    /// use telegram_api_client::SendDocument;
    ///
    /// let json = serde_json::to_value(SendDocument::new(1, "BQACAgI")).unwrap();
    /// assert_eq!(json["document"], "BQACAgI");
    /// ```
    pub struct SendDocument {
        /// `file_id` or HTTP URL of the document.
        pub document: String,
    }
    "A document, referenced by `file_id` or URL."
}

media_send! {
    /// `sendAudio` body.
    ///
    /// ```
    /// use telegram_api_client::SendAudio;
    ///
    /// let json = serde_json::to_value(SendAudio::new(1, "CQACAgI")).unwrap();
    /// assert_eq!(json["audio"], "CQACAgI");
    /// ```
    pub struct SendAudio {
        /// `file_id` or HTTP URL of the audio file.
        pub audio: String,
    }
    "An audio track, referenced by `file_id` or URL."
}

media_send! {
    /// `sendVideo` body.
    ///
    /// ```
    /// use telegram_api_client::SendVideo;
    ///
    /// let json = serde_json::to_value(SendVideo::new(1, "DQACAgI")).unwrap();
    /// assert_eq!(json["video"], "DQACAgI");
    /// ```
    pub struct SendVideo {
        /// `file_id` or HTTP URL of the video.
        pub video: String,
    }
    "A video, referenced by `file_id` or URL."
}

media_send! {
    /// `sendAnimation` body.
    ///
    /// ```
    /// use telegram_api_client::SendAnimation;
    ///
    /// let json = serde_json::to_value(SendAnimation::new(1, "EQACAgI")).unwrap();
    /// assert_eq!(json["animation"], "EQACAgI");
    /// ```
    pub struct SendAnimation {
        /// `file_id` or HTTP URL of the animation.
        pub animation: String,
    }
    "An animation (GIF or soundless MP4), referenced by `file_id` or URL."
}

media_send! {
    /// `sendVoice` body.
    ///
    /// ```
    /// use telegram_api_client::SendVoice;
    ///
    /// let json = serde_json::to_value(SendVoice::new(1, "FQACAgI")).unwrap();
    /// assert_eq!(json["voice"], "FQACAgI");
    /// ```
    pub struct SendVoice {
        /// `file_id` or HTTP URL of the voice note.
        pub voice: String,
    }
    "A voice note, referenced by `file_id` or URL."
}

/// `sendMediaGroup` body — an album of 2 to 10 items.
///
/// ```
/// use telegram_api_client::{InputMedia, SendMediaGroup};
///
/// let album = SendMediaGroup::new(-1001, vec![
///     InputMedia::photo("a").with_caption("first"),
///     InputMedia::photo("b"),
/// ]);
/// let json = serde_json::to_value(&album).unwrap();
/// assert_eq!(json["media"][0]["type"], "photo");
/// assert_eq!(json["media"][1]["media"], "b");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct SendMediaGroup {
    /// Target chat.
    pub chat_id: ChatId,
    /// Album items; 2 to 10 of them.
    pub media: Vec<InputMedia>,
    /// Delivery flags.
    #[serde(flatten)]
    pub delivery: DeliveryOptions,
}

impl SendMediaGroup {
    /// An album.
    #[must_use]
    pub fn new(chat_id: impl Into<ChatId>, media: Vec<InputMedia>) -> Self {
        Self {
            chat_id: chat_id.into(),
            media,
            delivery: DeliveryOptions::default(),
        }
    }

    /// Send silently.
    #[must_use]
    pub fn silent(mut self) -> Self {
        self.delivery = self.delivery.silent();
        self
    }

    /// Reply to `message_id`.
    #[must_use]
    pub fn reply_to(mut self, message_id: i64) -> Self {
        self.delivery = self.delivery.reply_to(message_id);
        self
    }

    /// Checks chat, album size and per-item captions.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an unusable chat, an album outside
    /// `2..=10` items, or a caption that is empty or too long.
    pub fn validate(&self) -> Result<()> {
        validate_chat("chat_id", &self.chat_id)?;
        if self.media.len() < MIN_MEDIA_GROUP || self.media.len() > MAX_MEDIA_GROUP {
            return Err(Error::InvalidRequest {
                field: "media",
                reason: format!(
                    "must contain {MIN_MEDIA_GROUP} to {MAX_MEDIA_GROUP} items, got {}",
                    self.media.len()
                ),
            });
        }
        for item in &self.media {
            if item.media.trim().is_empty() {
                return Err(Error::InvalidRequest {
                    field: "media",
                    reason: "item media reference must not be empty".to_string(),
                });
            }
            if let Some(caption) = item.caption.as_deref() {
                validate_text("caption", caption, MAX_CAPTION)?;
            }
        }
        Ok(())
    }
}

/// `sendLocation` body.
///
/// ```
/// use telegram_api_client::SendLocation;
///
/// let request = SendLocation::new(42, 48.7665, 11.4258);
/// let json = serde_json::to_value(&request).unwrap();
/// assert_eq!(json["latitude"], 48.7665);
/// assert_eq!(json["longitude"], 11.4258);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct SendLocation {
    /// Target chat.
    pub chat_id: ChatId,
    /// Degrees north, `-90.0..=90.0`.
    pub latitude: f64,
    /// Degrees east, `-180.0..=180.0`.
    pub longitude: f64,
    /// Accuracy radius in metres.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horizontal_accuracy: Option<f64>,
    /// Live location period in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live_period: Option<u32>,
    /// Direction of movement, `1..=360`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading: Option<u16>,
    /// Alert radius in metres.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proximity_alert_radius: Option<u32>,
    /// Delivery flags.
    #[serde(flatten)]
    pub delivery: DeliveryOptions,
}

impl SendLocation {
    /// A static location.
    #[must_use]
    pub fn new(chat_id: impl Into<ChatId>, latitude: f64, longitude: f64) -> Self {
        Self {
            chat_id: chat_id.into(),
            latitude,
            longitude,
            ..Self::default()
        }
    }

    /// Send without a notification sound.
    #[must_use]
    pub fn silent(mut self) -> Self {
        self.delivery = self.delivery.silent();
        self
    }

    /// Checks chat and coordinates.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an unusable chat, out-of-range or `NaN`
    /// coordinates, `heading` outside `1..=360`, or a zero `live_period`.
    pub fn validate(&self) -> Result<()> {
        validate_chat("chat_id", &self.chat_id)?;
        if !(-90.0..=90.0).contains(&self.latitude) {
            return Err(Error::InvalidRequest {
                field: "latitude",
                reason: format!("must be within -90..=90, got {}", self.latitude),
            });
        }
        if !(-180.0..=180.0).contains(&self.longitude) {
            return Err(Error::InvalidRequest {
                field: "longitude",
                reason: format!("must be within -180..=180, got {}", self.longitude),
            });
        }
        if self
            .heading
            .is_some_and(|heading| !(1..=360).contains(&heading))
        {
            return Err(Error::InvalidRequest {
                field: "heading",
                reason: "must be within 1..=360".to_string(),
            });
        }
        if self.live_period == Some(0) {
            return Err(Error::InvalidRequest {
                field: "live_period",
                reason: "must be greater than 0".to_string(),
            });
        }
        Ok(())
    }
}

/// `sendVenue` body.
///
/// ```
/// use telegram_api_client::SendVenue;
///
/// let request = SendVenue::new(42, 48.7665, 11.4258, "Café", "Am Platz 1");
/// let json = serde_json::to_value(&request).unwrap();
/// assert_eq!(json["title"], "Café");
/// assert_eq!(json["address"], "Am Platz 1");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct SendVenue {
    /// Target chat.
    pub chat_id: ChatId,
    /// Degrees north.
    pub latitude: f64,
    /// Degrees east.
    pub longitude: f64,
    /// Venue name.
    pub title: String,
    /// Street address.
    pub address: String,
    /// Foursquare venue id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foursquare_id: Option<String>,
    /// Foursquare category type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foursquare_type: Option<String>,
    /// Google Places id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub google_place_id: Option<String>,
    /// Google Places type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub google_place_type: Option<String>,
    /// Delivery flags.
    #[serde(flatten)]
    pub delivery: DeliveryOptions,
}

impl SendVenue {
    /// A venue at the given coordinates.
    #[must_use]
    pub fn new(
        chat_id: impl Into<ChatId>,
        latitude: f64,
        longitude: f64,
        title: impl Into<String>,
        address: impl Into<String>,
    ) -> Self {
        Self {
            chat_id: chat_id.into(),
            latitude,
            longitude,
            title: title.into(),
            address: address.into(),
            ..Self::default()
        }
    }

    /// Send without a notification sound.
    #[must_use]
    pub fn silent(mut self) -> Self {
        self.delivery = self.delivery.silent();
        self
    }

    /// Checks chat, coordinates, title and address.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an unusable chat, out-of-range
    /// coordinates, or an empty title/address.
    pub fn validate(&self) -> Result<()> {
        validate_chat("chat_id", &self.chat_id)?;
        if !(-90.0..=90.0).contains(&self.latitude) || !(-180.0..=180.0).contains(&self.longitude) {
            return Err(Error::InvalidRequest {
                field: "latitude",
                reason: "coordinates are out of range".to_string(),
            });
        }
        validate_text("title", &self.title, MAX_CAPTION)?;
        validate_text("address", &self.address, MAX_CAPTION)
    }
}

/// `sendContact` body.
///
/// ```
/// use telegram_api_client::SendContact;
///
/// let request = SendContact::new(42, "+4915112345678", "Florian");
/// let json = serde_json::to_value(&request).unwrap();
/// assert_eq!(json["phone_number"], "+4915112345678");
/// assert!(json.get("last_name").is_none());
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct SendContact {
    /// Target chat.
    pub chat_id: ChatId,
    /// Phone number in any format.
    pub phone_number: String,
    /// First name.
    pub first_name: String,
    /// Last name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
    /// Extra vCard data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vcard: Option<String>,
    /// Delivery flags.
    #[serde(flatten)]
    pub delivery: DeliveryOptions,
}

impl SendContact {
    /// A contact card.
    #[must_use]
    pub fn new(
        chat_id: impl Into<ChatId>,
        phone_number: impl Into<String>,
        first_name: impl Into<String>,
    ) -> Self {
        Self {
            chat_id: chat_id.into(),
            phone_number: phone_number.into(),
            first_name: first_name.into(),
            ..Self::default()
        }
    }

    /// Send without a notification sound.
    #[must_use]
    pub fn silent(mut self) -> Self {
        self.delivery = self.delivery.silent();
        self
    }

    /// Checks chat, phone number and name.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an unusable chat, or an empty phone
    /// number/first name.
    pub fn validate(&self) -> Result<()> {
        validate_chat("chat_id", &self.chat_id)?;
        validate_text("phone_number", &self.phone_number, 64)?;
        validate_text("first_name", &self.first_name, 64)
    }
}

/// `sendPoll` body.
///
/// ```
/// use telegram_api_client::{PollType, SendPoll};
///
/// let poll = SendPoll::quiz(42, "2 + 2?", vec!["3".into(), "4".into()], 1);
/// let json = serde_json::to_value(&poll).unwrap();
/// assert_eq!(json["type"], "quiz");
/// assert_eq!(json["correct_option_id"], 1);
/// assert_eq!(json["options"][0], "3");
///
/// let plain = SendPoll::new(42, "Lunch?", vec!["Pizza".into(), "Salad".into()]);
/// assert_eq!(plain.kind, Some(PollType::Regular));
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct SendPoll {
    /// Target chat.
    pub chat_id: ChatId,
    /// Poll question, up to 300 characters.
    pub question: String,
    /// Answer options, 2 to 12.
    pub options: Vec<String>,
    /// `regular` or `quiz`.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<PollType>,
    /// Hide who voted for what.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_anonymous: Option<bool>,
    /// Allow more than one answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allows_multiple_answers: Option<bool>,
    /// Index of the correct answer (quizzes only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correct_option_id: Option<usize>,
    /// Shown when a user picks a wrong answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
    /// Seconds the poll stays open (5–600), mutually exclusive with `close_date`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_period: Option<u32>,
    /// Unix timestamp at which the poll closes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub close_date: Option<i64>,
    /// Start the poll closed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_closed: Option<bool>,
    /// Delivery flags.
    #[serde(flatten)]
    pub delivery: DeliveryOptions,
}

impl SendPoll {
    /// A regular, anonymous poll.
    #[must_use]
    pub fn new(
        chat_id: impl Into<ChatId>,
        question: impl Into<String>,
        options: Vec<String>,
    ) -> Self {
        Self {
            chat_id: chat_id.into(),
            question: question.into(),
            options,
            // Telegram defaults to `regular`, but being explicit makes the wire
            // body self-describing.
            kind: Some(PollType::Regular),
            ..Self::default()
        }
    }

    /// A quiz poll with `correct_option_id`.
    #[must_use]
    pub fn quiz(
        chat_id: impl Into<ChatId>,
        question: impl Into<String>,
        options: Vec<String>,
        correct_option_id: usize,
    ) -> Self {
        Self {
            kind: Some(PollType::Quiz),
            correct_option_id: Some(correct_option_id),
            is_anonymous: Some(false),
            ..Self::new(chat_id, question, options)
        }
    }

    /// Send without a notification sound.
    #[must_use]
    pub fn silent(mut self) -> Self {
        self.delivery = self.delivery.silent();
        self
    }

    /// Checks chat, question, options and quiz consistency.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an unusable chat, an empty or over-long
    /// question, an option count outside `2..=12`, an empty or over-long
    /// option, a `correct_option_id` outside the option list, both
    /// `open_period` and `close_date` set, or `open_period` outside `5..=600`.
    pub fn validate(&self) -> Result<()> {
        validate_chat("chat_id", &self.chat_id)?;
        validate_text("question", &self.question, MAX_POLL_QUESTION)?;
        if self.options.len() < MIN_POLL_OPTIONS || self.options.len() > MAX_POLL_OPTIONS {
            return Err(Error::InvalidRequest {
                field: "options",
                reason: format!(
                    "must contain {MIN_POLL_OPTIONS} to {MAX_POLL_OPTIONS} options, got {}",
                    self.options.len()
                ),
            });
        }
        for option in &self.options {
            validate_text("options", option, MAX_POLL_OPTION)?;
        }
        if self
            .correct_option_id
            .is_some_and(|id| id >= self.options.len())
        {
            return Err(Error::InvalidRequest {
                field: "correct_option_id",
                reason: format!("must index one of the {} options", self.options.len()),
            });
        }
        if self.open_period.is_some() && self.close_date.is_some() {
            return Err(Error::InvalidRequest {
                field: "open_period",
                reason: "open_period and close_date are mutually exclusive".to_string(),
            });
        }
        if self
            .open_period
            .is_some_and(|period| !(5..=600).contains(&period))
        {
            return Err(Error::InvalidRequest {
                field: "open_period",
                reason: "must be within 5..=600 seconds".to_string(),
            });
        }
        Ok(())
    }
}

/// `sendDice` body.
///
/// ```
/// use telegram_api_client::SendDice;
///
/// let json = serde_json::to_value(SendDice::new(42).with_emoji("🎯")).unwrap();
/// assert_eq!(json["emoji"], "🎯");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct SendDice {
    /// Target chat.
    pub chat_id: ChatId,
    /// Emoji deciding the animation (`🎲`, `🎯`, `🏀`, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<String>,
    /// Delivery flags.
    #[serde(flatten)]
    pub delivery: DeliveryOptions,
}

impl SendDice {
    /// A dice with the default emoji.
    #[must_use]
    pub fn new(chat_id: impl Into<ChatId>) -> Self {
        Self {
            chat_id: chat_id.into(),
            ..Self::default()
        }
    }

    /// Picks the animation emoji.
    #[must_use]
    pub fn with_emoji(mut self, emoji: impl Into<String>) -> Self {
        self.emoji = Some(emoji.into());
        self
    }

    /// Send without a notification sound.
    #[must_use]
    pub fn silent(mut self) -> Self {
        self.delivery = self.delivery.silent();
        self
    }

    /// Checks chat and emoji.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an unusable chat or an empty emoji.
    pub fn validate(&self) -> Result<()> {
        validate_chat("chat_id", &self.chat_id)?;
        if let Some(emoji) = self.emoji.as_deref() {
            if emoji.trim().is_empty() {
                return Err(Error::InvalidRequest {
                    field: "emoji",
                    reason: "must not be empty".to_string(),
                });
            }
        }
        Ok(())
    }
}
