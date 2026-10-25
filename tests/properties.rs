//! Property tests: invariants that must hold for *all* inputs, not just the
//! examples an author thought of.

use proptest::prelude::*;
use serde_json::{Map, Value};
use std::time::Duration;
use telegram_api_client::{
    guess_mime, is_idempotent, is_valid_mime, send::MAX_POLL_OPTION, ChatId, EntityKind, Error,
    InputMedia, InputMediaKind, MessageEntity, MultipartFile, MultipartForm, ParseMode, PollType,
    RetryPolicy, SendMessage, SendPoll, Update, UpdateCursor,
};

proptest! {
    #![proptest_config(ProptestConfig { cases: 96, ..ProptestConfig::default() })]

    /// Arbitrary Unicode — including NUL, control characters and astral-plane
    /// emoji — must survive JSON serialization byte-for-byte.
    #[test]
    fn text_is_json_escaped_losslessly(text in "\\PC*") {
        let request = SendMessage::new(1_i64, text.clone());
        let body = serde_json::to_value(&request).unwrap();
        prop_assert_eq!(body["text"].as_str(), Some(text.as_str()));

        // And it survives a trip back through the parser.
        let back: SendMessage = serde_json::from_value(body).unwrap();
        prop_assert_eq!(back.text, text);
    }

    /// `parse_mode` is a lossless string wrapper: unknown values are carried.
    #[test]
    fn parse_mode_round_trips(raw in "\\PC*") {
        let mode = ParseMode::from(raw.as_str());
        prop_assert_eq!(mode.as_str(), raw.as_str());
        prop_assert_eq!(ParseMode::from(mode.as_str()), mode.clone());

        let json = serde_json::to_string(&mode).unwrap();
        let back: ParseMode = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(back, mode);
    }

    /// Entity kinds survive both the enum conversion and serde.
    #[test]
    fn entity_round_trips(
        kind in "[a-z_]{0,24}",
        offset in any::<i64>(),
        length in any::<i64>(),
        url in prop::option::of("[ -~]{0,40}"),
    ) {
        let mut entity = MessageEntity::kind(EntityKind::from(kind.as_str()), offset, length);
        entity.url = url;
        let json = serde_json::to_value(&entity).unwrap();
        let back: MessageEntity = serde_json::from_value(json).unwrap();
        prop_assert_eq!(back, entity);
    }

    /// Unknown reply markup shapes are preserved verbatim, keys and all.
    #[test]
    fn unknown_reply_markup_round_trips(keys in prop::collection::vec("[a-z]{1,8}", 0..4)) {
        let mut map = Map::new();
        for key in keys {
            map.insert(key, Value::from(1));
        }
        let value = Value::Object(map);
        let markup: telegram_api_client::ReplyMarkup =
            serde_json::from_value(value.clone()).unwrap();
        prop_assert_eq!(serde_json::to_value(&markup).unwrap(), value);
    }

    /// Numeric and username chat ids both serialize to what the Bot API expects.
    #[test]
    fn chat_id_round_trips(id in any::<i64>(), name in "[A-Za-z0-9_]{1,32}") {
        let numeric = ChatId::id(id);
        prop_assert_eq!(
            serde_json::to_value(&numeric).unwrap(),
            Value::from(id)
        );

        let username = ChatId::username(name.as_str());
        prop_assert_eq!(
            serde_json::to_value(&username).unwrap(),
            Value::from(name.as_str())
        );
        prop_assert_eq!(ChatId::username(format!("@{name}")), username);
    }

    /// The cursor never hands out an id twice and never walks backwards,
    /// whatever mix of duplicates, gaps and re-deliveries the batches contain.
    #[test]
    fn update_cursor_is_exactly_once(
        batches in prop::collection::vec(prop::collection::vec(0_i64..40, 0..8), 0..10)
    ) {
        let mut cursor = UpdateCursor::new();
        let mut seen = std::collections::HashSet::new();
        let mut highest_seen: Option<i64> = None;

        for batch in batches {
            let updates = batch
                .iter()
                .map(|id| Update { update_id: *id, ..Update::default() })
                .collect::<Vec<_>>();
            let fresh = cursor.confirm(&updates);

            for update in &fresh {
                prop_assert!(
                    seen.insert(update.update_id),
                    "id {} was delivered twice",
                    update.update_id
                );
                if let Some(last) = highest_seen {
                    prop_assert!(
                        update.update_id > last,
                        "ids went backwards: {} after {}",
                        update.update_id,
                        last
                    );
                }
                highest_seen = Some(update.update_id);
            }

            // Re-confirming the same batch is always a no-op.
            prop_assert!(cursor.confirm(&updates).is_empty());
            prop_assert_eq!(cursor.next_offset(), highest_seen.map(|id| id + 1));
        }
    }

    /// Backoff never decreases and never exceeds the cap.
    #[test]
    fn retry_delay_is_monotonic_and_capped(
        base_ms in 1_u64..2_000,
        cap_ms in 1_u64..60_000,
    ) {
        let policy = RetryPolicy::new(32, Duration::from_millis(base_ms), Duration::from_millis(cap_ms));
        let mut previous = Duration::ZERO;
        for attempt in 0..40 {
            let delay = policy.delay_for(attempt);
            prop_assert!(delay >= previous, "delay decreased at attempt {attempt}");
            prop_assert!(delay <= policy.max_delay, "delay exceeded cap at attempt {attempt}");
            previous = delay;
        }
    }

    /// The retry hint from Telegram is authoritative: never shortened, and
    /// used to refuse the retry when it is longer than the ceiling.
    #[test]
    fn retry_hint_wins(retry_after in 0_u64..600, attempt in 1_u8..=8) {
        let policy = RetryPolicy::new(5, Duration::from_millis(10), Duration::from_secs(60));
        let error = telegram_api_client::Error::RateLimited {
            retry_after: Some(retry_after),
            code: 429,
            description: String::new(),
        };
        prop_assert_eq!(
            policy.delay_for_error(attempt.into(), &error),
            Duration::from_secs(retry_after),
            "the hint is returned verbatim, never clamped to max_delay"
        );
        // The retry is taken exactly when the hint fits under the ceiling.
        prop_assert_eq!(
            policy.should_retry(1, &error),
            Duration::from_secs(retry_after) <= policy.max_delay
        );
    }

    /// The same rule for the failure shape that carries no phase at all: an
    /// unphased transport error must never be retried on a state-changing
    /// method, whatever the message or the attempt budget.
    #[test]
    fn no_non_idempotent_request_is_retried_after_an_unphased_transport_failure(
        method in "[A-Za-z]{0,24}",
        message in "[A-Za-z0-9 :.]{0,40}",
    ) {
        prop_assume!(!is_idempotent(&method));
        let error = Error::Transport {
            message,
            source: None,
        }
        .classify_for(&method);

        prop_assert!(error.is_ambiguous(), "{method} must be ambiguous");
        prop_assert!(!error.is_retryable());
        prop_assert!(!RetryPolicy::new(100, Duration::from_millis(1), Duration::from_secs(1))
            .should_retry(1, &error));
    }

    /// A failure that provably happened before the write is retryable for every
    /// method, sends included.
    #[test]
    fn a_not_sent_failure_is_retryable_for_every_method(method in "[A-Za-z]{0,24}") {
        let error = Error::not_sent("connect refused").classify_for(&method);
        prop_assert!(error.is_retryable());
        prop_assert!(!error.is_ambiguous());
        prop_assert!(RetryPolicy::default().should_retry(1, &error));
    }

    /// `parse_retry_after` understands every shape Telegram writes, including
    /// the flood-control one, and never invents a number.
    #[test]
    fn parse_retry_after_understands_the_flood_control_shape(
        prefix in "[0-9: .]{0,10}",
        seconds in 1_u64..1_000_000,
        suffix in " ?[A-Za-z]{0,12}",
    ) {
        for marker in ["retry after ", "retry_after ", "retry in "] {
            let description = format!("{prefix}{marker}{seconds}{suffix}");
            prop_assert_eq!(
                telegram_api_client::parse_retry_after(&description),
                Some(seconds),
                "{}",
                description.clone()
            );
            let without_marker = format!("{prefix}too many requests{suffix}");
            prop_assert_eq!(
                telegram_api_client::parse_retry_after(&without_marker),
                None,
                "{}",
                without_marker.clone()
            );
        }
    }

    /// `parse_retry_after` finds exactly the number after the marker and never
    /// invents one otherwise.
    #[test]
    fn parse_retry_after_extracts_the_number(prefix in "[0-9: .]{0,20}", seconds in 1_u64..1_000_000) {
        let description = format!("{prefix}retry after {seconds}");
        prop_assert_eq!(telegram_api_client::parse_retry_after(&description), Some(seconds));

        let without_marker = format!("{prefix}too many requests");
        prop_assert_eq!(telegram_api_client::parse_retry_after(&without_marker), None);
    }

    /// Poll validation accepts exactly the documented window.
    #[test]
    fn poll_validation_matches_the_documented_window(
        options in prop::collection::vec("[A-Za-z0-9 ]{1,120}", 0..14)
    ) {
        let poll = SendPoll::new(1_i64, "q", options.clone());
        let in_window = (2..=12).contains(&options.len())
            && options
                .iter()
                .all(|option| !option.trim().is_empty() && option.encode_utf16().count() <= MAX_POLL_OPTION);
        prop_assert_eq!(poll.validate().is_ok(), in_window);
    }

    /// Media group items keep their unknown fields through a round trip, so a
    /// newer Bot API field cannot be dropped on the way out.
    #[test]
    fn input_media_preserves_extra_fields(
        extra in prop::collection::btree_map("x[a-z]{1,6}", any::<i64>(), 0..4)
    ) {
        let mut media = InputMedia::new(InputMediaKind::from("hologram"), "ref");
        for (key, value) in &extra {
            media.extra.insert(key.clone(), Value::from(*value));
        }
        let json = serde_json::to_value(&media).unwrap();
        for key in extra.keys() {
            prop_assert!(json.get(key).is_some(), "lost {key}");
        }
        let back: InputMedia = serde_json::from_value(json).unwrap();
        prop_assert_eq!(back, media);
    }

    /// Every quiz keeps its correct answer inside the option list.
    #[test]
    fn quiz_correct_option_always_indexes(
        count in 2_usize..=12,
        index in 0_usize..14,
    ) {
        let options = (0..count).map(|i| format!("o{i}")).collect::<Vec<_>>();
        let poll = SendPoll::quiz(1_i64, "q", options, index);
        prop_assert_eq!(poll.validate().is_ok(), index < count);
        prop_assert_eq!(poll.kind, Some(PollType::Quiz));
    }

    // ---- delivery classification -----------------------------------------

    /// The rule the delivery contract rests on: no state-changing method is
    /// ever retried after a 5xx, for any 5xx code and any method name.
    #[test]
    fn no_non_idempotent_request_is_retried_after_a_5xx(
        method in "[A-Za-z]{0,24}",
        code in 500_i64..=599,
    ) {
        prop_assume!(!is_idempotent(&method));
        let error = Error::Api {
            code,
            description: "upstream".to_string(),
        }
        .classify_for(&method);

        prop_assert!(error.is_ambiguous(), "{method} {code} must be ambiguous");
        prop_assert!(!error.is_retryable());
        prop_assert!(!RetryPolicy::default().should_retry(1, &error));
        prop_assert!(!RetryPolicy::new(100, Duration::from_millis(1), Duration::from_secs(1))
            .should_retry(1, &error));
    }

    /// Reads are idempotent, so the same 5xx stays retryable for them.
    #[test]
    fn a_read_stays_retryable_after_a_5xx(
        suffix in "[A-Z][A-Za-z]{0,12}",
        code in 500_i64..=599,
    ) {
        let method = format!("get{suffix}");
        prop_assert!(is_idempotent(&method));
        let error = Error::Api {
            code,
            description: "upstream".to_string(),
        }
        .classify_for(&method);

        prop_assert!(error.is_retryable());
        prop_assert!(!error.is_ambiguous());
    }

    /// A 429 is safe to repeat whatever the method: the server rejected before
    /// processing.
    #[test]
    fn a_429_is_always_retryable(method in "[A-Za-z]{0,24}", hint in any::<Option<u64>>()) {
        let error = Error::RateLimited {
            retry_after: hint,
            code: 429,
            description: "Too Many Requests".to_string(),
        }
        .classify_for(&method);

        prop_assert!(error.is_retryable());
        prop_assert!(!error.is_ambiguous());
        prop_assert_eq!(error.retry_after().is_some(), hint.is_some());
    }

    /// `is_idempotent` is exactly the `get*` family with an uppercase suffix.
    #[test]
    fn idempotency_is_the_get_family(method in "[A-Za-z]{0,10}") {
        let expected = method
            .strip_prefix("get")
            .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_uppercase()));
        prop_assert_eq!(is_idempotent(&method), expected);
    }

    // ---- upload encoders --------------------------------------------------

    /// Guessed MIME types are always syntactically valid, whatever the name.
    #[test]
    fn guessed_mime_types_are_valid(name in "\\PC*") {
        prop_assert!(
            is_valid_mime(guess_mime(&name)),
            "guess for {name:?} was {:?}",
            guess_mime(&name)
        );
    }

    /// A valid MIME type has exactly one slash and no whitespace.
    #[test]
    fn valid_mime_types_have_one_slash(mime in "\\PC*") {
        if is_valid_mime(&mime) {
            prop_assert_eq!(mime.matches('/').count(), 1);
            prop_assert!(!mime.chars().any(char::is_whitespace));
        }
    }

    /// Field values and file bytes survive the multipart encoding byte for byte,
    /// and the payload starts right after the part's blank line.
    #[test]
    fn multipart_round_trips_payloads(
        field_value in "\\PC*",
        filename in "\\PC*",
        bytes in prop::collection::vec(any::<u8>(), 0..64),
    ) {
        let form = MultipartForm::new()
            .field("caption", field_value.clone())
            .with_file(MultipartFile::new(
                "photo",
                filename.clone(),
                "application/octet-stream",
                bytes.clone(),
            ));
        let boundary = form.boundary();
        let body = form.encode(&boundary);

        // One opening delimiter per part, one closing delimiter at the end.
        let opening = format!("--{boundary}\r\n").into_bytes();
        let closing = format!("--{boundary}--\r\n").into_bytes();
        prop_assert_eq!(count(&body, &opening), 2);
        prop_assert!(body.ends_with(&closing));

        // The field payload sits directly after its header terminator.
        let first_header_end = position(&body, b"\r\n\r\n").expect("field header ends");
        let value_start = first_header_end + 4;
        prop_assert_eq!(
            &body[value_start..value_start + field_value.len()],
            field_value.as_bytes()
        );

        // Same for the file payload.
        let file_header_start =
            position(&body[value_start..], &opening).expect("file part starts") + value_start;
        let file_header = &body[file_header_start + opening.len()..];
        let file_header_end = position(file_header, b"\r\n\r\n").expect("file header ends");
        let file_payload = file_header_start + opening.len() + file_header_end + 4;
        prop_assert_eq!(&body[file_payload..file_payload + bytes.len()], &bytes[..]);
    }

    /// A hostile file name cannot add a header line: the header block of a part
    /// always holds exactly its two `CRLF`-terminated header lines.
    #[test]
    fn file_names_cannot_inject_headers(filename in "\\PC*") {
        let form = MultipartForm::new().with_file(MultipartFile::new(
            "photo",
            filename.clone(),
            "image/png",
            vec![1],
        ));
        let boundary = form.boundary();
        let body = form.encode(&boundary);
        let opening = format!("--{boundary}\r\n").into_bytes();
        let start = position(&body, &opening).expect("part starts") + opening.len();
        let header_end = position(&body[start..], b"\r\n\r\n").expect("header ends");
        let header = &body[start..start + header_end];
        let rendered = String::from_utf8_lossy(header).into_owned();

        // Two header lines, so exactly one line break between them: a third
        // would mean a smuggled header.
        prop_assert_eq!(count(header, b"\r\n"), 1, "header block was {}", rendered);
        prop_assert!(!header.starts_with(b"\r\n"));
    }

    /// The boundary is a pure function of the content.
    #[test]
    fn boundaries_are_deterministic(value in "\\PC*") {
        let a = MultipartForm::new().field("caption", value.clone());
        let b = MultipartForm::new().field("caption", value.clone());
        prop_assert_eq!(a.boundary(), b.boundary());
        prop_assert!(a.boundary().starts_with("telegram-api-client-"));
    }
}

/// Byte-level `memchr`-style search, so the property tests can assert on the
/// raw wire format without depending on a parser.
fn position(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn count(haystack: &[u8], needle: &[u8]) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count()
}
