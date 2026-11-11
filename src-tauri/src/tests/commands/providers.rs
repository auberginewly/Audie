use super::*;

#[test]
fn patch_sets_asr_model_and_empty_clears_to_adapter_default() {
    // Setting a model overrides current; an empty patch value is meaningful
    // (= adapter built-in default), so it clears rather than keeping current.
    let current = Settings {
        asr_model: "gpt-4o-transcribe".into(),
        ..Settings::default()
    };
    let patched = settings_from_patch(
        current.clone(),
        SettingsPatch {
            hotkey: None,
            asr_provider: None,
            asr_model: Some("whisper-large-v3".into()),
            llm_provider: None,
            enhance_enabled: None,
            enhance_prompt: None,
            openai_compatible_base_url: None,
            openai_compatible_model: None,
            llm_api_key_id: None,
            doubao_endpoint: None,
            doubao_resource_id: None,
            glm_endpoint: None,
            aliyun_endpoint: None,
            stepfun_endpoint: None,
            input_device: None,
            onboarding_completed: None,
            onboarding_test_completed: None,
            primary_language: None,
            history_retention: None,
            ui_language: None,
            show_in_dock: None,
            compose_hotkey: None,
            compose_prompt: None,
            rewrite_prompt: None,
            llm_models: None,
        },
    )
    .expect("patch with model");
    assert_eq!(patched.asr_model, "whisper-large-v3");

    let cleared = settings_from_patch(
        current,
        SettingsPatch {
            hotkey: None,
            asr_provider: None,
            asr_model: Some(String::new()),
            llm_provider: None,
            enhance_enabled: None,
            enhance_prompt: None,
            openai_compatible_base_url: None,
            openai_compatible_model: None,
            llm_api_key_id: None,
            doubao_endpoint: None,
            doubao_resource_id: None,
            glm_endpoint: None,
            aliyun_endpoint: None,
            stepfun_endpoint: None,
            input_device: None,
            onboarding_completed: None,
            onboarding_test_completed: None,
            primary_language: None,
            history_retention: None,
            ui_language: None,
            show_in_dock: None,
            compose_hotkey: None,
            compose_prompt: None,
            rewrite_prompt: None,
            llm_models: None,
        },
    )
    .expect("patch clearing model");
    assert_eq!(cleared.asr_model, "");
}

#[test]
fn patch_sets_provider_endpoints_and_blank_keeps_current_values() {
    let current = Settings::default();
    let patched = settings_from_patch(
        current.clone(),
        SettingsPatch {
            hotkey: None,
            asr_provider: None,
            asr_model: None,
            llm_provider: None,
            enhance_enabled: None,
            enhance_prompt: None,
            openai_compatible_base_url: None,
            openai_compatible_model: None,
            llm_api_key_id: None,
            doubao_endpoint: None,
            doubao_resource_id: None,
            glm_endpoint: Some("https://glm.example.test/audio/transcriptions".into()),
            aliyun_endpoint: Some("wss://aliyun.example.test/inference".into()),
            stepfun_endpoint: Some("https://stepfun.example.test/audio/transcriptions".into()),
            input_device: None,
            onboarding_completed: None,
            onboarding_test_completed: None,
            primary_language: None,
            history_retention: None,
            ui_language: None,
            show_in_dock: None,
            compose_hotkey: None,
            compose_prompt: None,
            rewrite_prompt: None,
            llm_models: None,
        },
    )
    .expect("patch endpoints");

    assert_eq!(
        patched.glm_endpoint,
        "https://glm.example.test/audio/transcriptions"
    );
    assert_eq!(
        patched.aliyun_endpoint,
        "wss://aliyun.example.test/inference"
    );
    assert_eq!(
        patched.stepfun_endpoint,
        "https://stepfun.example.test/audio/transcriptions"
    );

    let blank = settings_from_patch(
        current.clone(),
        SettingsPatch {
            hotkey: None,
            asr_provider: None,
            asr_model: None,
            llm_provider: None,
            enhance_enabled: None,
            enhance_prompt: None,
            openai_compatible_base_url: None,
            openai_compatible_model: None,
            llm_api_key_id: None,
            doubao_endpoint: None,
            doubao_resource_id: None,
            glm_endpoint: Some("  ".into()),
            aliyun_endpoint: Some(String::new()),
            stepfun_endpoint: Some("\t".into()),
            input_device: None,
            onboarding_completed: None,
            onboarding_test_completed: None,
            primary_language: None,
            history_retention: None,
            ui_language: None,
            show_in_dock: None,
            compose_hotkey: None,
            compose_prompt: None,
            rewrite_prompt: None,
            llm_models: None,
        },
    )
    .expect("keep endpoints on blank patch");

    assert_eq!(blank.glm_endpoint, current.glm_endpoint);
    assert_eq!(blank.aliyun_endpoint, current.aliyun_endpoint);
    assert_eq!(blank.stepfun_endpoint, current.stepfun_endpoint);
}

#[test]
fn provider_lists_expose_voxt_style_metadata() {
    let asr = available_asr_providers();
    let llm = available_llm_providers();

    assert_eq!(
        asr.iter()
            .map(|provider| provider.id.as_str())
            .collect::<Vec<_>>(),
        ["openai", "glm", "aliyun_fun", "stepfun"]
    );
    assert_eq!(
        llm.iter()
            .map(|provider| provider.id.as_str())
            .collect::<Vec<_>>(),
        ["openai_compatible",]
    );

    let openai_compatible = &llm[0];
    assert_eq!(openai_compatible.title, "OpenAI Compatible");
    assert_eq!(openai_compatible.engine, "Remote LLM");
    assert!(openai_compatible.requires_key);
    assert!(openai_compatible.tags.contains(&"Configurable".to_string()));
}
