//! A long-polling loop that ends cleanly: `cargo run --example poll_loop`.
//!
//! With `TELEGRAM_BOT_TOKEN` set it polls the real Bot API until the process is
//! interrupted (Ctrl-C ends it, because the loop only confirms what it handled).
//! Without it, a `MockTransport` replays two batches and a stop between them, to
//! show what the watermark does across a restart.

// `Telegram`, `Bot` and `API` are proper nouns, not identifiers.
#![allow(clippy::doc_markdown)]

use std::env;
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::json;
use telegram_api_client::{
    GetUpdates, MockReply, MockTransport, ReqwestTransport, Result, TelegramClient, Transport,
    UpdateCursor,
};

/// Polls once: handles every update in the batch and acknowledges it one by one.
///
/// Acknowledging per update instead of once for the batch is what makes an
/// interruption cheap: the offset stops at the last *handled* update, so a
/// restart redelivers the tail and nothing else.
async fn poll_once<T: Transport>(
    client: &TelegramClient<T>,
    cursor: &mut UpdateCursor,
    stop: &AtomicBool,
    label: &str,
) -> Result<usize> {
    let mut request = GetUpdates::new().timeout(25);
    if let Some(offset) = cursor.next_offset() {
        request = request.offset(offset);
    }

    let mut handled = 0;
    for update in client.get_updates(&request).await? {
        // The flag is checked between updates, not inside an HTTP call: a
        // dropped request confirms nothing, so ending here is always safe.
        if stop.load(Ordering::Relaxed) {
            println!(
                "{label}: ending before update {}, it stays unconfirmed",
                update.update_id
            );
            break;
        }
        if let Some(text) = update.text() {
            println!("{label}: update {} says {text:?}", update.update_id);
        }
        cursor.ack(update.update_id);
        handled += 1;
    }

    println!(
        "{label}: {handled} handled, next offset {:?}",
        cursor.next_offset()
    );
    Ok(handled)
}

/// The loop a real bot runs. Returns once the stop flag is raised.
async fn run<T: Transport>(
    client: &TelegramClient<T>,
    cursor: &mut UpdateCursor,
    stop: &AtomicBool,
) -> Result<usize> {
    let mut total = 0;
    while !stop.load(Ordering::Relaxed) {
        total += poll_once(client, cursor, stop, "loop").await?;
    }
    Ok(total)
}

#[tokio::main]
async fn main() -> Result<()> {
    let stop = AtomicBool::new(false);

    if let Ok(token) = env::var("TELEGRAM_BOT_TOKEN") {
        let client = TelegramClient::new(token, ReqwestTransport::new());
        let mut cursor = UpdateCursor::new();
        println!("polling the Bot API until interrupted");
        let total = run(&client, &mut cursor, &stop).await?;
        println!("{total} updates handled");
        return Ok(());
    }

    // Offline: the same functions, fed by a mock that answers one batch and then
    // the tail after a stop.
    let transport = MockTransport::new();
    transport.push(MockReply::result(json!([
        {"update_id": 7, "message": {"message_id": 1, "chat": {"id": -1}, "text": "hello"}},
        {"update_id": 8, "message": {"message_id": 2, "chat": {"id": -1}, "text": "world"}}
    ])));
    transport.push(MockReply::result(json!([
        {"update_id": 9, "message": {"message_id": 3, "chat": {"id": -1}, "text": "after the stop"}}
    ])));
    let client = TelegramClient::new("123:ABC", transport.clone());

    let mut cursor = UpdateCursor::new();
    poll_once(&client, &mut cursor, &stop, "before stop").await?;

    // A signal arrives between two polls; the loop notices it before its next
    // request and returns without confirming anything further.
    stop.store(true, Ordering::Relaxed);
    let total = run(&client, &mut cursor, &stop).await?;
    assert_eq!(
        total, 0,
        "a raised flag ends the loop before the next request"
    );

    // Persist the watermark; a restart resumes from it and repeats nothing that
    // was acknowledged before the stop.
    let persisted = cursor.next_offset();
    println!("persisted offset: {persisted:?}");

    let mut resumed = UpdateCursor::after(cursor.last_confirmed().unwrap_or_default());
    let mut request = GetUpdates::new();
    if let Some(offset) = persisted {
        request = request.offset(offset);
    }
    let rest = client.get_updates(&request).await?;
    for update in &rest {
        resumed.ack(update.update_id);
    }
    println!(
        "after restart: {} update(s) redelivered, next offset {:?}",
        rest.len(),
        resumed.next_offset()
    );
    assert_eq!(rest.len(), 1, "the mock replays exactly the unhandled tail");
    assert_eq!(
        persisted,
        Some(9),
        "the watermark sits past both handled updates"
    );

    Ok(())
}
