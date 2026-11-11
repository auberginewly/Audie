use super::*;

fn recovery_take(mode: DictationMode) -> LastTake {
    LastTake {
        audio: Arc::new(AudioData {
            samples: vec![0.1; 3_200],
            sample_rate: 16_000,
            channels: 1,
        }),
        transcript: Some("口述内容".into()),
        duration_ms: 200,
        generation: 1,
        mode,
        prompt: Some("录音开始时的模板".into()),
        rewrite_source: (mode == DictationMode::Rewrite).then(|| "被选中的原文".into()),
    }
}

mod lifecycle;
mod processing;
mod snapshots;
