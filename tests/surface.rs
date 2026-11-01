//! Surface coverage: every public builder and accessor is exercised at least
//! once, so "it compiles" is not the only thing known about it.
//!
//! These are not behaviour tests — the behaviour lives in the module test files.
//! Each assertion only checks the value the builder was supposed to set, which is
//! enough to keep the whole public surface honest (and covered).

use std::time::Duration;

use serde_json::json;
use telegram_api_client::send::MAX_FILENAME_BYTES;
use telegram_api_client::{
    parse_retry_after, AnswerCallbackQuery, AnswerInlineQuery, BanChatMember, CallbackQuery,
    ChatAction, ChatId, ChatType, CopyMessage, DeleteMessage, DeleteWebhook, DeliveryOptions,
    EditMessageCaption, EditMessageMedia, EditMessageReplyMarkup, EditMessageText, EditTarget,
    EntityKind, Error, File, ForwardMessage, GetChatMember, GetUpdates, InlineKeyboardButton,
    InlineKeyboardMarkup, InlineQuery, InlineQueryResult, InlineQueryResultsButton, InputMedia,
    InputMediaKind, KeyboardButton, LinkPreviewOptions, MediaOptions, MenuButton, Message,
    MessageEntity, MessageOptions, MockTransport, MultipartFile, MultipartForm, NoDelay, ParseMode,
    PinChatMessage, PollType, ReplyKeyboardMarkup, ReplyKeyboardRemove, ReplyMarkup, RetryPolicy,
    SendAnimation, SendAudio, SendChatAction, SendContact, SendDice, SendDocument, SendLocation,
    SendMediaGroup, SendMessage, SendPhoto, SendPoll, SendVenue, SendVideo, SendVoice,
    SetChatMenuButton, SetWebhook, Sleeper, TelegramClient, ThreadSleeper, UnpinChatMessage,
    Update, UpdateCursor, UploadFile, User, WebAppInfo, WebhookInfo,
};

fn markup() -> ReplyMarkup {
    ReplyMarkup::inline(InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback("Yes", "yes"),
    ]]))
}

#[test]
fn keyboards_round_trip_in_every_variant() {
    let inline = ReplyMarkup::inline(InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback("A", "a"),
        InlineKeyboardButton::url("Docs", "https://example.com"),
    ]]));
    let keyboard = ReplyMarkup::keyboard(
        ReplyKeyboardMarkup::new(vec![vec![KeyboardButton::text("B")
            .requesting_contact()
            .requesting_location()]])
        .resized()
        .one_time(),
    );
    let remove = ReplyMarkup::remove();
    let force = ReplyMarkup::force_reply();

    for markup in [inline, keyboard, remove, force] {
        let encoded = serde_json::to_value(&markup).expect("serialize");
        let decoded: ReplyMarkup = serde_json::from_value(encoded.clone()).expect("deserialize");
        assert_eq!(decoded, markup, "{encoded}");
    }

    let entity = MessageEntity::bold(0, 3).with_language("de");
    assert_eq!(entity.language.as_deref(), Some("de"));
    assert!(ReplyKeyboardRemove::default().remove_keyboard);
}

#[test]
fn delivery_and_message_options_compose() {
    let options = DeliveryOptions::default()
        .silent()
        .protected()
        .reply_to(7)
        .with_reply_markup(markup());
    let json = serde_json::to_value(&options).expect("serialize");
    assert_eq!(json["disable_notification"], json!(true));
    assert_eq!(json["protect_content"], json!(true));
    assert_eq!(json["reply_to_message_id"], json!(7));
    assert!(json["reply_markup"].is_object());

    let entities = vec![MessageEntity::bold(0, 3)];
    let message = MessageOptions::default()
        .with_parse_mode(ParseMode::MarkdownV2)
        .with_entities(entities.clone())
        .with_link_preview(LinkPreviewOptions::disabled())
        .silent()
        .reply_to(9)
        .with_reply_markup(markup());
    let json = serde_json::to_value(&message).expect("serialize");
    assert_eq!(json["parse_mode"], json!("MarkdownV2"));
    assert_eq!(json["entities"][0]["type"], json!("bold"));
    assert_eq!(json["link_preview_options"]["is_disabled"], json!(true));
    assert_eq!(json["reply_to_message_id"], json!(9));

    let media = MediaOptions::default()
        .with_caption("cap")
        .with_parse_mode(ParseMode::Html)
        .silent()
        .reply_to(3)
        .with_reply_markup(markup());
    let json = serde_json::to_value(&media).expect("serialize");
    assert_eq!(json["caption"], json!("cap"));
    assert_eq!(json["reply_to_message_id"], json!(3));

    // Empty captions are rejected, an acceptable one passes.
    assert!(MediaOptions::default()
        .with_caption(" ")
        .validate()
        .is_err());
    assert!(media.validate().is_ok());
}

