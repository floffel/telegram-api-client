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

## Examples

Four examples ship in the package, each runnable without a bot account —
without `TELEGRAM_BOT_TOKEN` they use `MockTransport` or build the request
offline instead of opening a socket, and print what they would do:

```text
cargo run --example send_text     # one message; without a token, the JSON body
cargo run --example poll_once     # drain one getUpdates batch, exactly once
cargo run --example poll_loop     # long-polling loop, stops between two batches
cargo run --example webhook       # verify secret + path, parse the update
```

`poll_once` is the one worth reading: it feeds a batch with a duplicate through
`UpdateCursor`, so the output shows `3 delivered, 2 not seen before` and a
redelivery of the same batch adds nothing. `webhook` runs fully offline and ends
by printing the three rejections — wrong secret, missing header, wrong path.

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
| 429 `Too Many Requests` | rejected before processing | **retried automatically**, waiting exactly `retry_after`; if the hint exceeds `max_delay` the error is returned instead of retrying early | nothing, unless the budget runs out |
| connect / DNS / TLS error (`Error::NotSent`) | the request never left the machine | **retried automatically**, even for a send | nothing |
| transport failure with no phase (`Error::Transport`) | the transport could not say whether anything was written | ambiguous for a send, retried for a `get*` | decide from your own ledger |
| 5xx on a `get*` method | read failed, nothing changed | retried automatically (idempotent) | nothing |
| 5xx on a state-changing method | the message **may** have been accepted | `Error::Ambiguous`, **never retried** | decide from your own ledger |
| timeout / reset after the write (`Error::Ambiguous`) | unknown | `Error::Ambiguous`, **never retried** | decide from your own ledger |
| 4xx (400/401/403/404/409, …) | rejected, nothing happened | returned immediately | fix the request |

Custom transports report the phase through three variants: `Error::NotSent`
(nothing written), `Error::Ambiguous` (written, outcome unknown) and — only if
the transport genuinely cannot tell — `Error::Transport`, which the client
resolves per method.

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
curve; `with_sleeper` swaps the timer. The default `ThreadSleeper` delegates the
wait to `futures-timer`, whose `Delay` completes on whatever executor drives it,
with no tokio/async-std/smol feature and no runtime activation; install your own
timer in production if you prefer, or `NoDelay` in tests. The backoff arithmetic
stays local on purpose, and the measured comparison is in `src/retry.rs`:
`backoff` 0.4 has had no release since 2021-12 and its `next_backoff` needs
`&mut self`, `backon` 1.6 computes the curve in `f32` seconds, and
`retry-policies` 0.5 answers with a `SystemTime` and carries no server hint.
`without_retry()` restores "the
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
* The ceiling belongs to the operation, not to the bytes: `sendPhoto`, a photo
  item in `sendMediaGroup` and `editMessageMedia` carrying a photo are capped at
  `MAX_PHOTO_BYTES` (10 MB), everything else at `MAX_UPLOAD_BYTES` (50 MB) —
  "10 MB max size for photos, 50 MB for other files" in the Bot API docs.
  `UploadFile::validate_with_limit` takes the ceiling explicitly;
  `validate()`/`validate_for()` keep the general 50 MB budget.
* Sending by URL is capped by Telegram at 5 MB for photos and 20 MB for other
  files (`MAX_PHOTO_URL_BYTES`, `MAX_FILE_URL_BYTES`), and `getFile` downloads
  stop at 20 MB (`MAX_DOWNLOAD_BYTES`, pre-check with
  `File::fits_download_limit`). A URL carries no size, so the crate documents
  those three instead of pretending to enforce them.
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
- **TLS trust store.** The `http` feature builds `reqwest` with
  `default-features = false, features = ["json", "rustls"]`. Since `reqwest`
  0.13 the `rustls` feature validates against the *platform* trust store
  (`rustls-platform-verifier`) and the bundled `webpki-roots` feature is gone,
  so a minimal container without `ca-certificates` now fails to connect instead
  of trusting a shipped bundle. Install a CA bundle there, or pass your own
  `reqwest::Client` through the `Transport` trait.

## Long polling

`getUpdates` has no "acknowledge this message" call: **the offset is the
acknowledgement**. Telegram reads `offset = n` as "everything below `n` is
handled", forgets those updates and never sends them again. Three properties
follow, and they decide how a safe loop looks:

- **Confirmation is irrevocable.** An update confirmed but not processed is gone.
  That is why `UpdateCursor` exists instead of `last_id + 1` being sent blindly.
- **Confirmation is cumulative.** `offset = 12` confirms 1..=11 whether or not
  they were seen, so the watermark only moves forward and holes cannot be filled
  later.
- **Updates expire after 24 hours.** A consumer down for longer resumes at its
  persisted offset and loses the difference; no client-side bookkeeping widens
  that window.

Two ways to move the cursor, and the choice is about crash cost:

- `UpdateCursor::confirm(&batch)` advances past the highest id of a batch that
  was handled **as a whole**. A crash between receiving and confirming repeats
  the batch; a crash after confirming but before finishing it loses the rest.
- `UpdateCursor::ack(id)` advances by exactly one, and only across the contiguous
  id. A hole, a replay or a backwards id is refused and leaves the cursor alone,
  so an update can be redelivered but never skipped. Prefer this whenever the
  batch is processed item by item.

A loop therefore looks like this — the crate owns no runtime and spawns no task,
so "stop polling" is "stop awaiting":

