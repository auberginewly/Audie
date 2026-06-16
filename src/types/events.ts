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