#[test]
fn send_message_builders_compose() {
    let request = SendMessage::new(ChatId::username("@floffel"), "hi")
        .with_parse_mode(ParseMode::Html)
        .with_entities(vec![MessageEntity::kind(EntityKind::Bold, 0, 2)])
        .with_link_preview(LinkPreviewOptions::disabled())
        .silent()
        .reply_to(4)
        .with_reply_markup(markup());
    let json = serde_json::to_value(&request).expect("serialize");
    assert_eq!(json["chat_id"], json!("floffel"));
    assert_eq!(json["parse_mode"], json!("HTML"));
    assert_eq!(json["entities"][0]["offset"], json!(0));
    assert_eq!(json["disable_notification"], json!(true));
    assert!(request.validate().is_ok());
}

#[test]
fn every_media_builder_composes() {
    let buttons =
        ReplyMarkup::keyboard(ReplyKeyboardMarkup::new(vec![vec![KeyboardButton::text(
            "A",
        )
        .requesting_contact()]]));
    let silent = ReplyMarkup::remove();

    let photo = SendPhoto::new(-1001_i64, "file-id")
        .with_caption("c")
        .with_parse_mode(ParseMode::MarkdownV2)
        .silent()
        .reply_to(1)
        .with_reply_markup(buttons.clone());
    let json = serde_json::to_value(&photo).expect("serialize");
    assert_eq!(json["caption"], json!("c"));
    assert!(json["reply_markup"]["keyboard"].is_array());
    assert!(photo.multipart_form().is_none(), "json send");

    let document = SendDocument::new(-1001_i64, "file-id").with_reply_markup(silent);
    assert_eq!(document.options.delivery.disable_notification, None);
    let audio = SendVoice::new(-1001_i64, "file-id").silent();
    assert_eq!(audio.options.delivery.disable_notification, Some(true));
    let video = SendVideo::new(-1001_i64, "file-id").reply_to(2);
    assert_eq!(video.options.delivery.reply_to_message_id, Some(2));
    let animation = SendAnimation::new(-1001_i64, "file-id").with_parse_mode(ParseMode::Html);
    assert_eq!(animation.options.parse_mode, Some(ParseMode::Html));
    let _audio = SendAudio::new(-1001_i64, "file-id").with_caption("a");

    let album = SendMediaGroup::new(
        -1001_i64,
        vec![InputMedia::photo("a"), InputMedia::document("b")],
    )
    .silent()
    .reply_to(5);
    assert!(album.validate().is_ok());
    assert!(album.multipart_form().is_none(), "no uploads, no multipart");

    let location = SendLocation::new(-1001_i64, 48.1, 11.5).silent();
    assert!(location.validate().is_ok());
    let venue = SendVenue::new(-1001_i64, 48.1, 11.5, "V", "A").silent();
    assert!(venue.validate().is_ok());
    let contact = SendContact::new(-1001_i64, "+49151", "Flo").silent();
    assert!(contact.validate().is_ok());
    let poll = SendPoll::new(-1001_i64, "Q?", vec!["a".to_string(), "b".to_string()]).silent();
    assert_eq!(poll.kind, Some(PollType::Regular));
    let dice = SendDice::new(-1001_i64).silent();
    assert!(dice.validate().is_ok());
    assert_eq!(PollType::Regular.as_str(), "regular");
}

