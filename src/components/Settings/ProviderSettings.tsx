// P1.1 provider configuration skeleton. This mirrors Rust settings and provider
// metadata only; key entry, provider testing, and runtime ASR switching are later
// P1 slices.

import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import {
  ProviderMetadataSchema,
  ProviderTestResultSchema,
  SettingsSchema,
  type AsrProviderId,
  type LlmProviderId,
  type ProviderKind,
  type ProviderMetadata,
  type Settings,
} from "../../types/settings";

const ProviderListSchema = ProviderMetadataSchema.array();

export function ProviderSettings() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [asrProviders, setAsrProviders] = useState<ProviderMetadata[]>([]);
  const [llmProviders, setLlmProviders] = useState<ProviderMetadata[]>([]);

  useEffect(() => {
    Promise.all([
      invoke("get_settings"),
      invoke("list_asr_providers"),
      invoke("list_llm_providers"),
    ])
      .then(([rawSettings, rawAsrProviders, rawLlmProviders]) => {
        const parsedSettings = SettingsSchema.safeParse(rawSettings);
        const parsedAsrProviders = ProviderListSchema.safeParse(rawAsrProviders);
        const parsedLlmProviders = ProviderListSchema.safeParse(rawLlmProviders);

        if (parsedSettings.success) setSettings(parsedSettings.data);
        if (parsedAsrProviders.success) setAsrProviders(parsedAsrProviders.data);
        if (parsedLlmProviders.success) setLlmProviders(parsedLlmProviders.data);
      })
      .catch((err) => console.error("load provider settings failed:", err));
  }, []);

  const updateSettings = async (patch: Partial<Settings>) => {
    try {
      const raw = await invoke("update_settings", { patch });
      const parsed = SettingsSchema.safeParse(raw);
      if (parsed.success) setSettings(parsed.data);
    } catch (err) {
      console.error("update provider settings failed:", err);
    }
  };

  if (!settings) return null;

  return (
    <section className="w-full max-w-md mx-auto space-y-3 text-left">
      <ProviderSelect
        label="ASR provider"
        value={settings.asr_provider}
        providers={asrProviders}
        onChange={(asr_provider) => updateSettings({ asr_provider })}
      />
      <ProviderSelect
        label="LLM provider"
        value={settings.llm_provider}
        providers={llmProviders}
        onChange={(llm_provider) => updateSettings({ llm_provider })}
      />
      <div className="divider my-1" />
      <ProviderKeyField
        title="Groq key"
        description="Groq ASR · whisper-large-v3-turbo"
        secretKeyId="groq_api_key"
        kind="asr"
        providerId="groq"
      />
      <ProviderKeyField
        title="OpenAI key"
        description="OpenAI ASR · whisper-1"
        secretKeyId="openai_api_key"
        kind="asr"
        providerId="openai"
      />
      <ProviderKeyField
        title="OpenAI-compatible key"
        description="LLM provider · base URL + key"
        secretKeyId="openai_compatible_api_key"
        kind="llm"
        providerId="openai_compatible"
        baseUrl={settings.openai_compatible_base_url}
        model={settings.openai_compatible_model}
        onBaseUrlChange={(openai_compatible_base_url) =>
          updateSettings({ openai_compatible_base_url })
        }
        onModelChange={(openai_compatible_model) => updateSettings({ openai_compatible_model })}
      />
      <div className="divider my-1" />
      <label className="flex items-center justify-between gap-3 text-sm">
        <span>AI 润色</span>
        <input
          type="checkbox"
          className="toggle toggle-sm"
          checked={settings.enhance_enabled}
          onChange={(event) => updateSettings({ enhance_enabled: event.target.checked })}
        />
      </label>
      <label className="form-control">
        <span className="label-text mb-1">润色 prompt</span>
        <textarea
          className="textarea textarea-bordered min-h-24 text-sm"
          value={settings.enhance_prompt}
          onChange={(event) => updateSettings({ enhance_prompt: event.target.value })}
        />
      </label>
    </section>
  );
}

type TestStatus =
  | { tone: "neutral"; message: string }
  | { tone: "success"; message: string }
  | { tone: "error"; message: string };

