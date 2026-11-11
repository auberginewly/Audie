use super::*;

#[test]
fn openai_requires_keychain_secret() {
    let config = TranscriptionConfig {
        asr_provider: "openai".into(),
        asr_model: String::new(),
        openai_api_key: None,
        doubao_endpoint: None,
        doubao_resource_id: None,
        doubao_app_id: None,
        doubao_api_key_or_access_token: None,
        glm_endpoint: None,
        glm_api_key: None,
        aliyun_endpoint: None,
        aliyun_api_key: None,
        stepfun_endpoint: None,
        stepfun_api_key: None,
    };

    let err = match build_provider(&config) {
        Ok(_) => panic!("expected OpenAI without key to fail"),
        Err(err) => err,
    };

    assert!(matches!(err, AppError::Provider(_)));
    assert_eq!(err.message(), "OpenAI API key 未配置，请先到设置页填写");
}

#[test]
fn manager_stream_transcribe_exposes_p2_contract_without_provider_impl() {
    let manager = TranscriptionManager::new();
    let (chunks_tx, chunks_rx) = std::sync::mpsc::channel();
    chunks_tx
        .send(Ok(crate::asr::AudioChunk {
            samples: vec![0.0],
            sample_rate: 16_000,
            channels: 1,
            sequence: 1,
            is_final: true,
        }))
        .unwrap();
    drop(chunks_tx);
    let config = TranscriptionConfig {
        asr_provider: "openai".into(),
        asr_model: String::new(),
        openai_api_key: Some("openai-key".into()),
        doubao_endpoint: None,
        doubao_resource_id: None,
        doubao_app_id: None,
        doubao_api_key_or_access_token: None,
        glm_endpoint: None,
        glm_api_key: None,
        aliyun_endpoint: None,
        aliyun_api_key: None,
        stepfun_endpoint: None,
        stepfun_api_key: None,
    };

    let err = manager.transcribe_stream(chunks_rx, &config).unwrap_err();

    assert!(matches!(err, AppError::Internal(_)));
    assert_eq!(
        err.message(),
        "streaming ASR is not implemented for this provider"
    );
}

#[test]
fn doubao_stream_requires_token_for_streaming_provider() {
    let config = TranscriptionConfig {
        asr_provider: "doubao_stream".into(),
        asr_model: String::new(),
        openai_api_key: None,
        doubao_endpoint: Some("wss://example.test".into()),
        doubao_resource_id: Some("resource".into()),
        doubao_app_id: None,
        doubao_api_key_or_access_token: None,
        glm_endpoint: None,
        glm_api_key: None,
        aliyun_endpoint: None,
        aliyun_api_key: None,
        stepfun_endpoint: None,
        stepfun_api_key: None,
    };

    let err = match build_provider(&config) {
        Ok(_) => panic!("expected Doubao stream without token to fail"),
        Err(err) => err,
    };

    assert!(matches!(err, AppError::Provider(_)));
    assert_eq!(
        err.message(),
        "Doubao API Key / Access Token 未配置，请先到设置页填写"
    );
}