#[test]
fn upload_inspection_helpers() {
    let upload = UploadFile::new("a.png", vec![1, 2, 3]);
    assert_eq!(upload.len(), 3);
    assert!(!upload.is_empty());
    assert!(upload.mime.contains("image"));
    // `Debug` shows the size, not the bytes.
    let debug = format!("{upload:?}");
    assert!(debug.contains("3 bytes"), "{debug}");

    let part = upload.to_part("photo");
    assert_eq!(part.len(), 3);
    assert!(!part.is_empty());
    assert!(format!("{part:?}").contains("3 bytes"));

    let empty = UploadFile::new("a.png", Vec::new());
    assert!(empty.is_empty(), "no bytes at all");

    assert!(upload.validate_for("document", &[]).is_ok());
}

#[test]
fn mime_guessing_covers_the_common_extensions() {
    let cases = [
        ("a.png", "image/png"),
        ("a.jpeg", "image/jpeg"),
        ("a.gif", "image/gif"),
        ("a.webp", "image/webp"),
        ("a.heic", "image/heic"),
        ("a.tgs", "application/x-tgsticker"),
        ("a.webm", "video/webm"),
        ("a.mp4", "video/mp4"),
        ("a.mov", "video/quicktime"),
        ("a.m4v", "video/x-m4v"),
        ("a.mp3", "audio/mpeg"),
        ("a.m4a", "audio/mp4"),
        ("a.oga", "audio/ogg"),
        ("a.opus", "audio/ogg"),
        ("a.wav", "audio/wav"),
        ("a.flac", "audio/flac"),
        ("a.pdf", "application/pdf"),
        ("a.zip", "application/zip"),
        ("a.gz", "application/gzip"),
        ("a.json", "application/json"),
        ("a.csv", "text/csv"),
        ("a.txt", "text/plain"),
        ("a.md", "text/plain"),
        ("a.unknown", "application/octet-stream"),
    ];
    for (name, expected) in cases {
        assert_eq!(telegram_api_client::guess_mime(name), expected, "{name}");
    }
}

#[test]
fn an_overlong_file_name_is_rejected() {
    let upload = UploadFile::new("a".repeat(MAX_FILENAME_BYTES + 1), vec![1]);
    assert!(matches!(
        upload.validate(),
        Err(Error::InvalidRequest { field: "file", .. })
    ));
    // Exactly at the limit is fine.
    let ok = UploadFile::new("a".repeat(MAX_FILENAME_BYTES), vec![1]);
    assert!(ok.validate().is_ok());
}

#[test]
fn every_input_media_kind_has_mime_families() {
    assert_eq!(InputMediaKind::Photo.mime_families(), ["image/"]);
    assert_eq!(InputMediaKind::Video.mime_families(), ["video/"]);
    assert_eq!(
        InputMediaKind::Animation.mime_families(),
        ["image/", "video/"]
    );
    assert_eq!(InputMediaKind::Audio.mime_families(), ["audio/"]);
    assert_eq!(InputMediaKind::Document.mime_families(), [] as [&str; 0]);
    assert_eq!(
        InputMediaKind::Unknown("hologram".into()).mime_families(),
        [] as [&str; 0]
    );

    assert!(InputMediaKind::Animation.accepts_mime("video/mp4"));
    assert!(InputMediaKind::Audio.accepts_mime("audio/ogg"));
    assert!(!InputMediaKind::Audio.accepts_mime("video/mp4"));
    assert!(InputMediaKind::Document.accepts_mime("application/pdf"));
    assert!(InputMediaKind::Unknown("x".into()).accepts_mime("text/plain"));

    let item = InputMedia::document("file-id")
        .with_parse_mode(ParseMode::Html)
        .with_caption("c");
    assert_eq!(item.parse_mode, Some(ParseMode::Html));
}

