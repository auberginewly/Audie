// Zod schemas for every Rust → frontend event payload.
// External input MUST be validated before use (CLAUDE.md TS rule: no `any`).
// Must mirror Rust definitions in src-tauri/src/state.rs (StateChange) and §3.7.

import { z } from "zod";

export const AppStateSchema = z.enum(["IDLE", "RECORDING", "PROCESSING", "SUCCESS", "ERROR", "CANCEL"]);
export type AppState = z.infer<typeof AppStateSchema>;

export const StateChangeSchema = z.object({
  from: AppStateSchema,
  to: AppStateSchema,
  reason: z.string().optional(),
});
export type StateChange = z.infer<typeof StateChangeSchema>;

export const EVENT_STATE_CHANGE = "state-change";

// `audio-level`: emitted by AudioManager ~30 FPS while RECORDING.
// `level` is a normalized peak in [0, 1]. PROJECT_SPEC.md §3.6.
export const AudioLevelSchema = z.object({
  level: z.number().min(0).max(1),
});
export type AudioLevel = z.infer<typeof AudioLevelSchema>;

export const EVENT_AUDIO_LEVEL = "audio-level";

// `mic-monitor-level`: AudioManager's Settings mic-preview level. Same {level}
// shape as `audio-level` (reuse AudioLevelSchema), but a separate event so the
// preview meter never drives the overlay waveform.
export const EVENT_MIC_MONITOR_LEVEL = "mic-monitor-level";

// `error`: emitted on any pipeline failure. Mirrors src-tauri ErrorPayload /
// AppError categories (PROJECT_SPEC.md §3.6 / §3.7).
export const ErrorCodeSchema = z.enum(["permission", "device", "network", "provider", "inject", "internal"]);
export type ErrorCode = z.infer<typeof ErrorCodeSchema>;

export const AppErrorSchema = z.object({
  code: ErrorCodeSchema,
  message: z.string(),
  recoverable: z.boolean(),
});
export type AppErrorEvent = z.infer<typeof AppErrorSchema>;

export const EVENT_ERROR = "error";

export const EnhanceProgressSchema = z.object({
  phase: z.enum(["started", "completed", "failed"]),
  message: z.string(),
});
export type EnhanceProgressEvent = z.infer<typeof EnhanceProgressSchema>;

export const EVENT_ENHANCE_PROGRESS = "enhance-progress";
