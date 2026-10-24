//! Update parsing.

use telegram_api_client::{Chat, Update};

#[test]
fn parses_group_message_with_sender() {
    let json = r#"{
      "update_id": 100,
      "message": {
        "message_id": 7,
        "from": { "id": 42, "first_name": "Florian", "username": "flo" },
        "chat": { "id": -1001, "type": "supergroup" },
        "date": 1729600000,
        "text": "Hallo"
      }
    }"#;
    let update: Update = serde_json::from_str(json).unwrap();
    assert_eq!(update.update_id, 100);
    let message = update.message.as_ref().unwrap();
    assert_eq!(
        message.chat,
        Chat {
            id: -1001,
            chat_type: "supergroup".into(),
            ..Chat::default()
        }
    );
    assert_eq!(
        message.from.as_ref().unwrap().username.as_deref(),
        Some("flo")
    );
    assert_eq!(update.text(), Some("Hallo"));
}

#[test]
fn update_without_message_has_no_text() {
    let update: Update = serde_json::from_str(r#"{"update_id":1}"#).unwrap();
    assert_eq!(update.text(), None);
    assert_eq!(update.chat_id(), None);
}

#[test]
fn media_only_message_has_no_text() {
    let json = r#"{"update_id":2,"message":{"message_id":1,"chat":{"id":5,"type":"private"}}}"#;
    let update: Update = serde_json::from_str(json).unwrap();
    assert_eq!(update.text(), None);
    assert_eq!(update.chat_id(), Some(5));
}
