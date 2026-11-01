//! Edge cases: the last valid value, the first invalid one, and permissive
//! decoding in both directions.

use serde_json::json;
use telegram_api_client::{
    AnswerCallbackQuery, AnswerInlineQuery, BanChatMember, ChatId, ChatType, CopyMessage,
    DeleteMessage, EditMessageCaption, EditMessageText, EditTarget, EntityKind, Error,
    GetChatMember, GetFile, GetUpdates, InlineQueryResult, InputMedia, LinkPreviewOptions,
    MessageEntity, MockReply, MockTransport, ParseMode, PollType, ReplyMarkup, SendDice,
    SendLocation, SendMediaGroup, SendMessage, SendPhoto, SendPoll, SendVenue, SetWebhook,
    TelegramClient, Update, UpdateCursor, WebhookGuard, WebhookInfo,
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

    // The minimal required shape — everything else is optional.
    let minimal: ChatMember = serde_json::from_str(r#"{"status":"member","user":{"id":42}}"#)
        .expect("status and user are enough");
    assert_eq!(minimal.status(), ChatMemberStatus::Member);
    assert_eq!(minimal.user().id, 42);
    assert_eq!(minimal.is_anonymous(), None);
    assert!(minimal.is_present());

    // Status and user are the two required members. Either one missing (or
    // present but null, or of the wrong shape) is a decode error instead of a
    // silently defaulted member.
    for payload in [
        r#"{"user":{"id":42,"first_name":"Flo"}}"#,  // no status
        r#"{"status":"member"}"#,                    // no user
        r"{}",                                       // neither
        r#"{"status":"member","user":null}"#,        // user null
        r#"{"status":null,"user":{"id":42}}"#,       // status null
        r#"{"status":"member","user":42}"#,          // user not an object
        r#"{"status":42,"user":{"id":42}}"#,         // status not a string
        r#"[{"status":"member","user":{"id":42}}]"#, // not even an object
    ] {
        let parsed: Result<ChatMember, _> = serde_json::from_str(payload);
        assert!(parsed.is_err(), "{payload} must not decode into a member");
    }
}

/// A member update carries the same required members, so a truncated one is
/// rejected one level up as well (the update itself still defaults whole
/// members it was not sent, which is the documented permissive behaviour).
#[test]
fn chat_member_update_rejects_a_partial_member() {
    use telegram_api_client::{ChatMemberStatus, ChatMemberUpdated};

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
    let updated: ChatMemberUpdated =
        serde_json::from_str(full).expect("the complete shape decodes");
    assert!(updated.joined());
    assert_eq!(updated.old_chat_member.status(), ChatMemberStatus::Left);
    assert_eq!(updated.new_chat_member.status(), ChatMemberStatus::Member);
}

/// Every status-specific member survives the decode and comes back out again
/// (issue #25): the administrator's whole permission set, the restricted
/// member's bits, the ban deadline, the custom title — plus a field no version
/// of this crate models.
#[test]
fn chat_member_keeps_status_specific_fields() {
    use telegram_api_client::ChatMember;

    // A complete administrator object, exactly as Bot API 10.3 documents it.
    let wire = json!({
        "status": "administrator",
        "user": {"id": 42, "first_name": "Flo"},
        "can_be_edited": false,
        "is_anonymous": true,
        "can_manage_chat": true,
        "can_delete_messages": true,
        "can_manage_video_chats": false,
        "can_restrict_members": true,
        "can_promote_members": false,
        "can_change_info": true,
        "can_invite_users": true,
        "can_post_stories": false,
        "can_edit_stories": false,
        "can_delete_stories": false,
        "can_post_messages": true,
        "can_edit_messages": true,
        "can_pin_messages": true,
        "can_manage_topics": false,
        "can_manage_direct_messages": false,
        "can_manage_tags": true,
        "can_send_welcome_messages": true,
        "custom_title": "Ops",
        "can_book_meetings": true
    });

    let member: ChatMember =
        serde_json::from_value(wire.clone()).expect("the complete shape decodes");
    let ChatMember::Administrator(admin) = &member else {
        panic!("status `administrator` must decode into the administrator shape");
    };
    assert!(!admin.can_be_edited);
    assert_eq!(admin.is_anonymous, Some(true));
    assert_eq!(admin.can_pin_messages, Some(true));
    assert_eq!(admin.can_manage_topics, Some(false));
    assert_eq!(admin.can_manage_direct_messages, Some(false));
    assert_eq!(admin.custom_title.as_deref(), Some("Ops"));
    assert_eq!(
        admin.extra.get("can_book_meetings"),
        Some(&json!(true)),
        "a field no variant models is kept, not dropped"
    );
    assert_eq!(member.custom_title(), Some("Ops"));
    assert_eq!(member.is_anonymous(), Some(true));

    // Nothing is dropped on the way out either: a real payload — every
    // mandatory member present — re-serialises to exactly what Telegram sent.
    assert_eq!(serde_json::to_value(&member).unwrap(), wire);
}

/// One document shape per status, each decoding into its own variant and back
/// to the same object. The compact payloads here omit the rights Telegram would
/// send, and the crate's own output stays stable for them as well.
#[test]
fn chat_member_maps_every_status_to_its_shape() {
    use telegram_api_client::ChatMember;

    let cases = [
        json!({"status":"creator","user":{"id":1},"is_anonymous":false,"custom_title":"Owner"}),
        json!({"status":"administrator","user":{"id":1},"can_pin_messages":true,"custom_title":"Ops"}),
        json!({"status":"member","user":{"id":1},"tag":"vip","until_date":1_700_000_001}),
        json!({"status":"restricted","user":{"id":1},"tag":"muted","is_member":true,"until_date":0}),
        json!({"status":"left","user":{"id":1}}),
        json!({"status":"kicked","user":{"id":1},"until_date":1_700_000_002}),
    ];

    for wire in cases {
        let member: ChatMember = serde_json::from_value(wire.clone()).expect("the shape decodes");
        assert_eq!(
            member.status().as_str(),
            wire["status"].as_str().expect("a status string"),
            "the variant must report the status it came from"
        );
        assert_eq!(member.user().id, 1);

        // Decode → encode → decode is a fixed point: no information is lost.
        let rendered = serde_json::to_value(&member).expect("the member serialises");
        let reparsed: ChatMember =
            serde_json::from_value(rendered).expect("own output parses back");
        assert_eq!(reparsed, member, "serialisation lost information");
    }

    // The status word selects the variant, and `kicked` is the wire spelling of
    // the ban.
    for status in [
        "creator",
        "administrator",
        "member",
        "restricted",
        "left",
        "kicked",
    ] {
        let member: ChatMember =
            serde_json::from_value(json!({"status": status, "user": {"id": 1}}))
                .expect("the status decodes");
        let shape = match &member {
            ChatMember::Creator(_) => "creator",
            ChatMember::Administrator(_) => "administrator",
            ChatMember::Member(_) => "member",
            ChatMember::Restricted(_) => "restricted",
            ChatMember::Left(_) => "left",
            ChatMember::Banned(_) => "kicked",
            _ => "unknown",
        };
        assert_eq!(shape, status, "status `{status}` must pick its own shape");
    }
}

/// `ChatMember::new` builds every status, and the member reports it back.
#[test]
fn chat_member_new_covers_every_status() {
    use telegram_api_client::{ChatMember, ChatMemberStatus, User};

    let user = User {
        id: 7,
        first_name: "Flo".to_string(),
        ..User::default()
    };
    for status in [
        ChatMemberStatus::Creator,
        ChatMemberStatus::Administrator,
        ChatMemberStatus::Member,
        ChatMemberStatus::Restricted,
        ChatMemberStatus::Left,
        ChatMemberStatus::Kicked,
        ChatMemberStatus::Unknown("ghost".to_string()),
    ] {
        let member = ChatMember::new(status.clone(), user.clone());
        assert_eq!(member.status(), status);
        assert_eq!(member.user().id, 7);
        // A restricted member built this way carries no `is_member`, so it is
        // not presence either.
        let expected = matches!(
            status,
            ChatMemberStatus::Creator | ChatMemberStatus::Administrator | ChatMemberStatus::Member
        );
        assert_eq!(member.is_present(), expected, "presence of {status:?}");
    }

    // The emptiness of a default member: no status this crate knows, and not in
    // the chat.
    let empty = ChatMember::default();
    assert_eq!(empty.status(), ChatMemberStatus::Unknown(String::new()));
    assert!(!empty.is_present());
    assert_eq!(
        ChatMember::new(ChatMemberStatus::Left, user).is_anonymous(),
        None
    );
}

/// A status this crate does not model is a new value, not a missing one: it
/// keeps its user and every other field, so nothing is lost on a round trip.
#[test]
fn chat_member_carries_an_unmodelled_status_with_its_payload() {
    use telegram_api_client::{ChatMember, ChatMemberStatus};

    let wire = json!({
        "status": "ghost",
        "user": {"id": 42, "first_name": "Flo"},
        "is_anonymous": true,
        "custom_title": "Casper",
        "until_date": 1_700_000_005_i64,
        "is_member": true,
        "haunting_since": 1_700_000_000
    });
    let member: ChatMember =
        serde_json::from_value(wire.clone()).expect("an unmodelled status decodes");
    assert_eq!(
        member.status(),
        ChatMemberStatus::Unknown("ghost".to_string())
    );
    assert_eq!(member.user().id, 42);
    assert!(!member.is_present(), "an unknown status is not presence");
    // The accessors answer from what the unmodelled shape actually carried.
    assert_eq!(member.is_anonymous(), Some(true));
    assert_eq!(member.custom_title(), Some("Casper"));
    assert_eq!(member.until_date(), Some(1_700_000_005));
    assert_eq!(member.is_member(), Some(true));
    assert_eq!(serde_json::to_value(&member).unwrap(), wire);

    // No user, no member — a new status does not weaken the required pair.
    assert!(serde_json::from_str::<ChatMember>(r#"{"status":"ghost"}"#).is_err());
}

/// The `is_member` bit decides presence for a restricted member, which is what
/// the join/leave helpers are built on.
#[test]
fn restricted_presence_follows_is_member() {
    use telegram_api_client::{ChatMember, ChatMemberStatus, User};

    let present: ChatMember = serde_json::from_str(
        r#"{"status":"restricted","user":{"id":1},"is_member":true,"can_send_messages":true}"#,
    )
    .unwrap();
    let absent: ChatMember = serde_json::from_str(
        r#"{"status":"restricted","user":{"id":1},"is_member":false,"can_send_messages":false}"#,
    )
    .unwrap();
    assert!(present.is_present());
    assert!(!absent.is_present());
    assert_eq!(absent.is_member(), Some(false));

    // A shape without the bit at all is not presence either.
    assert!(!ChatMember::new(ChatMemberStatus::Restricted, User::default()).is_present());
}

/// Every member is decoded as the type the Bot API documents. The documented
/// type decodes, a different one is an error instead of a silently defaulted
/// member — the boundary pair for each member type's payload.
#[test]
fn chat_member_fields_keep_their_wire_types() {
    use telegram_api_client::ChatMember;

    let cases = [
        (
            r#"{"status":"restricted","user":{"id":1},"is_member":"yes"}"#,
            r#"{"status":"restricted","user":{"id":1},"is_member":true}"#,
        ),
        (
            r#"{"status":"restricted","user":{"id":1},"can_send_messages":1}"#,
            r#"{"status":"restricted","user":{"id":1},"can_send_messages":false}"#,
        ),
        (
            r#"{"status":"kicked","user":{"id":1},"until_date":"soon"}"#,
            r#"{"status":"kicked","user":{"id":1},"until_date":0}"#,
        ),
        (
            r#"{"status":"creator","user":{"id":1},"custom_title":16}"#,
            r#"{"status":"creator","user":{"id":1},"custom_title":"Ops"}"#,
        ),
        (
            r#"{"status":"administrator","user":{"id":1},"can_pin_messages":"true"}"#,
            r#"{"status":"administrator","user":{"id":1},"can_pin_messages":true}"#,
        ),
        (
            r#"{"status":"creator","user":{"id":1},"is_anonymous":"false"}"#,
            r#"{"status":"creator","user":{"id":1},"is_anonymous":false}"#,
        ),
        (
            r#"{"status":"member","user":{"id":1},"until_date":"never"}"#,
            r#"{"status":"member","user":{"id":1},"until_date":0}"#,
        ),
    ];

    for (wrong, right) in cases {
        assert!(
            serde_json::from_str::<ChatMember>(wrong).is_err(),
            "{wrong} carries a value of the wrong type and must not decode"
        );
        assert!(
            serde_json::from_str::<ChatMember>(right).is_ok(),
            "{right} carries the documented type"
        );
    }
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
    assert!(matches!(
        target.validate(),
        Err(Error::InvalidRequest { .. })
    ));

    // Neither form.
    assert!(EditTarget::default().validate().is_err());

    // Half a chat form.
    let half = EditTarget {
        chat_id: Some(ChatId::id(CHAT)),
        ..EditTarget::default()
    };
    assert_invalid(half.validate().unwrap_err(), "message_id");

    // Blank inline id is treated as absent.
    let mut blank = EditTarget::inline("   ");
    blank.inline_message_id = Some("   ".into());
    assert!(blank.validate().is_err());

    // Non-positive message id.
    assert_invalid(
        EditTarget::chat(CHAT, 0).validate().unwrap_err(),
        "message_id",
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

    let file: telegram_api_client::File = serde_json::from_str("{}").unwrap();
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
async fn wrong_result_shape_is_a_payload_error() {
    let (client, transport) = client();
    transport.push(MockReply::result(json!("not a message")));
    let error = client
        .send_photo(&SendPhoto::new(CHAT, "f"))
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Payload { .. }));
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
