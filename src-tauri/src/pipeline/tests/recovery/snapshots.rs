use super::*;

#[test]
fn recovery_uses_original_prompt_for_each_mode_without_a_template_library() {
    let mut settings = settings::Settings {
        enhance_prompt: "original polish".into(),
        rewrite_prompt: "original rewrite".into(),
        compose_prompt: "original compose".into(),
        ..settings::Settings::default()
    };
    for (mode, original) in [
        (DictationMode::Polish, "original polish"),
        (DictationMode::Rewrite, "original rewrite"),
        (DictationMode::Compose, "original compose"),
    ] {
        let snapshot = prompt_from_snapshot(None, &settings, mode);
        assert_eq!(snapshot, original);
        let changed = settings::Settings {
            enhance_prompt: "changed polish".into(),
            rewrite_prompt: "changed rewrite".into(),
            compose_prompt: "changed compose".into(),
            ..settings.clone()
        };
        assert_eq!(
            prompt_from_snapshot(Some(snapshot), &changed, mode),
            original
        );
    }
    settings.enhance_prompt = "updated polish".into();
    assert_eq!(
        prompt_from_snapshot(None, &settings, DictationMode::Polish),
        "updated polish"
    );
}

#[test]
fn recovery_prompt_and_rewrite_source_are_independent_state_types() {
    let prompt = ActivePromptSlot(parking_lot::Mutex::new(Some("fixed prompt".into())));
    let source: RewriteSourceSlot = Arc::new(parking_lot::Mutex::new(Some("selected text".into())));
    assert_ne!(
        std::any::TypeId::of::<ActivePromptSlot>(),
        std::any::TypeId::of::<RewriteSourceSlot>()
    );
    assert_eq!(prompt.0.lock().take().as_deref(), Some("fixed prompt"));
    assert_eq!(source.lock().take().as_deref(), Some("selected text"));
}
