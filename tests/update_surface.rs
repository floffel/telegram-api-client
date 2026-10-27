//! One fixture per `Update` field of the pinned Bot API version.
//!
//! The table below is the executable copy of `docs/contracts.md`: every kind in
//! `UpdateKind::ALL` must have exactly one fixture, `Update::kind()` must report
//! it, and the matching payload field must be populated.

use telegram_api_client::{Update, UpdateKind};

/// One wire fixture and the kind it should be classified as.
struct Fixture {
    kind: UpdateKind,
    json: &'static str,
}

const FIXTURES: [Fixture; 27] = [
    Fixture {
        kind: UpdateKind::Message,
        json: r#"{"update_id":1,"message":{"message_id":1,"chat":{"id":10},"text":"hi"}}"#,
    },
    Fixture {
        kind: UpdateKind::EditedMessage,
        json: r#"{"update_id":2,"edited_message":{"message_id":1,"chat":{"id":10},"text":"hi!"}}"#,
    },
    Fixture {
        kind: UpdateKind::ChannelPost,
        json: r#"{"update_id":3,"channel_post":{"message_id":7,"chat":{"id":-10},"text":"news"}}"#,
    },
    Fixture {
        kind: UpdateKind::EditedChannelPost,
        json: r#"{"update_id":4,"edited_channel_post":{"message_id":7,"chat":{"id":-10},"text":"fixed"}}"#,
    },
    Fixture {
        kind: UpdateKind::BusinessConnection,
        json: r#"{"update_id":5,"business_connection":{"id":"bc1","user":{"id":1},"user_chat_id":1,"date":1700000000,"is_enabled":true}}"#,
    },
    Fixture {
        kind: UpdateKind::BusinessMessage,
        json: r#"{"update_id":6,"business_message":{"message_id":2,"chat":{"id":1},"text":"biz"}}"#,
    },
    Fixture {
        kind: UpdateKind::EditedBusinessMessage,
        json: r#"{"update_id":7,"edited_business_message":{"message_id":2,"chat":{"id":1},"text":"biz!"}}"#,
    },
    Fixture {
        kind: UpdateKind::DeletedBusinessMessages,
        json: r#"{"update_id":8,"deleted_business_messages":{"business_connection_id":"bc1","chat":{"id":1},"message_ids":[4,5]}}"#,
    },
    Fixture {
        kind: UpdateKind::GuestMessage,
        json: r#"{"update_id":9,"guest_message":{"message_id":3,"chat":{"id":1},"text":"guest"}}"#,
    },
    Fixture {
        kind: UpdateKind::MessageReaction,
        json: r#"{"update_id":10,"message_reaction":{"chat":{"id":10},"message_id":1,"date":1700000000,"old_reaction":[],"new_reaction":[{"type":"emoji","emoji":"👍"}]}}"#,
    },
    Fixture {
        kind: UpdateKind::MessageReactionCount,
        json: r#"{"update_id":11,"message_reaction_count":{"chat":{"id":10},"message_id":1,"date":1700000000,"reactions":[{"type":{"type":"emoji","emoji":"🎉"},"total_count":2}]}}"#,
    },
    Fixture {
        kind: UpdateKind::InlineQuery,
        json: r#"{"update_id":12,"inline_query":{"id":"q1","from":{"id":1},"query":"ping","offset":""}}"#,
    },
    Fixture {
        kind: UpdateKind::ChosenInlineResult,
        json: r#"{"update_id":13,"chosen_inline_result":{"result_id":"r1","from":{"id":1},"query":"ping"}}"#,
    },
    Fixture {
        kind: UpdateKind::CallbackQuery,
        json: r#"{"update_id":14,"callback_query":{"id":"cq1","from":{"id":1},"data":"yes","chat_instance":"-1"}}"#,
    },
    Fixture {
        kind: UpdateKind::ShippingQuery,
        json: r#"{"update_id":15,"shipping_query":{"id":"sq1","from":{"id":1},"invoice_payload":"o1","shipping_address":{"country_code":"DE","city":"Dresden"}}}"#,
    },
    Fixture {
        kind: UpdateKind::PreCheckoutQuery,
        json: r#"{"update_id":16,"pre_checkout_query":{"id":"pc1","from":{"id":1},"currency":"EUR","total_amount":1450,"invoice_payload":"o1"}}"#,
    },
    Fixture {
        kind: UpdateKind::PurchasedPaidMedia,
        json: r#"{"update_id":17,"purchased_paid_media":{"from":{"id":1},"paid_media_payload":"media1"}}"#,
    },
    Fixture {
        kind: UpdateKind::Poll,
        json: r#"{"update_id":18,"poll":{"id":"p1","question":"Lunch?","options":[{"text":"Yes","voter_count":1}],"total_voter_count":1,"is_closed":false,"is_anonymous":true,"type":"regular","allows_multiple_answers":false}}"#,
    },
    Fixture {
        kind: UpdateKind::PollAnswer,
        json: r#"{"update_id":19,"poll_answer":{"poll_id":"p1","user":{"id":1},"option_ids":[0]}}"#,
    },
    Fixture {
        kind: UpdateKind::MyChatMember,
        json: r#"{"update_id":20,"my_chat_member":{"chat":{"id":-1},"from":{"id":1},"date":1700000000,"old_chat_member":{"status":"left","user":{"id":9}},"new_chat_member":{"status":"member","user":{"id":9}}}}"#,
    },
    Fixture {
        kind: UpdateKind::ChatMember,
        json: r#"{"update_id":21,"chat_member":{"chat":{"id":-1},"from":{"id":1},"date":1700000000,"old_chat_member":{"status":"member","user":{"id":9}},"new_chat_member":{"status":"kicked","user":{"id":9}}}}"#,
    },
    Fixture {
        kind: UpdateKind::ChatJoinRequest,
        json: r#"{"update_id":22,"chat_join_request":{"chat":{"id":-1},"from":{"id":9},"user_chat_id":9,"date":1700000000}}"#,
    },
    Fixture {
        kind: UpdateKind::ChatBoost,
        json: r#"{"update_id":23,"chat_boost":{"chat":{"id":-1},"boost":{"boost_id":"b1","add_date":1,"expiration_date":2,"source":{"source":"premium","user":{"id":9}}}}}"#,
    },
    Fixture {
        kind: UpdateKind::RemovedChatBoost,
        json: r#"{"update_id":24,"removed_chat_boost":{"chat":{"id":-1},"boost_id":"b1","remove_date":3,"source":{"source":"gift_code","user":{"id":9}}}}"#,
    },
    Fixture {
        kind: UpdateKind::ManagedBot,
        json: r#"{"update_id":25,"managed_bot":{"user":{"id":1},"bot":{"id":2,"first_name":"Helper","is_bot":true}}}"#,
    },
    Fixture {
        kind: UpdateKind::Subscription,
        json: r#"{"update_id":26,"subscription":{"user":{"id":1},"invoice_payload":"sub1","state":"active"}}"#,
    },
    Fixture {
        kind: UpdateKind::StoppedMessageGeneration,
        json: r#"{"update_id":27,"stopped_message_generation":{"chat":{"id":-1},"draft_id":9}}"#,
    },
];