function ProviderKeyField({
  title,
  description,
  secretKeyId,
  kind,
  providerId,
  baseUrl,
  model,
  onBaseUrlChange,
  onModelChange,
}: {
  title: string;
  description: string;
  secretKeyId: string;
  kind: ProviderKind;
  providerId: AsrProviderId | LlmProviderId;
  baseUrl?: string;
  model?: string;
  onBaseUrlChange?: (value: string) => void;
  onModelChange?: (value: string) => void;
}) {
  const [apiKey, setApiKey] = useState("");
  const [hasSavedSecret, setHasSavedSecret] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isTesting, setIsTesting] = useState(false);
  const [status, setStatus] = useState<TestStatus>({
    tone: "neutral",
    message: "未检查",
  });

  useEffect(() => {
    invoke("has_secret", { keyId: secretKeyId })
      .then((raw) => {
        if (typeof raw === "boolean") {
          setHasSavedSecret(raw);
          setStatus({
            tone: "neutral",
            message: raw ? "已配置 key" : "未配置 key",
          });
        }
      })
      .catch((err) => {
        setStatus({ tone: "error", message: errorMessage(err) });
      });
  }, [secretKeyId]);

  const saveSecret = async () => {
    const trimmed = apiKey.trim();
    if (!trimmed) {
      setStatus({ tone: "error", message: "请先填写 API key" });
      return null;
    }

    setIsSaving(true);
    try {
      await invoke("set_secret", { keyId: secretKeyId, value: trimmed });
      setHasSavedSecret(true);
      setApiKey("");
      setStatus({ tone: "success", message: "key 已保存到系统 keychain" });
      return trimmed;
    } catch (err) {
      setStatus({ tone: "error", message: errorMessage(err) });
      return null;
    } finally {
      setIsSaving(false);
    }
  };

  const deleteSecret = async () => {
    setIsSaving(true);
    try {
      await invoke("delete_secret", { keyId: secretKeyId });
      setHasSavedSecret(false);
      setApiKey("");
      setStatus({ tone: "neutral", message: "key 已删除" });
    } catch (err) {
      setStatus({ tone: "error", message: errorMessage(err) });
    } finally {
      setIsSaving(false);
    }
  };

  const testProvider = async () => {
    if (apiKey.trim()) {
      const saved = await saveSecret();
      if (!saved) return;
    } else if (!hasSavedSecret) {
      setStatus({ tone: "error", message: "请先填写 API key" });
      return;
    }

    setIsTesting(true);
    setStatus({ tone: "neutral", message: "正在测试 provider..." });
    try {
      const raw = await invoke("test_provider", {
        request: {
          kind,
          provider_id: providerId,
          key_id: secretKeyId,
          base_url: baseUrl !== undefined ? baseUrl.trim() : null,
        },
      });
      const parsed = ProviderTestResultSchema.safeParse(raw);
      if (parsed.success) {
        setStatus({
          tone: parsed.data.ok ? "success" : "error",
          message: parsed.data.message,
        });
      } else {
        setStatus({ tone: "error", message: "provider 测试响应格式错误" });
      }
    } catch (err) {
      setStatus({ tone: "error", message: errorMessage(err) });
    } finally {
      setIsTesting(false);
    }
  };

  const statusClass =
    status.tone === "success"
      ? "text-success"
      : status.tone === "error"
        ? "text-error"
        : "opacity-60";

  return (
    <section className="space-y-2 rounded border border-base-300/70 p-3">
      <div className="flex items-start justify-between gap-3">
        <div>
          <h2 className="text-sm font-medium">{title}</h2>
          <p className="text-xs opacity-60">{description}</p>
        </div>
        <span className={`badge badge-sm ${hasSavedSecret ? "badge-success" : "badge-ghost"}`}>
          {hasSavedSecret ? "已保存" : "未保存"}
        </span>
      </div>
      {baseUrl !== undefined && onBaseUrlChange ? (
        <label className="form-control">
          <span className="label-text mb-1">Base URL</span>
          <input
            className="input input-bordered input-sm"
            value={baseUrl}
            onChange={(event) => onBaseUrlChange(event.target.value)}
            placeholder="https://api.openai.com/v1"
          />
        </label>
      ) : null}
      {model !== undefined && onModelChange ? (
        <label className="form-control">
          <span className="label-text mb-1">Model</span>
          <input
            className="input input-bordered input-sm"
            value={model}
            onChange={(event) => onModelChange(event.target.value)}
            placeholder="gpt-4o-mini"
          />
        </label>
      ) : null}
      <label className="form-control">
        <span className="label-text mb-1">API key</span>
        <input
          className="input input-bordered input-sm font-mono"
          type="password"
          value={apiKey}
          onChange={(event) => setApiKey(event.target.value)}
          placeholder={hasSavedSecret ? "输入新 key 可覆盖" : "sk-..."}
        />
      </label>
      <div className="flex items-center justify-between gap-2">
        <p className={`min-w-0 text-xs ${statusClass}`}>{status.message}</p>
        <div className="flex shrink-0 gap-2">
          {hasSavedSecret ? (
            <button className="btn btn-ghost btn-xs" onClick={deleteSecret} disabled={isSaving}>
              删除
            </button>
          ) : null}
          <button className="btn btn-outline btn-xs" onClick={saveSecret} disabled={isSaving}>
            保存
          </button>
          <button
            className="btn btn-primary btn-xs"
            onClick={testProvider}
            disabled={isSaving || isTesting}
          >
            {isTesting ? "测试中" : "测试"}
          </button>
        </div>
      </div>
    </section>
  );
}

function errorMessage(err: unknown): string {
  if (typeof err === "string") return err;
  if (err && typeof err === "object" && "message" in err) {
    const message = (err as { message?: unknown }).message;
    if (typeof message === "string") return message;
  }
  return "操作失败，请查看日志";
}

function ProviderSelect<TProviderId extends AsrProviderId | LlmProviderId>({
  label,
  value,
  providers,
  onChange,
}: {
  label: string;
  value: TProviderId;
  providers: ProviderMetadata[];
  onChange: (value: TProviderId) => void;
}) {
  const selected = providers.find((provider) => provider.id === value);

  return (
    <label className="form-control">
      <span className="label-text mb-1">{label}</span>
      <select
        className="select select-bordered select-sm"
        value={value}
        onChange={(event) => onChange(event.target.value as TProviderId)}
      >
        {providers.map((provider) => (
          <option key={provider.id} value={provider.id}>
            {provider.title}
            {provider.default_model ? ` · ${provider.default_model}` : ""}
          </option>
        ))}
      </select>
      {selected ? (
        <span className="mt-1 flex flex-wrap gap-1">
          <span className="badge badge-ghost badge-sm">{selected.engine}</span>
          {selected.tags.map((tag) => (
            <span key={tag} className="badge badge-outline badge-sm">
              {tag}
            </span>
          ))}
        </span>
      ) : null}
    </label>
  );
}
