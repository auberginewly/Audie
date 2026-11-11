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

fn privacy_test_endpoint(status: u16, body: &'static str) -> (String, std::thread::JoinHandle<()>) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!(
        "http://{}/?token=PRIVATE_URL_QUERY",
        listener.local_addr().unwrap()
    );
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0; 4096];
        assert!(stream.read(&mut request).unwrap() > 0);
        write!(stream, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
    });
    (endpoint, server)
}

#[test]
fn privacy_model_list_errors_omit_provider_body_and_url() {
    for (status, body) in [
        (400, "PRIVATE_RESPONSE_AUTH"),
        (200, r#"{"data":"PRIVATE_MALFORMED_MODEL"}"#),
    ] {
        let (endpoint, server) = privacy_test_endpoint(status, body);
        let error = fetch_chat_model_ids(
            &endpoint,
            Some("PRIVATE_KEY"),
            std::time::Duration::from_secs(2),
        )
        .unwrap_err();
        server.join().unwrap();
        assert_eq!(error.code(), "provider");
        assert!(!error.to_string().contains("PRIVATE_"));
    }
}
