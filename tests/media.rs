//! Wire fixtures for the poll media family.

use telegram_api_client::{Poll, PollMedia, PollMediaKind, PollOption};

#[test]
fn photo_member() {
    let media: PollMedia = serde_json::from_str(
        r#"{"photo":[{"file_id":"AgAC","file_unique_id":"AQAD","width":320,"height":240,"file_size":12345}]}"#,
    )
    .expect("photo fixture parses");

    assert_eq!(media.kind(), Some(PollMediaKind::Photo));
    let photos = media.photo.as_ref().expect("photo member");
    assert_eq!(photos.len(), 1);
    assert_eq!(photos[0].file_id, "AgAC");
    assert_eq!(photos[0].file_unique_id, "AQAD");
    assert_eq!(photos[0].width, 320);
    assert_eq!(photos[0].height, 240);
    assert_eq!(photos[0].file_size, Some(12345));
}

#[test]
fn link_member() {
    let media: PollMedia = serde_json::from_str(r#"{"link":{"url":"https://example.com/x"}}"#)
        .expect("link fixture parses");

    assert_eq!(media.kind(), Some(PollMediaKind::Link));
    assert_eq!(
        media.link.as_ref().map(|link| link.url.as_str()),
        Some("https://example.com/x")
    );
}

#[test]
fn venue_member() {
    let media: PollMedia = serde_json::from_str(
        r#"{"venue":{"location":{"latitude":51.05,"longitude":13.74},"title":"PATO","address":"Koeniginstraße 34"}}"#,
    )
    .expect("venue fixture parses");

    assert_eq!(media.kind(), Some(PollMediaKind::Venue));
    let venue = media.venue.as_ref().expect("venue member");
    assert_eq!(venue.title, "PATO");
    assert_eq!(venue.address, "Koeniginstraße 34");
    assert!((venue.location.latitude - 51.05).abs() < f64::EPSILON);
    assert!((venue.location.longitude - 13.74).abs() < f64::EPSILON);
}

#[test]
fn video_member() {
    let media: PollMedia = serde_json::from_str(
        r#"{"video":{"file_id":"BAAC","file_unique_id":"AQAD","width":1280,"height":720,"duration":12,"mime_type":"video/mp4","qualities":[{"file_id":"BAAD","file_unique_id":"AQAE","width":1280,"height":720,"codec":"h264"}]}}"#,
    )
    .expect("video fixture parses");

    assert_eq!(media.kind(), Some(PollMediaKind::Video));
    let video = media.video.as_ref().expect("video member");
    assert_eq!(video.file_id, "BAAC");
    assert_eq!(video.mime_type.as_deref(), Some("video/mp4"));
    let qualities = video.qualities.as_ref().expect("video qualities");
    assert_eq!(qualities.len(), 1);
    assert_eq!(qualities[0].file_id, "BAAD");
    assert_eq!(qualities[0].codec, "h264");
}

#[test]
fn sticker_member() {
    let media: PollMedia = serde_json::from_str(
        r#"{"sticker":{"file_id":"CAAC","file_unique_id":"AQAD","type":"regular","width":512,"height":512,"is_animated":false,"is_video":false}}"#,
    )
    .expect("sticker fixture parses");

    assert_eq!(media.kind(), Some(PollMediaKind::Sticker));
    let sticker = media.sticker.as_ref().expect("sticker member");
    assert_eq!(sticker.file_id, "CAAC");
    assert_eq!(sticker.sticker_type, "regular");
    assert_eq!(sticker.width, 512);
    assert_eq!(sticker.height, 512);
    assert!(!sticker.is_animated);
    assert!(!sticker.is_video);
}

#[test]
fn animation_member() {
    let media: PollMedia = serde_json::from_str(
        r#"{"animation":{"file_id":"DAAC","file_unique_id":"AQAD","width":320,"height":240,"duration":2,"thumbnail":{"file_id":"DAAD","file_unique_id":"AQAE","width":64,"height":48},"file_name":"loop.mp4","mime_type":"video/mp4","file_size":2048}}"#,
    )
    .expect("animation fixture parses");

    assert_eq!(media.kind(), Some(PollMediaKind::Animation));
    let animation = media.animation.as_ref().expect("animation member");
    assert_eq!(animation.duration, 2);
    assert_eq!(animation.file_name.as_deref(), Some("loop.mp4"));
    assert_eq!(
        animation.thumbnail.as_ref().map(|photo| photo.width),
        Some(64)
    );
}

#[test]
fn audio_member() {
    let media: PollMedia = serde_json::from_str(
        r#"{"audio":{"file_id":"EAAC","file_unique_id":"AQAD","duration":9,"performer":"PATO","title":"Koenig","file_name":"track.mp3","mime_type":"audio/mpeg","file_size":4096,"thumbnail":{"file_id":"EAAD","file_unique_id":"AQAE","width":90,"height":90}}}"#,
    )
    .expect("audio fixture parses");

    assert_eq!(media.kind(), Some(PollMediaKind::Audio));
    let audio = media.audio.as_ref().expect("audio member");
    assert_eq!(audio.duration, 9);
    assert_eq!(audio.performer.as_deref(), Some("PATO"));
    assert_eq!(audio.title.as_deref(), Some("Koenig"));
    assert_eq!(audio.thumbnail.as_ref().map(|photo| photo.height), Some(90));
}

#[test]
fn document_member() {
    let media: PollMedia = serde_json::from_str(
        r#"{"document":{"file_id":"FAAC","file_unique_id":"AQAD","thumbnail":{"file_id":"FAAD","file_unique_id":"AQAE","width":64,"height":64},"file_name":"menu.pdf","mime_type":"application/pdf","file_size":8192}}"#,
    )
    .expect("document fixture parses");

    assert_eq!(media.kind(), Some(PollMediaKind::Document));
    let document = media.document.as_ref().expect("document member");
    assert_eq!(document.file_name.as_deref(), Some("menu.pdf"));
    assert_eq!(document.mime_type.as_deref(), Some("application/pdf"));
    assert_eq!(document.file_size, Some(8192));
}

#[test]
fn live_photo_member() {
    let media: PollMedia = serde_json::from_str(
        r#"{"live_photo":{"photo":[{"file_id":"GAAC","file_unique_id":"AQAD","width":1080,"height":1920}],"file_id":"GAAD","file_unique_id":"AQAE","width":1080,"height":1920,"duration":3,"mime_type":"video/mp4","file_size":16384}}"#,
    )
    .expect("live photo fixture parses");

    assert_eq!(media.kind(), Some(PollMediaKind::LivePhoto));
    let live_photo = media.live_photo.as_ref().expect("live photo member");
    assert_eq!(live_photo.duration, 3);
    assert_eq!(live_photo.width, 1080);
    assert_eq!(live_photo.photo.as_ref().map(Vec::len), Some(1));
}

#[test]
fn location_member() {
    let media: PollMedia =
        serde_json::from_str(r#"{"location":{"latitude":51.05,"longitude":13.74}}"#)
            .expect("location fixture parses");

    assert_eq!(media.kind(), Some(PollMediaKind::Location));
    let location = media.location.as_ref().expect("location member");
    assert!((location.latitude - 51.05).abs() < f64::EPSILON);
    assert!((location.longitude - 13.74).abs() < f64::EPSILON);
}

#[test]
fn empty_object_has_no_member() {
    let media: PollMedia = serde_json::from_str("{}").expect("empty fixture parses");

    assert_eq!(media, PollMedia::default());
    assert!(media.animation.is_none());
    assert!(media.audio.is_none());
    assert!(media.document.is_none());
    assert!(media.link.is_none());
    assert!(media.live_photo.is_none());
    assert!(media.location.is_none());
    assert!(media.photo.is_none());
    assert!(media.sticker.is_none());
    assert!(media.venue.is_none());
    assert!(media.video.is_none());
    assert!(media.is_empty());
    assert_eq!(media.kind(), None);
}

#[test]
fn unknown_field_is_ignored() {
    let poll: Poll = serde_json::from_str(r#"{"media":{"poll_media_hint":"x"}}"#)
        .expect("unknown member field is ignored");

    let media = poll.media.as_ref().expect("media member");
    assert!(media.is_empty());
    assert_eq!(media.kind(), None);
}

#[test]
fn poll_carries_media_and_explanation_media() {
    let poll: Poll = serde_json::from_str(
        r#"{"id":"p1","question":"Where?","options":[{"text":"PATO","voter_count":1}],"media":{"photo":[{"file_id":"AgAC","file_unique_id":"AQAD","width":320,"height":240,"file_size":12345}]},"explanation_media":{"link":{"url":"https://example.com/x"}}}"#,
    )
    .expect("poll with both media fields parses");

    assert_eq!(
        poll.media.as_ref().and_then(PollMedia::kind),
        Some(PollMediaKind::Photo)
    );
    assert_eq!(
        poll.explanation_media.as_ref().and_then(PollMedia::kind),
        Some(PollMediaKind::Link)
    );
}

#[test]
fn poll_option_carries_media() {
    let option: PollOption = serde_json::from_str(
        r#"{"text":"PATO","voter_count":1,"media":{"venue":{"location":{"latitude":51.05,"longitude":13.74},"title":"PATO","address":"Koeniginstraße 34"}}}"#,
    )
    .expect("poll option with media parses");

    assert_eq!(option.text, "PATO");
    let media = option.media.as_ref().expect("option media");
    assert_eq!(media.kind(), Some(PollMediaKind::Venue));
    assert_eq!(
        media.venue.as_ref().map(|venue| venue.title.as_str()),
        Some("PATO")
    );
}

use proptest::prelude::*;
use telegram_api_client::{
    Animation, Audio, Document, Link, LivePhoto, Location, MaskPosition, PhotoSize, Sticker, Venue,
    Video,
};

/// A `PollMedia` carrying exactly the member the index selects, plus that
/// member as the expected [`PollMediaKind`].
fn with_member(index: u8, text: &str) -> (PollMedia, PollMediaKind) {
    let mut media = PollMedia::default();
    let photo = PhotoSize {
        file_id: text.to_owned(),
        ..PhotoSize::default()
    };
    let kind = match index % 10 {
        0 => {
            media.animation = Some(Animation {
                file_id: text.to_owned(),
                ..Animation::default()
            });
            PollMediaKind::Animation
        }
        1 => {
            media.audio = Some(Audio {
                title: Some(text.to_owned()),
                ..Audio::default()
            });
            PollMediaKind::Audio
        }
        2 => {
            media.document = Some(Document {
                file_name: Some(text.to_owned()),
                ..Document::default()
            });
            PollMediaKind::Document
        }
        3 => {
            media.link = Some(Link {
                url: text.to_owned(),
            });
            PollMediaKind::Link
        }
        4 => {
            media.live_photo = Some(LivePhoto {
                photo: Some(vec![photo]),
                ..LivePhoto::default()
            });
            PollMediaKind::LivePhoto
        }
        5 => {
            media.location = Some(Location {
                latitude: 51.05,
                longitude: 13.74,
                ..Location::default()
            });
            PollMediaKind::Location
        }
        6 => {
            media.photo = Some(vec![photo]);
            PollMediaKind::Photo
        }
        7 => {
            media.sticker = Some(Sticker {
                sticker_type: text.to_owned(),
                ..Sticker::default()
            });
            PollMediaKind::Sticker
        }
        8 => {
            media.venue = Some(Venue {
                title: text.to_owned(),
                ..Venue::default()
            });
            PollMediaKind::Venue
        }
        _ => {
            media.video = Some(Video {
                mime_type: Some(text.to_owned()),
                ..Video::default()
            });
            PollMediaKind::Video
        }
    };
    (media, kind)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 96, ..ProptestConfig::default() })]

    /// Exactly one member set must name that member, and the object must
    /// survive a JSON round trip unchanged - arbitrary Unicode included.
    #[test]
    fn kind_names_the_only_present_member(index in any::<u8>(), text in "\\PC{0,16}") {
        let (media, expected) = with_member(index, &text);
        prop_assert_eq!(media.kind(), Some(expected));
        prop_assert!(!media.is_empty());

        let json = serde_json::to_string(&media)?;
        let back: PollMedia = serde_json::from_str(&json)?;
        prop_assert_eq!(back, media);
    }

    /// `kind()` reports a member exactly when `is_empty()` says there is one,
    /// whatever subset of the ten fields a payload happens to carry.
    #[test]
    fn kind_is_some_exactly_when_a_member_is_present(flags in any::<u16>()) {
        let mut media = PollMedia::default();
        if flags & 0b0000_0000_0001 != 0 {
            media.animation = Some(Animation::default());
        }
        if flags & 0b0000_0000_0010 != 0 {
            media.audio = Some(Audio::default());
        }
        if flags & 0b0000_0000_0100 != 0 {
            media.document = Some(Document::default());
        }
        if flags & 0b0000_0000_1000 != 0 {
            media.link = Some(Link::default());
        }
        if flags & 0b0000_0001_0000 != 0 {
            media.live_photo = Some(LivePhoto::default());
        }
        if flags & 0b0000_0010_0000 != 0 {
            media.location = Some(Location::default());
        }
        if flags & 0b0000_0100_0000 != 0 {
            media.photo = Some(Vec::new());
        }
        if flags & 0b0000_1000_0000 != 0 {
            media.sticker = Some(Sticker::default());
        }
        if flags & 0b0001_0000_0000 != 0 {
            media.venue = Some(Venue::default());
        }
        if flags & 0b0010_0000_0000 != 0 {
            media.video = Some(Video::default());
        }

        prop_assert_eq!(media.kind().is_some(), !media.is_empty());
        prop_assert_eq!(media.kind().is_none(), media.is_empty());
    }
}

