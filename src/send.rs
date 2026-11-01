//! `sendX` request bodies.
//!
//! Every struct serializes to the exact JSON the Bot API expects (`snake_case`,
//! absent optional fields omitted) and validates what can be validated without
//! the network. Media can be referenced by `file_id` or HTTP URL, or uploaded
//! from memory or disk with [`UploadFile`]; an upload turns the request into a
//! `multipart/form-data` body.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, Result};
use crate::transport::{MultipartFile, MultipartForm};
use crate::types::{
    ChatId, InputMedia, LinkPreviewOptions, MessageEntity, ParseMode, PollType, ReplyMarkup,
};

/// Longest accepted `text`, in UTF-16 code units (what Telegram counts).
pub const MAX_MESSAGE_TEXT: usize = 4096;
/// Longest accepted `caption`, in UTF-16 code units.
pub const MAX_CAPTION: usize = 1024;
/// Longest accepted poll question, in characters.
pub const MAX_POLL_QUESTION: usize = 300;
/// Longest accepted poll answer option, in characters.
pub const MAX_POLL_OPTION: usize = 100;
/// Most answer options a poll may have.
pub const MAX_POLL_OPTIONS: usize = 12;
/// Fewest answer options a poll may have.
pub const MIN_POLL_OPTIONS: usize = 2;
/// Most items a media group may contain.
pub const MAX_MEDIA_GROUP: usize = 10;
/// Fewest items a media group may contain.
pub const MIN_MEDIA_GROUP: usize = 2;
/// Most bytes the Bot API accepts for a non-photo upload (50 MB).
///
/// From the Bot API, "Sending files": "Post the file using multipart/form-data
/// in the usual way that files are uploaded via the browser. 10 MB max size for
/// photos, 50 MB for other files." Photos therefore get [`MAX_PHOTO_BYTES`];
/// `sendDocument`, `sendAudio`, `sendVideo`, `sendAnimation` and `sendVoice` use
/// this budget, and so do albums and message edits of non-photo media.
///
/// Checked before the request is built, so an oversized file is rejected
/// without allocating a body or opening a socket.
pub const MAX_UPLOAD_BYTES: usize = 50 * 1024 * 1024;
/// Most bytes a photo upload may carry (10 MB).
///
/// The same "Sending files" sentence caps photos at 10 MB, and `sendPhoto`
/// repeats it: "The photo must be at most 10 MB in size." A larger image still
/// fits the 50 MB budget of [`MAX_UPLOAD_BYTES`] when it goes out as a document.
///
/// This limit applies to `sendPhoto`, to a photo item in `sendMediaGroup` and to
/// `editMessageMedia` carrying a photo — every operation that validates through
/// [`UploadFile::validate_with_limit`].
///
/// ```rust
/// use telegram_api_client::{MAX_PHOTO_BYTES, MAX_UPLOAD_BYTES};
///
/// assert_eq!(MAX_PHOTO_BYTES, 10 * 1024 * 1024);
/// assert!(MAX_PHOTO_BYTES < MAX_UPLOAD_BYTES);
/// ```
pub const MAX_PHOTO_BYTES: usize = 10 * 1024 * 1024;
/// Most bytes Telegram downloads when a photo is sent by URL (5 MB).
///
/// "Sending files": "Provide Telegram with an HTTP URL for the file to be sent.
/// Telegram will download and send the file. 5 MB max size for photos and 20 MB
/// max for other types of content." A URL reference carries no size, so the
/// crate cannot enforce this ceiling; the constant is here for a caller that
/// knows the remote size and wants to check before handing over the URL.
///
/// ```rust
/// use telegram_api_client::MAX_PHOTO_URL_BYTES;
///
/// let remote_bytes = 4 * 1024 * 1024;
/// assert!(remote_bytes <= MAX_PHOTO_URL_BYTES);
/// ```
pub const MAX_PHOTO_URL_BYTES: usize = 5 * 1024 * 1024;
/// Most bytes Telegram downloads when any other file is sent by URL (20 MB).
///
/// The 20 MB half of the [`MAX_PHOTO_URL_BYTES`] sentence: documents, audio,
/// video, animations and voice notes. Enforced by the caller, for the same
/// reason — nothing about a URL tells the crate how large the file is.
///
/// ```rust
/// use telegram_api_client::{MAX_FILE_URL_BYTES, MAX_PHOTO_URL_BYTES};
///
/// let remote_bytes = 18 * 1024 * 1024;
/// assert!(remote_bytes <= MAX_FILE_URL_BYTES);
/// assert!(MAX_PHOTO_URL_BYTES < MAX_FILE_URL_BYTES);
/// ```
pub const MAX_FILE_URL_BYTES: usize = 20 * 1024 * 1024;
/// Longest accepted file name, in bytes.
pub const MAX_FILENAME_BYTES: usize = 255;

/// Rejects empty and over-long text, counting UTF-16 code units like Telegram.
pub(crate) fn validate_text(field: &'static str, value: &str, max: usize) -> Result<()> {
    if value.trim().is_empty() {
        return Err(Error::InvalidRequest {
            field,
            reason: "must not be empty".to_string(),
        });
    }
    let units = value.encode_utf16().count();
    if units > max {
        return Err(Error::InvalidRequest {
            field,
            reason: format!("must be at most {max} UTF-16 code units, got {units}"),
        });
    }
    Ok(())
}

/// Rejects an obviously unusable chat reference.
pub(crate) fn validate_chat(field: &'static str, chat: &ChatId) -> Result<()> {
    match chat {
        ChatId::Id(0) => Err(Error::InvalidRequest {
            field,
            reason: "chat id 0 is not a valid chat".to_string(),
        }),
        ChatId::Username(name) if name.is_empty() => Err(Error::InvalidRequest {
            field,
            reason: "username must not be empty".to_string(),
        }),
        ChatId::Id(_) | ChatId::Username(_) => Ok(()),
    }
}

/// Rejects a `message_thread_id` that names no message thread.
///
/// The value is in the same id space as `message_id` — it is the id of the
/// topic's opening message — so `0` and negative ids address nothing Telegram
/// can route a request to. Sending one comes back as a request error naming no
/// field; refusing it here names `message_thread_id` instead. `None` means "not
/// in a forum topic" and stays valid.
pub(crate) fn validate_thread(field: &'static str, thread: Option<i64>) -> Result<()> {
    match thread {
        Some(id) if id <= 0 => Err(Error::InvalidRequest {
            field,
            reason: format!("must be positive when set, got {id}"),
        }),
        _ => Ok(()),
    }
}

