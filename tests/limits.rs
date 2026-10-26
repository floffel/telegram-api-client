//! Per-operation upload limits. Telegram caps photos at 10 MB and everything
//! else at 50 MB; a single flat constant would let a 30 MB "photo" pass local
//! validation and be rejected by the server, which is the failure the pre-flight
//! check exists to prevent.

use proptest::prelude::*;
use telegram_api_client::{
    EditMessageMedia, Error, File, InputMedia, InputMediaKind, SendAnimation, SendAudio,
    SendDocument, SendMediaGroup, SendPhoto, SendVideo, SendVoice, UploadFile, MAX_DOWNLOAD_BYTES,
    MAX_FILE_URL_BYTES, MAX_PHOTO_BYTES, MAX_PHOTO_URL_BYTES, MAX_UPLOAD_BYTES,
};

const CHAT: i64 = -1001;

/// A payload of exactly `len` bytes — the content never matters, only its size.
fn bytes(len: usize) -> Vec<u8> {
    vec![0_u8; len]
}

/// The `reason` of an `InvalidRequest`, panicking on any other outcome.
fn invalid_request(result: telegram_api_client::Result<()>) -> (String, String) {
    match result {
        Err(Error::InvalidRequest { field, reason }) => (field.to_string(), reason),
        other => panic!("expected InvalidRequest, got {other:?}"),
    }
}

// ---- photos: 10 MB --------------------------------------------------------

#[test]
fn a_photo_exactly_at_the_limit_is_accepted() {
    let request =
        SendPhoto::new(CHAT, "x").with_upload(UploadFile::new("a.png", bytes(MAX_PHOTO_BYTES)));
    assert!(request.validate().is_ok(), "10 MB is still a photo");
}

#[test]
fn a_photo_one_byte_over_the_limit_is_rejected_with_the_photo_limit() {
    let request =
        SendPhoto::new(CHAT, "x").with_upload(UploadFile::new("a.png", bytes(MAX_PHOTO_BYTES + 1)));
    let (field, reason) = invalid_request(request.validate());
    assert_eq!(field, "file", "the upload itself is the offending field");
    assert!(
        reason.contains(&MAX_PHOTO_BYTES.to_string()),
        "the message must name the photo limit, got: {reason}"
    );
}

#[test]
fn the_same_bytes_are_fine_as_a_non_photo() {
    // One byte over the photo ceiling is 40 MB below the document ceiling: the
    // type of the operation decides, not the bytes.
    let len = MAX_PHOTO_BYTES + 1;
    assert!(SendDocument::new(CHAT, "x")
        .with_upload(UploadFile::new("a.pdf", bytes(len)))
        .validate()
        .is_ok());
    assert!(SendVideo::new(CHAT, "x")
        .with_upload(UploadFile::new("a.mp4", bytes(len)))
        .validate()
        .is_ok());
    assert!(SendAudio::new(CHAT, "x")
        .with_upload(UploadFile::new("a.mp3", bytes(len)))
        .validate()
        .is_ok());
    assert!(SendAnimation::new(CHAT, "x")
        .with_upload(UploadFile::new("a.gif", bytes(len)))
        .validate()
        .is_ok());
    assert!(SendVoice::new(CHAT, "x")
        .with_upload(UploadFile::new("a.ogg", bytes(len)))
        .validate()
        .is_ok());
}

// ---- everything else: 50 MB ------------------------------------------------

#[test]
fn a_document_exactly_at_the_general_limit_is_accepted() {
    let file = UploadFile::new("a.pdf", bytes(MAX_UPLOAD_BYTES));
    assert!(file
        .validate_with_limit("document", &[], MAX_UPLOAD_BYTES)
        .is_ok());
}

#[test]
fn a_document_one_byte_over_the_general_limit_is_rejected() {
    let file = UploadFile::new("a.pdf", bytes(MAX_UPLOAD_BYTES + 1));
    let (field, reason) =
        invalid_request(file.validate_with_limit("document", &[], MAX_UPLOAD_BYTES));
    assert_eq!(field, "file");
    assert!(
        reason.contains(&MAX_UPLOAD_BYTES.to_string()),
        "the message must name the document limit, got: {reason}"
    );
}

// ---- albums and edits validate per item kind -------------------------------

#[test]
fn a_media_group_photo_item_uses_the_photo_limit() {
    let album = SendMediaGroup::new(
        CHAT,
        vec![
            InputMedia::photo("a"),
            InputMedia::photo("b")
                .with_upload(UploadFile::new("b.png", bytes(MAX_PHOTO_BYTES + 1))),
        ],
    );
    let (field, reason) = invalid_request(album.validate());
    assert_eq!(
        field, "file",
        "a size error names the upload, not the album"
    );
    assert!(reason.contains(&MAX_PHOTO_BYTES.to_string()), "{reason}");
}

