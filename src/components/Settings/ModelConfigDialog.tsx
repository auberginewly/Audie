// Model config dialog — opened from a model card's 配置 button. Body is driven by
// the model id (Doubao / Groq / DeepSeek·OpenAI / Whisper). Key, base_url, and
// model fields write to real backend commands; the rest (model variant, language,
// thinking, temperature, dictionary) are mock for visual fidelity (see plan).

import { useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";

import type { SecretKeyId, Settings } from "../../types/settings";
import type { UseSettings } from "../../hooks/useSettings";
import { Button, IconButton, Input, Select, StatusMessage, type StatusTone } from "../ui";
import type { ModelMeta } from "./models";

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-[7px]">
      <label className="text-[13px] text-text-secondary">{label}</label>
      {children}
    </div>
  );
}

// A keychain-backed password field that loads + saves on its own.
function KeyInput({ keyId, placeholder }: { keyId: SecretKeyId; placeholder: string }) {
  const [value, setValue] = useState("");
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    let cancelled = false;
    invoke("has_secret", { keyId })
      .then(async (raw) => {
        if (cancelled || typeof raw !== "boolean") return;
        setSaved(raw);
        if (!raw) return;
        const secret = await invoke("get_secret_for_settings", { keyId });
        if (!cancelled && typeof secret === "string") setValue(secret);
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
      placeholder={saved ? "已保存 key" : placeholder}
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
          <Button variant="secondary" icon="audio-lines" onClick={() => setStatus({ tone: "neutral", message: "测试为演示（见 plan）" })}>
            测试
          </Button>
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
