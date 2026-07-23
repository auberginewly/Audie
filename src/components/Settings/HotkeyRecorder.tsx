// Trigger-key recorder (P3.10). macOS captures natively because the webview cannot
// see fn; Windows captures the focused webview's keydown after temporarily stopping
// the registered global shortcuts.

import { useCallback, useEffect, useRef, useState, type KeyboardEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type { Hotkey } from "../../types/settings";
import { KeyCombo } from "../ui";
import { useI18n } from "../../i18n";
import { getRuntimePlatform } from "../../lib/runtimePlatform";
import { captureWindowsHotkey } from "../../lib/windowsHotkeyCapture";

const BOX =
  "inline-flex min-h-8 min-w-[92px] items-center justify-center rounded-sm border px-2.5 py-1 cursor-pointer transition-colors duration-150 ease-[var(--ease-out)]";

interface HotkeyRecorderProps {
  value: Hotkey;
  onChange: (next: Hotkey) => Promise<boolean>;
  // 另一个触发键 —— 录到相同的键时拒绝（润色/改写键与写作键不能相同）。
  conflictWith?: string;
}

export function HotkeyRecorder({ value, onChange, conflictWith }: HotkeyRecorderProps) {
  const { t } = useI18n();
  const platform = getRuntimePlatform();
  const [recording, setRecording] = useState(false);
  const [hint, setHint] = useState<string | null>(null);
  const ref = useRef<HTMLButtonElement>(null);
  const active = useRef(false); // guards against a double stop (capture + blur)

  // Off the listener effect's deps (like stopRef) so changing it doesn't resubscribe.
  const conflictWithRef = useRef(conflictWith);
  useEffect(() => {
    conflictWithRef.current = conflictWith;
  }, [conflictWith]);

  // End a capture. A new key goes through `onChange` → update_settings, which
  // unregisters the capture tap and registers the new trigger; cancel / same key
  // just restores the real trigger. Doing exactly ONE avoids a restore-vs-set race.
  const stop = useCallback(
    (next?: string) => {
      if (!active.current) return;
      active.current = false;
      setRecording(false);
      if (next && next !== value) {
        void onChange(next).then((saved) => {
          if (!saved) {
            setHint(t("settings.hotkey.saveFailed"));
            void invoke("end_trigger_capture").catch((err: unknown) => {
              console.error("restore trigger after failed save:", err);
            });
          }
        });
      } else {
        void invoke("end_trigger_capture").catch((err: unknown) => {
          console.error("end capture failed:", err);
        });
      }
    },
    [onChange, t, value],
  );
  // Keep the listeners off `stop` (which changes every render) so the effect only
  // (re)subscribes on record start/stop, not on every parent re-render.
  const stopRef = useRef(stop);
  useEffect(() => {
    stopRef.current = stop;
  }, [stop]);

  useEffect(
    () => () => {
      if (!active.current) return;
      active.current = false;
      void invoke("end_trigger_capture").catch((err: unknown) => {
        console.error("end capture on unmount failed:", err);
      });
    },
    [],
  );

  useEffect(() => {
    if (!recording) return;
    ref.current?.focus(); // so clicking away (blur) cancels
    const captured = listen<string>("trigger-captured", (e) => {
      // Reject a key already used by the other trigger — keep recording so the user
      // can try another (same UX as a rejected system combo).
      if (conflictWithRef.current?.toLowerCase() === e.payload.toLowerCase()) {
        setHint(t("settings.hotkey.conflict"));
        return;
      }
      setHint(null);
      stopRef.current(e.payload);
    });
    const rejected = listen<string>("trigger-capture-rejected", (e) => {
      setHint(e.payload); // keep recording — user tries another key
    });
    return () => {
      void captured.then((f) => {
        f();
      });
      void rejected.then((f) => {
        f();
      });
    };
  }, [recording, t]);

  const start = async () => {
    setHint(null);
    try {
      await invoke("begin_trigger_capture");
      active.current = true;
      setRecording(true);
    } catch {
      setHint(t("settings.hotkey.permissionNeeded"));
    }
  };

  const handleWindowsKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (!recording || platform !== "windows") return;
    event.preventDefault();
    event.stopPropagation();

    const result = captureWindowsHotkey(event);
    if (result.kind === "ignore") return;
    if (result.kind === "rejected") {
      setHint(t("settings.hotkey.unsupportedWindows"));
      return;
    }
    if (conflictWithRef.current?.toLowerCase() === result.hotkey.toLowerCase()) {
      setHint(t("settings.hotkey.conflict"));
      return;
    }
    setHint(null);
    stop(result.hotkey);
  };

  const keys = value.split("+").map((key) => key.trim());

  return (
    <div className="flex flex-col items-end gap-1">
      <button
        ref={ref}
        type="button"
        onClick={() => {
          if (recording)
            stop(); // click again = cancel
          else void start();
        }}
        onBlur={() => {
          if (recording) stop();
        }}
        onKeyDown={handleWindowsKeyDown}
        className={[BOX, recording ? "border-accent-fill bg-accent-bg" : "border-transparent bg-gray-200"].join(" ")}
      >
        {recording ? (
          <span className="text-[13px] text-accent-text">{t("settings.hotkey.recording")}</span>
        ) : (
          <KeyCombo keys={keys} size="sm" literal={platform === "windows"} />
        )}
      </button>
      {hint ? <span className="text-[11px] text-warning-text">{hint}</span> : null}
    </div>
  );
}
