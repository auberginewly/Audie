// Trigger-key recorder (P3.9). One control: click it and press any modifier combo
// (⌃⌥⇧⌘ + key) to set the trigger. The factory default is fn, which a browser can't
// capture as a key event — so "重置为 fn" restores it. The stored value IS the
// backend trigger string (parse_trigger is the gate). A recorded combo must carry a
// modifier, or a bare letter key would also type into the focused app (the tap is
// listen-only and doesn't swallow non-fn keys).

import { useEffect, useRef, useState } from "react";

import type { Hotkey } from "../../types/settings";
import { KeyCombo } from "../ui";

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

const RECORDER_BASE =
  "inline-flex min-h-8 items-center gap-1.5 rounded-sm border px-2.5 py-1 cursor-pointer transition-colors duration-150 ease-[var(--ease-out)]";

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

  const keys = value.split("+").map((k) => k.trim().toLowerCase());

  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-wrap items-center gap-2">
        <button
          ref={ref}
          type="button"
          onClick={() => {
            setRecording(true);
            setHint(null);
          }}
          onBlur={() => setRecording(false)}
          className={[
            RECORDER_BASE,
            recording ? "border-accent-fill bg-accent-bg" : "border-transparent bg-gray-200",
          ].join(" ")}
        >
          {recording ? (
            <span className="text-[13px] text-accent-text">按下快捷键…</span>
          ) : (
            <KeyCombo keys={keys} size="sm" />
          )}
        </button>
        {value !== "Fn" && !recording ? (
          <button
            type="button"
            className="text-xs text-text-tertiary hover:text-text-secondary cursor-pointer"
            onClick={() => {
              setHint(null);
              onChange("Fn" as Hotkey);
            }}
          >
            重置为 fn
          </button>
        ) : null}
      </div>
      {hint ? <span className="text-[11px] text-warning-text">{hint}</span> : null}
    </div>
  );
}
