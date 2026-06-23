// A keychain-backed secret field rebuilt on the design system. Handles
// has/get/set/delete against the Rust keychain commands, an optional
// test_provider probe, and optional base-url/model inputs (OpenAI-compatible).
// Same command contracts the old ProviderKeyField / DoubaoSecretField drove.

import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import {
  ProviderTestRequestSchema,
  ProviderTestResultSchema,
  type AsrProviderId,
  type LlmProviderId,
  type ProviderKind,
  type SecretKeyId,
  type TestProviderKeyId,
} from "../../types/settings";
import { Badge, Button, Input, StatusMessage, type StatusTone } from "../ui";

type TestConfig = {
  kind: ProviderKind;
  providerId: AsrProviderId | LlmProviderId;
  keyId: TestProviderKeyId;
};

type SecretFieldProps = {
  title: string;
  description?: string;
  secretKeyId: SecretKeyId;
  placeholder?: string;
  test?: TestConfig;
  baseUrl?: string;
  model?: string;
  onBaseUrlChange?: (value: string) => void;
  onModelChange?: (value: string) => void;
};

type Status = { tone: StatusTone; message: string };

export function SecretField({
  title,
  description,
  secretKeyId,
  placeholder,
  test,
  baseUrl,
  model,
  onBaseUrlChange,
  onModelChange,
}: SecretFieldProps) {
  const [apiKey, setApiKey] = useState("");
  const [hasSaved, setHasSaved] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isTesting, setIsTesting] = useState(false);
  const [status, setStatus] = useState<Status>({ tone: "neutral", message: "未检查" });

  useEffect(() => {
    let cancelled = false;
    invoke("has_secret", { keyId: secretKeyId })
      .then(async (raw) => {
        if (cancelled || typeof raw !== "boolean") return;
        setHasSaved(raw);
        if (!raw) {
          setStatus({ tone: "neutral", message: "未配置 key" });
          return;
        }
        const secret = await invoke("get_secret_for_settings", { keyId: secretKeyId });
        if (cancelled) return;
        if (typeof secret === "string" && secret.trim()) {
          setApiKey(secret);
          setStatus({ tone: "neutral", message: "已从 Keychain 载入" });
        } else {
          setStatus({ tone: "neutral", message: "已保存 key" });
        }
      })
      .catch((err) => {
        if (!cancelled) setStatus({ tone: "danger", message: errorMessage(err) });
      });
    return () => {
      cancelled = true;
    };
  }, [secretKeyId]);

  const save = async () => {
    const trimmed = apiKey.trim();
    if (!trimmed) {
      setStatus({ tone: "danger", message: "请先填写 API key" });
      return;
    }
    setIsSaving(true);
    try {
      await invoke("set_secret", { keyId: secretKeyId, value: trimmed });
      setHasSaved(true);
      setApiKey("");
      setStatus({ tone: "success", message: "key 已保存到系统 keychain" });
    } catch (err) {
      setStatus({ tone: "danger", message: errorMessage(err) });
    } finally {
      setIsSaving(false);
    }
  };

  const remove = async () => {
    setIsSaving(true);
    try {
      await invoke("delete_secret", { keyId: secretKeyId });
      setHasSaved(false);
      setApiKey("");
      setStatus({ tone: "neutral", message: "key 已删除" });
    } catch (err) {
      setStatus({ tone: "danger", message: errorMessage(err) });
    } finally {
      setIsSaving(false);
    }
  };

  const runTest = async () => {
    if (!test) return;
    const inline = apiKey.trim();
    if (!inline) {
      setStatus({ tone: "danger", message: "请先填写 API key" });
      return;
    }
    setIsTesting(true);
    setStatus({ tone: "pending", message: "正在测试 provider…" });
    try {
      const request = ProviderTestRequestSchema.safeParse({
        kind: test.kind,
        provider_id: test.providerId,
        key_id: test.keyId,
        api_key: inline,
        base_url: baseUrl !== undefined ? baseUrl.trim() : null,
      });
      if (!request.success) {
        setStatus({ tone: "danger", message: "provider 测试请求格式错误" });
        return;
      }
      const raw = await invoke("test_provider", { request: request.data });
      const parsed = ProviderTestResultSchema.safeParse(raw);
      if (parsed.success) {
        setStatus({ tone: parsed.data.ok ? "success" : "danger", message: parsed.data.message });
      } else {
        setStatus({ tone: "danger", message: "provider 测试响应格式错误" });
      }
    } catch (err) {
      setStatus({ tone: "danger", message: errorMessage(err) });
    } finally {
      setIsTesting(false);
    }
  };

  return (
    <div className="p-3.5">
      <div className="mb-2 flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="text-sm font-medium text-text-primary">{title}</div>
          {description ? <div className="mt-px text-xs text-text-tertiary">{description}</div> : null}
        </div>
        <Badge tone={hasSaved ? "success" : "neutral"} dot>
          {hasSaved ? "已保存" : "未保存"}
        </Badge>
      </div>

      {baseUrl !== undefined && onBaseUrlChange ? (
        <label className="mb-2 block">
          <span className="mb-1 block text-xs text-text-secondary">Base URL</span>
          <Input
            mono
            value={baseUrl}
            onChange={(e) => onBaseUrlChange(e.target.value)}
            placeholder="https://api.openai.com/v1"
          />
        </label>
      ) : null}
      {model !== undefined && onModelChange ? (
        <label className="mb-2 block">
          <span className="mb-1 block text-xs text-text-secondary">Model</span>
          <Input
            mono
            value={model}
            onChange={(e) => onModelChange(e.target.value)}
            placeholder="gpt-4o-mini"
          />
        </label>
      ) : null}

      <Input
        type="password"
        mono
        value={apiKey}
        onChange={(e) => setApiKey(e.target.value)}
        placeholder={hasSaved ? "已保存 key" : (placeholder ?? "sk-…")}
      />

      <div className="mt-2 flex items-center justify-between gap-2">
        <StatusMessage tone={status.tone}>{status.message}</StatusMessage>
        <div className="flex shrink-0 gap-2">
          {hasSaved ? (
            <Button size="sm" variant="ghost" onClick={remove} disabled={isSaving}>
              删除 Key
            </Button>
          ) : null}
          <Button size="sm" variant="secondary" onClick={save} disabled={isSaving}>
            保存
          </Button>
          {test ? (
            <Button size="sm" variant="accent" onClick={runTest} disabled={isSaving || isTesting}>
              {isTesting ? "测试中" : "测试连接"}
            </Button>
          ) : null}
        </div>
      </div>
    </div>
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
