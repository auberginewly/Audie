use super::*;

#[test]
fn openai_transcription_config_reads_only_openai_key() {
    let mut requested = Vec::new();
    let settings = settings::Settings {
        asr_provider: "openai".into(),
        ..settings::Settings::default()
    };

    let config = transcription_config_from_settings(&settings, |key_id| {
        requested.push(key_id.to_string());
        Some(format!("{key_id}-value"))
    });

    assert_eq!(requested, vec!["openai_api_key"]);
    assert_eq!(
        config.openai_api_key.as_deref(),
        Some("openai_api_key-value")
    );
}

#[test]
fn glm_transcription_config_reads_only_glm_key() {
    let mut requested = Vec::new();
    let settings = settings::Settings {
        asr_provider: "glm".into(),
        glm_endpoint: "https://glm.example.test/audio/transcriptions".into(),
        ..settings::Settings::default()
    };

    let config = transcription_config_from_settings(&settings, |key_id| {
        requested.push(key_id.to_string());
        Some(format!("{key_id}-value"))
    });

    assert_eq!(requested, vec![crate::asr::glm::SECRET_API_KEY]);
    assert_eq!(
        config.glm_endpoint.as_deref(),
        Some("https://glm.example.test/audio/transcriptions")
    );
    assert_eq!(config.glm_api_key.as_deref(), Some("glm_api_key-value"));
    assert_eq!(config.aliyun_api_key, None);
    assert_eq!(config.stepfun_api_key, None);
}

#[test]
fn aliyun_transcription_config_reads_only_aliyun_key() {
    let mut requested = Vec::new();
    let settings = settings::Settings {
        asr_provider: "aliyun_fun".into(),
        ..settings::Settings::default()
    };

    let config = transcription_config_from_settings(&settings, |key_id| {
        requested.push(key_id.to_string());
        Some(format!("{key_id}-value"))
    });

    assert_eq!(requested, vec![crate::asr::aliyun::config::SECRET_API_KEY]);
    assert_eq!(
        config.aliyun_api_key.as_deref(),
        Some("aliyun_dashscope_api_key-value")
    );
    assert_eq!(
        config.aliyun_endpoint.as_deref(),
        Some(crate::asr::aliyun::config::DEFAULT_ENDPOINT)
    );
    assert_eq!(config.glm_api_key, None);
}

#[test]
fn stepfun_transcription_config_reads_only_stepfun_key() {
    let mut requested = Vec::new();
    let settings = settings::Settings {
        asr_provider: "stepfun".into(),
        ..settings::Settings::default()
    };

    let config = transcription_config_from_settings(&settings, |key_id| {
        requested.push(key_id.to_string());
        Some(format!("{key_id}-value"))
    });

    assert_eq!(requested, vec![crate::asr::stepfun::config::SECRET_API_KEY]);
    assert_eq!(
        config.stepfun_api_key.as_deref(),
        Some("stepfun_api_key-value")
    );
    assert_eq!(
        config.stepfun_endpoint.as_deref(),
        Some(crate::asr::stepfun::config::ENDPOINT)
    );
    assert_eq!(config.glm_api_key, None);
}

#[test]
fn doubao_streaming_config_without_token_returns_none_for_batch_fallback() {
    let mut requested = Vec::new();

    let config = doubao_streaming_config_from_settings(
        "doubao_stream",
        "wss://example.test".into(),
        "resource".into(),
        |key_id| {
            requested.push(key_id.to_string());
            None
        },
    );

    assert!(config.is_none());
    assert_eq!(
        requested,
        vec![doubao_config::SECRET_API_KEY_OR_ACCESS_TOKEN]
    );
}

#[test]
fn doubao_batch_config_keeps_endpoint_and_reads_only_doubao_secrets() {
    let mut requested = Vec::new();
    let settings = settings::Settings {
        asr_provider: "doubao_stream".into(),
        doubao_endpoint: "wss://doubao.example.test".into(),
        doubao_resource_id: "resource".into(),
        ..settings::Settings::default()
    };

    let config = transcription_config_from_settings(&settings, |key_id| {
        requested.push(key_id.to_string());
        Some(format!("{key_id}-value"))
    });

    assert_eq!(
        requested,
        vec![
            doubao_config::SECRET_API_KEY_OR_ACCESS_TOKEN,
            doubao_config::SECRET_APP_ID
        ]
    );
    assert_eq!(
        config.doubao_endpoint.as_deref(),
        Some("wss://doubao.example.test")
    );
    assert_eq!(config.doubao_resource_id.as_deref(), Some("resource"));
    assert_eq!(
        config.doubao_api_key_or_access_token.as_deref(),
        Some("doubao_access_token-value")
    );
}

#[test]
fn doubao_streaming_config_skips_when_provider_not_doubao_stream() {
    let mut requested = Vec::new();

    // A saved token must not activate streaming when the user picked another
    // provider — and we must not even touch the keychain in that case.
    let config = doubao_streaming_config_from_settings(
        "openai",
        "wss://example.test".into(),
        "resource".into(),
        |key_id| {
            requested.push(key_id.to_string());
            Some(format!("{key_id}-value"))
        },
    );

    assert!(config.is_none());
    assert!(requested.is_empty());
}

#[test]
fn doubao_streaming_config_reads_token_then_optional_app_id_by_default() {
    let mut requested = Vec::new();

    let config = doubao_streaming_config_from_settings(
        "doubao_stream",
        "wss://example.test".into(),
        "resource".into(),
        |key_id| {
            requested.push(key_id.to_string());
            Some(format!("{key_id}-value"))
        },
    )
    .expect("token enables streaming config");

    assert_eq!(
        requested,
        vec![
            doubao_config::SECRET_API_KEY_OR_ACCESS_TOKEN,
            doubao_config::SECRET_APP_ID
        ]
    );
    assert_eq!(config.asr_provider, "doubao_stream");
    assert_eq!(
        config.doubao_endpoint.as_deref(),
        Some("wss://example.test")
    );
    assert_eq!(config.doubao_resource_id.as_deref(), Some("resource"));
    assert_eq!(
        config.doubao_api_key_or_access_token.as_deref(),
        Some("doubao_access_token-value")
    );
}
