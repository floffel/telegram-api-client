//! Media objects carried by poll questions, explanations and options.
//!
//! The Bot API models these as small, independent objects. [`PollMedia`] is a
//! flat object with ten documented member fields, and Telegram documents that
//! at most one is present per object. It is represented as an explicit one-of
//! value: known members are typed, unknown members and adjacent fields are
//! preserved for a lossless round trip.
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
//! let photo: PhotoSize = serde_json::from_str(
//!     r#"{"file_id":"A","file_unique_id":"u","width":320,"height":240}"#,
//! ).unwrap();
//! assert_eq!(photo.width, 320);
//!
//! // `-1` is not repaired to `0`: nothing but a real count parses.
//! assert!(serde_json::from_str::<PhotoSize>(r#"{"width":-1}"#).is_err());
//! assert!(serde_json::from_str::<PhotoSize>(r#"{"file_size":-1}"#).is_err());
//! ```

use std::fmt;

use serde::de::{MapAccess, SeqAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::events::Location;
use crate::file::File;

/// Upper bound for arrays nested inside a poll media object.
///
/// Telegram does not publish a numeric maximum for these arrays. This local
/// bound keeps a hostile webhook body from allocating an arbitrary number of
/// elements while still leaving ample room for every documented media shape.
pub const MAX_MEDIA_COLLECTION_ITEMS: usize = 100;

struct BoundedVec<T>(Vec<T>);

struct BoundedVecVisitor<T>(std::marker::PhantomData<T>);

impl<'de, T> Visitor<'de> for BoundedVecVisitor<T>
where
    T: Deserialize<'de>,
{
    type Value = BoundedVec<T>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "an array with at most {MAX_MEDIA_COLLECTION_ITEMS} items"
        )
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        // A size hint is attacker-controlled. Never reserve more than the
        // accepted maximum, and enforce the maximum before growth.
        let capacity = sequence
            .size_hint()
            .unwrap_or(0)
            .min(MAX_MEDIA_COLLECTION_ITEMS);
        let mut values = Vec::with_capacity(capacity);
        while let Some(value) = sequence.next_element()? {
            if values.len() == MAX_MEDIA_COLLECTION_ITEMS {
                return Err(serde::de::Error::custom(format!(
                    "media array exceeds the limit of {MAX_MEDIA_COLLECTION_ITEMS} items"
                )));
            }
            values.push(value);
        }
        Ok(BoundedVec(values))
    }
}

impl<'de, T> Deserialize<'de> for BoundedVec<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(BoundedVecVisitor(std::marker::PhantomData))
    }
}

struct OptionalBoundedVecVisitor<T>(std::marker::PhantomData<T>);

impl<'de, T> Visitor<'de> for OptionalBoundedVecVisitor<T>
where
    T: Deserialize<'de>,
{
    type Value = Option<Vec<T>>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "null or an array with at most {MAX_MEDIA_COLLECTION_ITEMS} items"
        )
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(None)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(None)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        BoundedVec::<T>::deserialize(deserializer).map(|values| Some(values.0))
    }
}

fn deserialize_optional_bounded_vec<'de, D, T>(deserializer: D) -> Result<Option<Vec<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    deserializer.deserialize_option(OptionalBoundedVecVisitor(std::marker::PhantomData))
}

