// 模型 — the design's model picker. Type tabs (ASR/LLM) + source filter + model
// cards, grouped 云端 / 本地. The active pick maps to the real provider enum where
// one exists; configured-status is real (keychain has_secret via
// useConfiguredModels — see models.ts + plan).

import { useState } from "react";

import type { UseSettings } from "../../hooks/useSettings";
import { useConfiguredModels } from "../../hooks/useConfiguredModels";
import { Badge, Button, Icon, Segmented } from "../ui";
import {
  MODELS,
  asrProviderForModelId,
  isKeyOptionalModel,
  llmHasStoredModel,
  llmModelIdForBaseUrl,
  llmPickPatch,
  modelIdForAsrProvider,
  type ModelMeta,
  type ModelSource,
  type ModelType,
} from "./models";
import { ModelConfigDialog } from "./ModelConfigDialog";

type Source = "all" | "cloud" | "local";

function ModelCard({
  m,
  subtitle,
  usable,
  inUse,
  onPick,
  onConfigure,
}: {
  m: ModelMeta;
  subtitle: string;
  usable: boolean;
  inUse: boolean;
  onPick: () => void;
  onConfigure: () => void;
}) {
  // Status badge: 使用中 (picked) > 已配置 (usable: cloud key stored OR key-optional
  // local — both are ready to pick) > 未配置.
  const statusBadge = inUse ? (
    <Badge tone="accent">使用中</Badge>
  ) : usable ? (
    <Badge tone="success">已配置</Badge>
  ) : (
    <Badge tone="neutral">未配置</Badge>
  );

  return (
    <div className="flex items-center gap-3 rounded-md bg-surface-card px-3.5 py-[13px]">
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium text-text-primary">{m.name}</span>
          {statusBadge}
          {m.tags.map((t) => (
            <Badge key={t} tone="neutral">
              {t}
            </Badge>
          ))}
        </div>
        {subtitle ? (
          <div className="mt-[3px] font-mono text-[11px] text-text-tertiary">{subtitle}</div>
        ) : null}
      </div>
      {!inUse && usable ? (
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

export function ModelSection({
  data,
  type,
  onType,
}: {
  data: UseSettings;
  type: ModelType;
  onType: (t: ModelType) => void;
}) {
  const { settings, update } = data;
  const [source, setSource] = useState<Source>("all");
  const [configModel, setConfigModel] = useState<ModelMeta | null>(null);

  // Real "已配置" state from keychain has_secret (no-read presence check) for every
  // model, refreshed on focus + after the config dialog saves a key.
  const { configured, refresh } = useConfiguredModels();

  // Active pick derives from saved settings so the 使用中 highlight persists: ASR
  // from asr_provider, LLM from the openai_compatible base_url (deepseek/openai).
  const pickedAsr = settings ? modelIdForAsrProvider(settings.asr_provider) : "doubao";
  const pickedLlm = settings ? llmModelIdForBaseUrl(settings.openai_compatible_base_url) : "";
  const picked: Record<ModelType, string> = { asr: pickedAsr, llm: pickedLlm };

  const onPick = (m: ModelMeta) => {
    if (m.type === "asr") {
      const provider = asrProviderForModelId(m.id);
      // doubao → "doubao_stream"; the backend activates streaming only when this
      // is selected AND a token exists, otherwise it surfaces a Provider error.
      // Reset asr_model to "" so the new provider falls back to its built-in
      // default instead of inheriting the previous provider's model id.
      if (provider) update({ asr_provider: provider, asr_model: "" });
    } else if (settings) {
      update(llmPickPatch(m.id, settings));
    }
  };

  const list = MODELS.filter((m) => m.type === type && (source === "all" || m.source === source));
  // A model is usable (选用-able / 已配置) when ready to use:
  //  - ASR: its required key is stored.
  //  - LLM: it has a stored model the user chose AND (local OR its key is stored).
  //    Until configured, an LLM card reads 未配置 so the user picks a model first
  //    (选用 alone would leave the shared model slot empty → polish errors).
  const usableModel = (m: ModelMeta) => {
    if (m.type === "asr") return configured(m.id) || isKeyOptionalModel(m.id);
    if (!settings) return false;
    return llmHasStoredModel(m.id, settings) && (isKeyOptionalModel(m.id) || configured(m.id));
  };
  const groups: { source: ModelSource; label: string }[] = [
    { source: "cloud", label: "云端" },
    { source: "local", label: "本地" },
  ];

  return (
    <section className="mb-7">
      <div className="mb-3 flex items-start gap-2.5 pl-1">
        <Icon name="cpu" size={16} className="mt-px text-text-tertiary" />
        <h2 className="text-sm font-semibold leading-5 tracking-[-0.28px] text-text-primary">模型</h2>
      </div>

      <div className="mb-3 flex flex-wrap items-center gap-2.5">
        <Segmented
          value={type}
          onChange={onType}
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

      {list.length ? (
        <div className="flex flex-col gap-5">
          {groups.map(({ source: groupSource, label }) => {
            const cards = list.filter((m) => m.source === groupSource);
            if (!cards.length) return null;
            return (
              <div key={groupSource} className="flex flex-col gap-2">
                <div className="pl-1 text-xs font-medium uppercase tracking-wide text-text-tertiary">
                  {label}
                </div>
                {cards.map((m) => (
                  <ModelCard
                    key={m.id}
                    m={m}
                    // LLM: show this provider's own model under the name (like other
                    // cards) — the active card's live model, else its stored model.
                    // No hardcoded guess; empty (never configured) hides the line.
                    subtitle={
                      m.type === "llm"
                        ? picked.llm === m.id
                          ? (settings?.openai_compatible_model ?? "")
                          : (settings?.llm_models?.[m.id] ?? "")
                        : m.model
                    }
                    usable={usableModel(m)}
                    inUse={picked[m.type] === m.id}
                    onPick={() => onPick(m)}
                    onConfigure={() => setConfigModel(m)}
                  />
                ))}
              </div>
            );
          })}
        </div>
      ) : (
        <div className="flex flex-col items-center gap-1.5 rounded-md bg-surface-card px-3.5 py-9 text-center">
          <Icon name="cpu" size={20} className="text-text-tertiary" />
          <span className="text-[13px] text-text-secondary">{source === "local" ? "暂无本地模型" : "没有匹配的模型"}</span>
          <span className="text-xs text-text-tertiary">
            {source === "local" ? "本地模型即将支持，敬请期待。" : "试试切换类型或来源。"}
          </span>
        </div>
      )}

      <ModelConfigDialog
        model={configModel}
        data={data}
        onClose={() => {
          setConfigModel(null);
          refresh(); // a just-saved key should flip the badge to 已配置
        }}
      />
    </section>
  );
}
