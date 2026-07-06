import { useEffect, useState } from "react";

import type { Settings } from "../../types/settings";
import type { UseSettings } from "../../hooks/useSettings";
import { Button, IconButton, StatusMessage } from "../ui";
import { llmModelIdForBaseUrl, llmPresetForModelId, type ModelMeta, type ModelOption } from "./models";
import { useI18n } from "../../i18n";
import { ModelBody } from "./model-config/ModelBody";
import { NO_TEST_BUTTON_IDS } from "./model-config/constants";
import {
  listProviderModels,
  refreshModels as refreshModelList,
  runProviderTest,
  saveModelConfig,
  type ActionStatus,
  type LlmDraft,
} from "./model-config/modelConfigActions";

interface ModelConfigDialogProps {
  model: ModelMeta | null;
  data: UseSettings;
  onClose: () => void;
}

export function ModelConfigDialog({ model, data, onClose }: ModelConfigDialogProps) {
  const { t } = useI18n();
  const [status, setStatus] = useState<ActionStatus | null>(null);
  const { settings, update } = data;

  // LLM endpoint + model shown/edited in the dialog. All LLM cards share one
  // backend slot (openai_compatible), so we seed from the saved slot ONLY when
  // this card is the active provider; otherwise from the card's own preset — so
  // opening Kimi shows api.moonshot.cn, not whatever the shared slot holds.
  // Saving applies the draft (switches the slot to this provider), instead of
  // writing live on open (which would leave the new endpoint paired with the old
  // provider's key).
  const [llmDraft, setLlmDraft] = useState<LlmDraft>({ baseUrl: "", model: "" });
  // Live model list fetched from the provider's /models (null = use curated list).
  const [liveModels, setLiveModels] = useState<ModelOption[] | null>(null);
  const [modelFetch, setModelFetch] = useState<ActionStatus | null>(null);
  useEffect(() => {
    if (model?.type !== "llm" || !settings) return;
    const preset = llmPresetForModelId(model.id);
    const active = llmModelIdForBaseUrl(settings.openai_compatible_base_url) === model.id;
    setLlmDraft({
      baseUrl: active ? settings.openai_compatible_base_url : preset.baseUrl,
      // Leave the model blank for a not-yet-active card — no guessed/hardcoded
      // model. The user fetches the live list (获取最新模型) or types one in.
      model: active ? settings.openai_compatible_model : "",
    });
    setLiveModels(null); // dropping the previous card's fetched list
    setModelFetch(null);
    // Re-seed only when the opened card changes, not on every settings write.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [model?.id]);

  // Local providers (Ollama / LM Studio) need no key and run on a fixed localhost
  // endpoint, so auto-fetch their model list on open (and auto-select the first) —
  // the user just hits 保存. Cloud cards stay manual (need a key first).
  useEffect(() => {
    if (model?.type !== "llm" || model.source !== "local") return;
    let cancelled = false;
    const baseUrl = llmPresetForModelId(model.id).baseUrl;
    setModelFetch({ tone: "pending", message: t("settings.config.fetching") });
    listProviderModels(baseUrl, null)
      .then((models) => {
        if (cancelled) return;
        if (!models.length) {
          setModelFetch({ tone: "danger", message: t("settings.config.noLocalModels") });
          return;
        }
        setLiveModels(models);
        setLlmDraft((d) => (d.model.trim() ? d : { ...d, model: models[0]?.id ?? "" }));
        setModelFetch({ tone: "success", message: t("settings.config.modelsFetched", { count: models.length }) });
      })
      .catch((err) => {
        if (!cancelled) {
          const message = err instanceof Error ? err.message : t("settings.config.testFailed");
          setModelFetch({ tone: "danger", message });
        }
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [model?.id]);

  if (!model || !settings) return null;

  const save = async (): Promise<boolean> => {
    const result = await saveModelConfig({ model, llmDraft, settings, update, t });
    setStatus(result.status);
    return result.ok;
  };

  const refreshModels = async () => {
    setModelFetch({ tone: "pending", message: t("settings.config.fetching") });
    const result = await refreshModelList({ model, llmDraft, t });
    if (result.models) setLiveModels(result.models);
    if (result.firstModel) setLlmDraft((d) => (d.model.trim() ? d : { ...d, model: result.firstModel ?? "" }));
    setModelFetch(result.status);
  };

  const runTest = async () => {
    setStatus({ tone: "pending", message: t("settings.config.testing") });
    setStatus(await runProviderTest({ model, llmDraft, t }));
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
        onMouseDown={(e) => {
          e.stopPropagation();
        }}
        className="flex max-h-full w-[min(460px,100%)] flex-col overflow-hidden rounded-lg bg-surface-overlay shadow-modal"
      >
        <div className="flex shrink-0 items-center gap-2.5 px-[18px] pb-3.5 pt-[18px]">
          <div className="min-w-0 flex-1 text-base font-semibold tracking-[-0.32px] text-text-primary">
            {t("settings.config.configureTitle", { name: model.name })}
          </div>
          <IconButton name="x" label={t("settings.close")} onClick={onClose} />
        </div>

        <div className="flex flex-col gap-4 overflow-y-auto px-[18px] pb-1 pt-0.5">
          <ModelBody
            model={model}
            settings={settings}
            setField={setField}
            llmDraft={llmDraft}
            setLlmDraft={setLlmDraft}
            liveModels={liveModels}
            modelFetch={modelFetch}
            onRefreshModels={refreshModels}
            t={t}
          />
        </div>

        <div className="flex shrink-0 items-center gap-2 px-[18px] py-4">
          {!NO_TEST_BUTTON_IDS.has(model.id) ? (
            <Button variant="secondary" disabled={status?.tone === "pending"} onClick={runTest}>
              {t("settings.config.test")}
            </Button>
          ) : null}
          {status ? (
            <StatusMessage tone={status.tone} icon={null}>
              {status.message}
            </StatusMessage>
          ) : null}
          <div className="flex-1" />
          <Button variant="ghost" onClick={onClose}>
            {t("settings.config.cancel")}
          </Button>
          <Button
            variant="accent"
            onClick={async () => {
              if (await save()) onClose();
            }}
          >
            {t("settings.config.save")}
          </Button>
        </div>
      </div>
    </div>
  );
}