```rust
let mut cursor = UpdateCursor::after(persisted_offset);

while !stop.load(Ordering::Relaxed) {
    let mut request = GetUpdates::new().timeout(25);
    if let Some(offset) = cursor.next_offset() {
        request = request.offset(offset);
    }
    for update in client.get_updates(&request).await? {
        handle(&update).await?;          // work first,
        cursor.ack(update.update_id);    // acknowledgement second
    }
}
```

Dropping the `get_updates` future (a `select!` arm, a `tokio::time::timeout`, a
cancelled task) aborts the request and confirms nothing. Persist
`next_offset()` to survive the process and resume with `UpdateCursor::after`:
what was acked stays acked, what was not is delivered again — possibly twice,
never zero times. That is the delivery contract for polling; the webhook path is
at-most-once by comparison, because Telegram does not retry a failed delivery.

`cargo run --example poll_loop` runs exactly this loop against a mock, stops
between two polls and prints the restart, so the watermark is checkable without a
bot token.

## Deliberate omissions

- **Automatic re-send of ambiguous requests.** Never — see "Delivery
  semantics". The decision needs the caller's ledger, which the crate does not
  have.
- **A spawned long-polling task.** The loop belongs to the caller, which owns the
  runtime and the stop signal — see "Long polling" for the offset/acknowledge
  contract it has to respect.
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

## Quality gates

CI runs `cargo fmt --check`, `cargo clippy --all-targets --all-features --
-D warnings`, `cargo build --examples --all-features` (a named gate, so a
broken example fails under its own name), `cargo test` with and without default features, `cargo doc` with
`-D warnings`, `cargo deny check`, and a coverage job
(`cargo llvm-cov --all-features --fail-under-lines 95`). Local numbers for
0.1.2: **97.6 % line, 96.9 % region, 99.0 % function coverage**. The lines that
remain uncovered are JSON-shape guards and OS-level branches (a thread that
cannot be spawned, a socket that vanishes mid-flight) that cannot be forced
deterministically.

Tests are grouped by intent: `delivery.rs` (retry and ambiguity contract),
`multipart.rs` (uploads and their edge matrix), `properties.rs` (proptest
invariants, including "no state-changing request is auto-retried after a 5xx"),
`http.rs` (the `reqwest` transport against a local socket), `surface.rs` (every
public builder), plus `api.rs`, `edge_cases.rs`, `webhook.rs`, `update.rs`,
`client.rs` and `concurrency.rs`.

## Fuzzing

The three parsers that face the outside world have a `cargo-fuzz` target under
`fuzz/`, one per parser, built and smoked on nightly by CI:

- **`update`** — `serde_json::from_slice::<Update>` on arbitrary bytes (broken
  UTF-8, truncated objects, deeply nested arrays, unknown fields). Invariants:
  parsing never panics, every accessor is total, and a payload the crate accepts
  survives a serialise/parse round trip unchanged.
- **`webhook`** — `WebhookGuard::verify_and_parse` on arbitrary secret, header,
  path and body. Invariants: a missing or wrong secret is always `Err`, and it is
  the *first* error — `Payload` is only reported after both checks passed, so a
  forged request can never be misread as a malformed one.
- **`multipart`** — the encoder on arbitrary field names, values, file names,
  MIME types and payloads. Invariants: exactly one header line per header per
  part (nothing a caller passes can add a line or a part), the body arrives byte
  for byte, and encoding is deterministic.

```
cd fuzz
cargo fuzz build
cargo fuzz run update    seeds/update
cargo fuzz run webhook   seeds/webhook
cargo fuzz run multipart seeds/multipart
```

The targets need nightly and `cargo fuzz`; the library itself does not. `fuzz/`
is a standalone crate, excluded from the published package, so it cannot affect
the coverage number or the MSRV job.

The `multipart` target paid for itself on its first run: it produced a MIME type
(`"\n-"`) that added a header line to a part, because the field name and file
name were escaped while the MIME type was interpolated verbatim. Fixed in 0.1.6
and pinned by `multipart_header_values_never_add_a_line` in `tests/properties.rs`.

## Versioning, MSRV and changelog

The crate follows [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html). While the
major version is `0`, a minor bump (`0.1` → `0.2`) may break the API and a patch
bump (`0.1.1` → `0.1.2`) is guaranteed to be additive or a fix: nothing that
compiled before stops compiling, and nothing that behaved correctly starts
failing. Concretely, these are **patch-level** changes here:

- new methods, new `with_*` builder setters, new types;
- a new variant on a `#[non_exhaustive]` enum — the error enum and the payload
  enums that can grow are marked that way precisely so this stays patch-level;
- documentation, dependency patch/minor updates, and stricter validation for
  input the Bot API would have rejected anyway.

**Breaking** (minor under `0.x`, major from `1.0`): removing or renaming a
public item, changing a method signature, adding a variant to an enum that is
*not* `#[non_exhaustive]`, adding a required parameter to a request type, or
turning a currently-ignored input into an error.

Every release gets an entry in [CHANGELOG.md](CHANGELOG.md); the git tag carries
no notes of its own, the changelog is the record.

**MSRV:** Rust **1.89**, as declared in `Cargo.toml` (`rust-version`). CI
verifies the minimum on that exact toolchain instead of only claiming it.
Raising it counts as a breaking change (minor while `0.x`), because it can break
a consumer's build without anything in this crate changing; it never happens in
a patch release.

**docs.rs** builds with `--all-features`, so the `http` transport and the
offline `MockTransport` path are both visible in the rendered documentation.

## Licence

Apache-2.0 OR MIT.