/// A file to upload: bytes, a name, and the MIME type Telegram should see.
///
/// ```rust
/// use telegram_api_client::UploadFile;
///
/// // MIME guessed from the extension, `application/octet-stream` otherwise.
/// let png = UploadFile::new("Grüße.png", vec![0x89, b'P', b'N', b'G']);
/// assert_eq!(png.mime, "image/png");
/// assert_eq!(png.len(), 4);
/// assert!(png.validate().is_ok());
///
/// // An explicit type wins over the guess.
/// let odd = UploadFile::new("data.bin", vec![1]).with_mime("application/x-custom");
/// assert_eq!(odd.mime, "application/x-custom");
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct UploadFile {
    /// Name sent in `Content-Disposition`; Unicode is kept as-is.
    pub filename: String,
    /// MIME type, `type/subtype`.
    pub mime: String,
    /// File contents.
    pub bytes: Vec<u8>,
}

impl UploadFile {
    /// A file from memory, with the MIME type guessed from `filename`.
    ///
    /// ```rust
    /// use telegram_api_client::UploadFile;
    ///
    /// let audio = UploadFile::new("voice.ogg", vec![0x4f, 0x67, 0x67, 0x53]);
    /// assert_eq!(audio.mime, "audio/ogg");
    ///
    /// let unknown = UploadFile::new("stuff", Vec::new());
    /// assert_eq!(unknown.mime, "application/octet-stream");
    /// ```
    #[must_use]
    pub fn new(filename: impl Into<String>, bytes: Vec<u8>) -> Self {
        let filename = filename.into();
        Self {
            mime: guess_mime(&filename).to_string(),
            filename,
            bytes,
        }
    }

    /// Reads a file from disk, rejecting an oversized file before allocating.
    ///
    /// The file name (not the whole path) is sent to Telegram.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] when the file cannot be read or is larger than
    /// [`MAX_UPLOAD_BYTES`].
    ///
    /// ```no_run
    /// use telegram_api_client::UploadFile;
    ///
    /// # fn demo() -> telegram_api_client::Result<()> {
    /// let photo = UploadFile::from_path("photo.png")?;
    /// assert_eq!(photo.filename, "photo.png");
    /// assert_eq!(photo.mime, "image/png");
    /// # Ok(()) }
    /// ```
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let metadata = std::fs::metadata(path).map_err(|error| Error::InvalidRequest {
            field: "file",
            reason: format!("cannot read {}: {error}", path.display()),
        })?;
        if metadata.len() > MAX_UPLOAD_BYTES as u64 {
            return Err(Error::InvalidRequest {
                field: "file",
                reason: format!(
                    "{MAX_UPLOAD_BYTES} bytes is the upload limit, {} has {}",
                    path.display(),
                    metadata.len()
                ),
            });
        }
        let bytes = std::fs::read(path).map_err(|error| Error::InvalidRequest {
            field: "file",
            reason: format!("cannot read {}: {error}", path.display()),
        })?;
        let filename = path
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        Ok(Self::new(filename, bytes))
    }

    /// Overrides the MIME type.
    ///
    /// ```rust
    /// use telegram_api_client::UploadFile;
    ///
    /// let file = UploadFile::new("blob", vec![1]).with_mime("image/png");
    /// assert_eq!(file.mime, "image/png");
    /// ```
    #[must_use]
    pub fn with_mime(mut self, mime: impl Into<String>) -> Self {
        self.mime = mime.into();
        self
    }

    /// Number of bytes to upload.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Whether the buffer is empty. Telegram rejects empty files.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Checks name, MIME type and size against the general 50 MB budget.
    ///
    /// This is the document budget ([`MAX_UPLOAD_BYTES`]). An operation that caps
    /// photos lower validates through [`Self::validate_with_limit`] with
    /// [`MAX_PHOTO_BYTES`] instead; [`Self::validate_for`] keeps this budget too.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an empty or unusable file name, a MIME type
    /// that is not `type/subtype`, an empty payload, or a payload over
    /// [`MAX_UPLOAD_BYTES`].
    ///
    /// ```rust
    /// use telegram_api_client::{Error, UploadFile};
    ///
    /// let empty = UploadFile::new("a.png", Vec::new());
    /// assert!(matches!(empty.validate(), Err(Error::InvalidRequest { field: "file", .. })));
    ///
    /// let bad_mime = UploadFile::new("a.png", vec![1]).with_mime("png");
    /// assert!(matches!(bad_mime.validate(), Err(Error::InvalidRequest { field: "mime", .. })));
    ///
    /// let injection = UploadFile::new("a\"\r\nX-Evil: 1.png", vec![1]);
    /// assert!(injection.validate().is_err());
    /// ```
    pub fn validate(&self) -> Result<()> {
        self.validate_with_limit("file", &[], MAX_UPLOAD_BYTES)
    }

    /// Checks the file against one operation limit: MIME families and bytes.
    ///
    /// `max_bytes` is the ceiling of the operation that will carry the file —
    /// [`MAX_PHOTO_BYTES`] for `sendPhoto`, [`MAX_UPLOAD_BYTES`] for a document.
    /// An empty `families` slice accepts any type, which is right for
    /// `sendDocument`.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an empty or unusable file name, a MIME type
    /// that is not `type/subtype`, an empty payload, a payload of more than
    /// `max_bytes` bytes, or a MIME type outside `families`.
    ///
    /// ```rust
    /// use telegram_api_client::{MAX_PHOTO_BYTES, MAX_UPLOAD_BYTES, UploadFile};
    ///
    /// // Exactly at the photo limit: accepted.
    /// let at_limit = UploadFile::new("a.png", vec![0; MAX_PHOTO_BYTES]);
    /// assert!(at_limit.validate_with_limit("photo", &["image/"], MAX_PHOTO_BYTES).is_ok());
    ///
    /// // One byte over: refused, and the message names the limit.
    /// let over = UploadFile::new("a.png", vec![0; MAX_PHOTO_BYTES + 1]);
    /// let error = over.validate_with_limit("photo", &["image/"], MAX_PHOTO_BYTES).unwrap_err();
    /// assert!(error.to_string().contains(&MAX_PHOTO_BYTES.to_string()));
    ///
    /// // The same bytes are fine as a document.
    /// assert!(over.validate_with_limit("document", &[], MAX_UPLOAD_BYTES).is_ok());
    /// ```
    pub fn validate_with_limit(
        &self,
        field: &'static str,
        families: &[&str],
        max_bytes: usize,
    ) -> Result<()> {
        validate_filename(&self.filename)?;
        if !is_valid_mime(&self.mime) {
            return Err(Error::InvalidRequest {
                field: "mime",
                reason: format!("`{}` is not a type/subtype MIME type", self.mime),
            });
        }
        if self.bytes.is_empty() {
            return Err(Error::InvalidRequest {
                field: "file",
                reason: "must not be empty".to_string(),
            });
        }
        if self.bytes.len() > max_bytes {
            return Err(Error::InvalidRequest {
                field: "file",
                reason: format!(
                    "must be at most {max_bytes} bytes for this upload, got {}",
                    self.bytes.len()
                ),
            });
        }
        if families.is_empty() || families.iter().any(|family| self.mime.starts_with(family)) {
            return Ok(());
        }
        Err(Error::InvalidRequest {
            field,
            reason: format!(
                "`{}` cannot be used here; expected a file of type {}",
                self.mime,
                families.join(" or ")
            ),
        })
    }

    /// Checks the file and that its MIME type belongs to one of `families`.
    ///
    /// An empty `families` slice accepts any type — right for `sendDocument`,
    /// which carries arbitrary files. The size ceiling is the general
    /// [`MAX_UPLOAD_BYTES`]; use [`Self::validate_with_limit`] where the
    /// operation caps photos at [`MAX_PHOTO_BYTES`].
    ///
    /// # Errors
    ///
    /// Everything [`Self::validate`] rejects, plus a MIME type outside the
    /// accepted families (an `application/pdf` sent as `sendPhoto`, say).
    ///
    /// ```rust
    /// use telegram_api_client::UploadFile;
    ///
    /// let pdf = UploadFile::new("a.pdf", vec![1]);
    /// assert!(pdf.validate_for("photo", &["image/"]).is_err());
    /// assert!(pdf.validate_for("document", &[]).is_ok());
    ///
    /// let png = UploadFile::new("a.png", vec![1]);
    /// assert!(png.validate_for("photo", &["image/"]).is_ok());
    /// ```
    pub fn validate_for(&self, field: &'static str, families: &[&str]) -> Result<()> {
        self.validate_with_limit(field, families, MAX_UPLOAD_BYTES)
    }

    /// Converts to the transport-level part named `field`.
    ///
    /// ```rust
    /// use telegram_api_client::UploadFile;
    ///
    /// let part = UploadFile::new("a.png", vec![1, 2]).to_part("photo");
    /// assert_eq!(part.field, "photo");
    /// assert_eq!(part.mime, "image/png");
    /// ```
    #[must_use]
    pub fn to_part(&self, field: &str) -> MultipartFile {
        MultipartFile::new(
            field,
            self.filename.clone(),
            self.mime.clone(),
            self.bytes.clone(),
        )
    }
}

