//! Media objects carried by poll questions, explanations and options.
//!
//! The Bot API models these as small, independent objects. [`PollMedia`] is not
//! a tagged union: it is a flat object whose ten optional member fields may be
//! present, and Telegram documents that at most one is present per object.
//! Every type here ignores unknown fields, so a newer Bot API cannot break
//! parsing.
//!
//! Numeric members are non-negative by construction: sizes are `u64`, pixel
//! dimensions are `u32`, durations are `u64`. A payload that carries a negative
//! number is refused while parsing instead of being clamped into a
//! plausible-looking value, so a malformed object can never masquerade as a
//! valid one. The rejection names the member and reaches the caller as
//! [`crate::Error::Payload`], the same typed error every undecodable body
//! produces.
//!
//! ```
//! use telegram_api_client::PhotoSize;
//!
//! let photo: PhotoSize = serde_json::from_str(r#"{"width":320,"height":240}"#).unwrap();
//! assert_eq!(photo.width, 320);
//!
//! // `-1` is not repaired to `0`: nothing but a real count parses.
//! assert!(serde_json::from_str::<PhotoSize>(r#"{"width":-1}"#).is_err());
//! assert!(serde_json::from_str::<PhotoSize>(r#"{"file_size":-1}"#).is_err());
//! ```

use serde::{Deserialize, Serialize};

use crate::events::Location;
use crate::file::File;

/// Why a numeric media member was refused while parsing.
///
/// The Bot API documents every size, dimension and duration as a non-negative
/// integer. A negative number is therefore malformed input, not a value to
/// repair: clamping `-1` to `0` would hand the caller a photo of no size and
/// hide the defect. Deserialization fails instead, and the field name says which
/// member was wrong.
///
/// The enum is [`non_exhaustive`](https://doc.rust-lang.org/reference/attributes/type_system.html#the-non_exhaustive-attribute):
/// match with a `_` arm so new variants cannot break you.
///
/// ```
/// use telegram_api_client::{MediaFieldError, PhotoSize, Sticker};
///
/// // The deserializer reports the member and the reason, it does not clamp.
/// let error = serde_json::from_str::<Sticker>(r#"{"width":-1}"#)
///     .expect_err("a negative width is rejected");
/// assert!(error.to_string().contains("`width`"));
/// assert!(error.to_string().contains("must not be negative"));
///
/// // Zero and the type maximum are boundaries, not errors.
/// let narrow: PhotoSize = serde_json::from_str(r#"{"width":0,"height":0}"#)
///     .expect("zero is a valid dimension");
/// assert_eq!(narrow.width, 0);
///
/// // The type itself is public, so a caller can reject a value before parsing.
/// let rejected = MediaFieldError::Negative { field: "file_size", value: -3 };
/// assert_eq!(
///     rejected.to_string(),
///     "media field `file_size` must not be negative, got -3"
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum MediaFieldError {
    /// The member carried a negative number.
    #[error("media field `{field}` must not be negative, got {value}")]
    Negative {
        /// Wire name of the offending member.
        field: &'static str,
        /// The rejected number.
        value: i128,
    },
    /// The member carried a number that does not fit the member's type.
    #[error("media field `{field}` is out of range, got {value}")]
    OutOfRange {
        /// Wire name of the offending member.
        field: &'static str,
        /// The rejected number.
        value: u128,
    },
}

/// Narrows a parsed integer to a non-negative field type.
///
/// Negative input is [`MediaFieldError::Negative`], too large input is
/// [`MediaFieldError::OutOfRange`]; nothing is clamped.
fn non_negative<T>(raw: i128, field: &'static str) -> Result<T, MediaFieldError>
where
    T: TryFrom<u128>,
{
    let value = u128::try_from(raw).map_err(|_| MediaFieldError::Negative { field, value: raw })?;
    T::try_from(value).map_err(|_| MediaFieldError::OutOfRange { field, value })
}

/// Deserializes a pixel dimension into `u32`.
///
/// A negative number is refused with [`MediaFieldError::Negative`], a number above
/// `u32::MAX` with [`MediaFieldError::OutOfRange`]; the bound is inclusive, so `0`
/// and `u32::MAX` both pass.
fn deserialize_width<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = i128::deserialize(deserializer)?;
    non_negative(raw, "width").map_err(serde::de::Error::custom)
}