#[test]
fn mask_position_is_reflexive_for_nan() {
    let nan = MaskPosition {
        point: "eyes".to_owned(),
        x_shift: f64::NAN,
        y_shift: 0.0,
        scale: 1.0,
    };
    assert_eq!(nan, nan, "bitwise comparison keeps Eq sound for NaN");

    let shifted = MaskPosition {
        x_shift: 0.0,
        ..nan.clone()
    };
    assert_ne!(nan, shifted, "a zero shift is not NaN");
}

#[test]
fn camel_case_members_are_tolerated() {
    let poll: Poll = serde_json::from_str(
        r#"{"media":{"livePhoto":{"fileId":"A","fileUniqueId":"u"}},"explanationMedia":{"document":{"fileId":"B"}}}"#,
    )
    .expect("camelCase poll media parses");

    let media = poll.media.as_ref().expect("media");
    assert_eq!(media.kind(), Some(PollMediaKind::LivePhoto));
    assert_eq!(
        media
            .live_photo
            .as_ref()
            .map(|photo| photo.file_id.as_str()),
        Some("A")
    );
    assert_eq!(
        poll.explanation_media.as_ref().and_then(PollMedia::kind),
        Some(PollMediaKind::Document)
    );
}

// ---------------------------------------------------------------------------
// Numeric members are non-negative (issue #60).
//
// Every size, duration and dimension the Bot API sends is a count. Before the
// fix these fields were `i64`, so `-1` deserialized and serialized happily and
// produced media objects Telegram would never send.
// ---------------------------------------------------------------------------

