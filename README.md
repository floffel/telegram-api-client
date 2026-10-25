# telegram-api-client

Minimal, typed client and webhook toolkit for the Telegram Bot API.

* **Client** — every method below, through a pluggable `Transport` (no network
  needed in tests; `MockTransport` ships in the crate).
* **Typed requests** — one struct per Bot API method, validated before the call.
* **Typed errors** — `#[non_exhaustive]`, with source chains and an explicit
  retryable / ambiguous / terminal split (`Error::is_retryable`,
  `Error::is_ambiguous`, `Error::retry_after`, `RetryPolicy`).
* **Delivery-aware retries** — the client only repeats what is provably safe
  (429 and pre-write connection failures). A 5xx on a send is reported as
  `Error::Ambiguous` and never retried, so a message cannot be duplicated
  behind the caller's back.
* **Uploads** — photos, documents, audio, video, animations, voice notes and
  albums can carry bytes from memory or disk (multipart), not just `file_id`s.
* **Webhook** — constant-time check of `X-Telegram-Bot-Api-Secret-Token` and of
  the webhook path derived from the secret (`sha256` hex).
* **Updates** — typed `Update`/`Message`/`CallbackQuery`/`InlineQuery` plus
  `UpdateCursor` for exactly-once polling.

General-purpose: routing, persistence and the decision *what* to do after an
ambiguous failure stay in the caller.

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

Uploading a local file instead of referencing one:

```rust,no_run
use telegram_api_client::{ReqwestTransport, SendPhoto, TelegramClient, UploadFile};

# async fn demo() -> telegram_api_client::Result<()> {
let client = TelegramClient::new("123:ABC", ReqwestTransport::new());
let photo = SendPhoto::new(-1001, "ignored").with_upload(UploadFile::from_path("cat.png")?);
client.send_photo(&photo).await?;
# Ok(()) }
```

## Implemented surface

| Bot API | client method | request type |
| --- | --- | --- |
| `sendMessage` | `send_text`, `send_message_request` | `SendMessage` |
| `sendPhoto` | `send_photo` | `SendPhoto` ¹ |
| `sendDocument` | `send_document` | `SendDocument` ¹ |
| `sendAudio` | `send_audio` | `SendAudio` ¹ |
| `sendVideo` | `send_video` | `SendVideo` ¹ |
| `sendAnimation` | `send_animation` | `SendAnimation` ¹ |
| `sendVoice` | `send_voice` | `SendVoice` ¹ |
| `sendMediaGroup` | `send_media_group` | `SendMediaGroup` ¹ |
| `sendLocation` | `send_location` | `SendLocation` |
| `sendVenue` | `send_venue` | `SendVenue` |
| `sendContact` | `send_contact` | `SendContact` |
| `sendPoll` | `send_poll` | `SendPoll` |
| `sendDice` | `send_dice` | `SendDice` |
| `editMessageText` | `edit_message_text` | `EditMessageText` |
| `editMessageCaption` | `edit_message_caption` | `EditMessageCaption` |
| `editMessageMedia` | `edit_message_media` | `EditMessageMedia` ¹ |
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
| `getMe` | `me` | — |
| `setWebhook` | `set_webhook` | `SetWebhook` |
| `deleteWebhook` | `delete_webhook` | `DeleteWebhook` |
| `getWebhookInfo` | `get_webhook_info` | — |
| `getUpdates` | `get_updates` | `GetUpdates` + `UpdateCursor` |
| `getFile` | `get_file`, `file_url` | `GetFile` |

¹ accepts an optional `UploadFile` through `with_upload`, which switches the
request to `multipart/form-data`.

The two 0.1 convenience wrappers `send_message(chat_id, text)` and `get_me()`
still exist with their original signatures and are **deprecated** since 0.1.1;
they return the raw `serde_json::Value` envelope. Use `send_text`,
`send_message_request` and `me` instead.

Reply/inline keyboards are typed (`ReplyMarkup`, `InlineKeyboardMarkup`,
`KeyboardButton`, …), as are `MessageEntity`, `LinkPreviewOptions`, `ChatId`
and the `MenuButton` variants.

