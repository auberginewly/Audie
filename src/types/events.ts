// Zod schemas for every Rust → frontend event payload.
// External input MUST be validated before use (CLAUDE.md TS rule: no `any`).
// Must mirror Rust definitions in src-tauri/src/state.rs (StateChange) and §3.7.

import { z } from "zod";

export const AppStateSchema = z.enum([
  "IDLE",
  "RECORDING",
  "PROCESSING",
  "SUCCESS",
  "ERROR",
  "CANCEL",
]);
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
