// The model catalog the picker renders. Shape mirrors the design's MODELS list.
// rating / tags / status are mock (visual only); the active pick maps to the
// real provider enum where one exists (see helpers below + plan mapping table).

import type { IconName } from "../ui";
import type { AsrProviderId, SecretKeyId } from "../../types/settings";

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
      return ["doubao_app_id", "doubao_access_token"];
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