/// Deserializes a pixel dimension into `u32`, see [`deserialize_width`].
fn deserialize_height<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = i128::deserialize(deserializer)?;
    non_negative(raw, "height").map_err(serde::de::Error::custom)
}

/// Deserializes a duration in seconds into `u64`.
///
/// The bound is inclusive: `0` and `u64::MAX` pass, a negative number does not.
fn deserialize_duration<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = i128::deserialize(deserializer)?;
    non_negative(raw, "duration").map_err(serde::de::Error::custom)
}

/// Deserializes an optional byte count into `Option<u64>`.
///
/// An absent member and an explicit `null` both stay `None`; a present number is
/// checked like [`deserialize_duration`].
fn deserialize_file_size<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Option::<i128>::deserialize(deserializer)? {
        Some(raw) => non_negative(raw, "file_size")
            .map(Some)
            .map_err(serde::de::Error::custom),
        None => Ok(None),
    }
}

/// Deserializes an optional cover timestamp in seconds into `Option<u64>`.
fn deserialize_start_timestamp<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Option::<i128>::deserialize(deserializer)? {
        Some(raw) => non_negative(raw, "start_timestamp")
            .map(Some)
            .map_err(serde::de::Error::custom),
        None => Ok(None),
    }
}

/// One size of a photo or thumbnail.
///
/// `width` and `height` are `u32`, `file_size` is `u64`: a negative number is
/// refused while parsing, never clamped.
///
/// ```
/// use telegram_api_client::PhotoSize;
///
/// let photo: PhotoSize = serde_json::from_str(
///     r#"{"file_id":"AgAC","file_unique_id":"AQAD","width":320,"height":240,"file_size":12345}"#,
/// ).unwrap();
/// assert_eq!(photo.width, 320);
/// assert_eq!(photo.file_size, Some(12345));
///
/// // The boundaries: zero and the type maximum parse, `-1` does not.
/// let empty: PhotoSize = serde_json::from_str(r#"{"width":0,"height":0,"file_size":0}"#).unwrap();
/// assert_eq!(empty.file_size, Some(0));
///
/// let huge: PhotoSize = serde_json::from_str(
///     r#"{"width":4294967295,"height":4294967295,"file_size":18446744073709551615}"#,
/// ).unwrap();
/// assert_eq!(huge.width, u32::MAX);
/// assert_eq!(huge.file_size, Some(u64::MAX));
///
/// assert!(serde_json::from_str::<PhotoSize>(r#"{"width":-1}"#).is_err());
/// assert!(serde_json::from_str::<PhotoSize>(r#"{"file_size":-1}"#).is_err());
/// // A number that does not fit the member's type is an error too.
/// assert!(serde_json::from_str::<PhotoSize>(r#"{"width":4294967296}"#).is_err());
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct PhotoSize {
    /// Identifier usable to re-send the photo.
    #[serde(default, alias = "fileId")]
    pub file_id: String,
    /// Identifier stable across bots.
    #[serde(default, alias = "fileUniqueId")]
    pub file_unique_id: String,
    /// Photo width in pixels; negative input is rejected.
    #[serde(default, deserialize_with = "deserialize_width")]
    pub width: u32,
    /// Photo height in pixels; negative input is rejected.
    #[serde(default, deserialize_with = "deserialize_height")]
    pub height: u32,
    /// Photo size in bytes, when known; negative input is rejected.
    #[serde(
        default,
        deserialize_with = "deserialize_file_size",
        alias = "fileSize",
        skip_serializing_if = "Option::is_none"
    )]
    pub file_size: Option<u64>,
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
/// Dimensions are `u32`, duration and size are `u64`; negative input is
/// refused while parsing.
///
/// ```
/// use telegram_api_client::LivePhoto;
///
/// let live_photo: LivePhoto = serde_json::from_str(
///     r#"{"file_id":"A","file_unique_id":"u","width":1080,"height":1920,"duration":3}"#,
/// ).unwrap();
/// assert_eq!(live_photo.duration, 3);
///
/// // Boundaries: zero and the maximum parse, a negative number does not.
/// let still: LivePhoto = serde_json::from_str(
///     r#"{"width":0,"height":0,"duration":0,"file_size":18446744073709551615}"#,
/// ).unwrap();
/// assert_eq!(still.file_size, Some(u64::MAX));
///
/// assert!(serde_json::from_str::<LivePhoto>(r#"{"width":-1}"#).is_err());
/// assert!(serde_json::from_str::<LivePhoto>(r#"{"duration":-1}"#).is_err());
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
    /// Video width in pixels; negative input is rejected.
    #[serde(default, deserialize_with = "deserialize_width")]
    pub width: u32,
    /// Video height in pixels; negative input is rejected.
    #[serde(default, deserialize_with = "deserialize_height")]
    pub height: u32,
    /// Duration in seconds; negative input is rejected.
    #[serde(default, deserialize_with = "deserialize_duration")]
    pub duration: u64,
    /// MIME type of the video, when known.
    #[serde(default, alias = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// File size in bytes, when known; negative input is rejected.
    #[serde(
        default,
        deserialize_with = "deserialize_file_size",
        alias = "fileSize",
        skip_serializing_if = "Option::is_none"
    )]
    pub file_size: Option<u64>,
}