#[test]
fn edit_builders_compose() {
    let text = EditMessageText::for_inline("inline-1", "new")
        .with_parse_mode(ParseMode::Html)
        .with_entities(vec![MessageEntity::bold(0, 3)])
        .with_link_preview(LinkPreviewOptions::disabled())
        .with_reply_markup(markup());
    assert_eq!(text.target.inline_message_id.as_deref(), Some("inline-1"));
    assert!(text.validate().is_ok());

    let caption = EditMessageCaption::for_inline("inline-1")
        .with_caption("c")
        .with_parse_mode(ParseMode::Html)
        .with_reply_markup(markup());
    assert!(caption.validate().is_ok());

    let media = EditMessageMedia::for_inline("inline-1", InputMedia::photo("AgACAgI"))
        .with_caption("c")
        .with_reply_markup(markup());
    assert!(media.validate().is_ok());
    assert!(media.multipart_form().is_none());

    let empty = EditMessageMedia::new(1_i64, 7, InputMedia::photo(""));
    assert!(matches!(
        empty.validate(),
        Err(Error::InvalidRequest { field: "media", .. })
    ));

    let keyboard = EditMessageReplyMarkup::for_inline("inline-1").with_reply_markup(markup());
    assert!(keyboard.validate().is_ok());

    let forward = ForwardMessage::new(1_i64, -1001_i64, 7).silent();
    assert!(forward.validate().is_ok());

    let copy = CopyMessage::new(1_i64, -1001_i64, 7)
        .with_caption("c")
        .silent();
    assert!(copy.validate().is_ok());
    let bad_copy = CopyMessage::new(1_i64, -1001_i64, 0);
    assert!(matches!(
        bad_copy.validate(),
        Err(Error::InvalidRequest {
            field: "message_id",
            ..
        })
    ));

    let delete = DeleteMessage::new(1_i64, 7);
    assert!(delete.validate().is_ok());

    let target = EditTarget::chat(1_i64, 7);
    assert_eq!(target.message_id, Some(7));
}

#[test]
fn callback_and_menu_builders_compose() {
    let answer = AnswerCallbackQuery::new("id-1").with_text("ok").alert();
    assert!(answer.validate().is_ok());
    assert!(AnswerCallbackQuery::new(" ").validate().is_err());

    let raw = InlineQueryResult::new(json!({"type": "article", "id": "1"}));
    assert_eq!(raw.value["type"], json!("article"));

    let button = InlineQueryResultsButton::start("Go", "param");
    assert_eq!(button.start_parameter.as_deref(), Some("param"));

    let inline = AnswerInlineQuery::new("q-1", vec![raw])
        .with_cache_time(30)
        .personal();
    assert_eq!(inline.cache_time, Some(30));
    assert!(inline.validate().is_ok());
    assert!(AnswerInlineQuery::new("", Vec::new()).validate().is_err());

    assert_eq!(
        WebAppInfo::new("https://example.com/app").url,
        "https://example.com/app"
    );

    // Every menu button serializes and round-trips, including unknown kinds.
    for button in [
        MenuButton::default_menu(),
        MenuButton::commands(),
        MenuButton::web_app("Open", "https://example.com/app"),
        MenuButton::Unknown(json!({ "type": "future" })),
    ] {
        let encoded = serde_json::to_value(&button).expect("serialize");
        let decoded: MenuButton = serde_json::from_value(encoded.clone()).expect("deserialize");
        assert_eq!(decoded, button, "{encoded}");
    }

    let global = SetChatMenuButton::global(MenuButton::Commands);
    assert!(global.validate().is_ok());
    let scoped = SetChatMenuButton::for_chat(1_i64, MenuButton::Default);
    assert_eq!(scoped.chat_id, Some(ChatId::Id(1)));
    assert!(scoped.validate().is_ok());
}

#[test]
fn chat_administration_builders_compose() {
    let ban = BanChatMember::new(-1001_i64, 42)
        .until(1_800_000_000)
        .revoking_messages();
    assert_eq!(ban.until_date, Some(1_800_000_000));
    assert!(ban.validate().is_ok());
    assert!(matches!(
        BanChatMember::new(-1001_i64, 0).validate(),
        Err(Error::InvalidRequest {
            field: "user_id",
            ..
        })
    ));
    assert!(matches!(
        GetChatMember::new(-1001_i64, -1).validate(),
        Err(Error::InvalidRequest {
            field: "user_id",
            ..
        })
    ));

    let pin = PinChatMessage::new(-1001_i64, 7).silent();
    assert!(pin.validate().is_ok());
    assert!(matches!(
        PinChatMessage::new(-1001_i64, 0).validate(),
        Err(Error::InvalidRequest {
            field: "message_id",
            ..
        })
    ));

    let unpin = UnpinChatMessage::new(-1001_i64).message(7);
    assert_eq!(unpin.message_id, Some(7));
    assert!(unpin.validate().is_ok());
    assert!(matches!(
        UnpinChatMessage::new(-1001_i64).message(0).validate(),
        Err(Error::InvalidRequest {
            field: "message_id",
            ..
        })
    ));

    let action = SendChatAction::new(-1001_i64, ChatAction::Unknown(String::new()));
    assert!(matches!(
        action.validate(),
        Err(Error::InvalidRequest {
            field: "action",
            ..
        })
    ));
    assert!(SendChatAction::typing(-1001_i64).validate().is_ok());
}

