// Doubao streaming ASR configuration (P2.4). Endpoint + resource id are plain
// store fields; AppID + Access Token are keychain secrets (key ids below),
// matching Voxt's "appID is sensitive" model. This slice does NOT connect — the
// WebSocket client + hot-path wiring land in P2.5/P2.6. Doubao deliberately does
// not appear in `list_asr_providers`; it only ever drives the streaming path.

import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import type { Settings, SecretKeyId } from "../../types/settings";

export function DoubaoSettings({
  settings,
  onChange,
}: {
  settings: Settings;
  onChange: (patch: Partial<Settings>) => void;
}) {
  return (
    <section className="space-y-2 rounded border border-base-300/70 p-3">
      <div>
        <h2 className="text-sm font-medium">豆包流式 ASR</h2>
        <p className="text-xs opacity-60">国内可直连的流式转写（实验性）</p>
      </div>

      <DoubaoSecretField title="AppID" keyId="doubao_app_id" placeholder="火山引擎 App ID" />
      <DoubaoSecretField
        title="Access Token"
        keyId="doubao_access_token"
        placeholder="火山引擎 Access Token"
      />

      <label className="form-control">
        <span className="label-text mb-1">Endpoint</span>
        <input
          className="input input-bordered input-sm font-mono"
          value={settings.doubao_endpoint}
          onChange={(event) => onChange({ doubao_endpoint: event.target.value })}
          placeholder="wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async"
        />
      </label>
      <label className="form-control">
        <span className="label-text mb-1">Resource ID</span>
        <input
          className="input input-bordered input-sm font-mono"
          value={settings.doubao_resource_id}
          onChange={(event) => onChange({ doubao_resource_id: event.target.value })}
          placeholder="volc.bigasr.sauc.duration"
        />
      </label>

      <label className="flex items-center justify-between gap-3 text-sm">
        <span>
          实验性流式预览
          <span className="block text-xs opacity-60">开启后录音并行送豆包（P2.6 起生效）</span>
        </span>
        <input
          type="checkbox"
          className="toggle toggle-sm"
          checked={settings.doubao_streaming_preview_enabled}
          onChange={(event) =>
            onChange({ doubao_streaming_preview_enabled: event.target.checked })
          }
        />
      </label>
    </section>
  );
}

type SecretStatus =
  | { tone: "neutral"; message: string }
  | { tone: "success"; message: string }
  | { tone: "error"; message: string };

// A keychain secret with no provider-test step (unlike P1's ProviderKeyField).
function DoubaoSecretField({
  title,
  keyId,
  placeholder,
}: {
  title: string;
  keyId: SecretKeyId;
  placeholder: string;
}) {
  const [value, setValue] = useState("");
  const [hasSaved, setHasSaved] = useState(false);
  const [isBusy, setIsBusy] = useState(false);
  const [status, setStatus] = useState<SecretStatus>({ tone: "neutral", message: "未检查" });

  useEffect(() => {
    let cancelled = false;

    invoke("has_secret", { keyId })
      .then(async (raw) => {
        if (cancelled || typeof raw !== "boolean") return;

        setHasSaved(raw);
        if (!raw) {
          setStatus({ tone: "neutral", message: "未配置" });
          return;
        }

        const secret = await invoke("get_secret_for_settings", { keyId });
        if (cancelled) return;

        if (typeof secret === "string" && secret.trim()) {
          setValue(secret);
          setStatus({ tone: "neutral", message: "已从 Keychain 载入" });
        } else {
          setStatus({ tone: "neutral", message: "已保存" });
        }
      })
      .catch((err) => {
        if (cancelled) return;
        setStatus({ tone: "error", message: errorMessage(err) });
      });

    return () => {
      cancelled = true;
    };
  }, [keyId]);

  const save = async () => {
    const trimmed = value.trim();
    if (!trimmed) {
      setStatus({ tone: "error", message: "请先填写内容" });
      return;
    }

    setIsBusy(true);
    try {
      await invoke("set_secret", { keyId, value: trimmed });
      setHasSaved(true);
      setValue("");
      setStatus({ tone: "success", message: "已保存到系统 keychain" });
    } catch (err) {
      setStatus({ tone: "error", message: errorMessage(err) });
    } finally {
      setIsBusy(false);
    }
  };

  const remove = async () => {
    setIsBusy(true);
    try {
      await invoke("delete_secret", { keyId });
      setHasSaved(false);
      setValue("");
      setStatus({ tone: "neutral", message: "已删除" });
    } catch (err) {
      setStatus({ tone: "error", message: errorMessage(err) });
    } finally {
      setIsBusy(false);
    }
  };

  const statusClass =
    status.tone === "success"
      ? "text-success"
      : status.tone === "error"
        ? "text-error"
        : "opacity-60";

  return (
    <label className="form-control">
      <span className="label-text mb-1 flex items-center gap-2">
        {title}
        <span className={`badge badge-sm ${hasSaved ? "badge-success" : "badge-ghost"}`}>
          {hasSaved ? "已保存" : "未保存"}
        </span>
      </span>
      <input
        className="input input-bordered input-sm font-mono"
        type="password"
        value={value}
        onChange={(event) => setValue(event.target.value)}
        placeholder={hasSaved ? "已保存" : placeholder}
      />
      <div className="mt-1 flex items-center justify-between gap-2">
        <p className={`min-w-0 text-xs ${statusClass}`}>{status.message}</p>
        <div className="flex shrink-0 gap-2">
          {hasSaved ? (
            <button className="btn btn-ghost btn-xs" onClick={remove} disabled={isBusy}>
              删除
            </button>
          ) : null}
          <button className="btn btn-outline btn-xs" onClick={save} disabled={isBusy}>
            保存
          </button>
        </div>
      </div>
    </label>
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
