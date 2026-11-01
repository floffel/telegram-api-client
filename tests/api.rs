//! Feature completeness: every client method hits its own Bot API endpoint
//! and posts exactly the body the Bot API documents.

use serde_json::{json, Value};
use telegram_api_client::{
    AnswerCallbackQuery, AnswerInlineQuery, BanChatMember, CopyMessage, DeleteMessage,
    DeleteWebhook, EditMessageCaption, EditMessageReplyMarkup, EditMessageText, ForwardMessage,
    GetChat, GetChatMember, GetFile, GetUpdates, InlineQueryResult, InputMedia, MenuButton,
    MockReply, MockTransport, PinChatMessage, SendAnimation, SendAudio, SendChatAction,
    SendContact, SendDice, SendDocument, SendLocation, SendMediaGroup, SendMessage, SendPhoto,
    SendPoll, SendVenue, SendVideo, SendVoice, SetChatMenuButton, SetWebhook, TelegramClient,
    UnpinChatMessage,
};

const TOKEN: &str = "123:ABC";
const CHAT: i64 = -1001;

/// A plausible edited/sent message.
fn message_json() -> Value {
    json!({
        "message_id": 7,
        "chat": { "id": CHAT, "type": "supergroup" },
        "date": 1_700_000_000,
        "text": "hi",
    })
}

fn client_with(reply: MockReply) -> (TelegramClient<MockTransport>, MockTransport) {
    let transport = MockTransport::new();
    transport.push(reply);
    (
        TelegramClient::new(TOKEN, transport.clone()).with_base_url("https://tg.test"),
        transport,
    )
}

fn ok_message() -> (TelegramClient<MockTransport>, MockTransport) {
    client_with(MockReply::result(message_json()))
}

fn ok_bool() -> (TelegramClient<MockTransport>, MockTransport) {
    client_with(MockReply::result(json!(true)))
}

/// Asserts the one recorded call: full URL, method, exact body, then returns it.
#[track_caller]
fn only_call(transport: &MockTransport) -> telegram_api_client::Call {
    let calls = transport.calls();
    assert_eq!(calls.len(), 1, "expected exactly one call, got {calls:?}");
    calls.into_iter().next().unwrap_or_else(|| unreachable!())
}

#[track_caller]
#[allow(clippy::needless_pass_by_value)] // the body is compared, not mutated
fn assert_call(transport: &MockTransport, method: &str, body: Value) {
    let call = only_call(transport);
    assert_eq!(call.method(), method);
    assert_eq!(
        call.url,
        format!("https://tg.test/bot{TOKEN}/{method}"),
        "url mismatch"
    );
    assert_eq!(call.body, body, "body mismatch for {method}");
}

#[tokio::test]
async fn send_message_variants() {
    // Typed one-liner posts the minimal body.
    let transport = MockTransport::new();
    let client = TelegramClient::new(TOKEN, transport.clone()).with_base_url("https://tg.test");
    client.send_text(CHAT, "Hallo").await.unwrap();
    assert_call(
        &transport,
        "sendMessage",
        json!({"chat_id": CHAT, "text": "Hallo"}),
    );

    // Typed call carries formatting, thread and keyboard.
    let (client, transport) = ok_message();
    let request = SendMessage::new(CHAT, "Hallo")
        .with_parse_mode(telegram_api_client::ParseMode::Html)
        .silent()
        .reply_to(3)
        .with_reply_markup(telegram_api_client::ReplyMarkup::remove());
    client.send_message_request(&request).await.unwrap();
    assert_call(
        &transport,
        "sendMessage",
        json!({
            "chat_id": CHAT,
            "text": "Hallo",
            "parse_mode": "HTML",
            "disable_notification": true,
            "reply_to_message_id": 3,
            "reply_markup": {"remove_keyboard": true},
        }),
    );
}

