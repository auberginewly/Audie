use super::*;

#[test]
fn joins_chat_completions_endpoint() {
    assert_eq!(
        chat_completions_endpoint("https://api.deepseek.com/v1/"),
        "https://api.deepseek.com/v1/chat/completions"
    );
}

#[test]
fn classifies_openai_compatible_http_errors() {
    assert!(matches!(
        classify_http_status(401, ""),
        AppError::Provider(_)
    ));
    assert!(matches!(
        classify_http_status(403, ""),
        AppError::Network(_)
    ));
    assert!(matches!(
        classify_http_status(429, ""),
        AppError::Network(_)
    ));
    assert!(matches!(
        classify_http_status(500, ""),
        AppError::Network(_)
    ));
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

fn privacy_test_endpoint(status: u16, body: &'static str) -> (String, std::thread::JoinHandle<()>) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!(
        "http://{}/?token=PRIVATE_URL_QUERY",
        listener.local_addr().unwrap()
    );
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0; 4096];
        assert!(stream.read(&mut request).unwrap() > 0);
        write!(stream, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
    });
    (endpoint, server)
}

#[test]
fn privacy_http_response_decode_errors_omit_body_and_url() {
    for (status, body) in [
        (400, "PRIVATE_RESPONSE_PROMPT_SELECTION"),
        (200, r#"{"choices":"PRIVATE_MALFORMED_RESPONSE"}"#),
    ] {
        let (endpoint, server) = privacy_test_endpoint(status, body);
        let provider =
            OpenAiCompatibleProvider::new("PRIVATE_KEY".into(), endpoint, "test-model".into());
        let error = provider
            .enhance("PRIVATE_TRANSCRIPT", "PRIVATE_PROMPT")
            .unwrap_err();
        server.join().unwrap();
        assert_eq!(error.code(), "provider");
        assert!(!error.to_string().contains("PRIVATE_"));
    }
}

#[test]
fn privacy_success_still_returns_enhanced_content() {
    let (endpoint, server) = privacy_test_endpoint(
        200,
        r#"{"choices":[{"message":{"content":"PRIVATE_ENHANCED_TEXT"}}]}"#,
    );
    let provider =
        OpenAiCompatibleProvider::new("PRIVATE_KEY".into(), endpoint, "test-model".into());
    assert_eq!(
        provider
            .enhance("PRIVATE_TRANSCRIPT", "PRIVATE_PROMPT")
            .unwrap(),
        "PRIVATE_ENHANCED_TEXT"
    );
    server.join().unwrap();
}