fn parse(json: &str) -> Update {
    match serde_json::from_str(json) {
        Ok(update) => update,
        Err(error) => panic!("fixture does not parse: {error}"),
    }
}

#[test]
fn every_update_kind_has_its_own_fixture() {
    assert_eq!(
        FIXTURES.len(),
        UpdateKind::ALL.len(),
        "fixture table is out of date"
    );
    for kind in UpdateKind::ALL {
        let hits = FIXTURES
            .iter()
            .filter(|fixture| fixture.kind == kind)
            .count();
        assert_eq!(
            hits, 1,
            "kind {kind} needs exactly one fixture, found {hits}"
        );
    }
}

#[test]
fn each_fixture_classifies_as_its_kind() {
    for fixture in &FIXTURES {
        let update = parse(fixture.json);
        assert_eq!(
            update.kind(),
            Some(fixture.kind.clone()),
            "fixture {}",
            fixture.json
        );
        assert_eq!(
            update.kind().map(|kind| kind.as_str().to_string()),
            Some(fixture.kind.as_str().to_string()),
            "wire name of {}",
            fixture.kind
        );
    }
}

#[test]
fn each_fixture_round_trips_through_serde() {
    for fixture in &FIXTURES {
        let update = parse(fixture.json);
        let encoded = match serde_json::to_string(&update) {
            Ok(encoded) => encoded,
            Err(error) => panic!("{} does not serialize: {error}", fixture.kind),
        };
        let again = parse(&encoded);
        assert_eq!(again, update, "round trip changed {}", fixture.kind);
        assert_eq!(again.kind(), Some(fixture.kind.clone()));
    }
}

