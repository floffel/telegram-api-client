//! The `reqwest` transport, exercised against a local socket: what goes out on
//! the wire, and how answers are classified on the way back.
//!
//! No network beyond `127.0.0.1`, and the Bot API is never contacted.

#![cfg(feature = "http")]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{self, Receiver};

use telegram_api_client::{
    Error, MockTransport, NoDelay, ReqwestTransport, RetryPolicy, SendPhoto, TelegramClient,
    Transport, UploadFile, MAX_RESPONSE_BYTES,
};

/// A one-shot HTTP server: answers `status_line` with `body`, and reports the
/// request head it saw.
fn serve_once(status_line: &'static str, body: &'static str) -> (u16, Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a port");
    let port = listener.local_addr().expect("local addr").port();
    let (sender, receiver) = mpsc::channel();

    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buffer = vec![0_u8; 16 * 1024];
            let read = stream.read(&mut buffer).unwrap_or(0);
            let _ = sender.send(String::from_utf8_lossy(&buffer[..read]).into_owned());

            let response = format!(
                "HTTP/1.1 {status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });

    (port, receiver)
}

fn client(port: u16) -> TelegramClient<ReqwestTransport> {
    TelegramClient::new("123:ABC", ReqwestTransport::new())
        .with_base_url(format!("http://127.0.0.1:{port}"))
}

#[tokio::test]
async fn a_json_call_round_trips_over_a_real_socket() {
    let (port, request) = serve_once("200 OK", r#"{"ok":true,"result":{"id":7,"is_bot":true}}"#);

    let me = client(port).me().await.expect("bot identity");
    assert_eq!(me.id, 7);

    let head = request
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("request head");
    assert!(head.starts_with("POST /bot123:ABC/getMe "), "{head}");
    assert!(head
        .to_lowercase()
        .contains("content-type: application/json"));
}

#[tokio::test]
async fn an_upload_goes_out_as_multipart_with_the_file_name() {
    let (port, request) = serve_once("200 OK", r#"{"ok":true,"result":{"message_id":1}}"#);

    let photo = SendPhoto::new(-1001_i64, "x")
        .with_upload(UploadFile::new("Grüße.png", vec![0x89, 0x50, 0x4e, 0x47]));
    client(port).send_photo(&photo).await.expect("upload");

    let head = request
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("request head");
    assert!(head.starts_with("POST /bot123:ABC/sendPhoto "), "{head}");
    assert!(
        head.to_lowercase()
            .contains("content-type: multipart/form-data; boundary=telegram-api-client-"),
        "{head}"
    );
    assert!(head.contains("filename=\"Grüße.png\""), "{head}");
}

#[tokio::test]
async fn a_429_answer_becomes_a_typed_rate_limit() {
    let (port, _request) = serve_once(
        "429 Too Many Requests",
        r#"{"ok":false,"error_code":429,"description":"Too Many Requests","parameters":{"retry_after":9}}"#,
    );

    let error = client(port)
        .without_retry()
        .me()
        .await
        .expect_err("rate limited");
    assert!(matches!(error, Error::RateLimited { code: 429, .. }));
    assert_eq!(error.retry_after(), Some(std::time::Duration::from_secs(9)));
}

#[tokio::test]
async fn a_5xx_on_a_send_is_ambiguous_even_over_the_wire() {
    let (port, _request) = serve_once("502 Bad Gateway", "<html>bad gateway</html>");

    let error = client(port)
        .send_text(-1001_i64, "hi")
        .await
        .expect_err("5xx must fail");
    assert!(error.is_ambiguous(), "{error:?}");
    assert!(error.to_string().contains("sendMessage"), "{error}");
}

#[tokio::test]
async fn a_non_json_answer_is_a_payload_error() {
    let (port, _request) = serve_once("200 OK", "this is not json");

    let error = client(port)
        .without_retry()
        .me()
        .await
        .expect_err("garbage");
    assert!(matches!(error, Error::Payload { .. }), "{error:?}");
}

#[tokio::test]
async fn a_connection_closed_after_the_write_is_ambiguous() {
    // The server reads the request and hangs up without answering: the request
    // was written, so the client cannot know whether it was processed.
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buffer = [0_u8; 4096];
            let _ = stream.read(&mut buffer);
            drop(stream);
        }
    });

    let error = client(port)
        .without_retry()
        .send_text(-1001_i64, "hi")
        .await
        .expect_err("the server hung up");
    assert!(error.is_ambiguous(), "{error:?}");
    assert!(!error.is_retryable());
}

