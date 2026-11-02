//! Callback answers, inline query answers and the chat menu button.

use std::collections::HashSet;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{json, Value};

use crate::error::{Error, Result};
use crate::send::validate_chat;
use crate::types::{ChatId, WebAppInfo};

/// Longest accepted `text` of a callback answer, in characters.
pub const MAX_CALLBACK_TEXT: usize = 200;
/// Most results a single inline query answer may carry.
pub const MAX_INLINE_RESULTS: usize = 50;
/// Longest accepted `id` of an inline query result, in bytes.
///
/// The Bot API calls `id` *"Unique identifier for this result, 1-64 Bytes"*, so
/// the limit counts UTF-8 bytes, not characters.
///
/// ```
/// use telegram_api_client::{InlineQueryResult, MAX_INLINE_RESULT_ID_BYTES};
/// use serde_json::json;
///
/// let id = "a".repeat(MAX_INLINE_RESULT_ID_BYTES);
/// assert!(InlineQueryResult::new(json!({"type": "article", "id": id}))
///     .validate()
///     .is_ok());
///
/// let id = "a".repeat(MAX_INLINE_RESULT_ID_BYTES + 1);
/// assert!(InlineQueryResult::new(json!({"type": "article", "id": id}))
///     .validate()
///     .is_err());
/// ```
pub const MAX_INLINE_RESULT_ID_BYTES: usize = 64;

/// `answerCallbackQuery` body.
///
/// ```
/// use telegram_api_client::AnswerCallbackQuery;
///
/// let answer = AnswerCallbackQuery::new("4382").with_text("Saved").alert();
/// let json = serde_json::to_value(&answer).unwrap();
/// assert_eq!(json["callback_query_id"], "4382");
/// assert_eq!(json["show_alert"], true);
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct AnswerCallbackQuery {
    /// Id from the [`crate::CallbackQuery`] being answered.
    pub callback_query_id: String,
    /// Notification text, up to 200 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Show the text as a modal alert instead of a toast.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_alert: Option<bool>,
    /// URL to open instead of answering (game/URL callbacks).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Seconds the answer may be cached client-side.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_time: Option<u32>,
}

impl AnswerCallbackQuery {
    /// Answers `callback_query_id`.
    #[must_use]
    pub fn new(callback_query_id: impl Into<String>) -> Self {
        Self {
            callback_query_id: callback_query_id.into(),
            ..Self::default()
        }
    }

    /// Sets the notification text.
    #[must_use]
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// Shows the text as a modal alert.
    #[must_use]
    pub fn alert(mut self) -> Self {
        self.show_alert = Some(true);
        self
    }

    /// Checks the query id and the text length.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an empty query id or text longer than
    /// [`MAX_CALLBACK_TEXT`].
    pub fn validate(&self) -> Result<()> {
        if self.callback_query_id.trim().is_empty() {
            return Err(Error::InvalidRequest {
                field: "callback_query_id",
                reason: "must not be empty".to_string(),
            });
        }
        if self
            .text
            .as_ref()
            .is_some_and(|text| text.chars().count() > MAX_CALLBACK_TEXT)
        {
            return Err(Error::InvalidRequest {
                field: "text",
                reason: format!("must be at most {MAX_CALLBACK_TEXT} characters"),
            });
        }
        Ok(())
    }
}

/// One result of an inline query.
///
/// Telegram has ~20 result shapes; this crate forwards them verbatim and gives
/// you a typed constructor for the common "article" one.
///
/// ```
/// use telegram_api_client::InlineQueryResult;
///
/// let article = InlineQueryResult::article("1", "Ping", "Pong");
/// let json = serde_json::to_value(&article).unwrap();
/// assert_eq!(json["type"], "article");
/// assert_eq!(json["input_message_content"]["message_text"], "Pong");
///
/// // Anything else round-trips untouched.
/// let custom: InlineQueryResult =
///     serde_json::from_str(r#"{"type":"gif","id":"9","gif_url":"https://x/y.gif"}"#).unwrap();
/// assert_eq!(custom.value["type"], "gif");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(transparent)]
pub struct InlineQueryResult {
    /// The raw Bot API object.
    pub value: Value,
}