/// Byte counts are noisy in `Debug`; the length is what matters when a test
/// fails on a mismatch.
impl std::fmt::Debug for UploadFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UploadFile")
            .field("filename", &self.filename)
            .field("mime", &self.mime)
            .field("bytes", &format_args!("{} bytes", self.bytes.len()))
            .finish()
    }
}

/// Rejects file names that would break out of a `Content-Disposition` header.
fn validate_filename(filename: &str) -> Result<()> {
    if filename.trim().is_empty() {
        return Err(Error::InvalidRequest {
            field: "file",
            reason: "file name must not be empty".to_string(),
        });
    }
    if filename.len() > MAX_FILENAME_BYTES {
        return Err(Error::InvalidRequest {
            field: "file",
            reason: format!("file name must be at most {MAX_FILENAME_BYTES} bytes"),
        });
    }
    if filename
        .chars()
        .any(|c| c.is_control() || c == '"' || c == '\\')
    {
        return Err(Error::InvalidRequest {
            field: "file",
            reason: "file name must not contain control characters, quotes or backslashes"
                .to_string(),
        });
    }
    Ok(())
}

/// Whether `mime` looks like a `type/subtype` pair of RFC 6838 tokens.
///
/// ```rust
/// use telegram_api_client::is_valid_mime;
///
/// assert!(is_valid_mime("image/png"));
/// assert!(is_valid_mime("application/vnd.api+json"));
/// assert!(!is_valid_mime("png"));
/// assert!(!is_valid_mime("image/"));
/// assert!(!is_valid_mime("image/png/extra"));
/// assert!(!is_valid_mime("image/p ng"));
/// ```
#[must_use]
pub fn is_valid_mime(mime: &str) -> bool {
    let mut parts = mime.split('/');
    let (Some(kind), Some(subtype), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    !kind.is_empty()
        && !subtype.is_empty()
        && kind.chars().all(is_token_char)
        && subtype.chars().all(is_token_char)
}

/// RFC 6838 token characters.
fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || "!#$%&'*+-.^_`|~".contains(c)
}

/// Guesses a MIME type from a file name's extension.
///
/// Unknown or missing extensions fall back to `application/octet-stream`.
///
/// ```rust
/// use telegram_api_client::guess_mime;
///
/// assert_eq!(guess_mime("photo.JPG"), "image/jpeg");
/// assert_eq!(guess_mime("clip.mp4"), "video/mp4");
/// assert_eq!(guess_mime("archive.tar.gz"), "application/gzip");
/// assert_eq!(guess_mime("Makefile"), "application/octet-stream");
/// ```
#[must_use]
pub fn guess_mime(filename: &str) -> &'static str {
    let extension = filename
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase());
    match extension.as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("heic") => "image/heic",
        Some("tgs") => "application/x-tgsticker",
        Some("webm") => "video/webm",
        Some("mp4") => "video/mp4",
        Some("mov") => "video/quicktime",
        Some("m4v") => "video/x-m4v",
        Some("mp3") => "audio/mpeg",
        Some("m4a") => "audio/mp4",
        Some("ogg" | "oga" | "opus") => "audio/ogg",
        Some("wav") => "audio/wav",
        Some("flac") => "audio/flac",
        Some("pdf") => "application/pdf",
        Some("zip") => "application/zip",
        Some("gz") => "application/gzip",
        Some("json") => "application/json",
        Some("csv") => "text/csv",
        Some("txt" | "md") => "text/plain",
        _ => "application/octet-stream",
    }
}

/// Converts a serialized request into a multipart form, dropping the reference
/// field that the file part replaces.
pub(crate) fn form_with_file(body: Value, param: &str, file: &UploadFile) -> MultipartForm {
    let mut form = MultipartForm::new();
    if let Value::Object(map) = body {
        for (key, value) in map {
            if key == param {
                continue;
            }
            form = form.field(key, text_field(&value));
        }
    }
    form.with_file(file.to_part(param))
}