#[test]
fn webhook_and_polling_builders_compose() {
    let webhook = SetWebhook::new("https://example.com/hook")
        .with_secret("s3cret")
        .max_connections(40)
        .allowed_updates(vec![
            telegram_api_client::UpdateKind::Message,
            telegram_api_client::UpdateKind::CallbackQuery,
        ])
        .dropping_pending();
    assert_eq!(webhook.max_connections, Some(40));
    assert!(webhook.validate().is_ok());
    // An unknown update kind (empty string) is rejected, an empty list is not.
    assert!(SetWebhook::new("https://example.com/hook")
        .allowed_updates(vec![
            telegram_api_client::UpdateKind::Unknown(String::new())
        ])
        .validate()
        .is_err());

    let delete = DeleteWebhook::new();
    assert_eq!(delete.drop_pending_updates, None);
    assert!(delete.validate().is_ok());

    let info: WebhookInfo =
        serde_json::from_value(json!({ "url": "https://x", "last_error_message": "boom" }))
            .expect("webhook info");
    assert!(info.is_registered());
    assert!(info.has_error());

    let updates = GetUpdates::new()
        .offset(5)
        .limit(50)
        .timeout(25)
        .allowed_updates(vec![telegram_api_client::UpdateKind::Message]);
    assert!(updates.validate().is_ok());
    assert!(GetUpdates::new()
        .allowed_updates(vec![
            telegram_api_client::UpdateKind::Unknown(String::new())
        ])
        .validate()
        .is_err());
    assert!(GetUpdates::new().offset(-1).validate().is_err());
    assert!(GetUpdates::new().limit(0).validate().is_err());

    let mut cursor = UpdateCursor::after(10);
    assert_eq!(cursor.next_offset(), Some(11));
    let confirmed = cursor.confirm(&[]);
    assert_eq!(confirmed.len(), 0);
}

#[test]
fn update_accessors_answer_for_every_shape() {
    // Display names fall back from username to id, and from first name to @user.
    let user: User = serde_json::from_value(json!({ "id": 5, "username": "flo" })).expect("user");
    assert_eq!(user.display_name(), "flo");
    let user: User = serde_json::from_value(json!({ "id": 5, "first_name": "Flo" })).expect("user");
    assert_eq!(user.display_name(), "Flo");
    let user: User = serde_json::from_value(json!({ "id": 5 })).expect("user");
    assert_eq!(user.display_name(), "5");

    let chat: telegram_api_client::Chat =
        serde_json::from_value(json!({ "id": 5, "username": "chan" })).expect("chat");
    assert_eq!(chat.label(), "@chan");
    let chat: telegram_api_client::Chat =
        serde_json::from_value(json!({ "id": 5, "type": "private" })).expect("chat");
    assert_eq!(chat.label(), "5");

    let message: Message = serde_json::from_value(json!({
        "message_id": 1,
        "date": 1,
        "chat": { "id": 5, "type": "private" },
        "text": "hi",
    }))
    .expect("message");
    let update: Update =
        serde_json::from_value(json!({ "update_id": 1, "message": message })).expect("update");
    assert!(update.message.is_some());

    let callback: CallbackQuery = serde_json::from_value(json!({
        "id": "cb-1",
        "from": { "id": 5 },
        "chat_instance": "ci",
        "data": "payload",
    }))
    .expect("callback");
    assert_eq!(callback.payload(), Some("payload"));

    let inline: InlineQuery = serde_json::from_value(json!({
        "id": "iq-1",
        "from": { "id": 5 },
        "query": "text",
        "offset": "0",
    }))
    .expect("inline query");
    assert_eq!(inline.query, "text");

    let update: Update =
        serde_json::from_value(json!({ "update_id": 1, "message": message })).expect("update");
    assert_eq!(update.text(), Some("hi"));
    assert_eq!(update.chat_id(), Some(5));

    let update: Update = serde_json::from_value(json!({
        "update_id": 2,
        "callback_query": {
            "id": "cb-2",
            "from": { "id": 5 },
            "chat_instance": "ci",
            "data": "clicked",
        }
    }))
    .expect("update with callback");
    assert_eq!(update.callback_data(), Some("clicked"));
    assert_eq!(update.callback_query_id(), Some("cb-2"));
}

