use super::*;
use std::cell::Cell;

#[test]
fn inline_api_key_is_trimmed() {
    assert_eq!(
        resolve_api_key_for_provider_action(
            Some("  inline-key  "),
            Some("openai_api_key"),
            false,
            |_| panic!("inline key must not check Keychain presence"),
            |_| panic!("inline key must not read Keychain data"),
        )
        .unwrap(),
        "inline-key"
    );
}

#[test]
fn cloud_requires_a_key() {
    // Blank or missing key for a non-local provider is an error.
    assert_eq!(
        resolve_api_key_for_provider_action(
            Some("  "),
            Some("openai_api_key"),
            false,
            |_| Ok(false),
            |_| panic!("missing key must not read Keychain data"),
        )
        .unwrap_err()
        .message(),
        "请先填写 API key"
    );
    assert_eq!(
        resolve_api_key_for_provider_action(
            None,
            Some("openai_api_key"),
            false,
            |_| Ok(false),
            |_| panic!("missing key must not read Keychain data"),
        )
        .unwrap_err()
        .message(),
        "请先填写 API key"
    );
}

#[test]
fn local_endpoint_needs_no_key() {
    // Ollama / LM Studio (localhost) test keyless.
    assert_eq!(
        resolve_api_key_for_provider_action(
            None,
            None,
            true,
            |_| panic!("local provider must not check Keychain presence"),
            |_| panic!("local provider must not read Keychain data"),
        )
        .unwrap(),
        ""
    );
}

#[test]
fn provider_action_prefers_inline_key_without_reading_keychain() {
    let key = resolve_api_key_for_provider_action(
        Some("  typed-key  "),
        Some("openai_api_key"),
        false,
        |_| panic!("inline key must not check Keychain presence"),
        |_| panic!("inline key must not read Keychain data"),
    )
    .unwrap();

    assert_eq!(key, "typed-key");
}

#[test]
fn provider_action_reads_saved_key_only_when_needed() {
    let reads = Cell::new(0);

    let key = resolve_api_key_for_provider_action(
        None,
        Some("openai_api_key"),
        false,
        |_| Ok(true),
        |_| {
            reads.set(reads.get() + 1);
            Ok("saved-key".into())
        },
    )
    .unwrap();

    assert_eq!(key, "saved-key");
    assert_eq!(reads.get(), 1);
}

#[test]
fn local_provider_action_never_reads_keychain() {
    let key = resolve_api_key_for_provider_action(
        None,
        None,
        true,
        |_| panic!("local provider must not check Keychain presence"),
        |_| panic!("local provider must not read Keychain data"),
    )
    .unwrap();

    assert_eq!(key, "");
}

#[test]
fn http_401_maps_to_provider_error() {
    let err = classify_http_status(401, "");

    assert!(matches!(err, AppError::Provider(_)));
    assert!(!err.recoverable());
}

#[test]
fn http_403_maps_to_network_error() {
    let err = classify_http_status(403, "");

    assert!(matches!(err, AppError::Network(_)));
    assert!(err.recoverable());
}

#[test]
fn privacy_http_diagnostics_keep_status_without_body() {
    for (status, category) in [
        (400, "provider"),
        (418, "provider"),
        (401, "provider"),
        (429, "network"),
        (503, "network"),
    ] {
        let error = classify_http_status(status, "PRIVATE_BODY_KEY_PROMPT_SELECTION");
        assert_eq!(error.code(), category);
        assert!(!error.to_string().contains("PRIVATE_BODY"));
        if status == 418 || status == 503 {
            assert!(error.message().contains(&status.to_string()));
        }
    }
}

#[test]
fn privacy_transport_diagnostics_omit_url_credentials_and_query() {
    let raw = reqwest::blocking::Client::new()
        .get("https://private-user:PRIVATE_PASSWORD@example.test/path?token=PRIVATE_QUERY")
        .header("Authorization", "PRIVATE_AUTH\ninvalid")
        .build()
        .unwrap_err();
    let raw = raw.with_url(
        reqwest::Url::parse(
            "https://private-user:PRIVATE_PASSWORD@example.test/?token=PRIVATE_QUERY",
        )
        .unwrap(),
    );
    let error = classify_reqwest_error(raw);
    assert_eq!(error.code(), "network");
    assert!(error.recoverable());
    assert!(!error.to_string().contains("PRIVATE_"));
    assert!(!error.to_string().contains("example.test"));
}