/// Renders a JSON value as the string a multipart field carries.
pub(crate) fn text_field(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// Optional flags shared by every `sendX` method.
///
/// ```
/// use telegram_api_client::{DeliveryOptions, ReplyMarkup};
///
/// let options = DeliveryOptions::default().silent().reply_to(7);
/// let json = serde_json::to_value(&options).unwrap();
/// assert_eq!(json["disable_notification"], true);
/// assert_eq!(json["reply_to_message_id"], 7);
/// assert!(json.get("reply_markup").is_none());
/// assert!(ReplyMarkup::remove() != ReplyMarkup::force_reply());
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct DeliveryOptions {
    /// Send without a notification sound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_notification: Option<bool>,
    /// Prevent forwarding and saving.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protect_content: Option<bool>,
    /// Id of the message to reply to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to_message_id: Option<i64>,
    /// Deliver even when the replied-to message is gone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_sending_without_reply: Option<bool>,
    /// Keyboard or force-reply markup.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_markup: Option<ReplyMarkup>,
    /// Forum topic to post into.
    ///
    /// The id of the topic's opening message. Omit it to post outside a topic;
    /// a value that is set must be positive, which [`Self::validate`] checks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_thread_id: Option<i64>,
}

impl DeliveryOptions {
    /// Send silently.
    #[must_use]
    pub fn silent(mut self) -> Self {
        self.disable_notification = Some(true);
        self
    }

    /// Protect the message from forwarding.
    #[must_use]
    pub fn protected(mut self) -> Self {
        self.protect_content = Some(true);
        self
    }

    /// Reply to `message_id`.
    #[must_use]
    pub fn reply_to(mut self, message_id: i64) -> Self {
        self.reply_to_message_id = Some(message_id);
        self
    }

    /// Attach a keyboard.
    #[must_use]
    pub fn with_reply_markup(mut self, markup: ReplyMarkup) -> Self {
        self.reply_markup = Some(markup);
        self
    }

    /// Checks the delivery flags.
    ///
    /// `disable_notification`, `protect_content`, `allow_sending_without_reply`
    /// and `reply_markup` have no value a pre-flight check could reject.
    /// `message_thread_id` does: it is a message id, so zero and negative values
    /// name no forum topic and are refused.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] on `message_thread_id` when it is set to a
    /// non-positive id.
    ///
    /// ```
    /// use telegram_api_client::{DeliveryOptions, Error};
    ///
    /// // Not posting into a topic: nothing to check.
    /// assert!(DeliveryOptions::default().validate().is_ok());
    ///
    /// // A topic's opening message id.
    /// let mut options = DeliveryOptions::default();
    /// options.message_thread_id = Some(7);
    /// assert!(options.validate().is_ok());
    ///
    /// options.message_thread_id = Some(0);
    /// assert!(matches!(
    ///     options.validate(),
    ///     Err(Error::InvalidRequest { field: "message_thread_id", .. })
    /// ));
    /// ```
    pub fn validate(&self) -> Result<()> {
        validate_thread("message_thread_id", self.message_thread_id)
    }
}

/// Optional flags for text messages.
///
/// ```
/// use telegram_api_client::{MessageOptions, ParseMode};
///
/// let options = MessageOptions::default().with_parse_mode(ParseMode::Html).silent();
/// let json = serde_json::to_value(&options).unwrap();
/// assert_eq!(json["parse_mode"], "HTML");
/// assert_eq!(json["disable_notification"], true);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct MessageOptions {
    /// How Telegram should parse the text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parse_mode: Option<ParseMode>,
    /// Pre-computed formatting entities.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entities: Vec<MessageEntity>,
    /// Link preview behaviour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_preview_options: Option<LinkPreviewOptions>,
    /// Delivery flags (notification, reply, keyboard, thread).
    #[serde(flatten)]
    pub delivery: DeliveryOptions,
}

impl MessageOptions {
    /// Sets the parse mode.
    #[must_use]
    pub fn with_parse_mode(mut self, mode: ParseMode) -> Self {
        self.parse_mode = Some(mode);
        self
    }

    /// Attaches formatting entities.
    #[must_use]
    pub fn with_entities(mut self, entities: Vec<MessageEntity>) -> Self {
        self.entities = entities;
        self
    }

    /// Overrides link previews.
    #[must_use]
    pub fn with_link_preview(mut self, options: LinkPreviewOptions) -> Self {
        self.link_preview_options = Some(options);
        self
    }

    /// Send silently.
    #[must_use]
    pub fn silent(mut self) -> Self {
        self.delivery = self.delivery.silent();
        self
    }

    /// Reply to `message_id`.
    #[must_use]
    pub fn reply_to(mut self, message_id: i64) -> Self {
        self.delivery = self.delivery.reply_to(message_id);
        self
    }

    /// Attach a keyboard.
    #[must_use]
    pub fn with_reply_markup(mut self, markup: ReplyMarkup) -> Self {
        self.delivery = self.delivery.with_reply_markup(markup);
        self
    }

    /// Checks the delivery flags carried alongside the text.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] on `message_thread_id` when it is set to a
    /// non-positive id.
    ///
    /// ```
    /// use telegram_api_client::{Error, MessageOptions};
    ///
    /// let mut options = MessageOptions::default();
    /// options.delivery.message_thread_id = Some(1);
    /// assert!(options.validate().is_ok());
    ///
    /// options.delivery.message_thread_id = Some(-1);
    /// assert!(matches!(
    ///     options.validate(),
    ///     Err(Error::InvalidRequest { field: "message_thread_id", .. })
    /// ));
    /// ```
    pub fn validate(&self) -> Result<()> {
        self.delivery.validate()
    }
}

/// Optional flags for caption-bearing media.
///
/// ```
/// use telegram_api_client::{MediaOptions, ParseMode};
///
/// let options = MediaOptions::default()
///     .with_caption("hi")
///     .with_parse_mode(ParseMode::MarkdownV2);
/// let json = serde_json::to_value(&options).unwrap();
/// assert_eq!(json["caption"], "hi");
/// assert_eq!(json["parse_mode"], "MarkdownV2");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct MediaOptions {
    /// Caption shown with the media.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    /// How Telegram should parse the caption.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parse_mode: Option<ParseMode>,
    /// Pre-computed caption entities.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub caption_entities: Vec<MessageEntity>,
    /// Put the caption above the media.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_caption_above_media: Option<bool>,
    /// Delivery flags (notification, reply, keyboard, thread).
    #[serde(flatten)]
    pub delivery: DeliveryOptions,
}

impl MediaOptions {
    /// Sets the caption.
    #[must_use]
    pub fn with_caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = Some(caption.into());
        self
    }

    /// Sets the caption parse mode.
    #[must_use]
    pub fn with_parse_mode(mut self, mode: ParseMode) -> Self {
        self.parse_mode = Some(mode);
        self
    }

    /// Send silently.
    #[must_use]
    pub fn silent(mut self) -> Self {
        self.delivery = self.delivery.silent();
        self
    }

    /// Reply to `message_id`.
    #[must_use]
    pub fn reply_to(mut self, message_id: i64) -> Self {
        self.delivery = self.delivery.reply_to(message_id);
        self
    }

    /// Attach a keyboard.
    #[must_use]
    pub fn with_reply_markup(mut self, markup: ReplyMarkup) -> Self {
        self.delivery = self.delivery.with_reply_markup(markup);
        self
    }

    /// Checks the caption length (and rejects an explicitly empty caption), and
    /// the delivery flags.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] when the caption is empty or longer than
    /// [`MAX_CAPTION`], or when `message_thread_id` is set to a non-positive id.
    pub fn validate(&self) -> Result<()> {
        self.delivery.validate()?;
        match self.caption.as_deref() {
            None => Ok(()),
            Some(caption) => validate_text("caption", caption, MAX_CAPTION),
        }
    }
}

