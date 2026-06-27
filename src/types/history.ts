// Zod schemas for the dictation-history payloads (list_history / get_usage_stats)
// and the history-updated event. Mirrors Rust `managers::history::{HistoryEntry,
// UsageStats}` (snake_case). External input MUST be validated (no `any`).

import { z } from "zod";

// "success" = a dictation with text (also covers a cancelled-but-transcribed take);
// "empty" = nothing recognized (silence / blank ASR), shown as 没有识别到内容.
export const HistoryKindSchema = z.enum(["success", "empty"]);
export type HistoryKind = z.infer<typeof HistoryKindSchema>;

export const HistoryEntrySchema = z.object({
  id: z.number(),
  created_at: z.number(), // UTC unix seconds
  kind: HistoryKindSchema,
  raw_text: z.string(),
  // Option<String> from Rust → null when absent; nullish tolerates null + missing.
  enhanced_text: z.string().nullish(),
  word_count: z.number(),
  duration_ms: z.number(),
});
export type HistoryEntry = z.infer<typeof HistoryEntrySchema>;

export const UsageStatsSchema = z.object({
  total_words: z.number(),
  total_duration_ms: z.number(),
  dictation_count: z.number(),
});
export type UsageStats = z.infer<typeof UsageStatsSchema>;

// Emitted by HistoryManager after any insert/delete/clear; screens re-fetch on it.
export const EVENT_HISTORY_UPDATED = "history-updated";