impl InlineQueryResult {
    /// Wraps an arbitrary Bot API result object.
    #[must_use]
    pub fn new(value: Value) -> Self {
        Self { value }
    }

    /// An `article` result that sends `message_text` when chosen.
    #[must_use]
    pub fn article(
        id: impl Into<String>,
        title: impl Into<String>,
        message_text: impl Into<String>,
    ) -> Self {
        Self {
            value: json!({
                "type": "article",
                "id": id.into(),
                "title": title.into(),
                "input_message_content": { "message_text": message_text.into() },
            }),
        }
    }

    /// Checks that this is a result the Bot API will accept.
    ///
    /// The value must be a JSON object with a non-empty string `type` and a
    /// string `id` of 1 to [`MAX_INLINE_RESULT_ID_BYTES`] bytes. Every other
    /// member is left exactly as it is, so a result shape this crate does not
    /// model still passes — including a `type` that does not exist yet.
    ///
    /// ```
    /// use telegram_api_client::InlineQueryResult;
    /// use serde_json::json;
    ///
    /// let custom = InlineQueryResult::new(
    ///     json!({"type": "gif", "id": "9", "gif_url": "https://example.com/y.gif"}),
    /// );
    /// assert!(custom.validate().is_ok());
    ///
    /// // Not an object, no type, an empty id and an overlong id are refused.
    /// for value in [
    ///     json!(null),
    ///     json!({"id": "9"}),
    ///     json!({"type": "gif", "id": ""}),
    ///     json!({"type": "gif", "id": "x".repeat(65)}),
    /// ] {
    ///     assert!(InlineQueryResult::new(value).validate().is_err());
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] naming `results` for a value that is not a JSON
    /// object, and `type` or `id` for the member that is wrong.
    pub fn validate(&self) -> Result<()> {
        check_result(&self.value, "result")
            .map(|_| ())
            .map_err(|(field, reason)| Error::InvalidRequest { field, reason })
    }
}

/// Checks one inline result and returns its `id` on success.
///
/// `path` names the value in the error message: `result` for a single one,
/// `results[3]` inside an answer. The returned field is the wrong member
/// (`type` or `id`), or `results` when the value is not an object at all.
fn check_result<'a>(
    value: &'a Value,
    path: &str,
) -> std::result::Result<&'a str, (&'static str, String)> {
    let Some(object) = value.as_object() else {
        return Err(("results", format!("{path} must be a JSON object")));
    };
    match object.get("type") {
        Some(Value::String(kind)) if !kind.trim().is_empty() => {}
        _ => return Err(("type", format!("{path}.type must be a non-empty string"))),
    }
    let Some(id) = object.get("id").and_then(Value::as_str) else {
        return Err(("id", format!("{path}.id must be a string")));
    };
    if id.is_empty() || id.len() > MAX_INLINE_RESULT_ID_BYTES {
        return Err((
            "id",
            format!(
                "{path}.id must be 1 to {MAX_INLINE_RESULT_ID_BYTES} bytes, got {}",
                id.len()
            ),
        ));
    }
    Ok(id)
}

/// A button shown above the inline results.
///
/// ```
/// use telegram_api_client::InlineQueryResultsButton;
///
/// let button = InlineQueryResultsButton::start("Share", "share_payload");
/// assert_eq!(serde_json::to_value(button).unwrap()["start_parameter"], "share_payload");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct InlineQueryResultsButton {
    /// Label.
    pub text: String,
    /// Deep-linking payload handed to the bot via `/start`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_parameter: Option<String>,
    /// Mini App to open instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_app: Option<WebAppInfo>,
}

impl InlineQueryResultsButton {
    /// A button that starts the bot with `parameter`.
    #[must_use]
    pub fn start(text: impl Into<String>, parameter: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            start_parameter: Some(parameter.into()),
            web_app: None,
        }
    }
}

