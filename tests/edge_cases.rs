//! Edge cases: the last valid value, the first invalid one, and permissive
//! decoding in both directions.

use serde_json::{json, Value};
use telegram_api_client::{
    AnswerCallbackQuery, AnswerInlineQuery, BanChatMember, ChatId, ChatType, CopyMessage,
    DeleteMessage, EditMessageCaption, EditMessageText, EditTarget, EntityKind, Error,
    GetChatMember, GetFile, GetUpdates, InlineQueryResult, InputMedia, LinkPreviewOptions,
    MenuButton, MessageEntity, MockReply, MockTransport, ParseMode, PollType, ReplyMarkup,
    SendDice, SendLocation, SendMediaGroup, SendMessage, SendPhoto, SendPoll, SendVenue,
    SetChatMenuButton, SetWebhook, TelegramClient, Update, UpdateCursor, WebhookGuard, WebhookInfo,
};

const CHAT: i64 = -1001;

/// A client that never retries, so error semantics can be asserted on exactly
/// one reply. Automatic retry has its own test file.
fn client() -> (TelegramClient<MockTransport>, MockTransport) {
    let transport = MockTransport::new();
    (
        TelegramClient::new("123:ABC", transport.clone()).without_retry(),
        transport,
    )
}

/// Asserts the error is an `InvalidRequest` on `field`.
#[track_caller]
fn assert_invalid(error: Error, field: &str) {
    match error {
        Error::InvalidRequest { field: actual, .. } => assert_eq!(actual, field),
        other => panic!("expected InvalidRequest({field}), got {other:?}"),
    }
}

// ---- text boundaries ----------------------------------------------------

#[test]
fn text_length_boundary_is_utf16_code_units() {
    let at_limit = "a".repeat(4096);
    assert!(SendMessage::new(CHAT, at_limit.clone()).validate().is_ok());

    let over = "a".repeat(4097);
    assert_invalid(SendMessage::new(CHAT, over).validate().unwrap_err(), "text");

    // Telegram counts UTF-16 units: an emoji is two units.
    let emoji_at_limit = "🎯".repeat(2048);
    assert_eq!(emoji_at_limit.chars().count(), 2048);
    assert!(SendMessage::new(CHAT, emoji_at_limit).validate().is_ok());

    let emoji_over = "🎯".repeat(2049);
    assert_invalid(
        SendMessage::new(CHAT, emoji_over).validate().unwrap_err(),
        "text",
    );
}

#[test]
fn empty_and_whitespace_text_are_rejected() {
    for text in ["", " ", "\n\t"] {
        assert_invalid(SendMessage::new(CHAT, text).validate().unwrap_err(), "text");
    }
}

#[test]
fn caption_boundary() {
    let ok = SendPhoto::new(CHAT, "file").with_caption("c".repeat(1024));
    assert!(ok.validate().is_ok());

    let over = SendPhoto::new(CHAT, "file").with_caption("c".repeat(1025));
    assert_invalid(over.validate().unwrap_err(), "caption");

    let empty = SendPhoto::new(CHAT, "file").with_caption("");
    assert_invalid(empty.validate().unwrap_err(), "caption");
}

#[test]
fn media_reference_must_not_be_empty() {
    assert_invalid(SendPhoto::new(CHAT, "  ").validate().unwrap_err(), "photo");
}

#[test]
fn chat_id_boundaries() {
    assert!(ChatId::id(1).as_id().is_some());
    assert!(SendMessage::new(1, "x").validate().is_ok());
    assert!(SendMessage::new(CHAT, "x").validate().is_ok());
    assert_invalid(SendMessage::new(0, "x").validate().unwrap_err(), "chat_id");
    // A username is fine, an empty one is not.
    assert!(SendMessage::new("@floffel", "x").validate().is_ok());
    assert_invalid(SendMessage::new("", "x").validate().unwrap_err(), "chat_id");
}