/// `sendMessage` body.
///
/// ```
/// use telegram_api_client::{ParseMode, SendMessage};
///
/// let request = SendMessage::new(42, "Hallo")
///     .with_parse_mode(ParseMode::Html)
///     .silent();
/// let json = serde_json::to_value(&request).unwrap();
/// assert_eq!(json["chat_id"], 42);
/// assert_eq!(json["parse_mode"], "HTML");
/// assert_eq!(json["disable_notification"], true);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct SendMessage {
    /// Target chat.
    pub chat_id: ChatId,
    /// Message text.
    pub text: String,
    /// Text flags.
    #[serde(flatten)]
    pub options: MessageOptions,
}

impl SendMessage {
    /// A plain text message.
    #[must_use]
    pub fn new(chat_id: impl Into<ChatId>, text: impl Into<String>) -> Self {
        Self {
            chat_id: chat_id.into(),
            text: text.into(),
            options: MessageOptions::default(),
        }
    }

    /// Sets the parse mode.
    #[must_use]
    pub fn with_parse_mode(mut self, mode: ParseMode) -> Self {
        self.options = self.options.with_parse_mode(mode);
        self
    }

    /// Attaches formatting entities.
    #[must_use]
    pub fn with_entities(mut self, entities: Vec<MessageEntity>) -> Self {
        self.options = self.options.with_entities(entities);
        self
    }

    /// Overrides link previews.
    #[must_use]
    pub fn with_link_preview(mut self, options: LinkPreviewOptions) -> Self {
        self.options = self.options.with_link_preview(options);
        self
    }

    /// Send silently.
    #[must_use]
    pub fn silent(mut self) -> Self {
        self.options = self.options.silent();
        self
    }

    /// Reply to `message_id`.
    #[must_use]
    pub fn reply_to(mut self, message_id: i64) -> Self {
        self.options = self.options.reply_to(message_id);
        self
    }

    /// Attach a keyboard.
    #[must_use]
    pub fn with_reply_markup(mut self, markup: ReplyMarkup) -> Self {
        self.options = self.options.with_reply_markup(markup);
        self
    }

    /// Checks chat, text and delivery flags.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an unusable chat, empty text, text over
    /// [`MAX_MESSAGE_TEXT`], or a zero or negative `message_thread_id`.
    pub fn validate(&self) -> Result<()> {
        validate_chat("chat_id", &self.chat_id)?;
        validate_text("text", &self.text, MAX_MESSAGE_TEXT)?;
        self.options.validate()
    }
}

macro_rules! media_send {
    (
        $(#[$meta:meta])*
        pub struct $name:ident {
            $(#[$fmeta:meta])*
            pub $field:ident: String,
        }
        $doc:literal
        accepts: $families:expr,
        limit: $limit:expr
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
        pub struct $name {
            /// Target chat.
            pub chat_id: ChatId,
            $(#[$fmeta])*
            pub $field: String,
            /// Bytes to upload instead of referencing the media remotely.
            ///
            /// Skipped on the wire: an upload turns the whole request into a
            /// `multipart/form-data` body.
            #[serde(skip)]
            pub upload: Option<UploadFile>,
            /// Caption and delivery flags.
            #[serde(flatten)]
            pub options: MediaOptions,
        }

        impl $name {
            #[doc = $doc]
            #[must_use]
            pub fn new(chat_id: impl Into<ChatId>, $field: impl Into<String>) -> Self {
                Self {
                    chat_id: chat_id.into(),
                    $field: $field.into(),
                    upload: None,
                    options: MediaOptions::default(),
                }
            }

            /// Sets the caption.
            #[must_use]
            pub fn with_caption(mut self, caption: impl Into<String>) -> Self {
                self.options = self.options.with_caption(caption);
                self
            }

            /// Sets the caption parse mode.
            #[must_use]
            pub fn with_parse_mode(mut self, mode: ParseMode) -> Self {
                self.options = self.options.with_parse_mode(mode);
                self
            }

            /// Send silently.
            #[must_use]
            pub fn silent(mut self) -> Self {
                self.options = self.options.silent();
                self
            }

            /// Reply to `message_id`.
            #[must_use]
            pub fn reply_to(mut self, message_id: i64) -> Self {
                self.options = self.options.reply_to(message_id);
                self
            }

            /// Attach a keyboard.
            #[must_use]
            pub fn with_reply_markup(mut self, markup: ReplyMarkup) -> Self {
                self.options = self.options.with_reply_markup(markup);
                self
            }

            /// Uploads local bytes instead of referencing a `file_id` or URL.
            ///
            /// The file becomes a multipart part named after the Bot API
            /// parameter, and the reference field is rewritten to the matching
            /// `attach://` placeholder.
            #[doc = concat!(
                "```\n",
                "use telegram_api_client::{", stringify!($name), ", UploadFile};\n",
                "\n",
                "let request = ", stringify!($name), "::new(1, \"file-id\")\n",
                "    .with_upload(UploadFile::new(\"a.bin\", vec![1, 2, 3]));\n",
                "let form = request.multipart_form().unwrap();\n",
                "assert_eq!(form.files()[0].field, \"", stringify!($field), "\");\n",
                "```\n",
            )]
            #[must_use]
            pub fn with_upload(mut self, upload: UploadFile) -> Self {
                self.upload = Some(upload);
                self.$field = format!("attach://{}", stringify!($field));
                self
            }

            /// The multipart body for this request, or `None` when no upload was
            /// configured and the request is plain JSON.
            ///
            /// ```no_run
            /// # use telegram_api_client::UploadFile;
            /// # use telegram_api_client::SendPhoto;
            /// # fn demo() {
            /// let upload = UploadFile::from_path("photo.png").unwrap();
            /// let request = SendPhoto::new(1, "ignored").with_upload(upload);
            /// assert!(request.multipart_form().is_some());
            ///
            /// let json_only = SendPhoto::new(1, "AgACAgI");
            /// assert!(json_only.multipart_form().is_none());
            /// # }
            /// ```
            #[must_use]
            pub fn multipart_form(&self) -> Option<MultipartForm> {
                let upload = self.upload.as_ref()?;
                let body = serde_json::to_value(self).ok()?;
                Some(form_with_file(body, stringify!($field), upload))
            }

            /// Checks chat, media reference or upload, caption and delivery
            /// flags.
            ///
            /// # Errors
            ///
            /// [`Error::InvalidRequest`] for an unusable chat, an empty media
            /// reference, an upload that is empty, too large, misnamed or of the
            /// wrong MIME family, a caption that is empty or too long, or a zero
            /// or negative `message_thread_id`.
            pub fn validate(&self) -> Result<()> {
                validate_chat("chat_id", &self.chat_id)?;
                if self.$field.trim().is_empty() {
                    return Err(Error::InvalidRequest {
                        field: stringify!($field),
                        reason: "must not be empty".to_string(),
                    });
                }
                if let Some(upload) = &self.upload {
                    upload.validate_with_limit(stringify!($field), $families, $limit)?;
                }
                self.options.validate()
            }
        }
    };
}

media_send! {
    /// `sendPhoto` body.
    ///
    /// ```
    /// use telegram_api_client::SendPhoto;
    ///
    /// let request = SendPhoto::new(-1001, "AgACAgIAAxkBAAI").with_caption("sky");
    /// let json = serde_json::to_value(&request).unwrap();
    /// assert_eq!(json["photo"], "AgACAgIAAxkBAAI");
    /// assert_eq!(json["caption"], "sky");
    /// ```
    pub struct SendPhoto {
        /// `file_id` or HTTP URL of the photo.
        pub photo: String,
    }
    "A photo, referenced by `file_id` or URL."
    accepts: &["image/"],
    limit: MAX_PHOTO_BYTES
}

media_send! {
    /// `sendDocument` body.
    ///
    /// ```
    /// use telegram_api_client::SendDocument;
    ///
    /// let json = serde_json::to_value(SendDocument::new(1, "BQACAgI")).unwrap();
    /// assert_eq!(json["document"], "BQACAgI");
    /// ```
    pub struct SendDocument {
        /// `file_id` or HTTP URL of the document.
        pub document: String,
    }
    "A document, referenced by `file_id` or URL."
    accepts: &[],
    limit: MAX_UPLOAD_BYTES
}

media_send! {
    /// `sendAudio` body.
    ///
    /// ```
    /// use telegram_api_client::SendAudio;
    ///
    /// let json = serde_json::to_value(SendAudio::new(1, "CQACAgI")).unwrap();
    /// assert_eq!(json["audio"], "CQACAgI");
    /// ```
    pub struct SendAudio {
        /// `file_id` or HTTP URL of the audio file.
        pub audio: String,
    }
    "An audio track, referenced by `file_id` or URL."
    accepts: &["audio/"],
    limit: MAX_UPLOAD_BYTES
}

media_send! {
    /// `sendVideo` body.
    ///
    /// ```
    /// use telegram_api_client::SendVideo;
    ///
    /// let json = serde_json::to_value(SendVideo::new(1, "DQACAgI")).unwrap();
    /// assert_eq!(json["video"], "DQACAgI");
    /// ```
    pub struct SendVideo {
        /// `file_id` or HTTP URL of the video.
        pub video: String,
    }
    "A video, referenced by `file_id` or URL."
    accepts: &["video/"],
    limit: MAX_UPLOAD_BYTES
}

media_send! {
    /// `sendAnimation` body.
    ///
    /// ```
    /// use telegram_api_client::SendAnimation;
    ///
    /// let json = serde_json::to_value(SendAnimation::new(1, "EQACAgI")).unwrap();
    /// assert_eq!(json["animation"], "EQACAgI");
    /// ```
    pub struct SendAnimation {
        /// `file_id` or HTTP URL of the animation.
        pub animation: String,
    }
    "An animation (GIF or soundless MP4), referenced by `file_id` or URL."
    accepts: &["image/", "video/"],
    limit: MAX_UPLOAD_BYTES
}

media_send! {
    /// `sendVoice` body.
    ///
    /// ```
    /// use telegram_api_client::SendVoice;
    ///
    /// let json = serde_json::to_value(SendVoice::new(1, "FQACAgI")).unwrap();
    /// assert_eq!(json["voice"], "FQACAgI");
    /// ```
    pub struct SendVoice {
        /// `file_id` or HTTP URL of the voice note.
        pub voice: String,
    }
    "A voice note, referenced by `file_id` or URL."
    accepts: &["audio/"],
    limit: MAX_UPLOAD_BYTES
}

/// `sendMediaGroup` body — an album of 2 to 10 items.
///
/// ```
/// use telegram_api_client::{InputMedia, SendMediaGroup};
///
/// let album = SendMediaGroup::new(-1001, vec![
///     InputMedia::photo("a").with_caption("first"),
///     InputMedia::photo("b"),
/// ]);
/// let json = serde_json::to_value(&album).unwrap();
/// assert_eq!(json["media"][0]["type"], "photo");
/// assert_eq!(json["media"][1]["media"], "b");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct SendMediaGroup {
    /// Target chat.
    pub chat_id: ChatId,
    /// Album items; 2 to 10 of them.
    pub media: Vec<InputMedia>,
    /// Delivery flags.
    #[serde(flatten)]
    pub delivery: DeliveryOptions,
}

