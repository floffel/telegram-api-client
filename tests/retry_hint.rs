//! Retry hints: the two sources of a 429 (`parameters.retry_after` and the
//! description), the boundary on both sides of every limit in the parser, and the
//! guarantee that a hostile description cannot make it allocate or scan without
//! bound (issues #42 and #43).

use std::time::Duration;

use proptest::prelude::*;
use serde_json::{json, Value};
use telegram_api_client::{parse_retry_after, Error, RetryPolicy, MAX_RETRY_HINT_SCAN_BYTES};

/// A 429 body with a description and a raw `parameters.retry_after` value, so a
/// test can express an absent, negative, fractional or zero hint.
fn rate_limited(description: &str, explicit: &Value) -> Error {
    let body = json!({
        "ok": false,
        "error_code": 429,
        "description": description,
        "parameters": { "retry_after": explicit },
    });
    Error::from_api_body(&body, Some(429)).expect("a 429 body is an error")
}

/// The `retry_after()` in whole seconds, which is what the accessor returns.
fn secs(error: &Error) -> Option<u64> {
    error.retry_after().map(|wait| wait.as_secs())
}

#[test]
fn explicit_zero_falls_back_to_the_description() {
    // Issue #42: `parameters.retry_after: 0` used to become `Some(0)`, shadowing
    // the usable textual hint and retrying with no wait at all.
    let error = rate_limited("Too Many Requests: retry after 6", &json!(0));
    assert_eq!(
        secs(&error),
        Some(6),
        "the textual hint is the only usable one"
    );
    assert_eq!(
        error.retry_after(),
        Some(Duration::from_secs(6)),
        "and it is the same value on both accessors"
    );
}

#[test]
fn explicit_zero_without_a_usable_description_is_no_hint_at_all() {
    for description in ["Too Many Requests", "retry after 0", "retry after "] {
        let error = rate_limited(description, &json!(0));
        assert_eq!(secs(&error), None, "{description}");
        assert_eq!(
            error.to_string(),
            "rate limited (429): retry later",
            "{description}"
        );
    }
}

#[test]
fn explicit_one_is_the_smallest_accepted_hint() {
    // The boundary on the accepted side: one second is a hint, zero is not.
    let error = rate_limited("Too Many Requests", &json!(1));
    assert_eq!(secs(&error), Some(1));
    assert_eq!(error.to_string(), "rate limited (429): retry after 1s");
}

#[test]
fn explicit_positive_hint_wins_over_the_description() {
    let error = rate_limited("Too Many Requests: retry after 2", &json!(9));
    assert_eq!(secs(&error), Some(9));
}

#[test]
fn explicit_hint_is_never_clamped_by_the_parser() {
    // A long hint belongs to the caller: the policy decides not to retry, the
    // parser does not shorten the wait (clamping is how a retry loop walks into a
    // second 429).
    let error = rate_limited("Too Many Requests", &json!(86_400));
    assert_eq!(secs(&error), Some(86_400));
    assert!(
        !RetryPolicy::default().should_retry(1, &error),
        "a hint above max_delay is handed back, not shortened"
    );
}

#[test]
fn malformed_explicit_hints_are_absent_hints() {
    // Only a non-negative integer is a hint. Anything else is treated exactly like
    // an absent field, and the description still gets its chance.
    for explicit in [
        json!(-1),
        json!(4.5),
        json!("7"),
        json!(null),
        json!([7]),
        json!({}),
    ] {
        assert_eq!(
            secs(&rate_limited("Too Many Requests", &explicit)),
            None,
            "{explicit}"
        );
        assert_eq!(
            secs(&rate_limited("Too Many Requests: retry after 8", &explicit)),
            Some(8),
            "{explicit}"
        );
    }
}

#[test]
fn the_scan_window_boundary_is_exact_on_both_sides() {
    // A marker whose digits still fit inside the window is read.
    let inside = "x".repeat(MAX_RETRY_HINT_SCAN_BYTES - 13);
    assert_eq!(
        parse_retry_after(&format!("{inside}retry after 5")),
        Some(5)
    );

    // One byte further and the digit falls outside the window: no hint, even
    // though the marker itself is still visible.
    let outside = "x".repeat(MAX_RETRY_HINT_SCAN_BYTES - 12);
    assert_eq!(parse_retry_after(&format!("{outside}retry after 5")), None);

    // A marker megabytes in is not a hint either, and finding that out must not
    // depend on the length of the description.
    let far = "x".repeat(1 << 20);
    assert_eq!(parse_retry_after(&format!("{far}retry after 5")), None);
    assert_eq!(parse_retry_after(&format!("retry after 5{far}")), Some(5));
}