/// An animation attached to a poll or poll option.
///
/// Dimensions are `u32`, duration and size are `u64`; negative input is
/// refused while parsing.
///
/// ```
/// use telegram_api_client::Animation;
///
/// let animation: Animation = serde_json::from_str(
///     r#"{"file_id":"A","file_unique_id":"u","width":320,"height":240,"duration":2,"mime_type":"video/mp4"}"#,
/// ).unwrap();
/// assert_eq!(animation.mime_type.as_deref(), Some("video/mp4"));
///
/// // Boundaries: zero and the maximum parse, a negative number does not.
/// let blank: Animation = serde_json::from_str(
///     r#"{"width":0,"height":0,"duration":0,"file_size":0}"#,
/// ).unwrap();
/// assert_eq!(blank.file_size, Some(0));
/// let widest: Animation = serde_json::from_str(r#"{"width":4294967295}"#).unwrap();
/// assert_eq!(widest.width, u32::MAX);
///
/// assert!(serde_json::from_str::<Animation>(r#"{"height":-1}"#).is_err());
/// assert!(serde_json::from_str::<Animation>(r#"{"file_size":-1}"#).is_err());
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Animation {
    /// Identifier usable to re-send the animation.
    #[serde(default, alias = "fileId")]
    pub file_id: String,
    /// Identifier stable across bots.
    #[serde(default, alias = "fileUniqueId")]
    pub file_unique_id: String,
    /// Animation width in pixels; negative input is rejected.
    #[serde(default, deserialize_with = "deserialize_width")]
    pub width: u32,
    /// Animation height in pixels; negative input is rejected.
    #[serde(default, deserialize_with = "deserialize_height")]
    pub height: u32,
    /// Duration in seconds; negative input is rejected.
    #[serde(default, deserialize_with = "deserialize_duration")]
    pub duration: u64,
    /// Thumbnail of the animation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<PhotoSize>,
    /// Original file name, when known.
    #[serde(default, alias = "fileName", skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    /// MIME type of the animation, when known.
    #[serde(default, alias = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// File size in bytes, when known; negative input is rejected.
    #[serde(
        default,
        deserialize_with = "deserialize_file_size",
        alias = "fileSize",
        skip_serializing_if = "Option::is_none"
    )]
    pub file_size: Option<u64>,
}

/// Audio attached to a poll or poll option.
///
/// `duration` is `u64`, `file_size` is `Option<u64>`; negative input is
/// refused while parsing.
///
/// ```
/// use telegram_api_client::Audio;
///
/// let audio: Audio = serde_json::from_str(
///     r#"{"file_id":"A","file_unique_id":"u","duration":9,"performer":"A","title":"T"}"#,
/// ).unwrap();
/// assert_eq!(audio.performer.as_deref(), Some("A"));
///
/// // Boundaries: zero and the maximum parse, a negative number does not.
/// let silent: Audio = serde_json::from_str(
///     r#"{"duration":0,"file_size":18446744073709551615}"#,
/// ).unwrap();
/// assert_eq!(silent.file_size, Some(u64::MAX));
///
/// assert!(serde_json::from_str::<Audio>(r#"{"duration":-1}"#).is_err());
/// assert!(serde_json::from_str::<Audio>(r#"{"file_size":-1}"#).is_err());
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Audio {
    /// Identifier usable to re-send the audio.
    #[serde(default, alias = "fileId")]
    pub file_id: String,
    /// Identifier stable across bots.
    #[serde(default, alias = "fileUniqueId")]
    pub file_unique_id: String,
    /// Duration in seconds; negative input is rejected.
    #[serde(default, deserialize_with = "deserialize_duration")]
    pub duration: u64,
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
    /// File size in bytes, when known; negative input is rejected.
    #[serde(
        default,
        deserialize_with = "deserialize_file_size",
        alias = "fileSize",
        skip_serializing_if = "Option::is_none"
    )]
    pub file_size: Option<u64>,
    /// Thumbnail of the audio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<PhotoSize>,
}

