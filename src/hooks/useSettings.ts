// Settings data layer for the redesigned window. Owns load + patch against the
// Rust store; presentational sections stay logic-free (CLAUDE.md §6.2). Mirrors
// the contracts the old ProviderSettings drove (get_settings / update_settings /
// list_*_providers).

import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import {
  AudioDeviceSchema,
  AutoDeviceSchema,
  ProviderMetadataSchema,
  SettingsSchema,
  type AudioDevice,
  type ProviderMetadata,
  type Settings,
} from "../types/settings";

const ProviderListSchema = ProviderMetadataSchema.array();
const MicrophoneListSchema = AudioDeviceSchema.array();

export type UseSettings = {
  settings: Settings | null;
  asrProviders: ProviderMetadata[];
  llmProviders: ProviderMetadata[];
  microphones: AudioDevice[];
  autoDevice: string | null;
  update: (patch: Partial<Settings>) => Promise<void>;
};

export function useSettings(): UseSettings {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [asrProviders, setAsrProviders] = useState<ProviderMetadata[]>([]);
  const [llmProviders, setLlmProviders] = useState<ProviderMetadata[]>([]);
  const [microphones, setMicrophones] = useState<AudioDevice[]>([]);
  const [autoDevice, setAutoDevice] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([
      invoke("get_settings"),
      invoke("list_asr_providers"),
      invoke("list_llm_providers"),
      invoke("list_microphones"),
      invoke("auto_input_device"),
    ])
      .then(([rawSettings, rawAsr, rawLlm, rawMics, rawAuto]) => {
        const parsed = SettingsSchema.safeParse(rawSettings);
        const asr = ProviderListSchema.safeParse(rawAsr);
        const llm = ProviderListSchema.safeParse(rawLlm);
        const mics = MicrophoneListSchema.safeParse(rawMics);
        const auto = AutoDeviceSchema.safeParse(rawAuto);
        if (parsed.success) setSettings(parsed.data);
        else console.error("settings parse failed (load):", parsed.error);
        if (asr.success) setAsrProviders(asr.data);
        if (llm.success) setLlmProviders(llm.data);
        if (mics.success) setMicrophones(mics.data);
        if (auto.success) setAutoDevice(auto.data);
      })
      .catch((err) => console.error("load settings failed:", err));
  }, []);

  const update = async (patch: Partial<Settings>) => {
    try {
      const raw = await invoke("update_settings", { patch });
      const parsed = SettingsSchema.safeParse(raw);
      if (parsed.success) setSettings(parsed.data);
      else console.error("settings parse failed (update):", parsed.error);
    } catch (err) {
      console.error("update settings failed:", err);
    }
  };

  return {
    settings,
    asrProviders,
    llmProviders,
    microphones,
    autoDevice,
    update,
  };
}