#[test]
fn missing_chat_id_cannot_be_deserialized() {
    // `chat_id` is not optional in the request model, so a body without it never
    // validates — it fails at the type level, before any network call.
    let parsed: Result<SendMessage, _> = serde_json::from_str(r#"{"text":"hi"}"#);
    assert!(parsed.is_err(), "chat_id must be required");
}

// ---- polls --------------------------------------------------------------

#[test]
fn poll_option_boundaries() {
    let two = SendPoll::new(CHAT, "q", vec!["a".into(), "b".into()]);
    assert!(two.validate().is_ok());

    let twelve = SendPoll::new(CHAT, "q", (0..12).map(|i| format!("o{i}")).collect());
    assert!(twelve.validate().is_ok());

    let one = SendPoll::new(CHAT, "q", vec!["a".into()]);
    assert_invalid(one.validate().unwrap_err(), "options");

    let thirteen = SendPoll::new(CHAT, "q", (0..13).map(|i| format!("o{i}")).collect());
    assert_invalid(thirteen.validate().unwrap_err(), "options");
}

#[test]
fn poll_question_and_option_length_boundaries() {
    assert!(
        SendPoll::new(CHAT, "q".repeat(300), vec!["a".into(), "b".into()])
            .validate()
            .is_ok()
    );
    assert_invalid(
        SendPoll::new(CHAT, "q".repeat(301), vec!["a".into(), "b".into()])
            .validate()
            .unwrap_err(),
        "question",
    );

    assert!(SendPoll::new(CHAT, "q", vec!["o".repeat(100), "b".into()])
        .validate()
        .is_ok());
    assert_invalid(
        SendPoll::new(CHAT, "q", vec!["o".repeat(101), "b".into()])
            .validate()
            .unwrap_err(),
        "options",
    );
    assert_invalid(
        SendPoll::new(CHAT, "q", vec![String::new(), "b".into()])
            .validate()
            .unwrap_err(),
        "options",
    );
}

#[test]
fn quiz_correct_option_must_index_the_options() {
    assert!(SendPoll::quiz(CHAT, "q", vec!["a".into(), "b".into()], 1)
        .validate()
        .is_ok());
    assert_invalid(
        SendPoll::quiz(CHAT, "q", vec!["a".into(), "b".into()], 2)
            .validate()
            .unwrap_err(),
        "correct_option_id",
    );
}

#[test]
fn poll_open_period_and_close_date_are_mutually_exclusive() {
    let mut poll = SendPoll::new(CHAT, "q", vec!["a".into(), "b".into()]);
    poll.open_period = Some(600);
    assert!(poll.validate().is_ok());

    poll.open_period = Some(601);
    assert_invalid(poll.validate().unwrap_err(), "open_period");

    poll.open_period = Some(5);
    poll.close_date = Some(1_700_000_000);
    assert_invalid(poll.validate().unwrap_err(), "open_period");
}

#[test]
fn dice_emoji_must_not_be_empty() {
    assert!(SendDice::new(CHAT).with_emoji("🎲").validate().is_ok());
    assert_invalid(
        SendDice::new(CHAT).with_emoji("").validate().unwrap_err(),
        "emoji",
    );
}

// ---- media groups -------------------------------------------------------

#[test]
fn media_group_size_boundaries() {
    let items = |n: usize| {
        (0..n)
            .map(|i| InputMedia::photo(format!("f{i}")))
            .collect::<Vec<_>>()
    };
    assert!(SendMediaGroup::new(CHAT, items(2)).validate().is_ok());
    assert!(SendMediaGroup::new(CHAT, items(10)).validate().is_ok());
    assert_invalid(
        SendMediaGroup::new(CHAT, items(1)).validate().unwrap_err(),
        "media",
    );
    assert_invalid(
        SendMediaGroup::new(CHAT, items(11)).validate().unwrap_err(),
        "media",
    );

    let empty_ref = SendMediaGroup::new(CHAT, vec![InputMedia::photo(""), InputMedia::photo("b")]);
    assert_invalid(empty_ref.validate().unwrap_err(), "media");
}

// ---- coordinates --------------------------------------------------------

#[test]
fn coordinate_boundaries() {
    // Exactly on the poles and the antimeridian are valid.
    assert!(SendLocation::new(CHAT, 90.0, 180.0).validate().is_ok());
    assert!(SendLocation::new(CHAT, -90.0, -180.0).validate().is_ok());
    assert!(SendLocation::new(CHAT, 0.0, 0.0).validate().is_ok());

    assert_invalid(
        SendLocation::new(CHAT, 90.000_1, 0.0)
            .validate()
            .unwrap_err(),
        "latitude",
    );
    assert_invalid(
        SendLocation::new(CHAT, 0.0, -180.000_1)
            .validate()
            .unwrap_err(),
        "longitude",
    );
    assert_invalid(
        SendLocation::new(CHAT, f64::NAN, 0.0)
            .validate()
            .unwrap_err(),
        "latitude",
    );
    assert_invalid(
        SendLocation::new(CHAT, f64::INFINITY, 0.0)
            .validate()
            .unwrap_err(),
        "latitude",
    );
}

#[test]
fn live_location_boundaries() {
    let mut location = SendLocation::new(CHAT, 0.0, 0.0);
    location.heading = Some(1);
    assert!(location.validate().is_ok());
    location.heading = Some(360);
    assert!(location.validate().is_ok());
    location.heading = Some(0);
    assert_invalid(location.validate().unwrap_err(), "heading");
    location.heading = Some(361);
    assert_invalid(location.validate().unwrap_err(), "heading");

    location.heading = None;
    location.live_period = Some(0);
    assert_invalid(location.validate().unwrap_err(), "live_period");
}

#[test]
fn venue_requires_title_and_address() {
    assert!(SendVenue::new(CHAT, 0.0, 0.0, "Café", "Street 1")
        .validate()
        .is_ok());
    assert_invalid(
        SendVenue::new(CHAT, 0.0, 0.0, "", "Street 1")
            .validate()
            .unwrap_err(),
        "title",
    );
    assert_invalid(
        SendVenue::new(CHAT, 0.0, 0.0, "Café", " ")
            .validate()
            .unwrap_err(),
        "address",
    );
}

// ---- message ids and edit targets --------------------------------------

#[test]
fn message_ids_must_be_positive() {
    assert_invalid(
        DeleteMessage::new(CHAT, 0).validate().unwrap_err(),
        "message_id",
    );
    assert_invalid(
        CopyMessage::new(CHAT, 42, -1).validate().unwrap_err(),
        "message_id",
    );
    assert_invalid(
        GetChatMember::new(CHAT, 0).validate().unwrap_err(),
        "user_id",
    );
    assert_invalid(
        BanChatMember::new(CHAT, 1)
            .until(-1)
            .validate()
            .unwrap_err(),
        "until_date",
    );
    assert_invalid(GetFile::new("   ").validate().unwrap_err(), "file_id");
}

// ---- chat members --------------------------------------------------------

/// `status` and `user` are mandatory in a `getChatMember` result: a payload
/// without either one is a decode error, not a default member (issue #26).
#[test]
fn chat_member_requires_status_and_user() {
    use telegram_api_client::{ChatMember, ChatMemberStatus};

    // The full shape Telegram sends, plus the minimal required one. Everything
    // else stays optional, so the second object is all a caller must send.
    let full: ChatMember = serde_json::from_str(
        r#"{"status":"kicked","user":{"id":42,"first_name":"Flo"},"is_anonymous":true}"#,
    )
    .expect("the complete shape decodes");
    assert_eq!(full.status, ChatMemberStatus::Kicked);
    assert_eq!(full.user.id, 42);
    assert_eq!(full.is_anonymous, Some(true));

    let minimal: ChatMember = serde_json::from_str(r#"{"status":"member","user":{"id":42}}"#)
        .expect("status and user are enough");
    assert_eq!(minimal.status, ChatMemberStatus::Member);
    assert_eq!(minimal.is_anonymous, None);
    assert!(minimal.is_present());

    // Status and user are the two required members. Either one missing (or
    // present but null, or of the wrong shape) is a decode error instead of a
    // silently defaulted member.
    for payload in [
        r#"{"user":{"id":42,"first_name":"Flo"}}"#, // no status
        r#"{"status":"member"}"#,                   // no user
        r"{}",                                      // neither
        r#"{"status":"member","user":null}"#,       // user null
        r#"{"status":null,"user":{"id":42}}"#,      // status null
        r#"{"status":"member","user":42}"#,         // user not an object
    ] {
        let parsed: Result<ChatMember, _> = serde_json::from_str(payload);
        assert!(parsed.is_err(), "{payload} must not decode into a member");
    }

    // A status this crate does not know is required data, not absent data: it
    // is still carried as `Unknown`.
    let fresh: ChatMember = serde_json::from_str(r#"{"status":"ghost","user":{"id":42}}"#).unwrap();
    assert_eq!(fresh.status, ChatMemberStatus::Unknown("ghost".to_string()));
    assert!(!fresh.is_present(), "an unknown status is not presence");
}

/// A `restricted` member carries an `is_member` flag, and it decides presence
/// (issue #24). Reading presence off `status` alone reported a restricted user
/// who had been pushed out of the chat as present, so a presence check turned
/// into a false positive.
#[test]
fn restricted_non_members_are_not_present() {
    use telegram_api_client::{ChatMember, ChatMemberPresence, ChatMemberStatus};

    fn member(status: &str, is_member: Option<bool>) -> ChatMember {
        ChatMember {
            status: ChatMemberStatus::from(status),
            is_member,
            ..ChatMember::default()
        }
    }

    // The reported bug: restricted with the flag on false is outside the chat.
    let outsider = member("restricted", Some(false));
    assert!(
        !outsider.is_present(),
        "a restricted non-member is not there"
    );
    assert_eq!(outsider.presence(), ChatMemberPresence::Absent);

    // The same status with the flag on true is a member with limits.
    let insider = member("restricted", Some(true));
    assert!(insider.is_present());
    assert_eq!(insider.presence(), ChatMemberPresence::Present);

    // A flag that was not sent is not read as an absence: only an explicit
    // `false` takes the user out of the chat.
    let unset = member("restricted", None);
    assert!(unset.is_present());
    assert_eq!(unset.presence(), ChatMemberPresence::Present);

    // The flag belongs to `restricted` alone. A crafted `is_member: false` on
    // another status must not make a member disappear.
    for status in ["creator", "administrator", "member"] {
        let present = member(status, Some(false));
        assert!(present.is_present(), "{status} is in the chat");
        assert_eq!(present.presence(), ChatMemberPresence::Present);
    }

    // `left` and `kicked` are absent whatever the flag claims.
    for status in ["left", "kicked"] {
        let gone = member(status, Some(true));
        assert!(!gone.is_present(), "{status} is outside the chat");
        assert_eq!(gone.presence(), ChatMemberPresence::Absent);
    }

    // A status this crate does not know yet is neither present nor absent. It
    // stays visible as `Unknown` instead of being collapsed into "absent".
    for status in ["ghost", ""] {
        let fresh = member(status, None);
        assert!(!fresh.is_present());
        assert_eq!(fresh.presence(), ChatMemberPresence::Unknown, "{status}");
    }
}

/// The `is_member` flag decodes in both spellings, rejects a wrong shape, and
/// is left off the wire again when the payload never carried it.
#[test]
fn restricted_flag_survives_the_wire() {
    use telegram_api_client::{ChatMember, ChatMemberStatus};

    let snake: ChatMember =
        serde_json::from_str(r#"{"status":"restricted","user":{"id":42},"is_member":false}"#)
            .expect("snake_case flag");
    assert_eq!(snake.is_member, Some(false));

    let camel: ChatMember =
        serde_json::from_str(r#"{"status":"restricted","user":{"id":42},"isMember":true}"#)
            .expect("camelCase flag");
    assert_eq!(camel.is_member, Some(true));

    // A flag of the wrong shape is a decode error, not a silent `false`.
    assert!(serde_json::from_str::<ChatMember>(
        r#"{"status":"restricted","user":{"id":42},"is_member":"no"}"#
    )
    .is_err());

    // An unset flag is omitted instead of being written as `null`, so a
    // re-serialized member keeps the shape Telegram sent.
    let plain = ChatMember {
        status: ChatMemberStatus::Member,
        ..ChatMember::default()
    };
    let encoded = serde_json::to_value(&plain).expect("serialize");
    assert!(encoded.get("is_member").is_none(), "{encoded}");

    let restricted: ChatMember =
        serde_json::from_str(r#"{"status":"restricted","user":{"id":42},"is_member":false}"#)
            .expect("restricted");
    let encoded = serde_json::to_value(&restricted).expect("serialize");
    assert_eq!(encoded["is_member"], serde_json::json!(false), "{encoded}");
}

/// A restricted user who is no longer a member leaves the chat, even though
/// their status is neither `left` nor `kicked` (issue #24).
#[test]
fn a_restricted_non_member_transition_is_a_leave() {
    use telegram_api_client::ChatMemberUpdated;

    let update: ChatMemberUpdated = serde_json::from_str(
        r#"{"chat":{"id":-1},"date":1,
            "old_chat_member":{"status":"member","user":{"id":9}},
            "new_chat_member":{"status":"restricted","user":{"id":9},"is_member":false}}"#,
    )
    .expect("restricted non-member");
    assert!(update.left(), "out of the chat is a leave");
    assert!(!update.joined());

    // The way back — still restricted, but a member again — is a join.
    let back: ChatMemberUpdated = serde_json::from_str(
        r#"{"chat":{"id":-1},"date":1,
            "old_chat_member":{"status":"restricted","user":{"id":9},"is_member":false},
            "new_chat_member":{"status":"restricted","user":{"id":9},"is_member":true}}"#,
    )
    .expect("restricted member");
    assert!(back.joined());
    assert!(!back.left());
}

/// A member update carries the same required members, so a truncated one is
/// rejected one level up as well (the update itself still defaults whole
/// members it was not sent, which is the documented permissive behaviour).
#[test]
fn chat_member_update_rejects_a_partial_member() {
    let partial = r#"{"chat":{"id":-1},"date":1,
        "old_chat_member":{"status":"left"},
        "new_chat_member":{"status":"member","user":{"id":9}}}"#;
    assert!(
        serde_json::from_str::<telegram_api_client::ChatMemberUpdated>(partial).is_err(),
        "a member without its user must not decode inside an update"
    );

    let full = r#"{"chat":{"id":-1},"date":1,
        "old_chat_member":{"status":"left","user":{"id":9}},
        "new_chat_member":{"status":"member","user":{"id":9}}}"#;
    let updated: telegram_api_client::ChatMemberUpdated =
        serde_json::from_str(full).expect("the complete shape decodes");
    assert!(updated.joined());
}

#[test]
fn edit_target_needs_exactly_one_addressing_form() {
    assert!(EditMessageText::new(CHAT, 7, "x").validate().is_ok());
    assert!(EditMessageText::for_inline("inline", "x")
        .validate()
        .is_ok());
    assert!(EditMessageCaption::for_inline("inline").validate().is_ok());

    // Both forms at once.
    let mut target = EditTarget::chat(CHAT, 7);
    target.inline_message_id = Some("inline".into());
    assert_invalid(target.validate().unwrap_err(), "inline_message_id");

    // Neither form.
    assert!(EditTarget::default().validate().is_err());

    // Half a chat form.
    let half = EditTarget {
        chat_id: Some(ChatId::id(CHAT)),
        ..EditTarget::default()
    };
    assert_invalid(half.validate().unwrap_err(), "message_id");

    // Non-positive message id.
    assert_invalid(
        EditTarget::chat(CHAT, 0).validate().unwrap_err(),
        "message_id",
    );
}

/// #36: a blank `inline_message_id` is a *present* addressing form. Reading it
/// as absent let `chat_id` + `message_id` validate as a chat edit while the
/// serializer still sent the blank field, so the body carried both addressing
/// forms.
#[test]
fn blank_inline_message_id_is_rejected_not_ignored() {
    /// The reason string of an `InvalidRequest`.
    #[track_caller]
    fn reason(error: Error) -> String {
        match error {
            Error::InvalidRequest { reason, .. } => reason,
            other => panic!("expected InvalidRequest, got {other:?}"),
        }
    }

    // Boundary pair: `None` is absent, anything `Some` is present. Between the
    // two lies the blank value, which used to be read as absent.
    assert!(EditTarget::chat(CHAT, 7).validate().is_ok());
    for blank in ["", " ", "   ", "\t", "\n", " \t\n "] {
        // On its own: no longer "neither form", but a blank field.
        let alone = EditTarget::inline(blank);
        assert_invalid(alone.validate().unwrap_err(), "inline_message_id");
        assert_eq!(
            reason(EditTarget::inline(blank).validate().unwrap_err()),
            "must not be empty"
        );

        // Next to a complete chat form: refused, because the serialized body
        // would carry both forms.
        let mut both = EditTarget::chat(CHAT, 7);
        both.inline_message_id = Some(blank.to_string());
        let body = serde_json::to_value(&both).expect("serialize");
        assert_eq!(body["inline_message_id"], json!(blank), "the field is sent");
        assert_eq!(body["chat_id"], json!(CHAT));
        assert_eq!(body["message_id"], json!(7));
        assert_invalid(both.validate().unwrap_err(), "inline_message_id");
        assert_eq!(
            reason(both.validate().unwrap_err()),
            "must not be empty",
            "a blank id is reported as blank, not as a second form"
        );
    }

    // One non-blank id next to a chat form is the "both forms" error instead.
    let mut two_forms = EditTarget::chat(CHAT, 7);
    two_forms.inline_message_id = Some(" inline ".into());
    assert_invalid(two_forms.validate().unwrap_err(), "inline_message_id");
    assert_eq!(
        reason(two_forms.validate().unwrap_err()),
        "use either chat_id + message_id or inline_message_id, not both"
    );

    // Nothing is put on the wire: see
    // `blank_inline_message_id_never_reaches_the_wire`.
}

/// #36, end to end: the client validates before it builds a request, so an
/// `editMessageText` body carrying both addressing forms is never posted.
#[tokio::test]
async fn blank_inline_message_id_never_reaches_the_wire() {
    let (client, transport) = client();
    let mut request = EditMessageText::new(CHAT, 7, "x");
    request.target.inline_message_id = Some("   ".into());

    let error = client.edit_message_text(&request).await.unwrap_err();
    assert_invalid(error, "inline_message_id");
    assert!(
        transport.calls().is_empty(),
        "a rejected request must not reach the transport"
    );
}

// ---- callbacks, inline, polling ----------------------------------------

#[test]
fn callback_text_boundary() {
    assert!(AnswerCallbackQuery::new("id")
        .with_text("x".repeat(200))
        .validate()
        .is_ok());
    assert_invalid(
        AnswerCallbackQuery::new("id")
            .with_text("x".repeat(201))
            .validate()
            .unwrap_err(),
        "text",
    );
    assert_invalid(
        AnswerCallbackQuery::new("").validate().unwrap_err(),
        "callback_query_id",
    );
}

#[test]
fn inline_result_count_boundaries() {
    let results = |n: usize| {
        (0..n)
            .map(|i| InlineQueryResult::article(i.to_string(), "t", "m"))
            .collect::<Vec<_>>()
    };
    assert!(AnswerInlineQuery::new("q", results(1)).validate().is_ok());
    assert!(AnswerInlineQuery::new("q", results(50)).validate().is_ok());
    assert_invalid(
        AnswerInlineQuery::new("q", results(0))
            .validate()
            .unwrap_err(),
        "results",
    );
    assert_invalid(
        AnswerInlineQuery::new("q", results(51))
            .validate()
            .unwrap_err(),
        "results",
    );
}

#[test]
fn inline_results_must_be_objects_with_a_type_and_an_id() {
    // `null`, a string and an array are not results; the two mandatory members
    // may not be missing, blank or of the wrong JSON type.
    let broken = [
        json!(null),
        json!("article"),
        json!([{ "type": "article", "id": "1" }]),
        json!({ "id": "1" }),
        json!({ "type": "", "id": "1" }),
        json!({ "type": " \t", "id": "1" }),
        json!({ "type": 7, "id": "1" }),
        json!({ "type": "article" }),
        json!({ "type": "article", "id": 7 }),
        json!({ "type": "article", "id": null }),
    ];
    for value in broken {
        let result = InlineQueryResult::new(value);
        assert!(
            result.validate().is_err(),
            "a lone result must be refused: {}",
            result.value
        );
        let answer = AnswerInlineQuery::new("q", vec![result]);
        assert_invalid(answer.validate().unwrap_err(), "results");
    }

    // An unknown `type` and unknown members are fine: the crate forwards shapes
    // it does not model instead of narrowing the Bot API.
    let future = InlineQueryResult::new(json!({
        "type": "hologram",
        "id": "1",
        "sticker_url": "https://example.com/x.webm",
        "something_new": { "nested": [1, 2, 3] },
    }));
    assert!(future.validate().is_ok());
    assert!(AnswerInlineQuery::new("q", vec![future]).validate().is_ok());
}

#[test]
fn inline_result_id_boundary_is_measured_in_bytes() {
    let with_id = |id: String| InlineQueryResult::new(json!({"type": "article", "id": id}));

    // 1 and 64 bytes are inside the limit, 0 and 65 are not.
    assert!(with_id("x".to_string()).validate().is_ok());
    assert!(with_id("x".repeat(64)).validate().is_ok());
    assert!(with_id(String::new()).validate().is_err());
    assert!(with_id("x".repeat(65)).validate().is_err());

    // Bytes, not characters: 16 four-byte emoji are exactly 64 bytes and pass,
    // 17 are 68 bytes and fail, and 21 characters alone are below the limit.
    let at_limit = "🎯".repeat(16);
    assert_eq!(at_limit.chars().count(), 16);
    assert_eq!(at_limit.len(), 64);
    assert!(with_id(at_limit).validate().is_ok());

    let over = "🎯".repeat(17);
    assert_eq!(over.chars().count(), 17);
    assert!(with_id(over).validate().is_err());

    // The answer reports the index and the measured size.
    let answer = AnswerInlineQuery::new(
        "q",
        vec![
            InlineQueryResult::article("a", "t", "m"),
            with_id("x".repeat(65)),
        ],
    );
    let error = answer.validate().unwrap_err();
    let message = error.to_string();
    assert_invalid(error, "results");
    assert!(message.contains("results[1].id"), "{message}");
    assert!(message.contains("65"), "{message}");
}

#[test]
fn inline_result_ids_must_be_unique_across_the_answer() {
    let result = |id: &str| InlineQueryResult::article(id, "t", "m");
    assert!(AnswerInlineQuery::new("q", vec![result("1"), result("2")])
        .validate()
        .is_ok());

    // The same id twice — even with different members — makes the answer
    // ambiguous for the client, so it is refused. Case matters: ids are opaque.
    let duplicate = AnswerInlineQuery::new("q", vec![result("1"), result("2"), result("1")]);
    let error = duplicate.validate().unwrap_err();
    let message = error.to_string();
    assert_invalid(error, "results");
    assert!(message.contains("results[2].id"), "{message}");
    assert!(message.contains("\"1\""), "{message}");

    assert!(AnswerInlineQuery::new("q", vec![result("a"), result("A")])
        .validate()
        .is_ok());

    // Duplicates at the 50-result ceiling are still caught.
    let mut full = (0..49).map(|i| result(&i.to_string())).collect::<Vec<_>>();
    full.push(result("0"));
    assert_eq!(full.len(), 50);
    assert_invalid(
        AnswerInlineQuery::new("q", full).validate().unwrap_err(),
        "results",
    );
}

#[test]
fn inline_result_validation_keeps_every_other_member() {
    let value = json!({
        "type": "photo",
        "id": "p1",
        "photo_url": "https://example.com/a.jpg",
        "thumbnail_url": "https://example.com/t.jpg",
        "photo_width": 320,
        "photo_height": 200,
        "caption": "#tag <b>bold</b> ✓",
        "future_member": { "nested": [1, 2, 3] },
    });
    let answer = AnswerInlineQuery::new("q", vec![InlineQueryResult::new(value.clone())]);
    assert!(answer.validate().is_ok());
    // Validation is a check, not a rewrite: the wire body is byte-identical.
    let body = serde_json::to_value(&answer).unwrap();
    assert_eq!(body["results"][0], value);
}

#[tokio::test]
async fn a_malformed_inline_result_never_reaches_the_transport() {
    let (client, transport) = client();
    let answer = AnswerInlineQuery::new(
        "q",
        vec![InlineQueryResult::new(json!({"type": "article"}))],
    );
    let error = client.answer_inline_query(&answer).await.unwrap_err();
    assert_invalid(error, "results");
    assert!(
        transport.last_call().is_none(),
        "validation happens before the request is posted"
    );
}

#[test]
fn get_updates_limit_and_offset_boundaries() {
    assert!(GetUpdates::new().limit(1).validate().is_ok());
    assert!(GetUpdates::new().limit(100).validate().is_ok());
    assert_invalid(GetUpdates::new().limit(0).validate().unwrap_err(), "limit");
    assert_invalid(
        GetUpdates::new().limit(101).validate().unwrap_err(),
        "limit",
    );
    assert!(GetUpdates::new().offset(0).validate().is_ok());
    assert_invalid(
        GetUpdates::new().offset(-1).validate().unwrap_err(),
        "offset",
    );
}

// ---- webhook ------------------------------------------------------------

#[test]
fn webhook_secret_boundaries() {
    let url = "https://example.com/hook";
    assert!(SetWebhook::new(url)
        .with_secret("a".repeat(256))
        .validate()
        .is_ok());
    assert_invalid(
        SetWebhook::new(url)
            .with_secret("a".repeat(257))
            .validate()
            .unwrap_err(),
        "secret_token",
    );
    assert_invalid(
        SetWebhook::new(url).with_secret("").validate().unwrap_err(),
        "secret_token",
    );
    assert_invalid(
        SetWebhook::new(url)
            .with_secret("has space")
            .validate()
            .unwrap_err(),
        "secret_token",
    );
    // Telegram's documented alphabet is fine.
    assert!(SetWebhook::new(url).with_secret("aZ0_-").validate().is_ok());
}

#[test]
fn webhook_url_must_be_https() {
    assert_invalid(
        SetWebhook::new("http://example.com/hook")
            .validate()
            .unwrap_err(),
        "url",
    );
    assert_invalid(SetWebhook::new("https://").validate().unwrap_err(), "url");
    assert_invalid(SetWebhook::new("").validate().unwrap_err(), "url");
}

#[test]
fn webhook_connection_boundaries() {
    let url = "https://example.com/hook";
    assert!(SetWebhook::new(url).max_connections(1).validate().is_ok());
    assert!(SetWebhook::new(url).max_connections(100).validate().is_ok());
    assert_invalid(
        SetWebhook::new(url)
            .max_connections(0)
            .validate()
            .unwrap_err(),
        "max_connections",
    );
    assert_invalid(
        SetWebhook::new(url)
            .max_connections(101)
            .validate()
            .unwrap_err(),
        "max_connections",
    );
}

// ---- 429 handling -------------------------------------------------------

#[tokio::test]
async fn rate_limit_with_parameters_is_typed_and_retryable() {
    let (client, transport) = client();
    transport.push(MockReply::Raw(json!({
        "ok": false,
        "error_code": 429,
        "description": "Too Many Requests: retry after 5",
        "parameters": { "retry_after": 5 }
    })));

    let error = client.me().await.unwrap_err();
    assert!(error.is_retryable());
    assert_eq!(error.retry_after(), Some(std::time::Duration::from_secs(5)));
    assert!(matches!(error, Error::RateLimited { code: 429, .. }));
    assert!(error.to_string().contains("retry after 5s"));
}

#[tokio::test]
async fn rate_limit_falls_back_to_the_description() {
    let (client, transport) = client();
    transport.push(MockReply::Raw(json!({
        "ok": false,
        "error_code": 429,
        "description": "Too Many Requests: retry after 12"
    })));
    let error = client.get_webhook_info().await.unwrap_err();
    assert_eq!(
        error.retry_after(),
        Some(std::time::Duration::from_secs(12))
    );
}

#[tokio::test]
async fn rate_limit_without_a_hint_is_still_retryable() {
    let (client, transport) = client();
    transport.push(MockReply::Raw(json!({
        "ok": false,
        "error_code": 429,
        "description": "Too Many Requests"
    })));
    let error = client.me().await.unwrap_err();
    assert_eq!(error.retry_after(), None);
    assert!(error.is_retryable());
}

#[test]
fn terminal_errors_are_not_retryable() {
    for code in [400, 401, 403, 404] {
        let error = Error::Api {
            code,
            description: "nope".into(),
        };
        assert!(!error.is_retryable(), "{code} must be terminal");
    }
    for code in [500, 502, 503] {
        let error = Error::Api {
            code,
            description: "later".into(),
        };
        assert!(error.is_retryable(), "{code} must be retryable");
    }
    assert!(!Error::InvalidRequest {
        field: "text",
        reason: "empty".into()
    }
    .is_retryable());
}

// ---- decoding tolerance -------------------------------------------------

#[test]
fn unknown_fields_are_ignored_everywhere() {
    let update: Update = serde_json::from_str(
        r#"{
            "update_id": 1,
            "future_field": {"nested": [1, 2]},
            "message": {
                "message_id": 2,
                "chat": {"id": 3, "type": "private", "new_thing": true},
                "text": "hi",
                "also_new": 7
            }
        }"#,
    )
    .unwrap();
    assert_eq!(update.text(), Some("hi"));

    let info: WebhookInfo = serde_json::from_str(
        r#"{"url":"https://example.com/hook","pending_update_count":1,"unknown":[]}"#,
    )
    .unwrap();
    assert_eq!(info.pending_update_count, 1);
}

#[test]
fn file_required_ids_and_paths_are_validated_while_decoding() {
    use telegram_api_client::File;

    assert!(serde_json::from_str::<File>(r#"{"file_unique_id":"u"}"#).is_err());
    assert!(serde_json::from_str::<File>(r#"{"file_id":"A"}"#).is_err());
    assert!(serde_json::from_str::<File>(r#"{"file_id":"","file_unique_id":"u"}"#).is_err());
    assert!(serde_json::from_str::<File>(r#"{"file_id":"A","file_unique_id":""}"#).is_err());
    assert!(serde_json::from_str::<File>(r#"{"file_id":1,"file_unique_id":"u"}"#).is_err());
    assert!(serde_json::from_str::<File>(r#"{"file_id":"A","file_unique_id":1}"#).is_err());

    for path in [
        "",
        "   ",
        "/photos/a.jpg",
        "photos/../a.jpg",
        "photos/./a.jpg",
        "photos/a.jpg?x=1",
        "photos/a.jpg#fragment",
        "photos//a.jpg",
        "photos/a.jpg\\..\\b",
        "photos/a\u{0000}.jpg",
    ] {
        let payload = serde_json::json!({
            "file_id": "A",
            "file_unique_id": "u",
            "file_path": path,
        });
        assert!(
            serde_json::from_value::<File>(payload).is_err(),
            "accepted unsafe path {path:?}"
        );
    }

    let file: File = serde_json::from_str(
        r#"{"file_id":"A","file_unique_id":"u","file_path":"photos/Grüße 2026.jpg"}"#,
    )
    .unwrap();
    assert_eq!(file.download_path(), Some("photos/Grüße 2026.jpg"));
}

#[test]
fn unknown_enum_variants_are_carried() {
    let update: Update = serde_json::from_str(
        r#"{"update_id":1,"message":{"message_id":2,"chat":{"id":3,"type":"forum"}}}"#,
    )
    .unwrap();
    let chat = update.primary_message().unwrap().chat.clone();
    assert_eq!(chat.chat_type, ChatType::Unknown("forum".into()));
    assert_eq!(chat.chat_type.as_str(), "forum");

    let entity: MessageEntity =
        serde_json::from_str(r#"{"type":"spoiler_v2","offset":0,"length":1}"#).unwrap();
    assert_eq!(entity.kind, EntityKind::Unknown("spoiler_v2".into()));

    let markup: ReplyMarkup = serde_json::from_str(r#"{"keyboard_layout":"grid"}"#).unwrap();
    assert!(matches!(markup, ReplyMarkup::Unknown(_)));

    // And they round-trip.
    let back = serde_json::to_value(&entity).unwrap();
    assert_eq!(back["type"], "spoiler_v2");
}

/// A known menu button keeps the members this crate does not model — at the top
/// level and inside the nested `web_app` object — and writes them back.
#[test]
fn menu_button_preserves_unknown_members_on_known_variants() {
    for raw in [
        r#"{"type":"default","badge":"new"}"#,
        r#"{"type":"commands","badge":"new","scope":{"chat":1}}"#,
        r#"{"type":"web_app","text":"Open","badge":7,"web_app":{"url":"https://example.com","color":"red"}}"#,
    ] {
        let original: Value = serde_json::from_str(raw).unwrap();
        let button: MenuButton = serde_json::from_value(original.clone()).unwrap();
        assert_eq!(
            serde_json::to_value(&button).unwrap(),
            original,
            "lost a member of {raw}"
        );
    }

    // The preserved members are readable, and a plain builder starts empty.
    let button: MenuButton = serde_json::from_str(
        r#"{"type":"web_app","text":"Open","badge":7,"web_app":{"url":"https://example.com","color":"red"}}"#,
    )
    .unwrap();
    let extra = button.extra().expect("known variant");
    assert_eq!(extra["badge"], 7);
    assert_eq!(extra["web_app"]["color"], "red");
    assert_eq!(
        MenuButton::web_app("Open", "https://example.com").extra(),
        Some(&serde_json::Map::new())
    );

    // A preserved member never displaces a modelled one.
    let crafted = MenuButton::web_app("Open", "https://example.com")
        .with_extra("type", json!("ignored"))
        .with_extra("text", json!("ignored"))
        .with_extra(
            "web_app",
            json!({"url": "https://evil.example", "color": "red"}),
        );
    assert_eq!(
        serde_json::to_value(&crafted).unwrap(),
        json!({
            "type": "web_app",
            "text": "Open",
            "web_app": {"url": "https://example.com", "color": "red"},
        })
    );

    // An unknown button type keeps its whole payload, and `extra` has nothing
    // to add on top of it.
    let unknown = MenuButton::Unknown(json!({"type": "future"})).with_extra("x", json!([1]));
    assert_eq!(unknown.extra(), None);
    assert_eq!(
        serde_json::to_value(&unknown).unwrap(),
        json!({"type": "future"})
    );
    let parsed: MenuButton = serde_json::from_str(r#"{"type":"future","x":[1]}"#).unwrap();
    assert_eq!(parsed.extra(), None);
    assert_eq!(
        serde_json::to_value(&parsed).unwrap(),
        json!({"type": "future", "x": [1]})
    );

    // Inputs that are not an object at all are carried, not rejected.
    for raw in [r#""default""#, "7", "null", "[]"] {
        let original: Value = serde_json::from_str(raw).unwrap();
        let button: MenuButton = serde_json::from_value(original.clone()).unwrap();
        assert_eq!(button, MenuButton::Unknown(original.clone()), "{raw}");
        assert_eq!(serde_json::to_value(&button).unwrap(), original, "{raw}");
    }

    // A `web_app` payload that lost its shape still parses into the variant,
    // with the modelled members left empty.
    let broken: MenuButton =
        serde_json::from_str(r#"{"type":"web_app","text":5,"web_app":"nope"}"#).unwrap();
    assert_eq!(broken, MenuButton::web_app("", ""));

    // `MenuButton::default()` is the `default` button, so `SetChatMenuButton`'s
    // derived default matches the wire default.
    assert_eq!(MenuButton::default(), MenuButton::default_menu());

    // The whole request round-trips too, not just the button.
    let request: SetChatMenuButton =
        serde_json::from_str(r#"{"chat_id":1,"menu_button":{"type":"default","badge":"new"}}"#)
            .unwrap();
    assert_eq!(
        serde_json::to_value(&request).unwrap(),
        json!({"chat_id": 1, "menu_button": {"type": "default", "badge": "new"}})
    );
}

#[test]
fn snake_case_and_camel_case_are_both_accepted() {
    let snake: Update = serde_json::from_str(
        r#"{"update_id": 1, "callback_query": {"id": "9", "inline_message_id": "m", "chat_instance": "c"}}"#,
    )
    .unwrap();
    let camel: Update = serde_json::from_str(
        r#"{"updateId": 1, "callbackQuery": {"id": "9", "inlineMessageId": "m", "chatInstance": "c"}}"#,
    )
    .unwrap();
    assert_eq!(snake, camel);
    assert_eq!(camel.callback_query_id(), Some("9"));
}

#[test]
fn missing_optional_fields_default() {
    let update: Update = serde_json::from_str(r#"{"update_id":1}"#).unwrap();
    assert_eq!(update.text(), None);
    assert_eq!(update.chat_id(), None);
    assert!(update.callback_query.is_none());

    let info: WebhookInfo = serde_json::from_str("{}").unwrap();
    assert!(!info.is_registered());
    assert_eq!(info.pending_update_count, 0);

    let file: telegram_api_client::File =
        serde_json::from_str(r#"{"file_id":"A","file_unique_id":"u"}"#).unwrap();
    assert_eq!(file.file_size, None);
    assert_eq!(file.file_path, None);
    assert!(!file.is_downloadable());
}

#[test]
fn malformed_json_is_a_payload_error_with_a_source() {
    let guard = WebhookGuard::new("s3cret").unwrap();
    let path = guard.derived_path().to_string();
    let error = guard
        .verify_and_parse(Some("s3cret"), &path, b"{not json")
        .unwrap_err();
    match error {
        Error::Payload { message, source } => {
            assert!(message.contains("line 1"), "message was {message}");
            assert!(source.is_some(), "source chain must be preserved");
        }
        other => panic!("expected Payload, got {other:?}"),
    }
}

#[tokio::test]
async fn missing_result_field_is_unexpected() {
    let (client, transport) = client();
    transport.push(MockReply::Raw(json!({"ok": true})));
    let error = client
        .get_chat(&telegram_api_client::GetChat::new(CHAT))
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Unexpected { .. }));
    assert!(error.to_string().contains("getChat"));
}

#[tokio::test]
async fn wrong_result_shape_is_payload_for_a_read_and_ambiguous_for_a_send() {
    // A `result` that does not decode is a decoding failure either way, but only
    // a read may be repeated freely: a send whose answer was lost may already be
    // delivered, so it is reported as ambiguous with the `Payload` error kept as
    // the cause (issue #29).
    let (reader, inbox) = client();
    inbox.push(MockReply::result(json!("not a chat")));
    let error = reader
        .get_chat(&telegram_api_client::GetChat::new(CHAT))
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Payload { .. }), "{error:?}");
    assert!(!error.is_ambiguous());

    let (sender, outbox) = client();
    outbox.push(MockReply::result(json!("not a message")));
    let error = sender
        .send_photo(&SendPhoto::new(CHAT, "f"))
        .await
        .unwrap_err();
    assert!(error.is_ambiguous(), "{error:?}");
    let cause = std::error::Error::source(&error).expect("cause kept");
    assert!(matches!(
        cause.downcast_ref::<Error>(),
        Some(Error::Payload { .. })
    ));
}

// ---- secrets and redaction ---------------------------------------------

#[test]
fn empty_secret_fails_closed() {
    assert!(matches!(WebhookGuard::new(""), Err(Error::EmptySecret)));
    // A configured secret never matches a missing or empty header.
    let guard = WebhookGuard::new("s3cret").unwrap();
    assert!(guard.verify_secret(None).is_err());
    assert!(guard.verify_secret(Some("")).is_err());
    assert!(guard.verify_secret(Some("s3cret ")).is_err());
    assert!(guard.verify_secret(Some("s3cre")).is_err());
    assert!(guard.verify_secret(Some("S3CRET")).is_err());
    // NUL bytes must not be trimmed away.
    assert!(guard.verify_secret(Some("s3cret\0")).is_err());
    assert!(guard.verify_path("").is_err());
}

#[test]
fn secrets_never_appear_in_debug_output() {
    let client = TelegramClient::new("123:SECRET", MockTransport::new());
    assert!(!format!("{client:?}").contains("SECRET"));

    let guard = WebhookGuard::new("super-secret").unwrap();
    assert!(!format!("{guard:?}").contains("super-secret"));

    let webhook = SetWebhook::new("https://example.com/hook").with_secret("SUPERSECRET123");
    assert!(!format!("{webhook:?}").contains("SUPERSECRET123"));
    assert!(format!("{webhook:?}").contains("redacted"));

    // The secret still reaches the wire, of course.
    assert_eq!(
        serde_json::to_value(&webhook).unwrap()["secret_token"],
        "SUPERSECRET123"
    );
}

#[test]
fn entities_and_link_previews_round_trip() {
    let entity = MessageEntity::bold(0, 5).with_url("https://example.com");
    let json = serde_json::to_value(&entity).unwrap();
    let back: MessageEntity = serde_json::from_value(json).unwrap();
    assert_eq!(entity, back);

    let options = LinkPreviewOptions::disabled();
    assert_eq!(options.is_disabled, Some(true));

    let mut poll = SendPoll::new(CHAT, "q", vec!["a".into(), "b".into()]);
    poll.kind = Some(PollType::Regular);
    assert!(poll.validate().is_ok());

    let text = SendMessage::new(CHAT, "hi").with_parse_mode(ParseMode::Html);
    assert_eq!(serde_json::to_value(&text).unwrap()["parse_mode"], "HTML");
}

#[test]
fn update_cursor_never_reprocesses_an_id() {
    let batch = |ids: &[i64]| -> Vec<Update> {
        ids.iter()
            .map(|id| Update {
                update_id: *id,
                ..Update::default()
            })
            .collect()
    };
    let mut cursor = UpdateCursor::new();
    assert_eq!(cursor.next_offset(), None);

    let first = cursor.confirm(&batch(&[1, 2, 3]));
    assert_eq!(first.len(), 3);
    assert_eq!(cursor.next_offset(), Some(4));

    // Overlapping batch: 2 and 3 are already confirmed.
    let second = cursor.confirm(&batch(&[3, 4, 4, 5]));
    assert_eq!(
        second.iter().map(|u| u.update_id).collect::<Vec<_>>(),
        vec![4, 5]
    );
    assert_eq!(cursor.next_offset(), Some(6));

    // Older ids are dropped entirely.
    assert_eq!(cursor.confirm(&batch(&[1, 2])), Vec::new());
    assert_eq!(cursor.next_offset(), Some(6));
}

#[test]
fn update_cursor_saturates_at_i64_max() {
    let mut cursor = UpdateCursor::after(i64::MAX);
    let update = Update {
        update_id: i64::MAX,
        ..Update::default()
    };
    assert_eq!(cursor.confirm(&[update]), Vec::new());
    assert_eq!(cursor.next_offset(), Some(i64::MAX));
}
