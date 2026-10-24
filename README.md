# telegram-api-client

Minimal, typed client and webhook toolkit for the Telegram Bot API.

* **Client** — every method below, through a pluggable `Transport` (no network
  needed in tests; `MockTransport` ships in the crate).
* **Typed requests** — one struct per Bot API method, validated before the call.
* **Typed errors** — `#[non_exhaustive]`, with source chains and an explicit
  retryable/terminal split (`Error::is_retryable`, `Error::retry_after`,
  `RetryPolicy`).
* **Webhook** — constant-time check of `X-Telegram-Bot-Api-Secret-Token` and of
  the webhook path derived from the secret (`sha256` hex).
* **Updates** — typed `Update`/`Message`/`CallbackQuery`/`InlineQuery` plus
  `UpdateCursor` for exactly-once polling.

General-purpose: routing, persistence and sleeping between retries stay in the
caller.

## Quick start

```rust,no_run
use telegram_api_client::{ReqwestTransport, SendMessage, SetWebhook, TelegramClient};

# async fn demo() -> telegram_api_client::Result<()> {
let client = TelegramClient::new("123:ABC", ReqwestTransport::new());

client.send_message_request(&SendMessage::new(-1001, "ping")).await?;
client.set_webhook(&SetWebhook::new("https://example.com/hook").with_secret("s3cret")).await?;
# Ok(()) }
```

Testing against the same code path, without a socket:

```rust
use telegram_api_client::{MockReply, MockTransport, SendPhoto, TelegramClient};

# async fn demo() -> telegram_api_client::Result<()> {
let transport = MockTransport::new();
transport.push(MockReply::result(serde_json::json!({"message_id": 7})));
let client = TelegramClient::new("123:ABC", transport.clone());

client.send_photo(&SendPhoto::new(1, "file-id")).await?;
assert_eq!(transport.last_call().unwrap().method(), "sendPhoto");
# Ok(()) }
```

## Implemented surface

| Bot API | client method | request type |
| --- | --- | --- |
| `sendMessage` | `send_message_request`, `send_message`¹ | `SendMessage` |
| `sendPhoto` | `send_photo` | `SendPhoto` |
| `sendDocument` | `send_document` | `SendDocument` |
| `sendAudio` | `send_audio` | `SendAudio` |
| `sendVideo` | `send_video` | `SendVideo` |
| `sendAnimation` | `send_animation` | `SendAnimation` |
| `sendVoice` | `send_voice` | `SendVoice` |
| `sendMediaGroup` | `send_media_group` | `SendMediaGroup` |
| `sendLocation` | `send_location` | `SendLocation` |
| `sendVenue` | `send_venue` | `SendVenue` |
| `sendContact` | `send_contact` | `SendContact` |
| `sendPoll` | `send_poll` | `SendPoll` |
| `sendDice` | `send_dice` | `SendDice` |
| `editMessageText` | `edit_message_text` | `EditMessageText` |
| `editMessageCaption` | `edit_message_caption` | `EditMessageCaption` |
| `editMessageReplyMarkup` | `edit_message_reply_markup` | `EditMessageReplyMarkup` |
| `deleteMessage` | `delete_message` | `DeleteMessage` |
| `forwardMessage` | `forward_message` | `ForwardMessage` |
| `copyMessage` | `copy_message` | `CopyMessage` |
| `answerCallbackQuery` | `answer_callback_query` | `AnswerCallbackQuery` |
| `answerInlineQuery` | `answer_inline_query` | `AnswerInlineQuery` |
| `setChatMenuButton` | `set_chat_menu_button` | `SetChatMenuButton` |
| `sendChatAction` | `send_chat_action` | `SendChatAction` |
| `getChat` | `get_chat` | `GetChat` |
| `getChatMember` | `get_chat_member` | `GetChatMember` |
| `banChatMember` | `ban_chat_member` | `BanChatMember` |
| `pinChatMessage` | `pin_chat_message` | `PinChatMessage` |
| `unpinChatMessage` | `unpin_chat_message` | `UnpinChatMessage` |
| `getMe` | `me`, `get_me`¹ | — |
| `setWebhook` | `set_webhook` | `SetWebhook` |
| `deleteWebhook` | `delete_webhook` | `DeleteWebhook` |
| `getWebhookInfo` | `get_webhook_info` | — |
| `getUpdates` | `get_updates` | `GetUpdates` + `UpdateCursor` |
| `getFile` | `get_file`, `file_url` | `GetFile` |

¹ raw `serde_json::Value` convenience wrappers kept for the 0.1 API; the typed
methods are the recommended ones.

Reply/inline keyboards are typed (`ReplyMarkup`, `InlineKeyboardMarkup`,
`KeyboardButton`, …), as are `MessageEntity`, `LinkPreviewOptions`, `ChatId`
and the `MenuButton` variants.

## Behaviour contract

- **Permissive decoding.** Unknown JSON fields are ignored, unknown enum values
  are carried (`ParseMode::Unknown`, `ChatType::Unknown`, `ReplyMarkup::Unknown`,
  …) and re-serialized unchanged, missing optionals default, and both
  `snake_case` and `camelCase` spellings are accepted.
- **Validation before the wire.** Text length is counted in UTF-16 code units
  like Telegram does (4096 for text, 1024 for captions), coordinates must lie in
  `-90..=90` / `-180..=180`, album sizes in `2..=10`, poll options in `2..=12`.
  Violations are `Error::InvalidRequest { field, .. }` — no network call.
- **429 is first class.** `Error::RateLimited { retry_after, .. }` keeps
  Telegram's `parameters.retry_after`; if only the description carries the hint
  it is parsed out. `RetryPolicy::delay_for_error` prefers that hint over its own
  exponential curve.
- **Secrets.** The bot token never appears in `Debug` (`TelegramClient`,
  `SetWebhook`), comparisons are constant time (`subtle`), and an empty
  configured secret is a hard error rather than a wildcard.
- **Concurrency.** `TelegramClient<T>` is `Send + Sync` when `T` is, and is
  cheap to clone; the test suite hammers one client from 64 tasks.

## Deliberate omissions

- **Multipart upload.** `Transport` is a JSON-only boundary, so media is
  referenced by `file_id` or HTTP URL. Uploading raw bytes needs a different
  transport contract and is out of scope.
- **Automatic retry loop.** `RetryPolicy` answers "retry?" and "how long?" but
  never sleeps, so the crate stays runtime-agnostic and a shutdown signal can
  win at any await point. The caller owns the loop.
- **Long-polling driver.** `get_updates` + `UpdateCursor` give exactly-once
  semantics; the loop around them belongs to the caller.
- **Out of scope surfaces.** Payments/invoices, games, Passport, sticker-set
  management, `editMessageMedia` and forum-topic management (`createForumTopic`
  and friends) are not modelled.
- **Coverage/fuzz gates.** The `cargo-llvm-cov` job with a threshold and the
  named fuzz targets are not wired yet (tracked in issue #3); the tests prove
  behaviour, the measurement jobs are still open.

## Licence

Apache-2.0 OR MIT.
