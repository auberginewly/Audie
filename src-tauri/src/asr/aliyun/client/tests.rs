use super::*;

#[test]
fn new_falls_back_to_default_model_when_blank() {
    let provider = AliyunProvider::new(config::DEFAULT_ENDPOINT.into(), "key".into(), "  ".into());
    assert_eq!(provider.config.model, config::DEFAULT_MODEL);
    assert_eq!(provider.config.endpoint, config::DEFAULT_ENDPOINT);
}

#[test]
fn new_keeps_explicit_endpoint() {
    let provider = AliyunProvider::new(
        "wss://aliyun.example.test/inference".into(),
        "key".into(),
        config::DEFAULT_MODEL.into(),
    );
    assert_eq!(
        provider.config.endpoint,
        "wss://aliyun.example.test/inference"
    );
}

#[test]
fn new_keeps_explicit_model() {
    let provider = AliyunProvider::new(
        config::DEFAULT_ENDPOINT.into(),
        "key".into(),
        "paraformer-realtime-v2".into(),
    );
    assert_eq!(provider.config.model, "paraformer-realtime-v2");
}

#[test]
fn auth_header_is_bearer_token() {
    let cfg = AliyunStreamConfig {
        endpoint: config::DEFAULT_ENDPOINT.into(),
        api_key: " sk-abc ".into(),
        model: config::DEFAULT_MODEL.into(),
    };
    let (name, value) = auth_header(&cfg);
    assert_eq!(name, "Authorization");
    assert_eq!(value, "Bearer sk-abc");
}

#[test]
fn build_request_sets_authorization_header() {
    let cfg = AliyunStreamConfig {
        endpoint: config::DEFAULT_ENDPOINT.into(),
        api_key: "sk-xyz".into(),
        model: config::DEFAULT_MODEL.into(),
    };
    let request = build_request(&cfg).expect("request builds");
    assert_eq!(request.headers()["Authorization"], "Bearer sk-xyz");
}

#[test]
fn new_task_id_is_32_hex_chars() {
    let id = new_task_id();
    assert_eq!(id.len(), 32);
    assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    assert!(!id.contains('-'));
}

#[test]
fn classify_upgrade_status_maps_auth_vs_network() {
    assert!(matches!(
        classify_upgrade_status(401),
        AppError::Provider(_)
    ));
    assert!(matches!(
        classify_upgrade_status(403),
        AppError::Provider(_)
    ));
    assert!(matches!(classify_upgrade_status(500), AppError::Network(_)));
}

#[test]
fn next_text_frame_unwraps_text_and_ends_on_close() {
    assert!(matches!(
        next_text_frame(Ok(Message::Text("{}".into()))).unwrap(),
        Frame::Text(t) if t == "{}"
    ));
    assert!(matches!(
        next_text_frame(Ok(Message::Close(None))).unwrap(),
        Frame::Closed
    ));
    assert!(matches!(
        next_text_frame(Ok(Message::Ping(vec![]))).unwrap(),
        Frame::Skip
    ));
}

#[test]
fn next_text_frame_maps_transport_error_to_network() {
    match next_text_frame(Err(WsError::ConnectionClosed)) {
        Err(AppError::Network(_)) => {}
        other => panic!("expected Network error, got {:?}", other.map(|_| "frame")),
    }
}
