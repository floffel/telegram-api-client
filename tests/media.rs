//! Wire fixtures and boundaries for the poll media family.

use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use telegram_api_client::{
    Animation, Audio, Document, Error, Link, LivePhoto, Location, MaskPoint, MaskPosition,
    PhotoSize, Poll, PollMedia, PollMediaKind, PollOption, Sticker, Venue, Video, VideoQuality,
    WebhookGuard, MAX_MEDIA_COLLECTION_ITEMS,
};

fn parse<T: DeserializeOwned>(value: Value) -> Result<T, serde_json::Error> {
    serde_json::from_value(value)
}

fn assert_required_fields<T: DeserializeOwned + std::fmt::Debug>(object: &Value, fields: &[&str]) {
    parse::<T>(object.clone()).expect("the complete fixture parses");
    for field in fields {
        let mut missing = object.clone();
        missing
            .as_object_mut()
            .expect("fixtures are objects")
            .remove(*field);
        let error = parse::<T>(missing).expect_err(field);
        assert!(
            error.to_string().contains("missing field"),
            "removing `{field}` must fail on the missing field: {error}"
        );
    }
}

#[test]
fn every_required_media_field_is_really_required() {
    assert_required_fields::<PhotoSize>(
        &json!({"file_id":"A","file_unique_id":"u","width":0,"height":0}),
        &["file_id", "file_unique_id", "width", "height"],
    );
    assert_required_fields::<Link>(&json!({"url":"https://example.com"}), &["url"]);
    assert_required_fields::<LivePhoto>(
        &json!({"file_id":"A","file_unique_id":"u","width":0,"height":0,"duration":0}),
        &["file_id", "file_unique_id", "width", "height", "duration"],
    );
    assert_required_fields::<Animation>(
        &json!({"file_id":"A","file_unique_id":"u","width":0,"height":0,"duration":0}),
        &["file_id", "file_unique_id", "width", "height", "duration"],
    );
    assert_required_fields::<Audio>(
        &json!({"file_id":"A","file_unique_id":"u","duration":0}),
        &["file_id", "file_unique_id", "duration"],
    );
    assert_required_fields::<Document>(
        &json!({"file_id":"A","file_unique_id":"u"}),
        &["file_id", "file_unique_id"],
    );
    assert_required_fields::<Venue>(
        &json!({"location":{"latitude":0.0,"longitude":0.0},"title":"T","address":"A"}),
        &["location", "title", "address"],
    );
    assert_required_fields::<VideoQuality>(
        &json!({"file_id":"A","file_unique_id":"u","width":0,"height":0,"codec":"h264"}),
        &["file_id", "file_unique_id", "width", "height", "codec"],
    );
    assert_required_fields::<Video>(
        &json!({"file_id":"A","file_unique_id":"u","width":0,"height":0,"duration":0}),
        &["file_id", "file_unique_id", "width", "height", "duration"],
    );
    assert_required_fields::<Sticker>(
        &json!({"file_id":"A","file_unique_id":"u","type":"regular","width":0,"height":0,
                "is_animated":false,"is_video":false}),
        &[
            "file_id",
            "file_unique_id",
            "type",
            "width",
            "height",
            "is_animated",
            "is_video",
        ],
    );
}

#[test]
fn optional_media_fields_may_be_absent() {
    let photo: PhotoSize = parse(json!({
        "file_id":"A","file_unique_id":"u","width":0,"height":0
    }))
    .expect("PhotoSize.file_size is optional");
    assert_eq!(photo.file_size, None);

    let live_photo: LivePhoto = parse(json!({
        "file_id":"A","file_unique_id":"u","width":0,"height":0,"duration":0
    }))
    .expect("LivePhoto optional members are optional");
    assert!(live_photo.photo.is_none());

    let video: Video = parse(json!({
        "file_id":"A","file_unique_id":"u","width":0,"height":0,"duration":0
    }))
    .expect("Video optional collections are optional");
    assert!(video.cover.is_none());
    assert!(video.qualities.is_none());
}

#[test]
fn photo_member() {
    let media: PollMedia = parse(json!({
        "photo":[{"file_id":"AgAC","file_unique_id":"AQAD","width":320,"height":240,
                  "file_size":12345}]
    }))
    .expect("photo fixture parses");

    assert_eq!(media.kind(), PollMediaKind::Photo);
    assert_eq!(media.tag(), "photo");
    let photos = media.photo_value().expect("photo member");
    assert_eq!(photos.len(), 1);
    assert_eq!(photos[0].file_id, "AgAC");
    assert_eq!(photos[0].file_unique_id, "AQAD");
    assert_eq!(photos[0].width, 320);
    assert_eq!(photos[0].height, 240);
    assert_eq!(photos[0].file_size, Some(12345));
    assert!(media.animation_value().is_none());
    assert!(media.video_value().is_none());
}

