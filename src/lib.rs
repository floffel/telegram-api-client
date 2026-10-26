//! Typed client and webhook toolkit for the Telegram Bot API.
//!
//! * [`webhook`] verifies the `X-Telegram-Bot-Api-Secret-Token` header and the
//!   derived webhook path.
//! * [`update`] parses `Update` payloads into typed messages.
//! * [`client`] sends every modelled method through a pluggable [`Transport`].
//! * [`types`] holds `ChatId`, parse modes, entities and keyboards.
//!
//! Nothing here knows about a product: persistence and routing stay in the caller.
//!
//! ```
//! use telegram_api_client::{MockTransport, SendMessage, TelegramClient};
//!
//! # async fn demo() -> telegram_api_client::Result<()> {
//! let transport = MockTransport::new();
//! let client = TelegramClient::new("123:ABC", transport.clone());
//!
//! let sent = client.send_message_request(&SendMessage::new(-1001, "ping")).await?;
//! assert_eq!(transport.last_call().unwrap().method(), "sendMessage");
//! # let _ = sent;
//! # Ok(()) }
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod callback;
pub mod chat;
pub mod client;
pub mod edit;
pub mod error;
pub mod file;
pub mod polling;
pub mod retry;
pub mod send;
pub mod transport;
pub mod types;
pub mod update;
pub mod webhook;
pub mod webhook_api;

pub use callback::{
    AnswerCallbackQuery, AnswerInlineQuery, InlineQueryResult, InlineQueryResultsButton,
    MenuButton, SetChatMenuButton,
};
pub use chat::{
    BanChatMember, ChatMember, GetChat, GetChatMember, PinChatMessage, SendChatAction,
    UnpinChatMessage,
};
pub use client::TelegramClient;
pub use edit::{
    CopyMessage, DeleteMessage, EditMessageCaption, EditMessageMedia, EditMessageReplyMarkup,
    EditMessageText, EditResult, EditTarget, ForwardMessage, MessageId,
};
pub use error::{parse_retry_after, Error, Result};
pub use file::{File, GetFile, MAX_DOWNLOAD_BYTES};
pub use polling::{GetUpdates, UpdateCursor};
pub use retry::{is_idempotent, NoDelay, RetryPolicy, Sleeper, ThreadSleeper};
pub use send::{
    guess_mime, is_valid_mime, DeliveryOptions, MediaOptions, MessageOptions, SendAnimation,
    SendAudio, SendContact, SendDice, SendDocument, SendLocation, SendMediaGroup, SendMessage,
    SendPhoto, SendPoll, SendVenue, SendVideo, SendVoice, UploadFile, MAX_FILE_URL_BYTES,
    MAX_PHOTO_BYTES, MAX_PHOTO_URL_BYTES, MAX_UPLOAD_BYTES,
};
#[cfg(feature = "http")]
pub use transport::ReqwestTransport;
pub use transport::{Call, MockReply, MockTransport, MultipartFile, MultipartForm, Transport};
pub use types::{
    ChatAction, ChatId, ChatMemberStatus, ChatType, EntityKind, ForceReply, InlineKeyboardButton,
    InlineKeyboardMarkup, InputMedia, InputMediaKind, KeyboardButton, LinkPreviewOptions,
    MessageEntity, ParseMode, PollType, ReplyKeyboardMarkup, ReplyKeyboardRemove, ReplyMarkup,
    UpdateKind, WebAppInfo,
};
pub use update::{CallbackQuery, Chat, InlineQuery, Message, Update, User};
pub use webhook::WebhookGuard;
pub use webhook_api::{DeleteWebhook, SetWebhook, WebhookInfo};
