// Zod schemas for the dictation-history payloads (list_history / get_usage_stats)
// and the history-updated event. Mirrors Rust `managers::history::{HistoryEntry,
// UsageStats}` (snake_case). External input MUST be validated (no `any`).

import { z } from "zod";

// "success" = a dictation with text (also covers a cancelled-but-transcribed take);
// "empty" = nothing recognized (silence / blank ASR), shown as 没有识别到内容.
export const HistoryKindSchema = z.enum(["success", "empty"]);
export type HistoryKind = z.infer<typeof HistoryKindSchema>;

// 处理模式：润色（口述听写）/ 改写（改选中文字）/ 写作（按要点生成）。老库行默认 polish。
export const HistoryModeSchema = z.enum(["polish", "rewrite", "compose"]);
export type HistoryMode = z.infer<typeof HistoryModeSchema>;

export const HistoryEntrySchema = z.object({
  id: z.number(),
  created_at: z.number(), // UTC unix seconds
  kind: HistoryKindSchema,
  mode: HistoryModeSchema,
  raw_text: z.string(),
  // Option<String> from Rust → null when absent; nullish tolerates null + missing.
  enhanced_text: z.string().nullish(),
  word_count: z.number(),
  duration_ms: z.number(),
});
export type HistoryEntry = z.infer<typeof HistoryEntrySchema>;

// 口述类（mode=polish）：时长/字数/次数；AI 产出类（compose+rewrite）：产出字数。
// 拆开是为了 Home 的「口述」卡不被写作生成的字数虚高（见 history.rs fetch_stats）。
export const UsageStatsSchema = z.object({
  spoken_words: z.number(),
  spoken_duration_ms: z.number(),
  spoken_count: z.number(),
  ai_output_words: z.number(),
});
export type UsageStats = z.infer<typeof UsageStatsSchema>;

// Emitted by HistoryManager after any insert/delete/clear; screens re-fetch on it.
export const EVENT_HISTORY_UPDATED = "history-updated";
