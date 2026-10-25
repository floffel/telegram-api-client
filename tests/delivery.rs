//! Delivery semantics: a failed call must never silently duplicate a message.
//!
//! Two facts are pinned here, and they pull in opposite directions:
//!
//! * 429 and pre-write connection failures are *safe* to repeat, so the client
//!   repeats them (honouring `retry_after`).
//! * A 5xx, or a transport failure after the request was written, means the
//!   message may already be delivered. It surfaces as [`Error::Ambiguous`] and
//!   is never retried, whatever the attempt budget says.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use telegram_api_client::{
    Error, GetChat, MockReply, MockTransport, RetryPolicy, SendMessage, SendPhoto, Sleeper,
    TelegramClient, UploadFile,
};

/// Records every delay it was asked to wait, and never waits.
#[derive(Clone, Default)]
struct RecordingSleeper(Arc<Mutex<Vec<Duration>>>);

impl Sleeper for RecordingSleeper {
    fn sleep(&self, delay: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        self.0.lock().expect("sleeper lock").push(delay);
        Box::pin(std::future::ready(()))
    }
}

impl RecordingSleeper {
    fn delays(&self) -> Vec<Duration> {
        self.0.lock().expect("sleeper lock").clone()
    }
}

fn message() -> Value {
    json!({
        "message_id": 7,
        "chat": { "id": -1001, "type": "supergroup" },
        "date": 1_700_000_000,
        "text": "hi",
    })
}

fn chat() -> Value {
    json!({ "id": -1001, "type": "supergroup", "title": "Test" })
}

/// A client whose transport replays `replies` in order, with a recording
/// sleeper so nothing actually waits.
fn client_with(
    replies: impl IntoIterator<Item = MockReply>,
) -> (
    TelegramClient<MockTransport>,
    MockTransport,
    RecordingSleeper,
) {
    let transport = MockTransport::new();
    transport.extend(replies);
    let sleeper = RecordingSleeper::default();
    let client = TelegramClient::new("123:ABC", transport.clone())
        .with_base_url("https://tg.test")
        .with_sleeper(sleeper.clone());
    (client, transport, sleeper)
}

#[tokio::test]
async fn a_5xx_on_a_send_is_ambiguous_and_costs_exactly_one_call() {
    let (client, transport, sleeper) = client_with([MockReply::api(502, "Bad Gateway")]);

    let error = client
        .send_message_request(&SendMessage::new(-1001, "hi"))
        .await
        .expect_err("5xx must fail");

    assert!(error.is_ambiguous(), "{error:?}");
    assert!(!error.is_retryable(), "{error:?}");
    match &error {
        Error::Ambiguous { method, .. } => assert_eq!(method, "sendMessage"),
        other => panic!("expected Ambiguous, got {other:?}"),
    }
    assert_eq!(transport.calls().len(), 1, "no automatic retry");
    assert!(
        sleeper.delays().is_empty(),
        "nothing to wait for: there is no retry"
    );
}

#[tokio::test]
async fn a_429_is_retried_once_and_honours_retry_after() {
    let (client, transport, sleeper) =
        client_with([MockReply::rate_limited(3), MockReply::result(message())]);

    let sent = client
        .send_message_request(&SendMessage::new(-1001, "hi"))
        .await
        .expect("second attempt succeeds");

    assert_eq!(sent.message_id, 7);
    assert_eq!(transport.calls().len(), 2, "one retry, not a loop");
    assert_eq!(sleeper.delays(), vec![Duration::from_secs(3)]);
}

#[tokio::test]
async fn a_429_loop_stops_at_the_attempt_budget() {
    let (client, transport, _sleeper) = client_with([
        MockReply::rate_limited(1),
        MockReply::rate_limited(1),
        MockReply::rate_limited(1),
        MockReply::rate_limited(1),
    ]);

    let error = client
        .send_message_request(&SendMessage::new(-1001, "hi"))
        .await
        .expect_err("all attempts are limited");

    assert!(matches!(error, Error::RateLimited { .. }), "{error:?}");
    assert_eq!(transport.calls().len(), 3, "default budget is 3 attempts");
}

#[tokio::test]
async fn a_pre_write_connection_error_is_retried() {
    let (client, transport, sleeper) = client_with([
        MockReply::not_sent("dns error: name resolution failed"),
        MockReply::result(message()),
    ]);

    let sent = client
        .send_message_request(&SendMessage::new(-1001, "hi"))
        .await
        .expect("retry succeeds");

    assert_eq!(sent.message_id, 7);
    assert_eq!(transport.calls().len(), 2);
    assert_eq!(
        sleeper.delays(),
        vec![Duration::from_millis(500)],
        "the first retry waits exactly base_delay"
    );
}