#[test]
fn only_the_named_field_is_populated() {
    for fixture in &FIXTURES {
        let update = parse(fixture.json);
        let empty = Update::default();
        // Exactly one payload field differs from the empty update, which is what
        // the Bot API promises ("at most one of the optional fields").
        let populated = serde_json::to_value(&update).expect("serializable");
        let object = populated.as_object().expect("object");
        let payload_fields: Vec<&String> = object
            .keys()
            .filter(|key| key.as_str() != "update_id")
            .collect();
        assert_eq!(
            payload_fields.len(),
            1,
            "{}: {payload_fields:?}",
            fixture.kind
        );
        assert_ne!(update, empty, "{}", fixture.kind);
    }
}

#[test]
fn unmodelled_payload_yields_no_kind() {
    let update = parse(r#"{"update_id":1,"brand_new_kind":{"id":"x"}}"#);
    assert_eq!(update.kind(), None);
    assert_eq!(
        update.primary_message().map(|message| message.message_id),
        None
    );
}

#[test]
fn camel_case_payload_fields_are_accepted() {
    let update = parse(
        r#"{"updateId":5,"businessConnection":{"id":"bc1","user":{"id":1},"userChatId":2,"date":3,"isEnabled":false}}"#,
    );
    assert_eq!(update.kind(), Some(UpdateKind::BusinessConnection));
    let connection = update.business_connection.expect("payload");
    assert_eq!(connection.user_chat_id, 2);
    assert!(!connection.is_enabled);
}

#[test]
fn only_the_first_matching_field_wins() {
    // The API never sends two payloads, but a proxy bug must not crash us: the
    // documented field order decides.
    let update = parse(
        r#"{"update_id":1,"message":{"message_id":1,"chat":{"id":2},"text":"a"},
            "callback_query":{"id":"cq1","data":"b"}}"#,
    );
    assert_eq!(update.kind(), Some(UpdateKind::Message));
    assert_eq!(update.callback_data(), Some("b"));
}

#[test]
fn chat_member_payload_is_reachable_from_both_fields() {
    for key in ["my_chat_member", "chat_member"] {
        let json = format!(
            r#"{{"update_id":1,"{key}":{{"chat":{{"id":-1}},"date":1,"new_chat_member":{{"status":"member","user":{{"id":2}}}}}}}}"#
        );
        let update = parse(&json);
        let changed = update.chat_member_update().expect("member payload");
        assert!(changed.new_chat_member.is_present());
        assert!(changed.joined(), "{key} should read as a join");
    }
}

#[test]
fn update_stays_small_enough_to_batch() {
    // `getUpdates` returns up to 100 updates per call, so a payload field that
    // is stored inline multiplies by 100 on every batch. 512 bytes leaves room
    // for a few more kinds in the next Bot API revision and still keeps a full
    // batch under 64 KB.
    let size = std::mem::size_of::<Update>();
    assert!(
        size <= 512,
        "Update grew to {size} bytes; box the new payload"
    );
    assert!(
        std::mem::size_of::<Option<Update>>() <= 512,
        "Option<Update> grew unexpectedly"
    );
}
