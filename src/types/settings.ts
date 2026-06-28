// Zod schema for the settings payload returned by get_settings / update_settings.
// Mirrors Rust `commands::Settings` + `HOTKEY_PRESETS` (src-tauri/src/commands.rs).
// P0.5 scope: hotkey only; microphone selection lands with the Settings page.

import { z } from "zod";

export const SettingsSchema = z.object({
  // Trigger key: "Fn", a function key ("F13"), or a combo ("Ctrl+Shift+Space").
  // Backend parse_trigger is the real gate (SPEC §5.8 P3.9), so keep it permissive.
  hotkey: z.string().min(1),
  asr_provider: z.enum([
    "groq",
    "openai",
    "whisper_cpp",
    "doubao_stream",
    "glm",
    "aliyun_fun",
    "stepfun",
    // macOS-only keyless on-device dictation (SFSpeechRecognizer). The backend
    // only lists it on macOS; normalize resets an unknown provider to default, so a
    // settings.toml carrying it on a non-macOS build degrades gracefully.
    "macos_native",
  ]),
  // Selected ASR model id (front/back share the exact string). "" = use each
  // adapter's built-in default. default("") tolerates the field being absent
  // while the backend struct ships in parallel.
  asr_model: z.string().default(""),
  // Selected local-ASR (whisper.cpp) model id from the ModelManager catalog, or a
  // discovered custom id. "" = no catalog pick → fall back to whisper_cpp_model_path.
  // default("") tolerates older settings.toml that predate the field.
  selected_local_asr_model: z.string().default(""),
  llm_provider: z.enum(["openai_compatible"]),
  enhance_enabled: z.boolean(),
  enhance_prompt: z.string().min(1),
  // nullish (not just nullable): the backend omits this key entirely when None
  // (serde skip_serializing_if, required because TOML has no null), so get_settings
  // may send it absent — a required nullable field would fail and blank all settings.
  whisper_cpp_model_path: z.string().nullish(),
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

// Local ASR model from the backend ModelManager catalog (get_available_models).
// Hand-written mirror of Rust `managers::model::ModelInfo` — keep field names and
// optionality in sync with that serde struct (Audie hand-writes Zod, no specta).
// Wired into the picker UI in Phase 3; defined here so the type lands with P1.
export const ModelInfoSchema = z.object({
  id: z.string(),
  name: z.string(),
  description: z.string(),
  filename: z.string(),
  // Option<String> on the Rust side: present for catalog models, null for custom
  // on-disk ones. nullish tolerates either null or an omitted key.
  url: z.string().nullish(),
  sha256: z.string().nullish(),
  size_mb: z.number(),
  is_downloaded: z.boolean(),
  // Phase 2 download state: in-flight download + bytes already on disk in the
  // `.partial` file (0 when none). Drive the picker's progress/cancel row.
  is_downloading: z.boolean(),
  partial_size: z.number(),
  is_recommended: z.boolean(),
  is_custom: z.boolean(),
  engine: z.string(),
});

export type ModelInfo = z.infer<typeof ModelInfoSchema>;

// Hand-written mirror of Rust `managers::model::DownloadProgress` — payload of the
// `model-download-progress` event (Phase 2 downloader). Keep field names in sync
// with that serde struct. The companion events (model-download-complete /
// -cancelled / -deleted) carry just the model_id string; -failed carries
// { model_id, error }.
export const DownloadProgressSchema = z.object({
  model_id: z.string(),
  downloaded: z.number(),
  total: z.number(),
  percentage: z.number(),
});

export type DownloadProgress = z.infer<typeof DownloadProgressSchema>;

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
// Token, so export/import cover all five without migrating saved secrets.
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
export const TestProviderKeyIdSchema = z.enum([
  "groq_api_key",
  "openai_api_key",
  "openai_compatible_api_key",
]);
export type TestProviderKeyId = z.infer<typeof TestProviderKeyIdSchema>;

export const ProviderTestRequestSchema = z.object({
  kind: ProviderKindSchema,
  provider_id: z.union([SettingsSchema.shape.asr_provider, SettingsSchema.shape.llm_provider]),
  key_id: TestProviderKeyIdSchema,
  api_key: z.string().nullable(),
  base_url: z.string().nullable(),
});

export type ProviderTestRequest = z.infer<typeof ProviderTestRequestSchema>;

export const KEYCHAIN_PLACEHOLDER = "<keychain>";

export const ExportedSecretPlaceholderSchema = z.object({
  key_id: SecretKeyIdSchema,
  value: z.literal(KEYCHAIN_PLACEHOLDER),
});

export const ExportedConfigSchema = z.object({
  settings: SettingsSchema,
  secrets: z.array(ExportedSecretPlaceholderSchema),
});

export type ExportedConfig = z.infer<typeof ExportedConfigSchema>;

export const ImportConfigResultSchema = z.object({
  settings: SettingsSchema,
  keys_to_refill: z.array(ExportedSecretPlaceholderSchema.shape.key_id),
  message: z.string(),
});

export type ImportConfigResult = z.infer<typeof ImportConfigResultSchema>;
