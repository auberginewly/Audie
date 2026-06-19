// Zod schema for the settings payload returned by get_settings / update_settings.
// Mirrors Rust `commands::Settings` + `HOTKEY_PRESETS` (src-tauri/src/commands.rs).
// P0.5 scope: hotkey only; microphone selection lands with the Settings page.

import { z } from "zod";

export const SettingsSchema = z.object({
  hotkey: z.enum(["Ctrl+Shift+Space", "Alt+Space", "Ctrl+Alt+Space"]),
  asr_provider: z.enum(["groq", "openai", "whisper_cpp"]),
  llm_provider: z.enum(["openai_compatible"]),
  enhance_enabled: z.boolean(),
  enhance_prompt: z.string().min(1),
  whisper_cpp_model_path: z.string().nullable(),
  openai_compatible_base_url: z.string().min(1),
  openai_compatible_model: z.string().min(1),
});

export type Settings = z.infer<typeof SettingsSchema>;
export type Hotkey = Settings["hotkey"];
export type AsrProviderId = Settings["asr_provider"];
export type LlmProviderId = Settings["llm_provider"];

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

export const KEYCHAIN_PLACEHOLDER = "<keychain>";

export const ExportedSecretPlaceholderSchema = z.object({
  key_id: z.enum(["groq_api_key", "openai_api_key", "openai_compatible_api_key"]),
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

// Single source for the dropdown — derived from the schema so they can't drift.
export const HOTKEY_PRESETS = SettingsSchema.shape.hotkey.options;
