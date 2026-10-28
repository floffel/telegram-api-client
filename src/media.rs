//! Media objects carried by poll questions, explanations and options.
//!
//! The Bot API models these as small, independent objects. [`PollMedia`] is not
//! a tagged union: it is a flat object whose ten optional member fields may be
//! present, and Telegram documents that at most one is present per object.
//! Every type here ignores unknown fields, so a newer Bot API cannot break
//! parsing.

use serde::{Deserialize, Serialize};

use crate::events::Location;
use crate::file::File;

/// One size of a photo or thumbnail.
///
/// ```
/// use telegram_api_client::PhotoSize;
///
/// let photo: PhotoSize = serde_json::from_str(
///     r#"{"file_id":"AgAC","file_unique_id":"AQAD","width":320,"height":240,"file_size":12345}"#,
/// ).unwrap();
/// assert_eq!(photo.width, 320);
/// assert_eq!(photo.file_size, Some(12345));
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct PhotoSize {
    /// Identifier usable to re-send the photo.
    #[serde(default, alias = "fileId")]
    pub file_id: String,
    /// Identifier stable across bots.
    #[serde(default, alias = "fileUniqueId")]
    pub file_unique_id: String,
    /// Photo width in pixels.
    #[serde(default)]
    pub width: i64,
    /// Photo height in pixels.
    #[serde(default)]
    pub height: i64,
    /// Photo size in bytes, when known.
    #[serde(default, alias = "fileSize", skip_serializing_if = "Option::is_none")]
    pub file_size: Option<i64>,
}

/// An HTTP link attached to a poll or poll option.
///
/// ```
/// use telegram_api_client::Link;
///
/// let link: Link = serde_json::from_str(r#"{"url":"https://example.com/x"}"#).unwrap();
/// assert_eq!(link.url, "https://example.com/x");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Link {
    /// URL of the link.
    #[serde(default)]
    pub url: String,
}

/// A live photo attached to a poll or poll option.
///
/// ```
/// use telegram_api_client::LivePhoto;
///
/// let live_photo: LivePhoto = serde_json::from_str(
///     r#"{"file_id":"A","file_unique_id":"u","width":1080,"height":1920,"duration":3}"#,
/// ).unwrap();
/// assert_eq!(live_photo.duration, 3);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct LivePhoto {
    /// Photo part of the live photo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub photo: Option<Vec<PhotoSize>>,
    /// Identifier usable to re-send the live photo.
    #[serde(default, alias = "fileId")]
    pub file_id: String,
    /// Identifier stable across bots.
    #[serde(default, alias = "fileUniqueId")]
    pub file_unique_id: String,
    /// Video width in pixels.
    #[serde(default)]
    pub width: i64,
    /// Video height in pixels.
    #[serde(default)]
    pub height: i64,
    /// Duration in seconds.
    #[serde(default)]
    pub duration: i64,
    /// MIME type of the video, when known.
    #[serde(default, alias = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// File size in bytes, when known.
    #[serde(default, alias = "fileSize", skip_serializing_if = "Option::is_none")]
    pub file_size: Option<i64>,
}

/// An animation attached to a poll or poll option.
///
/// ```
/// use telegram_api_client::Animation;
///
/// let animation: Animation = serde_json::from_str(
///     r#"{"file_id":"A","file_unique_id":"u","width":320,"height":240,"duration":2,"mime_type":"video/mp4"}"#,
/// ).unwrap();
/// assert_eq!(animation.mime_type.as_deref(), Some("video/mp4"));
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Animation {
    /// Identifier usable to re-send the animation.
    #[serde(default, alias = "fileId")]
    pub file_id: String,
    /// Identifier stable across bots.
    #[serde(default, alias = "fileUniqueId")]
    pub file_unique_id: String,
    /// Animation width in pixels.
    #[serde(default)]
    pub width: i64,
    /// Animation height in pixels.
    #[serde(default)]
    pub height: i64,
    /// Duration in seconds.
    #[serde(default)]
    pub duration: i64,
    /// Thumbnail of the animation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<PhotoSize>,
    /// Original file name, when known.
    #[serde(default, alias = "fileName", skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    /// MIME type of the animation, when known.
    #[serde(default, alias = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// File size in bytes, when known.
    #[serde(default, alias = "fileSize", skip_serializing_if = "Option::is_none")]
    pub file_size: Option<i64>,
}

