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

## `PollMedia`

Quote, "PollMedia": *"At most one of the optional fields can be present in any
given object."* The object is modelled as a flat set of ten optional fields,
not as a tagged union.

| Field | Type | Rust field |
| --- | --- | --- |
| `animation` | Animation | `PollMedia::animation` |
| `audio` | Audio | `PollMedia::audio` |
| `document` | Document | `PollMedia::document` |
| `link` | Link | `PollMedia::link` |
| `live_photo` | LivePhoto | `PollMedia::live_photo` |
| `location` | Location | `PollMedia::location` |
| `photo` | Array of PhotoSize | `PollMedia::photo` |
| `sticker` | Sticker | `PollMedia::sticker` |
| `venue` | Venue | `PollMedia::venue` |
| `video` | Video | `PollMedia::video` |

| Item | Value |
| --- | --- |
| Source URL | <https://core.telegram.org/bots/api> |
| Fetched | 2026-10-27 |

## `ChatMemberRestricted.is_member`

Quote, "ChatMemberRestricted": *"Represents a chat member that is under certain
restrictions in the chat. Supergroups only."* Its `is_member` row reads
*"True, if the user is a member of the chat at the moment of the request"* — so
the status alone does not say whether a restricted user is in the chat.
`ChatMember::presence()` reads the flag for that one status:

| Status and flag | `ChatMemberPresence` |
| --- | --- |
| `creator`, `administrator`, `member` | `Present` |
| `restricted`, `is_member: true` | `Present` |
| `restricted`, `is_member: false` | `Absent` |
| `restricted`, no flag sent | `Present` — a missing flag is not an absence |
| `left`, `kicked` | `Absent` — whatever `is_member` says |
| any other status | `Unknown` — neither present nor absent |

| Item | Value |
| --- | --- |
| Source URL | <https://core.telegram.org/bots/api#chatmemberrestricted> |
| Fetched | 2026-11-02 |

## Edit addressing: `chat_id`/`message_id` vs `inline_message_id`

The four edit methods (`editMessageText`, `editMessageCaption`,
`editMessageMedia`, `editMessageReplyMarkup`) share one parameter block. Quote,
`editMessageText`:

| Parameter | Quote |
| --- | --- |
| `chat_id` | *"Required if `inline_message_id` is not specified."* |
| `message_id` | *"Required if `inline_message_id` is not specified. Identifier of the message to edit."* |
| `inline_message_id` | *"Required if `chat_id` and `message_id` are not specified. Identifier of the inline message."* |

The condition is **specified**, not "non-blank", so the crate reads presence:
`EditTarget` serializes `inline_message_id` whenever it is `Some`, and
`EditTarget::validate` therefore treats every `Some` as the inline form. A blank
value cannot be silently dropped from the rule while staying in the body — it is
refused with `Error::InvalidRequest { field: "inline_message_id" }` instead
(#36).

| Item | Value |
| --- | --- |
| Source URL | <https://core.telegram.org/bots/api#editmessagetext> |
| Fetched | 2026-11-02 |

## Known gaps (tracked, not hidden)

- `Message` still models the subset the crate can act on (id, chat, sender,
  date, text, caption, thread id). The rest of the object is a known gap.