#[test]
fn link_member() {
    let media: PollMedia =
        parse(json!({"link":{"url":"https://example.com/x"}})).expect("link fixture parses");

    assert_eq!(media.kind(), PollMediaKind::Link);
    assert_eq!(media.tag(), "link");
    assert_eq!(
        media.link_value().map(|link| link.url.as_str()),
        Some("https://example.com/x")
    );
    assert!(media.photo_value().is_none());
}

#[test]
fn venue_member() {
    let media: PollMedia = parse(json!({
        "venue":{"location":{"latitude":51.05,"longitude":13.74},"title":"PATO",
                 "address":"Koeniginstraße 34"}
    }))
    .expect("venue fixture parses");

    assert_eq!(media.kind(), PollMediaKind::Venue);
    assert_eq!(media.tag(), "venue");
    let venue = media.venue_value().expect("venue member");
    assert_eq!(venue.title, "PATO");
    assert_eq!(venue.address, "Koeniginstraße 34");
    assert!((venue.location.latitude - 51.05).abs() < f64::EPSILON);
    assert!((venue.location.longitude - 13.74).abs() < f64::EPSILON);
    assert!(media.location_value().is_none());
}

#[test]
fn video_member() {
    let media: PollMedia = parse(json!({
        "video":{"file_id":"BAAC","file_unique_id":"AQAD","width":1280,"height":720,
                 "duration":12,"mime_type":"video/mp4",
                 "qualities":[{"file_id":"BAAD","file_unique_id":"AQAE","width":1280,
                               "height":720,"codec":"h264"}]}
    }))
    .expect("video fixture parses");

    assert_eq!(media.kind(), PollMediaKind::Video);
    assert_eq!(media.tag(), "video");
    let video = media.video_value().expect("video member");
    assert_eq!(video.file_id, "BAAC");
    assert_eq!(video.mime_type.as_deref(), Some("video/mp4"));
    let qualities = video.qualities.as_ref().expect("video qualities");
    assert_eq!(qualities.len(), 1);
    assert_eq!(qualities[0].file_id, "BAAD");
    assert_eq!(qualities[0].codec, "h264");
    assert!(media.sticker_value().is_none());
}

#[test]
fn sticker_member() {
    let media: PollMedia = parse(json!({
        "sticker":{"file_id":"CAAC","file_unique_id":"AQAD","type":"regular","width":512,
                   "height":512,"is_animated":false,"is_video":false}
    }))
    .expect("sticker fixture parses");

    assert_eq!(media.kind(), PollMediaKind::Sticker);
    assert_eq!(media.tag(), "sticker");
    let sticker = media.sticker_value().expect("sticker member");
    assert_eq!(sticker.file_id, "CAAC");
    assert_eq!(sticker.sticker_type, "regular");
    assert_eq!(sticker.width, 512);
    assert_eq!(sticker.height, 512);
    assert!(!sticker.is_animated);
    assert!(!sticker.is_video);
    assert!(media.venue_value().is_none());
}

#[test]
fn animation_member() {
    let media: PollMedia = parse(json!({
        "animation":{"file_id":"DAAC","file_unique_id":"AQAD","width":320,"height":240,
                     "duration":2,"thumbnail":{"file_id":"DAAD","file_unique_id":"AQAE",
                     "width":64,"height":48},"file_name":"loop.mp4","mime_type":"video/mp4",
                     "file_size":2048}
    }))
    .expect("animation fixture parses");

    assert_eq!(media.kind(), PollMediaKind::Animation);
    assert_eq!(media.tag(), "animation");
    let animation = media.animation_value().expect("animation member");
    assert_eq!(animation.duration, 2);
    assert_eq!(animation.file_name.as_deref(), Some("loop.mp4"));
    assert_eq!(
        animation.thumbnail.as_ref().map(|photo| photo.width),
        Some(64)
    );
    assert!(media.audio_value().is_none());
}

#[test]
fn audio_member() {
    let media: PollMedia = parse(json!({
        "audio":{"file_id":"EAAC","file_unique_id":"AQAD","duration":9,"performer":"PATO",
                 "title":"Koenig","file_name":"track.mp3","mime_type":"audio/mpeg",
                 "file_size":4096,"thumbnail":{"file_id":"EAAD","file_unique_id":"AQAE",
                 "width":90,"height":90}}
    }))
    .expect("audio fixture parses");

    assert_eq!(media.kind(), PollMediaKind::Audio);
    assert_eq!(media.tag(), "audio");
    let audio = media.audio_value().expect("audio member");
    assert_eq!(audio.duration, 9);
    assert_eq!(audio.performer.as_deref(), Some("PATO"));
    assert_eq!(audio.title.as_deref(), Some("Koenig"));
    assert_eq!(audio.thumbnail.as_ref().map(|photo| photo.height), Some(90));
    assert!(media.document_value().is_none());
}