#[test]
fn client_accessors_and_file_urls() {
    let client =
        TelegramClient::new("1:A", MockTransport::new()).with_base_url("https://proxy.test/");
    assert_eq!(client.base_url(), "https://proxy.test/");

    let file = File {
        file_path: Some("/photos/a b.jpg".into()),
        ..File::default()
    };
    let url = client.file_url(&file).expect("download url");
    assert!(url.starts_with("https://proxy.test/file/bot1:A/"), "{url}");

    let error = client.file_url(&File::default()).expect_err("no path");
    assert!(matches!(error, Error::Unexpected { .. }));
}

#[test]
fn error_helpers_cover_the_edges() {
    let with_source =
        Error::ambiguous("sendMessage", "timeout").with_source(std::io::Error::other("io"));
    assert!(with_source.is_ambiguous());
    assert!(std::error::Error::source(&with_source).is_some());

    // A variant without a source slot ignores the attachment (issue #41).
    let api = Error::Api {
        code: 400,
        description: "nope".into(),
    }
    .with_source(std::io::Error::other("io"));
    assert!(matches!(api, Error::Api { .. }));

    let limited = Error::RateLimited {
        retry_after: None,
        code: 429,
        description: String::new(),
    };
    assert!(limited.to_string().contains("retry later"));
    assert_eq!(parse_retry_after("retry_after 4"), Some(4));
    assert_eq!(parse_retry_after("retry after 0"), None);

    // Chat ids render for logs and for the wire.
    assert_eq!(ChatId::Id(7).to_string(), "7");
    assert_eq!(ChatId::Username("x".into()).to_string(), "x");
    assert_eq!(ChatId::from("y".to_string()), ChatId::username("y"));
    assert_eq!(ChatId::Id(7).as_id(), Some(7));
    assert_eq!(ChatId::Id(7).as_username(), None);
    assert_eq!(ChatId::username("@a").as_username(), Some("a"));

    // Enum Display impls are used in messages and logs.
    assert_eq!(ChatType::Private.to_string(), "private");
    assert_eq!(ParseMode::Html.to_string(), "HTML");
}

#[test]
fn transport_helpers_report_their_shape() {
    let file = MultipartFile::new("photo", "a.png", "image/png", vec![1]);
    assert_eq!(file.len(), 1);
    assert!(!file.is_empty());
    assert!(MultipartFile::new("photo", "a", "image/png", Vec::new()).is_empty());

    let form = MultipartForm::new().field("chat_id", 1).with_file(file);
    assert_eq!(form.fields().len(), 1);
    assert!(!form.is_empty());
    assert!(format!("{form:?}").contains("chat_id"));
    assert!(
        MultipartForm::new().is_empty(),
        "a fresh form has nothing in it"
    );

    let transport = MockTransport::new();
    assert!(transport.is_empty(), "nothing sent yet");
    assert_eq!(transport.len(), 0);
    let reply: telegram_api_client::MockReply = json!({ "message_id": 1 }).into();
    assert!(reply.into_result().is_ok());
}

#[tokio::test]
async fn the_default_sleepers_both_complete() {
    NoDelay.sleep(Duration::from_secs(60)).await;
    let start = std::time::Instant::now();
    ThreadSleeper.sleep(Duration::from_millis(5)).await;
    assert!(start.elapsed() >= Duration::from_millis(5));

    // A policy with a single attempt never retries.
    let policy = RetryPolicy::new(1, Duration::from_millis(1), Duration::from_millis(2));
    let limited = Error::RateLimited {
        retry_after: Some(1),
        code: 429,
        description: String::new(),
    };
    assert!(!policy.should_retry(1, &limited));
}

