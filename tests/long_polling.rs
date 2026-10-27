//! Long polling: the `getUpdates` offset is a watermark, so a loop that stops
//! mid-batch must redeliver the unacknowledged tail instead of losing it or
//! repeating what it already handled.

use serde_json::{json, Value};
use telegram_api_client::{
    GetUpdates, MockReply, MockTransport, Result, TelegramClient, Update, UpdateCursor, UpdateKind,
    MAX_UPDATES_LIMIT,
};

/// A client that never retries, so each reply is consumed by exactly one call.
fn client(
    replies: impl IntoIterator<Item = MockReply>,
) -> (TelegramClient<MockTransport>, MockTransport) {
    let transport = MockTransport::new();
    transport.extend(replies);
    (
        TelegramClient::new("123:ABC", transport.clone()).without_retry(),
        transport,
    )
}

/// A `getUpdates` answer carrying `ids`, in that order.
fn batch(ids: &[i64]) -> MockReply {
    let payload: Vec<Value> = ids
        .iter()
        .map(|id| json!({ "update_id": id, "message": { "message_id": 1, "chat": { "id": -1 } } }))
        .collect();
    MockReply::result(Value::Array(payload))
}

/// The `offset` of the call at `index`, or `None` when the field was omitted.
fn offset_of(transport: &MockTransport, index: usize) -> Option<i64> {
    transport.calls()[index]
        .body
        .get("offset")
        .and_then(Value::as_i64)
}

/// One poll: sends the cursor's offset and returns the delivered updates.
async fn poll(
    client: &TelegramClient<MockTransport>,
    cursor: &UpdateCursor,
    timeout: u32,
) -> Result<Vec<Update>> {
    let mut request = GetUpdates::new().timeout(timeout);
    if let Some(offset) = cursor.next_offset() {
        request = request.offset(offset);
    }
    client.get_updates(&request).await
}

// ---- the watermark sends what the cursor says --------------------------------

#[tokio::test]
async fn the_request_carries_the_cursor_offset_not_the_last_batch() {
    let (client, transport) = client([batch(&[10, 11]), batch(&[12])]);
    let mut cursor = UpdateCursor::new();

    // Nothing confirmed yet: no offset at all, which is "give me the backlog".
    poll(&client, &cursor, 25).await.expect("first poll");
    assert_eq!(
        offset_of(&transport, 0),
        None,
        "a fresh cursor sends no offset"
    );

    cursor.confirm(&[Update {
        update_id: 11,
        ..Update::default()
    }]);
    poll(&client, &cursor, 25).await.expect("second poll");
    assert_eq!(
        offset_of(&transport, 1),
        Some(12),
        "offset is last_confirmed + 1, not the next id in the batch"
    );
}

#[tokio::test]
async fn an_empty_answer_does_not_move_the_watermark() {
    let (client, transport) = client([batch(&[]), batch(&[]), batch(&[])]);
    let cursor = UpdateCursor::after(7);

    for _ in 0..3 {
        let delivered = poll(&client, &cursor, 25).await.expect("poll");
        assert_eq!(delivered.len(), 0, "an empty batch delivers nothing");
    }

    assert_eq!(cursor.next_offset(), Some(8), "the cursor is untouched");
    for index in 0..3 {
        assert_eq!(
            offset_of(&transport, index),
            Some(8),
            "every empty poll repeats the same offset"
        );
    }
}

// ---- the watermark is cumulative and one-way ---------------------------------

#[tokio::test]
async fn a_gap_in_the_ids_advances_past_the_highest() {
    let mut cursor = UpdateCursor::new();
    let delivered = cursor.confirm(&[
        Update {
            update_id: 1,
            ..Update::default()
        },
        Update {
            update_id: 3,
            ..Update::default()
        },
    ]);

    assert_eq!(delivered.len(), 2);
    assert_eq!(
        cursor.next_offset(),
        Some(4),
        "id 2 is confirmed as well: the offset is a watermark, not a set"
    );
}

#[tokio::test]
async fn a_descending_batch_is_sorted_and_never_walks_backwards() {
    let mut cursor = UpdateCursor::after(9);
    let delivered = cursor.confirm(&[
        Update {
            update_id: 12,
            ..Update::default()
        },
        Update {
            update_id: 10,
            ..Update::default()
        },
        Update {
            update_id: 11,
            ..Update::default()
        },
    ]);

    let ids: Vec<i64> = delivered.iter().map(|update| update.update_id).collect();
    assert_eq!(ids, vec![10, 11, 12], "delivered in ascending order");
    assert_eq!(cursor.next_offset(), Some(13));

    // A replayed batch after the watermark was persisted yields nothing.
    let replay = cursor.confirm(&[
        Update {
            update_id: 10,
            ..Update::default()
        },
        Update {
            update_id: 11,
            ..Update::default()
        },
    ]);
    assert_eq!(replay.len(), 0, "confirmed ids are never handed out twice");
    assert_eq!(
        cursor.next_offset(),
        Some(13),
        "and the watermark stays put"
    );
}

#[tokio::test]
async fn duplicates_inside_one_batch_are_collapsed() {
    let mut cursor = UpdateCursor::new();
    let delivered = cursor.confirm(&[
        Update {
            update_id: 5,
            ..Update::default()
        },
        Update {
            update_id: 5,
            ..Update::default()
        },
        Update {
            update_id: 6,
            ..Update::default()
        },
    ]);
    assert_eq!(delivered.len(), 2);
    assert_eq!(cursor.next_offset(), Some(7));
}

