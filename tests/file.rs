//! `getFile` request and response hardening.

use serde_json::{json, Value};
use std::collections::BTreeMap;
use telegram_api_client::{
    Error, File, GetFile, MockTransport, TelegramClient, MAX_DOWNLOAD_BYTES, MAX_FILE_ID_BYTES,
};

#[test]
fn required_file_identifiers_decode_and_round_trip() {
    let file: File = serde_json::from_value(json!({
        "file_id": "AgACAgI",
        "file_unique_id": "unique",
        "file_size": 1024,
        "file_path": "photos/a.jpg",
    }))
    .unwrap();
    assert_eq!(file.file_id, "AgACAgI");
    assert_eq!(file.file_unique_id, "unique");
    assert_eq!(file.file_size, Some(1024));
    assert_eq!(file.download_path(), Some("photos/a.jpg"));
    assert_eq!(
        serde_json::to_value(&file).unwrap(),
        json!({
            "file_id": "AgACAgI",
            "file_unique_id": "unique",
            "file_size": 1024,
            "file_path": "photos/a.jpg",
        })
    );
}

#[test]
fn file_identifiers_cannot_be_missing_empty_padded_control_or_oversized() {
    for payload in [
        json!({}),
        json!({"file_id": "A"}),
        json!({"file_unique_id": "u"}),
        json!({"file_id": "", "file_unique_id": "u"}),
        json!({"file_id": "A", "file_unique_id": ""}),
        json!({"file_id": " A", "file_unique_id": "u"}),
        json!({"file_id": "A", "file_unique_id": "u "}),
        json!({"file_id": "A\u{0000}", "file_unique_id": "u"}),
        json!({"file_id": "A", "file_unique_id": "u\n"}),
        json!({"file_id": "A".repeat(MAX_FILE_ID_BYTES + 1), "file_unique_id": "u"}),
    ] {
        assert!(
            serde_json::from_value::<File>(payload.clone()).is_err(),
            "accepted {payload}"
        );
    }

    for id in ["A", "a".repeat(MAX_FILE_ID_BYTES).as_str()] {
        let payload = json!({"file_id": id, "file_unique_id": "u"});
        assert!(serde_json::from_value::<File>(payload).is_ok(), "{id}");
    }
}

#[test]
fn get_file_accepts_an_id_of_exactly_the_bound() {
    let at_limit = "a".repeat(MAX_FILE_ID_BYTES);
    assert!(GetFile::new(&at_limit).validate().is_ok());
    let over = "a".repeat(MAX_FILE_ID_BYTES + 1);
    let error = GetFile::new(over).validate().unwrap_err();
    assert!(matches!(
        error,
        Error::InvalidRequest {
            field: "file_id",
            ..
        }
    ));
}

#[test]
fn get_file_rejects_padded_control_and_empty_ids() {
    for id in ["", " ", " A", "A ", "A\t", "A\u{0000}", "A\u{007f}"] {
        let error = GetFile::new(id).validate().unwrap_err();
        assert!(
            matches!(
                error,
                Error::InvalidRequest {
                    field: "file_id",
                    ..
                }
            ),
            "{id:?}: {error:?}"
        );
    }
}

#[test]
fn unknown_file_fields_are_preserved_including_nested_values() {
    let original = json!({
        "file_id": "A",
        "file_unique_id": "u",
        "future_string": "kept",
        "future_object": {"nested": [1, true, null]},
    });
    let file: File = serde_json::from_value(original.clone()).unwrap();
    assert_eq!(file.extra.get("future_string"), Some(&Value::from("kept")));
    assert_eq!(serde_json::to_value(&file).unwrap(), original);
}

#[test]
fn aliases_and_absent_optional_fields_are_supported() {
    let file: File = serde_json::from_value(json!({
        "fileId": "A",
        "fileUniqueId": "u",
    }))
    .unwrap();
    assert_eq!(file.file_id, "A");
    assert_eq!(file.file_unique_id, "u");
    assert_eq!(file.file_size, None);
    assert_eq!(file.file_path, None);
    assert!(!file.is_downloadable());
}

#[test]
fn unsafe_paths_are_rejected_during_decoding() {
    for path in [
        "",
        " ",
        "/absolute",
        "a/../b",
        "a/./b",
        "a//b",
        "a?query",
        "a#fragment",
        "a\\b",
        "a\tb",
        "a\u{0000}b",
        "a /b",
        "a/ b",
    ] {
        let result = serde_json::from_value::<File>(json!({
            "file_id": "A",
            "file_unique_id": "u",
            "file_path": path,
        }));
        assert!(result.is_err(), "accepted {path:?}");
    }
}

#[test]
fn download_urls_percent_encode_each_segment() {
    let client =
        TelegramClient::new("1:A", MockTransport::new()).with_base_url("https://proxy.test/base/");
    let file: File = serde_json::from_value(json!({
        "file_id": "A",
        "file_unique_id": "u",
        "file_path": "photos/Grüße 2026.jpg",
    }))
    .unwrap();
    assert_eq!(
        client.file_url(&file).unwrap(),
        "https://proxy.test/base/file/bot1:A/photos/Gr%C3%BC%C3%9Fe%202026.jpg"
    );
}

#[test]
fn manually_constructed_unsafe_paths_never_report_downloadable() {
    let file = File {
        file_id: "A".into(),
        file_unique_id: "u".into(),
        file_size: Some(1),
        file_path: Some("../secret".into()),
        extra: BTreeMap::default(),
    };
    assert_eq!(file.download_path(), None);
    assert!(!file.is_downloadable());
    assert_eq!(file.fits_download_limit(), Some(true));
    let client = TelegramClient::new("1:A", MockTransport::new());
    assert!(matches!(
        client.file_url(&file),
        Err(Error::Unexpected { .. })
    ));
}

#[test]
fn download_limit_is_inclusive_and_unknown_stays_unknown() {
    let decode = |size: Option<u64>| {
        let mut payload = json!({"file_id": "A", "file_unique_id": "u"});
        if let Some(size) = size {
            payload["file_size"] = json!(size);
        }
        serde_json::from_value::<File>(payload).unwrap()
    };
    assert_eq!(decode(None).fits_download_limit(), None);
    assert_eq!(decode(Some(0)).fits_download_limit(), Some(true));
    assert_eq!(
        decode(Some(MAX_DOWNLOAD_BYTES as u64)).fits_download_limit(),
        Some(true)
    );
    assert_eq!(
        decode(Some(MAX_DOWNLOAD_BYTES as u64 + 1)).fits_download_limit(),
        Some(false)
    );
    assert_eq!(decode(Some(u64::MAX)).fits_download_limit(), Some(false));
}