/// Why a numeric media member was refused while parsing.
///
/// The Bot API documents every size, dimension and duration as a non-negative
/// integer. A negative number is therefore malformed input, not a value to
/// repair: clamping `-1` to `0` would hand the caller a photo of no size and
/// hide the defect. Deserialization fails instead, and the field name says which
/// member was wrong.
///
/// Deliberately private. No public path returns this type: [`non_negative`]
/// renders it into the deserializer, so a caller sees the member name in
/// [`crate::Error::Payload`] — the same typed error every undecodable body
/// produces — and not a second, unused error surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
enum MediaFieldError {
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
/// A negative number is refused as "must not be negative", a number too large
/// for `T` as "is out of range"; both messages name `field`, and nothing is
/// clamped.
fn non_negative<T>(raw: i128, field: &'static str) -> Result<T, MediaFieldError>
where
    T: TryFrom<u128>,
{
    let value = u128::try_from(raw).map_err(|_| MediaFieldError::Negative { field, value: raw })?;
    T::try_from(value).map_err(|_| MediaFieldError::OutOfRange { field, value })
}

/// Deserializes a pixel dimension into `u32`.
///
/// A negative number and a number above `u32::MAX` are both refused; the bound
/// is inclusive, so `0` and `u32::MAX` pass.
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
/// let empty: PhotoSize = serde_json::from_str(
///     r#"{"file_id":"A","file_unique_id":"u","width":0,"height":0,"file_size":0}"#,
/// ).unwrap();
/// assert_eq!(empty.file_size, Some(0));
///
/// let huge: PhotoSize = serde_json::from_str(
///     r#"{"file_id":"A","file_unique_id":"u","width":4294967295,"height":4294967295,"file_size":18446744073709551615}"#,
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
#[non_exhaustive]
pub struct PhotoSize {
    /// Identifier usable to re-send the photo.
    #[serde(alias = "fileId")]
    pub file_id: String,
    /// Identifier stable across bots.
    #[serde(alias = "fileUniqueId")]
    pub file_unique_id: String,
    /// Photo width in pixels; negative input is rejected.
    #[serde(deserialize_with = "deserialize_width")]
    pub width: u32,
    /// Photo height in pixels; negative input is rejected.
    #[serde(deserialize_with = "deserialize_height")]
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
#[non_exhaustive]
pub struct Link {
    /// URL of the link.
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
///     r#"{"file_id":"A","file_unique_id":"u","width":0,"height":0,"duration":0,"file_size":18446744073709551615}"#,
/// ).unwrap();
/// assert_eq!(still.file_size, Some(u64::MAX));
///
/// assert!(serde_json::from_str::<LivePhoto>(r#"{"width":-1}"#).is_err());
/// assert!(serde_json::from_str::<LivePhoto>(r#"{"duration":-1}"#).is_err());
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct LivePhoto {
    /// Photo part of the live photo.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_bounded_vec",
        skip_serializing_if = "Option::is_none"
    )]
    pub photo: Option<Vec<PhotoSize>>,
    /// Identifier usable to re-send the live photo.
    #[serde(alias = "fileId")]
    pub file_id: String,
    /// Identifier stable across bots.
    #[serde(alias = "fileUniqueId")]
    pub file_unique_id: String,
    /// Video width in pixels; negative input is rejected.
    #[serde(deserialize_with = "deserialize_width")]
    pub width: u32,
    /// Video height in pixels; negative input is rejected.
    #[serde(deserialize_with = "deserialize_height")]
    pub height: u32,
    /// Duration in seconds; negative input is rejected.
    #[serde(deserialize_with = "deserialize_duration")]
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
///     r#"{"file_id":"A","file_unique_id":"u","width":0,"height":0,"duration":0,"file_size":0}"#,
/// ).unwrap();
/// assert_eq!(blank.file_size, Some(0));
/// let widest: Animation = serde_json::from_str(
///     r#"{"file_id":"A","file_unique_id":"u","width":4294967295,"height":0,"duration":0}"#,
/// ).unwrap();
/// assert_eq!(widest.width, u32::MAX);
///
/// assert!(serde_json::from_str::<Animation>(r#"{"height":-1}"#).is_err());
/// assert!(serde_json::from_str::<Animation>(r#"{"file_size":-1}"#).is_err());
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Animation {
    /// Identifier usable to re-send the animation.
    #[serde(alias = "fileId")]
    pub file_id: String,
    /// Identifier stable across bots.
    #[serde(alias = "fileUniqueId")]
    pub file_unique_id: String,
    /// Animation width in pixels; negative input is rejected.
    #[serde(deserialize_with = "deserialize_width")]
    pub width: u32,
    /// Animation height in pixels; negative input is rejected.
    #[serde(deserialize_with = "deserialize_height")]
    pub height: u32,
    /// Duration in seconds; negative input is rejected.
    #[serde(deserialize_with = "deserialize_duration")]
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
///     r#"{"file_id":"A","file_unique_id":"u","duration":0,"file_size":18446744073709551615}"#,
/// ).unwrap();
/// assert_eq!(silent.file_size, Some(u64::MAX));
///
/// assert!(serde_json::from_str::<Audio>(r#"{"duration":-1}"#).is_err());
/// assert!(serde_json::from_str::<Audio>(r#"{"file_size":-1}"#).is_err());
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Audio {
    /// Identifier usable to re-send the audio.
    #[serde(alias = "fileId")]
    pub file_id: String,
    /// Identifier stable across bots.
    #[serde(alias = "fileUniqueId")]
    pub file_unique_id: String,
    /// Duration in seconds; negative input is rejected.
    #[serde(deserialize_with = "deserialize_duration")]
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
/// let empty: Document =
///     serde_json::from_str(r#"{"file_id":"A","file_unique_id":"u","file_size":0}"#).unwrap();
/// assert_eq!(empty.file_size, Some(0));
/// let largest: Document =
///     serde_json::from_str(r#"{"file_id":"A","file_unique_id":"u","file_size":18446744073709551615}"#)
///         .unwrap();
/// assert_eq!(largest.file_size, Some(u64::MAX));
///
/// assert!(serde_json::from_str::<Document>(r#"{"file_size":-1}"#).is_err());
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Document {
    /// Identifier usable to re-send the document.
    #[serde(alias = "fileId")]
    pub file_id: String,
    /// Identifier stable across bots.
    #[serde(alias = "fileUniqueId")]
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
#[non_exhaustive]
pub struct Venue {
    /// Geographical location of the venue.
    pub location: Location,
    /// Venue name.
    pub title: String,
    /// Venue address.
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
///     serde_json::from_str(r#"{"file_id":"A","file_unique_id":"u","width":0,"height":0,"codec":"h264","file_size":0}"#)
///         .unwrap();
/// assert_eq!(smallest.width, 0);
/// let largest: VideoQuality =
///     serde_json::from_str(r#"{"file_id":"A","file_unique_id":"u","width":4294967295,"height":4294967295,"codec":"h264"}"#)
///         .unwrap();
/// assert_eq!(largest.height, u32::MAX);
///
/// assert!(serde_json::from_str::<VideoQuality>(r#"{"width":-1}"#).is_err());
/// assert!(serde_json::from_str::<VideoQuality>(r#"{"file_size":-1}"#).is_err());
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct VideoQuality {
    /// Identifier usable to re-send this quality.
    #[serde(alias = "fileId")]
    pub file_id: String,
    /// Identifier stable across bots.
    #[serde(alias = "fileUniqueId")]
    pub file_unique_id: String,
    /// Video width in pixels; negative input is rejected.
    #[serde(deserialize_with = "deserialize_width")]
    pub width: u32,
    /// Video height in pixels; negative input is rejected.
    #[serde(deserialize_with = "deserialize_height")]
    pub height: u32,
    /// Video codec.
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
///     r#"{"file_id":"A","file_unique_id":"u","width":0,"height":0,"duration":0,"start_timestamp":0,"file_size":0}"#,
/// ).unwrap();
/// assert_eq!(smallest.start_timestamp, Some(0));
/// let longest: Video =
///     serde_json::from_str(r#"{"file_id":"A","file_unique_id":"u","width":0,"height":0,"duration":18446744073709551615}"#)
///         .unwrap();
/// assert_eq!(longest.duration, u64::MAX);
///
/// assert!(serde_json::from_str::<Video>(r#"{"width":-1}"#).is_err());
/// assert!(serde_json::from_str::<Video>(r#"{"start_timestamp":-1}"#).is_err());
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Video {
    /// Identifier usable to re-send the video.
    #[serde(alias = "fileId")]
    pub file_id: String,
    /// Identifier stable across bots.
    #[serde(alias = "fileUniqueId")]
    pub file_unique_id: String,
    /// Video width in pixels; negative input is rejected.
    #[serde(deserialize_with = "deserialize_width")]
    pub width: u32,
    /// Video height in pixels; negative input is rejected.
    #[serde(deserialize_with = "deserialize_height")]
    pub height: u32,
    /// Duration in seconds; negative input is rejected.
    #[serde(deserialize_with = "deserialize_duration")]
    pub duration: u64,
    /// Thumbnail of the video.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<PhotoSize>,
    /// Covers available for the video.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_bounded_vec",
        skip_serializing_if = "Option::is_none"
    )]
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
    #[serde(
        default,
        deserialize_with = "deserialize_optional_bounded_vec",
        skip_serializing_if = "Option::is_none"
    )]
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

/// The part of a face a mask is positioned against.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MaskPoint {
    /// The forehead.
    Forehead,
    /// The eyes.
    Eyes,
    /// The mouth.
    Mouth,
    /// The chin.
    Chin,
    /// A part this crate does not know yet, preserved verbatim.
    Unknown(String),
}

impl MaskPoint {
    /// The wire spelling of this point.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Forehead => "forehead",
            Self::Eyes => "eyes",
            Self::Mouth => "mouth",
            Self::Chin => "chin",
            Self::Unknown(value) => value,
        }
    }
}

impl Serialize for MaskPoint {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for MaskPoint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "forehead" => Self::Forehead,
            "eyes" => Self::Eyes,
            "mouth" => Self::Mouth,
            "chin" => Self::Chin,
            _ => Self::Unknown(value),
        })
    }
}

