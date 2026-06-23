// 配置 — import/export. Export redacts every key to "<keychain>"; import writes
// only non-sensitive settings and reports which keys to refill. Same commands
// the old ConfigImportExport drove (export_config / import_config).

import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import {
  ExportedConfigSchema,
  ImportConfigResultSchema,
  type ExportedConfig,
  type SecretKeyId,
  type Settings,
} from "../../types/settings";
import { Button, StatusMessage, Textarea, type StatusTone } from "../ui";
import { SettingSection } from "./SettingSection";

type Status = { tone: StatusTone; message: string };

export function ConfigSection({ onImported }: { onImported: (settings: Settings) => void }) {
  const [text, setText] = useState("");
  const [isBusy, setIsBusy] = useState(false);
  const [status, setStatus] = useState<Status>({ tone: "neutral", message: "导出不会包含 API key 明文" });

  const exportConfig = async () => {
    setIsBusy(true);
    try {
      const raw = await invoke("export_config");
      const parsed = ExportedConfigSchema.safeParse(raw);
      if (!parsed.success) {
        setStatus({ tone: "danger", message: "导出配置响应格式错误" });
        return;
      }
      const json = JSON.stringify(parsed.data, null, 2);
      setText(json);
      await copyText(json);
      setStatus({ tone: "success", message: "配置已导出，key 已用 <keychain> 占位" });
    } catch (err) {
      setStatus({ tone: "danger", message: errorMessage(err) });
    } finally {
      setIsBusy(false);
    }
  };

  const importConfig = async () => {
    let json: unknown;
    try {
      json = JSON.parse(text);
    } catch {
      setStatus({ tone: "danger", message: "配置 JSON 格式不正确" });
      return;
    }
    const parsed = ExportedConfigSchema.safeParse(json);
    if (!parsed.success) {
      setStatus({ tone: "danger", message: "配置内容不符合 Audie 导出格式" });
      return;
    }
    setIsBusy(true);
    try {
      const raw = await invoke("import_config", { config: parsed.data satisfies ExportedConfig });
      const result = ImportConfigResultSchema.safeParse(raw);
      if (!result.success) {
        setStatus({ tone: "danger", message: "导入配置响应格式错误" });
        return;
      }
      onImported(result.data.settings);
      const refill = keyLabels(result.data.keys_to_refill);
      setStatus({
        tone: "success",
        message: refill ? `配置已导入，请重新填写 ${refill} key` : result.data.message,
      });
    } catch (err) {
      setStatus({ tone: "danger", message: errorMessage(err) });
    } finally {
      setIsBusy(false);
    }
  };

  return (
    <SettingSection
      icon="arrow-down-up"
      title="配置导入导出"
      description="只导入非敏感设置，API key 需要重新填写"
      action={
        <div className="flex gap-2">
          <Button size="sm" variant="secondary" onClick={exportConfig} disabled={isBusy}>
            导出配置
          </Button>
          <Button size="sm" variant="accent" onClick={importConfig} disabled={isBusy}>
            导入配置
          </Button>
        </div>
      }
    >
      <div className="p-3.5">
        <Textarea
          mono
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder="导出后会显示 JSON；也可以粘贴配置 JSON 后导入"
          className="min-h-28"
        />
        <div className="mt-2">
          <StatusMessage tone={status.tone}>{status.message}</StatusMessage>
        </div>
      </div>
    </SettingSection>
  );
}

function keyLabels(keyIds: SecretKeyId[]): string {
  if (keyIds.length === 0) return "";
  return keyIds
    .map((keyId) => {
      switch (keyId) {
        case "groq_api_key":
          return "Groq";
        case "openai_api_key":
          return "OpenAI";
        case "openai_compatible_api_key":
          return "OpenAI-compatible";
        case "doubao_app_id":
          return "豆包 AppID";
        case "doubao_access_token":
          return "豆包 API Key / Access Token";
      }
    })
    .join(" / ");
}

async function copyText(text: string) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    // Best-effort; the textarea still holds the JSON.
  }
}

function errorMessage(err: unknown): string {
  if (typeof err === "string") return err;
  if (err && typeof err === "object" && "message" in err) {
    const message = (err as { message?: unknown }).message;
    if (typeof message === "string") return message;
  }
  return "操作失败，请查看日志";
}
