//! Verify and parse one webhook delivery: `cargo run --example webhook`.
//!
//! No network and no bot token: the guard checks the secret header and the
//! path derived from the secret, then the body is parsed into a typed `Update`.
//! Run it as-is to see both the accepted and the rejected case.

// `Telegram` is a proper noun, not an identifier.
#![allow(clippy::doc_markdown)]

use telegram_api_client::WebhookGuard;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let secret = std::env::var("TELEGRAM_WEBHOOK_SECRET").unwrap_or_else(|_| "s3cret".to_owned());
    // An empty secret is an error, not "no auth configured": `new` refuses it.
    let guard = WebhookGuard::new(secret.clone())?;

    // Point the Bot API at this path in `setWebhook`: it is `sha256(secret)`,
    // so a scanner cannot hit the endpoint without knowing the secret.
    println!("webhook path: {}", guard.derived_path());

    let body = br#"{"update_id":1,"message":{"message_id":2,"chat":{"id":-3},"text":"hi"}}"#;

    let update = guard.verify_and_parse(Some(&secret), guard.derived_path(), body)?;
    println!(
        "verified update {} from chat {:?}: {:?}",
        update.update_id,
        update.chat_id(),
        update.text()
    );

    // Wrong secret, missing header and wrong path are rejected before the body
    // is parsed - the comparison is constant time, and an absent header fails
    // closed.
    for attempt in [
        guard.verify_and_parse(Some("nope"), guard.derived_path(), body),
        guard.verify_and_parse(None, guard.derived_path(), body),
        guard.verify_and_parse(Some(&secret), "/elsewhere", body),
    ] {
        println!("rejected: {}", attempt.unwrap_err());
    }

    Ok(())
}
