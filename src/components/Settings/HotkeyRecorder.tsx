// Trigger-key recorder (P3.9). One box showing the current trigger (default fn).
// Click it → "按下快捷键…" → press any key to set it. Anything goes (no forced
// modifier): a bare modifier tap (fn / shift / ctrl / alt / cmd), a single key
// (F13, a letter…), or a combo (⌃⌥⇧⌘ + key). fn — which the webview can't see as a
// key event — is captured natively: begin_trigger_capture swaps the live trigger
// for a capture trigger that emits `trigger-record-fn` on an fn tap. Other modifiers
// are caught here via their keyup (a clean tap with no other key in between). The
// stored value IS the backend trigger string (parse_trigger is the gate). A bare
// typing key (letter/space) will also type into apps when pressed — the user's call.

import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type { Hotkey } from "../../types/settings";
import { KeyCombo } from "../ui";

const FUNCTION_KEY = /^F([1-9]|1[0-9]|20)$/;

// Browser modifier key → backend modifier name. A clean tap of one of these (down →
// up with nothing in between) is a bare-modifier trigger.
const MOD_NAMES: Record<string, string> = {
  Control: "Ctrl",
  Alt: "Alt",
  Shift: "Shift",
  Meta: "Cmd",
};

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

// Build a backend trigger string from a non-modifier keydown (combo or bare key).
function eventToTrigger(e: KeyboardEvent): { trigger: string } | { error: string } {
  const mods: string[] = [];
  if (e.ctrlKey) mods.push("Ctrl");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  if (e.metaKey) mods.push("Cmd");
  const main = mainKeyName(e);
  if (!main) return { error: "不支持这个键，换一个" };
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
    let pendingMod: string | null = null; // a modifier held alone, awaiting a clean keyup

    const onKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      if (e.key === "Escape" && !e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey) {
        stop(); // cancel
        return;
      }
      if (e.key in MOD_NAMES) {
        const held = [e.ctrlKey, e.altKey, e.shiftKey, e.metaKey].filter(Boolean).length;
        pendingMod = held === 1 ? e.key : null; // only a lone modifier can be a tap
        return;
      }
      pendingMod = null; // a real key joined → not a bare-modifier tap
      const res = eventToTrigger(e);
      if ("error" in res) {
        if (res.error) setHint(res.error);
        return;
      }
      setHint(null);
      stop(res.trigger);
    };

    const onKeyUp = (e: KeyboardEvent) => {
      if (e.key in MOD_NAMES && pendingMod === e.key) {
        pendingMod = null;
        setHint(null);
        stop(MOD_NAMES[e.key]); // bare modifier tap, e.g. "Shift"
      }
    };

    window.addEventListener("keydown", onKeyDown, true);
    window.addEventListener("keyup", onKeyUp, true);
    const unlisten = listen("trigger-record-fn", () => {
      setHint(null);
      stop("Fn");
    });
    return () => {
      window.removeEventListener("keydown", onKeyDown, true);
      window.removeEventListener("keyup", onKeyUp, true);
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