/// Every `ChatMember` shape and every accessor on it — plus both ways a caller
/// can build one by hand — so a variant that stops answering, or answers for
/// the wrong status, fails here first.
#[test]
fn chat_member_accessors_cover_every_shape() {
    use serde_json::Map;
    use telegram_api_client::{ChatMember, ChatMemberOwner, ChatMemberStatus, User};

    let owner: ChatMember = serde_json::from_str(
        r#"{"status":"creator","user":{"id":1},"is_anonymous":false,"custom_title":"Owner"}"#,
    )
    .unwrap();
    assert_eq!(owner.status(), ChatMemberStatus::Creator);
    assert_eq!(owner.user().id, 1);
    assert_eq!(owner.is_anonymous(), Some(false));
    assert_eq!(owner.custom_title(), Some("Owner"));
    assert_eq!(owner.until_date(), None);
    assert_eq!(owner.is_member(), None);
    assert!(owner.is_present());

    let admin: ChatMember =
        serde_json::from_str(r#"{"status":"administrator","user":{"id":1},"is_anonymous":true}"#)
            .unwrap();
    assert_eq!(admin.status(), ChatMemberStatus::Administrator);
    assert_eq!(admin.is_anonymous(), Some(true));
    assert_eq!(admin.custom_title(), None);
    assert_eq!(admin.until_date(), None);
    assert!(admin.is_present());

    let member: ChatMember =
        serde_json::from_str(r#"{"status":"member","user":{"id":1},"until_date":1700000001}"#)
            .unwrap();
    assert_eq!(member.status(), ChatMemberStatus::Member);
    assert_eq!(member.until_date(), Some(1_700_000_001));
    assert_eq!(member.is_member(), None);
    assert_eq!(member.custom_title(), None);
    assert!(member.is_present());

    let restricted: ChatMember = serde_json::from_str(
        r#"{"status":"restricted","user":{"id":1},"is_member":true,"until_date":1700000002}"#,
    )
    .unwrap();
    assert_eq!(restricted.status(), ChatMemberStatus::Restricted);
    assert_eq!(restricted.until_date(), Some(1_700_000_002));
    assert_eq!(restricted.is_member(), Some(true));
    assert_eq!(restricted.custom_title(), None);
    assert!(restricted.is_present());

    let left: ChatMember = serde_json::from_str(r#"{"status":"left","user":{"id":1}}"#).unwrap();
    assert_eq!(left.status(), ChatMemberStatus::Left);
    assert_eq!(left.until_date(), None);
    assert_eq!(left.is_member(), None);
    assert_eq!(left.is_anonymous(), None);
    assert!(!left.is_present());

    let banned: ChatMember =
        serde_json::from_str(r#"{"status":"kicked","user":{"id":1},"until_date":0}"#).unwrap();
    assert_eq!(banned.status(), ChatMemberStatus::Kicked);
    assert_eq!(banned.until_date(), Some(0));
    assert_eq!(banned.custom_title(), None);
    assert!(!banned.is_present());

    // The payload structs are public: a caller builds the one it needs.
    let built = ChatMember::Creator(ChatMemberOwner {
        user: User {
            id: 9,
            ..User::default()
        },
        custom_title: Some("Owner".to_string()),
        ..ChatMemberOwner::default()
    });
    assert_eq!(built.custom_title(), Some("Owner"));
    assert!(built.is_present());

    // The unmodelled shape keeps its payload; `status` and `user` inside `extra`
    // are ignored, because the variant owns them.
    let mut extra = Map::new();
    extra.insert("status".to_string(), json!("member"));
    extra.insert("user".to_string(), json!({"id": 99}));
    extra.insert("haunting_since".to_string(), json!(5));
    let ghost = ChatMember::Unknown {
        status: ChatMemberStatus::Unknown("ghost".to_string()),
        user: User {
            id: 3,
            ..User::default()
        },
        extra,
    };
    let rendered = serde_json::to_value(&ghost).unwrap();
    assert_eq!(rendered["status"], "ghost");
    assert_eq!(rendered["user"]["id"], 3);
    assert_eq!(rendered["haunting_since"], 5);
}