#[test]
fn a_media_group_video_item_keeps_the_general_limit() {
    let album = SendMediaGroup::new(
        CHAT,
        vec![
            InputMedia::video("a"),
            InputMedia::video("b")
                .with_upload(UploadFile::new("b.mp4", bytes(MAX_PHOTO_BYTES + 1))),
        ],
    );
    assert!(album.validate().is_ok(), "a 10 MB video is not a photo");
}

#[test]
fn an_edited_photo_uses_the_photo_limit() {
    let edit = EditMessageMedia::new(
        CHAT,
        7,
        InputMedia::photo("x").with_upload(UploadFile::new("x.png", bytes(MAX_PHOTO_BYTES + 1))),
    );
    let (field, reason) = invalid_request(edit.validate());
    assert_eq!(field, "file", "a size error names the upload, not the edit");
    assert!(reason.contains(&MAX_PHOTO_BYTES.to_string()), "{reason}");

    let video = EditMessageMedia::new(
        CHAT,
        7,
        InputMedia::video("x").with_upload(UploadFile::new("x.mp4", bytes(MAX_PHOTO_BYTES + 1))),
    );
    assert!(video.validate().is_ok());
}

#[test]
fn every_media_kind_names_its_own_ceiling() {
    assert_eq!(InputMediaKind::Photo.upload_limit_bytes(), MAX_PHOTO_BYTES);
    for kind in [
        InputMediaKind::Video,
        InputMediaKind::Animation,
        InputMediaKind::Audio,
        InputMediaKind::Document,
        InputMediaKind::from("sticker"),
        InputMediaKind::Unknown("nonsense".to_string()),
    ] {
        assert_eq!(
            kind.upload_limit_bytes(),
            MAX_UPLOAD_BYTES,
            "{kind:?} is not a photo"
        );
    }
}

// ---- the ceilings the crate documents but cannot enforce -------------------

// Compile-time, not run-time: a later edit that flips a ceiling fails the
// build instead of quietly shipping a limit Telegram does not honour.
const _: () = {
    assert!(
        MAX_PHOTO_URL_BYTES < MAX_FILE_URL_BYTES,
        "5 MB for photos by URL, 20 MB for the rest"
    );
    assert!(
        MAX_PHOTO_BYTES < MAX_UPLOAD_BYTES,
        "10 MB for photo uploads, 50 MB for the rest"
    );
    assert!(
        MAX_PHOTO_BYTES < MAX_FILE_URL_BYTES,
        "the photo upload ceiling is below the URL ceiling"
    );
    assert!(
        MAX_FILE_URL_BYTES == MAX_DOWNLOAD_BYTES,
        "both are the Bot API 20 MB"
    );
};
// ---- the ceilings the crate documents but cannot enforce -------------------

#[test]
fn a_download_fits_the_ceiling_inclusively() {
    let at_limit = File {
        file_size: Some(MAX_DOWNLOAD_BYTES as u64),
        ..File::default()
    };
    assert_eq!(at_limit.fits_download_limit(), Some(true));

    let over = File {
        file_size: Some(MAX_DOWNLOAD_BYTES as u64 + 1),
        ..File::default()
    };
    assert_eq!(over.fits_download_limit(), Some(false));

    // Empty files and unknown sizes: Telegram reports `file_size: 0` for an
    // empty file, and omits the field when it does not know.
    let empty = File {
        file_size: Some(0),
        ..File::default()
    };
    assert_eq!(empty.fits_download_limit(), Some(true));
    assert_eq!(File::default().fits_download_limit(), None);
}

// ---- properties -----------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig { cases: 96, ..ProptestConfig::default() })]

    /// The ceiling is inclusive, and nothing below it is refused: for every
    /// length and every limit, acceptance is exactly `0 < len <= limit`.
    #[test]
    fn the_ceiling_is_inclusive_and_nothing_else(len in 0_usize..2048, limit in 1_usize..1024) {
        let file = UploadFile::new("a.bin", bytes(len));
        let accepted = file.validate_with_limit("file", &[], limit).is_ok();
        prop_assert_eq!(accepted, len > 0 && len <= limit);
    }

    /// A refusal names the limit that applied, so a caller can tell the photo
    /// ceiling from the document one without guessing.
    #[test]
    fn a_refusal_quotes_the_limit(len in 1_usize..2048, limit in 1_usize..1024) {
        let file = UploadFile::new("a.bin", bytes(len + limit));
        let (_, reason) = invalid_request(file.validate_with_limit("file", &[], limit));
        prop_assert!(reason.contains(&limit.to_string()), "{}", reason);
    }
}
