// 模型 — the design's model picker. Type tabs (ASR/LLM) + source filter + model
// cards. The active pick maps to the real provider enum where one exists; rating/
// tags/configured-status are mock (see models.ts + plan).

import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import type { UseSettings } from "../../hooks/useSettings";
import { Badge, Button, Icon, Segmented } from "../ui";
import { MODELS, asrProviderForModelId, modelIdForAsrProvider, type ModelMeta, type ModelType } from "./models";
import { ModelConfigDialog } from "./ModelConfigDialog";

type Source = "all" | "cloud" | "local";

function ModelCard({
  m,
  status,
  inUse,
  onPick,
  onConfigure,
}: {
  m: ModelMeta;
  status: ModelMeta["status"];
  inUse: boolean;
  onPick: () => void;
  onConfigure: () => void;
}) {
  return (
    <div className="flex items-center gap-3 rounded-md bg-surface-card px-3.5 py-[13px]">
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium text-text-primary">{m.name}</span>
          <Badge tone="neutral">{m.source === "local" ? "本地" : "云端"}</Badge>
          {inUse ? (
            <Badge tone="accent">使用中</Badge>
          ) : status === "configured" ? (
            <Badge tone="success">已配置</Badge>
          ) : (
            <Badge tone="neutral">未配置</Badge>
          )}
        </div>
        <div className="mt-[3px] font-mono text-[11px] text-text-tertiary">{m.model}</div>
      </div>
      {!inUse && status === "configured" ? (
        <Button size="sm" variant="secondary" onClick={onPick}>
          选用
        </Button>
      ) : null}
      <Button size="sm" variant="secondary" onClick={onConfigure}>
        配置
      </Button>
    </div>
  );
}

export function ModelSection({ data }: { data: UseSettings }) {
  const { settings, update } = data;
  const [type, setType] = useState<ModelType>("asr");
  const [source, setSource] = useState<Source>("all");
  const [configModel, setConfigModel] = useState<ModelMeta | null>(null);

  // Real "已配置" state for doubao: presence-check the streaming token (no-read,
  // never unlocks the keychain) so the badge can't claim configured when it isn't.
  // The mock `status` on every other card is visual-only (see models.ts).
  const [doubaoConfigured, setDoubaoConfigured] = useState<boolean | null>(null);
  const refreshDoubao = () => {
    invoke("has_secret", { keyId: "doubao_access_token" })
      .then((raw) => setDoubaoConfigured(typeof raw === "boolean" ? raw : false))
      .catch(() => {});
  };
  useEffect(() => {
    refreshDoubao();
  }, []);

  // Active pick: ASR derives from the real provider; LLM is visual (one slot).
  const [pickedLlm, setPickedLlm] = useState("deepseek");
  const pickedAsr = settings ? modelIdForAsrProvider(settings.asr_provider) : "doubao";
  const picked: Record<ModelType, string> = { asr: pickedAsr, llm: pickedLlm };

  // doubao's badge tracks the keychain; all others keep their mock status.
  const statusOf = (m: ModelMeta): ModelMeta["status"] =>
    m.id === "doubao" && doubaoConfigured !== null
      ? doubaoConfigured
        ? "configured"
        : "unconfigured"
      : m.status;

  const onPick = (m: ModelMeta) => {
    if (m.type === "asr") {
      const provider = asrProviderForModelId(m.id);
      // doubao → "doubao_stream"; the backend activates streaming only when this
      // is selected AND a token exists, otherwise it surfaces a Provider error.
      if (provider) update({ asr_provider: provider });
    } else {
      setPickedLlm(m.id);
      update({ llm_provider: "openai_compatible" });
    }
  };

  const list = MODELS.filter((m) => m.type === type && (source === "all" || m.source === source));

  return (
    <section className="mb-7">
      <div className="mb-3 flex items-start gap-2.5 pl-1">
        <Icon name="cpu" size={16} className="mt-px text-text-tertiary" />
        <h2 className="text-sm font-semibold leading-5 tracking-[-0.28px] text-text-primary">模型</h2>
      </div>

      <div className="mb-3 flex flex-wrap items-center gap-2.5">
        <Segmented
          value={type}
          onChange={setType}
          options={[
            { id: "asr", label: "ASR" },
            { id: "llm", label: "LLM" },
          ]}
        />
        <span className="h-[18px] w-px bg-border-subtle" />
        <Segmented
          value={source}
          onChange={setSource}
          options={[
            { id: "all", label: "全部" },
            { id: "cloud", label: "云端" },
            { id: "local", label: "本地" },
          ]}
        />
      </div>

      <div className="flex flex-col gap-2">
        {list.length ? (
          list.map((m) => (
            <ModelCard
              key={m.id}
              m={m}
              status={statusOf(m)}
              inUse={picked[m.type] === m.id}
              onPick={() => onPick(m)}
              onConfigure={() => setConfigModel(m)}
            />
          ))
        ) : (
          <div className="flex flex-col items-center gap-1.5 rounded-md bg-surface-card px-3.5 py-9 text-center">
            <Icon name="cpu" size={20} className="text-text-tertiary" />
            <span className="text-[13px] text-text-secondary">{source === "local" ? "暂无本地模型" : "没有匹配的模型"}</span>
            <span className="text-xs text-text-tertiary">
              {source === "local" ? "本地模型即将支持，敬请期待。" : "试试切换类型或来源。"}
            </span>
          </div>
        )}
      </div>

      <ModelConfigDialog
        model={configModel}
        data={data}
        onClose={() => {
          setConfigModel(null);
          refreshDoubao(); // a just-saved token should flip the badge to 已配置
        }}
      />
    </section>
  );
}