#[test]
fn the_window_is_cut_on_a_char_boundary() {
    // 512 is not a char boundary here, so the window has to back off instead of
    // slicing a multi-byte character in half and panicking.
    let prefix = format!("a{}", "ä".repeat(300));
    assert_eq!(parse_retry_after(&format!("{prefix}retry after 5")), None);

    // The same text with the marker inside the window still parses.
    let short = "ä".repeat(200);
    assert_eq!(parse_retry_after(&format!("{short}retry after 5")), Some(5));
}

#[test]
fn digits_are_bounded_at_the_width_of_u64() {
    // 20 digits that fit are still a hint ...
    assert_eq!(
        parse_retry_after("retry after 18446744073709551615"),
        Some(u64::MAX)
    );
    // ... 20 digits that overflow are not ...
    assert_eq!(parse_retry_after("retry after 18446744073709551616"), None);
    // ... and neither is a 21-digit run, which no wait in seconds can be.
    assert_eq!(parse_retry_after("retry after 123456789012345678901"), None);
    assert_eq!(
        parse_retry_after("retry after 0000000000000000000005"),
        Some(5)
    );
    assert_eq!(parse_retry_after("retry after 0000"), None);
}

#[test]
fn markers_are_case_insensitive_and_the_leftmost_one_wins() {
    assert_eq!(parse_retry_after("RETRY AFTER 3"), Some(3));
    assert_eq!(parse_retry_after("Retry in: 4"), Some(4));
    assert_eq!(parse_retry_after("retry in 3, retry after 9"), Some(3));
    assert_eq!(parse_retry_after("retry after: = 5"), Some(5));
    assert_eq!(parse_retry_after("retry after\t6"), Some(6));
    // Not a marker: the separators are part of the shape Telegram writes.
    assert_eq!(parse_retry_after("retryafter 7"), None);
    assert_eq!(parse_retry_after("retries after 7"), None);
}

#[test]
fn non_ascii_text_before_the_marker_does_not_shift_or_hide_it() {
    assert_eq!(
        parse_retry_after("Übertragung fehlgeschlagen: retry after 4"),
        Some(4)
    );
    assert_eq!(parse_retry_after("⏳ retry in 2 s"), Some(2));
}

proptest! {
    /// Whatever the body says, a hint handed to the caller is a real wait.
    /// Issue #42 is exactly the exception this pins.
    #[test]
    fn a_hint_from_the_body_is_never_zero(
        explicit in any::<u64>(),
        description in "[0-9retry afte:Ri]{0,40}",
    ) {
        let error = rate_limited(&description, &json!(explicit));
        prop_assert_ne!(secs(&error), Some(0), "{} / {}", explicit, description);
    }

    /// The parser is total: any description parses or is rejected, never panics,
    /// and never invents a zero wait.
    #[test]
    fn arbitrary_descriptions_parse_or_are_rejected(description in ".{0,600}") {
        if let Some(seconds) = parse_retry_after(&description) {
            prop_assert!(seconds > 0, "{}", description);
            prop_assert_eq!(
                parse_retry_after(&format!("retry after {seconds}")),
                Some(seconds)
            );
        }
    }

    /// A hint inside the window is found whatever precedes it - the bounded scan
    /// must not lose traffic it used to read.
    #[test]
    fn a_marker_inside_the_window_is_found(
        prefix in "[a-z]{0,480}",
        seconds in 1_u64..=1_000_000,
    ) {
        let description = format!("{prefix}retry after {seconds}");
        prop_assert_eq!(parse_retry_after(&description), Some(seconds), "{}", description);
    }

    /// Bytes past the window cannot produce a hint on their own.
    #[test]
    fn text_beyond_the_window_yields_no_hint(garbage in "[x]{512,4096}") {
        prop_assert_eq!(parse_retry_after(&garbage), None);
    }
}
