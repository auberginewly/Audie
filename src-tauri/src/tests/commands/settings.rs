use super::*;

#[test]
fn default_settings_include_p1_provider_fields() {
    let settings = Settings::default();

    assert_eq!(settings.hotkey, DEFAULT_HOTKEY);
    assert_eq!(settings.asr_provider, DEFAULT_ASR_PROVIDER);
    // Empty = each adapter uses its built-in default model (no behavior change).
    assert_eq!(settings.asr_model, "");
    assert_eq!(settings.llm_provider, "openai_compatible");
    assert!(settings.enhance_enabled);
    assert!(!settings.onboarding_completed);
    assert!(!settings.onboarding_test_completed);
    assert_eq!(settings.history_retention, DEFAULT_HISTORY_RETENTION);
    assert!(!settings.enhance_prompt.trim().is_empty());
    assert_eq!(
        settings.openai_compatible_base_url,
        DEFAULT_OPENAI_COMPATIBLE_BASE_URL
    );
    assert_eq!(
        settings.openai_compatible_model,
        DEFAULT_OPENAI_COMPATIBLE_MODEL
    );
    assert_eq!(
        settings.doubao_endpoint,
        crate::asr::doubao::config::DEFAULT_ENDPOINT
    );
    assert_eq!(
        settings.doubao_resource_id,
        crate::asr::doubao::config::DEFAULT_RESOURCE_ID
    );
    assert_eq!(settings.glm_endpoint, crate::asr::glm::ENDPOINT);
    assert_eq!(
        settings.aliyun_endpoint,
        crate::asr::aliyun::config::DEFAULT_ENDPOINT
    );
    assert_eq!(
        settings.stepfun_endpoint,
        crate::asr::stepfun::config::ENDPOINT
    );
}

#[test]
fn legacy_doubao_resource_id_migrates_to_seed_asr_2_default() {
    assert_eq!(
        normalize_doubao_resource_id(crate::asr::doubao::config::LEGACY_RESOURCE_ID.into()),
        crate::asr::doubao::config::DEFAULT_RESOURCE_ID
    );
}

#[test]
fn settings_round_trip_through_toml() {
    let original = Settings::default();
    let text = toml::to_string_pretty(&original).expect("serialize");
    assert_eq!(
        original,
        toml::from_str::<Settings>(&text).expect("deserialize")
    );

    // A non-empty asr_model is a plain string field (no skip_serializing_if) —
    // it survives the round trip just like asr_provider.
    let with_model = Settings {
        asr_model: "whisper-large-v3".into(),
        ..Settings::default()
    };
    let text = toml::to_string_pretty(&with_model).expect("serialize");
    assert_eq!(
        with_model,
        toml::from_str::<Settings>(&text).expect("deserialize")
    );

    let with_custom_endpoints = Settings {
        glm_endpoint: "https://glm.example.test/audio/transcriptions".into(),
        aliyun_endpoint: "wss://aliyun.example.test/inference".into(),
        stepfun_endpoint: "https://stepfun.example.test/audio/transcriptions".into(),
        ..Settings::default()
    };
    let text = toml::to_string_pretty(&with_custom_endpoints).expect("serialize");
    assert_eq!(
        with_custom_endpoints,
        toml::from_str::<Settings>(&text).expect("deserialize")
    );

    // A populated per-provider model map serializes as the trailing [llm_models]
    // table (it MUST be the last field — a TOML table follows all scalar keys).
    let with_llm_models = Settings {
        llm_models: HashMap::from([
            ("lmstudio".to_string(), "qwen3-14b".to_string()),
            ("deepseek".to_string(), "deepseek-chat".to_string()),
        ]),
        ..Settings::default()
    };
    let text = toml::to_string_pretty(&with_llm_models).expect("serialize");
    assert_eq!(
        with_llm_models,
        toml::from_str::<Settings>(&text).expect("deserialize")
    );
}

#[test]
fn partial_toml_fills_missing_fields_from_default() {
    // A hand-edited file with only a few keys must not error — container
    // serde(default) backfills the rest.
    let parsed: Settings = toml::from_str("hotkey = \"F13\"\nonboarding_completed = true\n")
        .expect("deserialize partial");
    assert_eq!(parsed.hotkey, "F13");
    assert!(parsed.onboarding_completed);
    assert!(!parsed.onboarding_test_completed);
    assert_eq!(parsed.asr_provider, DEFAULT_ASR_PROVIDER);
}

#[test]
fn patch_persists_onboarding_test_completion() {
    let patched = settings_from_patch(
        Settings::default(),
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
            glm_endpoint: None,
            aliyun_endpoint: None,
            stepfun_endpoint: None,
            input_device: None,
            onboarding_completed: None,
            onboarding_test_completed: Some(true),
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
    .expect("patch test completion");

    assert!(patched.onboarding_test_completed);
}

#[test]
fn normalize_clamps_invalid_and_keeps_doubao_stream() {
    let clamped = normalize_settings(Settings {
        asr_provider: "bogus".into(),
        ..Settings::default()
    });
    assert_eq!(clamped.asr_provider, DEFAULT_ASR_PROVIDER);

    let kept = normalize_settings(Settings {
        asr_provider: "doubao_stream".into(),
        ..Settings::default()
    });
    assert_eq!(kept.asr_provider, "doubao_stream");

    let retention = normalize_settings(Settings {
        history_retention: "bogus".into(),
        ..Settings::default()
    });
    assert_eq!(retention.history_retention, DEFAULT_HISTORY_RETENTION);

    let fixed = normalize_settings(Settings {
        openai_compatible_base_url: "  ".into(),
        doubao_resource_id: crate::asr::doubao::config::LEGACY_RESOURCE_ID.into(),
        ..Settings::default()
    });
    assert_eq!(
        fixed.openai_compatible_base_url,
        DEFAULT_OPENAI_COMPATIBLE_BASE_URL
    );
    assert_eq!(
        fixed.doubao_resource_id,
        crate::asr::doubao::config::DEFAULT_RESOURCE_ID
    );

    // An empty LLM model is NOT reset to a default — picking a provider seeds no
    // hardcoded model; the user fetches/types one (build_provider errors if blank).
    let no_model = normalize_settings(Settings {
        openai_compatible_model: "  ".into(),
        ..Settings::default()
    });
    assert_eq!(no_model.openai_compatible_model, "");
}

#[test]
fn legacy_json_migrates_into_settings() {
    // The store wrote a flat JSON object: serde maps known keys, ignores unknown
    // (e.g. the retired doubao_streaming_preview_enabled), defaults the missing.
    let json = r#"{"hotkey":"F13","asr_provider":"doubao_stream","doubao_streaming_preview_enabled":true,"onboarding_completed":true}"#;
    let parsed: Settings = serde_json::from_str(json).expect("deserialize legacy json");
    assert_eq!(parsed.hotkey, "F13");
    assert_eq!(parsed.asr_provider, "doubao_stream");
    assert!(parsed.onboarding_completed);
    assert_eq!(parsed.llm_provider, DEFAULT_LLM_PROVIDER);
}