impl SendMediaGroup {
    /// An album.
    #[must_use]
    pub fn new(chat_id: impl Into<ChatId>, media: Vec<InputMedia>) -> Self {
        Self {
            chat_id: chat_id.into(),
            media,
            delivery: DeliveryOptions::default(),
        }
    }

    /// Send silently.
    #[must_use]
    pub fn silent(mut self) -> Self {
        self.delivery = self.delivery.silent();
        self
    }

    /// Reply to `message_id`.
    #[must_use]
    pub fn reply_to(mut self, message_id: i64) -> Self {
        self.delivery = self.delivery.reply_to(message_id);
        self
    }

    /// Checks chat, album size, uploads, per-item captions and delivery flags.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an unusable chat, an album outside
    /// `2..=10` items, an upload that does not match its item kind, a caption
    /// that is empty or too long, or a zero or negative `message_thread_id`.
    pub fn validate(&self) -> Result<()> {
        validate_chat("chat_id", &self.chat_id)?;
        if self.media.len() < MIN_MEDIA_GROUP || self.media.len() > MAX_MEDIA_GROUP {
            return Err(Error::InvalidRequest {
                field: "media",
                reason: format!(
                    "must contain {MIN_MEDIA_GROUP} to {MAX_MEDIA_GROUP} items, got {}",
                    self.media.len()
                ),
            });
        }
        for item in &self.media {
            if item.media.trim().is_empty() {
                return Err(Error::InvalidRequest {
                    field: "media",
                    reason: "item media reference must not be empty".to_string(),
                });
            }
            if let Some(upload) = &item.upload {
                let families = item.kind.mime_families();
                upload.validate_with_limit("media", families, item.kind.upload_limit_bytes())?;
            }
            if let Some(caption) = item.caption.as_deref() {
                validate_text("caption", caption, MAX_CAPTION)?;
            }
        }
        self.delivery.validate()
    }