#[tokio::test]
async fn media_methods() {
    let (client, transport) = ok_message();
    client
        .send_photo(&SendPhoto::new(CHAT, "photo-id").with_caption("cap"))
        .await
        .unwrap();
    assert_call(
        &transport,
        "sendPhoto",
        json!({"chat_id": CHAT, "photo": "photo-id", "caption": "cap"}),
    );

    let (client, transport) = ok_message();
    client
        .send_document(&SendDocument::new(CHAT, "doc-id").silent())
        .await
        .unwrap();
    assert_call(
        &transport,
        "sendDocument",
        json!({"chat_id": CHAT, "document": "doc-id", "disable_notification": true}),
    );

    let (client, transport) = ok_message();
    client
        .send_audio(&SendAudio::new(CHAT, "audio-id"))
        .await
        .unwrap();
    assert_call(
        &transport,
        "sendAudio",
        json!({"chat_id": CHAT, "audio": "audio-id"}),
    );

    let (client, transport) = ok_message();
    client
        .send_video(&SendVideo::new(CHAT, "video-id"))
        .await
        .unwrap();
    assert_call(
        &transport,
        "sendVideo",
        json!({"chat_id": CHAT, "video": "video-id"}),
    );

    let (client, transport) = ok_message();
    client
        .send_animation(&SendAnimation::new(CHAT, "anim-id"))
        .await
        .unwrap();
    assert_call(
        &transport,
        "sendAnimation",
        json!({"chat_id": CHAT, "animation": "anim-id"}),
    );

    let (client, transport) = ok_message();
    client
        .send_voice(&SendVoice::new(CHAT, "voice-id"))
        .await
        .unwrap();
    assert_call(
        &transport,
        "sendVoice",
        json!({"chat_id": CHAT, "voice": "voice-id"}),
    );
}

#[tokio::test]
async fn send_media_group_flattens_items_in_order() {
    let (client, transport) =
        client_with(MockReply::result(json!([message_json(), message_json()])));
    let album = SendMediaGroup::new(
        CHAT,
        vec![
            InputMedia::photo("a").with_caption("first"),
            InputMedia::video("b"),
        ],
    )
    .silent();

    let sent = client.send_media_group(&album).await.unwrap();
    assert_eq!(sent.len(), 2);
    assert_call(
        &transport,
        "sendMediaGroup",
        json!({
            "chat_id": CHAT,
            "disable_notification": true,
            "media": [
                {"type": "photo", "media": "a", "caption": "first"},
                {"type": "video", "media": "b"},
            ],
        }),
    );
}

#[tokio::test]
async fn location_venue_contact() {
    let (client, transport) = ok_message();
    client
        .send_location(&SendLocation::new(CHAT, 48.7665, 11.4258))
        .await
        .unwrap();
    assert_call(
        &transport,
        "sendLocation",
        json!({"chat_id": CHAT, "latitude": 48.7665, "longitude": 11.4258}),
    );

    let (client, transport) = ok_message();
    client
        .send_venue(&SendVenue::new(
            CHAT,
            48.7665,
            11.4258,
            "Café",
            "Am Platz 1",
        ))
        .await
        .unwrap();
    assert_call(
        &transport,
        "sendVenue",
        json!({
            "chat_id": CHAT,
            "latitude": 48.7665,
            "longitude": 11.4258,
            "title": "Café",
            "address": "Am Platz 1",
        }),
    );

    let (client, transport) = ok_message();
    client
        .send_contact(&SendContact::new(CHAT, "+4915112345678", "Flo"))
        .await
        .unwrap();
    assert_call(
        &transport,
        "sendContact",
        json!({"chat_id": CHAT, "phone_number": "+4915112345678", "first_name": "Flo"}),
    );
}

#[tokio::test]
async fn poll_and_dice() {
    let (client, transport) = ok_message();
    let poll = SendPoll::quiz(CHAT, "2 + 2?", vec!["3".into(), "4".into()], 1);
    client.send_poll(&poll).await.unwrap();
    assert_call(
        &transport,
        "sendPoll",
        json!({
            "chat_id": CHAT,
            "question": "2 + 2?",
            "options": ["3", "4"],
            "type": "quiz",
            "correct_option_id": 1,
            "is_anonymous": false,
        }),
    );

    let (client, transport) = ok_message();
    client
        .send_dice(&SendDice::new(CHAT).with_emoji("🎯"))
        .await
        .unwrap();
    assert_call(
        &transport,
        "sendDice",
        json!({"chat_id": CHAT, "emoji": "🎯"}),
    );
}