/// `answerInlineQuery` body.
///
/// ```
/// use telegram_api_client::{AnswerInlineQuery, InlineQueryResult};
///
/// let answer = AnswerInlineQuery::new("q1", vec![InlineQueryResult::article("1", "Ping", "Pong")])
///     .with_cache_time(30);
/// let json = serde_json::to_value(&answer).unwrap();
/// assert_eq!(json["cache_time"], 30);
/// assert_eq!(json["results"][0]["id"], "1");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct AnswerInlineQuery {
    /// Id from the inline query being answered.
    pub inline_query_id: String,
    /// Results to show, 1 to 50.
    pub results: Vec<InlineQueryResult>,
    /// Seconds the client may cache the results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_time: Option<u32>,
    /// Show results tailored to this user only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_personal: Option<bool>,
    /// Cursor for the next page of results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_offset: Option<String>,
    /// Button shown above the results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub button: Option<InlineQueryResultsButton>,
}

impl AnswerInlineQuery {
    /// Answers `inline_query_id` with `results`.
    #[must_use]
    pub fn new(inline_query_id: impl Into<String>, results: Vec<InlineQueryResult>) -> Self {
        Self {
            inline_query_id: inline_query_id.into(),
            results,
            ..Self::default()
        }
    }

    /// Sets the client-side cache time in seconds.
    #[must_use]
    pub fn with_cache_time(mut self, seconds: u32) -> Self {
        self.cache_time = Some(seconds);
        self
    }

    /// Marks the answer as personal.
    #[must_use]
    pub fn personal(mut self) -> Self {
        self.is_personal = Some(true);
        self
    }

    /// Checks the query id, the result count and every result's structure.
    ///
    /// Each result must be a JSON object with a non-empty string `type` and a
    /// unique string `id` of 1 to [`MAX_INLINE_RESULT_ID_BYTES`] bytes; the Bot
    /// API rejects the whole answer otherwise. Nothing is rewritten — a result
    /// carries every other member verbatim.
    ///
    /// ```
    /// use telegram_api_client::{AnswerInlineQuery, InlineQueryResult};
    /// use serde_json::json;
    ///
    /// let good = InlineQueryResult::article("1", "Ping", "Pong");
    /// let answer = AnswerInlineQuery::new("q1", vec![good.clone()]);
    /// assert!(answer.validate().is_ok());
    ///
    /// // A duplicate id, a missing type and an empty id fail before the wire.
    /// for bad in [
    ///     vec![good.clone(), good.clone()],
    ///     vec![InlineQueryResult::new(json!({"id": "2"}))],
    ///     vec![InlineQueryResult::new(json!({"type": "article", "id": ""}))],
    /// ] {
    ///     assert!(AnswerInlineQuery::new("q1", bad).validate().is_err());
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an empty query id, zero results, more than
    /// [`MAX_INLINE_RESULTS`] results, a result that is not a JSON object, a
    /// missing, non-string or blank `type`, and a missing, non-string, empty,
    /// overlong or duplicate `id`.
    pub fn validate(&self) -> Result<()> {
        if self.inline_query_id.trim().is_empty() {
            return Err(Error::InvalidRequest {
                field: "inline_query_id",
                reason: "must not be empty".to_string(),
            });
        }
        if self.results.is_empty() || self.results.len() > MAX_INLINE_RESULTS {
            return Err(Error::InvalidRequest {
                field: "results",
                reason: format!(
                    "must contain 1 to {MAX_INLINE_RESULTS} results, got {}",
                    self.results.len()
                ),
            });
        }
        // The count is bounded, so the set stays small.
        let mut ids = HashSet::with_capacity(self.results.len());
        for (index, result) in self.results.iter().enumerate() {
            let path = format!("results[{index}]");
            let id = check_result(&result.value, &path).map_err(|(_, reason)| {
                Error::InvalidRequest {
                    field: "results",
                    reason,
                }
            })?;
            if !ids.insert(id) {
                return Err(Error::InvalidRequest {
                    field: "results",
                    reason: format!("{path}.id \"{id}\" duplicates an earlier result"),
                });
            }
        }
        Ok(())
    }
}

