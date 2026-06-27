// Model config dialog — opened from a model card's 配置 button. Body is driven by
// the model id (Doubao / Groq / DeepSeek·OpenAI / Whisper). Key, base_url, and
// model fields write to real backend commands; the rest (model variant, language,
// thinking, temperature, dictionary) are mock for visual fidelity (see plan).

import { useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";

import { ProviderTestResultSchema, type SecretKeyId, type Settings } from "../../types/settings";
import type { UseSettings } from "../../hooks/useSettings";
import { Button, IconButton, Input, Select, StatusMessage, type StatusTone } from "../ui";
import type { ModelMeta } from "./models";

// Tauri serializes AppError as { code, message } (error.rs serde tag/content), so
// a rejected command surfaces its user-facing message here.
function errorMessage(err: unknown): string {
  if (err && typeof err === "object" && "message" in err) {
    const m = (err as { message: unknown }).message;
    if (typeof m === "string" && m) return m;
  }
  return typeof err === "string" && err ? err : "测试失败，请查看日志";
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-[7px]">
      <label className="text-[13px] text-text-secondary">{label}</label>
      {children}
    </div>
  );
}

// A keychain-backed password field. Shows whether a key is already stored via the
// no-read `has_secret` presence check (which never asks macOS to unlock + reveal
// the secret), and only writes a NEW value through save(). We deliberately do NOT
// read the decrypted key back into the input: that data-read triggered a macOS
// Keychain password prompt every time a model config opened. Leaving an already-
// configured field blank keeps the stored key as-is (save() skips empty inputs).
function KeyInput({ keyId, placeholder }: { keyId: SecretKeyId; placeholder: string }) {
  const [value, setValue] = useState("");
  const [configured, setConfigured] = useState(false);

  useEffect(() => {
    let cancelled = false;
    invoke("has_secret", { keyId })
      .then((raw) => {
        if (!cancelled && typeof raw === "boolean") setConfigured(raw);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [keyId]);

  return (
    <Input
      mono
      type="password"
      value={value}
      onChange={(e) => setValue(e.target.value)}
      placeholder={configured ? "已配置（留空则保持不变）" : placeholder}
      data-key-id={keyId}
    />
  );
}

type ModelConfigDialogProps = {
  model: ModelMeta | null;
  data: UseSettings;
  onClose: () => void;
};

export function ModelConfigDialog({ model, data, onClose }: ModelConfigDialogProps) {
  const [status, setStatus] = useState<{ tone: StatusTone; message: string } | null>(null);
  const { settings, update } = data;
  if (!model || !settings) return null;

  // Persist every keychain field currently rendered (reads the DOM inputs the
  // KeyInput components own), plus base_url/model which update live.
  const save = async () => {
    const inputs = document.querySelectorAll<HTMLInputElement>("input[data-key-id]");
    try {
      for (const el of inputs) {
        const keyId = el.getAttribute("data-key-id");
        const val = el.value.trim();
        if (!keyId) continue;
        if (val) await invoke("set_secret", { keyId, value: val });
      }
      setStatus({ tone: "success", message: "已保存到系统 keychain" });
    } catch {
      setStatus({ tone: "danger", message: "保存失败，请查看日志" });
    }
  };

  // Live connection test (replaces the old mock). Doubao is WebSocket-only so it
  // routes to a dedicated command; the rest probe /models via test_provider. Keys
  // come from the visible input, else the saved keychain value (user-initiated, so
  // a keychain prompt is acceptable).
  const readKey = async (keyId: SecretKeyId): Promise<string | null> => {
    const el = document.querySelector<HTMLInputElement>(`input[data-key-id="${keyId}"]`);
    const typed = el?.value.trim();
    if (typed) return typed;
    try {
      const raw = await invoke("get_secret_for_settings", { keyId });
      return typeof raw === "string" && raw ? raw : null;
    } catch {
      return null;
    }
  };

  const runTest = async () => {
    setStatus({ tone: "pending", message: "测试中…" });
    const startedAt = performance.now();
    try {
      let raw: unknown;
      if (model.id === "doubao") {
        raw = await invoke("test_doubao_connection");
      } else if (model.id === "groq") {
        raw = await invoke("test_provider", {
          request: {
            kind: "asr",
            provider_id: "groq",
            key_id: "groq_api_key",
            api_key: await readKey("groq_api_key"),
            base_url: null,
          },
        });
      } else {
        raw = await invoke("test_provider", {
          request: {
            kind: "llm",
            provider_id: "openai_compatible",
            key_id: "openai_compatible_api_key",
            api_key: await readKey("openai_compatible_api_key"),
            base_url: settings.openai_compatible_base_url,
          },
        });
      }
      const ms = Math.round(performance.now() - startedAt);
      const parsed = ProviderTestResultSchema.safeParse(raw);
      const base = parsed.success ? parsed.data.message : "连接测试通过";
      setStatus({ tone: "success", message: `${base} · ${ms}ms` });
    } catch (err) {
      setStatus({ tone: "danger", message: errorMessage(err) });
    }
  };

  const setField = (patch: Partial<Settings>) => update(patch);

  return (
    <div
      onMouseDown={onClose}
      className="absolute inset-0 z-[90] flex items-center justify-center bg-black/55 p-8 backdrop-blur-[3px]"
    >
      <div
        role="dialog"
        aria-modal="true"
        onMouseDown={(e) => e.stopPropagation()}
        className="flex max-h-full w-[min(460px,100%)] flex-col overflow-hidden rounded-lg bg-surface-overlay shadow-modal"
      >
        <div className="flex shrink-0 items-center gap-2.5 px-[18px] pb-3.5 pt-[18px]">
          <div className="min-w-0 flex-1 text-base font-semibold tracking-[-0.32px] text-text-primary">
            配置 {model.name}
          </div>
          <IconButton name="x" label="关闭" onClick={onClose} />
        </div>

        <div className="flex flex-col gap-4 overflow-y-auto px-[18px] pb-1 pt-0.5">
          <ModelBody model={model} settings={settings} setField={setField} />
        </div>

        <div className="flex shrink-0 items-center gap-2 px-[18px] py-4">
          {model.id !== "whisper-local" ? (
            <Button variant="secondary" disabled={status?.tone === "pending"} onClick={runTest}>
              测试
            </Button>
          ) : null}
          {status ? <StatusMessage tone={status.tone}>{status.message}</StatusMessage> : null}
          <div className="flex-1" />
          <Button variant="ghost" onClick={onClose}>
            取消
          </Button>
          <Button
            variant="accent"
            onClick={async () => {
              await save();
              onClose();
            }}
          >
            保存
          </Button>
        </div>
      </div>
    </div>
  );
}

function ModelBody({
  model,
  settings,
  setField,
}: {
  model: ModelMeta;
  settings: Settings;
  setField: (patch: Partial<Settings>) => void;
}) {
  if (model.id === "doubao") {
    return (
      <>
        <Field label="模型">
          {/* mock: variant selection isn't backed */}
          <Select defaultValue="hourly">
            <option value="hourly">Doubao ASR 2.0 (Hourly)</option>
            <option value="concurrent">Doubao ASR 2.0 (Concurrent)</option>
          </Select>
        </Field>
        <Field label="App ID">
          <KeyInput keyId="doubao_app_id" placeholder="旧版控制台 App ID" />
        </Field>
        <Field label="Access Token">
          <KeyInput keyId="doubao_access_token" placeholder="粘贴 Access Token" />
        </Field>
        <Field label="Endpoint">
          <Input
            mono
            defaultValue={settings.doubao_endpoint}
            onChange={(e) => setField({ doubao_endpoint: e.target.value })}
            placeholder="wss://openspeech.bytedance.com/…"
          />
        </Field>
        <Field label="Resource ID">
          <Input
            mono
            defaultValue={settings.doubao_resource_id}
            onChange={(e) => setField({ doubao_resource_id: e.target.value })}
            placeholder="volc.seedasr.sauc.duration"
          />
        </Field>
      </>
    );
  }

  if (model.id === "groq") {
    return (
      <>
        <Field label="模型">
          <Select defaultValue="turbo">
            <option value="turbo">whisper-large-v3-turbo</option>
            <option value="v3">whisper-large-v3</option>
          </Select>
        </Field>
        <Field label="API Key">
          <KeyInput keyId="groq_api_key" placeholder="粘贴 API Key" />
        </Field>
        {/* mock: per-request language hint isn't backed */}
        <Field label="语言">
          <Select defaultValue="auto">
            <option value="auto">自动检测</option>
            <option value="zh">简体中文</option>
            <option value="en">English</option>
          </Select>
        </Field>
      </>
    );
  }

  if (model.id === "whisper-local") {
    return (
      <Field label="模型文件路径">
        <Input
          mono
          defaultValue={settings.whisper_cpp_model_path ?? ""}
          onChange={(e) => setField({ whisper_cpp_model_path: e.target.value || null })}
          placeholder="/path/to/ggml-large-v3.bin"
        />
      </Field>
    );
  }

  // deepseek / openai — both drive the single openai_compatible LLM slot.
  return (
    <>
      <Field label="端点">
        <Input
          mono
          defaultValue={settings.openai_compatible_base_url}
          onChange={(e) => setField({ openai_compatible_base_url: e.target.value })}
          placeholder="https://api.deepseek.com/v1"
        />
      </Field>
      <Field label="API Key">
        <KeyInput keyId="openai_compatible_api_key" placeholder="粘贴 API Key" />
      </Field>
      <Field label="模型">
        <Input
          mono
          defaultValue={settings.openai_compatible_model}
          onChange={(e) => setField({ openai_compatible_model: e.target.value })}
          placeholder="deepseek-chat"
        />
      </Field>
    </>
  );
}
