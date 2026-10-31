//! Attaching a source keeps the cause, and a second attachment keeps the first
//! one instead of erasing it (issue #41).

use std::error::Error as StdError;
use std::time::Duration;

use proptest::prelude::*;
use telegram_api_client::Error;

/// An IO failure with a recognisable message.
fn io(message: &'static str) -> std::io::Error {
    std::io::Error::other(message)
}

/// A failure that carries a cause of its own.
#[derive(Debug)]
struct Caused {
    message: &'static str,
    cause: std::io::Error,
}

impl std::fmt::Display for Caused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl StdError for Caused {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(&self.cause)
    }
}

/// The facts a caller routes on, plus the rendered text.
fn snapshot(error: &Error) -> (bool, bool, Option<Duration>, String) {
    (
        error.is_retryable(),
        error.is_ambiguous(),
        error.retry_after(),
        error.to_string(),
    )
}

#[test]
fn not_sent_keeps_the_connect_failure() {
    let error = Error::not_sent("dns: name resolution failed").with_source(io("no such host"));

    let cause = StdError::source(&error).expect("cause kept");
    assert_eq!(cause.to_string(), "no such host");
    assert!(
        error.is_retryable(),
        "nothing was written, so a send may repeat"
    );
    assert_eq!(
        error.to_string(),
        "request not sent: dns: name resolution failed",
        "the transport message stays the face of the error"
    );
}

#[test]
fn unphased_transport_keeps_the_failure_across_classification() {
    let error = Error::Transport {
        message: "boom".into(),
        source: None,
    }
    .with_source(io("connection reset"));

    assert_eq!(
        StdError::source(&error).expect("cause kept").to_string(),
        "connection reset"
    );
    assert!(!error.is_retryable(), "the phase is still unknown");

    // Classifying for a method must not lose the cause on the way.
    let classified = error.classify_for("sendMessage");
    assert!(classified.is_ambiguous());
    assert!(
        StdError::source(&classified).is_some(),
        "cause survives classify_for"
    );
}

#[test]
fn ambiguous_keeps_the_failure() {
    let error = Error::ambiguous("sendMessage", "timed out").with_source(io("no such host"));

    assert!(error.is_ambiguous());
    assert_eq!(
        StdError::source(&error).expect("cause kept").to_string(),
        "no such host"
    );
}

#[test]
fn variants_without_a_source_slot_are_unchanged() {
    let api = || Error::Api {
        code: 400,
        description: "Bad Request: chat not found".into(),
    };
    let limited = || Error::RateLimited {
        retry_after: Some(3),
        code: 429,
        description: "Too Many Requests".into(),
    };
    let invalid = || Error::InvalidRequest {
        field: "chat_id",
        reason: "must not be empty".into(),
    };
    let unexpected = || Error::Unexpected {
        method: "getChat".into(),
        detail: "no result".into(),
    };

    for (before, after) in [
        (api(), api().with_source(io("no such host"))),
        (limited(), limited().with_source(io("no such host"))),
        (invalid(), invalid().with_source(io("no such host"))),
        (unexpected(), unexpected().with_source(io("no such host"))),
    ] {
        assert_eq!(
            snapshot(&after),
            snapshot(&before),
            "classification and text unchanged"
        );
        assert!(
            StdError::source(&after).is_none(),
            "a variant without a source slot keeps none"
        );
    }

    // The unit variants have no cause at all.
    for unit in [
        Error::EmptySecret,
        Error::SecretMismatch,
        Error::PathMismatch,
    ] {
        let attached = unit.with_source(io("no such host"));
        assert!(StdError::source(&attached).is_none());
    }
}

#[test]
fn a_second_attachment_keeps_the_first_cause() {
    let error = Error::not_sent("connect failed")
        .with_source(io("no such host"))
        .with_source(io("connection refused"));

    let newest = StdError::source(&error).expect("newest cause is visible");
    assert_eq!(newest.to_string(), "connection refused");
    let earlier = StdError::source(newest).expect("earlier cause kept");
    assert_eq!(earlier.to_string(), "no such host");
    assert!(error.to_string().contains("connect failed"));
}

#[test]
fn a_newest_cause_with_its_own_source_keeps_that_chain() {
    // Documented limit: `std::error::Error` chains are linear, so a cause that
    // brings its own cause replaces the walk instead of branching it.
    let error = Error::not_sent("connect failed")
        .with_source(io("earlier"))
        .with_source(Caused {
            message: "wrapper",
            cause: io("deep"),
        });

    let newest = StdError::source(&error).expect("newest cause");
    assert_eq!(newest.to_string(), "wrapper");
    let deeper = StdError::source(newest).expect("the wrapper brings its own cause");
    assert_eq!(deeper.to_string(), "deep");
    assert_ne!(deeper.to_string(), "earlier", "linear chain, one branch");
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 96, ..ProptestConfig::default() })]

    /// Attaching a source is diagnosis only: routing and text never change, for
    /// any message, method or status code.
    #[test]
    fn attaching_a_source_never_changes_classification(
        message in "[ -~]{0,64}",
        method in "[A-Za-z_]{0,32}",
        code in 100_i64..600,
    ) {
        let api = || Error::Api { code, description: message.clone() };
        prop_assert_eq!(snapshot(&api().with_source(io("boom"))), snapshot(&api()));

        let limited = || Error::RateLimited {
            retry_after: Some(3),
            code: 429,
            description: message.clone(),
        };
        prop_assert_eq!(snapshot(&limited().with_source(io("boom"))), snapshot(&limited()));

        let not_sent = || Error::not_sent(message.clone());
        prop_assert_eq!(snapshot(&not_sent().with_source(io("boom"))), snapshot(&not_sent()));

        let ambiguous = || Error::ambiguous(method.clone(), message.clone());
        prop_assert_eq!(
            snapshot(&ambiguous().with_source(io("boom"))),
            snapshot(&ambiguous())
        );

        let unphased = || Error::Transport {
            message: message.clone(),
            source: None,
        };
        prop_assert_eq!(snapshot(&unphased().with_source(io("boom"))), snapshot(&unphased()));
    }

    /// The cause is reachable for arbitrary text, and `classify_for` keeps it.
    #[test]
    fn the_cause_survives_classification(
        method in "[A-Za-z_]{0,32}",
        message in "[ -~]{0,64}",
    ) {
        let with = Error::Transport {
            message: message.clone(),
            source: None,
        }
        .with_source(io("reset"))
        .classify_for(&method);
        let without = Error::Transport {
            message,
            source: None,
        }
        .classify_for(&method);

        prop_assert_eq!(snapshot(&with), snapshot(&without));
        prop_assert!(StdError::source(&with).is_some());
    }
}