#[tokio::test]
async fn a_truncated_response_body_is_ambiguous() {
    // Headers promise 999 bytes, the socket delivers 3: reading the body fails
    // after the request was accepted.
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buffer = [0_u8; 4096];
            let _ = stream.read(&mut buffer);
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 999\r\nConnection: close\r\n\r\n{\"o\"",
            );
            let _ = stream.flush();
        }
    });

    let error = client(port)
        .without_retry()
        .send_text(-1001_i64, "hi")
        .await
        .expect_err("truncated body");
    assert!(error.is_ambiguous(), "{error:?}");
}

#[tokio::test]
async fn a_refused_connection_is_a_pre_write_error() {
    // Bind and immediately drop, so the port is closed for the request.
    let port = {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        listener.local_addr().expect("addr").port()
    };

    let error = client(port)
        .without_retry()
        .me()
        .await
        .expect_err("nothing listens there");
    // `NotSent`, not `Transport`: the transport proved the body never left the
    // machine, which is what makes retrying a send safe.
    assert!(matches!(error, Error::NotSent { .. }), "{error:?}");
    assert!(error.is_retryable(), "a connect failure is safe to retry");
    assert!(!error.is_ambiguous());
}

#[tokio::test]
async fn a_refused_connection_is_retried_within_the_budget() {
    let port = {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        listener.local_addr().expect("addr").port()
    };

    let client = client(port)
        .with_sleeper(NoDelay)
        .with_retry_policy(RetryPolicy::new(
            2,
            std::time::Duration::from_millis(1),
            std::time::Duration::from_millis(2),
        ));

    let error = client.me().await.expect_err("nothing listens there");
    assert!(matches!(error, Error::NotSent { .. }), "{error:?}");
    // Retried up to the budget: one call per attempt, and the error is the
    // last one.
    assert!(error.is_retryable());

    // A JSON-only mock never sees a socket: it proves the client stays generic.
    let mock = TelegramClient::new("1:A", MockTransport::new());
    assert!(mock.send_text(-1_i64, "hi").await.is_ok());
}

// ---- the response limit (issue #32) ---------------------------------------

/// A one-shot HTTP server that writes `head` (status line plus headers) and
/// `body` verbatim, so a test can send a length header that lies, or none at
/// all.
fn serve_raw(status_line: &'static str, head: String, body: Vec<u8>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a port");
    let port = listener.local_addr().expect("local addr").port();

    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            // Drain the request head; the body is empty for a JSON `{}` call.
            let mut buffer = vec![0_u8; 16 * 1024];
            let _ = stream.read(&mut buffer);

            let mut response = format!("HTTP/1.1 {status_line}\r\n").into_bytes();
            response.extend_from_slice(head.as_bytes());
            response.extend_from_slice(b"\r\n");
            response.extend_from_slice(&body);
            let _ = stream.write_all(&response);
            let _ = stream.flush();
        }
    });

    port
}

/// A successful envelope of exactly `len` bytes, so a test can sit on the
/// limit itself rather than near it.
fn envelope_of_len(len: usize) -> Vec<u8> {
    const OPEN: &str = r#"{"ok":true,"result":""#;
    const CLOSE: &str = r#""}"#;
    let padding = len
        .checked_sub(OPEN.len() + CLOSE.len())
        .expect("the body needs room for the envelope itself");
    let mut body = String::from(OPEN);
    body.push_str(&"a".repeat(padding));
    body.push_str(CLOSE);
    assert_eq!(body.len(), len, "the fixture is exactly the asked-for size");
    body.into_bytes()
}

/// Posts an empty JSON body through a transport with a `cap`-byte limit.
async fn post(port: u16, cap: usize) -> telegram_api_client::Result<serde_json::Value> {
    let transport = ReqwestTransport::new().with_max_response_bytes(cap);
    transport
        .post_json(
            &format!("http://127.0.0.1:{port}/bot1:A/getMe"),
            &serde_json::json!({}),
        )
        .await
}

#[test]
fn the_default_limit_is_the_documented_one() {
    assert_eq!(
        ReqwestTransport::new().max_response_bytes(),
        MAX_RESPONSE_BYTES
    );
    assert_eq!(MAX_RESPONSE_BYTES, 16 * 1024 * 1024);
}