use telegram_api_client::{Error, VideoQuality, WebhookGuard};

/// Declares one bounded numeric member: `-1` must be refused with an error that
/// names the member, `0` and the type maximum must parse.
macro_rules! bounded_member {
    ($name:ident, $ty:ty, $template:literal, $field:literal, $max:ident) => {
        #[test]
        fn $name() {
            let parse = |value: &str| -> Result<$ty, serde_json::Error> {
                serde_json::from_str(&$template.replace("{v}", value))
            };

            // Just outside the range: refused, and the error names the member.
            let refused = parse("-1").expect_err("a negative count must not parse");
            assert!(
                refused.to_string().contains(&format!("`{}`", $field)),
                "the rejection names `{}`: {refused}",
                $field
            );

            // Just inside both ends of the range: accepted.
            assert!(parse("0").is_ok(), "0 is the inclusive lower bound");
            assert!(parse($max).is_ok(), "{} is the inclusive upper bound", $max);
        }
    };
}

/// `u32::MAX` as it arrives on the wire.
const WIRE_U32_MAX: &str = "4294967295";
/// `u64::MAX` as it arrives on the wire.
const WIRE_U64_MAX: &str = "18446744073709551615";

bounded_member!(
    photo_size_width_is_non_negative,
    PhotoSize,
    r#"{"width":{v}}"#,
    "width",
    WIRE_U32_MAX
);
bounded_member!(
    photo_size_height_is_non_negative,
    PhotoSize,
    r#"{"height":{v}}"#,
    "height",
    WIRE_U32_MAX
);
bounded_member!(
    photo_size_file_size_is_non_negative,
    PhotoSize,
    r#"{"file_size":{v}}"#,
    "file_size",
    WIRE_U64_MAX
);

