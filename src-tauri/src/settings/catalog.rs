use serde::Serialize;
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct ProviderMetadata {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub engine: String,
    pub default_model: Option<String>,
    pub requires_key: bool,
    pub tags: Vec<String>,
}

fn provider_metadata(
    id: &str,
    title: &str,
    kind: &str,
    engine: &str,
    default_model: Option<&str>,
    requires_key: bool,
    tags: &[&str],
) -> ProviderMetadata {
    ProviderMetadata {
        id: id.to_string(),
        title: title.to_string(),
        kind: kind.to_string(),
        engine: engine.to_string(),
        default_model: default_model.map(str::to_string),
        requires_key,
        tags: tags.iter().map(|tag| (*tag).to_string()).collect(),
    }
}

pub fn available_asr_providers() -> Vec<ProviderMetadata> {
    vec![
        provider_metadata(
            "openai",
            "OpenAI",
            "asr",
            "Remote ASR",
            Some("whisper-1"),
            true,
            &["Remote", "Multilingual"],
        ),
        provider_metadata(
            "glm",
            "智谱 GLM ASR",
            "asr",
            "Remote ASR",
            Some(crate::asr::glm::DEFAULT_MODEL),
            true,
            &["Remote", "中文"],
        ),
        provider_metadata(
            "aliyun_fun",
            "通义 Paraformer ASR",
            "asr",
            "Remote ASR",
            Some(crate::asr::aliyun::config::DEFAULT_MODEL),
            true,
            &["Remote", "实时", "中文"],
        ),
        provider_metadata(
            "stepfun",
            "StepFun ASR",
            "asr",
            "Remote ASR",
            Some(crate::asr::stepfun::config::DEFAULT_MODEL),
            true,
            &["Remote", "中文"],
        ),
    ]
}

pub fn available_llm_providers() -> Vec<ProviderMetadata> {
    vec![provider_metadata(
        "openai_compatible",
        "OpenAI Compatible",
        "llm",
        "Remote LLM",
        None,
        true,
        &["Remote", "Configurable"],
    )]
}
