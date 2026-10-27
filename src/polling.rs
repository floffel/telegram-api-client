//! Long polling: `getUpdates` plus a cursor that guarantees no double processing.
//!
//! # The offset is a watermark, not a set
//!
//! `getUpdates` returns the updates whose id is at or above `offset`, and
//! Telegram reads `offset = n` as "everything below `n` is handled": it confirms
//! those updates, forgets them and never sends them again. Three consequences
//! follow, and they shape every loop built on this module.
//!
//! * Confirmation is irrevocable. An update that was confirmed but never
//!   processed is gone for good. Confirm only what was actually handled, which
//!   is the whole reason [`UpdateCursor`] exists instead of a bare `last_id + 1`.
//! * Confirmation is cumulative. `offset = 12` confirms 1..=11 whether or not
//!   the caller saw them, so the watermark only moves forward and holes cannot
//!   be filled in later.
//! * Updates expire. The Bot API keeps them for 24 hours, so a consumer that is
//!   down for longer resumes at the persisted offset and loses the difference.
//!   That window is the at-least-once horizon of every client; no bookkeeping
//!   on our side can widen it.
//!
//! # Batch or single acknowledgement
//!
//! [`UpdateCursor::confirm`] takes a whole batch and advances past its highest
//! id, correct for "these three are done, move on". [`UpdateCursor::ack`]
//! advances one update at a time and only across a contiguous id, so a crash in
//! the middle of a batch redelivers the tail instead of losing it. Prefer `ack`
//! when the batch is processed item by item: a non-contiguous `ack` is refused
//! and leaves the cursor untouched rather than confirming an unhandled update.
//!
//! # Aborting cleanly
//!
//! The crate owns no runtime and spawns no task, so "stop polling" is "stop
//! awaiting": a loop checks its shutdown flag and breaks, or the future is
//! dropped in a `select!`. Dropping a `get_updates` future aborts the request and
//! confirms nothing, because confirmation only ever happens through the cursor.
//! Persist [`UpdateCursor::next_offset`] to make the restart point survive the
//! process and resume with [`UpdateCursor::after`]: whatever was acked stays
//! acked, whatever was not is redelivered — possibly twice, never zero times.
//!
//! ```
//! use std::sync::atomic::{AtomicBool, Ordering};
//! use telegram_api_client::{GetUpdates, MockReply, MockTransport, TelegramClient, UpdateCursor};
//!
//! # async fn demo() -> telegram_api_client::Result<()> {
//! let transport = MockTransport::new();
//! transport.push(MockReply::result(serde_json::json!([
//!     {"update_id": 7, "message": {"message_id": 1, "chat": {"id": -1}, "text": "one"}},
//!     {"update_id": 8, "message": {"message_id": 2, "chat": {"id": -1}, "text": "two"}}
//! ])));
//! let client = TelegramClient::new("123:ABC", transport.clone());
//!
//! let shutdown = AtomicBool::new(false);
//! let mut cursor = UpdateCursor::new();
//!
//! while !shutdown.load(Ordering::Relaxed) {
//!     let mut request = GetUpdates::new().timeout(25);
//!     if let Some(offset) = cursor.next_offset() {
//!         request = request.offset(offset);
//!     }
//!     for update in client.get_updates(&request).await? {
//!         // The work happens first; the acknowledgement is what makes it final.
//!         let handled = update.text().is_some();
//!         assert!(handled);
//!         cursor.ack(update.update_id);
//!     }
//!     // A real loop waits for its shutdown signal here instead of stopping at once.
//!     shutdown.store(true, Ordering::Relaxed);
//! }
//!
//! // Everything was acked, so a restart continues at 9 and redelivers nothing.
//! assert_eq!(cursor.next_offset(), Some(9));
//! # Ok(()) }
//! ```

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::types::UpdateKind;
use crate::update::Update;

/// Largest batch `getUpdates` will return.
pub const MAX_UPDATES_LIMIT: u16 = 100;