    /// The multipart body for this album, or `None` when every item is
    /// referenced by `file_id`/URL.
    ///
    /// Uploaded items are attached as `file0`, `file1`, … and referenced from
    /// the `media` array as `attach://file0`, exactly as the Bot API expects.
    ///
    /// ```rust
    /// use telegram_api_client::{InputMedia, SendMediaGroup, UploadFile};
    ///
    /// let album = SendMediaGroup::new(
    ///     -1001,
    ///     vec![
    ///         InputMedia::photo("AgACAgI"),
    ///         InputMedia::photo("ignored").with_upload(UploadFile::new("b.png", vec![1, 2])),
    ///     ],
    /// );
    /// assert!(album.validate().is_ok());
    ///
    /// let form = album.multipart_form().unwrap();
    /// assert_eq!(form.files().len(), 1);
    /// assert_eq!(form.files()[0].field, "file1");
    /// assert!(form
    ///     .json_fields()["media"]
    ///     .as_str()
    ///     .unwrap()
    ///     .contains("attach://file1"));
    /// ```
    #[must_use]
    pub fn multipart_form(&self) -> Option<MultipartForm> {
        if !self.media.iter().any(|item| item.upload.is_some()) {
            return None;
        }
        let body = serde_json::to_value(self).ok()?;
        let Value::Object(map) = body else {
            return None;
        };

        let mut form = MultipartForm::new();
        for (key, value) in map {
            if key == "media" {
                let Value::Array(items) = value else {
                    form = form.field(key, text_field(&value));
                    continue;
                };
                let mut rewritten = Vec::with_capacity(items.len());
                for (index, item) in items.into_iter().enumerate() {
                    let upload = self
                        .media
                        .get(index)
                        .and_then(|media| media.upload.as_ref());
                    let mut item = item;
                    if let Some(upload) = upload {
                        let field = format!("file{index}");
                        if let Value::Object(ref mut object) = item {
                            object.insert(
                                "media".to_string(),
                                Value::String(format!("attach://{field}")),
                            );
                        }
                        form = form.with_file(upload.to_part(&field));
                    }
                    rewritten.push(item);
                }
                form = form.field(key, Value::Array(rewritten));
                continue;
            }
            form = form.field(key, text_field(&value));
        }
        Some(form)
    }
}

/// `sendLocation` body.
///
/// ```
/// use telegram_api_client::SendLocation;
///
/// let request = SendLocation::new(42, 48.7665, 11.4258);
/// let json = serde_json::to_value(&request).unwrap();
/// assert_eq!(json["latitude"], 48.7665);
/// assert_eq!(json["longitude"], 11.4258);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct SendLocation {
    /// Target chat.
    pub chat_id: ChatId,
    /// Degrees north, `-90.0..=90.0`.
    pub latitude: f64,
    /// Degrees east, `-180.0..=180.0`.
    pub longitude: f64,
    /// Accuracy radius in metres.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horizontal_accuracy: Option<f64>,
    /// Live location period in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live_period: Option<u32>,
    /// Direction of movement, `1..=360`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading: Option<u16>,
    /// Alert radius in metres.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proximity_alert_radius: Option<u32>,
    /// Delivery flags.
    #[serde(flatten)]
    pub delivery: DeliveryOptions,
}

impl SendLocation {
    /// A static location.
    #[must_use]
    pub fn new(chat_id: impl Into<ChatId>, latitude: f64, longitude: f64) -> Self {
        Self {
            chat_id: chat_id.into(),
            latitude,
            longitude,
            ..Self::default()
        }
    }

    /// Send without a notification sound.
    #[must_use]
    pub fn silent(mut self) -> Self {
        self.delivery = self.delivery.silent();
        self
    }

    /// Checks chat, coordinates and delivery flags.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an unusable chat, out-of-range or `NaN`
    /// coordinates, `heading` outside `1..=360`, a zero `live_period`, or a zero
    /// or negative `message_thread_id`.
    pub fn validate(&self) -> Result<()> {
        validate_chat("chat_id", &self.chat_id)?;
        if !(-90.0..=90.0).contains(&self.latitude) {
            return Err(Error::InvalidRequest {
                field: "latitude",
                reason: format!("must be within -90..=90, got {}", self.latitude),
            });
        }
        if !(-180.0..=180.0).contains(&self.longitude) {
            return Err(Error::InvalidRequest {
                field: "longitude",
                reason: format!("must be within -180..=180, got {}", self.longitude),
            });
        }
        if self
            .heading
            .is_some_and(|heading| !(1..=360).contains(&heading))
        {
            return Err(Error::InvalidRequest {
                field: "heading",
                reason: "must be within 1..=360".to_string(),
            });
        }
        if self.live_period == Some(0) {
            return Err(Error::InvalidRequest {
                field: "live_period",
                reason: "must be greater than 0".to_string(),
            });
        }
        self.delivery.validate()
    }
}

/// `sendVenue` body.
///
/// ```
/// use telegram_api_client::SendVenue;
///
/// let request = SendVenue::new(42, 48.7665, 11.4258, "Café", "Am Platz 1");
/// let json = serde_json::to_value(&request).unwrap();
/// assert_eq!(json["title"], "Café");
/// assert_eq!(json["address"], "Am Platz 1");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct SendVenue {
    /// Target chat.
    pub chat_id: ChatId,
    /// Degrees north.
    pub latitude: f64,
    /// Degrees east.
    pub longitude: f64,
    /// Venue name.
    pub title: String,
    /// Street address.
    pub address: String,
    /// Foursquare venue id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foursquare_id: Option<String>,
    /// Foursquare category type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foursquare_type: Option<String>,
    /// Google Places id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub google_place_id: Option<String>,
    /// Google Places type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub google_place_type: Option<String>,
    /// Delivery flags.
    #[serde(flatten)]
    pub delivery: DeliveryOptions,
}

impl SendVenue {
    /// A venue at the given coordinates.
    #[must_use]
    pub fn new(
        chat_id: impl Into<ChatId>,
        latitude: f64,
        longitude: f64,
        title: impl Into<String>,
        address: impl Into<String>,
    ) -> Self {
        Self {
            chat_id: chat_id.into(),
            latitude,
            longitude,
            title: title.into(),
            address: address.into(),
            ..Self::default()
        }
    }

    /// Send without a notification sound.
    #[must_use]
    pub fn silent(mut self) -> Self {
        self.delivery = self.delivery.silent();
        self
    }

    /// Checks chat, coordinates, title, address and delivery flags.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an unusable chat, out-of-range
    /// coordinates, an empty title/address, or a zero or negative
    /// `message_thread_id`.
    pub fn validate(&self) -> Result<()> {
        validate_chat("chat_id", &self.chat_id)?;
        if !(-90.0..=90.0).contains(&self.latitude) || !(-180.0..=180.0).contains(&self.longitude) {
            return Err(Error::InvalidRequest {
                field: "latitude",
                reason: "coordinates are out of range".to_string(),
            });
        }
        validate_text("title", &self.title, MAX_CAPTION)?;
        validate_text("address", &self.address, MAX_CAPTION)?;
        self.delivery.validate()
    }
}

/// `sendContact` body.
///
/// ```
/// use telegram_api_client::SendContact;
///
/// let request = SendContact::new(42, "+4915112345678", "Florian");
/// let json = serde_json::to_value(&request).unwrap();
/// assert_eq!(json["phone_number"], "+4915112345678");
/// assert!(json.get("last_name").is_none());
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct SendContact {
    /// Target chat.
    pub chat_id: ChatId,
    /// Phone number in any format.
    pub phone_number: String,
    /// First name.
    pub first_name: String,
    /// Last name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
    /// Extra vCard data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vcard: Option<String>,
    /// Delivery flags.
    #[serde(flatten)]
    pub delivery: DeliveryOptions,
}