#[test]
fn document_member() {
    let media: PollMedia = parse(json!({
        "document":{"file_id":"FAAC","file_unique_id":"AQAD","thumbnail":{"file_id":"FAAD",
                    "file_unique_id":"AQAE","width":64,"height":64},"file_name":"menu.pdf",
                    "mime_type":"application/pdf","file_size":8192}
    }))
    .expect("document fixture parses");

    assert_eq!(media.kind(), PollMediaKind::Document);
    assert_eq!(media.tag(), "document");
    let document = media.document_value().expect("document member");
    assert_eq!(document.file_name.as_deref(), Some("menu.pdf"));
    assert_eq!(document.mime_type.as_deref(), Some("application/pdf"));
    assert_eq!(document.file_size, Some(8192));
    assert!(media.link_value().is_none());
}

#[test]
fn live_photo_member() {
    let media: PollMedia = parse(json!({
        "live_photo":{"photo":[{"file_id":"GAAC","file_unique_id":"AQAD","width":1080,
                       "height":1920}],"file_id":"GAAD","file_unique_id":"AQAE","width":1080,
                       "height":1920,"duration":3,"mime_type":"video/mp4","file_size":16384}
    }))
    .expect("live photo fixture parses");

    assert_eq!(media.kind(), PollMediaKind::LivePhoto);
    assert_eq!(media.tag(), "live_photo");
    let live_photo = media.live_photo_value().expect("live photo member");
    assert_eq!(live_photo.duration, 3);
    assert_eq!(live_photo.width, 1080);
    assert_eq!(live_photo.photo.as_ref().map(Vec::len), Some(1));
    assert!(media.location_value().is_none());
}

#[test]
fn location_member() {
    let media: PollMedia = parse(json!({"location":{"latitude":51.05,"longitude":13.74}}))
        .expect("location fixture parses");

    assert_eq!(media.kind(), PollMediaKind::Location);
    assert_eq!(media.tag(), "location");
    let location = media.location_value().expect("location member");
    assert!((location.latitude - 51.05).abs() < f64::EPSILON);
    assert!((location.longitude - 13.74).abs() < f64::EPSILON);
    assert!(media.live_photo_value().is_none());
}

#[test]
fn empty_and_multi_member_objects_are_rejected() {
    let empty = serde_json::from_str::<PollMedia>("{}")
        .expect_err("poll media requires exactly one member");
    assert!(empty.to_string().contains("exactly one member"));

    let multiple = parse::<PollMedia>(json!({
        "link":{"url":"https://example.com"},
        "location":{"latitude":0.0,"longitude":0.0}
    }))
    .expect_err("poll media accepts only one member");
    assert!(multiple.to_string().contains("exactly one member"));

    let alias_duplicate = parse::<PollMedia>(json!({
        "live_photo":{"file_id":"A","file_unique_id":"u","width":1,"height":1,"duration":0},
        "livePhoto":{"file_id":"B","file_unique_id":"v","width":1,"height":1,"duration":0}
    }))
    .expect_err("an alias of the same member is a duplicate");
    assert!(alias_duplicate.to_string().contains("exactly one member"));
}

#[test]
fn empty_photo_arrays_are_rejected() {
    assert!(parse::<PollMedia>(json!({"photo":[]})).is_err());
    assert!(PollMedia::photo(Vec::new()).is_err());
}

#[test]
fn unknown_member_is_preserved_with_its_payload_and_extra_fields() {
    let input = json!({
        "future_media":{"nested":[1,true],"name":"new"},
        "later_field":{"kept":true}
    });
    let media: PollMedia = parse(input.clone()).expect("unknown member parses");

    assert_eq!(media.kind(), PollMediaKind::Unknown);
    assert_eq!(media.tag(), "future_media");
    assert_eq!(
        media.unknown_value(),
        Some(&json!({"nested":[1,true],"name":"new"}))
    );
    assert_eq!(media.extra_fields()["later_field"], json!({"kept":true}));
    assert_eq!(serde_json::to_value(&media).expect("serializable"), input);
}

#[test]
fn known_member_keeps_adjacent_fields() {
    let input = json!({
        "link":{"url":"https://example.com/x"},
        "future_field":{"nested":true}
    });
    let media: PollMedia = parse(input.clone()).expect("known member with extras parses");
    assert_eq!(media.kind(), PollMediaKind::Link);
    assert_eq!(media.extra_fields()["future_field"], json!({"nested":true}));
    assert_eq!(serde_json::to_value(&media).expect("serializable"), input);
}

#[test]
fn unknown_field_before_known_member_is_kept() {
    let input = r#"{"future_field":{"nested":true},"link":{"url":"https://example.com/x"}}"#;
    let media: PollMedia = serde_json::from_str(input).expect("known member with leading extra");

    assert_eq!(media.kind(), PollMediaKind::Link);
    assert_eq!(media.extra_fields()["future_field"], json!({"nested":true}));
    assert_eq!(
        serde_json::to_value(&media).expect("serializable"),
        serde_json::from_str::<Value>(input).expect("valid fixture")
    );
}

