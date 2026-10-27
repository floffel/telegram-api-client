//! Fuzz target: `WebhookGuard::verify_and_parse` on arbitrary header, path and
//! body.
//!
//! Input is NUL separated: `secret \0 header \0 path \0 body`, where a missing
//! `header` stands for an absent `X-Telegram-Bot-Api-Secret-Token`.
//!
//! Invariants:
//! 1. Nothing in the request can make verification panic.
//! 2. A wrong or missing secret is always `Err`, and it is the *first* error —
//!    the body is not even looked at.
//! 3. `Payload` is only ever reported after both checks passed, so a caller can
//!    never mistake a forged request for a malformed one.
//! 4. A guard accepts its own secret and its own derived path; a same-length
//!    neighbour is refused.

#![no_main]

use libfuzzer_sys::fuzz_target;
use telegram_api_client::{Error, WebhookGuard};

fuzz_target!(|data: &[u8]| {
    let mut chunks = data.splitn(4, |b| *b == 0);
    let secret = String::from_utf8_lossy(chunks.next().unwrap_or_default()).into_owned();
    let header = chunks
        .next()
        .map(|raw| String::from_utf8_lossy(raw).into_owned());
    let path = String::from_utf8_lossy(chunks.next().unwrap_or_default()).into_owned();
    let body = chunks.next().unwrap_or_default();

    let Ok(guard) = WebhookGuard::new(secret.clone()) else {
        // Only secrets Telegram would reject end up here; they never verify.
        return;
    };
    let derived = guard.derived_path().to_owned();

    assert!(guard.verify_secret(Some(&secret)).is_ok(), "own secret");
    assert!(guard.verify_secret(None).is_err(), "missing header");
    assert!(guard.verify_secret(Some("")).is_err(), "empty header");
    assert!(guard.verify_path(&derived).is_ok(), "own path");
    assert!(guard.verify_path("").is_err(), "empty path");

    // A same-length neighbour must not be accepted, whatever the secret is.
    let neighbour: String = secret
        .chars()
        .map(|c| if c == 'a' { 'b' } else { 'a' })
        .collect();
    assert!(guard.verify_secret(Some(&neighbour)).is_err(), "neighbour");

    let result = guard.verify_and_parse(header.as_deref(), &path, body);
    let secret_matches = header.as_deref() == Some(secret.as_str());
    let path_matches = path == derived;

    match result {
        Ok(_) => {
            assert!(secret_matches, "parsed with a wrong secret");
            assert!(path_matches, "parsed with a wrong path");
        }
        Err(Error::SecretMismatch) => assert!(!secret_matches, "mismatch with the right secret"),
        Err(Error::PathMismatch) => {
            assert!(secret_matches, "path checked before the secret");
            assert!(!path_matches, "mismatch with the right path");
        }
        Err(Error::Payload { .. }) => {
            assert!(
                secret_matches && path_matches,
                "payload error before verification"
            );
        }
        Err(other) => panic!("unexpected error from verify_and_parse: {other}"),
    }
});