## Delivery semantics

An HTTP call can fail in three ways, and the Bot API gives us no idempotency
key to distinguish the last two after the fact:

| Failure | What happened | Client behaviour | Caller should |
| --- | --- | --- | --- |
| 429 `Too Many Requests` | rejected before processing | **retried automatically**, waiting `retry_after` | nothing, unless the budget runs out |
| connect / DNS / TLS error | the request never left the machine | **retried automatically** | nothing |
| 5xx on a `get*` method | read failed, nothing changed | retried automatically (idempotent) | nothing |
| 5xx on a state-changing method | the message **may** have been accepted | `Error::Ambiguous`, **never retried** | decide from your own ledger |
| timeout / reset after the write | unknown | `Error::Ambiguous`, **never retried** | decide from your own ledger |
| 4xx (400/401/403/404/409, …) | rejected, nothing happened | returned immediately | fix the request |

So the crate is **at-most-once by default for ambiguous failures**: a message
is never silently duplicated, and a call never becomes at-least-once behind your
back. `Error::is_ambiguous()` is the test a caller needs:

```rust
use telegram_api_client::{Error, RetryPolicy};

let gateway = Error::Api { code: 502, description: "Bad Gateway".into() };

// Same 5xx, two requests, two outcomes:
let send = gateway.classify_for("sendMessage");
assert!(send.is_ambiguous() && !send.is_retryable());

let read = Error::Api { code: 502, description: "Bad Gateway".into() }.classify_for("getChat");
assert!(read.is_retryable() && !read.is_ambiguous());

// The attempt budget never overrides the classification.
assert!(!RetryPolicy::default().should_retry(1, &send));
```

Rule of thumb for callers: on `Error::Ambiguous`, look your own record up (did
I already see a message carrying this correlation payload?), then either accept
the uncertainty or re-send deliberately. Telegram has no
`biz_opaque_callback_data`-style dedupe field, so the usual technique is a
marker your own consumer can search for — a `reply_parameters` reference to a
message you already control, a callback id you persisted before sending, or an
idempotency key inside the text that your consumer ignores.

**Tuning.** `RetryPolicy` sets the attempt budget (default 3) and the backoff
curve; `with_sleeper` swaps the timer. The default `ThreadSleeper` waits on a
helper thread, so no async runtime is required — install your runtime's timer in
production if you prefer, or `NoDelay` in tests. `without_retry()` restores "the
caller owns every retry". The crate never sleeps between *your* retries and
never hides an ambiguous outcome.

## Uploads

`UploadFile` carries bytes plus a file name and a MIME type:

```rust
use telegram_api_client::{SendDocument, UploadFile};

let contract = UploadFile::new("Vertrag 2026.pdf", std::fs::read("contract.pdf").unwrap())
    .with_mime("application/pdf");
let request = SendDocument::new(-1001, "ignored").with_upload(contract);
assert!(request.validate().is_ok());
assert!(request.multipart_form().is_some());
```

* `UploadFile::from_path` reads from disk, rejecting files over 50 MB
  (`MAX_UPLOAD_BYTES`) **before** allocating.
* The MIME type is guessed from the extension (`guess_mime`) and may be
  overridden; a syntactically invalid type (`is_valid_mime`) is refused.
* Each method accepts only the families Telegram accepts: `image/` for photos,
  `audio/` for audio and voice, `video/` for video, either for animations,
  anything for documents.
* File names are checked: empty, longer than 255 bytes, or containing control
  characters, quotes or backslashes is refused (header injection). Unicode
  names are preserved verbatim.
* Empty files are refused before the request is built.
* In albums every uploaded item is attached as `file0`, `file1`, … and the
  `media` array references them as `attach://file0`, exactly as documented.
* The encoded body is deterministic: `MultipartForm::boundary` is derived from
  the content (no `rand` dependency), so the wire format is testable byte for
  byte.

