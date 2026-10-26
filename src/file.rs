//! `getFile` and the download URL it unlocks.

use serde::{Deserialize, Serialize};

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
    /// [`Error::InvalidRequest`] when the file id is empty.
    pub fn validate(&self) -> Result<()> {
        if self.file_id.trim().is_empty() {
            return Err(Error::InvalidRequest {
                field: "file_id",
                reason: "must not be empty".to_string(),
            });
        }
        Ok(())
    }
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
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct File {
    /// Identifier usable for downloads and re-sends.
    #[serde(default, alias = "fileId")]
    pub file_id: String,
    /// Identifier stable across bots.
    #[serde(default, alias = "fileUniqueId")]
    pub file_unique_id: String,
    /// Size in bytes, when known.
    #[serde(default, alias = "fileSize", skip_serializing_if = "Option::is_none")]
    pub file_size: Option<u64>,
    /// Path to append to the file endpoint, when known.
    #[serde(default, alias = "filePath", skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
}

impl File {
    /// The relative path for the download URL, if Telegram provided one.
    ///
    /// ```
    /// use telegram_api_client::File;
    ///
    /// assert_eq!(File::default().download_path(), None);
    /// ```
    #[must_use]
    pub fn download_path(&self) -> Option<&str> {
        self.file_path.as_deref().filter(|path| !path.is_empty())
    }

    /// Whether Telegram gave us enough to build a download URL.
    ///
    /// ```
    /// use telegram_api_client::File;
    ///
    /// let file = File { file_path: Some("photos/a.jpg".into()), ..File::default() };
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
    /// let at_limit = File { file_size: Some(MAX_DOWNLOAD_BYTES as u64), ..File::default() };
    /// assert_eq!(at_limit.fits_download_limit(), Some(true));
    ///
    /// let over = File { file_size: Some(MAX_DOWNLOAD_BYTES as u64 + 1), ..File::default() };
    /// assert_eq!(over.fits_download_limit(), Some(false));
    ///
    /// assert_eq!(File::default().fits_download_limit(), None);
    /// ```
    #[must_use]
    pub fn fits_download_limit(&self) -> Option<bool> {
        self.file_size.map(|size| size <= MAX_DOWNLOAD_BYTES as u64)
    }
}
