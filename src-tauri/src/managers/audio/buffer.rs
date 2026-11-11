use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::TrySendError;

use parking_lot::Mutex;

use super::{ChunkSink, LevelAccum, RetainedSamples};
use crate::asr::AudioChunk;

impl RetainedSamples {
    pub(super) fn new(max_samples: usize) -> Self {
        Self {
            max_samples,
            exceeded: AtomicBool::new(false),
        }
    }

    fn accepted_len(&self, current_len: usize, incoming_len: usize) -> usize {
        let remaining = self.max_samples.saturating_sub(current_len);
        let accepted = remaining.min(incoming_len);
        if accepted < incoming_len {
            self.exceeded.store(true, Ordering::Relaxed);
        }
        accepted
    }

    pub(super) fn exceeded(&self) -> bool {
        self.exceeded.load(Ordering::Relaxed)
    }
}
impl ChunkSink {
    fn forward(&self, samples: Vec<f32>) {
        let sequence = self.sequence.fetch_add(1, Ordering::Relaxed);
        let result = self.tx.try_send(Ok(AudioChunk {
            samples,
            sample_rate: self.sample_rate,
            channels: self.channels,
            sequence,
            is_final: false,
        }));
        match result {
            Ok(()) | Err(TrySendError::Disconnected(_)) => {}
            Err(TrySendError::Full(_)) => {
                self.overflowed.store(true, Ordering::Relaxed);
            }
        }
    }

    /// Send an end-of-input sentinel so the streaming consumer's recv loop ends
    /// deterministically at stop. We must NOT rely on the Sender's Drop for this:
    /// the cpal callback holds a ChunkSink clone whose release can lag past stop,
    /// so the doubao loop would block on recv forever and never send its closing
    /// frame — the take then hangs until the 20s timeout. An explicit is_final
    /// chunk breaks the loop now.
    pub(super) fn finish(&self) {
        let sequence = self.sequence.fetch_add(1, Ordering::Relaxed);
        let result = self.tx.try_send(Ok(AudioChunk {
            samples: Vec::new(),
            sample_rate: self.sample_rate,
            channels: self.channels,
            sequence,
            is_final: true,
        }));
        match result {
            Ok(()) | Err(TrySendError::Disconnected(_)) => {}
            Err(TrySendError::Full(_)) => {
                self.overflowed.store(true, Ordering::Relaxed);
                log::warn!("streaming audio queue full at final sentinel; receiver will finish on channel close");
            }
        }
    }
}
// Each `process_*` does one locked pass: track the peak for the waveform and
// append normalized f32 samples for transcription. When a streaming sink is
// present, the same normalized samples are also forwarded as an `AudioChunk`.
pub(super) fn process_f32(
    accum: &Mutex<LevelAccum>,
    buffer: &Mutex<Vec<f32>>,
    sink: Option<&ChunkSink>,
    retained: &RetainedSamples,
    data: &[f32],
    level_only: bool,
) {
    let mut peak = 0.0f32;
    let mut accepted = data.len();
    {
        // `level_only` (Settings mic preview) tracks the peak only — no buffer
        // lock, no sample retention — so a long preview can't grow memory.
        let mut buf = if level_only {
            None
        } else {
            Some(buffer.lock())
        };
        if let Some(buf) = buf.as_mut() {
            accepted = retained.accepted_len(buf.len(), data.len());
            buf.reserve(accepted);
        }
        for (index, &s) in data.iter().enumerate() {
            let a = s.abs();
            if a > peak {
                peak = a;
            }
            if index < accepted {
                if let Some(buf) = buf.as_mut() {
                    buf.push(s);
                }
            }
        }
    }
    merge_peak(accum, peak);
    if let Some(sink) = sink.filter(|_| accepted > 0) {
        sink.forward(data[..accepted].to_vec());
    }
}

pub(super) fn process_i16(
    accum: &Mutex<LevelAccum>,
    buffer: &Mutex<Vec<f32>>,
    sink: Option<&ChunkSink>,
    retained: &RetainedSamples,
    data: &[i16],
    level_only: bool,
) {
    let scale = i16::MAX as f32;
    let mut peak = 0.0f32;
    let mut chunk: Option<Vec<f32>> = None;
    {
        let mut buf = if level_only {
            None
        } else {
            Some(buffer.lock())
        };
        let accepted = buf.as_ref().map_or(data.len(), |buf| {
            retained.accepted_len(buf.len(), data.len())
        });
        if let Some(buf) = buf.as_mut() {
            buf.reserve(accepted);
        }
        if sink.is_some() && accepted > 0 {
            chunk = Some(Vec::with_capacity(accepted));
        }
        for (index, &s) in data.iter().enumerate() {
            let v = s as f32 / scale;
            let a = v.abs();
            if a > peak {
                peak = a;
            }
            if index < accepted {
                if let Some(buf) = buf.as_mut() {
                    buf.push(v);
                }
                if let Some(chunk) = chunk.as_mut() {
                    chunk.push(v);
                }
            }
        }
    }
    merge_peak(accum, peak);
    if let (Some(sink), Some(chunk)) = (sink, chunk) {
        sink.forward(chunk);
    }
}

pub(super) fn process_u16(
    accum: &Mutex<LevelAccum>,
    buffer: &Mutex<Vec<f32>>,
    sink: Option<&ChunkSink>,
    retained: &RetainedSamples,
    data: &[u16],
    level_only: bool,
) {
    let mut peak = 0.0f32;
    let mut chunk: Option<Vec<f32>> = None;
    {
        let mut buf = if level_only {
            None
        } else {
            Some(buffer.lock())
        };
        let accepted = buf.as_ref().map_or(data.len(), |buf| {
            retained.accepted_len(buf.len(), data.len())
        });
        if let Some(buf) = buf.as_mut() {
            buf.reserve(accepted);
        }
        if sink.is_some() && accepted > 0 {
            chunk = Some(Vec::with_capacity(accepted));
        }
        for (index, &s) in data.iter().enumerate() {
            // u16 silence is centered at 32768.
            let v = (s as f32 - 32768.0) / 32768.0;
            let a = v.abs();
            if a > peak {
                peak = a;
            }
            if index < accepted {
                if let Some(buf) = buf.as_mut() {
                    buf.push(v);
                }
                if let Some(chunk) = chunk.as_mut() {
                    chunk.push(v);
                }
            }
        }
    }
    merge_peak(accum, peak);
    if let (Some(sink), Some(chunk)) = (sink, chunk) {
        sink.forward(chunk);
    }
}
fn merge_peak(accum: &Mutex<LevelAccum>, local: f32) {
    if local <= 0.0 {
        return;
    }
    let mut g = accum.lock();
    if local > g.peak {
        g.peak = local;
    }
}

#[cfg(test)]
mod tests;
