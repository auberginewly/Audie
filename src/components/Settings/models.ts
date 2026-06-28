// The model catalog the picker renders. Shape mirrors the design's MODELS list.
// The active pick maps to the real provider enum where one exists (ASR) or to an
// openai_compatible preset (LLM) — see helpers below + the plan mapping table.
// tags are meaningful badges (云端/本地/兼容/实时), not mock scores.

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
  tags: string[];
};

export type ModelOption = { id: string; title: string };

export const MODELS: ModelMeta[] = [
  // ASR
  { id: "doubao", name: "豆包", type: "asr", source: "cloud", icon: "audio-lines", model: "Doubao ASR 2.0 (Hourly)", tags: ["云端"] },
  { id: "groq", name: "Groq", type: "asr", source: "cloud", icon: "audio-lines", model: "whisper-large-v3-turbo", tags: ["云端"] },
  { id: "openai-asr", name: "OpenAI Transcribe", type: "asr", source: "cloud", icon: "audio-lines", model: "whisper-1", tags: ["云端"] },
  { id: "glm-asr", name: "智谱 GLM ASR", type: "asr", source: "cloud", icon: "audio-lines", model: "glm-asr-1", tags: ["云端"] },
  { id: "aliyun-asr", name: "通义 Paraformer ASR", type: "asr", source: "cloud", icon: "audio-lines", model: "fun-asr-realtime", tags: ["云端"] },
  { id: "stepfun-asr", name: "StepFun ASR", type: "asr", source: "cloud", icon: "audio-lines", model: "stepaudio-2.5-asr", tags: ["云端"] },
  { id: "whisper-local", name: "Whisper", type: "asr", source: "local", icon: "audio-lines", model: "whisper-large-v3", tags: ["本地", "离线"] },
  // macOS 本机听写: keyless, OS-managed model — always "installed", nothing to download.
  // model copy is a label only (no model id / variant); maps to the macos_native provider.
  { id: "macos-native", name: "macOS 本机听写", type: "asr", source: "local", icon: "audio-lines", model: "系统内置（离线）", tags: ["本地", "内置", "离线"] },
  // LLM — all drive the single openai_compatible slot. No hardcoded model: the card
  // subtitle shows the real configured model (active card) or nothing; model field
  // is unused for LLM display, kept empty so the catalog carries no guessed ids.
  { id: "deepseek", name: "DeepSeek", type: "llm", source: "cloud", icon: "sparkles", model: "", tags: ["云端"] },
  { id: "openai", name: "OpenAI", type: "llm", source: "cloud", icon: "sparkles", model: "", tags: ["云端"] },
  { id: "kimi", name: "Kimi（月之暗面）", type: "llm", source: "cloud", icon: "sparkles", model: "", tags: ["云端"] },
  { id: "siliconflow", name: "硅基流动", type: "llm", source: "cloud", icon: "sparkles", model: "", tags: ["云端"] },
  { id: "zhipu", name: "智谱 GLM", type: "llm", source: "cloud", icon: "sparkles", model: "", tags: ["云端"] },
  { id: "qwen", name: "通义千问", type: "llm", source: "cloud", icon: "sparkles", model: "", tags: ["云端"] },
  { id: "openrouter", name: "OpenRouter", type: "llm", source: "cloud", icon: "sparkles", model: "", tags: ["云端"] },
  { id: "ollama", name: "Ollama", type: "llm", source: "local", icon: "sparkles", model: "", tags: ["本地"] },
  { id: "lmstudio", name: "LM Studio", type: "llm", source: "local", icon: "sparkles", model: "", tags: ["本地"] },
];

// The keychain secrets a model needs before it's usable — the source of truth for
// "已配置" badges (see useConfiguredModels). LLM stays a single backend provider
// (openai_compatible), so every cloud LLM card shares one key slot. Ollama / LM
// Studio run locally with an optional key, so they require none (Voxt's
// apiKeyIsOptional). whisper-local also has none: local inference can't satisfy
// onboarding gating yet.
export function requiredSecretsForModel(id: string): SecretKeyId[] {
  switch (id) {
    case "groq":
      return ["groq_api_key"];
    case "openai-asr":
      return ["openai_api_key"];
    case "glm-asr":
      return ["glm_api_key"];
    case "aliyun-asr":
      return ["aliyun_dashscope_api_key"];
    case "stepfun-asr":
      return ["stepfun_api_key"];
    case "doubao":
      // app_id is optional (old-console only); the new console uses just the
      // access token / API key, so the token alone means configured (backend
      // treats a blank app_id as new-console mode — client.rs from_settings).
      return ["doubao_access_token"];
    case "deepseek":
    case "openai":
    case "kimi":
    case "siliconflow":
    case "zhipu":
    case "qwen":
    case "openrouter": {
      // 4b: each cloud LLM card has its own key, so "已配置" is per-provider (no
      // longer all-or-nothing on one shared key).
      const keyId = llmKeyIdForModelId(id);
      return keyId ? [keyId] : [];
    }
    default:
      // ollama / lmstudio (key optional) + whisper-local
      return [];
  }
}

