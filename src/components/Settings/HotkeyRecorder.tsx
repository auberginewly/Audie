// Click-to-record hotkey field (design's HotkeyRecorder). The backend only
// accepts three preset combos, so a recorded combo is saved only when it maps to
// one; anything else shows an honest hint and reverts (see plan risk note).

import { useEffect, useRef, useState } from "react";

import { HOTKEY_PRESETS, type Hotkey } from "../../types/settings";
import { KeyCombo } from "../ui";

const PRESET_SET = new Set<string>(HOTKEY_PRESETS);

// Build the backend hotkey string from a keydown event. Order matches the
// presets: Ctrl, Alt, Shift, then the main key (only Space is a valid preset key).
function eventToHotkey(e: KeyboardEvent): string | null {
  if (["Control", "Alt", "Shift", "Meta"].includes(e.key)) return null; // modifier alone
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  const main = e.key === " " ? "Space" : e.key.length === 1 ? e.key.toUpperCase() : e.key;
  parts.push(main);
  return parts.join("+");
}

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
      if (e.key === "Escape") {
        setRecording(false);
        return;
      }
      const combo = eventToHotkey(e);
      if (!combo) return; // still holding modifiers
      setRecording(false);
      if (PRESET_SET.has(combo)) {
        setHint(null);
        onChange(combo as Hotkey);
      } else {
        setHint("暂仅支持 ⌃⇧Space / ⌥Space / ⌃⌥Space");
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [recording, onChange]);

  const keys = value.split("+").map((k) => k.trim().toLowerCase());

  return (
    <div className="flex flex-col items-end gap-1">
      <button
        ref={ref}
        type="button"
        onClick={() => {
          setRecording(true);
          setHint(null);
        }}
        onBlur={() => setRecording(false)}
        className={[
          "inline-flex min-h-8 items-center gap-1.5 rounded-sm border px-2.5 py-1 outline-none",
          "transition-colors duration-150 ease-[var(--ease-out)] cursor-pointer",
          recording ? "border-accent-fill bg-accent-bg" : "border-transparent bg-gray-200",
        ].join(" ")}
      >
        {recording ? (
          <span className="text-[13px] text-accent-text">按下快捷键…</span>
        ) : (
          <KeyCombo keys={keys} size="sm" />
        )}
      </button>
      {hint ? <span className="text-[11px] text-warning-text">{hint}</span> : null}
    </div>
  );
}