/// `getUpdates` body.
///
/// ```
/// use telegram_api_client::GetUpdates;
///
/// let request = GetUpdates::new().offset(11).limit(50).timeout(25);
/// let json = serde_json::to_value(&request).unwrap();
/// assert_eq!(json["offset"], 11);
/// assert_eq!(json["limit"], 50);
/// assert_eq!(json["timeout"], 25);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct GetUpdates {
    /// First update id to return; everything below is confirmed to Telegram.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
    /// Batch size, `1..=100`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u16>,
    /// Long-polling timeout in seconds; `0` polls and returns immediately.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u32>,
    /// Update kinds to fetch; empty means all.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_updates: Vec<UpdateKind>,
}

impl GetUpdates {
    /// A request with Telegram's defaults (all updates, limit 100, no wait).
    ///
    /// ```
    /// use telegram_api_client::GetUpdates;
    ///
    /// assert_eq!(serde_json::to_string(&GetUpdates::new()).unwrap(), "{}");
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts at `offset`.
    #[must_use]
    pub fn offset(mut self, offset: i64) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Sets the batch size.
    #[must_use]
    pub fn limit(mut self, limit: u16) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets the long-polling timeout.
    #[must_use]
    pub fn timeout(mut self, seconds: u32) -> Self {
        self.timeout = Some(seconds);
        self
    }

    /// Restricts the update kinds.
    #[must_use]
    pub fn allowed_updates(mut self, kinds: Vec<UpdateKind>) -> Self {
        self.allowed_updates = kinds;
        self
    }

    /// Checks the offset and batch size.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for a negative offset, a `limit` of `0` or
    /// above [`MAX_UPDATES_LIMIT`], or an empty update kind.
    pub fn validate(&self) -> Result<()> {
        if self.offset.is_some_and(|offset| offset < 0) {
            return Err(Error::InvalidRequest {
                field: "offset",
                reason: "must be zero or positive".to_string(),
            });
        }
        if self
            .limit
            .is_some_and(|limit| limit == 0 || limit > MAX_UPDATES_LIMIT)
        {
            return Err(Error::InvalidRequest {
                field: "limit",
                reason: format!("must be within 1..={MAX_UPDATES_LIMIT}"),
            });
        }
        if self
            .allowed_updates
            .iter()
            .any(|kind| kind.as_str().is_empty())
        {
            return Err(Error::InvalidRequest {
                field: "allowed_updates",
                reason: "update kinds must not be empty".to_string(),
            });
        }
        Ok(())
    }
}

/// Tracks the `getUpdates` offset so an update is processed exactly once.
///
/// Telegram confirms every update *below* `offset`; the cursor only advances
/// after you call [`UpdateCursor::confirm`], so a crash between receiving and
/// processing re-delivers the batch instead of losing it.
///
/// ```
/// use telegram_api_client::{Update, UpdateCursor};
///
/// let batch: Vec<Update> = serde_json::from_str(r#"[{"update_id": 10}, {"update_id": 11}]"#).unwrap();
///
/// let mut cursor = UpdateCursor::new();
/// assert_eq!(cursor.next_offset(), None);
///
/// let fresh = cursor.confirm(&batch);
/// assert_eq!(fresh.len(), 2);
/// assert_eq!(cursor.next_offset(), Some(12));
///
/// // Replaying the same batch yields nothing new.
/// assert!(cursor.confirm(&batch).is_empty());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UpdateCursor {
    /// Highest id handed out so far; everything at or below it is confirmed.
    last_confirmed: Option<i64>,
}

impl UpdateCursor {
    /// A cursor that has confirmed nothing yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A cursor resuming after `last_confirmed_id`.
    ///
    /// ```
    /// use telegram_api_client::UpdateCursor;
    ///
    /// assert_eq!(UpdateCursor::after(41).next_offset(), Some(42));
    /// ```
    #[must_use]
    pub fn after(last_confirmed_id: i64) -> Self {
        Self {
            last_confirmed: Some(last_confirmed_id),
        }
    }

    /// The offset to send with the next `getUpdates` call.
    #[must_use]
    pub fn next_offset(&self) -> Option<i64> {
        self.last_confirmed.map(|last| last.saturating_add(1))
    }

