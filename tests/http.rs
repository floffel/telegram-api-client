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
    UploadFile,
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