/// Where a mask is positioned on a sticker face.
///
/// ```
/// use telegram_api_client::{MaskPoint, MaskPosition};
///
/// let position: MaskPosition = serde_json::from_str(
///     r#"{"point":"eyes","x_shift":0.1,"y_shift":-0.2,"scale":1.5}"#,
/// ).unwrap();
/// assert_eq!(position.point, MaskPoint::Eyes);
/// ```
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct MaskPosition {
    /// The face part; future values are preserved as [`MaskPoint::Unknown`].
    pub point: MaskPoint,
    /// Horizontal shift from the mask point.
    pub x_shift: f64,
    /// Vertical shift from the mask point.
    pub y_shift: f64,
    /// Positive scale of the mask.
    pub scale: f64,
}

impl Default for MaskPosition {
    /// A valid, neutral position: eyes, no shift and unit scale.
    fn default() -> Self {
        Self {
            point: MaskPoint::Eyes,
            x_shift: 0.0,
            y_shift: 0.0,
            scale: 1.0,
        }
    }
}

impl MaskPosition {
    /// A mask position with the supplied point and floating-point members.
    ///
    /// Call [`validate`](Self::validate) before sending when the values are not
    /// already known to be finite and the scale is positive.
    #[must_use]
    pub fn new(point: MaskPoint, x_shift: f64, y_shift: f64, scale: f64) -> Self {
        Self {
            point,
            x_shift,
            y_shift,
            scale,
        }
    }

