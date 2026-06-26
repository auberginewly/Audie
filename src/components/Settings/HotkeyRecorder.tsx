// Trigger-key picker (P3.9). A curated set of good triggers as keycap chips:
// fn (default) + function keys that don't collide with typing + modifier combos.
// Browsers can't capture fn and most keyboards lack F13–F15 physically, so a
// curated picker beats free-form recording here — and the backend only accepts
// keys parse_trigger knows. The chip's value IS the backend trigger string.

import type { Hotkey } from "../../types/settings";
import { KeyCombo } from "../ui";

type TriggerOption = { value: string; keys: string[] };

const TRIGGER_OPTIONS: TriggerOption[] = [
  { value: "Fn", keys: ["fn"] },
  { value: "F13", keys: ["f13"] },
  { value: "F14", keys: ["f14"] },
  { value: "Ctrl+Shift+Space", keys: ["ctrl", "shift", "space"] },
  { value: "Alt+Space", keys: ["alt", "space"] },
  { value: "Ctrl+Alt+Space", keys: ["ctrl", "alt", "space"] },
];

type HotkeyRecorderProps = {
  value: Hotkey;
  onChange: (next: Hotkey) => void;
};

export function HotkeyRecorder({ value, onChange }: HotkeyRecorderProps) {
  return (
    <div className="flex flex-wrap items-center gap-1.5">
      {TRIGGER_OPTIONS.map((opt) => {
        const active = opt.value === value;
        return (
          <button
            key={opt.value}
            type="button"
            onClick={() => onChange(opt.value)}
            aria-pressed={active}
            className={[
              "inline-flex min-h-8 items-center rounded-sm border px-2.5 py-1",
              "transition-colors duration-150 ease-[var(--ease-out)] cursor-pointer",
              active ? "border-accent-fill bg-accent-bg" : "border-transparent bg-gray-200",
            ].join(" ")}
          >
            <KeyCombo keys={opt.keys} size="sm" />
          </button>
        );
      })}
    </div>
  );
}
