//! Fuzz target: `parse_retry_after` on an arbitrary description.
//!
//! The description arrives in a response body, so it is attacker-influenced input:
//! arbitrary UTF-8, NUL bytes, absurd lengths, markers in the middle of
//! multi-byte characters.
//!
//! Invariants:
//! 1. Parsing never panics, and never allocates in proportion to the input.
//! 2. A hint that comes back is a real wait: never zero (issue #42).
//! 3. A hint that was read can be read again, so the bounded scan (issue #43)
//!    does not depend on where the number sits in the description.

#![no_main]

use libfuzzer_sys::fuzz_target;
use telegram_api_client::parse_retry_after;

fuzz_target!(|data: &[u8]| {
    let Ok(description) = std::str::from_utf8(data) else {
        return;
    };

    if let Some(seconds) = parse_retry_after(description) {
        assert!(seconds > 0, "a zero-second hint is not a hint");
        assert_eq!(
            parse_retry_after(&format!("retry after {seconds}")),
            Some(seconds),
            "{}",
            description.len()
        );
    }
});