    /// The highest confirmed id, i.e. the watermark worth persisting.
    ///
    /// `None` means nothing has been confirmed and the next poll starts at the
    /// beginning of the 24-hour backlog.
    ///
    /// ```
    /// use telegram_api_client::UpdateCursor;
    ///
    /// assert_eq!(UpdateCursor::new().last_confirmed(), None);
    /// assert_eq!(UpdateCursor::after(41).last_confirmed(), Some(41));
    /// assert_eq!(UpdateCursor::after(41).next_offset(), Some(42));
    /// ```
    #[must_use]
    pub fn last_confirmed(&self) -> Option<i64> {
        self.last_confirmed
    }

    /// Acknowledges one processed update, moving the watermark by exactly one.
    ///
    /// Returns `true` when the cursor moved. It moves only for an id that is one
    /// past the current watermark (or for any positive id while nothing is
    /// confirmed yet), because confirmation is cumulative: acknowledging an id
    /// above a hole would confirm the hole as well. A backwards, duplicate,
    /// non-contiguous or non-positive id returns `false` and leaves the cursor
    /// untouched, so that update is redelivered instead of silently dropped.
    ///
    /// Use this when a batch is processed item by item, and [`Self::confirm`]
    /// when the whole batch is handled at once.
    ///
    /// ```
    /// use telegram_api_client::UpdateCursor;
    ///
    /// let mut cursor = UpdateCursor::after(41);
    /// assert!(cursor.ack(42));
    /// assert_eq!(cursor.next_offset(), Some(43));
    ///
    /// // A gap is refused: confirming 44 would also confirm the unhandled 43.
    /// assert!(!cursor.ack(44));
    /// assert_eq!(cursor.last_confirmed(), Some(42));
    ///
    /// // So is a replayed id, and so is going backwards.
    /// assert!(!cursor.ack(42));
    /// assert!(!cursor.ack(41));
    ///
    /// // An empty cursor takes the first positive id and nothing below it.
    /// let mut fresh = UpdateCursor::new();
    /// assert!(!fresh.ack(0));
    /// assert!(fresh.ack(1));
    /// assert_eq!(fresh.next_offset(), Some(2));
    /// ```
    pub fn ack(&mut self, update_id: i64) -> bool {
        if update_id < 1 {
            return false;
        }
        let advanced = match self.last_confirmed {
            None => true,
            // `checked_add`, not `saturating_add`: at `i64::MAX` the saturated
            // successor is the maximum itself, which would accept a replay.
            Some(last) => last.checked_add(1) == Some(update_id),
        };
        if advanced {
            self.last_confirmed = Some(update_id);
        }
        advanced
    }

    /// Accepts a batch: returns the updates that were not seen before, ordered
    /// by ascending update id, and advances the cursor past the highest id.
    ///
    /// Duplicate ids inside one batch are collapsed; ids at or below the
    /// current offset are dropped.
    ///
    /// The whole batch is confirmed at once, so call this only once every update
    /// in it is handled: a crash in between confirms the unhandled remainder and
    /// loses it. For item-by-item processing use [`Self::ack`], which never moves
    /// across an id that was not acknowledged.
    ///
    /// ```
    /// use telegram_api_client::{Update, UpdateCursor};
    ///
    /// let batch: Vec<Update> =
    ///     serde_json::from_str(r#"[{"update_id": 5}, {"update_id": 5}, {"update_id": 7}]"#).unwrap();
    /// let mut cursor = UpdateCursor::after(6);
    /// let fresh = cursor.confirm(&batch);
    /// assert_eq!(fresh.len(), 1);
    /// assert_eq!(fresh[0].update_id, 7);
    /// assert_eq!(cursor.next_offset(), Some(8));
    /// ```
    pub fn confirm(&mut self, updates: &[Update]) -> Vec<Update> {
        let mut seen = HashSet::new();
        let mut fresh = Vec::new();
        let mut highest = self.last_confirmed;

        for update in updates {
            let already_confirmed = self
                .last_confirmed
                .is_some_and(|last| update.update_id <= last);
            if already_confirmed || !seen.insert(update.update_id) {
                continue;
            }
            highest = Some(highest.map_or(update.update_id, |id| id.max(update.update_id)));
            fresh.push(update.clone());
        }

        // Telegram delivers ascending ids, but a replayed or proxied batch may
        // not; sorting keeps the "no id twice, never backwards" guarantee.
        fresh.sort_by_key(|update| update.update_id);
        self.last_confirmed = highest;
        fresh
    }
}