/// Audio attached to a poll or poll option.
///
/// ```
/// use telegram_api_client::Audio;
///
/// let audio: Audio = serde_json::from_str(
///     r#"{"file_id":"A","file_unique_id":"u","duration":9,"performer":"A","title":"T"}"#,
/// ).unwrap();
/// assert_eq!(audio.performer.as_deref(), Some("A"));
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Audio {
    /// Identifier usable to re-send the audio.
    #[serde(default, alias = "fileId")]
    pub file_id: String,
    /// Identifier stable across bots.
    #[serde(default, alias = "fileUniqueId")]
    pub file_unique_id: String,
    /// Duration in seconds.
    #[serde(default)]
    pub duration: i64,
    /// Performer, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub performer: Option<String>,
    /// Track title, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Original file name, when known.
    #[serde(default, alias = "fileName", skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    /// MIME type of the audio, when known.
    #[serde(default, alias = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// File size in bytes, when known.
    #[serde(default, alias = "fileSize", skip_serializing_if = "Option::is_none")]
    pub file_size: Option<i64>,
    /// Thumbnail of the audio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<PhotoSize>,
}

/// A document attached to a poll or poll option.
///
/// ```
/// use telegram_api_client::Document;
///
/// let document: Document = serde_json::from_str(
///     r#"{"file_id":"A","file_unique_id":"u","file_name":"a.pdf","mime_type":"application/pdf"}"#,
/// ).unwrap();
/// assert_eq!(document.file_name.as_deref(), Some("a.pdf"));
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Document {
    /// Identifier usable to re-send the document.
    #[serde(default, alias = "fileId")]
    pub file_id: String,
    /// Identifier stable across bots.
    #[serde(default, alias = "fileUniqueId")]
    pub file_unique_id: String,
    /// Thumbnail of the document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<PhotoSize>,
    /// Original file name, when known.
    #[serde(default, alias = "fileName", skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    /// MIME type of the document, when known.
    #[serde(default, alias = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// File size in bytes, when known.
    #[serde(default, alias = "fileSize", skip_serializing_if = "Option::is_none")]
    pub file_size: Option<i64>,
}

/// A venue attached to a poll or poll option.
///
/// ```
/// use telegram_api_client::Venue;
///
/// let venue: Venue = serde_json::from_str(
///     r#"{"location":{"latitude":51.05,"longitude":13.74},"title":"PATO","address":"Koeniginstraße 34"}"#,
/// ).unwrap();
/// assert_eq!(venue.title, "PATO");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Venue {
    /// Geographical location of the venue.
    #[serde(default)]
    pub location: Location,
    /// Venue name.
    #[serde(default)]
    pub title: String,
    /// Venue address.
    #[serde(default)]
    pub address: String,
    /// Foursquare identifier, when known.
    #[serde(
        default,
        alias = "foursquareId",
        skip_serializing_if = "Option::is_none"
    )]
    pub foursquare_id: Option<String>,
    /// Foursquare type, when known.
    #[serde(
        default,
        alias = "foursquareType",
        skip_serializing_if = "Option::is_none"
    )]
    pub foursquare_type: Option<String>,
    /// Google Places identifier, when known.
    #[serde(
        default,
        alias = "googlePlaceId",
        skip_serializing_if = "Option::is_none"
    )]
    pub google_place_id: Option<String>,
    /// Google Places type, when known.
    #[serde(
        default,
        alias = "googlePlaceType",
        skip_serializing_if = "Option::is_none"
    )]
    pub google_place_type: Option<String>,
}

/// One encoded quality of a video.
///
/// ```
/// use telegram_api_client::VideoQuality;
///
/// let quality: VideoQuality = serde_json::from_str(
///     r#"{"file_id":"A","file_unique_id":"u","width":1280,"height":720,"codec":"h264"}"#,
/// ).unwrap();
/// assert_eq!(quality.codec, "h264");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct VideoQuality {
    /// Identifier usable to re-send this quality.
    #[serde(default, alias = "fileId")]
    pub file_id: String,
    /// Identifier stable across bots.
    #[serde(default, alias = "fileUniqueId")]
    pub file_unique_id: String,
    /// Video width in pixels.
    #[serde(default)]
    pub width: i64,
    /// Video height in pixels.
    #[serde(default)]
    pub height: i64,
    /// Video codec.
    #[serde(default)]
    pub codec: String,
    /// File size in bytes, when known.
    #[serde(default, alias = "fileSize", skip_serializing_if = "Option::is_none")]
    pub file_size: Option<i64>,
}

