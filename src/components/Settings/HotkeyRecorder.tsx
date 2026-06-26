// Trigger-key recorder (P3.9). One box showing the current trigger (default fn).
// Click it → "按下快捷键…" → press any key to set it: combos (⌃⌥⇧⌘ + key) are
// captured here in the webview, while fn — which the webview can't see as a key
// event — is captured natively: begin_trigger_capture swaps the live trigger for a
// capture trigger that emits `trigger-record-fn` on an fn tap. The stored value IS
// the backend trigger string (parse_trigger is the gate). A recorded combo must
// carry a modifier, or a bare letter key would also type into the focused app.

import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

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

const BOX =
  "inline-flex min-h-8 min-w-[92px] items-center justify-center rounded-sm border px-2.5 py-1 cursor-pointer transition-colors duration-150 ease-[var(--ease-out)]";

type HotkeyRecorderProps = {
  value: Hotkey;
  onChange: (next: Hotkey) => void;
};

export function HotkeyRecorder({ value, onChange }: HotkeyRecorderProps) {
  const [recording, setRecording] = useState(false);
  const [hint, setHint] = useState<string | null>(null);
  const ref = useRef<HTMLButtonElement>(null);

  // End capture (restore the real trigger) and optionally apply the new key.
  const stop = useCallback(
    (next?: string) => {
      void invoke("end_trigger_capture").catch((err) => console.error("end capture failed:", err));
      setRecording(false);
      if (next) onChange(next as Hotkey);
    },
    [onChange],
  );

  useEffect(() => {
    if (!recording) return;
    ref.current?.focus();
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      if (e.key === "Escape" && !e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey) {
        stop(); // cancel
        return;
      }
      const res = eventToTrigger(e);
      if ("error" in res) {
        if (res.error) setHint(res.error);
        return;
      }
      setHint(null);
      stop(res.trigger);
    };
    window.addEventListener("keydown", onKey, true);
    const unlisten = listen("trigger-record-fn", () => {
      setHint(null);
      stop("Fn");
    });
    return () => {
      window.removeEventListener("keydown", onKey, true);
      void unlisten.then((f) => f());
    };
  }, [recording, stop]);

  const start = async () => {
    setHint(null);
    try {
      await invoke("begin_trigger_capture");
      setRecording(true);
    } catch {
      setHint("需要输入监控权限才能录制");
    }
  };

  const keys = value.split("+").map((k) => k.trim().toLowerCase());

  return (
    <div className="flex flex-col items-end gap-1">
      <button
        ref={ref}
        type="button"
        onClick={() => {
          if (!recording) void start();
        }}
        onBlur={() => {
          if (recording) stop();
        }}
        className={[
          BOX,
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
