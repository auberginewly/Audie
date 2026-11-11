use super as config;
use super::*;

#[test]
fn build_request_uses_new_console_api_key_when_app_id_is_blank() {
    let cfg = DoubaoStreamConfig {
        endpoint: config::DEFAULT_ENDPOINT.into(),
        auth: DoubaoAuth::from_settings(" ".into(), "api-key".into()),
        resource_id: config::DEFAULT_RESOURCE_ID.into(),
    };

    let request = build_request(&cfg).expect("request builds");
    let headers = request.headers();

    assert_eq!(headers["X-Api-Key"], "api-key");
    assert!(!headers.contains_key("X-Api-App-Key"));
    assert!(!headers.contains_key("X-Api-Access-Key"));
    assert_eq!(headers["X-Api-Sequence"], "-1");
}

#[test]
fn build_request_uses_old_console_app_id_and_access_token_when_app_id_is_set() {
    let cfg = DoubaoStreamConfig {
        endpoint: config::DEFAULT_ENDPOINT.into(),
        auth: DoubaoAuth::from_settings("app-id".into(), "access-token".into()),
        resource_id: config::DEFAULT_RESOURCE_ID.into(),
    };

    let request = build_request(&cfg).expect("request builds");
    let headers = request.headers();

    assert_eq!(headers["X-Api-App-Key"], "app-id");
    assert_eq!(headers["X-Api-Access-Key"], "access-token");
    assert!(!headers.contains_key("X-Api-Key"));
    assert_eq!(headers["X-Api-Sequence"], "-1");
}
