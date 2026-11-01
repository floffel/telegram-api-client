//! Client payloads against a recording transport.

use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use telegram_api_client::{Error, Result, TelegramClient, Transport};

#[derive(Clone)]
struct Recorder {
    calls: Arc<Mutex<Vec<(String, Value)>>>,
    reply: Value,
}

impl Transport for Recorder {
    fn post_json(
        &self,
        url: &str,
        body: &Value,
    ) -> impl std::future::Future<Output = Result<Value>> {
        self.calls
            .lock()
            .unwrap()
            .push((url.to_string(), body.clone()));
        std::future::ready(Ok(self.reply.clone()))
    }
}

fn recorder(reply: Value) -> Recorder {
    Recorder {
        calls: Arc::default(),
        reply,
    }
}

#[tokio::test]
async fn send_message_posts_chat_id_and_text() {
    let rec = recorder(json!({"ok": true, "result": {"message_id": 1}}));
    let client = TelegramClient::new("123:ABC", rec.clone());

    client.send_text(-1001, "Hallo").await.unwrap();

    let calls = rec.calls.lock().unwrap().clone();
    let (url, body) = &calls[0];
    assert!(url.ends_with("/bot123:ABC/sendMessage"), "url was {url}");
    assert_eq!(body["chat_id"], -1001);
    assert_eq!(body["text"], "Hallo");
}

#[tokio::test]
async fn get_me_hits_its_own_method() {
    let rec = recorder(json!({"ok": true, "result": {"id": 1}}));
    let client = TelegramClient::new("123:ABC", rec.clone());
    client.me().await.unwrap();
    let calls = rec.calls.lock().unwrap().clone();
    assert!(calls[0].0.ends_with("/getMe"));
}

#[test]
fn base_url_is_configurable() {
    let rec = recorder(json!({}));
    let host = ["https:/", "/api.example.com"].join("");
    let client = TelegramClient::new("t", rec).with_base_url(host.clone());
    assert_eq!(client.method_url("getMe"), format!("{host}/bott/getMe"));
}

#[test]
fn debug_does_not_leak_the_token() {
    let client = TelegramClient::new("123:SECRET", recorder(json!({})));
    let rendered = format!("{client:?}");
    assert!(!rendered.contains("SECRET"), "token im Debug: {rendered}");
}

/// Regression for #31: the deprecated wrappers used to hand back a failed body
/// as a success, because only the typed methods ran the envelope check. The
/// `Recorder` returns exactly the case the bug describes — the transport yields
/// the envelope as `Ok` — so the decision has to happen on the client side.
#[tokio::test]
#[allow(deprecated)]
async fn legacy_send_message_surfaces_a_failed_envelope() {
    let rec = recorder(json!({
        "ok": false,
        "error_code": 400,
        "description": "Bad Request: chat not found"
    }));
    let client = TelegramClient::new("123:ABC", rec.clone());

    let error = client.send_message(-1001, "Hallo").await.unwrap_err();

    assert!(matches!(error, Error::Api { code: 400, .. }), "{error:?}");
    assert!(error.to_string().contains("chat not found"), "{error}");
    assert_eq!(
        rec.calls.lock().unwrap().len(),
        1,
        "a terminal Bot API error is not retried"
    );
}

#[tokio::test]
#[allow(deprecated)]
async fn legacy_get_me_surfaces_a_failed_envelope() {
    let rec = recorder(json!({
        "ok": false,
        "error_code": 401,
        "description": "Unauthorized"
    }));
    let client = TelegramClient::new("123:ABC", rec.clone());

    let error = client.get_me().await.unwrap_err();

    assert!(matches!(error, Error::Api { code: 401, .. }), "{error:?}");
    assert_eq!(rec.calls.lock().unwrap().len(), 1);
}

/// An answer with no `result` is a decoding failure, not an empty success — for
/// both wrappers, exactly as on the typed path.
#[tokio::test]
#[allow(deprecated)]
async fn legacy_wrappers_reject_an_envelope_without_result() {
    let rec = recorder(json!({ "ok": true }));
    let client = TelegramClient::new("123:ABC", rec.clone());

    // A read keeps the decoding error: nothing changed, repeating is free.
    let me = client.get_me().await.unwrap_err();
    assert!(matches!(me, Error::Unexpected { .. }), "{me:?}");
    assert!(me.to_string().contains("result"), "{me}");

    // A send may report the same answer as `Ambiguous` (the bot replied, so the
    // message may well exist) — either way it is not handed back as a success.
    let sent = client.send_message(-1001, "Hallo").await.unwrap_err();
    assert!(
        matches!(sent, Error::Unexpected { .. } | Error::Ambiguous { .. }),
        "{sent:?}"
    );
}
