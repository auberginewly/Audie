// Zod schema for the settings payload returned by get_settings / update_settings.
// Mirrors Rust `settings::Settings` (src-tauri/src/settings/types.rs).
// P0.5 scope: hotkey only; microphone selection lands with the Settings page.

import { z } from "zod";
import { AsrProviderIdSchema, LlmProviderIdSchema } from "../providers/schema.ts";

export const SettingsSchema = z.object({
  // Trigger key: "Fn", a function key ("F13"), or a combo ("Ctrl+Shift+Space").
  // Backend parse_trigger is the real gate (SPEC §5.8 P3.9), so keep it permissive.
  hotkey: z.string().min(1),
  asr_provider: AsrProviderIdSchema,
  // Selected ASR model id (front/back share the exact string). "" = use each
  // adapter's built-in default. default("") tolerates the field being absent
  // while the backend struct ships in parallel.
  asr_model: z.string().default(""),
  llm_provider: LlmProviderIdSchema,
  // 「AI 润色」总开关。true（默认）= 配了 LLM 即自动润色；false = 纯转写（只插转写原文）。
  enhance_enabled: z.boolean(),
  enhance_prompt: z.string().min(1),
  openai_compatible_base_url: z.string().min(1),
  // Empty allowed: picking a provider seeds no model (hardcoded ids go stale) — the
  // user fetches/types one. Backend errors clearly if enhance runs without a model.
  openai_compatible_model: z.string(),
  // Keychain key id for the active LLM provider's key (4b: each cloud LLM card
  // stores its own key). "" = key-optional local provider. Defaults backend-side
  // to the legacy shared id; permissive string — backend reads whatever id it holds.
  llm_api_key_id: z.string().default("openai_compatible_api_key"),
  doubao_endpoint: z.string().min(1),
  doubao_resource_id: z.string().min(1),
  glm_endpoint: z.string().min(1),
  aliyun_endpoint: z.string().min(1),
  stepfun_endpoint: z.string().min(1),
  input_device: z.string(),
  onboarding_completed: z.boolean(),
  onboarding_test_completed: z.boolean(),
  // User's main language; the backend prepends it as a line to the enhance prompt.
  // "" = follow system locale (resolved backend-side).
  primary_language: z.string(),
  // How long dictation history is kept (History screen). Backend normalize clamps
  // anything unknown to "forever", so the enum is safe.
  history_retention: z.enum(["never", "day", "week", "month", "forever"]),
  ui_language: z.enum(["zh-Hans", "zh-Hant", "en"]),
  show_in_dock: z.boolean().default(true),
  // 写作模式（compose）：独立触发键（"" = 未配置）、总开关、提示词。后端 normalize 保证
  // prompt 非空，故沿用 enhance_prompt 的 min(1)。
  compose_hotkey: z.string(),
  compose_prompt: z.string().min(1),
  rewrite_prompt: z.string().min(1),
  // Per-provider LLM model keyed by card id (deepseek/lmstudio/…). Lets 选用 restore
  // each provider's own model instead of clearing it (single backend slot).
  llm_models: z.record(z.string(), z.string()).default({}),
});

export type Settings = z.infer<typeof SettingsSchema>;
export type Hotkey = Settings["hotkey"];