#[tokio::test]
async fn a_declared_body_exactly_at_the_limit_is_read() {
    let body = envelope_of_len(256);
    let port = serve_raw(
        "200 OK",
        format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
            body.len()
        ),
        body,
    );

    let value = post(port, 256).await.expect("exactly at the limit is fine");
    assert_eq!(value["ok"], true);
    assert_eq!(
        value["result"].as_str().map(str::len),
        Some(256 - r#"{"ok":true,"result":""}"#.len()),
        "the whole body arrived, not just its prefix"
    );
}

#[tokio::test]
async fn a_declared_body_one_byte_over_the_limit_is_refused() {
    let body = envelope_of_len(257);
    let port = serve_raw(
        "200 OK",
        format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
            body.len()
        ),
        body,
    );

    let error = post(port, 256).await.expect_err("one byte over the limit");
    match &error {
        Error::ResponseTooLarge { method, limit } => {
            assert_eq!(method, "getMe", "the error names the method");
            assert_eq!(*limit, 256, "and the limit that applied");
        }
        other => panic!("expected ResponseTooLarge, got {other:?}"),
    }
    assert!(
        !error.is_retryable(),
        "the same body would be too large again"
    );
    assert!(error.is_ambiguous(), "the answer arrived and was dropped");
}

#[tokio::test]
async fn a_streamed_body_exactly_at_the_limit_is_read() {
    // No `Content-Length` at all: the header gate cannot help, so the counting
    // gate is the one under test.
    let body = envelope_of_len(256);
    let port = serve_raw(
        "200 OK",
        "Content-Type: application/json\r\nConnection: close\r\n".to_string(),
        body,
    );

    let value = post(port, 256).await.expect("exactly at the limit is fine");
    assert_eq!(
        value["result"].as_str().map(str::len),
        Some(256 - r#"{"ok":true,"result":""}"#.len()),
        "the whole body arrived, not just its prefix"
    );
}

#[tokio::test]
async fn a_streamed_body_one_byte_over_the_limit_is_refused_while_reading() {
    let body = envelope_of_len(257);
    let port = serve_raw(
        "200 OK",
        "Content-Type: application/json\r\nConnection: close\r\n".to_string(),
        body,
    );

    let error = post(port, 256)
        .await
        .expect_err("one byte over the limit, counted not declared");
    assert!(
        matches!(error, Error::ResponseTooLarge { limit: 256, .. }),
        "{error:?}"
    );
}

#[tokio::test]
async fn a_hostile_content_length_is_refused_before_the_body_is_read() {
    // The head promises 100 MB and the socket will never deliver it: the
    // refusal has to come from the header, or the client would sit there
    // allocating until the server gave up.
    let port = serve_raw(
        "200 OK",
        "Content-Type: application/json\r\nContent-Length: 100000000\r\nConnection: close\r\n"
            .to_string(),
        b"{\"ok\":true,".to_vec(),
    );

    let error = post(port, 1024)
        .await
        .expect_err("the declared length alone exceeds the limit");
    assert!(
        matches!(error, Error::ResponseTooLarge { limit: 1024, .. }),
        "the header is judged before a byte of body is read: {error:?}"
    );
}

#[tokio::test]
async fn an_oversized_answer_is_not_retried_under_a_retry_budget() {
    // The server answers once and closes. A retry would find a closed port and
    // end as `NotSent`, so seeing `ResponseTooLarge` proves no retry happened.
    let body = envelope_of_len(512);
    let port = serve_raw(
        "200 OK",
        format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
            body.len()
        ),
        body,
    );

    let client = TelegramClient::new("1:A", ReqwestTransport::new().with_max_response_bytes(64))
        .with_base_url(format!("http://127.0.0.1:{port}"))
        .with_sleeper(NoDelay)
        .with_retry_policy(RetryPolicy::new(
            3,
            std::time::Duration::from_millis(1),
            std::time::Duration::from_millis(2),
        ));

    let error = client
        .send_text(-1001_i64, "hi")
        .await
        .expect_err("the answer was dropped");
    assert!(
        matches!(error, Error::ResponseTooLarge { .. }),
        "a size refusal is terminal for both reads and sends: {error:?}"
    );
    assert!(error.is_ambiguous(), "the send was most likely applied");
}