impl SendContact {
    /// A contact card.
    #[must_use]
    pub fn new(
        chat_id: impl Into<ChatId>,
        phone_number: impl Into<String>,
        first_name: impl Into<String>,
    ) -> Self {
        Self {
            chat_id: chat_id.into(),
            phone_number: phone_number.into(),
            first_name: first_name.into(),
            ..Self::default()
        }
    }

    /// Send without a notification sound.
    #[must_use]
    pub fn silent(mut self) -> Self {
        self.delivery = self.delivery.silent();
        self
    }

    /// Checks chat, phone number, name and delivery flags.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an unusable chat, an empty phone
    /// number/first name, or a zero or negative `message_thread_id`.
    pub fn validate(&self) -> Result<()> {
        validate_chat("chat_id", &self.chat_id)?;
        validate_text("phone_number", &self.phone_number, 64)?;
        validate_text("first_name", &self.first_name, 64)?;
        self.delivery.validate()
    }
}

/// `sendPoll` body.
///
/// ```
/// use telegram_api_client::{PollType, SendPoll};
///
/// let poll = SendPoll::quiz(42, "2 + 2?", vec!["3".into(), "4".into()], 1);
/// let json = serde_json::to_value(&poll).unwrap();
/// assert_eq!(json["type"], "quiz");
/// assert_eq!(json["correct_option_id"], 1);
/// assert_eq!(json["options"][0], "3");
///
/// let plain = SendPoll::new(42, "Lunch?", vec!["Pizza".into(), "Salad".into()]);
/// assert_eq!(plain.kind, Some(PollType::Regular));
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct SendPoll {
    /// Target chat.
    pub chat_id: ChatId,
    /// Poll question, up to 300 characters.
    pub question: String,
    /// Answer options, 2 to 12.
    pub options: Vec<String>,
    /// `regular` or `quiz`.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<PollType>,
    /// Hide who voted for what.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_anonymous: Option<bool>,
    /// Allow more than one answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allows_multiple_answers: Option<bool>,
    /// Index of the correct answer (quizzes only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correct_option_id: Option<usize>,
    /// Shown when a user picks a wrong answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
    /// Seconds the poll stays open (5–600), mutually exclusive with `close_date`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_period: Option<u32>,
    /// Unix timestamp at which the poll closes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub close_date: Option<i64>,
    /// Start the poll closed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_closed: Option<bool>,
    /// Delivery flags.
    #[serde(flatten)]
    pub delivery: DeliveryOptions,
}

impl SendPoll {
    /// A regular, anonymous poll.
    #[must_use]
    pub fn new(
        chat_id: impl Into<ChatId>,
        question: impl Into<String>,
        options: Vec<String>,
    ) -> Self {
        Self {
            chat_id: chat_id.into(),
            question: question.into(),
            options,
            // Telegram defaults to `regular`, but being explicit makes the wire
            // body self-describing.
            kind: Some(PollType::Regular),
            ..Self::default()
        }
    }

    /// A quiz poll with `correct_option_id`.
    #[must_use]
    pub fn quiz(
        chat_id: impl Into<ChatId>,
        question: impl Into<String>,
        options: Vec<String>,
        correct_option_id: usize,
    ) -> Self {
        Self {
            kind: Some(PollType::Quiz),
            correct_option_id: Some(correct_option_id),
            is_anonymous: Some(false),
            ..Self::new(chat_id, question, options)
        }
    }

    /// Send without a notification sound.
    #[must_use]
    pub fn silent(mut self) -> Self {
        self.delivery = self.delivery.silent();
        self
    }

    /// Checks chat, question, options, quiz consistency and delivery flags.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an unusable chat, an empty or over-long
    /// question, an option count outside `2..=12`, an empty or over-long
    /// option, a `correct_option_id` outside the option list, both
    /// `open_period` and `close_date` set, `open_period` outside `5..=600`, or a
    /// zero or negative `message_thread_id`.
    pub fn validate(&self) -> Result<()> {
        validate_chat("chat_id", &self.chat_id)?;
        validate_text("question", &self.question, MAX_POLL_QUESTION)?;
        if self.options.len() < MIN_POLL_OPTIONS || self.options.len() > MAX_POLL_OPTIONS {
            return Err(Error::InvalidRequest {
                field: "options",
                reason: format!(
                    "must contain {MIN_POLL_OPTIONS} to {MAX_POLL_OPTIONS} options, got {}",
                    self.options.len()
                ),
            });
        }
        for option in &self.options {
            validate_text("options", option, MAX_POLL_OPTION)?;
        }
        if self
            .correct_option_id
            .is_some_and(|id| id >= self.options.len())
        {
            return Err(Error::InvalidRequest {
                field: "correct_option_id",
                reason: format!("must index one of the {} options", self.options.len()),
            });
        }
        if self.open_period.is_some() && self.close_date.is_some() {
            return Err(Error::InvalidRequest {
                field: "open_period",
                reason: "open_period and close_date are mutually exclusive".to_string(),
            });
        }
        if self
            .open_period
            .is_some_and(|period| !(5..=600).contains(&period))
        {
            return Err(Error::InvalidRequest {
                field: "open_period",
                reason: "must be within 5..=600 seconds".to_string(),
            });
        }
        self.delivery.validate()
    }
}

/// `sendDice` body.
///
/// ```
/// use telegram_api_client::SendDice;
///
/// let json = serde_json::to_value(SendDice::new(42).with_emoji("🎯")).unwrap();
/// assert_eq!(json["emoji"], "🎯");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct SendDice {
    /// Target chat.
    pub chat_id: ChatId,
    /// Emoji deciding the animation (`🎲`, `🎯`, `🏀`, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<String>,
    /// Delivery flags.
    #[serde(flatten)]
    pub delivery: DeliveryOptions,
}

impl SendDice {
    /// A dice with the default emoji.
    #[must_use]
    pub fn new(chat_id: impl Into<ChatId>) -> Self {
        Self {
            chat_id: chat_id.into(),
            ..Self::default()
        }
    }

    /// Picks the animation emoji.
    #[must_use]
    pub fn with_emoji(mut self, emoji: impl Into<String>) -> Self {
        self.emoji = Some(emoji.into());
        self
    }

    /// Send without a notification sound.
    #[must_use]
    pub fn silent(mut self) -> Self {
        self.delivery = self.delivery.silent();
        self
    }

    /// Checks chat, emoji and delivery flags.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an unusable chat, an empty emoji, or a zero
    /// or negative `message_thread_id`.
    pub fn validate(&self) -> Result<()> {
        validate_chat("chat_id", &self.chat_id)?;
        if let Some(emoji) = self.emoji.as_deref() {
            if emoji.trim().is_empty() {
                return Err(Error::InvalidRequest {
                    field: "emoji",
                    reason: "must not be empty".to_string(),
                });
            }
        }
        self.delivery.validate()
    }
}
