use super::super::{AudioMeta, MAX_RETAINED_AUDIO_BYTES, MAX_RETAINED_SAMPLES};
use super::*;
use std::sync::atomic::AtomicU64;
use std::sync::{mpsc, Arc};

#[test]
fn process_f32_forwards_chunk_when_sink_present() {
    let accum = Mutex::new(LevelAccum::default());
    let buffer = Mutex::new(Vec::new());
    let retained = RetainedSamples::new(usize::MAX);
    let (tx, rx) = mpsc::sync_channel(1);
    let overflowed = Arc::new(AtomicBool::new(false));
    let sink = ChunkSink {
        tx,
        overflowed,
        sequence: AtomicU64::new(0),
        sample_rate: 16_000,
        channels: 1,
    };

    process_f32(
        &accum,
        &buffer,
        Some(&sink),
        &retained,
        &[0.5, -0.25],
        false,
    );

    // Batch buffer still gets the samples …
    assert_eq!(*buffer.lock(), vec![0.5, -0.25]);
    // … and the same samples arrive as an AudioChunk.
    let chunk = rx.recv().unwrap().unwrap();
    assert_eq!(chunk.samples, vec![0.5, -0.25]);
    assert_eq!(chunk.sequence, 0);
    assert!(!chunk.is_final);
}

#[test]
fn process_f32_without_sink_only_fills_buffer() {
    let accum = Mutex::new(LevelAccum::default());
    let buffer = Mutex::new(Vec::new());
    let retained = RetainedSamples::new(usize::MAX);

    process_f32(&accum, &buffer, None, &retained, &[0.1, 0.2], false);

    assert_eq!(*buffer.lock(), vec![0.1, 0.2]);
}

#[test]
fn process_i16_normalizes_and_forwards() {
    let accum = Mutex::new(LevelAccum::default());
    let buffer = Mutex::new(Vec::new());
    let retained = RetainedSamples::new(usize::MAX);
    let (tx, rx) = mpsc::sync_channel(1);
    let overflowed = Arc::new(AtomicBool::new(false));
    let sink = ChunkSink {
        tx,
        overflowed,
        sequence: AtomicU64::new(0),
        sample_rate: 16_000,
        channels: 1,
    };

    process_i16(
        &accum,
        &buffer,
        Some(&sink),
        &retained,
        &[i16::MAX, 0],
        false,
    );

    let chunk = rx.recv().unwrap().unwrap();
    assert_eq!(chunk.samples, vec![1.0, 0.0]);
    assert_eq!(*buffer.lock(), vec![1.0, 0.0]);
}

#[test]
fn process_f32_level_only_tracks_peak_without_buffering() {
    let accum = Mutex::new(LevelAccum::default());
    let buffer = Mutex::new(Vec::new());
    let retained = RetainedSamples::new(usize::MAX);

    // The Settings mic preview can run for minutes, so it must not retain
    // samples — but it must still update the peak that drives the meter.
    process_f32(&accum, &buffer, None, &retained, &[0.5, -0.25], true);

    assert!(buffer.lock().is_empty());
    assert_eq!(accum.lock().peak, 0.5);
}

#[test]
fn recording_longer_than_120_seconds_is_still_retained() {
    let meta = AudioMeta {
        sample_rate: 48_000,
        channels: 2,
    };
    let samples_at_125_seconds = meta.sample_rate as usize * meta.channels as usize * 125;
    let retained = RetainedSamples::new(MAX_RETAINED_SAMPLES);

    assert_eq!(retained.accepted_len(samples_at_125_seconds, 1_024), 1_024);
    assert!(!retained.exceeded());
}

#[test]
fn retained_sample_limit_equals_512_mib_of_f32_audio() {
    assert_eq!(
        MAX_RETAINED_SAMPLES * std::mem::size_of::<f32>(),
        MAX_RETAINED_AUDIO_BYTES
    );
}

#[test]
fn process_f32_below_memory_fuse_keeps_all_samples() {
    let accum = Mutex::new(LevelAccum::default());
    let buffer = Mutex::new(vec![0.1]);
    let retained = RetainedSamples::new(4);

    process_f32(&accum, &buffer, None, &retained, &[0.2, 0.3], false);

    assert_eq!(*buffer.lock(), vec![0.1, 0.2, 0.3]);
    assert!(!retained.exceeded());
}

#[test]
fn process_f32_caps_batch_buffer_and_sets_overflow() {
    let accum = Mutex::new(LevelAccum::default());
    let buffer = Mutex::new(vec![0.1, 0.2]);
    let retained = RetainedSamples::new(3);

    process_f32(&accum, &buffer, None, &retained, &[0.3, 0.4], false);

    assert_eq!(*buffer.lock(), vec![0.1, 0.2, 0.3]);
    assert!(retained.exceeded());
    assert_eq!(accum.lock().peak, 0.4);
}

#[test]
fn process_i16_caps_forwarded_chunk_to_retained_samples() {
    let accum = Mutex::new(LevelAccum::default());
    let buffer = Mutex::new(vec![0.0]);
    let retained = RetainedSamples::new(2);
    let (tx, rx) = mpsc::sync_channel(1);
    let overflowed = Arc::new(AtomicBool::new(false));
    let sink = ChunkSink {
        tx,
        overflowed,
        sequence: AtomicU64::new(0),
        sample_rate: 16_000,
        channels: 1,
    };

    process_i16(
        &accum,
        &buffer,
        Some(&sink),
        &retained,
        &[i16::MAX, i16::MIN],
        false,
    );

    assert_eq!(*buffer.lock(), vec![0.0, 1.0]);
    assert!(retained.exceeded());
    let chunk = rx.try_recv().unwrap().unwrap();
    assert_eq!(chunk.samples, vec![1.0]);
}

#[test]
fn process_u16_after_cap_tracks_peak_without_growing_buffer() {
    let accum = Mutex::new(LevelAccum::default());
    let buffer = Mutex::new(vec![0.1]);
    let retained = RetainedSamples::new(1);

    process_u16(&accum, &buffer, None, &retained, &[u16::MAX], false);

    assert_eq!(*buffer.lock(), vec![0.1]);
    assert!(retained.exceeded());
    assert!(accum.lock().peak > 0.9);
}

#[test]
fn chunk_sink_sets_overflow_when_stream_queue_is_full() {
    let (tx, rx) = mpsc::sync_channel(1);
    let overflowed = Arc::new(AtomicBool::new(false));
    let sink = ChunkSink {
        tx,
        overflowed: overflowed.clone(),
        sequence: AtomicU64::new(0),
        sample_rate: 16_000,
        channels: 1,
    };

    sink.forward(vec![0.1]);
    sink.forward(vec![0.2]);

    assert!(overflowed.load(Ordering::Relaxed));
    let chunk = rx.try_recv().unwrap().unwrap();
    assert_eq!(chunk.samples, vec![0.1]);
    assert_eq!(chunk.sequence, 0);
}

#[test]
fn chunk_sink_finish_does_not_block_when_stream_queue_is_full() {
    let (tx, _rx) = mpsc::sync_channel(1);
    let overflowed = Arc::new(AtomicBool::new(false));
    let sink = ChunkSink {
        tx,
        overflowed: overflowed.clone(),
        sequence: AtomicU64::new(0),
        sample_rate: 16_000,
        channels: 1,
    };

    sink.forward(vec![0.1]);
    sink.finish();

    assert!(overflowed.load(Ordering::Relaxed));
}
