// P0.5 — the most minimal hotkey picker: a preset dropdown. Reads/writes go
// through Rust commands (get_settings / update_settings); Rust owns the store
// and re-registers the global shortcut. No business logic here (§6.2).

import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import { SettingsSchema, HOTKEY_PRESETS, type Hotkey } from "../../types/settings";

// macOS keycaps read ⌥ Option, not "Alt" — show the symbols the user actually
// sees on the keyboard. The value stays the combo string global-shortcut needs.
const HOTKEY_LABELS: Record<Hotkey, string> = {
  "Ctrl+Shift+Space": "⌃ ⇧ Space",
  "Alt+Space": "⌥ Space",
  "Ctrl+Alt+Space": "⌃ ⌥ Space",
};

export function HotkeySettings() {
  const [hotkey, setHotkey] = useState<Hotkey | null>(null);

  useEffect(() => {
    invoke("get_settings")
      .then((raw) => {
        const parsed = SettingsSchema.safeParse(raw);
        if (parsed.success) setHotkey(parsed.data.hotkey);
      })
      .catch((err) => console.error("get_settings failed:", err));
  }, []);

  const onChange = async (next: Hotkey) => {
    try {
      const raw = await invoke("update_settings", { hotkey: next });
      const parsed = SettingsSchema.safeParse(raw);
      if (parsed.success) setHotkey(parsed.data.hotkey);
    } catch (err) {
      console.error("update_settings failed:", err);
    }
  };

  if (!hotkey) return null;

  return (
    <label className="form-control w-full max-w-xs mx-auto">
      <span className="label-text mb-1 text-left">快捷键</span>
      <select
        className="select select-bordered select-sm"
        value={hotkey}
        onChange={(e) => onChange(e.target.value as Hotkey)}
      >
        {HOTKEY_PRESETS.map((preset) => (
          <option key={preset} value={preset}>
            {HOTKEY_LABELS[preset]}
          </option>
        ))}
      </select>
    </label>
  );
}