#[test]
fn constructors_round_trip_every_member() {
    let mut photo = PhotoSize::default();
    photo.file_id = "A".into();
    photo.file_unique_id = "u".into();
    photo.width = 1;
    photo.height = 1;
    let mut video = Video::default();
    video.file_id = "V".into();
    video.file_unique_id = "v".into();
    video.width = 1;
    video.height = 1;
    video.duration = 1;
    let cases = vec![
        (
            PollMedia::animation(Animation::default()),
            PollMediaKind::Animation,
        ),
        (PollMedia::audio(Audio::default()), PollMediaKind::Audio),
        (
            PollMedia::document(Document::default()),
            PollMediaKind::Document,
        ),
        (
            PollMedia::link({
                let mut link = Link::default();
                link.url = "https://example.com".into();
                link
            }),
            PollMediaKind::Link,
        ),
        (
            PollMedia::live_photo(LivePhoto::default()),
            PollMediaKind::LivePhoto,
        ),
        (
            PollMedia::location(Location::default()),
            PollMediaKind::Location,
        ),
        (
            PollMedia::photo(vec![photo]).expect("non-empty photo"),
            PollMediaKind::Photo,
        ),
        (
            PollMedia::sticker(Sticker::default()),
            PollMediaKind::Sticker,
        ),
        (PollMedia::venue(Venue::default()), PollMediaKind::Venue),
        (PollMedia::video(video), PollMediaKind::Video),
    ];

    for (media, kind) in cases {
        assert_eq!(media.kind(), kind);
        let encoded = serde_json::to_string(&media).expect("constructor output serializes");
        let decoded: PollMedia = serde_json::from_str(&encoded).expect("round trip parses");
        assert_eq!(decoded, media);
    }
}

#[test]
fn poll_carries_media_and_explanation_media() {
    let poll: Poll = parse(json!({
        "id":"p1","question":"Where?","options":[{"text":"PATO","voter_count":1}],
        "media":{"photo":[{"file_id":"AgAC","file_unique_id":"AQAD","width":320,
                  "height":240,"file_size":12345}]},
        "explanation_media":{"link":{"url":"https://example.com/x"}}
    }))
    .expect("poll with both media fields parses");

    assert_eq!(
        poll.media.as_ref().map(PollMedia::kind),
        Some(PollMediaKind::Photo)
    );
    assert_eq!(
        poll.explanation_media.as_ref().map(PollMedia::kind),
        Some(PollMediaKind::Link)
    );
}

#[test]
fn poll_option_carries_media() {
    let option: PollOption = parse(json!({
        "text":"PATO","voter_count":1,
        "media":{"venue":{"location":{"latitude":51.05,"longitude":13.74},"title":"PATO",
                 "address":"Koeniginstraße 34"}}
    }))
    .expect("poll option with media parses");

    assert_eq!(option.text, "PATO");
    let media = option.media.as_ref().expect("option media");
    assert_eq!(media.kind(), PollMediaKind::Venue);
    assert_eq!(
        media.venue_value().map(|venue| venue.title.as_str()),
        Some("PATO")
    );
}

#[test]
fn camel_case_members_are_tolerated_with_complete_required_fields() {
    let poll: Poll = parse(json!({
        "media":{"livePhoto":{"fileId":"A","fileUniqueId":"u","width":1,"height":1,"duration":0}},
        "explanationMedia":{"document":{"fileId":"B","fileUniqueId":"v"}}
    }))
    .expect("camelCase poll media parses");

    let media = poll.media.as_ref().expect("media");
    assert_eq!(media.kind(), PollMediaKind::LivePhoto);
    assert_eq!(
        media.live_photo_value().map(|photo| photo.file_id.as_str()),
        Some("A")
    );
    assert_eq!(
        poll.explanation_media.as_ref().map(PollMedia::kind),
        Some(PollMediaKind::Document)
    );
}

fn photos(count: usize) -> Value {
    Value::Array(
        (0..count)
            .map(|index| {
                json!({"file_id":format!("file-{index}"),"file_unique_id":"u",
                       "width":1,"height":1})
            })
            .collect(),
    )
}

fn qualities(count: usize) -> Value {
    Value::Array(
        (0..count)
            .map(|index| {
                json!({"file_id":format!("quality-{index}"),"file_unique_id":"u",
                       "width":1,"height":1,"codec":"h264"})
            })
            .collect(),
    )
}

