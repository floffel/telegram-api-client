//! Multipart uploads: the request shape Telegram expects, plus the edges that
//! must never reach the wire.

use serde_json::json;
use telegram_api_client::{
    EditMessageMedia, Error, InputMedia, MockReply, MockTransport, SendDocument, SendMediaGroup,
    SendPhoto, TelegramClient, UploadFile, MAX_UPLOAD_BYTES,
};

const TOKEN: &str = "123:ABC";
const CHAT: i64 = -1001;

fn message() -> serde_json::Value {
    json!({
        "message_id": 7,
        "chat": { "id": CHAT, "type": "supergroup" },
        "date": 1_700_000_000,
    })
}

fn client() -> (TelegramClient<MockTransport>, MockTransport) {
    let transport = MockTransport::new();
    transport.push(MockReply::result(message()));
    (
        TelegramClient::new(TOKEN, transport.clone()).with_base_url("https://tg.test"),
        transport,
    )
}

// ---- the happy path -------------------------------------------------------

#[tokio::test]
async fn send_photo_uploads_bytes_as_multipart() {
    let (client, transport) = client();
    let request = SendPhoto::new(CHAT, "ignored")
        .with_upload(UploadFile::new("Grüße.png", vec![0x89, 0x50, 0x4e, 0x47]))
        .with_caption("sky");

    client.send_photo(&request).await.expect("upload succeeds");

    let calls = transport.calls();
    assert_eq!(calls.len(), 1);
    let call = &calls[0];
    assert!(call.is_multipart(), "an upload must not be sent as JSON");
    assert_eq!(call.method(), "sendPhoto");

    // Text fields travel as form fields, not in a JSON body.
    assert_eq!(call.body["chat_id"], json!("-1001"));
    assert_eq!(call.body["caption"], json!("sky"));
    assert!(
        call.body.get("photo").is_none(),
        "the reference field is replaced by the file part"
    );

    // The file part keeps name, bytes and MIME type.
    let file = &call.files()[0];
    assert_eq!(file.field, "photo");
    assert_eq!(file.filename, "Grüße.png");
    assert_eq!(file.mime, "image/png");
    assert_eq!(
        call.file_bytes("photo"),
        Some(&[0x89, 0x50, 0x4e, 0x47][..])
    );
}

#[tokio::test]
async fn the_encoded_body_is_valid_multipart() {
    let (client, transport) = client();
    let request = SendPhoto::new(CHAT, "x").with_upload(UploadFile::new("a.png", vec![1, 2, 3]));
    client.send_photo(&request).await.expect("upload succeeds");

    let form = transport
        .last_call()
        .expect("one call")
        .multipart
        .clone()
        .expect("multipart body");
    let (boundary, body) = form.to_http_body();
    let text = String::from_utf8_lossy(&body);

    assert!(text.starts_with(&format!("--{boundary}\r\n")));
    assert!(text.ends_with(&format!("--{boundary}--\r\n")));
    assert!(text.contains("name=\"chat_id\""));
    assert!(text.contains("name=\"photo\"; filename=\"a.png\""));
    assert!(text.contains("Content-Type: image/png"));
    // Exactly one occurrence of the closing delimiter, at the very end.
    assert_eq!(text.matches(&format!("--{boundary}--")).count(), 1);
}

#[tokio::test]
async fn a_file_id_reference_stays_json() {
    let (client, transport) = client();
    client
        .send_photo(&SendPhoto::new(CHAT, "AgACAgIAAxkBAAI"))
        .await
        .expect("json send");

    let call = transport.last_call().expect("one call");
    assert!(!call.is_multipart());
    assert_eq!(call.body["photo"], json!("AgACAgIAAxkBAAI"));
    assert_eq!(call.files().len(), 0);
}

#[tokio::test]
async fn a_document_upload_accepts_any_mime_type() {
    let (client, transport) = client();
    let request = SendDocument::new(CHAT, "x").with_upload(UploadFile::new(
        "Vertrag 2026.pdf",
        vec![0x25, 0x50, 0x44, 0x46],
    ));

    client
        .send_document(&request)
        .await
        .expect("document upload");

    let call = transport.last_call().expect("one call");
    assert_eq!(call.files()[0].mime, "application/pdf");
    assert_eq!(call.files()[0].filename, "Vertrag 2026.pdf");
}

