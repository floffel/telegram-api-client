//! Long polling: `getUpdates` plus a cursor that guarantees no double processing.

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

    /// Accepts a batch: returns the updates that were not seen before, ordered
    /// by ascending update id, and advances the cursor past the highest id.
    ///
    /// Duplicate ids inside one batch are collapsed; ids at or below the
    /// current offset are dropped.
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
