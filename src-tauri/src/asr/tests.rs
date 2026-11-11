use super::*;

#[test]
fn transcript_delta_can_represent_partial_text() {
    let delta = TranscriptDelta {
        text: "hel".into(),
        is_final: false,
        sequence: 1,
    };

    assert_eq!(delta.text, "hel");
    assert!(!delta.is_final);
    assert_eq!(delta.sequence, 1);
}

#[test]
fn transcript_delta_can_represent_final_text() {
    let delta = TranscriptDelta {
        text: "hello".into(),
        is_final: true,
        sequence: 2,
    };

    assert_eq!(delta.text, "hello");
    assert!(delta.is_final);
    assert_eq!(delta.sequence, 2);
}

#[test]
fn downmix_to_mono_averages_stereo_frames() {
    let mono = downmix_to_mono(&[1.0, -1.0, 0.5, 0.5], 2);
    assert_eq!(mono, vec![0.0, 0.5]);
}

#[test]
fn resample_linear_rejects_zero_source_rate() {
    let err = resample_linear(&[0.0, 1.0], 0, PCM16_TARGET_RATE).unwrap_err();
    assert!(matches!(err, AppError::Device(_)));
}

#[test]
fn pcm16_mono_16k_bytes_downmixes_and_keeps_16k_passthrough() {
    // Already 16k mono → no resample, so the two samples map straight to
    // little-endian i16 (0.0 → 0, 1.0 → i16::MAX = 0x7FFF).
    let audio = AudioData {
        samples: vec![0.0, 1.0],
        sample_rate: PCM16_TARGET_RATE,
        channels: 1,
    };
    let pcm = pcm16_mono_16k_bytes(&audio).expect("encodes");
    assert_eq!(pcm, vec![0, 0, 255, 127]);
}

#[test]
fn pcm16_mono_16k_bytes_resamples_8k_to_16k() {
    let audio = AudioData {
        samples: vec![0.0, 1.0, 0.0, -1.0],
        sample_rate: 8_000,
        channels: 1,
    };
    let pcm = pcm16_mono_16k_bytes(&audio).expect("encodes");
    // 4 samples @ 8k → 8 samples @ 16k → 16 bytes.
    assert_eq!(pcm.len(), 16);
}