/// A video attached to a poll or poll option.
///
/// ```
/// use telegram_api_client::Video;
///
/// let video: Video = serde_json::from_str(
///     r#"{"file_id":"A","file_unique_id":"u","width":1280,"height":720,"duration":12}"#,
/// ).unwrap();
/// assert_eq!(video.duration, 12);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Video {
    /// Identifier usable to re-send the video.
    #[serde(default, alias = "fileId")]
    pub file_id: String,
    /// Identifier stable across bots.
    #[serde(default, alias = "fileUniqueId")]
    pub file_unique_id: String,
    /// Video width in pixels.
    #[serde(default)]
    pub width: i64,
    /// Video height in pixels.
    #[serde(default)]
    pub height: i64,
    /// Duration in seconds.
    #[serde(default)]
    pub duration: i64,
    /// Thumbnail of the video.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<PhotoSize>,
    /// Covers available for the video.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<Vec<PhotoSize>>,
    /// Timestamp of the cover in the video, when known.
    #[serde(
        default,
        alias = "startTimestamp",
        skip_serializing_if = "Option::is_none"
    )]
    pub start_timestamp: Option<i64>,
    /// Available encoded qualities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qualities: Option<Vec<VideoQuality>>,
    /// Original file name, when known.
    #[serde(default, alias = "fileName", skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    /// MIME type of the video, when known.
    #[serde(default, alias = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// File size in bytes, when known.
    #[serde(default, alias = "fileSize", skip_serializing_if = "Option::is_none")]
    pub file_size: Option<i64>,
}

/// Where a mask is positioned on a sticker face.
///
/// ```
/// use telegram_api_client::MaskPosition;
///
/// let position: MaskPosition = serde_json::from_str(
///     r#"{"point":"eyes","x_shift":0.1,"y_shift":-0.2,"scale":1.5}"#,
/// ).unwrap();
/// assert_eq!(position.point, "eyes");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct MaskPosition {
    /// One of `forehead`, `eyes`, `mouth` or `chin`.
    #[serde(default)]
    pub point: String,
    /// Horizontal shift from the mask point.
    #[serde(default, alias = "xShift")]
    pub x_shift: f64,
    /// Vertical shift from the mask point.
    #[serde(default, alias = "yShift")]
    pub y_shift: f64,
    /// Scale of the mask.
    #[serde(default)]
    pub scale: f64,
}

/// Compares the raw bits of the floating-point fields, so the type stays
/// reflexive even if a payload carries `NaN`, matching [`Location`].
impl PartialEq for MaskPosition {
    fn eq(&self, other: &Self) -> bool {
        self.point == other.point
            && self.x_shift.to_bits() == other.x_shift.to_bits()
            && self.y_shift.to_bits() == other.y_shift.to_bits()
            && self.scale.to_bits() == other.scale.to_bits()
    }
}

impl Eq for MaskPosition {}

/// A sticker attached to a poll or poll option.
///
/// ```
/// use telegram_api_client::Sticker;
///
/// let sticker: Sticker = serde_json::from_str(
///     r#"{"file_id":"CAAC","file_unique_id":"AQAD","type":"regular","width":512,"height":512,"is_animated":false,"is_video":false}"#,
/// ).unwrap();
/// assert_eq!(sticker.sticker_type, "regular");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Sticker {
    /// Identifier usable to re-send the sticker.
    #[serde(default, alias = "fileId")]
    pub file_id: String,
    /// Identifier stable across bots.
    #[serde(default, alias = "fileUniqueId")]
    pub file_unique_id: String,
    /// Sticker width in pixels.
    #[serde(default)]
    pub width: i64,
    /// Sticker height in pixels.
    #[serde(default)]
    pub height: i64,
    /// Whether the sticker is animated.
    #[serde(default, alias = "isAnimated")]
    pub is_animated: bool,
    /// Whether the sticker is a video.
    #[serde(default, alias = "isVideo")]
    pub is_video: bool,
    /// Sticker type, serialized as `type` on the wire.
    #[serde(default, rename = "type", alias = "stickerType")]
    pub sticker_type: String,
    /// Thumbnail of the sticker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<PhotoSize>,
    /// Emoji associated with the sticker, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<String>,
    /// Name of the sticker set, when known.
    #[serde(default, alias = "setName", skip_serializing_if = "Option::is_none")]
    pub set_name: Option<String>,
    /// Premium animation of the sticker, when known.
    #[serde(
        default,
        alias = "premiumAnimation",
        skip_serializing_if = "Option::is_none"
    )]
    pub premium_animation: Option<File>,
    /// Mask position, for mask stickers.
    #[serde(
        default,
        alias = "maskPosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub mask_position: Option<MaskPosition>,
    /// Custom emoji identifier, for custom emoji stickers.
    #[serde(
        default,
        alias = "customEmojiId",
        skip_serializing_if = "Option::is_none"
    )]
    pub custom_emoji_id: Option<String>,
    /// Whether the sticker needs repainting, when known.
    #[serde(
        default,
        alias = "needsRepainting",
        skip_serializing_if = "Option::is_none"
    )]
    pub needs_repainting: Option<bool>,
    /// File size in bytes, when known.
    #[serde(default, alias = "fileSize", skip_serializing_if = "Option::is_none")]
    pub file_size: Option<i64>,
}

