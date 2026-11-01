//! Behaviour of the non-message payloads: accessors, defaults and edges.
//!
//! The fixture table in `tests/update_surface.rs` proves that every kind parses;
//! this file proves what the accessors answer once it has parsed. Doctests do
//! not count for `cargo llvm-cov`, so the same assertions live here too.

use telegram_api_client::{
    ChatBoostSource, ChatMember, ChatMemberStatus, ChatMemberUpdated, Location, ReactionCount,
    ReactionType, User,
};

fn reaction(json: &str) -> ReactionType {
    serde_json::from_str(json).expect("reaction parses")
}

#[test]
fn reaction_kinds_answer_their_accessors() {
    let emoji = reaction(r#"{"type":"emoji","emoji":"👍"}"#);
    assert_eq!(emoji.glyph(), Some("👍"));
    assert_eq!(emoji.custom_emoji_id(), None);

    let custom = reaction(r#"{"type":"custom_emoji","custom_emoji_id":"5370000000"}"#);
    assert_eq!(custom.glyph(), None);
    assert_eq!(custom.custom_emoji_id(), Some("5370000000"));

    let paid = reaction(r#"{"type":"paid"}"#);
    assert_eq!(paid.glyph(), None);
    assert_eq!(paid.custom_emoji_id(), None);

    // A kind from a future Bot API must parse and answer `None`, not error.
    let future = reaction(r#"{"type":"hologram","id":"x"}"#);
    assert_eq!(future, ReactionType::Unknown);
    assert_eq!(future.glyph(), None);
    assert_eq!(future.custom_emoji_id(), None);
    assert_eq!(ReactionType::default(), ReactionType::Unknown);
}

#[test]
fn reaction_count_sums_and_keeps_the_reaction() {
    let counts: Vec<ReactionCount> = serde_json::from_str(
        r#"[{"type":{"type":"emoji","emoji":"👍"},"total_count":2},
            {"type":{"type":"custom_emoji","custom_emoji_id":"9"},"total_count":40}]"#,
    )
    .expect("counts parse");
    assert_eq!(counts[0].r#type.glyph(), Some("👍"));
    assert_eq!(counts[1].r#type.custom_emoji_id(), Some("9"));

    let update: telegram_api_client::MessageReactionCountUpdated =
        serde_json::from_str(r#"{"message_id":1,"reactions":[]}"#).expect("empty reactions");
    assert_eq!(update.total(), 0, "no reactions sums to zero");
}

#[test]
fn removing_every_reaction_is_reported() {
    let removed: telegram_api_client::MessageReactionUpdated = serde_json::from_str(
        r#"{"message_id":1,"old_reaction":[{"type":"emoji","emoji":"👍"}],"new_reaction":[]}"#,
    )
    .expect("parse");
    assert!(removed.removed_all());

    let changed: telegram_api_client::MessageReactionUpdated =
        serde_json::from_str(r#"{"message_id":1,"new_reaction":[{"type":"emoji","emoji":"👎"}]}"#)
            .expect("parse");
    assert!(!changed.removed_all());
}

#[test]
fn boost_sources_are_tolerant_and_name_their_kind() {
    let premium: ChatBoostSource =
        serde_json::from_str(r#"{"source":"premium","user":{"id":7}}"#).expect("premium");
    assert_eq!(premium.source_kind(), "premium");
    assert_eq!(premium.user().map(|user| user.id), Some(7));

    let gift: ChatBoostSource =
        serde_json::from_str(r#"{"source":"gift_code","user":{"id":8}}"#).expect("gift");
    assert_eq!(gift.source_kind(), "gift_code");
    assert_eq!(gift.user().map(|user| user.id), Some(8));

    let giveaway: ChatBoostSource = serde_json::from_str(
        r#"{"source":"giveaway","giveaway_message_id":3,"user":{"id":9},"prize_star_count":100,"is_unclaimed":false}"#,
    )
    .expect("giveaway");
    assert_eq!(giveaway.source_kind(), "giveaway");
    assert_eq!(giveaway.user().map(|user| user.id), Some(9));

    let anonymous: ChatBoostSource =
        serde_json::from_str(r#"{"source":"giveaway","giveaway_message_id":3}"#).expect("anon");
    assert_eq!(anonymous.user().map(|user| user.id), None);

    let future: ChatBoostSource =
        serde_json::from_str(r#"{"source":"telepathy","user":{"id":1}}"#).expect("future");
    assert_eq!(future, ChatBoostSource::Unknown);
    assert_eq!(future.source_kind(), "unknown");
    assert_eq!(future.user().map(|user| user.id), None);
}

/// A member with `status` and, for a restricted one, `is_member` set — the bit
/// that decides presence for that shape.
fn member(status: ChatMemberStatus) -> ChatMember {
    match ChatMember::new(status, User::default()) {
        ChatMember::Restricted(mut restricted) => {
            restricted.is_member = true;
            ChatMember::Restricted(restricted)
        }
        other => other,
    }
}

#[test]
fn membership_transitions_are_classified() {
    let cases = [
        (
            ChatMemberStatus::Left,
            ChatMemberStatus::Member,
            true,
            false,
            "left -> member is a join",
        ),
        (
            ChatMemberStatus::Member,
            ChatMemberStatus::Kicked,
            false,
            true,
            "member -> kicked is a leave",
        ),
        (
            ChatMemberStatus::Member,
            ChatMemberStatus::Administrator,
            false,
            false,
            "promotion is neither",
        ),
        (
            ChatMemberStatus::Kicked,
            ChatMemberStatus::Restricted,
            true,
            false,
            "banned -> restricted reads as a re-join",
        ),
        (
            // An unknown status counts as absent, so this reads as a join.
            ChatMemberStatus::Unknown("ghost".to_string()),
            ChatMemberStatus::Member,
            true,
            false,
            "unknown -> member is a join",
        ),
    ];

    for (old, new, joined, left, why) in cases {
        let change = ChatMemberUpdated {
            old_chat_member: member(old.clone()),
            new_chat_member: member(new.clone()),
            ..ChatMemberUpdated::default()
        };
        assert_eq!(change.joined(), joined, "{why}");
        assert_eq!(change.left(), left, "{why}");
    }
}

#[test]
fn a_retracted_poll_vote_is_not_a_vote() {
    let vote: telegram_api_client::PollAnswer =
        serde_json::from_str(r#"{"poll_id":"p1","option_ids":[1]}"#).expect("vote");
    assert!(vote.is_vote());

    let retracted: telegram_api_client::PollAnswer =
        serde_json::from_str(r#"{"poll_id":"p1","option_ids":[]}"#).expect("retracted");
    assert!(!retracted.is_vote());

    let persistent: telegram_api_client::PollAnswer =
        serde_json::from_str(r#"{"poll_id":"p1","option_persistent_ids":["a"]}"#).expect("persist");
    assert!(persistent.is_vote(), "persistent ids alone are a vote");
}

#[test]
fn business_rights_default_to_granted() {
    use telegram_api_client::BusinessBotRights;

    let absent: BusinessBotRights = serde_json::from_str("{}").expect("empty rights");
    assert!(absent.can_reply);
    assert!(absent.can_manage_stories);
    assert_eq!(absent, BusinessBotRights::default());

    let revoked: BusinessBotRights =
        serde_json::from_str(r#"{"can_reply":false,"can_delete_all_messages":false}"#)
            .expect("partial rights");
    assert!(!revoked.can_reply);
    assert!(!revoked.can_delete_all_messages);
    assert!(revoked.can_read_messages, "unmentioned rights stay granted");
}

#[test]
fn a_connection_without_rights_keeps_none() {
    let connection: telegram_api_client::BusinessConnection = serde_json::from_str(
        r#"{"id":"bc1","user":{"id":1},"user_chat_id":1,"date":5,"is_enabled":true}"#,
    )
    .expect("connection");
    assert_eq!(connection.rights, None);

    let revoked: telegram_api_client::BusinessConnection =
        serde_json::from_str(r#"{"id":"bc1","is_enabled":false,"rights":{"can_reply":false}}"#)
            .expect("connection with rights");
    assert!(!revoked.rights.expect("rights").can_reply);
}

#[test]
fn invite_links_expose_their_url() {
    let link: telegram_api_client::ChatInviteLink = serde_json::from_str(
        r#"{"invite_link":"https://t.me/+abc","creator":{"id":1},"creates_join_request":true,
            "is_primary":false,"is_revoked":false,"member_limit":10,"pending_join_request_count":2}"#,
    )
    .expect("link");
    assert_eq!(link.url(), "https://t.me/+abc");
    assert_eq!(link.member_limit, Some(10));
    assert_eq!(link.pending_join_request_count, Some(2));
    assert_eq!(link.name, None);
}

#[test]
fn locations_compare_by_value_and_survive_nan() {
    let a: Location = serde_json::from_str(r#"{"latitude":51.05,"longitude":13.74}"#).expect("a");
    let b: Location = serde_json::from_str(r#"{"latitude":51.05,"longitude":13.74}"#).expect("b");
    assert_eq!(a, b);
    assert_eq!(a, a, "Eq is reflexive for a normal location");

    let nan = Location {
        latitude: f64::NAN,
        ..Location::default()
    };
    assert_eq!(nan, nan, "bitwise comparison keeps Eq sound for NaN");

    let slightly_off = Location {
        latitude: 51.050_000_1,
        ..Location::default()
    };
    assert_ne!(a, slightly_off);
}

#[test]
fn payment_payloads_keep_their_optional_parts() {
    let query: telegram_api_client::PreCheckoutQuery = serde_json::from_str(
        r#"{"id":"pc1","from":{"id":1},"currency":"EUR","total_amount":1450,
            "invoice_payload":"order-7","shipping_option_id":"express",
            "order_info":{"name":"Flo","email":"a@b.c",
                          "shipping_address":{"country_code":"DE","city":"Dresden"}}}"#,
    )
    .expect("query");
    assert_eq!(query.shipping_option_id.as_deref(), Some("express"));
    let order = query.order_info.expect("order info");
    assert_eq!(order.name.as_deref(), Some("Flo"));
    assert_eq!(
        order.shipping_address.map(|address| address.city),
        Some("Dresden".to_string())
    );

    let bare: telegram_api_client::PreCheckoutQuery =
        serde_json::from_str(r#"{"id":"pc2","currency":"XTR","total_amount":1}"#).expect("bare");
    assert_eq!(bare.shipping_option_id, None);
    assert_eq!(bare.order_info, None);
}

#[test]
fn subscription_state_is_carried_verbatim() {
    let update: telegram_api_client::BotSubscriptionUpdated = serde_json::from_str(
        r#"{"user":{"id":1},"invoice_payload":"sub1","state":"a-state-from-the-future"}"#,
    )
    .expect("subscription");
    assert_eq!(update.state, "a-state-from-the-future");

    let empty = telegram_api_client::BotSubscriptionUpdated::default();
    assert_eq!(empty.state, "");
    assert_eq!(empty.user, None);
}
