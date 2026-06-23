// 服务商 — ASR/LLM provider selection + per-provider keys + Doubao streaming.
// Every control maps to a real Rust command; no unbacked model-picker mock.

import {
  type AsrProviderId,
  type LlmProviderId,
  type ProviderMetadata,
  type Settings,
} from "../../types/settings";
import { Badge, Input, Select } from "../ui";
import { SettingSection, SettingRow } from "./SettingSection";
import { SecretField } from "./SecretField";

type ProviderSectionProps = {
  settings: Settings;
  asrProviders: ProviderMetadata[];
  llmProviders: ProviderMetadata[];
  update: (patch: Partial<Settings>) => void;
};

export function ProviderSection({ settings, asrProviders, llmProviders, update }: ProviderSectionProps) {
  return (
    <>
      <SettingSection icon="cpu" title="服务商" description="选择转写与润色引擎，配置各自的 key">
        <SettingRow
          label="ASR 引擎"
          divider={false}
          control={
            <div className="w-56">
              <Select
                value={settings.asr_provider}
                onChange={(e) => update({ asr_provider: e.target.value as AsrProviderId })}
              >
                {asrProviders.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.title}
                    {p.default_model ? ` · ${p.default_model}` : ""}
                  </option>
                ))}
              </Select>
            </div>
          }
        />
        <ProviderTags providers={asrProviders} active={settings.asr_provider} />
        <SettingRow
          label="LLM 引擎"
          control={
            <div className="w-56">
              <Select
                value={settings.llm_provider}
                onChange={(e) => update({ llm_provider: e.target.value as LlmProviderId })}
              >
                {llmProviders.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.title}
                    {p.default_model ? ` · ${p.default_model}` : ""}
                  </option>
                ))}
              </Select>
            </div>
          }
        />
        <ProviderTags providers={llmProviders} active={settings.llm_provider} />
      </SettingSection>

      <SettingSection icon="key" title="API Key" description="key 存系统钥匙串，绝不写入配置文件">
        <SecretField
          title="Groq"
          description="Groq ASR · whisper-large-v3-turbo"
          secretKeyId="groq_api_key"
          test={{ kind: "asr", providerId: "groq", keyId: "groq_api_key" }}
        />
        <div className="h-px bg-border-subtle" />
        <SecretField
          title="OpenAI"
          description="OpenAI ASR · whisper-1"
          secretKeyId="openai_api_key"
          test={{ kind: "asr", providerId: "openai", keyId: "openai_api_key" }}
        />
        <div className="h-px bg-border-subtle" />
        <SecretField
          title="OpenAI-compatible"
          description="LLM 润色 · base URL + key + model"
          secretKeyId="openai_compatible_api_key"
          test={{ kind: "llm", providerId: "openai_compatible", keyId: "openai_compatible_api_key" }}
          baseUrl={settings.openai_compatible_base_url}
          model={settings.openai_compatible_model}
          onBaseUrlChange={(openai_compatible_base_url) => update({ openai_compatible_base_url })}
          onModelChange={(openai_compatible_model) => update({ openai_compatible_model })}
        />
      </SettingSection>

      <SettingSection icon="audio-lines" title="豆包流式 ASR" description="保存 API Key 后自动用于流式；未配置时走批量 ASR">
        <SecretField
          title="AppID"
          description="旧版控制台 App ID；新版留空"
          secretKeyId="doubao_app_id"
          placeholder="旧版控制台 App ID"
        />
        <div className="h-px bg-border-subtle" />
        <SecretField
          title="API Key / Access Token"
          description="新版控制台 API Key；旧版控制台 Access Token"
          secretKeyId="doubao_access_token"
          placeholder="新版 API Key 或旧版 Access Token"
        />
        <div className="h-px bg-border-subtle" />
        <SettingRow
          label="Endpoint"
          control={
            <div className="w-64">
              <Input
                mono
                value={settings.doubao_endpoint}
                onChange={(e) => update({ doubao_endpoint: e.target.value })}
                placeholder="wss://openspeech.bytedance.com/…"
              />
            </div>
          }
        />
        <SettingRow
          label="Resource ID"
          control={
            <div className="w-64">
              <Input
                mono
                value={settings.doubao_resource_id}
                onChange={(e) => update({ doubao_resource_id: e.target.value })}
                placeholder="volc.seedasr.sauc.duration"
              />
            </div>
          }
        />
      </SettingSection>
    </>
  );
}

function ProviderTags({
  providers,
  active,
}: {
  providers: ProviderMetadata[];
  active: string;
}) {
  const selected = providers.find((p) => p.id === active);
  if (!selected) return null;
  return (
    <div className="flex flex-wrap items-center gap-1.5 px-3.5 pb-3">
      <Badge tone="neutral">{selected.engine}</Badge>
      {selected.tags.map((tag) => (
        <Badge key={tag} tone="neutral">
          {tag}
        </Badge>
      ))}
    </div>
  );
}
