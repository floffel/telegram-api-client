# Wire contracts

Every shape in this crate is pinned to a published document. This file records
what was read, when, and what the fixture in `tests/` asserts, so a shape can be
re-checked instead of remembered.

## API surface contract

The crate separates values Telegram sends from values callers send:

- Update, event, media, chat-member, file and webhook-info structs are received
  payloads. They carry `#[non_exhaustive]`, so adding a field is patch-compatible
  and downstream struct literals cannot pin the current field set.
- Request builders (`SendMessage`, `GetChat`, `SetWebhook`, and the other method
  bodies) are caller-constructed. They remain exhaustive literals so required
  parameters stay explicit and request ergonomics are unchanged.

Received structs with all-optional members derive or implement `Default`. For a
received type with required identifiers, prefer its constructor (`Update::new`,
`File::new`, `MaskPosition::new`); otherwise start from `Type::default()` and
assign fields. `MessageId` remains decode-only because a fabricated zero would
mean a scheduled copy, and `PollMedia` exposes constructors over its private
representation. `..Default::default()` is not a downstream construction escape
hatch for a `#[non_exhaustive]` struct. `tests/api_surface.rs` pins both the
construction paths and the received-type list.

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
| restricted `is_member` | `Option<bool>`: `Some(true)`/`Some(false)` from the wire, `None` when omitted |
| presence | `presence()` / `is_present()`: creator/administrator/member are present; restricted is absent only for explicit `is_member: false`; left/kicked are absent; unknown is neither |

| Item | Value |
| --- | --- |
| Source URL | <https://core.telegram.org/bots/api#chatmember> |
| Fetched | 2026-11-01 |
| Fixtures | `tests/edge_cases.rs` (all six shapes, the required pair, an unmodelled status, per-field types) and `tests/properties.rs` (round trip) |

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

## `BanChatMember.until_date`

Quote, `banChatMember` `until_date`: *"Date when the user will be unbanned; Unix
time. If user is banned for more than 366 days or less than 30 seconds from the
current time they are considered to be banned forever."*

`0` is Telegram's explicit permanent-ban spelling, and omitting the field means
the same. For a nonzero temporary ban, `BanChatMember::validate_at(now)` accepts
exactly the inclusive window `now + 30 seconds ..= now + 366 days`.
`BanChatMember::validate()` delegates to it with the current system clock.
Timestamps below or above that window are rejected locally because Telegram
would otherwise silently make them permanent. A clock before the Unix epoch or
a window that overflows `i64` fails closed.

| Test | `now` | `until_date` | Result |
| --- | --- | --- | --- |
| too soon | `n` | `n + 29` | rejected |
| earliest temporary ban | `n` | `n + 30` | accepted |
| latest temporary ban | `n` | `n + 366 days` | accepted |
| too far | `n` | `n + 366 days + 1` | rejected |
| permanent, omitted | `n` | `None` | accepted |
| permanent, explicit | `n` | `0` | accepted |

| Item | Value |
| --- | --- |
| Source URL | <https://core.telegram.org/bots/api#banchatmember> |
| Fetched | 2026-11-06 |

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

## `copyMessage` and `MessageId`

Quote, `MessageId`: *"Unique message identifier. In specific instances (e.g.,
message containing a video sent to a big chat), the server might automatically
schedule a message instead of sending it immediately. In such cases, this field
will be **0** and the relevant message will be unusable until it is actually
sent."* Quote, `copyMessage`: *"Returns the `MessageId` of the sent message on
success."*

The crate therefore requires the `message_id` member during decoding and accepts
`0` as the documented scheduled-copy state. `MessageId::is_pending()` reports
that state. Negative values are rejected because no Telegram message id is
negative. Omitted, `null`, mistyped and misspelled members are decode errors,
not an implicit zero; `MessageId` deliberately has no `Default` implementation
that could fabricate the same value.

| Item | Value |
| --- | --- |
| Source URL | <https://core.telegram.org/bots/api#messageid> |
| Source URL | <https://core.telegram.org/bots/api#copymessage> |
| Fetched | 2026-11-06 |

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

## Media references and uploads

Quote, "Sending files": *"There are three ways to send files (photos, stickers,
audio, media, etc.):"*

1. *"If the file is already stored somewhere on the Telegram servers, you don't
   need to reupload it: each file object has a `file_id` field, simply pass this
   `file_id` as a parameter instead of uploading. There are no limits for files
   sent this way."*
2. *"Provide Telegram with an HTTP URL for the file to be sent. Telegram will
   download and send the file. 5 MB max size for photos and 20 MB max for other
   types of content."*
3. *"Post the file using multipart/form-data in the usual way that files are
   uploaded via the browser. 10 MB max size for photos, 50 MB for other files."*

The three are one field, not three fields, so the crate keeps them in one field
too: `SendPhoto::photo`, `InputMedia::media` and the sibling fields carry a
`file_id` or URL, and `with_upload` adds the bytes next to it. What goes on the
wire is decided by the encoder, and validation has to agree with it:

| Request | Reference on the wire | `validate()` |
| --- | --- | --- |
| upload, any reference (empty too) | rewritten to the part (`attach://file0`) or the part itself | accepted — the string is a placeholder |
| no upload, `file_id` or URL | sent verbatim | accepted |
| no upload, empty | sent verbatim, resolves to nothing | `Error::InvalidRequest`, "must not be empty without an upload" |
| no upload, `attach://…` | sent verbatim, no part to satisfy it | `Error::InvalidRequest`, "is an attach:// placeholder" |

| Item | Value |
| --- | --- |
| Source URL | <https://core.telegram.org/bots/api#sending-files> |
| Fetched | 2026-11-02 |

## Known gaps (tracked, not hidden)

- `Message` still models the subset the crate can act on (id, chat, sender,
  date, text, caption, thread id). The rest of the object is a known gap.
