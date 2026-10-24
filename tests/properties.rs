//! Property tests: invariants that must hold for *all* inputs, not just the
//! examples an author thought of.

use proptest::prelude::*;
use serde_json::{Map, Value};
use std::time::Duration;
use telegram_api_client::{
    send::MAX_POLL_OPTION, ChatId, EntityKind, InputMedia, InputMediaKind, MessageEntity,
    ParseMode, PollType, RetryPolicy, SendMessage, SendPoll, Update, UpdateCursor,
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

    /// The retry hint from Telegram wins over our own curve.
    #[test]
    fn retry_hint_wins(retry_after in 0_u64..600, attempt in 0_u8..=8) {
        let policy = RetryPolicy::new(5, Duration::from_millis(10), Duration::from_secs(60));
        let error = telegram_api_client::Error::RateLimited {
            retry_after: Some(retry_after),
            code: 429,
            description: String::new(),
        };
        prop_assert_eq!(
            policy.delay_for_error(attempt.into(), &error),
            Duration::from_secs(retry_after).min(policy.max_delay)
        );
    }

    /// `parse_retry_after` finds exactly the number after the marker and never
    /// invents one otherwise.
    #[test]
    fn parse_retry_after_extracts_the_number(prefix in "[A-Za-z: .]{0,20}", seconds in 1_u64..1_000_000) {
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
}
