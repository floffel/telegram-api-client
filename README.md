# telegram-api-client

Minimal, typed client and webhook toolkit for the Telegram Bot API.

* **Webhook** — constant-time check of `X-Telegram-Bot-Api-Secret-Token` and of the webhook path derived from the secret (`sha256` hex).
* **Updates** — typed `Update`/`Message`/`Chat` subset with helpers.
* **Client** — `sendMessage`, `getMe`, and any method via `method_url`, behind a pluggable `Transport` (no network needed in tests).

General-purpose: routing, persistence and retries stay in the caller.

Licensed under Apache-2.0 OR MIT.