#[tokio::test]
async fn edit_methods() {
    let (client, transport) = client_with(MockReply::result(message_json()));
    client
        .edit_message_text(
            &EditMessageText::new(CHAT, 7, "fixed")
                .with_parse_mode(telegram_api_client::ParseMode::MarkdownV2),
        )
        .await
        .unwrap();
    assert_call(
        &transport,
        "editMessageText",
        json!({"chat_id": CHAT, "message_id": 7, "text": "fixed", "parse_mode": "MarkdownV2"}),
    );

    let (client, transport) = client_with(MockReply::result(message_json()));
    client
        .edit_message_caption(&EditMessageCaption::new(CHAT, 7).with_caption("new"))
        .await
        .unwrap();
    assert_call(
        &transport,
        "editMessageCaption",
        json!({"chat_id": CHAT, "message_id": 7, "caption": "new"}),
    );

    let (client, transport) = client_with(MockReply::result(message_json()));
    client
        .edit_message_reply_markup(&EditMessageReplyMarkup::new(CHAT, 7).with_reply_markup(
            telegram_api_client::ReplyMarkup::inline(
                telegram_api_client::InlineKeyboardMarkup::new(vec![vec![
                    telegram_api_client::InlineKeyboardButton::callback("ok", "ok"),
                ]]),
            ),
        ))
        .await
        .unwrap();
    assert_call(
        &transport,
        "editMessageReplyMarkup",
        json!({
            "chat_id": CHAT,
            "message_id": 7,
            "reply_markup": {"inline_keyboard": [[{"text": "ok", "callback_data": "ok"}]]},
        }),
    );
}

#[tokio::test]
async fn edit_inline_message_uses_only_inline_id_and_decodes_true() {
    let (client, transport) = client_with(MockReply::result(json!(true)));
    let result = client
        .edit_message_text(&EditMessageText::for_inline("inline-1", "fixed"))
        .await
        .unwrap();
    assert_eq!(result, telegram_api_client::EditResult::Inline(true));
    assert_call(
        &transport,
        "editMessageText",
        json!({"inline_message_id": "inline-1", "text": "fixed"}),
    );
}

#[tokio::test]
async fn delete_forward_copy() {
    let (client, transport) = ok_bool();
    assert!(client
        .delete_message(&DeleteMessage::new(CHAT, 7))
        .await
        .unwrap());
    assert_call(
        &transport,
        "deleteMessage",
        json!({"chat_id": CHAT, "message_id": 7}),
    );

    let (client, transport) = ok_message();
    client
        .forward_message(&ForwardMessage::new(42, CHAT, 7))
        .await
        .unwrap();
    assert_call(
        &transport,
        "forwardMessage",
        json!({"chat_id": 42, "from_chat_id": CHAT, "message_id": 7}),
    );

    let (client, transport) = client_with(MockReply::result(json!({"message_id": 99})));
    let created = client
        .copy_message(&CopyMessage::new(42, CHAT, 7))
        .await
        .unwrap();
    assert_eq!(created.message_id, 99);
    assert_call(
        &transport,
        "copyMessage",
        json!({"chat_id": 42, "from_chat_id": CHAT, "message_id": 7}),
    );
}

#[tokio::test]
async fn callback_and_inline() {
    let (client, transport) = ok_bool();
    client
        .answer_callback_query(&AnswerCallbackQuery::new("4382").with_text("ok").alert())
        .await
        .unwrap();
    assert_call(
        &transport,
        "answerCallbackQuery",
        json!({"callback_query_id": "4382", "text": "ok", "show_alert": true}),
    );

    let (client, transport) = ok_bool();
    let answer =
        AnswerInlineQuery::new("q1", vec![InlineQueryResult::article("1", "Ping", "Pong")])
            .with_cache_time(30);
    client.answer_inline_query(&answer).await.unwrap();
    assert_call(
        &transport,
        "answerInlineQuery",
        json!({
            "inline_query_id": "q1",
            "cache_time": 30,
            "results": [{
                "type": "article",
                "id": "1",
                "title": "Ping",
                "input_message_content": {"message_text": "Pong"},
            }],
        }),
    );

    let (client, transport) = ok_bool();
    client
        .set_chat_menu_button(&SetChatMenuButton::for_chat(CHAT, MenuButton::Commands))
        .await
        .unwrap();
    assert_call(
        &transport,
        "setChatMenuButton",
        json!({"chat_id": CHAT, "menu_button": {"type": "commands"}}),
    );
}

