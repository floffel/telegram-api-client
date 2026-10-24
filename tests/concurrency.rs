//! Concurrency: the client must be `Send + Sync` and survive many tasks at once.

use std::collections::HashSet;

use serde_json::json;
use telegram_api_client::{MockTransport, SendMessage, TelegramClient};

#[test]
fn public_types_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<MockTransport>();
    assert_send_sync::<TelegramClient<MockTransport>>();
    assert_send_sync::<telegram_api_client::WebhookGuard>();
    assert_send_sync::<telegram_api_client::Error>();
    assert_send_sync::<telegram_api_client::Update>();
    assert_send_sync::<SendMessage>();
}

#[test]
fn errors_are_send_sync_error_sources() {
    fn assert_error<T: std::error::Error + Send + Sync + 'static>() {}
    assert_error::<telegram_api_client::Error>();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn one_client_serves_many_tasks() {
    const TASKS: usize = 64;

    let transport = MockTransport::new();
    let client = TelegramClient::new("123:ABC", transport.clone()).with_base_url("https://tg.test");

    let mut handles = Vec::new();
    for index in 1..=TASKS {
        let client = client.clone();
        let chat = i64::try_from(index).expect("chat id fits");
        handles.push(tokio::spawn(async move {
            client
                .send_message_request(&SendMessage::new(chat, format!("msg {chat}")))
                .await
                .map(|message| message.message_id)
        }));
    }

    for handle in handles {
        // The default mock reply is an empty object, which decodes to a default
        // message — what matters here is that no task panicked or deadlocked.
        assert_eq!(handle.await.unwrap().unwrap(), 0);
    }

    let calls = transport.calls();
    assert_eq!(calls.len(), TASKS, "every task must be recorded");

    // Each call is intact and belongs to exactly one task.
    let mut chats = HashSet::new();
    for call in &calls {
        assert_eq!(call.method(), "sendMessage");
        let chat = call.body["chat_id"]
            .as_i64()
            .expect("chat_id must be a number");
        assert!(chats.insert(chat), "chat {chat} was sent twice");
        assert_eq!(call.body["text"], json!(format!("msg {chat}")));
    }
    assert_eq!(chats.len(), TASKS);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shared_transport_is_not_corrupted_by_parallel_writes() {
    let transport = MockTransport::new();
    let client = TelegramClient::new("123:ABC", transport.clone());

    let mut handles = Vec::new();
    for index in 1..=16 {
        let client = client.clone();
        handles.push(tokio::spawn(async move {
            client.send_message(index, &format!("body-{index}")).await
        }));
    }
    for handle in handles {
        handle.await.unwrap().unwrap();
    }

    let calls = transport.calls();
    assert_eq!(calls.len(), 16);
    // Bodies are never interleaved: every recorded body has its own text.
    for call in calls {
        let text = call.body["text"].as_str().unwrap_or_default().to_string();
        assert_eq!(text, format!("body-{}", call.body["chat_id"]));
    }
}
