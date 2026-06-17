// P1.1 provider configuration skeleton. This mirrors Rust settings and provider
// metadata only; key entry, provider testing, and runtime ASR switching are later
// P1 slices.

import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import {
  ProviderMetadataSchema,
  SettingsSchema,
  type AsrProviderId,
  type LlmProviderId,
  type ProviderMetadata,
  type Settings,
} from "../../types/settings";

const ProviderListSchema = ProviderMetadataSchema.array();

export function ProviderSettings() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [asrProviders, setAsrProviders] = useState<ProviderMetadata[]>([]);
  const [llmProviders, setLlmProviders] = useState<ProviderMetadata[]>([]);

  useEffect(() => {
    Promise.all([
      invoke("get_settings"),
      invoke("list_asr_providers"),
      invoke("list_llm_providers"),
    ])
      .then(([rawSettings, rawAsrProviders, rawLlmProviders]) => {
        const parsedSettings = SettingsSchema.safeParse(rawSettings);
        const parsedAsrProviders = ProviderListSchema.safeParse(rawAsrProviders);
        const parsedLlmProviders = ProviderListSchema.safeParse(rawLlmProviders);

        if (parsedSettings.success) setSettings(parsedSettings.data);
        if (parsedAsrProviders.success) setAsrProviders(parsedAsrProviders.data);
        if (parsedLlmProviders.success) setLlmProviders(parsedLlmProviders.data);
      })
      .catch((err) => console.error("load provider settings failed:", err));
  }, []);

  const updateSettings = async (patch: Partial<Settings>) => {
    try {
      const raw = await invoke("update_settings", { patch });
      const parsed = SettingsSchema.safeParse(raw);
      if (parsed.success) setSettings(parsed.data);
    } catch (err) {
      console.error("update provider settings failed:", err);
    }
  };

  if (!settings) return null;

  return (
    <section className="w-full max-w-md mx-auto space-y-3 text-left">
      <ProviderSelect
        label="ASR provider"
        value={settings.asr_provider}
        providers={asrProviders}
        onChange={(asr_provider) => updateSettings({ asr_provider })}
      />
      <ProviderSelect
        label="LLM provider"
        value={settings.llm_provider}
        providers={llmProviders}
        onChange={(llm_provider) => updateSettings({ llm_provider })}
      />
      <label className="flex items-center justify-between gap-3 text-sm">
        <span>AI 润色</span>
        <input
          type="checkbox"
          className="toggle toggle-sm"
          checked={settings.enhance_enabled}
          onChange={(event) => updateSettings({ enhance_enabled: event.target.checked })}
        />
      </label>
      <label className="form-control">
        <span className="label-text mb-1">润色 prompt</span>
        <textarea
          className="textarea textarea-bordered min-h-24 text-sm"
          value={settings.enhance_prompt}
          onChange={(event) => updateSettings({ enhance_prompt: event.target.value })}
        />
      </label>
    </section>
  );
}

function ProviderSelect<TProviderId extends AsrProviderId | LlmProviderId>({
  label,
  value,
  providers,
  onChange,
}: {
  label: string;
  value: TProviderId;
  providers: ProviderMetadata[];
  onChange: (value: TProviderId) => void;
}) {
  const selected = providers.find((provider) => provider.id === value);

  return (
    <label className="form-control">
      <span className="label-text mb-1">{label}</span>
      <select
        className="select select-bordered select-sm"
        value={value}
        onChange={(event) => onChange(event.target.value as TProviderId)}
      >
        {providers.map((provider) => (
          <option key={provider.id} value={provider.id}>
            {provider.title}
            {provider.default_model ? ` · ${provider.default_model}` : ""}
          </option>
        ))}
      </select>
      {selected ? (
        <span className="mt-1 flex flex-wrap gap-1">
          <span className="badge badge-ghost badge-sm">{selected.engine}</span>
          {selected.tags.map((tag) => (
            <span key={tag} className="badge badge-outline badge-sm">
              {tag}
            </span>
          ))}
        </span>
      ) : null}
    </label>
  );
}
