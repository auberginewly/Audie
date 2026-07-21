import type { ReactNode } from "react";

import type { Settings } from "../../../types/settings";
import type { I18nContextValue } from "../../../i18n";
import { Input, StatusMessage, type StatusTone } from "../../ui";
import {
  asrModelOptionsForModelId,
  llmKeyIdForModelId,
  llmPresetForModelId,
  requiredSecretsForModel,
  type ModelMeta,
  type ModelOption,
} from "../models";
import { GENERIC_ASR_MODEL_IDS } from "./constants";
import { KeyInput } from "./KeyInput";
import { OptionSelect } from "./OptionSelect";
import { RecommendedLocalModels } from "./RecommendedLocalModels";
import type { LlmDraft } from "./modelConfigActions";

function asrEndpointField(
  modelId: string,
): keyof Pick<Settings, "glm_endpoint" | "aliyun_endpoint" | "stepfun_endpoint"> | null {
  if (modelId === "glm-asr") return "glm_endpoint";
  if (modelId === "aliyun-asr") return "aliyun_endpoint";
  if (modelId === "stepfun-asr") return "stepfun_endpoint";
  return null;
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-[7px]">
      <label className="text-[13px] text-text-secondary">{label}</label>
      {children}
    </div>
  );
}

interface ModelBodyProps {
  model: ModelMeta;
  settings: Settings;
  setField: (patch: Partial<Settings>) => void;
  llmDraft: LlmDraft;
  setLlmDraft: (next: LlmDraft | ((current: LlmDraft) => LlmDraft)) => void;
  liveModels: ModelOption[] | null;
  modelFetch: { tone: StatusTone; message: string } | null;
  onRefreshModels: () => void;
  t: I18nContextValue["t"];
}

export function ModelBody({
  model,
  settings,
  setField,
  llmDraft,
  setLlmDraft,
  liveModels,
  modelFetch,
  onRefreshModels,
  t,
}: ModelBodyProps) {
  if (model.id === "doubao") {
    return (
      <>
        <Field label="App ID">
          <KeyInput keyId="doubao_app_id" placeholder={t("settings.config.appIdPlaceholder")} />
        </Field>
        <Field label="Access Token">
          <KeyInput keyId="doubao_access_token" placeholder={t("settings.config.tokenPlaceholder")} />
        </Field>
        <Field label="Endpoint">
          <Input
            mono
            defaultValue={settings.doubao_endpoint}
            onChange={(e) => {
              setField({ doubao_endpoint: e.target.value });
            }}
            placeholder="wss://openspeech.bytedance.com/…"
          />
        </Field>
        <Field label="Resource ID">
          <Input
            mono
            defaultValue={settings.doubao_resource_id}
            onChange={(e) => {
              setField({ doubao_resource_id: e.target.value });
            }}
            placeholder="volc.seedasr.sauc.duration"
          />
        </Field>
      </>
    );
  }

  if (GENERIC_ASR_MODEL_IDS.has(model.id)) {
    const keyId = requiredSecretsForModel(model.id)[0];
    const options = asrModelOptionsForModelId(model.id);
    const endpointField = asrEndpointField(model.id);
    return (
      <>
        <Field label={t("settings.config.model")}>
          <OptionSelect
            options={options}
            value={settings.asr_model}
            placeholder={options[0]?.id ?? ""}
            onChange={(asr_model) => {
              setField({ asr_model });
            }}
            customLabel={t("settings.config.custom")}
          />
        </Field>
        {endpointField ? (
          <Field label={t("settings.config.endpoint")}>
            <Input
              mono
              defaultValue={settings[endpointField]}
              onChange={(e) => {
                setField({ [endpointField]: e.target.value });
              }}
            />
          </Field>
        ) : null}
        <Field label="API Key">
          <KeyInput keyId={keyId} placeholder={t("settings.config.apiKeyPlaceholder")} />
        </Field>
      </>
    );
  }

  const preset = llmPresetForModelId(model.id);
  const llmKeyId = llmKeyIdForModelId(model.id);
  return (
    <>
      <Field label={t("settings.config.endpoint")}>
        <Input
          mono
          value={llmDraft.baseUrl}
          onChange={(e) => {
            setLlmDraft({ ...llmDraft, baseUrl: e.target.value });
          }}
          placeholder={preset.baseUrl}
        />
      </Field>
      {llmKeyId ? (
        <Field label="API Key">
          <KeyInput keyId={llmKeyId} placeholder={t("settings.config.apiKeyPlaceholder")} />
        </Field>
      ) : null}
      <div className="flex flex-col gap-[7px]">
        <div className="flex items-center justify-between gap-2">
          <label className="text-[13px] text-text-secondary">{t("settings.config.model")}</label>
          <button
            type="button"
            disabled={modelFetch?.tone === "pending"}
            onClick={onRefreshModels}
            className="text-[12px] text-accent hover:underline disabled:opacity-50"
          >
            {t("settings.config.fetchModels")}
          </button>
        </div>
        {liveModels && liveModels.length ? (
          <OptionSelect
            options={liveModels}
            value={llmDraft.model}
            placeholder={t("settings.config.chooseModel")}
            onChange={(value) => {
              setLlmDraft({ ...llmDraft, model: value });
            }}
            customLabel={t("settings.config.custom")}
          />
        ) : (
          <Input
            mono
            value={llmDraft.model}
            onChange={(e) => {
              setLlmDraft({ ...llmDraft, model: e.target.value });
            }}
            placeholder={
              llmKeyId ? t("settings.config.modelPlaceholderWithKey") : t("settings.config.modelPlaceholderNoKey")
            }
          />
        )}
        {modelFetch ? (
          <StatusMessage tone={modelFetch.tone} icon={null}>
            {modelFetch.message}
          </StatusMessage>
        ) : null}
      </div>
      {model.source === "local" ? (
        <RecommendedLocalModels
          onPick={(tag) => {
            setLlmDraft({ ...llmDraft, model: tag });
          }}
        />
      ) : null}
    </>
  );
}