#[tokio::test]
async fn an_unphased_transport_failure_is_never_retried_on_a_send() {
    // A third-party transport that cannot say whether the request was written
    // is the case the audit found: `Transport` used to be retryable for every
    // method, so a send could be repeated after a reset that had already
    // delivered it. It is now ambiguous for a send and costs one call.
    let (client, transport, sleeper) =
        client_with([MockReply::unphased("connection reset by peer")]);

    let error = client
        .send_message_request(&SendMessage::new(-1001, "hi"))
        .await
        .expect_err("unknown outcome must not be retried");

    match &error {
        Error::Ambiguous { method, detail, .. } => {
            assert_eq!(method, "sendMessage");
            assert!(detail.contains("connection reset"), "{detail}");
        }
        other => panic!("expected Ambiguous, got {other:?}"),
    }
    assert!(!error.is_retryable());
    assert_eq!(transport.calls().len(), 1, "exactly one delivery attempt");
    assert!(sleeper.delays().is_empty(), "nothing to wait for");
}

#[tokio::test]
async fn an_unphased_transport_failure_is_retried_on_a_read() {
    // Reads are idempotent by definition, so the same unphased failure stays
    // safely repeatable.
    let (client, transport, _sleeper) = client_with([
        MockReply::unphased("connection reset by peer"),
        MockReply::result(chat()),
    ]);

    let fetched = client
        .get_chat(&GetChat::new(-1001))
        .await
        .expect("a read may be repeated");

    assert_eq!(fetched.label(), "Test");
    assert_eq!(transport.calls().len(), 2);
}

#[tokio::test]
async fn a_retry_after_longer_than_max_delay_is_handed_back_instead_of_shortened() {
    // The default ceiling is 30 s. Telegram asking for 60 s used to be clamped
    // to 30 s, retried too early, and answered with a second 429 — burning the
    // attempt budget. The hint is now authoritative, and a hint the policy
    // cannot honour ends the call with the RateLimited error.
    let (client, transport, sleeper) =
        client_with([MockReply::rate_limited(60), MockReply::result(message())]);

    let error = client
        .send_message_request(&SendMessage::new(-1001, "hi"))
        .await
        .expect_err("60 s is above the 30 s ceiling");

    match &error {
        Error::RateLimited { retry_after, .. } => assert_eq!(*retry_after, Some(60)),
        other => panic!("expected RateLimited, got {other:?}"),
    }
    assert_eq!(transport.calls().len(), 1, "no early retry");
    assert!(
        sleeper.delays().is_empty(),
        "the client did not wait at all"
    );

    // Boundary pair: exactly max_delay is still retried, one second more is not.
    let policy = RetryPolicy::default();
    let at_limit = Error::RateLimited {
        retry_after: Some(30),
        code: 429,
        description: String::new(),
    };
    let past_limit = Error::RateLimited {
        retry_after: Some(31),
        code: 429,
        description: String::new(),
    };
    assert!(policy.should_retry(1, &at_limit));
    assert!(!policy.should_retry(1, &past_limit));
    assert_eq!(
        policy.delay_for_error(1, &at_limit),
        Duration::from_secs(30),
        "a hint at the ceiling is used verbatim"
    );
    assert_eq!(
        policy.delay_for_error(1, &past_limit),
        Duration::from_secs(31),
        "never clamped down to max_delay"
    );
}

#[tokio::test]
async fn the_backoff_starts_at_base_delay_and_doubles() {
    // `post` passes the attempt that just failed, so the first retry must wait
    // base_delay (500 ms), not 2 * base_delay as it did before the fix.
    let (client, transport, sleeper) = client_with([
        MockReply::not_sent("connect refused"),
        MockReply::not_sent("connect refused"),
        MockReply::result(message()),
    ]);

    client
        .send_message_request(&SendMessage::new(-1001, "hi"))
        .await
        .expect("third attempt succeeds");

    assert_eq!(transport.calls().len(), 3);
    assert_eq!(
        sleeper.delays(),
        vec![Duration::from_millis(500), Duration::from_secs(1)],
        "500 ms then 1 s"
    );

    // The curve itself, including the cap.
    let policy = RetryPolicy::new(10, Duration::from_millis(500), Duration::from_secs(30));
    assert_eq!(policy.delay_for(0), Duration::from_millis(500));
    assert_eq!(policy.delay_for(1), Duration::from_secs(1));
    assert_eq!(policy.delay_for(2), Duration::from_secs(2));
    assert_eq!(policy.delay_for(6), Duration::from_secs(30), "capped");
    assert_eq!(policy.delay_for(u32::MAX), Duration::from_secs(30));
}

