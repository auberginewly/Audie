// 模型 — the model picker. Type tabs (ASR/LLM) + source filter + model cards,
// grouped 云端 / 本地. Cloud cards map to the real provider enum; configured-status
// is real (keychain has_secret via useConfiguredModels).
//
// 本地 is LLM-only: provider cards (Ollama / LM Studio) with 配置 (endpoint/model +
// per-RAM recommendations), plus a zero-click probe (discover_local_llm) that merges
// any running server's live models into its card for one-click 选用 — NO scan button.
// Local ASR was removed (whisper.cpp / model downloader / macOS native): ASR is
// cloud-only now, so the 本地 group never shows for the ASR tab.

import { useEffect, useMemo, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";

import type { UseSettings } from "../../hooks/useSettings";
import { useConfiguredModels } from "../../hooks/useConfiguredModels";
import { DiscoveredLocalLlmSchema, type DiscoveredLocalLlm } from "../../types/settings";
import { Badge, Button, Icon, Segmented } from "../ui";
import {
  MODELS,
  asrProviderForModelId,
  discoveredLlmPickPatch,
  isKeyOptionalModel,
  llmHasStoredModel,
  llmModelIdForBaseUrl,
  llmPickPatch,
  modelIdForAsrProvider,
  type ModelMeta,
  type ModelType,
} from "./models";
import { ModelConfigDialog } from "./ModelConfigDialog";
import { LocalLlmCard } from "./LocalLlmCard";

type Source = "all" | "cloud" | "local";

// Human label for an auto-detected local-LLM server (probe provider id → card name).
const LOCAL_LLM_LABEL: Record<string, string> = {
  ollama: "Ollama",
  lmstudio: "LM Studio",
  llamacpp: "llama.cpp",
};

function CloudModelCard({
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
  // Status badge: 使用中 (picked) > 已配置 (usable: cloud key stored) > 未配置.
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

// A discovered local-LLM server with no catalog card (e.g. llama.cpp): its live
// models with one-click 选用. Card-backed servers (Ollama / LM Studio) merge into
// their own card instead (LocalLlmCard).
function DiscoveredLlmCard({
  server,
  activeBaseUrlHost,
  activeModel,
  onPick,
}: {
  server: DiscoveredLocalLlm;
  activeBaseUrlHost: string;
  activeModel: string;
  onPick: (model: string) => void;
}) {
  const serverHost = hostOf(server.base_url);
  return (
    <div className="flex flex-col gap-2 rounded-md bg-surface-card px-3.5 py-[13px]">
      <div className="flex items-center gap-2">
        <span className="text-sm font-medium text-text-primary">
          {LOCAL_LLM_LABEL[server.provider] ?? server.provider}
        </span>
        <Badge tone="success">已运行</Badge>
        <Badge tone="neutral">本地</Badge>
      </div>
      <div className="flex flex-col gap-1.5">
        {server.models.map((model) => {
          const inUse = serverHost === activeBaseUrlHost && model === activeModel;
          return (
            <div key={model} className="flex items-center gap-3">
              <span className="min-w-0 flex-1 truncate font-mono text-[12px] text-text-secondary">
                {model}
              </span>
              {inUse ? (
                <Badge tone="accent">使用中</Badge>
              ) : (
                <Button size="sm" variant="secondary" onClick={() => onPick(model)}>
                  选用
                </Button>
              )}
            </div>
          );
        })}
      </div>
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

  // Real "已配置" state from keychain has_secret (no-read presence check), refreshed
  // on focus + after the config dialog saves a key.
  const { configured, refresh } = useConfiguredModels();

  // Zero-click local-LLM auto-detect (A2): probe known local servers when the LLM tab
  // opens, so the 本地 LLM list fills itself — NO button.
  const [discovered, setDiscovered] = useState<DiscoveredLocalLlm[]>([]);
  useEffect(() => {
    if (type !== "llm") return;
    let cancelled = false;
    invoke("discover_local_llm")
      .then((raw) => {
        if (cancelled) return;
        const parsed = DiscoveredLocalLlmSchema.array().safeParse(raw);
        if (parsed.success) setDiscovered(parsed.data);
      })
      .catch((err) => console.error("discover local llm failed:", err));
    return () => {
      cancelled = true;
    };
  }, [type]);

  // Active pick derives from saved settings so the 使用中 highlight persists.
  const pickedAsr = settings ? modelIdForAsrProvider(settings.asr_provider) : "doubao";
  const pickedLlm = settings ? llmModelIdForBaseUrl(settings.openai_compatible_base_url) : "";
  const picked: Record<ModelType, string> = { asr: pickedAsr, llm: pickedLlm };
  const activeBaseUrlHost = settings ? hostOf(settings.openai_compatible_base_url) : "";
  const activeModel = settings?.openai_compatible_model ?? "";

  const onPickCloud = (m: ModelMeta) => {
    if (m.type === "asr") {
      const provider = asrProviderForModelId(m.id);
      // doubao → "doubao_stream"; reset asr_model so the new provider uses its
      // built-in default instead of the previous provider's model id.
      if (provider) update({ asr_provider: provider, asr_model: "" });
    } else if (settings) {
      update(llmPickPatch(m.id, settings));
    }
  };

  const onPickDiscoveredLlm = (server: DiscoveredLocalLlm, model: string) => {
    if (settings) update(discoveredLlmPickPatch(server.provider, server.base_url, model, settings));
  };

  const cloudCards = useMemo(
    () => MODELS.filter((m) => m.type === type && m.source === "cloud"),
    [type],
  );
  // Local LLM provider cards (Ollama / LM Studio): always rendered so the user can
  // 配置 endpoint/model (and see the per-RAM recommendations) even when no server is
  // running yet. The auto-detected running models merge into each card.
  const localLlmCards = useMemo(
    () => (type === "llm" ? MODELS.filter((m) => m.type === "llm" && m.source === "local") : []),
    [type],
  );

  const usableModel = (m: ModelMeta) => {
    if (m.type === "asr") return configured(m.id);
    if (!settings) return false;
    return llmHasStoredModel(m.id, settings) && (isKeyOptionalModel(m.id) || configured(m.id));
  };

  // 本地 is LLM-only now (local ASR removed).
  const showCloud = source === "all" || source === "cloud";
  const showLocal = source === "all" || source === "local";
  const localHasContent = type === "llm" && (localLlmCards.length > 0 || discovered.length > 0);
  const anyContent = (showCloud && cloudCards.length > 0) || (showLocal && localHasContent);

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

      {anyContent ? (
        <div className="flex flex-col gap-5">
          {showCloud && cloudCards.length ? (
            <div className="flex flex-col gap-2">
              <GroupLabel>云端</GroupLabel>
              {cloudCards.map((m) => (
                <CloudModelCard
                  key={m.id}
                  m={m}
                  subtitle={
                    m.type === "llm"
                      ? picked.llm === m.id
                        ? (settings?.openai_compatible_model ?? "")
                        : (settings?.llm_models?.[m.id] ?? "")
                      : m.model
                  }
                  usable={usableModel(m)}
                  inUse={picked[m.type] === m.id}
                  onPick={() => onPickCloud(m)}
                  onConfigure={() => setConfigModel(m)}
                />
              ))}
            </div>
          ) : null}

          {showLocal && localHasContent ? (
            <div className="flex flex-col gap-2">
              <GroupLabel>本地</GroupLabel>
              {localLlmCards.map((m) => {
                // Merge the matched running server INTO the provider card (its live
                // models list inline), instead of a separate 运行中 row.
                const server = discovered.find((d) => d.provider === m.id) ?? null;
                return (
                  <LocalLlmCard
                    key={m.id}
                    name={m.name}
                    isActive={picked.llm === m.id}
                    activeModel={settings?.openai_compatible_model ?? ""}
                    storedModel={settings?.llm_models?.[m.id] ?? ""}
                    usable={usableModel(m)}
                    server={server}
                    onPickStored={() => onPickCloud(m)}
                    onPickModel={(model) => server && onPickDiscoveredLlm(server, model)}
                    onConfigure={() => setConfigModel(m)}
                  />
                );
              })}
              {/* Discovered servers with no catalog card (e.g. llama.cpp) have
                  nothing to merge into — keep them standalone. */}
              {discovered
                .filter((d) => !localLlmCards.some((c) => c.id === d.provider))
                .map((server) => (
                  <DiscoveredLlmCard
                    key={server.provider}
                    server={server}
                    activeBaseUrlHost={activeBaseUrlHost}
                    activeModel={activeModel}
                    onPick={(model) => onPickDiscoveredLlm(server, model)}
                  />
                ))}
            </div>
          ) : null}
        </div>
      ) : (
        <div className="flex flex-col items-center gap-1.5 rounded-md bg-surface-card px-3.5 py-9 text-center">
          <Icon name="cpu" size={20} className="text-text-tertiary" />
          <span className="text-[13px] text-text-secondary">
            {source === "local" ? "暂无本地模型" : "没有匹配的模型"}
          </span>
          <span className="text-xs text-text-tertiary">
            {source === "local"
              ? type === "asr"
                ? "本地 ASR 暂不支持，请使用云端 ASR。"
                : "启动 Ollama / LM Studio 后会自动出现在这里。"
              : "试试切换类型或来源。"}
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

function GroupLabel({ children }: { children: ReactNode }) {
  return (
    <div className="pl-1 text-xs font-medium uppercase tracking-wide text-text-tertiary">
      {children}
    </div>
  );
}

// Hostname of a base_url, or "" if unparseable — used to match the active LLM
// endpoint against a discovered server (host-level so trailing-slash variants match).
function hostOf(baseUrl: string): string {
  try {
    return new URL(baseUrl.trim()).host;
  } catch {
    return "";
  }
}
