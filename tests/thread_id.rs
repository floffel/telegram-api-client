//! `message_thread_id` boundaries.
//!
//! Telegram documents the field as the *"Unique identifier for the target
//! message thread (topic) of a forum"* — an id from the same space as
//! `message_id`, so `0` and negative values name no thread. Every request struct
//! in the crate that accepts one is checked here on both sides of the boundary:
//! absent (`None`) and `1` are accepted, `0` and negative ids are refused on
//! `message_thread_id` before anything reaches the transport.

use proptest::prelude::*;
use telegram_api_client::{
    DeliveryOptions, Error, ForwardMessage, MediaOptions, MessageOptions, MockReply, MockTransport,
    SendAnimation, SendAudio, SendChatAction, SendContact, SendDice, SendDocument, SendLocation,
    SendMediaGroup, SendMessage, SendPhoto, SendPoll, SendVenue, SendVideo, SendVoice,
    TelegramClient,
};

const CHAT: i64 = -1001;

/// The `(field, reason)` of an `InvalidRequest`, panicking on any other outcome.
fn invalid_request(result: telegram_api_client::Result<()>) -> (String, String) {
    match result {
        Err(Error::InvalidRequest { field, reason }) => (field.to_string(), reason),
        other => panic!("expected InvalidRequest, got {other:?}"),
    }
}

/// Asserts a refusal names `message_thread_id` and quotes the offending value,
/// so a caller can see which id was wrong without re-reading the request.
#[track_caller]
fn assert_thread_refused(result: telegram_api_client::Result<()>, id: i64) {
    let (field, reason) = invalid_request(result);
    assert_eq!(field, "message_thread_id", "reason: {reason}");
    assert!(
        reason.contains(&id.to_string()),
        "the refusal must quote {id}, got: {reason}"
    );
}

/// Every request that carries a `message_thread_id`, built with `thread` set.
///
/// The closure returns the request's `validate()` outcome, which keeps the set
/// in one place: a new request struct with a thread id belongs in this list.
fn requests_with_thread(
    thread: Option<i64>,
) -> Vec<(&'static str, telegram_api_client::Result<()>)> {
    let options = DeliveryOptions {
        message_thread_id: thread,
        ..DeliveryOptions::default()
    };
    let message_options = MessageOptions {
        delivery: options.clone(),
        ..MessageOptions::default()
    };
    let media_options = MediaOptions {
        delivery: options.clone(),
        ..MediaOptions::default()
    };

    let mut chat_action = SendChatAction::typing(CHAT);
    chat_action.message_thread_id = thread;

    let mut forward = ForwardMessage::new(CHAT, CHAT, 7);
    forward.message_thread_id = thread;

    let mut message = SendMessage::new(CHAT, "hi");
    message.options = message_options.clone();

    let mut photo = SendPhoto::new(CHAT, "file-id");
    photo.options = media_options.clone();

    // The remaining media senders are one macro invocation each: they share the
    // same `MediaOptions`, so they are listed rather than sampled.
    let mut document = SendDocument::new(CHAT, "file-id");
    document.options = media_options.clone();
    let mut audio = SendAudio::new(CHAT, "file-id");
    audio.options = media_options.clone();
    let mut video = SendVideo::new(CHAT, "file-id");
    video.options = media_options.clone();
    let mut animation = SendAnimation::new(CHAT, "file-id");
    animation.options = media_options.clone();
    let mut voice = SendVoice::new(CHAT, "file-id");
    voice.options = media_options.clone();

    let mut album = SendMediaGroup::new(
        CHAT,
        vec![
            telegram_api_client::InputMedia::photo("a"),
            telegram_api_client::InputMedia::photo("b"),
        ],
    );
    album.delivery = options.clone();

    let mut location = SendLocation::new(CHAT, 48.1, 11.5);
    location.delivery = options.clone();

    let mut venue = SendVenue::new(CHAT, 48.1, 11.5, "Café", "Am Platz 1");
    venue.delivery = options.clone();

    let mut contact = SendContact::new(CHAT, "+4915112345678", "Florian");
    contact.delivery = options.clone();

    let mut poll = SendPoll::new(CHAT, "Lunch?", vec!["Pizza".into(), "Salad".into()]);
    poll.delivery = options.clone();

    let mut dice = SendDice::new(CHAT);
    dice.delivery = options.clone();

    vec![
        ("DeliveryOptions", DeliveryOptions::validate(&options)),
        ("MessageOptions", message_options.validate()),
        ("MediaOptions", media_options.validate()),
        ("SendChatAction", chat_action.validate()),
        ("ForwardMessage", forward.validate()),
        ("SendMessage", message.validate()),
        ("SendPhoto", photo.validate()),
        ("SendDocument", document.validate()),
        ("SendAudio", audio.validate()),
        ("SendVideo", video.validate()),
        ("SendAnimation", animation.validate()),
        ("SendVoice", voice.validate()),
        ("SendMediaGroup", album.validate()),
        ("SendLocation", location.validate()),
        ("SendVenue", venue.validate()),
        ("SendContact", contact.validate()),
        ("SendPoll", poll.validate()),
        ("SendDice", dice.validate()),
    ]
}