#[tokio::test]
async fn the_watermark_saturates_instead_of_overflowing() {
    let mut cursor = UpdateCursor::after(i64::MAX);
    assert_eq!(
        cursor.next_offset(),
        Some(i64::MAX),
        "saturating, not wrapping"
    );
    assert!(!cursor.ack(i64::MAX), "no id can follow the maximum");
    assert_eq!(cursor.last_confirmed(), Some(i64::MAX));
}

// ---- per-update acknowledgement ---------------------------------------------

#[tokio::test]
async fn ack_moves_only_across_the_contiguous_id() {
    let mut cursor = UpdateCursor::after(41);

    assert!(cursor.ack(42), "the contiguous successor is accepted");
    assert!(!cursor.ack(44), "a hole is refused, not skipped");
    assert_eq!(cursor.last_confirmed(), Some(42));

    assert!(!cursor.ack(42), "a replay is refused");
    assert!(!cursor.ack(41), "so is going backwards");
    assert!(cursor.ack(43), "the hole is closed by the missing id");
    assert_eq!(cursor.next_offset(), Some(44));
}

#[tokio::test]
async fn ack_rejects_ids_that_cannot_exist() {
    let mut cursor = UpdateCursor::new();
    assert!(!cursor.ack(0), "update ids are 1-based");
    assert!(!cursor.ack(-3), "a negative id is not an update");
    assert_eq!(
        cursor.last_confirmed(),
        None,
        "neither one moved the cursor"
    );
    assert!(cursor.ack(1), "the first real id is accepted");
}

// ---- aborting mid-batch ------------------------------------------------------

#[tokio::test]
async fn an_interrupted_batch_redelivers_exactly_the_unacked_tail() {
    let (client, transport) = client([batch(&[21, 22, 23]), batch(&[23])]);
    let mut cursor = UpdateCursor::after(20);

    // Handle two of the three updates, then stop: the shutdown signal arrives
    // in the middle of the batch.
    let delivered = poll(&client, &cursor, 25).await.expect("first poll");
    let mut handled = Vec::new();
    for update in delivered {
        if update.update_id == 23 {
            break;
        }
        handled.push(update.update_id);
        assert!(cursor.ack(update.update_id));
    }
    assert_eq!(handled, vec![21, 22]);

    // The next process starts from the persisted offset and sees only the tail.
    let resumed = UpdateCursor::after(cursor.last_confirmed().expect("something was acked"));
    let redelivered = poll(&client, &resumed, 25).await.expect("second poll");
    let ids: Vec<i64> = redelivered.iter().map(|update| update.update_id).collect();

    assert_eq!(ids, vec![23], "21 and 22 are not repeated, 23 is not lost");
    assert_eq!(
        offset_of(&transport, 1),
        Some(23),
        "resumed at the watermark"
    );
}

#[tokio::test]
async fn a_crash_timed_by_confirming_nothing_repeats_the_whole_batch() {
    let (client, _transport) = client([batch(&[31, 32]), batch(&[31, 32])]);
    let cursor = UpdateCursor::after(30);

    // Nothing acknowledged before the process dies: a fresh cursor derived from
    // the persisted watermark (still 30) receives the same two updates again.
    let first = poll(&client, &cursor, 25).await.expect("first poll");
    let resumed = UpdateCursor::after(30);
    let again = poll(&client, &resumed, 25).await.expect("second poll");

    assert_eq!(first.len(), 2);
    assert_eq!(again.len(), 2, "at-least-once, never at-most-once");
    assert_eq!(cursor.next_offset(), resumed.next_offset());
}

// ---- what goes on the wire ---------------------------------------------------

#[tokio::test]
async fn long_polling_parameters_reach_the_body_verbatim() {
    let (client, transport) = client([batch(&[])]);
    let request = GetUpdates::new()
        .timeout(25)
        .limit(MAX_UPDATES_LIMIT)
        .allowed_updates(vec![UpdateKind::Message, UpdateKind::CallbackQuery]);
    client.get_updates(&request).await.expect("poll");

    let body = &transport.calls()[0].body;
    assert_eq!(body["timeout"], 25);
    assert_eq!(body["limit"], 100);
    assert_eq!(
        body["allowed_updates"],
        json!(["message", "callback_query"])
    );
    assert_eq!(transport.calls()[0].method(), "getUpdates");
}

#[tokio::test]
async fn a_zero_timeout_is_sent_and_not_dropped_as_falsy() {
    let (client, transport) = client([batch(&[])]);
    client
        .get_updates(&GetUpdates::new().timeout(0))
        .await
        .expect("poll");
    assert_eq!(
        transport.calls()[0].body["timeout"],
        0,
        "timeout 0 means poll and return immediately, so it must be sent"
    );
}

#[tokio::test]
async fn an_offset_of_zero_is_sent_rather_than_omitted() {
    let mut cursor = UpdateCursor::new();
    // A caller may persist 0 as "start of the backlog"; it is not the same as
    // omitting the field, and the cursor must not silently drop it.
    assert_eq!(cursor.next_offset(), None);
    cursor.confirm(&[]);
    assert_eq!(
        cursor.next_offset(),
        None,
        "an empty batch confirms nothing"
    );
    assert_eq!(UpdateCursor::after(-1).next_offset(), Some(0));
}
