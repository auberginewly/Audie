// Trigger-key recorder (P3.10). One box showing the current trigger (default fn).
// Click it → "按下快捷键…" → press any key/combo to set it. Capture is fully native:
// begin_trigger_capture runs a listen-only CGEventTap (the webview can't see fn), and
// Rust emits `trigger-captured` (the key the user formed) or `trigger-capture-rejected`
// (Caps Lock / system combos like Cmd+Q). The webview no longer reads KeyboardEvent.

import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type { Hotkey } from "../../types/settings";
import { KeyCombo } from "../ui";

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
  const active = useRef(false); // guards against a double stop (capture + blur)

  // End a capture. A new key goes through `onChange` → update_settings, which
  // unregisters the capture tap and registers the new trigger; cancel / same key
  // just restores the real trigger. Doing exactly ONE avoids a restore-vs-set race.
  const stop = useCallback(
    (next?: string) => {
      if (!active.current) return;
      active.current = false;
      setRecording(false);
      if (next && next !== value) {
        onChange(next as Hotkey);
      } else {
        void invoke("end_trigger_capture").catch((err) => console.error("end capture failed:", err));
      }
    },
    [onChange, value],
  );
  // Keep the listeners off `stop` (which changes every render) so the effect only
  // (re)subscribes on record start/stop, not on every parent re-render.
  const stopRef = useRef(stop);
  stopRef.current = stop;

  useEffect(() => {
    if (!recording) return;
    ref.current?.focus(); // so clicking away (blur) cancels
    const captured = listen<string>("trigger-captured", (e) => {
      setHint(null);
      stopRef.current(e.payload);
    });
    const rejected = listen<string>("trigger-capture-rejected", (e) => {
      setHint(e.payload); // keep recording — user tries another key
    });
    return () => {
      void captured.then((f) => f());
      void rejected.then((f) => f());
    };
  }, [recording]);

  const start = async () => {
    setHint(null);
    try {
      await invoke("begin_trigger_capture");
      active.current = true;
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
          if (recording) stop(); // click again = cancel
          else void start();
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
