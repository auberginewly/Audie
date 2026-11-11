use super::*;

#[test]
fn is_chat_model_keeps_chat_drops_non_chat() {
    // Chat models kept.
    assert!(is_chat_model("gpt-5.2"));
    assert!(is_chat_model("deepseek-chat"));
    assert!(is_chat_model("glm-4.6"));
    assert!(is_chat_model("qwen-plus-latest"));
    // Non-chat noise dropped (OpenAI mixes these into /models).
    assert!(!is_chat_model("text-embedding-3-large"));
    assert!(!is_chat_model("whisper-1"));
    assert!(!is_chat_model("gpt-4o-transcribe"));
    assert!(!is_chat_model("tts-1"));
    assert!(!is_chat_model("dall-e-3"));
}

#[test]
fn local_llm_targets_map_to_picker_card_ids() {
    // The probe's provider ids must match the picker cards (models.ts) so the
    // discovered server lights up the right 本地 card. llamacpp has no card but
    // is still listed under its generic id.
    let ids: Vec<&str> = LOCAL_LLM_TARGETS.iter().map(|(id, _)| *id).collect();
    assert_eq!(ids, ["ollama", "lmstudio", "llamacpp"]);
    // Each base_url joins cleanly to the OpenAI-compatible /models path.
    for (_, base) in LOCAL_LLM_TARGETS {
        assert!(join_url(base, OPENAI_COMPATIBLE_MODELS_PATH).ends_with("/v1/models"));
    }
}
