use super::*;

#[test]
fn openai_compatible_endpoint_joins_base_url_and_models_path() {
    let endpoint = provider_test_endpoint(
        ProviderKind::Llm,
        "openai_compatible",
        Some("https://api.deepseek.com/v1/".into()),
    )
    .unwrap();

    assert_eq!(endpoint, "https://api.deepseek.com/v1/models");
}

#[test]
fn openai_compatible_requires_base_url() {
    let err = provider_test_endpoint(ProviderKind::Llm, "openai_compatible", Some(" ".into()))
        .unwrap_err();

    assert!(matches!(err, AppError::Provider(_)));
    assert_eq!(err.message(), "请填写 OpenAI-compatible base URL");
}
