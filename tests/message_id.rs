//! `copyMessage` results must carry an explicit, non-negative message id
//! (#34).

use serde_json::json;
use telegram_api_client::{
    CopyMessage, Error, MessageId, MockReply, MockTransport, TelegramClient,
};

#[test]
fn message_id_requires_an_explicit_non_negative_integer() {
    for invalid in [
        json!({}),
        json!({"message_id": null}),
        json!({"messageId": 7}),
        json!({"message_id": -1}),
        json!({"message_id": i64::MIN}),
        json!({"message_id": "7"}),
    ] {
        assert!(
            serde_json::from_value::<MessageId>(invalid.clone()).is_err(),
            "{invalid} must not decode"
        );
    }

    let pending: MessageId = serde_json::from_value(json!({"message_id": 0})).unwrap();
    assert_eq!(pending.message_id, 0);
    assert!(pending.is_pending());

    let sent: MessageId = serde_json::from_value(json!({"message_id": 7})).unwrap();
    assert_eq!(sent.message_id, 7);
    assert!(!sent.is_pending());
}

#[test]
fn message_id_round_trips_zero_and_positive_values() {
    for message_id in [0, 1, i64::MAX] {
        let wire = json!({"message_id": message_id});
        let decoded: MessageId = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), wire);
    }
}

#[test]
fn direct_construction_does_not_erase_the_wire_invariant() {
    let invalid = MessageId { message_id: -1 };
    assert!(!invalid.is_pending());
    let encoded = serde_json::to_value(invalid).unwrap();
    assert!(
        serde_json::from_value::<MessageId>(encoded).is_err(),
        "negative values cannot enter through deserialization"
    );
}

fn payload_cause(error: &Error) -> &Error {
    let Error::Ambiguous { source, .. } = error else {
        panic!("expected Ambiguous, got {error:?}");
    };
    source
        .as_deref()
        .and_then(|source| source.downcast_ref::<Error>())
        .unwrap_or_else(|| panic!("expected payload cause, got {error:?}"))
}

async fn copy_with(result: serde_json::Value) -> Result<MessageId, Error> {
    let transport = MockTransport::new();
    transport.push(MockReply::result(result));
    let client = TelegramClient::try_new("123:ABC", transport)
        .unwrap()
        .without_retry();
    client.copy_message(&CopyMessage::new(1, -1001, 7)).await
}

#[tokio::test]
async fn copy_message_rejects_every_bad_result_shape() {
    for invalid in [
        json!({}),
        json!({"message_id": null}),
        json!({"messageId": 7}),
        json!({"message_id": -1}),
        json!({"message_id": i64::MIN}),
        json!({"message_id": "7"}),
    ] {
        let error = copy_with(invalid.clone())
            .await
            .expect_err("a malformed copyMessage result is ambiguous");
        assert!(
            matches!(payload_cause(&error), Error::Payload { message, .. } if message.contains("copyMessage")),
            "{invalid}: {error:?}"
        );
        assert!(!error.is_retryable(), "{invalid}: {error:?}");
    }
}

#[tokio::test]
async fn copy_message_exposes_scheduled_and_sent_ids() {
    let pending = copy_with(json!({"message_id": 0}))
        .await
        .expect("zero is a documented scheduled-copy result");
    assert!(pending.is_pending());

    let sent = copy_with(json!({"message_id": 99}))
        .await
        .expect("a positive id is a sent copy");
    assert_eq!(sent.message_id, 99);
    assert!(!sent.is_pending());
}