`Transport::post_multipart` is part of the trait, with a default implementation
that reports "this transport cannot upload" — a JSON-only transport keeps
compiling and fails loudly instead of silently dropping the file.

## Behaviour contract

- **Permissive decoding.** Unknown JSON fields are ignored, unknown enum values
  are carried (`ParseMode::Unknown`, `ChatType::Unknown`, `ReplyMarkup::Unknown`,
  …) and re-serialized unchanged, missing optionals default, and both
  `snake_case` and `camelCase` spellings are accepted.
- **Validation before the wire.** Text length is counted in UTF-16 code units
  like Telegram does (4096 for text, 1024 for captions), coordinates must lie in
  `-90..=90` / `-180..=180`, album sizes in `2..=10`, poll options in `2..=12`,
  uploads within the size and MIME rules above. Violations are
  `Error::InvalidRequest { field, .. }` — no network call.
- **429 is first class.** `Error::RateLimited { retry_after, .. }` keeps
  Telegram's `parameters.retry_after`; if only the description carries the hint
  it is parsed out. `RetryPolicy::delay_for_error` prefers that hint over its own
  exponential curve.
- **Secrets.** The bot token never appears in `Debug` (`TelegramClient`,
  `SetWebhook`), comparisons are constant time (`subtle`), and an empty
  configured secret is a hard error rather than a wildcard.
- **Concurrency.** `TelegramClient<T>` is `Send + Sync` when `T` is, and is
  cheap to clone; the test suite hammers one client from 64 tasks. Every method
  takes `&self`.

## Deliberate omissions

- **Automatic re-send of ambiguous requests.** Never — see "Delivery
  semantics". The decision needs the caller's ledger, which the crate does not
  have.
- **Long-polling driver.** `get_updates` + `UpdateCursor` give exactly-once
  semantics; the loop around them belongs to the caller.
- **Forum topics** (`createForumTopic`, `editForumTopic`, `closeForumTopic`,
  `reopenForumTopic`, `deleteForumTopic`, `generalForumTopic*`,
  `getForumTopicIconStickers`) — not modelled; `message_thread_id` on sends is.
- **Payments and invoices** (`sendInvoice`, `createInvoiceLink`,
  `answerShippingQuery`, `answerPreCheckoutQuery`, `refundStarPayment`) — out of
  scope: taking money needs a payment-provider integration this crate cannot
  own.
- **Games** (`sendGame`, `setGameScore`, `getGameHighScores`) — out of scope; no
  caller in the product uses them.
- **Telegram Passport** (encrypted passport data, `PassportElementError`) — out
  of scope: it is a cryptographic protocol of its own.
- **Sticker-set management** (`createNewStickerSet`, `addStickerToSet`, …) and
  **bot command management** (`setMyCommands`, `getMyCommands`, …) — not
  modelled; `setChatMenuButton` covers the menu-button entry point.
- **Fuzz targets.** Property tests cover the encoders and the delivery
  classification; a `cargo fuzz` target per parser is still open (issue #3).

## Quality gates

CI runs `cargo fmt --check`, `cargo clippy --all-targets --all-features --
-D warnings`, `cargo test` with and without default features, `cargo doc` with
`-D warnings`, `cargo deny check`, and a coverage job
(`cargo llvm-cov --all-features --fail-under-lines 95`). Local numbers for
0.1.1: **97.6 % line, 96.9 % region, 99.0 % function coverage**. The lines that
remain uncovered are JSON-shape guards and OS-level branches (a thread that
cannot be spawned, a socket that vanishes mid-flight) that cannot be forced
deterministically.

Tests are grouped by intent: `delivery.rs` (retry and ambiguity contract),
`multipart.rs` (uploads and their edge matrix), `properties.rs` (proptest
invariants, including "no state-changing request is auto-retried after a 5xx"),
`http.rs` (the `reqwest` transport against a local socket), `surface.rs` (every
public builder), plus `api.rs`, `edge_cases.rs`, `webhook.rs`, `update.rs`,
`client.rs` and `concurrency.rs`.

## Licence

Apache-2.0 OR MIT.
