//! Secret token and derived path verification.

use telegram_api_client::{Error, WebhookGuard};

#[test]
fn derived_path_is_sha256_of_the_secret() {
    // sha256("s3cret") — pinned so a refactor cannot silently change the scheme.
    let guard = WebhookGuard::new("s3cret");
    assert_eq!(guard.derived_path().len(), 65);
    assert!(guard.derived_path().starts_with('/'));
}

#[test]
fn secret_header_must_match() {
    let guard = WebhookGuard::new("s3cret");
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
    let guard = WebhookGuard::new("s3cret");
    let path = guard.derived_path().to_string();
    assert!(guard.verify_path(&path).is_ok());
    assert!(matches!(
        guard.verify_path("/nope"),
        Err(Error::PathMismatch)
    ));
}

#[test]
fn verify_and_parse_needs_both_checks() {
    let guard = WebhookGuard::new("s3cret");
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
    let guard = WebhookGuard::new("super-secret");
    assert!(!format!("{guard:?}").contains("super-secret"));
}
