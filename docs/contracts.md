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

## `ChatMember`

Quote, "ChatMember": *"This object contains information about one member of a
chat. Currently, the following 6 types of chat members are supported"* —
`ChatMemberOwner`, `ChatMemberAdministrator`, `ChatMemberMember`,
`ChatMemberRestricted`, `ChatMemberLeft`, `ChatMemberBanned`. Only `status` and
`user` are common to all six, so the object is modelled as a status-tagged enum
(`ChatMember`) with one payload struct per type, rather than as a struct that
drops the rest. The `status` word selects the shape; the crate's variant names
follow the payload types, so `"kicked"` decodes to `ChatMember::Banned`.

| `status` | Payload type | Status-specific fields | Crate |
| --- | --- | --- | --- |
| `creator` | `ChatMemberOwner` | `is_anonymous`, `custom_title` | `ChatMember::Creator` |
| `administrator` | `ChatMemberAdministrator` | `can_be_edited`, `is_anonymous`, `can_manage_chat`, `can_delete_messages`, `can_manage_video_chats`, `can_restrict_members`, `can_promote_members`, `can_change_info`, `can_invite_users`, `can_post_stories`, `can_edit_stories`, `can_delete_stories`, `can_post_messages`*, `can_edit_messages`*, `can_pin_messages`*, `can_manage_topics`*, `can_manage_direct_messages`*, `can_manage_tags`*, `can_send_welcome_messages`, `custom_title`* | `ChatMember::Administrator` |
| `member` | `ChatMemberMember` | `tag`*, `until_date`* | `ChatMember::Member` |
| `restricted` | `ChatMemberRestricted` | `tag`*, `is_member`, `can_send_messages`, `can_send_audios`, `can_send_documents`, `can_send_photos`, `can_send_videos`, `can_send_video_notes`, `can_send_voice_notes`, `can_send_polls`, `can_send_other_messages`, `can_add_web_page_previews`, `can_react_to_messages`, `can_edit_tag`, `can_change_info`, `can_invite_users`, `can_pin_messages`, `can_manage_topics`, `until_date` | `ChatMember::Restricted` |
| `left` | `ChatMemberLeft` | — | `ChatMember::Left` |
| `kicked` | `ChatMemberBanned` | `until_date` | `ChatMember::Banned` |

`*` optional in the Bot API. Every field is `user` plus the ones listed.

| Rule | Modelled as |
| --- | --- |
| `status` and `user` are mandatory | absence or `null` is a decode error (`Error::Payload`), never a default member (#26) |
| a status the crate does not model | `ChatMember::Unknown { status, user, extra }` — carried, not rejected |
| a field no variant models | that shape's `extra` map, re-serialised unchanged |
| an absent mandatory `can_*` right | `false` (under-reporting, never an invented permission) |
| an absent optional `can_*` right | `None` |
| `until_date` | the wire value, `0` meaning "forever" — not filtered out |
| presence | `is_present()`: true for creator/administrator/member, `is_member` for restricted, false for left/kicked/unknown |

| Item | Value |
| --- | --- |
| Source URL | <https://core.telegram.org/bots/api#chatmember> |
| Fetched | 2026-11-01 |
| Fixtures | `tests/edge_cases.rs` (all six shapes, the required pair, an unmodelled status, per-field types) and `tests/properties.rs` (round trip) |

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

## Known gaps (tracked, not hidden)

- `Message` still models the subset the crate can act on (id, chat, sender,
  date, text, caption, thread id). The rest of the object is a known gap.
