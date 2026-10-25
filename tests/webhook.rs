//! Secret token and derived path verification.

use telegram_api_client::{Error, WebhookGuard};

#[test]
fn derived_path_is_sha256_of_the_secret() {
    // sha256("s3cret") — pinned so a refactor cannot silently change the scheme.
    let guard = WebhookGuard::new("s3cret").unwrap();
    assert_eq!(guard.derived_path().len(), 65);
    assert!(guard.derived_path().starts_with('/'));
}

#[test]
fn secret_header_must_match() {
    let guard = WebhookGuard::new("s3cret").unwrap();
    assert!(guard.verify_secret(Some("s3cret")).is_ok());
    assert!(matches!(
        guard.verify_secret(Some("wrong")),
        Err(Error::SecretMismatch)
    ));
    assert!(matches!(
        guard.verify_secret(None),
        Err(Error::SecretMismatch)
    ));
}

#[test]
fn path_must_match() {
    let guard = WebhookGuard::new("s3cret").unwrap();
    let path = guard.derived_path().to_string();
    assert!(guard.verify_path(&path).is_ok());
    assert!(matches!(
        guard.verify_path("/nope"),
        Err(Error::PathMismatch)
    ));
}

#[test]
fn verify_and_parse_needs_both_checks() {
    let guard = WebhookGuard::new("s3cret").unwrap();
    let body = br#"{"update_id":1,"message":{"message_id":2,"chat":{"id":-3,"type":"group"},"text":"hi"}}"#;
    let path = guard.derived_path().to_string();
    let update = guard.verify_and_parse(Some("s3cret"), &path, body).unwrap();
    assert_eq!(update.chat_id(), Some(-3));
    assert_eq!(update.text(), Some("hi"));

    assert!(guard.verify_and_parse(Some("nope"), &path, body).is_err());
    assert!(guard
        .verify_and_parse(Some("s3cret"), "/other", body)
        .is_err());
}

#[test]
fn debug_does_not_leak_the_secret() {
    let guard = WebhookGuard::new("super-secret").unwrap();
    assert!(!format!("{guard:?}").contains("super-secret"));
}

#[test]
fn secret_comparison_does_not_depend_on_the_configured_length() {
    // The guard used to `ct_eq` the raw secret against the header, and
    // `subtle::ConstantTimeEq` returns at the length check — an attacker who
    // could time the endpoint learned how long the token is. Both sides are now
    // SHA-256 digests, so every candidate costs the same 32 bytes of comparison
    // whatever its length.
    for secret in ["a", "ab", "abcdefghijklmnopqrstuvwxyz0123456789-_-_-_-"] {
        let guard = WebhookGuard::new(secret).unwrap();
        assert!(
            guard.verify_secret(Some(secret)).is_ok(),
            "the exact secret verifies ({})",
            secret.len()
        );

        // The three shapes a length side channel would have to distinguish:
        // shorter, equal-length-but-wrong, longer.
        let shorter = &secret[..secret.len() - 1];
        assert!(guard.verify_secret(Some(shorter)).is_err(), "prefix");
        let mut same_length = secret.to_string();
        same_length.pop();
        same_length.push(if secret.ends_with('a') { 'b' } else { 'a' });
        assert!(
            guard.verify_secret(Some(&same_length)).is_err(),
            "same length"
        );
        let longer = format!("{secret}x");
        assert!(guard.verify_secret(Some(&longer)).is_err(), "extension");
    }

    // Secrets of very different lengths are indistinguishable by digest size:
    // both guards store exactly one 32-byte digest, so `size_of` does not leak
    // the token either.
    let short = WebhookGuard::new("a").unwrap();
    let long = WebhookGuard::new("a".repeat(256)).unwrap();
    assert_eq!(std::mem::size_of_val(&short), std::mem::size_of_val(&long));
}

#[test]
fn a_guard_only_accepts_secrets_telegram_would_register() {
    // Telegram's documented contract: 1..=256 characters of A-Z a-z 0-9 _ -.
    for accepted in ["a", "aZ0_-", &"a".repeat(256), "0_0-0"] {
        assert!(
            WebhookGuard::new(accepted).is_ok(),
            "{accepted:.8}… should be accepted"
        );
    }
    for rejected in [
        "a".repeat(257),
        "has space".to_string(),
        "ümlaut".to_string(),
        "per.iod".to_string(),
        "new\nline".to_string(),
        "sl/ash".to_string(),
    ] {
        assert!(
            WebhookGuard::new(&rejected).is_err(),
            "{rejected:.8}… is not a token Telegram accepts"
        );
    }

    // Boundary pair on the length limit, stated explicitly.
    assert!(WebhookGuard::new("a".repeat(256)).is_ok());
    assert!(WebhookGuard::new("a".repeat(257)).is_err());

    // Empty stays the dedicated error, not a generic validation failure.
    assert!(matches!(WebhookGuard::new(""), Err(Error::EmptySecret)));
}

#[test]
fn a_guard_and_a_set_webhook_agree_on_what_a_secret_is() {
    use telegram_api_client::SetWebhook;

    // The guard now runs the same validator as `setWebhook`, so the two cannot
    // drift apart again.
    for secret in ["aZ0_-", "a", &"a".repeat(256), "has space", "ümlaut"] {
        let request = SetWebhook::new("https://example.com/hook").with_secret(secret);
        assert_eq!(
            WebhookGuard::new(secret).is_ok(),
            request.validate().is_ok(),
            "the guard and setWebhook disagree about {secret:.8}…"
        );
    }
}