bounded_member!(
    live_photo_width_is_non_negative,
    LivePhoto,
    r#"{"width":{v}}"#,
    "width",
    WIRE_U32_MAX
);
bounded_member!(
    live_photo_height_is_non_negative,
    LivePhoto,
    r#"{"height":{v}}"#,
    "height",
    WIRE_U32_MAX
);
bounded_member!(
    live_photo_duration_is_non_negative,
    LivePhoto,
    r#"{"duration":{v}}"#,
    "duration",
    WIRE_U64_MAX
);
bounded_member!(
    live_photo_file_size_is_non_negative,
    LivePhoto,
    r#"{"file_size":{v}}"#,
    "file_size",
    WIRE_U64_MAX
);

bounded_member!(
    animation_width_is_non_negative,
    Animation,
    r#"{"width":{v}}"#,
    "width",
    WIRE_U32_MAX
);
bounded_member!(
    animation_height_is_non_negative,
    Animation,
    r#"{"height":{v}}"#,
    "height",
    WIRE_U32_MAX
);
bounded_member!(
    animation_duration_is_non_negative,
    Animation,
    r#"{"duration":{v}}"#,
    "duration",
    WIRE_U64_MAX
);
bounded_member!(
    animation_file_size_is_non_negative,
    Animation,
    r#"{"file_size":{v}}"#,
    "file_size",
    WIRE_U64_MAX
);