/// A document attached to a poll or poll option.
///
/// `file_size` is `Option<u64>`; a negative size is refused while parsing.
///
/// ```
/// use telegram_api_client::Document;
///
/// let document: Document = serde_json::from_str(
///     r#"{"file_id":"A","file_unique_id":"u","file_name":"a.pdf","mime_type":"application/pdf"}"#,
/// ).unwrap();
/// assert_eq!(document.file_name.as_deref(), Some("a.pdf"));
///
/// // Boundaries: an empty and an oversized file parse, a negative one does not.
/// let empty: Document = serde_json::from_str(r#"{"file_size":0}"#).unwrap();
/// assert_eq!(empty.file_size, Some(0));
/// let largest: Document =
///     serde_json::from_str(r#"{"file_size":18446744073709551615}"#).unwrap();
/// assert_eq!(largest.file_size, Some(u64::MAX));
///
/// assert!(serde_json::from_str::<Document>(r#"{"file_size":-1}"#).is_err());
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
    /// File size in bytes, when known; negative input is rejected.
    #[serde(
        default,
        deserialize_with = "deserialize_file_size",
        alias = "fileSize",
        skip_serializing_if = "Option::is_none"
    )]
    pub file_size: Option<u64>,
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
/// Dimensions are `u32`, `file_size` is `u64`; negative input is refused while
/// parsing.
///
/// ```
/// use telegram_api_client::VideoQuality;
///
/// let quality: VideoQuality = serde_json::from_str(
///     r#"{"file_id":"A","file_unique_id":"u","width":1280,"height":720,"codec":"h264"}"#,
/// ).unwrap();
/// assert_eq!(quality.codec, "h264");
///
/// // Boundaries: zero and the maximum parse, a negative number does not.
/// let smallest: VideoQuality =
///     serde_json::from_str(r#"{"width":0,"height":0,"file_size":0}"#).unwrap();
/// assert_eq!(smallest.width, 0);
/// let largest: VideoQuality =
///     serde_json::from_str(r#"{"width":4294967295,"height":4294967295}"#).unwrap();
/// assert_eq!(largest.height, u32::MAX);
///
/// assert!(serde_json::from_str::<VideoQuality>(r#"{"width":-1}"#).is_err());
/// assert!(serde_json::from_str::<VideoQuality>(r#"{"file_size":-1}"#).is_err());
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct VideoQuality {
    /// Identifier usable to re-send this quality.
    #[serde(default, alias = "fileId")]
    pub file_id: String,
    /// Identifier stable across bots.
    #[serde(default, alias = "fileUniqueId")]
    pub file_unique_id: String,
    /// Video width in pixels; negative input is rejected.
    #[serde(default, deserialize_with = "deserialize_width")]
    pub width: u32,
    /// Video height in pixels; negative input is rejected.
    #[serde(default, deserialize_with = "deserialize_height")]
    pub height: u32,
    /// Video codec.
    #[serde(default)]
    pub codec: String,
    /// File size in bytes, when known; negative input is rejected.
    #[serde(
        default,
        deserialize_with = "deserialize_file_size",
        alias = "fileSize",
        skip_serializing_if = "Option::is_none"
    )]
    pub file_size: Option<u64>,
}