/// The button in the chat's input field.
///
/// ```
/// use telegram_api_client::MenuButton;
///
/// assert_eq!(
///     serde_json::to_string(&MenuButton::default_menu()).unwrap(),
///     r#"{"type":"default"}"#
/// );
/// assert_eq!(
///     serde_json::to_value(MenuButton::web_app("Open", "https://example.com")).unwrap()["web_app"]["url"],
///     "https://example.com"
/// );
///
/// // Unknown button types survive a round trip.
/// let raw: MenuButton = serde_json::from_str(r#"{"type":"future","x":1}"#).unwrap();
/// assert_eq!(serde_json::to_value(&raw).unwrap()["type"], "future");
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum MenuButton {
    /// Hide the button.
    #[default]
    Default,
    /// Show the bot's command list.
    Commands,
    /// Open a Mini App.
    WebApp {
        /// Button label.
        text: String,
        /// HTTPS URL of the Mini App.
        url: String,
    },
    /// A shape this crate does not model.
    Unknown(Value),
}

impl MenuButton {
    /// Hide the menu button.
    #[must_use]
    pub fn default_menu() -> Self {
        Self::Default
    }

    /// Show the command list.
    #[must_use]
    pub fn commands() -> Self {
        Self::Commands
    }

    /// Open a Mini App.
    #[must_use]
    pub fn web_app(text: impl Into<String>, url: impl Into<String>) -> Self {
        Self::WebApp {
            text: text.into(),
            url: url.into(),
        }
    }
}

impl Serialize for MenuButton {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        match self {
            Self::Default => json!({ "type": "default" }).serialize(serializer),
            Self::Commands => json!({ "type": "commands" }).serialize(serializer),
            Self::WebApp { text, url } => json!({
                "type": "web_app",
                "text": text,
                "web_app": { "url": url },
            })
            .serialize(serializer),
            Self::Unknown(value) => value.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for MenuButton {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        Ok(match value.get("type").and_then(Value::as_str) {
            Some("default") => Self::Default,
            Some("commands") => Self::Commands,
            Some("web_app") => {
                let text = value
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let url = value
                    .get("web_app")
                    .and_then(|web_app| web_app.get("url"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                Self::WebApp { text, url }
            }
            _ => Self::Unknown(value),
        })
    }
}

/// `setChatMenuButton` body.
///
/// ```
/// use telegram_api_client::{MenuButton, SetChatMenuButton};
///
/// let request = SetChatMenuButton {
///     chat_id: Some(42.into()),
///     menu_button: MenuButton::commands(),
/// };
/// let json = serde_json::to_value(&request).unwrap();
/// assert_eq!(json["chat_id"], 42);
/// assert_eq!(json["menu_button"]["type"], "commands");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct SetChatMenuButton {
    /// Chat to change; omit to change the bot's default button.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_id: Option<ChatId>,
    /// The new button.
    #[serde(default)]
    pub menu_button: MenuButton,
}

impl SetChatMenuButton {
    /// Changes the bot-wide default button.
    #[must_use]
    pub fn global(menu_button: MenuButton) -> Self {
        Self {
            chat_id: None,
            menu_button,
        }
    }

    /// Changes the button of one chat.
    #[must_use]
    pub fn for_chat(chat_id: impl Into<ChatId>, menu_button: MenuButton) -> Self {
        Self {
            chat_id: Some(chat_id.into()),
            menu_button,
        }
    }

    /// Checks the optional chat reference.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidRequest`] for an unusable chat reference.
    pub fn validate(&self) -> Result<()> {
        match self.chat_id.as_ref() {
            Some(chat_id) => validate_chat("chat_id", chat_id),
            None => Ok(()),
        }
    }
}
