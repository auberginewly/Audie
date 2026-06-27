// Zod schema for the settings payload returned by get_settings / update_settings.
// Mirrors Rust `commands::Settings` + `HOTKEY_PRESETS` (src-tauri/src/commands.rs).
// P0.5 scope: hotkey only; microphone selection lands with the Settings page.

import { z } from "zod";

export const SettingsSchema = z.object({
  // Trigger key: "Fn", a function key ("F13"), or a combo ("Ctrl+Shift+Space").
  // Backend parse_trigger is the real gate (SPEC §5.8 P3.9), so keep it permissive.
  hotkey: z.string().min(1),
  asr_provider: z.enum(["groq", "openai", "whisper_cpp", "doubao_stream"]),
  llm_provider: z.enum(["openai_compatible"]),
  enhance_enabled: z.boolean(),
  enhance_prompt: z.string().min(1),
  // nullish (not just nullable): the backend omits this key entirely when None
  // (serde skip_serializing_if, required because TOML has no null), so get_settings
  // may send it absent — a required nullable field would fail and blank all settings.
  whisper_cpp_model_path: z.string().nullish(),
  openai_compatible_base_url: z.string().min(1),
  openai_compatible_model: z.string().min(1),
  doubao_endpoint: z.string().min(1),
  doubao_resource_id: z.string().min(1),
  input_device: z.string(),
  onboarding_completed: z.boolean(),
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
  "doubao_app_id",
  "doubao_access_token",
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
