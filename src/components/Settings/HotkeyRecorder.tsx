// Trigger-key picker (P3.9). Quick chips for the bare keys a browser can't record
// (fn — the default — plus F13/F14 that most keyboards lack physically), and a
// "record custom" button that captures any modifier combo. The stored value IS the
// backend trigger string (parse_trigger is the gate). A recorded combo must carry a
// modifier, or a bare letter key would also type into the focused app (the tap is
// listen-only and doesn't swallow non-fn keys).

import { useEffect, useRef, useState } from "react";

import type { Hotkey } from "../../types/settings";
import { KeyCombo } from "../ui";

const CHIPS: { value: string; keys: string[] }[] = [
  { value: "Fn", keys: ["fn"] },
  { value: "F13", keys: ["f13"] },
  { value: "F14", keys: ["f14"] },
];
const CHIP_VALUES = new Set(CHIPS.map((c) => c.value));

const FUNCTION_KEY = /^F([1-9]|1[0-9]|20)$/;

// Map a browser key to the backend's key name (keycode_for in macos.rs), or null
// when it isn't a key a trigger can use.
function mainKeyName(e: KeyboardEvent): string | null {
  const k = e.key;
  if (k === " ") return "Space";
  if (["Enter", "Tab", "Escape"].includes(k)) return k;
  if (k.startsWith("Arrow")) return k.slice(5); // ArrowLeft -> Left
  if (FUNCTION_KEY.test(k)) return k;
  if (/^[a-zA-Z]$/.test(k)) return k.toUpperCase();
  if (/^[0-9]$/.test(k)) return k;
  return null;
}

// Build a backend trigger string from a keydown, or an error to show (empty string
// = still holding modifiers, wait for the main key).
function eventToTrigger(e: KeyboardEvent): { trigger: string } | { error: string } {
  if (["Control", "Alt", "Shift", "Meta"].includes(e.key)) return { error: "" };
  const mods: string[] = [];
  if (e.ctrlKey) mods.push("Ctrl");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  if (e.metaKey) mods.push("Cmd");
  const main = mainKeyName(e);
  if (!main) return { error: "不支持这个键，换一个" };
  if (mods.length === 0 && !FUNCTION_KEY.test(main)) {
    return { error: "请配合 ⌃ ⌥ ⇧ ⌘ 一起按（单个键会被打出来）" };
  }
  return { trigger: [...mods, main].join("+") };
}

const CHIP_BASE =
  "inline-flex min-h-8 items-center rounded-sm border px-2.5 py-1 cursor-pointer transition-colors duration-150 ease-[var(--ease-out)]";

type HotkeyRecorderProps = {
  value: Hotkey;
  onChange: (next: Hotkey) => void;
};

export function HotkeyRecorder({ value, onChange }: HotkeyRecorderProps) {
  const [recording, setRecording] = useState(false);
  const [hint, setHint] = useState<string | null>(null);
  const ref = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!recording) return;
    ref.current?.focus();
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      const res = eventToTrigger(e);
      if ("error" in res) {
        if (res.error) setHint(res.error);
        return;
      }
      setRecording(false);
      setHint(null);
      onChange(res.trigger as Hotkey);
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [recording, onChange]);

  const isCustom = !CHIP_VALUES.has(value);
  const customKeys = value.split("+").map((k) => k.trim().toLowerCase());

  const cls = (active: boolean) =>
    [CHIP_BASE, active ? "border-accent-fill bg-accent-bg" : "border-transparent bg-gray-200"].join(" ");

  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-wrap items-center gap-1.5">
        {CHIPS.map((c) => (
          <button
            key={c.value}
            type="button"
            aria-pressed={c.value === value}
            onClick={() => {
              setHint(null);
              setRecording(false);
              onChange(c.value as Hotkey);
            }}
            className={cls(c.value === value)}
          >
            <KeyCombo keys={c.keys} size="sm" />
          </button>
        ))}
        <button
          ref={ref}
          type="button"
          aria-pressed={isCustom}
          onClick={() => {
            setRecording(true);
            setHint(null);
          }}
          onBlur={() => setRecording(false)}
          className={[CHIP_BASE, "gap-1.5", recording || isCustom ? "border-accent-fill bg-accent-bg" : "border-transparent bg-gray-200"].join(" ")}
        >
          {recording ? (
            <span className="text-[13px] text-accent-text">按下快捷键…</span>
          ) : isCustom ? (
            <KeyCombo keys={customKeys} size="sm" />
          ) : (
            <span className="text-[13px] text-text-secondary">＋ 自定义</span>
          )}
        </button>
      </div>
      {hint ? <span className="text-[11px] text-warning-text">{hint}</span> : null}
    </div>
  );
}