#[tokio::test]
async fn a_file_on_disk_is_read_and_named() {
    let path = std::env::temp_dir().join(format!(
        "telegram-api-client-test-{}.png",
        std::process::id()
    ));
    std::fs::write(&path, [0x89, 0x50, 0x4e, 0x47]).expect("write fixture");

    let upload = UploadFile::from_path(&path).expect("read fixture");
    assert_eq!(upload.mime, "image/png");
    assert_eq!(upload.len(), 4);
    assert_eq!(
        std::path::Path::new(&upload.filename)
            .extension()
            .and_then(std::ffi::OsStr::to_str),
        Some("png")
    );
    assert!(
        !upload.filename.contains('/'),
        "only the file name is sent: {}",
        upload.filename
    );

    let (client, transport) = client();
    client
        .send_photo(&SendPhoto::new(CHAT, "x").with_upload(upload))
        .await
        .expect("upload from disk");
    assert!(transport.last_call().expect("one call").is_multipart());

    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn a_media_group_attaches_every_upload_in_order() {
    let transport = MockTransport::new();
    transport.push(MockReply::result(json!([message(), message()])));
    let client = TelegramClient::new(TOKEN, transport.clone()).with_base_url("https://tg.test");

    let album = SendMediaGroup::new(
        CHAT,
        vec![
            InputMedia::photo("AgACAgI"),
            InputMedia::photo("x").with_upload(UploadFile::new("b.png", vec![2])),
            InputMedia::video("y").with_upload(UploadFile::new("c.mp4", vec![3])),
        ],
    );
    client.send_media_group(&album).await.expect("album");

    let call = transport.last_call().expect("one call");
    assert!(call.is_multipart());
    let fields: Vec<&str> = call
        .files()
        .iter()
        .map(|file| file.field.as_str())
        .collect();
    assert_eq!(fields, ["file1", "file2"]);

    let media = call.body["media"].as_str().expect("media is a JSON string");
    assert!(media.contains("attach://file1"));
    assert!(media.contains("attach://file2"));
    assert!(
        media.contains("\"media\":\"AgACAgI\""),
        "json items stay: {media}"
    );
}

#[tokio::test]
async fn edit_message_media_uploads_the_new_media() {
    let (client, transport) = client();
    transport.push(MockReply::result(message()));

    let edit = EditMessageMedia::new(
        CHAT,
        7,
        InputMedia::photo("x").with_upload(UploadFile::new("new.png", vec![9, 9])),
    );
    client.edit_message_media(&edit).await.expect("edit");

    let call = transport.last_call().expect("one call");
    assert_eq!(call.method(), "editMessageMedia");
    assert!(call.is_multipart());
    assert_eq!(call.files()[0].field, "file0");
    assert_eq!(call.body["message_id"], json!("7"));
    assert!(call.body["media"]
        .as_str()
        .expect("media json")
        .contains("attach://file0"));
}

#[tokio::test]
async fn edit_message_media_stays_json_without_upload() {
    let (client, transport) = client();
    transport.push(MockReply::result(message()));

    let edit = EditMessageMedia::new(CHAT, 7, InputMedia::video("DQACAgI"));
    client.edit_message_media(&edit).await.expect("edit");

    let call = transport.last_call().expect("one call");
    assert!(!call.is_multipart());
    assert_eq!(call.body["media"]["type"], json!("video"));
}

// ---- the edge matrix ------------------------------------------------------

#[test]
fn an_empty_file_is_rejected_before_the_wire() {
    let request = SendPhoto::new(CHAT, "x").with_upload(UploadFile::new("a.png", Vec::new()));
    match request.validate() {
        Err(Error::InvalidRequest { field, reason }) => {
            assert_eq!(field, "file", "the upload itself is the offending field");
            assert!(reason.contains("empty"), "{reason}");
        }
        other => panic!("expected InvalidRequest, got {other:?}"),
    }
}

#[test]
fn a_file_over_the_limit_is_rejected() {
    let huge = vec![0_u8; MAX_UPLOAD_BYTES + 1];
    let upload = UploadFile::new("big.pdf", huge);
    assert!(upload.validate().is_err());

    let request = SendDocument::new(CHAT, "x").with_upload(upload);
    assert!(matches!(
        request.validate(),
        Err(Error::InvalidRequest { field: "file", .. })
    ));
}

#[test]
fn a_file_exactly_at_the_limit_is_accepted() {
    let upload = UploadFile::new("exact.pdf", vec![0_u8; MAX_UPLOAD_BYTES]);
    assert!(upload.validate().is_ok(), "the limit itself is allowed");
}

#[test]
fn the_wrong_mime_family_is_rejected() {
    let pdf = UploadFile::new("a.pdf", vec![1]);
    let photo = SendPhoto::new(CHAT, "x").with_upload(pdf.clone());
    match photo.validate() {
        Err(Error::InvalidRequest { field, reason }) => {
            assert_eq!(field, "photo");
            assert!(reason.contains("application/pdf"), "{reason}");
        }
        other => panic!("expected InvalidRequest, got {other:?}"),
    }

    // The same file is fine as a document, and a video is fine as an animation.
    assert!(SendDocument::new(CHAT, "x")
        .with_upload(pdf)
        .validate()
        .is_ok());
}

#[test]
fn a_unicode_filename_survives_encoding() {
    let upload = UploadFile::new("Bäume 🌳.png", vec![1, 2]);
    assert!(upload.validate().is_ok());

    let form = SendPhoto::new(CHAT, "x")
        .with_upload(upload)
        .multipart_form()
        .expect("multipart form");
    let body = String::from_utf8(form.encode("BOUNDARY")).expect("utf8 body");

    assert!(body.contains("filename=\"Bäume 🌳.png\""), "{body}");
}

#[test]
fn a_filename_cannot_inject_headers() {
    let hostile = UploadFile::new("a\"\r\nX-Evil: 1.png", vec![1]);
    assert!(matches!(
        hostile.validate(),
        Err(Error::InvalidRequest { field: "file", .. })
    ));

    // Even a transport-level part built by hand cannot break out of the header:
    // the encoder neutralises quotes and line breaks.
    let form = telegram_api_client::MultipartForm::new().with_file(
        telegram_api_client::MultipartFile::new(
            "photo",
            "a\"\r\nX-Evil: 1.png",
            "image/png",
            vec![1],
        ),
    );
    let body = String::from_utf8(form.encode("B")).expect("utf8");
    assert!(!body.contains("\r\nX-Evil"), "injected header: {body}");
}

#[test]
fn a_missing_chat_id_is_rejected() {
    let upload = UploadFile::new("a.png", vec![1]);
    let request = SendPhoto::new(0_i64, "x").with_upload(upload);
    assert!(matches!(
        request.validate(),
        Err(Error::InvalidRequest {
            field: "chat_id",
            ..
        })
    ));
}

#[test]
fn an_empty_file_name_is_rejected() {
    let upload = UploadFile::new("", vec![1]);
    assert!(matches!(
        upload.validate(),
        Err(Error::InvalidRequest { field: "file", .. })
    ));
}

#[test]
fn a_malformed_mime_type_is_rejected() {
    let upload = UploadFile::new("a.png", vec![1]).with_mime("not a mime");
    assert!(matches!(
        upload.validate(),
        Err(Error::InvalidRequest { field: "mime", .. })
    ));
}

#[tokio::test]
async fn an_invalid_upload_never_reaches_the_transport() {
    let (client, transport) = client();
    let request = SendPhoto::new(CHAT, "x").with_upload(UploadFile::new("a.png", Vec::new()));

    let error = client
        .send_photo(&request)
        .await
        .expect_err("rejected locally");
    assert!(matches!(error, Error::InvalidRequest { .. }));
    assert_eq!(transport.calls().len(), 0, "no HTTP call for a bad upload");
}

#[tokio::test]
async fn a_missing_from_disk_file_is_a_typed_error() {
    let error =
        UploadFile::from_path("/nonexistent/telegram-api-client.png").expect_err("missing file");
    assert!(matches!(error, Error::InvalidRequest { field: "file", .. }));
}

// ---- the encoder as a public API: every header value is neutralised --------

/// Lines in the header block that starts at the first `needle` after `from`.
fn header_lines(body: &str, from: usize) -> Vec<&str> {
    let rest = &body[from..];
    let end = rest.find("\r\n\r\n").expect("header block terminator");
    rest[..end].split("\r\n").collect()
}

#[test]
fn a_mime_type_cannot_inject_headers() {
    // Found by the `multipart` fuzz target: the field name and file name were
    // escaped, the MIME type was interpolated verbatim.
    let form = telegram_api_client::MultipartForm::new().with_file(
        telegram_api_client::MultipartFile::new(
            "photo",
            "a.png",
            "image/png\r\nX-Evil: 1",
            vec![1],
        ),
    );
    let body = String::from_utf8(form.encode("B")).expect("utf8");
    assert!(
        !body.contains("\r\nX-Evil: 1\r\n"),
        "injected header: {body}"
    );

    let start = body.find("--B\r\n").expect("delimiter") + "--B\r\n".len();
    let lines = header_lines(&body, start);
    assert_eq!(lines.len(), 2, "one line per header: {lines:?}");
    assert!(lines[1].starts_with("Content-Type: "), "{lines:?}");
    assert!(!lines[0].contains('\r') && !lines[0].contains('\n'));
}

#[test]
fn a_field_name_cannot_inject_headers() {
    let form = telegram_api_client::MultipartForm::new()
        .field("chat_id\r\nX-Evil: 1", "1")
        .field("name", "value");
    let body = String::from_utf8(form.encode("B")).expect("utf8");
    assert!(!body.contains("X-Evil: 1\r\n"), "injected header: {body}");

    let second = body.find("--B\r\n").expect("first delimiter") + "--B\r\n".len();
    let after_first = body[second..].find("--B\r\n").expect("second delimiter");
    let lines = header_lines(&body, second + after_first + "--B\r\n".len());
    assert_eq!(
        lines.len(),
        1,
        "a text field has one header line: {lines:?}"
    );
}

#[test]
fn body_content_keeps_its_line_breaks() {
    // Escaping is for headers only: a caption with newlines must arrive intact.
    let form = telegram_api_client::MultipartForm::new().field("caption", "line1\r\nline2");
    let body = String::from_utf8(form.encode("B")).expect("utf8");
    assert!(body.contains("line1\r\nline2"), "{body}");
}

#[test]
fn a_file_payload_arrives_byte_for_byte() {
    let payload: Vec<u8> = (0..=255).collect();
    let form = telegram_api_client::MultipartForm::new().with_file(
        telegram_api_client::MultipartFile::new(
            "document",
            "all.bin",
            "application/octet-stream",
            payload.clone(),
        ),
    );
    let (boundary, body) = form.to_http_body();
    let needle = payload.as_slice();
    assert!(
        body.windows(needle.len()).any(|window| window == needle),
        "payload missing from the encoded body (boundary {boundary})"
    );
}