// Keychain key id for an LLM card's own API key (4b). null = key-optional local
// card (Ollama / LM Studio). OpenAI LLM reuses openai_api_key (same account as
// OpenAI Transcribe). Written into Settings.llm_api_key_id when the card is picked.
export function llmKeyIdForModelId(id: string): SecretKeyId | null {
  switch (id) {
    case "deepseek":
      return "deepseek_api_key";
    case "openai":
      return "openai_api_key";
    case "kimi":
      return "kimi_api_key";
    case "siliconflow":
      return "siliconflow_api_key";
    case "zhipu":
      return "zhipu_api_key";
    case "qwen":
      return "qwen_api_key";
    case "openrouter":
      return "openrouter_api_key";
    default:
      return null; // ollama / lmstudio — key optional
  }
}

// Local LLM cards whose key is optional (run against a localhost endpoint). They
// can be 选用 without a stored key, unlike cloud cards that gate on a secret.
export function isKeyOptionalModel(id: string): boolean {
  return id === "ollama" || id === "lmstudio";
}

// ── ASR ──────────────────────────────────────────────────────────────────────

// Design model id → backend ASR provider enum (null = card with no real slot).
// Picking doubao selects the streaming provider explicitly; the backend only
// activates streaming when asr_provider == "doubao_stream" AND a token is stored.
export function asrProviderForModelId(id: string): AsrProviderId | null {
  if (id === "groq") return "groq";
  if (id === "openai-asr") return "openai";
  if (id === "whisper-local") return "whisper_cpp";
  if (id === "macos-native") return "macos_native";
  if (id === "doubao") return "doubao_stream";
  if (id === "glm-asr") return "glm";
  if (id === "aliyun-asr") return "aliyun_fun";
  if (id === "stepfun-asr") return "stepfun";
  return null;
}

// Backend ASR provider → which catalog card should read as active on load.
export function modelIdForAsrProvider(provider: AsrProviderId): string {
  if (provider === "groq") return "groq";
  if (provider === "openai") return "openai-asr";
  if (provider === "whisper_cpp") return "whisper-local";
  if (provider === "macos_native") return "macos-native";
  if (provider === "glm") return "glm-asr";
  if (provider === "aliyun_fun") return "aliyun-asr";
  if (provider === "stepfun") return "stepfun-asr";
  return "doubao"; // doubao_stream + fallback
}

// Curated ASR model lists (front/back use the exact same id strings — see plan
// contract). Empty = no model choice for that card (doubao uses resource_id, not
// asr_model; whisper-local uses a file path). The first entry is the default.
export function asrModelOptionsForModelId(id: string): ModelOption[] {
  if (id === "groq") {
    return [
      { id: "whisper-large-v3-turbo", title: "whisper-large-v3-turbo" },
      { id: "whisper-large-v3", title: "whisper-large-v3" },
    ];
  }
  if (id === "openai-asr") {
    return [
      { id: "whisper-1", title: "whisper-1" },
      { id: "gpt-4o-transcribe", title: "GPT-4o Transcribe" },
      { id: "gpt-4o-mini-transcribe", title: "GPT-4o Mini Transcribe" },
    ];
  }
  if (id === "glm-asr") {
    return [
      { id: "glm-asr-1", title: "glm-asr-1" },
      { id: "glm-asr-2512", title: "glm-asr-2512" },
    ];
  }
  if (id === "aliyun-asr") {
    return [
      { id: "fun-asr-realtime", title: "Fun-ASR Realtime" },
      { id: "paraformer-realtime-v2", title: "Paraformer Realtime v2" },
    ];
  }
  if (id === "stepfun-asr") {
    return [
      { id: "stepaudio-2.5-asr", title: "Step-Audio 2.5 ASR" },
      { id: "stepaudio-2-asr-pro", title: "Step-Audio 2 ASR Pro" },
    ];
  }
  // doubao: variants map to resource_id (controlled separately), not asr_model.
  return [];
}

// ── LLM ──────────────────────────────────────────────────────────────────────