/// A video attached to a poll or poll option.
///
/// Dimensions are `u32`; duration, `start_timestamp` and `file_size` are `u64`.
/// Negative input is refused while parsing.
///
/// ```
/// use telegram_api_client::Video;
///
/// let video: Video = serde_json::from_str(
///     r#"{"file_id":"A","file_unique_id":"u","width":1280,"height":720,"duration":12}"#,
/// ).unwrap();
/// assert_eq!(video.duration, 12);
///
/// // Boundaries: zero and the maximum parse, a negative number does not.
/// let smallest: Video = serde_json::from_str(
///     r#"{"width":0,"height":0,"duration":0,"start_timestamp":0,"file_size":0}"#,
/// ).unwrap();
/// assert_eq!(smallest.start_timestamp, Some(0));
/// let longest: Video =
///     serde_json::from_str(r#"{"duration":18446744073709551615}"#).unwrap();
/// assert_eq!(longest.duration, u64::MAX);
///
/// assert!(serde_json::from_str::<Video>(r#"{"width":-1}"#).is_err());
/// assert!(serde_json::from_str::<Video>(r#"{"start_timestamp":-1}"#).is_err());
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Video {
    /// Identifier usable to re-send the video.
    #[serde(default, alias = "fileId")]
    pub file_id: String,
    /// Identifier stable across bots.
    #[serde(default, alias = "fileUniqueId")]
    pub file_unique_id: String,
    /// Video width in pixels; negative input is rejected.
    #[serde(default, deserialize_with = "deserialize_width")]
    pub width: u32,
    /// Video height in pixels; negative input is rejected.
    #[serde(default, deserialize_with = "deserialize_height")]
    pub height: u32,
    /// Duration in seconds; negative input is rejected.
    #[serde(default, deserialize_with = "deserialize_duration")]
    pub duration: u64,
    /// Thumbnail of the video.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<PhotoSize>,
    /// Covers available for the video.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<Vec<PhotoSize>>,
    /// Timestamp of the cover in the video, when known; negative input is
    /// rejected.
    #[serde(
        default,
        deserialize_with = "deserialize_start_timestamp",
        alias = "startTimestamp",
        skip_serializing_if = "Option::is_none"
    )]
    pub start_timestamp: Option<u64>,
    /// Available encoded qualities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qualities: Option<Vec<VideoQuality>>,
    /// Original file name, when known.
    #[serde(default, alias = "fileName", skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    /// MIME type of the video, when known.
    #[serde(default, alias = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// File size in bytes, when known; negative input is rejected.
    #[serde(
        default,
        deserialize_with = "deserialize_file_size",
        alias = "fileSize",
        skip_serializing_if = "Option::is_none"
    )]
    pub file_size: Option<u64>,
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
/// Dimensions are `u32`, `file_size` is `u64`; negative input is refused while
/// parsing.
///
/// ```
/// use telegram_api_client::Sticker;
///
/// let sticker: Sticker = serde_json::from_str(
///     r#"{"file_id":"CAAC","file_unique_id":"AQAD","type":"regular","width":512,"height":512,"is_animated":false,"is_video":false}"#,
/// ).unwrap();
/// assert_eq!(sticker.sticker_type, "regular");
///
/// // Boundaries: zero and the maximum parse, a negative number does not.
/// let empty: Sticker =
///     serde_json::from_str(r#"{"width":0,"height":0,"file_size":0}"#).unwrap();
/// assert_eq!(empty.file_size, Some(0));
/// let biggest: Sticker =
///     serde_json::from_str(r#"{"width":4294967295,"height":4294967295}"#).unwrap();
/// assert_eq!(biggest.width, u32::MAX);
///
/// assert!(serde_json::from_str::<Sticker>(r#"{"height":-1}"#).is_err());
/// assert!(serde_json::from_str::<Sticker>(r#"{"file_size":-1}"#).is_err());
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Sticker {
    /// Identifier usable to re-send the sticker.
    #[serde(default, alias = "fileId")]
    pub file_id: String,
    /// Identifier stable across bots.
    #[serde(default, alias = "fileUniqueId")]
    pub file_unique_id: String,
    /// Sticker width in pixels; negative input is rejected.
    #[serde(default, deserialize_with = "deserialize_width")]
    pub width: u32,
    /// Sticker height in pixels; negative input is rejected.
    #[serde(default, deserialize_with = "deserialize_height")]
    pub height: u32,
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
    /// File size in bytes, when known; negative input is rejected.
    #[serde(
        default,
        deserialize_with = "deserialize_file_size",
        alias = "fileSize",
        skip_serializing_if = "Option::is_none"
    )]
    pub file_size: Option<u64>,
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
