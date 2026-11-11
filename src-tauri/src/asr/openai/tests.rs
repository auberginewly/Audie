use super::*;

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