// ---- the boundary ---------------------------------------------------------

#[test]
fn an_absent_thread_id_is_accepted_everywhere() {
    for (name, result) in requests_with_thread(None) {
        assert!(result.is_ok(), "{name} must accept no thread id");
    }
}

#[test]
fn the_first_valid_thread_id_is_accepted_everywhere() {
    // 1 is the id of the "General" topic, so it is the smallest thread id
    // Telegram actually uses.
    for (name, result) in requests_with_thread(Some(1)) {
        assert!(result.is_ok(), "{name} must accept thread id 1");
    }
}

#[test]
fn zero_is_refused_everywhere() {
    for (name, result) in requests_with_thread(Some(0)) {
        let (field, reason) = invalid_request(result);
        assert_eq!(field, "message_thread_id", "{name}");
        assert!(reason.contains('0'), "{name}: {reason}");
    }
}

#[test]
fn negative_ids_are_refused_everywhere() {
    for (name, result) in requests_with_thread(Some(-1)) {
        let (field, reason) = invalid_request(result);
        assert_eq!(field, "message_thread_id", "{name}");
        assert!(reason.contains("-1"), "{name}: {reason}");
    }
}

#[test]
fn the_extremes_of_the_range_behave_like_the_boundary() {
    // The largest representable id is still a positive id: nothing about it is
    // out of range for the crate, Telegram is the one that would object.
    for (name, result) in requests_with_thread(Some(i64::MAX)) {
        assert!(result.is_ok(), "{name} must accept i64::MAX");
    }
    for (name, result) in requests_with_thread(Some(i64::MIN)) {
        let (field, _) = invalid_request(result);
        assert_eq!(field, "message_thread_id", "{name}");
    }
}

#[test]
fn an_absent_thread_id_leaves_no_key_on_the_wire() {
    // Telegram reads a missing `message_thread_id` as "the chat's main thread";
    // a serialized `null` or a `0` would be a different request, so the field
    // stays out of the body entirely until it is set.
    let action = SendChatAction::typing(CHAT);
    let json = serde_json::to_value(&action).expect("a typing action serializes");
    assert!(
        json.get("message_thread_id").is_none(),
        "an unset thread id must not be serialized: {json}"
    );

    let mut action = action;
    action.message_thread_id = Some(1);
    let json = serde_json::to_value(&action).expect("a typing action serializes");
    assert_eq!(json["message_thread_id"], 1);
}

// ---- the refusal, in detail ----------------------------------------------

#[test]
fn send_chat_action_refuses_a_zero_thread_and_names_the_value() {
    let mut action = SendChatAction::typing(CHAT);
    action.message_thread_id = Some(0);
    assert_thread_refused(action.validate(), 0);

    action.message_thread_id = Some(-42);
    assert_thread_refused(action.validate(), -42);
}

#[test]
fn forward_message_refuses_a_non_positive_thread() {
    let mut forward = ForwardMessage::new(CHAT, CHAT, 7);
    forward.message_thread_id = Some(0);
    assert_thread_refused(forward.validate(), 0);
}

#[test]
fn the_thread_is_checked_after_the_rest_of_the_request() {
    // Field order stays predictable: a chat error wins over a thread error, so a
    // caller fixing one field at a time is not bounced between them.
    let mut action = SendChatAction::new(0, telegram_api_client::ChatAction::Typing);
    action.message_thread_id = Some(0);
    assert_eq!(invalid_request(action.validate()).0, "chat_id");

    let mut action = SendChatAction::typing(CHAT);
    action.action = telegram_api_client::ChatAction::from("");
    action.message_thread_id = Some(0);
    assert_eq!(invalid_request(action.validate()).0, "action");
}