#[tokio::test]
async fn chat_administration() {
    let (client, transport) = ok_bool();
    client
        .send_chat_action(&SendChatAction::typing(CHAT))
        .await
        .unwrap();
    assert_call(
        &transport,
        "sendChatAction",
        json!({"chat_id": CHAT, "action": "typing"}),
    );

    let (client, transport) = client_with(MockReply::result(json!({
        "id": CHAT,
        "type": "supergroup",
        "title": "Team",
    })));
    let chat = client.get_chat(&GetChat::new(CHAT)).await.unwrap();
    assert_eq!(chat.label(), "Team");
    assert_call(&transport, "getChat", json!({"chat_id": CHAT}));

    let (client, transport) = client_with(MockReply::result(json!({
        "status": "creator",
        "user": {"id": 42, "first_name": "Flo"},
    })));
    let member = client
        .get_chat_member(&GetChatMember::new(CHAT, 42))
        .await
        .unwrap();
    assert!(member.is_present());
    assert_eq!(member.user().display_name(), "Flo");
    assert_call(
        &transport,
        "getChatMember",
        json!({"chat_id": CHAT, "user_id": 42}),
    );

    let (client, transport) = ok_bool();
    client
        .ban_chat_member(&BanChatMember::new(CHAT, 42).until(0))
        .await
        .unwrap();
    assert_call(
        &transport,
        "banChatMember",
        json!({"chat_id": CHAT, "user_id": 42, "until_date": 0}),
    );

    let (client, transport) = ok_bool();
    client
        .pin_chat_message(&PinChatMessage::new(CHAT, 7).silent())
        .await
        .unwrap();
    assert_call(
        &transport,
        "pinChatMessage",
        json!({"chat_id": CHAT, "message_id": 7, "disable_notification": true}),
    );

    let (client, transport) = ok_bool();
    client
        .unpin_chat_message(&UnpinChatMessage::new(CHAT).message(7))
        .await
        .unwrap();
    assert_call(
        &transport,
        "unpinChatMessage",
        json!({"chat_id": CHAT, "message_id": 7}),
    );
}

/// `status` and `user` are mandatory in a `getChatMember` result, so a truncated
/// one is a typed decode failure that keeps the serde cause — never a member
/// with a default identity (issue #26).
#[tokio::test]
async fn truncated_chat_member_reply_is_a_payload_error() {
    use std::error::Error as StdError;

    // The valid shape decodes; the two members below are the boundary.
    let (client, _) = client_with(MockReply::result(json!({
        "status": "administrator",
        "user": {"id": 42, "first_name": "Flo"},
    })));
    let member = client
        .get_chat_member(&GetChatMember::new(CHAT, 42))
        .await
        .unwrap();
    assert_eq!(member.user().id, 42);
    assert!(matches!(
        member,
        telegram_api_client::ChatMember::Administrator(_)
    ));

    for truncated in [
        json!({"user": {"id": 42, "first_name": "Flo"}}), // no status
        json!({"status": "administrator"}),               // no user
        json!({}),                                        // neither
    ] {
        let (client, _) = client_with(MockReply::result(truncated.clone()));
        let error = client
            .get_chat_member(&GetChatMember::new(CHAT, 42))
            .await
            .expect_err("a member without status and user must not decode");

        match &error {
            telegram_api_client::Error::Payload { message, source } => {
                assert!(
                    message.starts_with("getChatMember: could not decode result"),
                    "message names the method: {message}"
                );
                assert!(
                    source.is_some(),
                    "the serde failure stays reachable for {truncated}"
                );
            }
            other => panic!("expected Error::Payload for {truncated}, got {other:?}"),
        }
        assert!(
            StdError::source(&error).is_some(),
            "the cause is attached to the error itself"
        );
        assert!(!error.is_retryable(), "the payload will not improve");
    }
}

