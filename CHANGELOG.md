# Changelog

All notable changes to this crate are documented in this file. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the crate
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html); what that
means for this API is spelled out in the README under
[Versioning and MSRV](README.md#versioning-msrv-and-changelog).

## [Unreleased]

### Changed

- **Numeric media members are unsigned, and a negative one is a parse error**
  (#60). `file_size`, `duration`, `width`, `height` and `start_timestamp` were
  `i64`, so `{"file_size": -1}` deserialized into a media object that cannot
  exist and was re-serialized unchanged. They are now `u64` (sizes, durations,
  `start_timestamp`) and `u32` (pixel dimensions), and the value is checked while
  parsing: a negative number fails with `Error::Payload` naming the member ("must
  not be negative"), a number above the member's maximum with "is out of range".
  Nothing is clamped, so a malformed object can never masquerade as a valid one.
  Serde compatibility is otherwise unchanged: absent members still default, `null`
  stays `None`, camelCase aliases are validated like the snake_case names, and
  unknown members are still ignored. **Breaking** for callers that build those
  structs or do arithmetic on the fields: the new types keep a caller from
  passing an impossible value, but they do not compile against `i64` any more.

### Fixed

- **`with_source` keeps the cause for `NotSent` and `Transport`** (#41). The
  method only ever filled the `Ambiguous` slot, so a transport that reported
  `Error::not_sent("dns failed").with_source(io_error)` lost the DNS/TLS cause
  even though `NotSent` has a source field, and an already attached source was
  replaced instead of preserved. It now attaches to every variant with a source
  slot (`Ambiguous`, `NotSent`, `Transport`) and chains a second attachment
  through `SourceChain`, so the newest failure is shown and the earlier one stays
  reachable. Classification is untouched: `is_retryable`, `is_ambiguous`,
  `retry_after` and `Display` are identical with and without a cause.
- **A zero `parameters.retry_after` is no longer a hint** (#42). A 429 carrying
  `"retry_after": 0` became `RateLimited { retry_after: Some(0) }`, which shadowed
  a perfectly good textual hint ("Too Many Requests: retry after 6") and let
  `RetryPolicy` retry with no wait at all until the attempt budget ran out. Both
  hint sources now go through one filter: a zero is an absent hint, and the
  description is consulted instead - exactly like a missing field.
- **Retry-hint parsing no longer allocates in proportion to its input** (#43).
  `parse_retry_after` copied the whole description to lowercase it and collected
  the digits into a second `String`, so a megabyte-long description from a hostile
  endpoint cost a megabyte of allocations per 429. The scan is now allocation-free
  and bounded twice: `MAX_RETRY_HINT_SCAN_BYTES` (512) bytes of description and 20
  significant digits (leading zeros do not count). Both limits are tested on both
  sides, and the scan window is cut on a char boundary so a multi-byte character
  straddling it cannot panic.
- The reported hint stays **unclamped**: a `retry_after` longer than
  `RetryPolicy::max_delay` is still handed to the caller unchanged, because
  shortening a server hint is how a retry loop walks into a second 429.
- **The deprecated `send_message` and `get_me` wrappers share the typed gate**
  (#31). Both posted a hand-built body straight to the transport and returned
  whatever came back: `send_message` skipped `SendMessage::validate`, so an empty
  or over-long text reached the network instead of failing with
  `Error::InvalidRequest`, and neither wrapper ran the envelope check, so a
  `{"ok": false}` body — or an answer without `result` — was handed back as a
  success when the transport yielded it. They now build the same request the
  typed path builds, validate it first, and put the answer through the same
  `result` gate before returning the envelope. Signatures, the posted bodies and
  the raw-`Value` return type are unchanged; only the failures that used to
  escape are errors now. The `MAX_MESSAGE_TEXT` boundary pair is tested on the
  wrapper itself, and the envelope cases run against a transport that returns the
  body verbatim: `ok: false`, a missing `result`, and a body-level 429, whose
  `retry_after` hint survives the trip through the gate.

## [0.1.8] - 2026-10-28

### Added

- **Every `Update` kind of Bot API 10.3.** The `Update` object modelled six of
  its 27 payload fields; all of them are now typed: `business_connection`,
  `business_message`, `edited_business_message`, `deleted_business_messages`,
  `guest_message`, `message_reaction`, `message_reaction_count`,
  `chosen_inline_result`, `shipping_query`, `pre_checkout_query`,
  `purchased_paid_media`, `poll`, `poll_answer`, `my_chat_member`, `chat_member`,
  `chat_join_request`, `chat_boost`, `removed_chat_boost`, `managed_bot`,
  `subscription`, `stopped_message_generation` — plus the four message variants
  the crate already had. The payload types live in the new `events` module.
- `BOT_API_VERSION` pins the revision the shapes were checked against (**10.3**);
  `docs/contracts.md` records the field table and its source URL.
- `Update::kind()` classifies an update into `UpdateKind`, `UpdateKind::ALL`
  lists all 27 kinds in `Update` field order, and `chat_member_update()` reaches
  a membership change from either `my_chat_member` or `chat_member`.
- `tests/update_surface.rs` carries one JSON fixture per kind and fails when a
  kind has none: classification, serde round trip, unknown-field tolerance and
  camelCase aliases are all asserted from the same table.
- `UpdateCursor::ack` acknowledges one processed update and moves the watermark
  only across a contiguous id, so a crash in the middle of a batch redelivers the
  unhandled tail instead of losing it. `UpdateCursor::last_confirmed` exposes the
  watermark itself for persistence, and `MAX_UPDATES_LIMIT` is re-exported at the
  crate root.
- The polling module documents the `getUpdates` offset contract — confirmation is
  irrevocable and cumulative, updates expire after 24 hours — plus how to end a
  loop cleanly. `tests/long_polling.rs` pins it (watermark, gaps, descending and
  duplicated batches, an interruption between two acks) and
  `examples/poll_loop.rs` runs the documented loop against a mock.

- **`PollMedia` and the member objects it carries.** A poll question, its
  explanation and a poll option can each carry media; `Poll::media`,
  `Poll::explanation_media` and `PollOption::media` were absent from the model,
  so such a poll parsed with that content silently dropped. All three are typed
  now, with `Animation`, `Audio`, `Document`, `Link`, `LivePhoto`, `Location`,
  `PhotoSize[]`, `Sticker`, `Venue` and `Video` modelled from the field tables of
  Bot API 10.3 (`docs/contracts.md` records the source URL, the fetch date and the
  field-to-Rust mapping). `PollMedia` is a flat object with ten optional members,
  not a tagged union — issue #16 had assumed `#[serde(tag = "type")]` and the
  published object says otherwise — so `PollMedia::kind()` and `is_empty()` read
  it without guessing.
- `VideoQuality` and `MaskPosition` are modelled as well, `Video` carries
  `qualities`, and `Sticker` carries `mask_position` and `premium_animation`.
  `MaskPosition` compares its floats by bit pattern, so `Eq` stays sound for
  `NaN`, matching `Location`.
### Changed

- **`Update` payload fields are boxed** (`Option<Box<Message>>` and so on).
  With all 27 fields inline the struct had grown to 8184 bytes, so a
  `getUpdates` batch of 100 updates copied ~800 KB and tripped
  `clippy::large_stack_arrays` in two call sites; one `Update` is now 224 bytes.
  Field access is unchanged (`Box` derefs), but code that names the field types
  explicitly — `fn handle(update: Option<Message>)` — has to say `Box<Message>`.
  A test pins the size at 512 bytes.

## [0.1.7] - 2026-10-27

### Fixed

- The `fuzz` CI job could never run: `taiki-e/install-action@cargo-fuzz` is not a
  recipe the action knows, so it reported success while installing nothing and
  the next step failed with `no such command: fuzz`. It now installs the tool
  itself (`cargo install cargo-fuzz --locked`). No change to the library: 0.1.6
  was published by a green `Release` run, only its `CI` run was red on that
  job.

## [0.1.6] - 2026-10-27

### Fixed

- A `MultipartFile` MIME type reached the part header unescaped: a value such as
  `"image/png\r\nX-Evil: 1"` added a header line to the encoded body. Field names
  and file names were already neutralised, the MIME type was interpolated
  verbatim. Found by the new `multipart` fuzz target on its first run; pinned by
  `a_mime_type_cannot_inject_headers` and the property
  `multipart_header_values_never_add_a_line`. `UploadFile::validate` already
  rejected such a type, but `MultipartForm` is public, so the encoder has to hold
  on its own.

### Added

- `fuzz/`: a `cargo-fuzz` target per parser — `update`
  (`serde_json::from_slice::<Update>` on arbitrary bytes, plus a serialise/parse
  round trip), `webhook` (`WebhookGuard::verify_and_parse` on arbitrary secret,
  header, path and body; a wrong secret is always the *first* error) and
  `multipart` (the encoder on arbitrary names, values, MIME types and payloads;
  one header line per header, payload verbatim). Each ships a seed corpus.
- A CI `fuzz` job on nightly: `cargo fmt --check` and
  `cargo clippy --all-targets -- -D warnings` on the fuzz crate, `cargo fuzz
  build`, then a 20-second smoke run per target. A target that stops compiling
  or starts panicking turns the run red.
- README section "Fuzzing" naming the three targets and what each asserts.
  `fuzz/**` is excluded from the published package, so the coverage number and
  the MSRV job are unaffected.

## [0.1.5] - 2026-10-26

### Fixed

- Photo uploads were validated against the 50 MB document budget, so a 30 MB
  "photo" passed the pre-flight check and was rejected by the server instead of
  by the crate. The ceiling now belongs to the operation: `sendPhoto`, a photo
  item in `sendMediaGroup` and `editMessageMedia` carrying a photo are capped at
  `MAX_PHOTO_BYTES` (10 MB), while documents, audio, video, animations and voice
  notes keep `MAX_UPLOAD_BYTES` (50 MB). Quoted from the Bot API, "Sending
  files": "10 MB max size for photos, 50 MB for other files".

### Added

- `UploadFile::validate_with_limit(field, families, max_bytes)`: the check the
  request types now call, so the ceiling is an argument instead of a constant.
  `validate()` and `validate_for()` keep the 50 MB budget and are otherwise
  unchanged.
- `MAX_PHOTO_URL_BYTES` (5 MB) and `MAX_FILE_URL_BYTES` (20 MB), the ceilings for
  sending by URL, plus `MAX_DOWNLOAD_BYTES` (20 MB) and
  `File::fits_download_limit()` for `getFile`. A URL carries no size, so the
  crate documents these instead of pretending to enforce them.
- `InputMediaKind::upload_limit_bytes()`, which albums and media edits use to
  pick the ceiling of each item.
- `tests/limits.rs`: the N/N+1 boundary per operation, per item kind and for the
  download ceiling, with two property tests (the ceiling is inclusive, and a
  refusal names the limit that applied).

## [0.1.4] - 2026-10-26

### Changed

- The default `ThreadSleeper` delegates its wait to `futures-timer` (3.0.4)
  instead of a hand-written future with its own waker slot and one spawned
  thread per wait. `Delay` completes on whichever executor drives the future and
  needs no runtime activation, which was re-verified from a bare
  `futures::executor::block_on`; the `Sleeper` trait, `NoDelay` and the retry
  arithmetic are unchanged.
- `src/retry.rs` now documents, with measured numbers, why the backoff curve
  stays local while the wait does not: `backoff` 0.4 has had no release since
  2021-12 and its `next_backoff` needs `&mut self`, `backon` 1.6 computes the
  curve in `f32` seconds, and `retry-policies` 0.5 answers with a `SystemTime`
  and carries no server hint (issue #15).

### Added

- `examples/send_text.rs`, `examples/poll_once.rs` and `examples/webhook.rs`, all
  runnable without a bot token (offline the first two print the request body or
  replay a `MockTransport` batch, the third needs no network at all), plus
  `cargo build --examples --all-features` as a named CI gate.

## [0.1.3] - 2026-10-25

### Added

- `Error::NotSent` for failures that provably happened before the request was
  written (connect, DNS, TLS). It is retryable for every method, sends
  included, and is what `ReqwestTransport` now reports for a refused
  connection. `Error::Transport` keeps its role as the *unphased* failure and is
  no longer retryable on its own: `classify_for` turns it into `Ambiguous` for a
  state-changing method and `NotSent` for a read.
- `MockReply::not_sent()` and `MockReply::unphased()` so a mock transport can
  exercise all three phases.

### Changed

- `RetryPolicy::should_retry` refuses a retry whose `retry_after` is longer than
  `max_delay` instead of retrying early, and `delay_for_error` returns Telegram's
  hint unchanged instead of clamping it. A 60 s hint with the default 30 s
  ceiling now ends the call with `Error::RateLimited`.
- `delay_for_error(attempt, …)` documents `attempt` as the 1-based attempt that
  failed and computes the backoff from `attempt - 1`: the first retry waits
  `base_delay`, not `2 * base_delay`.
- `parse_retry_after` also understands the flood-control wording
  (`"Flood control exceeded. Retry in 12 seconds"`) and still rejects zero,
  missing and overflowing values.
- `WebhookGuard` stores only `sha256(secret)` and compares digests, so the
  secret comparison no longer leaks the configured token's length; the guard also
  applies `setWebhook`'s secret rules and refuses to be built from a token
  Telegram would not register.
- `ThreadSleeper` tracks actual completion with an atomic flag instead of
  treating a second poll as "the sleep finished", and no longer completes
  early when it cannot spawn a thread.

### Fixed

- A non-idempotent request can no longer be retried after a transport failure
  whose phase is unknown, which was the last path that could duplicate a message.
- A spurious or repeated poll of the default sleeper no longer skips the backoff
  or rate-limit delay.

## [0.1.2] - 2026-10-25

### Changed

- `reqwest` 0.12 → 0.13 (`json` + `rustls` features) and `sha2` 0.10 → 0.11.
  Dependency-only release: no public API change, same MSRV.

## [0.1.1] - 2026-10-25

### Added

- `Error::Ambiguous` for every outcome that cannot be told apart from "the
  server processed the request" — a 5xx on a state-changing method and any
  transport failure observed after the request was written. `is_retryable()` is
  `false` and `is_ambiguous()` is `true` for it; `Error::classify_for(method)`
  is the pure split and is pinned by a property test.
- Multipart uploads: `UploadFile` (bytes or `from_path`), `MultipartForm` /
  `MultipartFile` with a deterministic boundary and header escaping,
  `Transport::post_multipart` with a default implementation, `with_upload()` on
  every `sendX`, `attach://` wiring for albums, and `editMessageMedia` with
  upload support.
- `MockTransport` records the multipart parts (`Call::file_bytes()`), so upload
  tests need neither network nor credentials.
- Typed replacements `TelegramClient::send_text()` and `me()`. The 0.1 wrappers
  `send_message()` and `get_me()` keep their signatures and are deprecated.
- Coverage job in CI (`cargo llvm-cov --all-features --fail-under-lines 95`);
  the threshold fails the build instead of only being reported.
- README sections: delivery-semantics table (429 / pre-write / 5xx-read /
  5xx-send / post-write / 4xx), uploads and limits, typed-vs-deprecated method
  table, deliberate omissions.

### Changed

- Automatic retries are bounded and safe: 429 (honouring `retry_after`) and
  pre-write connect failures only. Reads stay retryable after a 5xx; a
  state-changing request is never retried behind the caller's back.
- `RetryPolicy` stays pure; waiting goes through the `Sleeper` seam whose
  default (`ThreadSleeper`) needs no async runtime. `without_retry()` hands the
  loop back to the caller.

### Deprecated

- `TelegramClient::send_message()` and `TelegramClient::get_me()` — use
  `send_text()` / `send_message_request()` and `me()`.

## [0.1.0] - 2026-10-24

### Added

- Typed client for the Bot API surface documented in the README: sends
  (`sendMessage` … `sendDice`), edits (`editMessageText`, `editMessageCaption`,
  `editMessageReplyMarkup`, `deleteMessage`), forwards (`forwardMessage`,
  `copyMessage`), callback and inline-query answers, chat administration, webhook
  management and `getUpdates` with `UpdateCursor`.
- `Transport` trait with the `reqwest` implementation behind the default-on
  `http` feature and `MockTransport` for tests; `cargo test
  --no-default-features` runs the whole suite offline.
- `Update` parsing into typed messages, entities and keyboards; unknown fields
  and unknown enum variants are carried through instead of failing the parse.
- `WebhookGuard` verifying the `X-Telegram-Bot-Api-Secret-Token` header and the
  derived webhook path, with constant-time comparison and a constructor that
  refuses an empty secret.
- Retry engine with `RetryPolicy`, `Sleeper` and `is_idempotent()`.
- Dual licence Apache-2.0 OR MIT, `#![forbid(unsafe_code)]`,
  `#![deny(missing_docs)]`, `cargo-deny` policy and tag-driven release CI.

[Unreleased]: https://github.com/floffel/telegram-api-client/compare/v0.1.2...HEAD
[0.1.2]: https://github.com/floffel/telegram-api-client/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/floffel/telegram-api-client/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/floffel/telegram-api-client/releases/tag/v0.1.0
