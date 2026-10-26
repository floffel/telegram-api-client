//! Send one text message: `cargo run --example send_text`.
//!
//! Needs the `http` feature (on by default) and two environment variables:
//!
//! ```text
//! TELEGRAM_BOT_TOKEN=123456:ABC-DEF \
//! TELEGRAM_CHAT_ID=-1001234567890 \
//! cargo run --example send_text
//! ```
//!
//! Without the token it prints the JSON body it would POST and exits, so the
//! example is runnable — and CI-buildable — on a machine with no bot.

// `Telegram` and `JSON` are proper nouns, not identifiers - the same allowance
// `lib.rs` makes for the crate itself.
#![allow(clippy::doc_markdown)]

use std::env;

use telegram_api_client::{ReqwestTransport, SendMessage, TelegramClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chat_id: i64 = env::var("TELEGRAM_CHAT_ID")
        .ok()
        .and_then(|raw| raw.parse().ok())
        .unwrap_or(-1001);
    // `SendMessage::new` takes the chat as `i64`, `@username` or an invite link.
    let request = SendMessage::new(chat_id, "Hello from the telegram-api-client example.");

    let Ok(token) = env::var("TELEGRAM_BOT_TOKEN") else {
        // Building the request is already the whole validation pass: empty
        // text, an over-long body and a `chat_id` of 0 are typed errors here,
        // not a 400 from the Bot API.
        println!("TELEGRAM_BOT_TOKEN not set - request body instead:");
        println!("{}", serde_json::to_string_pretty(&request)?);
        return Ok(());
    };

    // The token is redacted in `Debug`; the client never prints it.
    let client = TelegramClient::new(token, ReqwestTransport::new());

    match client.send_message_request(&request).await {
        Ok(message) => println!("sent as message {}", message.message_id),
        // A 5xx or a connection lost after the request was written is
        // *ambiguous*: the message may already be in the chat. The client does
        // not retry it, because Telegram has no idempotency key and a retry
        // would post the text twice.
        Err(err) if err.is_ambiguous() => {
            eprintln!("outcome unknown - check the chat before resending: {err}");
        }
        // 429 and pre-write connection failures were already retried inside the
        // client; anything reaching this arm is terminal.
        Err(err) => return Err(err.into()),
    }

    Ok(())
}
