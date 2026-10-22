//! Typed client and webhook toolkit for the Telegram Bot API.
//!
//! * [`webhook`] verifies the `X-Telegram-Bot-Api-Secret-Token` header and the
//!   derived webhook path.
//! * [`update`] parses `Update` payloads into typed messages.
//! * [`client`] sends messages through a pluggable [`Transport`].
//!
//! Nothing here knows about a product: persistence and routing stay in the caller.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod client;
pub mod error;
pub mod transport;
pub mod update;
pub mod webhook;

pub use client::TelegramClient;
pub use error::{Error, Result};
#[cfg(feature = "http")]
pub use transport::ReqwestTransport;
pub use transport::Transport;
pub use update::{Chat, Message, Update};
pub use webhook::WebhookGuard;
