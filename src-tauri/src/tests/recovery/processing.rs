use super::*;

#[test]
fn recovery_preserves_each_modes_input_prompt_and_history() {
    for mode in [
        DictationMode::Polish,
        DictationMode::Rewrite,
        DictationMode::Compose,
    ] {
        let take = recovery_take(mode);
        let config = EnhanceConfig {
            enhance_enabled: true,
            enhance_prompt: take.prompt.clone().unwrap(),
            ..EnhanceConfig::default()
        };
        let generation = AtomicU64::new(1);
        let mut inserted = String::new();
        let mut history = None;

        let outcome = finish_pipeline_tail(
            &take,
            "口述内容",
            false,
            &config,
            &generation,
            1,
            |input, config| {
                let expected = if mode == DictationMode::Rewrite {
                    "原文：\n被选中的原文\n\n指令：\n口述内容"
                } else {
                    "口述内容"
                };
                assert_eq!(input, expected);
                assert_eq!(config.enhance_prompt, "录音开始时的模板");
                Ok("处理结果".into())
            },
            |text| {
                inserted = text.into();
                Ok(())
            },
            |mode, raw, enhanced, duration| {
                history = Some((mode.to_string(), raw.to_string(), enhanced, duration));
            },
        )
        .unwrap();

        assert_eq!(outcome, Some(EnhanceOutcome::Enhanced));
        assert_eq!(inserted, "处理结果");
        let (recorded_mode, raw, enhanced, duration) = history.unwrap();
        assert_eq!(recorded_mode, mode.as_str());
        assert_eq!(
            raw,
            if mode == DictationMode::Rewrite {
                "指令：口述内容\n\n引用：\n被选中的原文"
            } else {
                "口述内容"
            }
        );
        assert_eq!(enhanced.as_deref(), Some("处理结果"));
        assert_eq!(duration, 200);
    }
}

#[test]
fn recovery_raw_insertion_skips_llm_and_preserves_rewrite_source() {
    for mode in [
        DictationMode::Polish,
        DictationMode::Rewrite,
        DictationMode::Compose,
    ] {
        let take = recovery_take(mode);
        let mut inserted = String::new();
        let outcome = finish_pipeline_tail(
            &take,
            "口述指令",
            true,
            &EnhanceConfig::default(),
            &AtomicU64::new(1),
            1,
            |_, _| panic!("raw insertion must not call the LLM"),
            |text| {
                inserted = text.into();
                Ok(())
            },
            |recorded_mode, _, enhanced, _| {
                assert_eq!(recorded_mode, mode.as_str());
                assert!(enhanced.is_none());
            },
        )
        .unwrap();

        assert_eq!(outcome, Some(EnhanceOutcome::Disabled));
        assert_eq!(
            inserted,
            if mode == DictationMode::Rewrite {
                "被选中的原文"
            } else {
                "口述指令"
            }
        );
    }
}

#[test]
fn recovery_cancel_during_llm_discards_success_and_failure() {
    for fail in [false, true] {
        let generation = AtomicU64::new(1);
        let outcome = finish_pipeline_tail(
            &recovery_take(DictationMode::Rewrite),
            "口述内容",
            false,
            &EnhanceConfig {
                enhance_enabled: true,
                ..EnhanceConfig::default()
            },
            &generation,
            1,
            |_, _| {
                generation.fetch_add(1, Ordering::SeqCst);
                if fail {
                    Err(AppError::Network("delayed failure".into()))
                } else {
                    Ok("delayed result".into())
                }
            },
            |_| panic!("cancelled LLM result must not inject"),
            |_, _, _, _| panic!("cancelled LLM result must not record a success"),
        )
        .unwrap();

        assert_eq!(outcome, None);
    }
}

#[test]
fn recovery_stale_worker_never_calls_provider_or_injector() {
    let outcome = finish_pipeline_tail(
        &recovery_take(DictationMode::Compose),
        "口述内容",
        false,
        &EnhanceConfig {
            enhance_enabled: true,
            ..EnhanceConfig::default()
        },
        &AtomicU64::new(2),
        1,
        |_, _| panic!("stale worker must not call the provider"),
        |_| panic!("stale worker must not inject"),
        |_, _, _, _| panic!("stale worker must not record history"),
    )
    .unwrap();

    assert_eq!(outcome, None);
}

#[test]
fn recovery_rewrite_retry_keeps_source_after_inject_failure() {
    let take = recovery_take(DictationMode::Rewrite);
    let config = EnhanceConfig {
        enhance_enabled: true,
        ..EnhanceConfig::default()
    };
    let generation = AtomicU64::new(1);
    let failed = finish_pipeline_tail(
        &take,
        "口述内容",
        false,
        &config,
        &generation,
        1,
        |_, _| Ok("首次结果".into()),
        |_| Err(AppError::Inject("paste failed".into())),
        |_, _, _, _| panic!("failed injection must not record success"),
    );
    assert!(matches!(failed, Err(AppError::Inject(_))));

    let outcome = finish_pipeline_tail(
        &take,
        "口述内容",
        false,
        &config,
        &generation,
        1,
        |input, _| {
            assert_eq!(input, "原文：\n被选中的原文\n\n指令：\n口述内容");
            Err(AppError::Network("LLM failed on retry".into()))
        },
        |text| {
            assert_eq!(text, "被选中的原文");
            Ok(())
        },
        |mode, raw, enhanced, _| {
            assert_eq!(mode, "rewrite");
            assert!(raw.contains("引用：\n被选中的原文"));
            assert!(enhanced.is_none());
        },
    )
    .unwrap();
    assert_eq!(outcome, Some(EnhanceOutcome::Failed));
}

#[test]
fn recovery_cancel_during_injection_suppresses_old_error_and_history() {
    let generation = AtomicU64::new(1);
    let outcome = finish_pipeline_tail(
        &recovery_take(DictationMode::Polish),
        "口述内容",
        true,
        &EnhanceConfig::default(),
        &generation,
        1,
        |_, _| panic!("raw insertion must not call LLM"),
        |_| {
            generation.fetch_add(1, Ordering::SeqCst);
            Err(AppError::Inject("late paste failure".into()))
        },
        |_, _, _, _| panic!("cancelled injection must not record a success"),
    )
    .unwrap();
    assert_eq!(outcome, None);
}
