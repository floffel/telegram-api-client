//! Drain one `getUpdates` batch with exactly-once semantics:
//! `cargo run --example poll_once`.
//!
//! With `TELEGRAM_BOT_TOKEN` set it polls the real Bot API once. Without it, a
//! `MockTransport` replays a batch that contains a duplicate to show what
//! `UpdateCursor` is for — the same code path, no network.

// `Telegram`, `Bot` and `API` are proper nouns, not identifiers.
#![allow(clippy::doc_markdown)]

use std::env;

use serde_json::json;
use telegram_api_client::{
    GetUpdates, MockReply, MockTransport, ReqwestTransport, Result, TelegramClient, Transport,
    Update, UpdateCursor,
};

/// Polls once and confirms the batch: only updates past the cursor survive.
async fn drain<T: Transport>(
    client: &TelegramClient<T>,
    cursor: &mut UpdateCursor,
    label: &str,
) -> Result<()> {
    let mut request = GetUpdates::new().timeout(25);
    if let Some(offset) = cursor.next_offset() {
        request = request.offset(offset);
    }

    let updates = client.get_updates(&request).await?;
    let fresh = cursor.confirm(&updates);

    println!(
        "{label}: {} delivered, {} not seen before",
        updates.len(),
        fresh.len()
    );
    for update in &fresh {
        println!("  update {}: {:?}", update.update_id, update.text());
    }
    println!("  next offset: {:?}", cursor.next_offset());
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Resume after the last id the caller already processed; `UpdateCursor::new()`
    // starts at the beginning.
    let mut cursor = UpdateCursor::after(40);

    if let Ok(token) = env::var("TELEGRAM_BOT_TOKEN") {
        let client = TelegramClient::new(token, ReqwestTransport::new());
        drain(&client, &mut cursor, "live").await?;
    } else {
        // The same batch Telegram sends after a restart: id 41 twice (a
        // replayed delivery) and the next one.
        let payload = json!([
            {"update_id": 41, "message": {"message_id": 7, "chat": {"id": -1001}, "text": "first"}},
            {"update_id": 41, "message": {"message_id": 7, "chat": {"id": -1001}, "text": "first"}},
            {"update_id": 42, "message": {"message_id": 8, "chat": {"id": -1001}, "text": "second"}}
        ]);
        let batch: Vec<Update> =
            serde_json::from_value(payload.clone()).expect("the fixture batch is a valid payload");

        let transport = MockTransport::new();
        transport.push(MockReply::result(payload));
        let client = TelegramClient::new("123:ABC", transport);

        drain(&client, &mut cursor, "mock").await?;
        // The offset is the confirmation: Telegram reads `offset = n` as
        // "everything below n is handled". Re-delivering the same batch
        // therefore repeats nothing, which is what makes the loop crash-safe.
        println!("redelivery: {} new", cursor.confirm(&batch).len());
    }

    Ok(())
}
