// Settings data layer for the redesigned window. Owns load + patch against the
// Rust store; presentational sections stay logic-free (CLAUDE.md §6.2). Mirrors
// the contracts the old ProviderSettings drove (get_settings / update_settings /
// list_*_providers).

import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import {
  ProviderMetadataSchema,
  SettingsSchema,
  type ProviderMetadata,
  type Settings,
} from "../types/settings";

const ProviderListSchema = ProviderMetadataSchema.array();

export type UseSettings = {
  settings: Settings | null;
  asrProviders: ProviderMetadata[];
  llmProviders: ProviderMetadata[];
  update: (patch: Partial<Settings>) => Promise<void>;
  applyImported: (next: Settings) => void;
};

export function useSettings(): UseSettings {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [asrProviders, setAsrProviders] = useState<ProviderMetadata[]>([]);
  const [llmProviders, setLlmProviders] = useState<ProviderMetadata[]>([]);

  useEffect(() => {
    Promise.all([
      invoke("get_settings"),
      invoke("list_asr_providers"),
      invoke("list_llm_providers"),
    ])
      .then(([rawSettings, rawAsr, rawLlm]) => {
        const parsed = SettingsSchema.safeParse(rawSettings);
        const asr = ProviderListSchema.safeParse(rawAsr);
        const llm = ProviderListSchema.safeParse(rawLlm);
        if (parsed.success) setSettings(parsed.data);
        if (asr.success) setAsrProviders(asr.data);
        if (llm.success) setLlmProviders(llm.data);
      })
      .catch((err) => console.error("load settings failed:", err));
  }, []);

  const update = async (patch: Partial<Settings>) => {
    try {
      const raw = await invoke("update_settings", { patch });
      const parsed = SettingsSchema.safeParse(raw);
      if (parsed.success) setSettings(parsed.data);
    } catch (err) {
      console.error("update settings failed:", err);
    }
  };

  return { settings, asrProviders, llmProviders, update, applyImported: setSettings };
}