/// The media attached to a poll question, explanation or option.
///
/// The Bot API documents that at most one member field is present in any given
/// object, but this is a flat object rather than a tagged union.
///
/// ```
/// use telegram_api_client::{PollMedia, PollMediaKind};
///
/// let media: PollMedia = serde_json::from_str(r#"{"link":{"url":"https://example.com/x"}}"#).unwrap();
/// assert_eq!(media.kind(), Some(PollMediaKind::Link));
/// assert!(!media.is_empty());
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct PollMedia {
    /// Animation member.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<Animation>,
    /// Audio member.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<Audio>,
    /// Document member.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document: Option<Document>,
    /// Link member.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<Link>,
    /// Live photo member.
    #[serde(default, alias = "livePhoto", skip_serializing_if = "Option::is_none")]
    pub live_photo: Option<LivePhoto>,
    /// Location member.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,
    /// Photo member.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub photo: Option<Vec<PhotoSize>>,
    /// Sticker member.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sticker: Option<Sticker>,
    /// Venue member.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub venue: Option<Venue>,
    /// Video member.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video: Option<Video>,
}

impl PollMedia {
    /// Whether all ten member fields are absent.
    ///
    /// ```
    /// use telegram_api_client::PollMedia;
    ///
    /// assert!(PollMedia::default().is_empty());
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.animation.is_none()
            && self.audio.is_none()
            && self.document.is_none()
            && self.link.is_none()
            && self.live_photo.is_none()
            && self.location.is_none()
            && self.photo.is_none()
            && self.sticker.is_none()
            && self.venue.is_none()
            && self.video.is_none()
    }

    /// The first present member in documented field order.
    ///
    /// Telegram never documents more than one present member, but if a payload
    /// carries several this method deterministically returns the first one.
    ///
    /// ```
    /// use telegram_api_client::{PollMedia, PollMediaKind};
    ///
    /// let media: PollMedia = serde_json::from_str(r#"{"video":{}}"#).unwrap();
    /// assert_eq!(media.kind(), Some(PollMediaKind::Video));
    /// ```
    #[must_use]
    pub fn kind(&self) -> Option<PollMediaKind> {
        if self.animation.is_some() {
            Some(PollMediaKind::Animation)
        } else if self.audio.is_some() {
            Some(PollMediaKind::Audio)
        } else if self.document.is_some() {
            Some(PollMediaKind::Document)
        } else if self.link.is_some() {
            Some(PollMediaKind::Link)
        } else if self.live_photo.is_some() {
            Some(PollMediaKind::LivePhoto)
        } else if self.location.is_some() {
            Some(PollMediaKind::Location)
        } else if self.photo.is_some() {
            Some(PollMediaKind::Photo)
        } else if self.sticker.is_some() {
            Some(PollMediaKind::Sticker)
        } else if self.venue.is_some() {
            Some(PollMediaKind::Venue)
        } else if self.video.is_some() {
            Some(PollMediaKind::Video)
        } else {
            None
        }
    }
}

/// The kind of media carried by a [`PollMedia`] object.
///
/// ```
/// use telegram_api_client::PollMediaKind;
///
/// assert_eq!(PollMediaKind::Photo, PollMediaKind::Photo);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PollMediaKind {
    /// The `animation` member.
    Animation,
    /// The `audio` member.
    Audio,
    /// The `document` member.
    Document,
    /// The `link` member.
    Link,
    /// The `live_photo` member.
    LivePhoto,
    /// The `location` member.
    Location,
    /// The `photo` member.
    Photo,
    /// The `sticker` member.
    Sticker,
    /// The `venue` member.
    Venue,
    /// The `video` member.
    Video,
}
