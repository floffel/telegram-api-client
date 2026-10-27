//! Fuzz target: `serde_json::from_slice::<Update>` on arbitrary bytes.
//!
//! What a webhook or a `getUpdates` page can carry is not under our control, so
//! the parser has to survive anything: broken UTF-8, truncated objects, absurd
//! nesting, unknown fields and unknown enum values.
//!
//! Invariants:
//! 1. Parsing never panics, and every accessor on an accepted update is safe.
//! 2. Parsing is permissive but not lossy: an accepted update serialises back
//!    to JSON that parses to the same value. Otherwise a caller that stores the
//!    re-serialised update (a queue, a log) silently loses information.

#![no_main]

use libfuzzer_sys::fuzz_target;
use telegram_api_client::Update;

fuzz_target!(|data: &[u8]| {
    let Ok(update) = serde_json::from_slice::<Update>(data) else {
        return;
    };

    // Accessors must be total on whatever was accepted.
    let _ = update.text();
    let _ = update.chat_id();
    let _ = update.callback_data();
    let _ = update.callback_query_id();
    if let Some(message) = update.primary_message() {
        let _ = message.chat.label();
        if let Some(from) = message.from.as_ref() {
            let _ = from.display_name();
        }
    }
    if let Some(from) = update
        .callback_query
        .as_ref()
        .and_then(|query| query.from.as_ref())
    {
        let _ = from.display_name();
    }

    // Round trip through the crate's own serialisation.
    let rendered = serde_json::to_vec(&update).expect("Update serialises");
    let reparsed: Update = serde_json::from_slice(&rendered).expect("own output parses back");
    assert_eq!(update, reparsed, "serialisation lost information");
});
