//! Client payloads against a recording transport.

use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use telegram_api_client::{Result, TelegramClient, Transport};

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

    client.send_message(-1001, "Hallo").await.unwrap();

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
    client.get_me().await.unwrap();
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