type CollectionDocument = (&'static str, Value, fn(Value) -> bool);

fn collection_documents(count: usize) -> [CollectionDocument; 4] {
    fn parses<T: DeserializeOwned>(value: Value) -> bool {
        parse::<T>(value).is_ok()
    }

    [
        (
            "PollMedia.photo",
            json!({"photo":photos(count)}),
            parses::<PollMedia>,
        ),
        (
            "LivePhoto.photo",
            json!({"file_id":"A","file_unique_id":"u","width":1,"height":1,
                   "duration":0,"photo":photos(count)}),
            parses::<LivePhoto>,
        ),
        (
            "Video.cover",
            json!({"file_id":"A","file_unique_id":"u","width":1,"height":1,
                   "duration":0,"cover":photos(count)}),
            parses::<Video>,
        ),
        (
            "Video.qualities",
            json!({"file_id":"A","file_unique_id":"u","width":1,"height":1,
                   "duration":0,"qualities":qualities(count)}),
            parses::<Video>,
        ),
    ]
}

#[test]
fn collection_limits_accept_100_and_reject_101() {
    for (label, accepted, parses) in collection_documents(MAX_MEDIA_COLLECTION_ITEMS) {
        assert!(parses(accepted), "{label} accepts the local limit");
    }
    for (label, rejected, parses) in collection_documents(MAX_MEDIA_COLLECTION_ITEMS + 1) {
        assert!(!parses(rejected), "{label} rejects one item over the limit");
    }
}

// ---------------------------------------------------------------------------
// Numeric members are non-negative (issue #60).
// ---------------------------------------------------------------------------

/// Declares one bounded numeric member: `-1` must be refused with an error that
/// names the member, `0` and the type maximum must parse.
macro_rules! bounded_member {
    ($name:ident, $ty:ty, $template:literal, $field:literal, $max:ident) => {
        #[test]
        fn $name() {
            let parse = |value: &str| -> Result<$ty, serde_json::Error> {
                serde_json::from_str(&$template.replace("{v}", value))
            };

            let refused = parse("-1").expect_err("a negative count must not parse");
            assert!(
                refused.to_string().contains(&format!("`{}`", $field)),
                "the rejection names `{}`: {refused}",
                $field
            );

            assert!(parse("0").is_ok(), "0 is the inclusive lower bound");
            assert!(parse($max).is_ok(), "{} is the inclusive upper bound", $max);
        }
    };
}

const WIRE_U32_MAX: &str = "4294967295";
const WIRE_U64_MAX: &str = "18446744073709551615";

bounded_member!(
    photo_size_width_is_non_negative,
    PhotoSize,
    r#"{"file_id":"A","file_unique_id":"u","width":{v},"height":1}"#,
    "width",
    WIRE_U32_MAX
);
bounded_member!(
    photo_size_height_is_non_negative,
    PhotoSize,
    r#"{"file_id":"A","file_unique_id":"u","width":1,"height":{v}}"#,
    "height",
    WIRE_U32_MAX
);
bounded_member!(
    photo_size_file_size_is_non_negative,
    PhotoSize,
    r#"{"file_id":"A","file_unique_id":"u","width":1,"height":1,"file_size":{v}}"#,
    "file_size",
    WIRE_U64_MAX
);
bounded_member!(
    live_photo_width_is_non_negative,
    LivePhoto,
    r#"{"file_id":"A","file_unique_id":"u","width":{v},"height":1,"duration":0}"#,
    "width",
    WIRE_U32_MAX
);
bounded_member!(
    live_photo_height_is_non_negative,
    LivePhoto,
    r#"{"file_id":"A","file_unique_id":"u","width":1,"height":{v},"duration":0}"#,
    "height",
    WIRE_U32_MAX
);
bounded_member!(
    live_photo_duration_is_non_negative,
    LivePhoto,
    r#"{"file_id":"A","file_unique_id":"u","width":1,"height":1,"duration":{v}}"#,
    "duration",
    WIRE_U64_MAX
);
bounded_member!(
    live_photo_file_size_is_non_negative,
    LivePhoto,
    r#"{"file_id":"A","file_unique_id":"u","width":1,"height":1,"duration":0,"file_size":{v}}"#,
    "file_size",
    WIRE_U64_MAX
);
bounded_member!(
    animation_width_is_non_negative,
    Animation,
    r#"{"file_id":"A","file_unique_id":"u","width":{v},"height":1,"duration":0}"#,
    "width",
    WIRE_U32_MAX
);
bounded_member!(
    animation_height_is_non_negative,
    Animation,
    r#"{"file_id":"A","file_unique_id":"u","width":1,"height":{v},"duration":0}"#,
    "height",
    WIRE_U32_MAX
);
bounded_member!(
    animation_duration_is_non_negative,
    Animation,
    r#"{"file_id":"A","file_unique_id":"u","width":1,"height":1,"duration":{v}}"#,
    "duration",
    WIRE_U64_MAX
);
bounded_member!(
    animation_file_size_is_non_negative,
    Animation,
    r#"{"file_id":"A","file_unique_id":"u","width":1,"height":1,"duration":0,"file_size":{v}}"#,
    "file_size",
    WIRE_U64_MAX
);
bounded_member!(
    audio_duration_is_non_negative,
    Audio,
    r#"{"file_id":"A","file_unique_id":"u","duration":{v}}"#,
    "duration",
    WIRE_U64_MAX
);
bounded_member!(
    audio_file_size_is_non_negative,
    Audio,
    r#"{"file_id":"A","file_unique_id":"u","duration":0,"file_size":{v}}"#,
    "file_size",
    WIRE_U64_MAX
);
bounded_member!(
    document_file_size_is_non_negative,
    Document,
    r#"{"file_id":"A","file_unique_id":"u","file_size":{v}}"#,
    "file_size",
    WIRE_U64_MAX
);
bounded_member!(
    video_quality_width_is_non_negative,
    VideoQuality,
    r#"{"file_id":"A","file_unique_id":"u","width":{v},"height":1,"codec":"h264"}"#,
    "width",
    WIRE_U32_MAX
);
bounded_member!(
    video_quality_height_is_non_negative,
    VideoQuality,
    r#"{"file_id":"A","file_unique_id":"u","width":1,"height":{v},"codec":"h264"}"#,
    "height",
    WIRE_U32_MAX
);
bounded_member!(
    video_quality_file_size_is_non_negative,
    VideoQuality,
    r#"{"file_id":"A","file_unique_id":"u","width":1,"height":1,"codec":"h264","file_size":{v}}"#,
    "file_size",
    WIRE_U64_MAX
);
bounded_member!(
    video_width_is_non_negative,
    Video,
    r#"{"file_id":"A","file_unique_id":"u","width":{v},"height":1,"duration":0}"#,
    "width",
    WIRE_U32_MAX
);
bounded_member!(
    video_height_is_non_negative,
    Video,
    r#"{"file_id":"A","file_unique_id":"u","width":1,"height":{v},"duration":0}"#,
    "height",
    WIRE_U32_MAX
);
bounded_member!(
    video_duration_is_non_negative,
    Video,
    r#"{"file_id":"A","file_unique_id":"u","width":1,"height":1,"duration":{v}}"#,
    "duration",
    WIRE_U64_MAX
);
bounded_member!(
    video_start_timestamp_is_non_negative,
    Video,
    r#"{"file_id":"A","file_unique_id":"u","width":1,"height":1,"duration":0,"start_timestamp":{v}}"#,
    "start_timestamp",
    WIRE_U64_MAX
);
bounded_member!(
    video_file_size_is_non_negative,
    Video,
    r#"{"file_id":"A","file_unique_id":"u","width":1,"height":1,"duration":0,"file_size":{v}}"#,
    "file_size",
    WIRE_U64_MAX
);
bounded_member!(
    sticker_width_is_non_negative,
    Sticker,
    r#"{"file_id":"A","file_unique_id":"u","type":"regular","width":{v},"height":1,"is_animated":false,"is_video":false}"#,
    "width",
    WIRE_U32_MAX
);
bounded_member!(
    sticker_height_is_non_negative,
    Sticker,
    r#"{"file_id":"A","file_unique_id":"u","type":"regular","width":1,"height":{v},"is_animated":false,"is_video":false}"#,
    "height",
    WIRE_U32_MAX
);
bounded_member!(
    sticker_file_size_is_non_negative,
    Sticker,
    r#"{"file_id":"A","file_unique_id":"u","type":"regular","width":1,"height":1,"is_animated":false,"is_video":false,"file_size":{v}}"#,
    "file_size",
    WIRE_U64_MAX
);

#[test]
fn dimensions_above_u32_max_are_rejected_not_truncated() {
    let refused = serde_json::from_str::<PhotoSize>(
        r#"{"file_id":"A","file_unique_id":"u","width":4294967296,"height":1}"#,
    )
    .expect_err("a width that does not fit u32 must not parse");
    assert!(refused.to_string().contains("`width`"));
}

#[test]
fn camel_case_sizes_are_validated_too() {
    assert!(
        serde_json::from_str::<Document>(r#"{"fileId":"A","fileUniqueId":"u","fileSize":0}"#)
            .is_ok()
    );
    let refused =
        serde_json::from_str::<Document>(r#"{"fileId":"A","fileUniqueId":"u","fileSize":-1}"#)
            .expect_err("the camelCase alias is the same member and is checked too");
    assert!(refused.to_string().contains("file_size"));
}

#[test]
fn a_negative_member_inside_poll_media_is_rejected() {
    let refused = serde_json::from_str::<Poll>(
        r#"{"media":{"photo":[{"file_id":"A","file_unique_id":"u","width":320,"height":1,"file_size":-1}]}}"#,
    )
    .expect_err("the wire path through PollMedia is validated as well");
    assert!(refused.to_string().contains("file_size"), "{refused}");
}

#[test]
fn a_negative_member_surfaces_as_a_payload_error() {
    let guard = WebhookGuard::new("s3cret").expect("a non-empty secret");
    let body = br#"{"update_id":1,"poll":{"media":{"photo":[{"file_id":"A","file_unique_id":"u","width":-1,"height":1}]}}}"#;

    let error = guard
        .verify_and_parse(Some("s3cret"), guard.derived_path(), body)
        .expect_err("a negative width cannot be decoded");

    assert!(matches!(error, Error::Payload { .. }), "{error:?}");
    assert!(error.to_string().contains("`width`"), "{error}");
}

#[test]
fn a_negative_poll_option_member_is_a_payload_error_that_names_the_member() {
    let guard = WebhookGuard::new("s3cret").expect("a non-empty secret");
    let body = br#"{"update_id":1,"poll":{"options":[{"text":"a","media":{"photo":[{"file_id":"A","file_unique_id":"u","width":1,"height":1,"fileSize":-1}]}}]}}"#;

    let error = guard
        .verify_and_parse(Some("s3cret"), guard.derived_path(), body)
        .expect_err("a negative file size cannot be decoded");

    match error {
        Error::Payload { message, source } => {
            assert!(message.contains("`file_size`"), "{message}");
            let source = source.expect("the parser error is kept as the source");
            assert!(source.to_string().contains("`file_size`"), "{source}");
        }
        other => panic!("expected Error::Payload, got {other:?}"),
    }
}

#[test]
fn unsigned_bounds_survive_a_round_trip() {
    let mut video = Video::default();
    video.file_id = "A".into();
    video.file_unique_id = "u".into();
    video.width = u32::MAX;
    video.duration = u64::MAX;
    video.start_timestamp = Some(0);
    video.file_size = Some(u64::MAX);
    let json = serde_json::to_string(&video).expect("the bounds serialize");
    assert!(!json.contains('-'));
    let back: Video = serde_json::from_str(&json).expect("the bounds parse back");
    assert_eq!(back, video);
}

// ---------------------------------------------------------------------------
// MaskPosition (issue #61).
// ---------------------------------------------------------------------------

#[test]
fn mask_points_round_trip_known_and_unknown_values() {
    for (wire, point) in [
        ("forehead", MaskPoint::Forehead),
        ("eyes", MaskPoint::Eyes),
        ("mouth", MaskPoint::Mouth),
        ("chin", MaskPoint::Chin),
        ("future", MaskPoint::Unknown("future".into())),
    ] {
        assert_eq!(point.as_str(), wire);
        let position = MaskPosition::new(point.clone(), -0.25, 0.5, 2.0);
        let encoded = serde_json::to_value(&position).expect("valid position serializes");
        assert_eq!(encoded["point"], wire);
        let decoded: MaskPosition =
            serde_json::from_value(encoded).expect("preserved point parses");
        assert_eq!(decoded, position);
    }
}

#[test]
fn mask_position_rejects_invalid_floats_before_serializing() {
    let valid = MaskPosition::new(MaskPoint::Eyes, 0.0, 0.0, 1.0);
    assert!(valid.validate().is_ok());
    assert!(serde_json::to_value(&valid).is_ok());

    for (label, mutate) in [
        (
            "x NaN",
            (|p: &mut MaskPosition| p.x_shift = f64::NAN) as fn(&mut MaskPosition),
        ),
        ("x infinity", |p| p.x_shift = f64::INFINITY),
        ("y NaN", |p| p.y_shift = f64::NAN),
        ("y negative infinity", |p| p.y_shift = f64::NEG_INFINITY),
        ("zero scale", |p| p.scale = 0.0),
        ("negative scale", |p| p.scale = -1.0),
        ("NaN scale", |p| p.scale = f64::NAN),
        ("infinite scale", |p| p.scale = f64::INFINITY),
    ] {
        let mut position = valid.clone();
        mutate(&mut position);
        assert!(position.validate().is_err(), "{label} must be refused");
        assert!(
            serde_json::to_value(&position).is_err(),
            "{label} must not serialize"
        );
    }
}

#[test]
fn mask_position_rejects_empty_points_and_non_positive_scales() {
    for point in [
        MaskPoint::Unknown(String::new()),
        MaskPoint::Unknown(" ".into()),
    ] {
        let position = MaskPosition::new(point, 0.0, 0.0, 1.0);
        if position.point.as_str().is_empty() {
            assert!(position.validate().is_err());
        } else {
            assert!(
                position.validate().is_ok(),
                "unknown non-empty points survive"
            );
        }
    }

    for scale in [0.0, -1.0] {
        let position = MaskPosition::new(MaskPoint::Eyes, 0.0, 0.0, scale);
        assert!(position.validate().is_err());
    }
}

#[test]
fn mask_position_decoding_refuses_invalid_values() {
    for json in [
        r#"{"point":"","x_shift":0.0,"y_shift":0.0,"scale":1.0}"#,
        r#"{"point":"eyes","x_shift":0.0,"y_shift":0.0,"scale":0.0}"#,
        r#"{"point":"eyes","x_shift":0.0,"y_shift":0.0,"scale":-1.0}"#,
    ] {
        assert!(
            serde_json::from_str::<MaskPosition>(json).is_err(),
            "{json}"
        );
    }
}

#[test]
fn mask_position_equality_is_reflexive_for_nan() {
    let nan = MaskPosition::new(MaskPoint::Eyes, f64::NAN, 0.0, 1.0);
    assert_eq!(nan, nan, "bitwise comparison keeps Eq sound for NaN");

    let mut shifted = nan.clone();
    shifted.x_shift = 0.0;
    assert_ne!(nan, shifted, "a zero shift is not NaN");
}

use proptest::prelude::*;

fn member_mask_document(flags: u16) -> Value {
    let mut object = serde_json::Map::new();
    if flags & (1 << 0) != 0 {
        object.insert(
            "animation".into(),
            json!({"file_id":"A","file_unique_id":"u","width":0,"height":0,"duration":0}),
        );
    }
    if flags & (1 << 1) != 0 {
        object.insert(
            "audio".into(),
            json!({"file_id":"A","file_unique_id":"u","duration":0}),
        );
    }
    if flags & (1 << 2) != 0 {
        object.insert(
            "document".into(),
            json!({"file_id":"A","file_unique_id":"u"}),
        );
    }
    if flags & (1 << 3) != 0 {
        object.insert("link".into(), json!({"url":"https://example.com"}));
    }
    if flags & (1 << 4) != 0 {
        object.insert(
            "live_photo".into(),
            json!({"file_id":"A","file_unique_id":"u","width":0,"height":0,"duration":0}),
        );
    }
    if flags & (1 << 5) != 0 {
        object.insert("location".into(), json!({"latitude":0.0,"longitude":0.0}));
    }
    if flags & (1 << 6) != 0 {
        object.insert(
            "photo".into(),
            json!([{"file_id":"A","file_unique_id":"u","width":0,"height":0}]),
        );
    }
    if flags & (1 << 7) != 0 {
        object.insert("sticker".into(), json!({"file_id":"A","file_unique_id":"u","type":"regular","width":0,"height":0,"is_animated":false,"is_video":false}));
    }
    if flags & (1 << 8) != 0 {
        object.insert(
            "venue".into(),
            json!({"location":{"latitude":0.0,"longitude":0.0},"title":"T","address":"A"}),
        );
    }
    if flags & (1 << 9) != 0 {
        object.insert(
            "video".into(),
            json!({"file_id":"A","file_unique_id":"u","width":0,"height":0,"duration":0}),
        );
    }
    Value::Object(object)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 96, ..ProptestConfig::default() })]

    #[test]
    fn member_masks_parse_exactly_when_they_name_one_member(
        flags in any::<u16>().prop_map(|value| value & 0x03ff)
    ) {
        let parsed = parse::<PollMedia>(member_mask_document(flags));
        prop_assert_eq!(parsed.is_ok(), flags.is_power_of_two(), "mask {:#012b}", flags);
    }

    #[test]
    fn bounded_collections_accept_every_size_through_the_limit(
        count in 0usize..=MAX_MEDIA_COLLECTION_ITEMS
    ) {
        for (label, document, parses) in collection_documents(count) {
            prop_assert_eq!(count != 0 || !label.starts_with("PollMedia"), parses(document),
                "{} rejected {} items", label, count);
        }
    }

    #[test]
    fn bounded_collections_refuse_more_than_the_limit(
        count in (MAX_MEDIA_COLLECTION_ITEMS + 1)..=200usize
    ) {
        for (label, document, parses) in collection_documents(count) {
            prop_assert!(!parses(document), "{} accepted {} items", label, count);
        }
    }

    #[test]
    fn finite_mask_positions_round_trip(
        x in -1000.0f64..1000.0,
        y in -1000.0f64..1000.0,
        scale in 0.001f64..1000.0
    ) {
        let position = MaskPosition::new(MaskPoint::Chin, x, y, scale);
        let encoded = serde_json::to_string(&position)?;
        let decoded: MaskPosition = serde_json::from_str(&encoded)?;
        prop_assert_eq!(decoded.point, position.point);
        prop_assert!((decoded.x_shift - position.x_shift).abs() <= position.x_shift.abs() * 1e-15);
        prop_assert!((decoded.y_shift - position.y_shift).abs() <= position.y_shift.abs() * 1e-15);
        prop_assert!((decoded.scale - position.scale).abs() <= position.scale.abs() * 1e-15);
    }
}
