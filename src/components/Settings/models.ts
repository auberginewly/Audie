// The model catalog the picker renders. Shape mirrors the design's MODELS list.
// rating / tags / status are mock (visual only); the active pick maps to the
// real provider enum where one exists (see helpers below + plan mapping table).

import type { IconName } from "../ui";
import type { AsrProviderId, SecretKeyId, Settings } from "../../types/settings";

export type ModelType = "asr" | "llm";
export type ModelSource = "cloud" | "local";

export type ModelMeta = {
  id: string;
  name: string;
  type: ModelType;
  source: ModelSource;
  icon: IconName;
  model: string;
  rating: string; // mock
  tags: string[]; // mock
};

export const MODELS: ModelMeta[] = [
  { id: "doubao", name: "豆包", type: "asr", source: "cloud", icon: "audio-lines", model: "Doubao ASR 2.0 (Hourly)", rating: "4.6", tags: ["云端", "均衡", "实时"] },
  { id: "groq", name: "Groq", type: "asr", source: "cloud", icon: "audio-lines", model: "whisper-large-v3-turbo", rating: "4.7", tags: ["云端", "快速"] },
  { id: "whisper-local", name: "Whisper", type: "asr", source: "local", icon: "audio-lines", model: "whisper-large-v3", rating: "4.5", tags: ["本地", "离线"] },
  { id: "deepseek", name: "DeepSeek", type: "llm", source: "cloud", icon: "sparkles", model: "DeepSeek V4 Flash", rating: "4.8", tags: ["云端", "推荐"] },
  { id: "openai", name: "OpenAI", type: "llm", source: "cloud", icon: "sparkles", model: "gpt-4o-mini", rating: "4.7", tags: ["云端", "兼容"] },
];

// The keychain secrets a model needs before it's usable — the source of truth for
// "已配置" badges (see useConfiguredModels), replacing the old mock status field.
// whisper-local has none: local inference isn't wired until P3 model mgmt, so it
// can't satisfy onboarding gating.
export function requiredSecretsForModel(id: string): SecretKeyId[] {
  switch (id) {
    case "groq":
      return ["groq_api_key"];
    case "doubao":
      // app_id is optional (old-console only); the new console uses just the
      // access token / API key, so the token alone means configured (backend
      // treats a blank app_id as new-console mode — client.rs from_settings).
      return ["doubao_access_token"];
    case "deepseek":
    case "openai":
      return ["openai_compatible_api_key"];
    default:
      return [];
  }
}

// Design model id → backend ASR provider enum (null = card with no real slot).
// Picking doubao now selects the streaming provider explicitly; the backend only
// activates streaming when asr_provider == "doubao_stream" AND a token is stored.
export function asrProviderForModelId(id: string): AsrProviderId | null {
  if (id === "groq") return "groq";
  if (id === "whisper-local") return "whisper_cpp";
  if (id === "doubao") return "doubao_stream";
  return null;
}

// Backend ASR provider → which catalog card should read as active on load.
export function modelIdForAsrProvider(provider: AsrProviderId): string {
  if (provider === "groq") return "groq";
  if (provider === "whisper_cpp") return "whisper-local";
  if (provider === "doubao_stream") return "doubao";
  return "doubao"; // openai ASR has no card — fall back to the streaming default
}

// LLM cards all drive the single openai_compatible slot; the card id picks a preset
// (endpoint + model) so 选用 OpenAI configures OpenAI and 选用 DeepSeek configures
// DeepSeek, instead of both showing whatever the one slot currently holds.
export function llmPresetForModelId(id: string): { baseUrl: string; model: string } {
  if (id === "openai") return { baseUrl: "https://api.openai.com/v1", model: "gpt-4o-mini" };
  return { baseUrl: "https://api.deepseek.com/v1", model: "deepseek-chat" }; // deepseek default
}

// Settings patch for picking an LLM card: switch to openai_compatible and apply the
// provider preset — but keep an existing same-provider config (don't clobber a
// custom endpoint/model when re-picking the provider already in use).
export function llmPickPatch(id: string, currentBaseUrl: string): Partial<Settings> {
  const patch: Partial<Settings> = { llm_provider: "openai_compatible" };
  const domain = id === "openai" ? "openai.com" : "deepseek";
  if (currentBaseUrl.includes(domain)) return patch;
  const preset = llmPresetForModelId(id);
  return { ...patch, openai_compatible_base_url: preset.baseUrl, openai_compatible_model: preset.model };
}

// Which LLM card the saved openai_compatible base_url maps to, so the picker's
// 使用中 highlight persists across restarts (like ASR derives from asr_provider).
// Custom endpoints fall back to the deepseek card.
export function llmModelIdForBaseUrl(baseUrl: string): string {
  return baseUrl.includes("openai.com") ? "openai" : "deepseek";
}