    /// Checks the point and floating-point members before they reach the wire.
    ///
    /// # Errors
    ///
    /// [`crate::Error::InvalidRequest`] when the point is empty, a shift is
    /// non-finite, or the scale is non-finite or non-positive.
    pub fn validate(&self) -> crate::Result<()> {
        if self.point.as_str().is_empty() {
            return Err(crate::Error::InvalidRequest {
                field: "mask_position.point",
                reason: "must not be empty".to_string(),
            });
        }
        if !self.x_shift.is_finite() {
            return Err(crate::Error::InvalidRequest {
                field: "mask_position.x_shift",
                reason: "must be finite".to_string(),
            });
        }
        if !self.y_shift.is_finite() {
            return Err(crate::Error::InvalidRequest {
                field: "mask_position.y_shift",
                reason: "must be finite".to_string(),
            });
        }
        if !self.scale.is_finite() || self.scale <= 0.0 {
            return Err(crate::Error::InvalidRequest {
                field: "mask_position.scale",
                reason: "must be finite and greater than zero".to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Deserialize, Serialize)]
struct MaskPositionWire {
    point: MaskPoint,
    #[serde(alias = "xShift")]
    x_shift: f64,
    #[serde(alias = "yShift")]
    y_shift: f64,
    scale: f64,
}

impl Serialize for MaskPosition {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.validate().map_err(serde::ser::Error::custom)?;
        MaskPositionWire {
            point: self.point.clone(),
            x_shift: self.x_shift,
            y_shift: self.y_shift,
            scale: self.scale,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for MaskPosition {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = MaskPositionWire::deserialize(deserializer)?;
        let position = Self {
            point: wire.point,
            x_shift: wire.x_shift,
            y_shift: wire.y_shift,
            scale: wire.scale,
        };
        position.validate().map_err(serde::de::Error::custom)?;
        Ok(position)
    }
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
///     serde_json::from_str(r#"{"file_id":"A","file_unique_id":"u","type":"regular","width":0,"height":0,"is_animated":false,"is_video":false,"file_size":0}"#)
///         .unwrap();
/// assert_eq!(empty.file_size, Some(0));
/// let biggest: Sticker =
///     serde_json::from_str(r#"{"file_id":"A","file_unique_id":"u","type":"regular","width":4294967295,"height":4294967295,"is_animated":false,"is_video":false}"#)
///         .unwrap();
/// assert_eq!(biggest.width, u32::MAX);
///
/// assert!(serde_json::from_str::<Sticker>(r#"{"height":-1}"#).is_err());
/// assert!(serde_json::from_str::<Sticker>(r#"{"file_size":-1}"#).is_err());
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Sticker {
    /// Identifier usable to re-send the sticker.
    #[serde(alias = "fileId")]
    pub file_id: String,
    /// Identifier stable across bots.
    #[serde(alias = "fileUniqueId")]
    pub file_unique_id: String,
    /// Sticker width in pixels; negative input is rejected.
    #[serde(deserialize_with = "deserialize_width")]
    pub width: u32,
    /// Sticker height in pixels; negative input is rejected.
    #[serde(deserialize_with = "deserialize_height")]
    pub height: u32,
    /// Whether the sticker is animated.
    #[serde(alias = "isAnimated")]
    pub is_animated: bool,
    /// Whether the sticker is a video.
    #[serde(alias = "isVideo")]
    pub is_video: bool,
    /// Sticker type, serialized as `type` on the wire.
    #[serde(rename = "type", alias = "stickerType")]
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
/// object. This type stores exactly one member and creates it through a
/// constructor, so a zero-member or multi-member value cannot be built through
/// the supported API. Unknown members are retained verbatim.
///
/// ```
/// use telegram_api_client::{PollMedia, PollMediaKind};
///
/// let media: PollMedia = serde_json::from_str(r#"{"link":{"url":"https://example.com/x"}}"#).unwrap();
/// assert_eq!(media.kind(), PollMediaKind::Link);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PollMedia {
    member: PollMediaMember,
    extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PollMediaMember {
    Animation(Animation),
    Audio(Audio),
    Document(Document),
    Link(Link),
    LivePhoto(LivePhoto),
    Location(Location),
    Photo(Vec<PhotoSize>),
    Sticker(Sticker),
    Venue(Venue),
    Video(Video),
    Unknown { tag: String, value: Value },
}

impl PollMediaMember {
    fn kind(&self) -> PollMediaKind {
        match self {
            Self::Animation(_) => PollMediaKind::Animation,
            Self::Audio(_) => PollMediaKind::Audio,
            Self::Document(_) => PollMediaKind::Document,
            Self::Link(_) => PollMediaKind::Link,
            Self::LivePhoto(_) => PollMediaKind::LivePhoto,
            Self::Location(_) => PollMediaKind::Location,
            Self::Photo(_) => PollMediaKind::Photo,
            Self::Sticker(_) => PollMediaKind::Sticker,
            Self::Venue(_) => PollMediaKind::Venue,
            Self::Video(_) => PollMediaKind::Video,
            Self::Unknown { .. } => PollMediaKind::Unknown,
        }
    }

    fn tag(&self) -> &str {
        match self {
            Self::Animation(_) => "animation",
            Self::Audio(_) => "audio",
            Self::Document(_) => "document",
            Self::Link(_) => "link",
            Self::LivePhoto(_) => "live_photo",
            Self::Location(_) => "location",
            Self::Photo(_) => "photo",
            Self::Sticker(_) => "sticker",
            Self::Venue(_) => "venue",
            Self::Video(_) => "video",
            Self::Unknown { tag, .. } => tag,
        }
    }
}

impl PollMedia {
    /// Wraps an animation member.
    #[must_use]
    pub fn animation(value: Animation) -> Self {
        Self::from_member(PollMediaMember::Animation(value))
    }

    /// Wraps an audio member.
    #[must_use]
    pub fn audio(value: Audio) -> Self {
        Self::from_member(PollMediaMember::Audio(value))
    }

    /// Wraps a document member.
    #[must_use]
    pub fn document(value: Document) -> Self {
        Self::from_member(PollMediaMember::Document(value))
    }

    /// Wraps a link member.
    #[must_use]
    pub fn link(value: Link) -> Self {
        Self::from_member(PollMediaMember::Link(value))
    }

    /// Wraps a live-photo member.
    #[must_use]
    pub fn live_photo(value: LivePhoto) -> Self {
        Self::from_member(PollMediaMember::LivePhoto(value))
    }

    /// Wraps a location member.
    #[must_use]
    pub fn location(value: Location) -> Self {
        Self::from_member(PollMediaMember::Location(value))
    }

    /// Wraps a non-empty photo array.
    ///
    /// # Errors
    ///
    /// [`crate::Error::InvalidRequest`] when `value` is empty.
    pub fn photo(value: Vec<PhotoSize>) -> crate::Result<Self> {
        if value.is_empty() || value.len() > MAX_MEDIA_COLLECTION_ITEMS {
            return Err(crate::Error::InvalidRequest {
                field: "poll_media.photo",
                reason: format!("must contain between 1 and {MAX_MEDIA_COLLECTION_ITEMS} items"),
            });
        }
        Ok(Self::from_member(PollMediaMember::Photo(value)))
    }

    /// Wraps a sticker member.
    #[must_use]
    pub fn sticker(value: Sticker) -> Self {
        Self::from_member(PollMediaMember::Sticker(value))
    }

    /// Wraps a venue member.
    #[must_use]
    pub fn venue(value: Venue) -> Self {
        Self::from_member(PollMediaMember::Venue(value))
    }

    /// Wraps a video member.
    #[must_use]
    pub fn video(value: Video) -> Self {
        Self::from_member(PollMediaMember::Video(value))
    }

    /// The member kind carried by this object.
    #[must_use]
    pub fn kind(&self) -> PollMediaKind {
        self.member.kind()
    }

    /// The wire tag of the member.
    #[must_use]
    pub fn tag(&self) -> &str {
        self.member.tag()
    }

    /// The animation, when this carries one.
    #[must_use]
    pub fn animation_value(&self) -> Option<&Animation> {
        match &self.member {
            PollMediaMember::Animation(value) => Some(value),
            _ => None,
        }
    }

    /// The audio, when this carries one.
    #[must_use]
    pub fn audio_value(&self) -> Option<&Audio> {
        match &self.member {
            PollMediaMember::Audio(value) => Some(value),
            _ => None,
        }
    }

    /// The document, when this carries one.
    #[must_use]
    pub fn document_value(&self) -> Option<&Document> {
        match &self.member {
            PollMediaMember::Document(value) => Some(value),
            _ => None,
        }
    }

    /// The link, when this carries one.
    #[must_use]
    pub fn link_value(&self) -> Option<&Link> {
        match &self.member {
            PollMediaMember::Link(value) => Some(value),
            _ => None,
        }
    }

    /// The live photo, when this carries one.
    #[must_use]
    pub fn live_photo_value(&self) -> Option<&LivePhoto> {
        match &self.member {
            PollMediaMember::LivePhoto(value) => Some(value),
            _ => None,
        }
    }

    /// The location, when this carries one.
    #[must_use]
    pub fn location_value(&self) -> Option<&Location> {
        match &self.member {
            PollMediaMember::Location(value) => Some(value),
            _ => None,
        }
    }

    /// The photo sizes, when this carries them.
    #[must_use]
    pub fn photo_value(&self) -> Option<&[PhotoSize]> {
        match &self.member {
            PollMediaMember::Photo(value) => Some(value),
            _ => None,
        }
    }

    /// The sticker, when this carries one.
    #[must_use]
    pub fn sticker_value(&self) -> Option<&Sticker> {
        match &self.member {
            PollMediaMember::Sticker(value) => Some(value),
            _ => None,
        }
    }

    /// The venue, when this carries one.
    #[must_use]
    pub fn venue_value(&self) -> Option<&Venue> {
        match &self.member {
            PollMediaMember::Venue(value) => Some(value),
            _ => None,
        }
    }

    /// The video, when this carries one.
    #[must_use]
    pub fn video_value(&self) -> Option<&Video> {
        match &self.member {
            PollMediaMember::Video(value) => Some(value),
            _ => None,
        }
    }

    /// The unknown member value, when this carries one.
    #[must_use]
    pub fn unknown_value(&self) -> Option<&Value> {
        match &self.member {
            PollMediaMember::Unknown { value, .. } => Some(value),
            _ => None,
        }
    }

    /// Additional fields preserved alongside the selected member.
    #[must_use]
    pub fn extra_fields(&self) -> &Map<String, Value> {
        &self.extra
    }

    fn from_member(member: PollMediaMember) -> Self {
        Self {
            member,
            extra: Map::new(),
        }
    }
}

impl Serialize for PollMedia {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if let PollMediaMember::Photo(photos) = &self.member {
            if photos.is_empty() || photos.len() > MAX_MEDIA_COLLECTION_ITEMS {
                return Err(serde::ser::Error::custom(
                    "poll media photo array is outside the accepted bounds",
                ));
            }
        }
        let mut map = serializer.serialize_map(Some(1 + self.extra.len()))?;
        match &self.member {
            PollMediaMember::Animation(value) => map.serialize_entry("animation", value)?,
            PollMediaMember::Audio(value) => map.serialize_entry("audio", value)?,
            PollMediaMember::Document(value) => map.serialize_entry("document", value)?,
            PollMediaMember::Link(value) => map.serialize_entry("link", value)?,
            PollMediaMember::LivePhoto(value) => map.serialize_entry("live_photo", value)?,
            PollMediaMember::Location(value) => map.serialize_entry("location", value)?,
            PollMediaMember::Photo(value) => map.serialize_entry("photo", value)?,
            PollMediaMember::Sticker(value) => map.serialize_entry("sticker", value)?,
            PollMediaMember::Venue(value) => map.serialize_entry("venue", value)?,
            PollMediaMember::Video(value) => map.serialize_entry("video", value)?,
            PollMediaMember::Unknown { tag, value } => map.serialize_entry(tag, value)?,
        }
        for (key, value) in &self.extra {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

fn set_media_member<E>(member: &mut Option<PollMediaMember>, next: PollMediaMember) -> Result<(), E>
where
    E: serde::de::Error,
{
    if member.is_some() {
        return Err(E::custom(
            "poll media must contain exactly one member, but several were supplied",
        ));
    }
    *member = Some(next);
    Ok(())
}

struct PollMediaVisitor;

impl<'de> Visitor<'de> for PollMediaVisitor {
    type Value = PollMedia;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a poll media object with exactly one media member")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut member = None;
        let mut unknown = Vec::new();
        let mut extra = Map::new();

        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "animation" => {
                    set_media_member(&mut member, PollMediaMember::Animation(map.next_value()?))?;
                }
                "audio" => {
                    set_media_member(&mut member, PollMediaMember::Audio(map.next_value()?))?;
                }
                "document" => {
                    set_media_member(&mut member, PollMediaMember::Document(map.next_value()?))?;
                }
                "link" => {
                    set_media_member(&mut member, PollMediaMember::Link(map.next_value()?))?;
                }
                "live_photo" | "livePhoto" => {
                    set_media_member(&mut member, PollMediaMember::LivePhoto(map.next_value()?))?;
                }
                "location" => {
                    set_media_member(&mut member, PollMediaMember::Location(map.next_value()?))?;
                }
                "photo" => {
                    let BoundedVec(photos) = map.next_value::<BoundedVec<PhotoSize>>()?;
                    if photos.is_empty() {
                        return Err(serde::de::Error::custom(
                            "poll media photo array must not be empty",
                        ));
                    }
                    set_media_member(&mut member, PollMediaMember::Photo(photos))?;
                }
                "sticker" => {
                    set_media_member(&mut member, PollMediaMember::Sticker(map.next_value()?))?;
                }
                "venue" => {
                    set_media_member(&mut member, PollMediaMember::Venue(map.next_value()?))?;
                }
                "video" => {
                    set_media_member(&mut member, PollMediaMember::Video(map.next_value()?))?;
                }
                _ => {
                    let value = map.next_value::<Value>()?;
                    if unknown.iter().any(|(seen, _)| seen == &key) || extra.contains_key(&key) {
                        return Err(serde::de::Error::custom(format!(
                            "poll media field `{key}` was supplied twice"
                        )));
                    }
                    unknown.push((key, value));
                }
            }
        }

        let member = if let Some(member) = member {
            member
        } else {
            unknown.sort_by(|left, right| left.0.cmp(&right.0));
            if unknown.is_empty() {
                return Err(serde::de::Error::custom(
                    "poll media must contain exactly one member",
                ));
            }
            let (tag, value) = unknown.remove(0);
            PollMediaMember::Unknown { tag, value }
        };
        extra.extend(unknown);
        Ok(PollMedia { member, extra })
    }
}

impl<'de> Deserialize<'de> for PollMedia {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(PollMediaVisitor)
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
    /// A member this crate does not know yet.
    Unknown,
}
