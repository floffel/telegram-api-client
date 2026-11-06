//! Compile-time contract for received payload types versus caller-built requests.

use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use telegram_api_client::{
    ChatMember, ChatMemberStatus, ChatMemberUpdated, File, GetChat, GetFile, Link, MaskPoint,
    MaskPosition, MessageReactionCountUpdated, PollMedia, ReactionCount, Update, User, WebhookInfo,
};

fn round_trip<T>(value: &T)
where
    T: Serialize + DeserializeOwned + PartialEq + Debug,
{
    let encoded = serde_json::to_value(value).expect("serialize");
    let decoded: T = serde_json::from_value(encoded).expect("deserialize");
    assert_eq!(&decoded, value);
}

#[test]
fn received_payloads_are_constructible_with_defaults_and_constructors() {
    let mut update = Update::new(42);
    update.message = Some(Box::default());
    round_trip(&update);

    let mut file = File::new("AgACAgI", "unique");
    file.file_size = Some(1024);
    file.file_path = Some("photos/a.jpg".into());
    round_trip(&file);

    let position = MaskPosition::new(MaskPoint::Eyes, 0.1, -0.2, 1.5);
    round_trip(&position);

    let mut link = Link::default();
    link.url = "https://example.com".into();
    round_trip(&PollMedia::link(link));

    let mut member_change = ChatMemberUpdated::default();
    member_change.old_chat_member = ChatMember::default();
    member_change.new_chat_member = ChatMember::new(ChatMemberStatus::Member, User::default());
    round_trip(&member_change);

    let mut reaction = ReactionCount::default();
    reaction.total_count = 3;
    let mut reactions = MessageReactionCountUpdated::default();
    reactions.reactions = vec![reaction];
    round_trip(&reactions);

    round_trip(&WebhookInfo::default());
}

#[test]
fn request_types_keep_exhaustive_struct_literals() {
    let get_chat = GetChat {
        chat_id: (-1001).into(),
    };
    assert_eq!(get_chat.chat_id, (-1001).into());

    let get_file = GetFile {
        file_id: "AgACAgI".into(),
    };
    assert_eq!(get_file.file_id, "AgACAgI");
}

#[test]
fn every_received_payload_struct_is_non_exhaustive() {
    let sources = [
        ("src/update.rs", include_str!("../src/update.rs")),
        ("src/events.rs", include_str!("../src/events.rs")),
        ("src/media.rs", include_str!("../src/media.rs")),
        ("src/chat.rs", include_str!("../src/chat.rs")),
        ("src/file.rs", include_str!("../src/file.rs")),
        ("src/webhook_api.rs", include_str!("../src/webhook_api.rs")),
        ("src/edit.rs", include_str!("../src/edit.rs")),
    ];
    let received = [
        ("src/update.rs", "Update"),
        ("src/update.rs", "Message"),
        ("src/update.rs", "User"),
        ("src/update.rs", "Chat"),
        ("src/update.rs", "CallbackQuery"),
        ("src/update.rs", "InlineQuery"),
        ("src/events.rs", "ReactionCount"),
        ("src/events.rs", "MessageReactionUpdated"),
        ("src/events.rs", "MessageReactionCountUpdated"),
        ("src/events.rs", "BusinessBotRights"),
        ("src/events.rs", "BusinessConnection"),
        ("src/events.rs", "BusinessMessagesDeleted"),
        ("src/events.rs", "ChatInviteLink"),
        ("src/events.rs", "ChatMemberUpdated"),
        ("src/events.rs", "ChatJoinRequest"),
        ("src/events.rs", "ChatBoost"),
        ("src/events.rs", "ChatBoostUpdated"),
        ("src/events.rs", "ChatBoostRemoved"),
        ("src/events.rs", "ShippingAddress"),
        ("src/events.rs", "ShippingQuery"),
        ("src/events.rs", "OrderInfo"),
        ("src/events.rs", "PreCheckoutQuery"),
        ("src/events.rs", "PaidMediaPurchased"),
        ("src/events.rs", "Location"),
        ("src/events.rs", "ChosenInlineResult"),
        ("src/events.rs", "PollOption"),
        ("src/events.rs", "Poll"),
        ("src/events.rs", "PollAnswer"),
        ("src/events.rs", "ManagedBotUpdated"),
        ("src/events.rs", "BotSubscriptionUpdated"),
        ("src/events.rs", "MessageGenerationStopped"),
        ("src/media.rs", "PhotoSize"),
        ("src/media.rs", "Link"),
        ("src/media.rs", "LivePhoto"),
        ("src/media.rs", "Animation"),
        ("src/media.rs", "Audio"),
        ("src/media.rs", "Document"),
        ("src/media.rs", "Venue"),
        ("src/media.rs", "VideoQuality"),
        ("src/media.rs", "Video"),
        ("src/media.rs", "MaskPosition"),
        ("src/media.rs", "Sticker"),
        ("src/media.rs", "PollMedia"),
        ("src/chat.rs", "ChatMemberOwner"),
        ("src/chat.rs", "ChatMemberAdministrator"),
        ("src/chat.rs", "ChatMemberMember"),
        ("src/chat.rs", "ChatMemberRestricted"),
        ("src/chat.rs", "ChatMemberLeft"),
        ("src/chat.rs", "ChatMemberBanned"),
        ("src/file.rs", "File"),
        ("src/webhook_api.rs", "WebhookInfo"),
        ("src/edit.rs", "MessageId"),
    ];

    for (path, name) in received {
        let source = sources
            .iter()
            .find_map(|(candidate, source)| (*candidate == path).then_some(*source))
            .unwrap_or_else(|| panic!("missing source {path}"));
        let declaration = format!("pub struct {name} ");
        let offset = source
            .find(&declaration)
            .unwrap_or_else(|| panic!("{path}: no declaration for {name}"));
        let declaration_line = source[offset..].lines().next().unwrap_or_default();
        let prefix = &source[..offset];
        let attributes = prefix
            .lines()
            .rev()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with("///"))
            .take_while(|line| line.starts_with("#["))
            .collect::<Vec<_>>();
        assert!(
            attributes.contains(&"#[non_exhaustive]"),
            "{path}: {declaration_line} must be #[non_exhaustive]"
        );
    }
}

#[test]
fn message_id_without_default_still_keeps_future_fields_compatible() {
    let source = include_str!("../src/edit.rs");
    let declaration = source
        .find("pub struct MessageId ")
        .expect("MessageId declaration");
    let attributes = source[..declaration]
        .lines()
        .rev()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("///"))
        .take_while(|line| line.starts_with("#["))
        .collect::<Vec<_>>();
    assert!(attributes.contains(&"#[non_exhaustive]"));

    let maximum = r#"{"message_id":9223372036854775807}"#;
    let decoded: telegram_api_client::MessageId = serde_json::from_str(maximum).unwrap();
    round_trip(&decoded);
}