#[tokio::test]
async fn webhook_lifecycle() {
    let (client, transport) = ok_bool();
    let request = SetWebhook::new("https://example.com/hook")
        .with_secret("s3cret")
        .max_connections(40)
        .allowed_updates(vec![telegram_api_client::UpdateKind::Message])
        .dropping_pending();
    client.set_webhook(&request).await.unwrap();
    assert_call(
        &transport,
        "setWebhook",
        json!({
            "url": "https://example.com/hook",
            "secret_token": "s3cret",
            "max_connections": 40,
            "allowed_updates": ["message"],
            "drop_pending_updates": true,
        }),
    );

    let (client, transport) = ok_bool();
    client
        .delete_webhook(&DeleteWebhook::dropping_pending())
        .await
        .unwrap();
    assert_call(
        &transport,
        "deleteWebhook",
        json!({"drop_pending_updates": true}),
    );

    let (client, transport) = client_with(MockReply::result(json!({
        "url": "https://example.com/hook",
        "pending_update_count": 3,
        "allowed_updates": ["message", "callback_query"],
    })));
    let info = client.get_webhook_info().await.unwrap();
    assert!(info.is_registered());
    assert_eq!(info.pending_update_count, 3);
    assert_call(&transport, "getWebhookInfo", json!({}));
}

#[tokio::test]
async fn updates_and_files() {
    let (client, transport) = client_with(MockReply::result(json!([
        {"update_id": 10},
        {"update_id": 11, "message": {"message_id": 1, "chat": {"id": CHAT, "type": "group"}, "text": "moin"}},
    ])));
    let updates = client
        .get_updates(
            &GetUpdates::new()
                .offset(10)
                .limit(50)
                .timeout(25)
                .allowed_updates(vec![telegram_api_client::UpdateKind::Message]),
        )
        .await
        .unwrap();
    assert_eq!(updates.len(), 2);
    assert_eq!(updates[1].text(), Some("moin"));
    assert_call(
        &transport,
        "getUpdates",
        json!({"offset": 10, "limit": 50, "timeout": 25, "allowed_updates": ["message"]}),
    );

    let (client, transport) = client_with(MockReply::result(json!({
        "file_id": "A",
        "file_unique_id": "u",
        "file_size": 1024,
        "file_path": "photos/a.jpg",
    })));
    let file = client.get_file(&GetFile::new("A")).await.unwrap();
    assert_eq!(
        client.file_url(&file).unwrap(),
        format!("https://tg.test/file/bot{TOKEN}/photos/a.jpg")
    );
    assert_call(&transport, "getFile", json!({"file_id": "A"}));
}

#[tokio::test]
async fn get_me_both_flavours() {
    let (client, transport) = client_with(MockReply::result(json!({
        "id": 1,
        "first_name": "bot",
        "username": "my_bot",
        "is_bot": true,
    })));
    let me = client.me().await.unwrap();
    assert_eq!(me.id, 1);
    assert_call(&transport, "getMe", json!({}));

    let transport = MockTransport::new();
    let client = TelegramClient::new(TOKEN, transport.clone());
    client.me().await.unwrap();
    assert_eq!(only_call(&transport).method(), "getMe");
}

/// The two raw-`Value` 0.1 convenience methods keep working, deprecated but
/// unchanged, so an existing caller's signature still compiles.
#[tokio::test]
#[allow(deprecated)]
async fn legacy_raw_methods_keep_their_signatures() {
    let transport = MockTransport::new();
    let client = TelegramClient::new(TOKEN, transport.clone()).with_base_url("https://tg.test");

    let envelope: Value = client.send_message(CHAT, "Hallo").await.unwrap();
    assert_eq!(envelope["ok"], true);
    assert_call(
        &transport,
        "sendMessage",
        json!({"chat_id": CHAT, "text": "Hallo"}),
    );

    transport.push(MockReply::result(json!({"id": 3, "is_bot": true})));
    let me: Value = client.get_me().await.unwrap();
    assert_eq!(me["result"]["id"], 3);
    assert_eq!(transport.calls().len(), 2);
}
