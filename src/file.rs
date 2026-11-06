//! `getFile` and the download URL it unlocks.

use std::collections::BTreeMap;

use serde::de::Error as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, Result};

/// Most bytes Telegram serves for one downloaded file (20 MB).
///
/// The Bot API, `File`: "The maximum file size to download is 20 MB". `getFile`
/// answers without a `file_path` when the file is too large, and this crate does
/// not stream downloads, so check [`File::fits_download_limit`] (or this
/// constant against a size you know) before pulling bytes through the URL from
/// `TelegramClient::file_url`.
///
/// ```rust
/// use telegram_api_client::MAX_DOWNLOAD_BYTES;
///
/// assert_eq!(MAX_DOWNLOAD_BYTES, 20 * 1024 * 1024);
/// assert!(MAX_DOWNLOAD_BYTES < telegram_api_client::MAX_UPLOAD_BYTES);
/// ```
pub const MAX_DOWNLOAD_BYTES: usize = 20 * 1024 * 1024;

/// Longest accepted `file_id`, in bytes.
///
/// The Bot API does not publish a maximum. This crate uses a deliberately
/// generous ceiling so malformed or resource-heavy requests are refused before
/// they reach the wire without rejecting real identifiers.
///
/// ```
/// use telegram_api_client::MAX_FILE_ID_BYTES;
///
/// assert_eq!(MAX_FILE_ID_BYTES, 256);
/// ```
pub const MAX_FILE_ID_BYTES: usize = 256;

/// `getFile` body.
///
/// ```
/// use telegram_api_client::GetFile;
///
/// assert_eq!(serde_json::to_value(GetFile::new("AgACAgIAAxkBAAI")).unwrap()["file_id"], "AgACAgIAAxkBAAI");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct GetFile {
    /// `file_id` of the file to resolve.
    pub file_id: String,
}

impl GetFile {
    /// Resolves `file_id`.
    #[must_use]
    pub fn new(file_id: impl Into<String>) -> Self {
        Self {
            file_id: file_id.into(),
        }
    }

    /// Checks the file id.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] when the file id is empty, has surrounding
    /// whitespace, contains a control character, or is longer than
    /// [`MAX_FILE_ID_BYTES`].
    pub fn validate(&self) -> Result<()> {
        validate_file_id(&self.file_id, "file_id").map_err(|reason| Error::InvalidRequest {
            field: "file_id",
            reason,
        })
    }
}

/// Rejects an empty, padded, control-bearing, or oversized Bot API file id.
fn validate_file_id(file_id: &str, field: &'static str) -> std::result::Result<(), String> {
    if file_id.trim().is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    if file_id.trim() != file_id {
        return Err(format!("{field} must not have surrounding whitespace"));
    }
    if file_id.len() > MAX_FILE_ID_BYTES {
        return Err(format!(
            "{field} must be at most {MAX_FILE_ID_BYTES} bytes, got {}",
            file_id.len()
        ));
    }
    if file_id.chars().any(char::is_control) {
        return Err(format!("{field} must not contain control characters"));
    }
    Ok(())
}

/// A file handle returned by `getFile`.
///
/// `file_path` is only valid for about an hour — fetch it right before
/// downloading, and never persist it.
///
/// ```
/// use telegram_api_client::File;
///
/// let file: File = serde_json::from_str(
///     r#"{"file_id":"A","file_unique_id":"u","file_size":1024,"file_path":"photos/a.jpg"}"#,
/// ).unwrap();
/// assert_eq!(file.download_path(), Some("photos/a.jpg"));
/// assert_eq!(file.file_size, Some(1024));
/// ```
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct File {
    /// Identifier usable for downloads and re-sends.
    #[serde(alias = "fileId", deserialize_with = "deserialize_file_id")]
    pub file_id: String,
    /// Identifier stable across bots.
    #[serde(
        alias = "fileUniqueId",
        deserialize_with = "deserialize_file_unique_id"
    )]
    pub file_unique_id: String,
    /// Size in bytes, when known.
    #[serde(default, alias = "fileSize", skip_serializing_if = "Option::is_none")]
    pub file_size: Option<u64>,
    /// Path to append to the file endpoint, when known.
    #[serde(
        default,
        alias = "filePath",
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_file_path"
    )]
    pub file_path: Option<String>,
    /// Fields this crate does not model, forwarded unchanged.
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, Value>,
}

