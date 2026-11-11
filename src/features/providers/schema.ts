import { z } from "zod";

export const AsrProviderIdSchema = z.enum(["openai", "doubao_stream", "glm", "aliyun_fun", "stepfun"]);
export type AsrProviderId = z.infer<typeof AsrProviderIdSchema>;
export const LlmProviderIdSchema = z.enum(["openai_compatible"]);
export type LlmProviderId = z.infer<typeof LlmProviderIdSchema>;

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
export const TestProviderKeyIdSchema = z.enum(["openai_api_key", "openai_compatible_api_key"]);
export type TestProviderKeyId = z.infer<typeof TestProviderKeyIdSchema>;

export const ProviderTestRequestSchema = z.object({
  kind: ProviderKindSchema,
  provider_id: z.union([AsrProviderIdSchema, LlmProviderIdSchema]),
  key_id: TestProviderKeyIdSchema,
  api_key: z.string().nullable(),
  base_url: z.string().nullable(),
});

export type ProviderTestRequest = z.infer<typeof ProviderTestRequestSchema>;