bounded_member!(
    audio_duration_is_non_negative,
    Audio,
    r#"{"duration":{v}}"#,
    "duration",
    WIRE_U64_MAX
);
bounded_member!(
    audio_file_size_is_non_negative,
    Audio,
    r#"{"file_size":{v}}"#,
    "file_size",
    WIRE_U64_MAX
);

bounded_member!(
    document_file_size_is_non_negative,
    Document,
    r#"{"file_size":{v}}"#,
    "file_size",
    WIRE_U64_MAX
);

bounded_member!(
    video_quality_width_is_non_negative,
    VideoQuality,
    r#"{"width":{v}}"#,
    "width",
    WIRE_U32_MAX
);
bounded_member!(
    video_quality_height_is_non_negative,
    VideoQuality,
    r#"{"height":{v}}"#,
    "height",
    WIRE_U32_MAX
);
bounded_member!(
    video_quality_file_size_is_non_negative,
    VideoQuality,
    r#"{"file_size":{v}}"#,
    "file_size",
    WIRE_U64_MAX
);

bounded_member!(
    video_width_is_non_negative,
    Video,
    r#"{"width":{v}}"#,
    "width",
    WIRE_U32_MAX
);
bounded_member!(
    video_height_is_non_negative,
    Video,
    r#"{"height":{v}}"#,
    "height",
    WIRE_U32_MAX
);
bounded_member!(
    video_duration_is_non_negative,
    Video,
    r#"{"duration":{v}}"#,
    "duration",
    WIRE_U64_MAX
);
bounded_member!(
    video_start_timestamp_is_non_negative,
    Video,
    r#"{"start_timestamp":{v}}"#,
    "start_timestamp",
    WIRE_U64_MAX
);
bounded_member!(
    video_file_size_is_non_negative,
    Video,
    r#"{"file_size":{v}}"#,
    "file_size",
    WIRE_U64_MAX
);

