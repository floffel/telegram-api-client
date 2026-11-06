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
given object."* The object is modelled as a private one-of value with
constructors and typed accessors. Decoding requires exactly one member; a
non-empty `photo` array is required for the photo member. Unknown member names
and adjacent fields are retained as [`serde_json::Value`], so a newer Bot API
round-trips without losing data (#57, #58).

| Field | Type | Constructor / accessor |
| --- | --- | --- |
| `animation` | Animation | `PollMedia::animation` / `animation_value` |
| `audio` | Audio | `PollMedia::audio` / `audio_value` |
| `document` | Document | `PollMedia::document` / `document_value` |
| `link` | Link | `PollMedia::link` / `link_value` |
| `live_photo` | LivePhoto | `PollMedia::live_photo` / `live_photo_value` |
| `location` | Location | `PollMedia::location` / `location_value` |
| `photo` | Array of PhotoSize | `PollMedia::photo` / `photo_value` |
| `sticker` | Sticker | `PollMedia::sticker` / `sticker_value` |
| `venue` | Venue | `PollMedia::venue` / `venue_value` |
| `video` | Video | `PollMedia::video` / `video_value` |
| any other name | JSON value | `PollMediaKind::Unknown` / `unknown_value` |

`LivePhoto.photo`, `Video.cover`, `Video.qualities` and `PollMedia.photo` are
bounded at `MAX_MEDIA_COLLECTION_ITEMS = 100` during deserialization. The bound
is enforced by a sequence visitor before the vector grows, so an attacker-size
array cannot trigger proportional pre-allocation (#59).

The required fields in every media object are required on decode. There are no
default empty identifiers or zero dimensions for absent fields (#56). Unknown
enum strings such as `MaskPoint` are retained; `MaskPosition` rejects an empty
point, non-finite shifts, and non-finite or non-positive scales both while
decoding and before serialization (#61).

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

## `MenuButton`

Quote, "MenuButton": *"This object describes the bot's menu button in a private
chat. It should be one of `MenuButtonCommands`, `MenuButtonWebApp`,
`MenuButtonDefault`"* — a tagged union on `type`. Three member names are
modelled; everything else is carried.

| `type` | Modelled as | Members the crate models |
| --- | --- | --- |
| `default` | `MenuButton::Default { extra }` | `type` |
| `commands` | `MenuButton::Commands { extra }` | `type` |
| `web_app` | `MenuButton::WebApp { text, url, extra }` | `type`, `text`, `web_app.url`; `text` and `web_app.url` are required strings |
| anything else | `MenuButton::Unknown(Value)` | the whole object, verbatim |

Every other member of a known variant is kept in `extra` and written back
unchanged, including the members of the nested `web_app` object that sit next
to `url`. The modelled members win over a preserved copy of the same name, so a
crafted `extra` cannot rewrite `type`, `text` or the URL. A recognized
`web_app` object missing either required string is a deserialization error; the
crate does not substitute empty strings (#18).
`tests/edge_cases.rs::menu_button_preserves_unknown_members_on_known_variants`
pins the round trip for all three known variants; `tests/properties.rs` does the
same for arbitrary member names and values.

| Item | Value |
| --- | --- |
| Source URL | <https://core.telegram.org/bots/api#menubutton> |
| Fetched | 2026-11-02 |

## Inline answer and result button validation

Quote, `answerInlineQuery` `next_offset`: *"Offset length can't exceed 64
bytes."* Quote, `InlineQueryResultsButton`: *"You must use exactly one of the
optional fields."* Its `start_parameter` is 1-64 characters from `A-Z`, `a-z`,
`0-9`, `_` and `-`; `web_app` is a `WebAppInfo`, whose `url` is an HTTPS URL.

The crate validates all four rules before posting `answerInlineQuery`, and
validates a `MenuButtonWebApp` label and HTTPS URL in
`SetChatMenuButton::validate`. Callback answer text uses Telegram's documented
200-character limit as UTF-16 code units.

| Item | Value |
| --- | --- |
| Source URL | <https://core.telegram.org/bots/api#answerinlinequery> |
| Source URL | <https://core.telegram.org/bots/api#inlinequeryresultsbutton> |
| Source URL | <https://core.telegram.org/bots/api#setchatmenubutton> |
| Fetched | 2026-11-06 |

## Known gaps (tracked, not hidden)

- `Message` still models the subset the crate can act on (id, chat, sender,
  date, text, caption, thread id). The rest of the object is a known gap.
