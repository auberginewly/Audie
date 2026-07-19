import { invoke } from "@tauri-apps/api/core";

import { ProviderTestResultSchema, type SecretKeyId, type Settings } from "../../../types/settings";
import type { UseSettings } from "../../../hooks/useSettings";
import type { StatusTone } from "../../ui";
import type { I18nContextValue } from "../../../i18n";
import { llmKeyIdForModelId, type ModelMeta, type ModelOption } from "../models";

export interface LlmDraft {
  baseUrl: string;
  model: string;
}

export interface ActionStatus {
  tone: StatusTone;
  message: string;
}

export function errorMessage(err: unknown, fallback: string): string {
  if (err && typeof err === "object" && "message" in err) {
    const m = err.message;
    if (typeof m === "string" && m) return m;
  }
  return typeof err === "string" && err ? err : fallback;
}

function readDraftKey(keyId: SecretKeyId): string | null {
  const el = document.querySelector<HTMLInputElement>(`input[data-key-id="${keyId}"]`);
  const typed = el?.value.trim() ?? "";
  return typed.length > 0 ? typed : null;
}

export async function listProviderModels(
  baseUrl: string,
  apiKey: string | null,
  keyId: SecretKeyId | null = null,
): Promise<ModelOption[]> {
  const raw = await invoke("list_provider_models", { baseUrl, apiKey, keyId });
  const ids = Array.isArray(raw) ? raw.filter((x): x is string => typeof x === "string") : [];
  return ids.map((id) => ({ id, title: id }));
}

export async function refreshModels({
  model,
  llmDraft,
  t,
}: {
  model: ModelMeta;
  llmDraft: LlmDraft;
  t: I18nContextValue["t"];
}): Promise<{ models: ModelOption[] | null; status: ActionStatus; firstModel?: string }> {
  try {
    const keyId = llmKeyIdForModelId(model.id);
    const apiKey = keyId ? readDraftKey(keyId) : null;
    const models = await listProviderModels(llmDraft.baseUrl, apiKey, keyId);
    if (!models.length) {
      return { models: null, status: { tone: "danger", message: t("settings.config.noModelsReturned") } };
    }
    return {
      models,
      firstModel: models[0]?.id,
      status: { tone: "success", message: t("settings.config.modelsFetched", { count: models.length }) },
    };
  } catch (err) {
    return {
      models: null,
      status: { tone: "danger", message: errorMessage(err, t("settings.config.testFailed")) },
    };
  }
}

export async function saveModelConfig({
  model,
  llmDraft,
  settings,
  update,
  t,
}: {
  model: ModelMeta;
  llmDraft: LlmDraft;
  settings: Settings;
  update: UseSettings["update"];
  t: I18nContextValue["t"];
}): Promise<{ ok: boolean; status: ActionStatus }> {
  if (model.type === "llm" && !llmDraft.model.trim()) {
    return { ok: false, status: { tone: "danger", message: t("settings.config.modelRequired") } };
  }

  const inputs = document.querySelectorAll<HTMLInputElement>("input[data-key-id]");
  try {
    if (model.type === "llm") {
      void update({
        llm_provider: "openai_compatible",
        openai_compatible_base_url: llmDraft.baseUrl.trim(),
        openai_compatible_model: llmDraft.model.trim(),
        llm_api_key_id: llmKeyIdForModelId(model.id) ?? "",
        llm_models: { ...settings.llm_models, [model.id]: llmDraft.model.trim() },
      });
    }

    let savedKey = false;
    for (const el of inputs) {
      const keyId = el.getAttribute("data-key-id");
      const val = el.value.trim();
      if (!keyId || !val) continue;
      await invoke("set_secret", { keyId, value: val });
      savedKey = true;
    }

    return {
      ok: true,
      status: {
        tone: "success",
        message: savedKey ? t("settings.config.savedKeychain") : t("settings.config.saved"),
      },
    };
  } catch {
    return { ok: false, status: { tone: "danger", message: t("settings.config.saveFailed") } };
  }
}

export async function runProviderTest({
  model,
  llmDraft,
  t,
}: {
  model: ModelMeta;
  llmDraft: LlmDraft;
  t: I18nContextValue["t"];
}): Promise<ActionStatus> {
  const startedAt = performance.now();
  try {
    let raw: unknown;
    if (model.id === "doubao") {
      raw = await invoke("test_doubao_connection");
    } else if (model.id === "openai-asr") {
      raw = await invoke("test_provider", {
        request: {
          kind: "asr",
          provider_id: "openai",
          key_id: "openai_api_key",
          api_key: readDraftKey("openai_api_key"),
          base_url: null,
        },
      });
    } else {
      const llmKeyId = llmKeyIdForModelId(model.id);
      raw = await invoke("test_provider", {
        request: {
          kind: "llm",
          provider_id: "openai_compatible",
          key_id: llmKeyId ?? "openai_compatible_api_key",
          api_key: llmKeyId ? readDraftKey(llmKeyId) : null,
          base_url: llmDraft.baseUrl,
        },
      });
    }

    const ms = Math.round(performance.now() - startedAt);
    const parsed = ProviderTestResultSchema.safeParse(raw);
    const base = parsed.success ? parsed.data.message : t("settings.config.testPassed");
    return { tone: "success", message: `${base} · ${ms}ms` };
  } catch (err) {
    return { tone: "danger", message: errorMessage(err, t("settings.config.testFailed")) };
  }
}