fn deserialize_file_id<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let file_id = String::deserialize(deserializer)?;
    validate_file_id(&file_id, "file_id").map_err(D::Error::custom)?;
    Ok(file_id)
}

fn deserialize_file_unique_id<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let file_id = String::deserialize(deserializer)?;
    validate_file_id(&file_id, "file_unique_id").map_err(D::Error::custom)?;
    Ok(file_id)
}

fn deserialize_file_path<'de, D>(deserializer: D) -> std::result::Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let path = Option::<String>::deserialize(deserializer)?;
    if let Some(path) = path.as_deref() {
        validate_download_path(path).map_err(D::Error::custom)?;
    }
    Ok(path)
}

/// Validates a Telegram file path as a relative, non-traversing URL path.
fn validate_download_path(path: &str) -> std::result::Result<(), String> {
    validate_file_path(path, "file_path")
}

fn validate_file_path(path: &str, field: &'static str) -> std::result::Result<(), String> {
    if path.is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    if path.trim().is_empty() {
        return Err(format!("{field} must not be whitespace-only"));
    }
    if path.starts_with('/') {
        return Err(format!("{field} must be relative, without a leading slash"));
    }
    if path.contains(['?', '#', '\\']) {
        return Err(format!("{field} must not contain `?`, `#`, or backslashes"));
    }
    if path.chars().any(char::is_control) {
        return Err(format!("{field} must not contain control characters"));
    }
    for segment in path.split('/') {
        if segment.is_empty() {
            return Err(format!("{field} must not contain empty segments"));
        }
        if segment.trim() != segment {
            return Err(format!(
                "{field} segments must not have surrounding whitespace"
            ));
        }
        if matches!(segment, "." | "..") {
            return Err(format!("{field} must not contain `.` or `..` segments"));
        }
    }
    Ok(())
}

impl File {
    /// The relative path for the download URL, if Telegram provided one.
    ///
    /// Paths that are not a safe relative URL path are ignored.
    ///
    /// ```
    /// use telegram_api_client::File;
    ///
    /// let file: File = serde_json::from_str(
    ///     r#"{"file_id":"A","file_unique_id":"u"}"#,
    /// ).unwrap();
    /// assert_eq!(file.download_path(), None);
    /// ```
    #[must_use]
    pub fn download_path(&self) -> Option<&str> {
        self.file_path
            .as_deref()
            .filter(|path| validate_download_path(path).is_ok())
    }

    /// Whether Telegram gave us enough to build a download URL.
    ///
    /// ```
    /// use telegram_api_client::File;
    ///
    /// let file: File = serde_json::from_str(
    ///     r#"{"file_id":"A","file_unique_id":"u","file_path":"photos/a.jpg"}"#,
    /// ).unwrap();
    /// assert!(file.is_downloadable());
    /// ```
    #[must_use]
    pub fn is_downloadable(&self) -> bool {
        self.download_path().is_some()
    }

    /// Whether Telegram reported a size this crate may download.
    ///
    /// `None` when Telegram reported no `file_size`: the size is unknown, so the
    /// call has to try the download rather than guess.
    ///
    /// ```rust
    /// use telegram_api_client::{File, MAX_DOWNLOAD_BYTES};
    ///
    /// let at_limit: File = serde_json::from_str(&format!(
    ///     r#"{{"file_id":"A","file_unique_id":"u","file_size":{MAX_DOWNLOAD_BYTES}}}"#,
    /// )).unwrap();
    /// assert_eq!(at_limit.fits_download_limit(), Some(true));
    ///
    /// let over: File = serde_json::from_str(&format!(
    ///     r#"{{"file_id":"A","file_unique_id":"u","file_size":{}}}"#,
    ///     MAX_DOWNLOAD_BYTES + 1,
    /// )).unwrap();
    /// assert_eq!(over.fits_download_limit(), Some(false));
    ///
    /// let unknown: File = serde_json::from_str(r#"{"file_id":"A","file_unique_id":"u"}"#).unwrap();
    /// assert_eq!(unknown.fits_download_limit(), None);
    /// ```
    #[must_use]
    pub fn fits_download_limit(&self) -> Option<bool> {
        self.file_size.map(|size| size <= MAX_DOWNLOAD_BYTES as u64)
    }
}