bounded_member!(
    sticker_width_is_non_negative,
    Sticker,
    r#"{"width":{v}}"#,
    "width",
    WIRE_U32_MAX
);
bounded_member!(
    sticker_height_is_non_negative,
    Sticker,
    r#"{"height":{v}}"#,
    "height",
    WIRE_U32_MAX
);
bounded_member!(
    sticker_file_size_is_non_negative,
    Sticker,
    r#"{"file_size":{v}}"#,
    "file_size",
    WIRE_U64_MAX
);

#[test]
fn dimensions_above_u32_max_are_rejected_not_truncated() {
    let refused = serde_json::from_str::<PhotoSize>(r#"{"width":4294967296}"#)
        .expect_err("a width that does not fit u32 must not parse");
    assert!(
        refused.to_string().contains("`width`"),
        "the rejection names the member: {refused}"
    );
    assert!(serde_json::from_str::<Sticker>(r#"{"height":4294967296}"#).is_err());
}

#[test]
fn camel_case_sizes_are_validated_too() {
    assert!(serde_json::from_str::<Document>(r#"{"fileSize":0}"#).is_ok());
    let refused = serde_json::from_str::<Document>(r#"{"fileSize":-1}"#)
        .expect_err("the camelCase alias is the same member and is checked too");
    assert!(refused.to_string().contains("file_size"));
}

#[test]
fn missing_and_unknown_members_keep_their_defaults() {
    let photo: PhotoSize = serde_json::from_str(r#"{"width":320,"future_member":"x"}"#)
        .expect("an unknown member is still ignored");
    assert_eq!(photo.width, 320);
    assert_eq!(photo.height, 0, "an absent dimension defaults to zero");
    assert_eq!(photo.file_size, None, "an absent size stays unknown");
}

#[test]
fn a_negative_member_inside_poll_media_is_rejected() {
    let refused = serde_json::from_str::<Poll>(
        r#"{"media":{"photo":[{"file_id":"A","width":320,"file_size":-1}]}}"#,
    )
    .expect_err("the wire path through PollMedia is validated as well");
    assert!(refused.to_string().contains("file_size"), "{refused}");
}

#[test]
fn a_negative_member_surfaces_as_a_payload_error() {
    // `-1` never reaches the caller as a value: the webhook parser maps the
    // serde rejection to `Error::Payload`, like every other undecodable body.
    let guard = WebhookGuard::new("s3cret").expect("a non-empty secret");
    let body = br#"{"update_id":1,"poll":{"media":{"photo":[{"file_id":"A","width":-1}]}}}"#;

    let error = guard
        .verify_and_parse(Some("s3cret"), guard.derived_path(), body)
        .expect_err("a negative width cannot be decoded");

    assert!(matches!(error, Error::Payload { .. }), "{error:?}");
    let message = error.to_string();
    assert!(message.contains("`width`"), "{message}");
}

#[test]
fn the_bounds_survive_a_round_trip() {
    let video = Video {
        width: u32::MAX,
        height: 0,
        duration: u64::MAX,
        start_timestamp: Some(0),
        file_size: Some(u64::MAX),
        ..Video::default()
    };
    let json = serde_json::to_string(&video).expect("the bounds serialize");
    assert!(
        !json.contains('-'),
        "an unsigned member can only emit an unsigned number: {json}"
    );
    let back: Video = serde_json::from_str(&json).expect("the bounds parse back");
    assert_eq!(back, video);
}

/// One numeric member: its label, its JSON template with `{v}` for the number
/// under test, and whether that object parses.
type NumericMember = (&'static str, &'static str, fn(&str) -> bool);

/// Every numeric member of every media object.
fn numeric_members() -> Vec<NumericMember> {
    fn accepts<T: serde::de::DeserializeOwned>(json: &str) -> bool {
        serde_json::from_str::<T>(json).is_ok()
    }

    vec![
        ("PhotoSize.width", r#"{"width":{v}}"#, accepts::<PhotoSize>),
        (
            "PhotoSize.height",
            r#"{"height":{v}}"#,
            accepts::<PhotoSize>,
        ),
        (
            "PhotoSize.file_size",
            r#"{"file_size":{v}}"#,
            accepts::<PhotoSize>,
        ),
        ("LivePhoto.width", r#"{"width":{v}}"#, accepts::<LivePhoto>),
        (
            "LivePhoto.height",
            r#"{"height":{v}}"#,
            accepts::<LivePhoto>,
        ),
        (
            "LivePhoto.duration",
            r#"{"duration":{v}}"#,
            accepts::<LivePhoto>,
        ),
        (
            "LivePhoto.file_size",
            r#"{"file_size":{v}}"#,
            accepts::<LivePhoto>,
        ),
        ("Animation.width", r#"{"width":{v}}"#, accepts::<Animation>),
        (
            "Animation.height",
            r#"{"height":{v}}"#,
            accepts::<Animation>,
        ),
        (
            "Animation.duration",
            r#"{"duration":{v}}"#,
            accepts::<Animation>,
        ),
        (
            "Animation.file_size",
            r#"{"file_size":{v}}"#,
            accepts::<Animation>,
        ),
        ("Audio.duration", r#"{"duration":{v}}"#, accepts::<Audio>),
        ("Audio.file_size", r#"{"file_size":{v}}"#, accepts::<Audio>),
        (
            "Document.file_size",
            r#"{"file_size":{v}}"#,
            accepts::<Document>,
        ),
        (
            "VideoQuality.width",
            r#"{"width":{v}}"#,
            accepts::<VideoQuality>,
        ),
        (
            "VideoQuality.height",
            r#"{"height":{v}}"#,
            accepts::<VideoQuality>,
        ),
        (
            "VideoQuality.file_size",
            r#"{"file_size":{v}}"#,
            accepts::<VideoQuality>,
        ),
        ("Video.width", r#"{"width":{v}}"#, accepts::<Video>),
        ("Video.height", r#"{"height":{v}}"#, accepts::<Video>),
        ("Video.duration", r#"{"duration":{v}}"#, accepts::<Video>),
        (
            "Video.start_timestamp",
            r#"{"start_timestamp":{v}}"#,
            accepts::<Video>,
        ),
        ("Video.file_size", r#"{"file_size":{v}}"#, accepts::<Video>),
        ("Sticker.width", r#"{"width":{v}}"#, accepts::<Sticker>),
        ("Sticker.height", r#"{"height":{v}}"#, accepts::<Sticker>),
        (
            "Sticker.file_size",
            r#"{"file_size":{v}}"#,
            accepts::<Sticker>,
        ),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    /// No integer below zero is accepted by any numeric member, whatever its
    /// magnitude - the whole negative range, not just `-1`.
    #[test]
    fn no_negative_number_is_ever_accepted(value in i64::MIN..=-1) {
        for (label, template, accepts) in numeric_members() {
            let json = template.replace("{v}", &value.to_string());
            prop_assert!(!accepts(&json), "{} accepted {value}", label);
        }
    }

    /// Unsigned counts survive a JSON round trip unchanged, at any magnitude.
    #[test]
    fn unsigned_members_round_trip(
        width in any::<u32>(),
        height in any::<u32>(),
        duration in any::<u64>(),
        file_size in any::<u64>(),
    ) {
        let sticker = Sticker { width, height, file_size: Some(file_size), ..Sticker::default() };
        prop_assert_eq!(serde_json::from_str::<Sticker>(&serde_json::to_string(&sticker)?)?, sticker);

        let audio = Audio { duration, file_size: Some(file_size), ..Audio::default() };
        prop_assert_eq!(serde_json::from_str::<Audio>(&serde_json::to_string(&audio)?)?, audio);
    }
}