// Per-card endpoint preset. Only the official base_url is presets — the model is
// intentionally left empty: hardcoded model ids go stale ("一直在更新"), so picking
// a provider seeds no model. The user fetches the live /models list or types one.
export function llmPresetForModelId(id: string): { baseUrl: string; model: string } {
  switch (id) {
    case "openai":
      return { baseUrl: "https://api.openai.com/v1", model: "" };
    case "kimi":
      return { baseUrl: "https://api.moonshot.cn/v1", model: "" };
    case "siliconflow":
      return { baseUrl: "https://api.siliconflow.cn/v1", model: "" };
    case "zhipu":
      return { baseUrl: "https://open.bigmodel.cn/api/paas/v4", model: "" };
    case "qwen":
      return { baseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1", model: "" };
    case "openrouter":
      return { baseUrl: "https://openrouter.ai/api/v1", model: "" };
    case "ollama":
      return { baseUrl: "http://localhost:11434/v1", model: "" };
    case "lmstudio":
      return { baseUrl: "http://localhost:1234/v1", model: "" };
    default:
      return { baseUrl: "https://api.deepseek.com/v1", model: "" }; // deepseek
  }
}

// Settings patch for picking (选用) an LLM card. Switches to that provider's
// official endpoint + its own key slot, and — since all cards share one backend
// model slot — preserves the OUTGOING provider's model into llm_models and
// restores the INCOMING provider's stored model (empty if it was never
// configured, which the badge surfaces as 未配置). No hardcoded model id.
export function llmPickPatch(id: string, settings: Settings): Partial<Settings> {
  const preset = llmPresetForModelId(id);
  const models = { ...(settings.llm_models ?? {}) };
  const outgoing = llmModelIdForBaseUrl(settings.openai_compatible_base_url);
  if (outgoing && settings.openai_compatible_model) {
    models[outgoing] = settings.openai_compatible_model;
  }
  return {
    llm_provider: "openai_compatible",
    openai_compatible_base_url: preset.baseUrl,
    openai_compatible_model: models[id] ?? "",
    // 4b: point the backend at this provider's own key slot ("" = key-optional local).
    llm_api_key_id: llmKeyIdForModelId(id) ?? "",
    llm_models: models,
  };
}

// Settings patch for picking a specific auto-detected local LLM (A2 zero-click
// probe → 选用). Points the single openai_compatible slot at the discovered
// server's base_url + the exact model the user chose. Key-optional (localhost), and
// the model is remembered under the card id so 选用 can restore it. Used for the
// 本地 LLM rows the probe fills in — distinct from llmPickPatch, which restores a
// card's stored model rather than selecting a concrete live one.
export function discoveredLlmPickPatch(
  cardId: string,
  baseUrl: string,
  model: string,
  settings: Settings,
): Partial<Settings> {
  const models = { ...(settings.llm_models ?? {}) };
  const outgoing = llmModelIdForBaseUrl(settings.openai_compatible_base_url);
  if (outgoing && settings.openai_compatible_model) {
    models[outgoing] = settings.openai_compatible_model;
  }
  models[cardId] = model;
  return {
    llm_provider: "openai_compatible",
    openai_compatible_base_url: baseUrl,
    openai_compatible_model: model,
    llm_api_key_id: "", // key-optional localhost server
    llm_models: models,
  };
}

// Whether an LLM provider card has a stored model the user chose (ready to 选用).
// The active card's model lives in openai_compatible_model until it's switched away
// (then llmPickPatch preserves it into llm_models), so callers OR this with inUse.
export function llmHasStoredModel(id: string, settings: Settings): boolean {
  return (settings.llm_models?.[id] ?? "").trim() !== "";
}

// Hostname of a base_url, or "" if unparseable. Used to match a saved endpoint to
// a card by host (so trailing-slash / sub-path variants still highlight correctly).
function hostOfBaseUrl(baseUrl: string): string {
  try {
    return new URL(baseUrl.trim()).host;
  } catch {
    return "";
  }
}

// Which LLM card the saved openai_compatible base_url maps to, so the picker's
// 使用中 highlight persists across restarts (like ASR derives from asr_provider).
// Matches by host; an unrecognized/custom endpoint highlights nothing ("") rather
// than misleadingly claiming DeepSeek.
export function llmModelIdForBaseUrl(baseUrl: string): string {
  const host = hostOfBaseUrl(baseUrl);
  if (!host) return "";
  const match = MODELS.filter((m) => m.type === "llm").find(
    (m) => hostOfBaseUrl(llmPresetForModelId(m.id).baseUrl) === host,
  );
  return match?.id ?? "";
}
