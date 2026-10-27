# Wire contracts

Every shape in this crate is pinned to a published document. This file records
what was read, when, and what the fixture in `tests/` asserts, so a shape can be
re-checked instead of remembered.

## Bot API revision

| Item | Value |
| --- | --- |
| Source | <https://core.telegram.org/bots/api> |
| Version on the page | **Bot API 10.3** (latest changelog entry: August 24, 2026) |
| Fetched | 2026-10-27 |
| Pinned in code | `telegram_api_client::BOT_API_VERSION` |

`BOT_API_VERSION` is the revision the `Update` object, the `Message` object and
the payload types in `src/events.rs` were checked against. A fixture per update
kind lives in `tests/update_surface.rs`; `UpdateKind::ALL` lists the same 27
kinds, and the test fails if one of them has no fixture.

## `Update` object

Quote, "Update": *"This object represents an incoming update. At most one of the
optional fields can be present in any given update."*

| Field | Type | Modelled as |
| --- | --- | --- |
| `update_id` | Integer | `Update::update_id` |
| `message` | Message | `Update::message` |
| `edited_message` | Message | `Update::edited_message` |
| `channel_post` | Message | `Update::channel_post` |
| `edited_channel_post` | Message | `Update::edited_channel_post` |
| `business_connection` | BusinessConnection | `Update::business_connection` |
| `business_message` | Message | `Update::business_message` |
| `edited_business_message` | Message | `Update::edited_business_message` |
| `deleted_business_messages` | BusinessMessagesDeleted | `Update::deleted_business_messages` |
| `guest_message` | Message | `Update::guest_message` |
| `message_reaction` | MessageReactionUpdated | `Update::message_reaction` |
| `message_reaction_count` | MessageReactionCountUpdated | `Update::message_reaction_count` |
| `inline_query` | InlineQuery | `Update::inline_query` |
| `chosen_inline_result` | ChosenInlineResult | `Update::chosen_inline_result` |
| `callback_query` | CallbackQuery | `Update::callback_query` |
| `shipping_query` | ShippingQuery | `Update::shipping_query` |
| `pre_checkout_query` | PreCheckoutQuery | `Update::pre_checkout_query` |
| `purchased_paid_media` | PaidMediaPurchased | `Update::purchased_paid_media` |
| `poll` | Poll | `Update::poll` |
| `poll_answer` | PollAnswer | `Update::poll_answer` |
| `my_chat_member` | ChatMemberUpdated | `Update::my_chat_member` |
| `chat_member` | ChatMemberUpdated | `Update::chat_member` |
| `chat_join_request` | ChatJoinRequest | `Update::chat_join_request` |
| `chat_boost` | ChatBoostUpdated | `Update::chat_boost` |
| `removed_chat_boost` | ChatBoostRemoved | `Update::removed_chat_boost` |
| `managed_bot` | ManagedBotUpdated | `Update::managed_bot` |
| `subscription` | BotSubscriptionUpdated | `Update::subscription` |
| `stopped_message_generation` | MessageGenerationStopped | `Update::stopped_message_generation` |

## Known gaps (tracked, not hidden)

- `PollMedia` (the `media` / `explanation_media` union of `Animation`, `Audio`,
  `Document`, `Link`, `LivePhoto`, `Location`, `PhotoSize`, `Sticker`, `Venue`,
  `Video`) is not modelled; the poll fields that carry it are absent from
  `Poll`. Tracked in issue #16.
- `Message` still models the subset the crate can act on (id, chat, sender,
  date, text, caption, thread id). The rest of the object is a known gap.
