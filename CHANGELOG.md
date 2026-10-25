# Changelog

All notable changes to this crate are documented in this file. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the crate
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html); what that
means for this API is spelled out in the README under
[Versioning and MSRV](README.md#versioning-msrv-and-changelog).

## [Unreleased]

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
