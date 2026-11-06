//! Formatting-entity and edit/copy request ranges (#37, #38, #39).

use telegram_api_client::{
    CopyMessage, EditMessageCaption, EditMessageMedia, EditMessageText, Error, InputMedia,
    MessageEntity, MockTransport, SendMediaGroup, SendMessage, SendPhoto, TelegramClient,
};

const CHAT: i64 = -1001;

fn invalid_field(result: telegram_api_client::Result<()>) -> &'static str {
    match result {
        Err(Error::InvalidRequest { field, .. }) => field,
        other => panic!("expected InvalidRequest, got {other:?}"),
    }
}

#[test]
fn edit_text_entities_use_utf16_ranges_and_boundaries() {
    let after_emoji =
        EditMessageText::new(CHAT, 7, "🎯x").with_entities(vec![MessageEntity::bold(2, 1)]);
    assert!(
        after_emoji.validate().is_ok(),
        "offset 2 starts after the two-unit emoji"
    );

    let whole = EditMessageText::new(CHAT, 7, "🎯x").with_entities(vec![MessageEntity::bold(0, 3)]);
    assert!(whole.validate().is_ok(), "three UTF-16 code units in total");

    for entity in [
        MessageEntity::bold(-1, 1),
        MessageEntity::bold(0, 0),
        MessageEntity::bold(0, -1),
        MessageEntity::bold(2, 2),
        MessageEntity::bold(4, 0),
        MessageEntity::bold(i64::MAX, 1),
    ] {
        let request = EditMessageText::new(CHAT, 7, "🎯x").with_entities(vec![entity.clone()]);
        assert_eq!(invalid_field(request.validate()), "entities", "{entity:?}");
    }
}

#[test]
fn caption_entity_ranges_are_checked_for_edits_and_copies() {
    let mut edit = EditMessageCaption::new(CHAT, 7).with_caption("🎯x");
    edit.caption_entities = vec![MessageEntity::bold(0, 3)];
    assert!(edit.validate().is_ok());

    edit.caption_entities = vec![MessageEntity::bold(3, 1)];
    assert_eq!(invalid_field(edit.validate()), "caption_entities");

    let mut copy = CopyMessage::new(CHAT, -1002, 7).with_caption("🎯x");
    copy.caption_entities = vec![MessageEntity::bold(0, 3)];
    assert!(copy.validate().is_ok());

    copy.caption_entities = vec![MessageEntity::bold(0, 4)];
    assert_eq!(invalid_field(copy.validate()), "caption_entities");
}

#[test]
fn entities_without_text_are_rejected() {
    let mut edit = EditMessageCaption::new(CHAT, 7);
    edit.caption_entities = vec![MessageEntity::bold(0, 1)];
    assert_eq!(invalid_field(edit.validate()), "caption_entities");

    let mut copy = CopyMessage::new(CHAT, -1002, 7);
    copy.caption_entities = vec![MessageEntity::bold(0, 1)];
    assert_eq!(invalid_field(copy.validate()), "caption_entities");
}

#[test]
fn plain_sends_and_album_captions_share_the_entity_check() {
    let invalid = SendMessage::new(CHAT, "🎯").with_entities(vec![MessageEntity::bold(0, 3)]);
    assert_eq!(invalid_field(invalid.validate()), "entities");

    let invalid = SendPhoto::new(CHAT, "photo").with_caption("x");
    let mut invalid = invalid;
    invalid.options.caption_entities = vec![MessageEntity::bold(1, 1)];
    assert_eq!(invalid_field(invalid.validate()), "caption_entities");

    let mut item = InputMedia::photo("a").with_caption("x");
    item.caption_entities = vec![MessageEntity::bold(0, 2)];
    let album = SendMediaGroup::new(CHAT, vec![item, InputMedia::photo("b")]);
    assert_eq!(invalid_field(album.validate()), "caption_entities");

    let mut media = InputMedia::photo("photo").with_caption("x");
    media.caption_entities = vec![MessageEntity::bold(0, 2)];
    let edit = EditMessageMedia::new(CHAT, 7, media);
    assert_eq!(invalid_field(edit.validate()), "caption_entities");
}

#[test]
fn copy_reply_target_must_be_positive_when_present() {
    for reply_to in [None, Some(1), Some(i64::MAX)] {
        let mut copy = CopyMessage::new(CHAT, -1002, 7);
        copy.reply_to_message_id = reply_to;
        assert!(copy.validate().is_ok(), "reply_to_message_id={reply_to:?}");
    }

    for reply_to in [Some(0), Some(-1), Some(i64::MIN)] {
        let mut copy = CopyMessage::new(CHAT, -1002, 7);
        copy.reply_to_message_id = reply_to;
        assert_eq!(
            invalid_field(copy.validate()),
            "reply_to_message_id",
            "reply_to_message_id={reply_to:?}"
        );
    }
}

#[tokio::test]
async fn invalid_entities_and_reply_ids_stop_before_the_transport() {
    let transport = MockTransport::new();
    let client = TelegramClient::new("123:ABC", transport.clone()).without_retry();

    let edit = EditMessageText::new(CHAT, 7, "x").with_entities(vec![MessageEntity::bold(0, 2)]);
    let error = client.edit_message_text(&edit).await.unwrap_err();
    assert_eq!(invalid_field(Err(error)), "entities");

    let mut copy = CopyMessage::new(CHAT, -1002, 7);
    copy.reply_to_message_id = Some(0);
    let error = client.copy_message(&copy).await.unwrap_err();
    assert_eq!(invalid_field(Err(error)), "reply_to_message_id");

    assert_eq!(transport.calls(), Vec::new());
}