#[test]
fn the_other_delivery_flags_stay_unchecked() {
    // Only the id space is enforceable locally; a reply target is not a thread.
    let mut options = DeliveryOptions::default().silent().reply_to(0);
    options.protect_content = Some(true);
    assert!(options.validate().is_ok(), "reply_to_message_id is #39");
}

// ---- nothing invalid reaches the wire ------------------------------------

#[tokio::test]
async fn an_invalid_thread_stops_before_the_transport() {
    let transport = MockTransport::new();
    transport.push(MockReply::result(serde_json::json!(true)));
    let client = TelegramClient::new("123:ABC", transport.clone()).without_retry();

    let mut action = SendChatAction::typing(CHAT);
    action.message_thread_id = Some(0);
    let error = client
        .send_chat_action(&action)
        .await
        .expect_err("a zero thread id is refused locally");
    assert_eq!(invalid_request(Err(error)).0, "message_thread_id");
    assert!(
        transport.calls().is_empty(),
        "a local refusal must not spend a request: {:?}",
        transport.calls()
    );

    // The same request with a usable thread id does go out.
    action.message_thread_id = Some(7);
    client
        .send_chat_action(&action)
        .await
        .expect("thread 7 is fine");
    assert_eq!(transport.calls().len(), 1);
    assert_eq!(
        transport.calls()[0].body["message_thread_id"],
        7,
        "the id is serialized as a number, not a string"
    );
}

#[tokio::test]
async fn an_invalid_thread_stops_a_send_message_too() {
    let transport = MockTransport::new();
    transport.push(MockReply::ok());
    let client = TelegramClient::new("123:ABC", transport.clone()).without_retry();

    let mut request = SendMessage::new(CHAT, "hi");
    request.options.delivery.message_thread_id = Some(-3);
    let error = client
        .send_message_request(&request)
        .await
        .expect_err("a negative thread id is refused locally");
    assert_eq!(invalid_request(Err(error)).0, "message_thread_id");
    assert!(transport.calls().is_empty());
}

// ---- properties -----------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    /// Acceptance is exactly `id > 0`, for every `i64` — no gap, no off-by-one.
    #[test]
    fn only_positive_thread_ids_are_accepted(id in any::<i64>()) {
        let mut action = SendChatAction::typing(CHAT);
        action.message_thread_id = Some(id);
        prop_assert_eq!(action.validate().is_ok(), id > 0, "id {}", id);

        let options = DeliveryOptions {
            message_thread_id: Some(id),
            ..DeliveryOptions::default()
        };
        prop_assert_eq!(options.validate().is_ok(), id > 0, "id {}", id);

        let mut forward = ForwardMessage::new(CHAT, CHAT, 7);
        forward.message_thread_id = Some(id);
        prop_assert_eq!(forward.validate().is_ok(), id > 0, "id {}", id);
    }

    /// A refusal always names the field and quotes the id, never a bare
    /// "bad request" the caller has to guess at.
    #[test]
    fn every_refusal_quotes_the_id(id in i64::MIN..=0) {
        let mut action = SendChatAction::typing(CHAT);
        action.message_thread_id = Some(id);
        let (field, reason) = invalid_request(action.validate());
        prop_assert_eq!(field, "message_thread_id");
        prop_assert!(reason.contains(&id.to_string()), "{}", reason);
    }

    /// The thread id survives the JSON round trip unchanged — validation must
    /// not depend on a value that serialization would rewrite.
    #[test]
    fn a_valid_thread_survives_serialization(id in 1_i64..=i64::MAX) {
        let mut action = SendChatAction::typing(CHAT);
        action.message_thread_id = Some(id);
        let json = serde_json::to_value(&action).unwrap();
        prop_assert_eq!(json["message_thread_id"].as_i64(), Some(id));

        let back: SendChatAction = serde_json::from_value(json).unwrap();
        prop_assert_eq!(back.message_thread_id, Some(id));
        prop_assert!(back.validate().is_ok());
    }
}
