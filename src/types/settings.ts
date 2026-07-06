// Zod schema for the settings payload returned by get_settings / update_settings.
// Mirrors Rust `commands::Settings` + `HOTKEY_PRESETS` (src-tauri/src/commands.rs).
// P0.5 scope: hotkey only; microphone selection lands with the Settings page.

import { z } from "zod";

export const SettingsSchema = z.object({
  // Trigger key: "Fn", a function key ("F13"), or a combo ("Ctrl+Shift+Space").
  // Backend parse_trigger is the real gate (SPEC §5.8 P3.9), so keep it permissive.
  hotkey: z.string().min(1),
  asr_provider: z.enum(["groq", "openai", "doubao_stream", "glm", "aliyun_fun", "stepfun"]),
  // Selected ASR model id (front/back share the exact string). "" = use each
  // adapter's built-in default. default("") tolerates the field being absent
  // while the backend struct ships in parallel.
  asr_model: z.string().default(""),
  llm_provider: z.enum(["openai_compatible"]),
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
  input_device: z.string(),
  onboarding_completed: z.boolean(),
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
export type AsrProviderId = Settings["asr_provider"];
export type LlmProviderId = Settings["llm_provider"];

// Microphone enumerated by `list_microphones` (Rust). `id` is the cpal device
// name and the value persisted into `input_device`; "" / "auto" = automatic.
export const AudioDeviceSchema = z.object({
  id: z.string(),
  label: z.string(),
});
export type AudioDevice = z.infer<typeof AudioDeviceSchema>;

// auto_input_device (Rust): the device the automatic path resolves to, or null.
export const AutoDeviceSchema = z.string().nullable();

export const ProviderMetadataSchema = z.object({
  id: z.string(),
  title: z.string(),
  kind: z.enum(["asr", "llm"]),
  engine: z.string(),
  default_model: z.string().nullable(),
  requires_key: z.boolean(),
  tags: z.array(z.string()),
});

export type ProviderMetadata = z.infer<typeof ProviderMetadataSchema>;

// Hand-written mirror of Rust `provider_test::DiscoveredLocalLlm` — one auto-detected
// local-LLM server returned by the discover_local_llm command (A2 zero-click probe).
// Keep field names in sync with that serde struct.
export const DiscoveredLocalLlmSchema = z.object({
  // Picker card id: ollama / lmstudio / llamacpp.
  provider: z.string(),
  base_url: z.string(),
  models: z.array(z.string()),
});

export type DiscoveredLocalLlm = z.infer<typeof DiscoveredLocalLlmSchema>;

export const ProviderKindSchema = z.enum(["asr", "llm"]);
export type ProviderKind = z.infer<typeof ProviderKindSchema>;

export const ProviderTestResultSchema = z.object({
  ok: z.boolean(),
  message: z.string(),
});

export type ProviderTestResult = z.infer<typeof ProviderTestResultSchema>;

// Every keychain-backed secret id. Doubao keeps the historical
// `doubao_access_token` id for either new-console API Key or old-console Access
// Token, covering both without migrating saved secrets.
export const SecretKeyIdSchema = z.enum([
  "groq_api_key",
  "openai_api_key",
  "openai_compatible_api_key",
  // Per-provider LLM keys (4b): each cloud LLM card stores its own key. OpenAI LLM
  // reuses openai_api_key (same account as OpenAI Transcribe).
  "deepseek_api_key",
  "kimi_api_key",
  "siliconflow_api_key",
  "zhipu_api_key",
  "qwen_api_key",
  "openrouter_api_key",
  "doubao_app_id",
  "doubao_access_token",
  "glm_api_key",
  "aliyun_dashscope_api_key",
  "stepfun_api_key",
]);
export type SecretKeyId = z.infer<typeof SecretKeyIdSchema>;

// Only these providers have a reachable `test_provider` probe (P1.3). Doubao
// streaming connectivity is exercised by a dev command in P2.5, not here.
export const TestProviderKeyIdSchema = z.enum(["groq_api_key", "openai_api_key", "openai_compatible_api_key"]);
export type TestProviderKeyId = z.infer<typeof TestProviderKeyIdSchema>;

export const ProviderTestRequestSchema = z.object({
  kind: ProviderKindSchema,
  provider_id: z.union([SettingsSchema.shape.asr_provider, SettingsSchema.shape.llm_provider]),
  key_id: TestProviderKeyIdSchema,
  api_key: z.string().nullable(),
  base_url: z.string().nullable(),
});

export type ProviderTestRequest = z.infer<typeof ProviderTestRequestSchema>;