#[tokio::test]
async fn a_failure_after_the_write_is_never_retried() {
    let (client, transport, _sleeper) =
        client_with([MockReply::ambiguous("connection reset after the body")]);

    let error = client
        .send_message_request(&SendMessage::new(-1001, "hi"))
        .await
        .expect_err("unknown delivery must fail");

    match &error {
        Error::Ambiguous { method, detail, .. } => {
            assert_eq!(method, "sendMessage", "the client fills in the method");
            assert!(detail.contains("reset"));
        }
        other => panic!("expected Ambiguous, got {other:?}"),
    }
    assert_eq!(transport.calls().len(), 1);
}

#[tokio::test]
async fn a_5xx_on_a_read_is_retried() {
    let (client, transport, _sleeper) = client_with([
        MockReply::api(503, "Service Unavailable"),
        MockReply::result(chat()),
    ]);

    let fetched = client
        .get_chat(&GetChat::new(-1001))
        .await
        .expect("reads are idempotent");

    assert_eq!(fetched.label(), "Test");
    assert_eq!(transport.calls().len(), 2);
}

#[tokio::test]
async fn a_5xx_on_a_multipart_upload_is_ambiguous_too() {
    let (client, transport, _sleeper) = client_with([MockReply::api(500, "Internal Server Error")]);
    let upload = UploadFile::new("a.png", vec![0x89, 0x50]).with_mime("image/png");

    let error = client
        .send_photo(&SendPhoto::new(-1001, "x").with_upload(upload))
        .await
        .expect_err("upload 5xx must fail");

    assert!(error.is_ambiguous(), "{error:?}");
    assert_eq!(transport.calls().len(), 1);
    assert!(transport.last_call().expect("one call").is_multipart());
}

#[tokio::test]
async fn retries_can_be_switched_off() {
    let transport = MockTransport::new();
    transport.extend([MockReply::rate_limited(2), MockReply::result(message())]);
    let client = TelegramClient::new("123:ABC", transport.clone()).without_retry();

    let error = client
        .send_message_request(&SendMessage::new(-1001, "hi"))
        .await
        .expect_err("retries are off");

    assert!(matches!(error, Error::RateLimited { .. }));
    assert_eq!(transport.calls().len(), 1);
}

#[tokio::test]
async fn the_default_client_retries_a_429_with_its_own_sleeper() {
    let transport = MockTransport::new();
    transport.extend([MockReply::rate_limited(0), MockReply::result(message())]);
    let client = TelegramClient::new("123:ABC", transport.clone());

    client
        .send_message_request(&SendMessage::new(-1001, "hi"))
        .await
        .expect("default policy retries once");

    assert_eq!(transport.calls().len(), 2);
    assert_eq!(client.retry_policy().max_attempts, 3);
}

#[tokio::test]
async fn a_terminal_error_is_returned_after_one_call() {
    let (client, transport, _sleeper) =
        client_with([MockReply::api(400, "Bad Request: chat not found")]);

    let error = client
        .send_message_request(&SendMessage::new(-1001, "hi"))
        .await
        .expect_err("400 is terminal");

    assert!(!error.is_retryable() && !error.is_ambiguous());
    assert_eq!(transport.calls().len(), 1);
}

#[test]
fn the_classification_matrix_is_explicit() {
    // (error, method, retryable, ambiguous)
    let gateway = || Error::Api {
        code: 502,
        description: "Bad Gateway".into(),
    };
    let limited = || Error::RateLimited {
        retry_after: Some(2),
        code: 429,
        description: "Too Many Requests".into(),
    };
    let unphased = || Error::Transport {
        message: "connection closed".into(),
        source: None,
    };
    let not_sent = || Error::NotSent {
        message: "connect refused".into(),
        source: None,
    };

    let cases = [
        (gateway().classify_for("sendMessage"), false, true),
        (gateway().classify_for("getChat"), true, false),
        (limited().classify_for("sendMessage"), true, false),
        // An unphased transport failure: ambiguous on a send, safely
        // repeatable on a read. It was retryable for every method before.
        (unphased().classify_for("sendMessage"), false, true),
        (unphased().classify_for("getChat"), true, false),
        (unphased(), false, false),
        // A proven pre-write failure is retryable even for a send.
        (not_sent().classify_for("sendMessage"), true, false),
        (
            Error::ambiguous("sendMessage", "timeout").classify_for("sendMessage"),
            false,
            true,
        ),
        (
            Error::Api {
                code: 409,
                description: "Conflict".into(),
            },
            false,
            false,
        ),
    ];

    for (error, retryable, ambiguous) in cases {
        assert_eq!(error.is_retryable(), retryable, "{error:?}");
        assert_eq!(error.is_ambiguous(), ambiguous, "{error:?}");
    }
}

#[test]
fn a_custom_budget_is_used() {
    let client = TelegramClient::new("123:ABC", MockTransport::new()).with_retry_policy(
        RetryPolicy::new(7, Duration::from_millis(1), Duration::from_secs(1)),
    );
    assert_eq!(client.retry_policy().max_attempts, 7);
}
