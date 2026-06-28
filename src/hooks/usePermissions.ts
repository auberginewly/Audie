// P3.12 — real macOS permission state for the onboarding wizard: microphone,
// accessibility, and Input Monitoring (the default `fn` trigger needs the last).
// Generalizes useInputMonitoring across all three, and re-reads on window focus so
// a grant made in System Settings reflects when the user returns. `openSettings`
// deep-links to the exact Privacy pane — macOS won't re-prompt after a denial, so
// the jump-to-Settings fallback is how users recover (mirrors Voxt's two-action row).

import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { z } from "zod";

import { openExternal } from "../lib/open";

const StatusSchema = z.boolean();

export type PermissionState = {
  granted: boolean | null; // null while loading
  request: () => Promise<void>;
  openSettings: () => void;
};

type PermKey = "microphone" | "accessibility" | "inputMonitoring" | "speechRecognition";

export type UsePermissions = Record<PermKey, PermissionState>;

const COMMANDS: Record<PermKey, { get: string; request: string; settingsUrl: string }> = {
  microphone: {
    get: "get_microphone_permission_status",
    request: "request_microphone_permission",
    settingsUrl: "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone",
  },
  accessibility: {
    get: "get_accessibility_permission_status",
    request: "request_accessibility_permission",
    settingsUrl: "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
  },
  inputMonitoring: {
    get: "get_input_monitoring_status",
    request: "request_input_monitoring_permission",
    settingsUrl: "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent",
  },
  // macOS 本机听写 (SFSpeechRecognizer) gates dictation on this; only relevant when
  // asr_provider == macos_native, so surfaced contextually (not in the base 3 rows).
  speechRecognition: {
    get: "get_speech_recognition_permission_status",
    request: "request_speech_recognition_permission",
    settingsUrl: "x-apple.systempreferences:com.apple.preference.security?Privacy_SpeechRecognition",
  },
};

const PERM_KEYS = Object.keys(COMMANDS) as PermKey[];

export function usePermissions(): UsePermissions {
  const [granted, setGranted] = useState<Record<PermKey, boolean | null>>({
    microphone: null,
    accessibility: null,
    inputMonitoring: null,
    speechRecognition: null,
  });

  const read = useCallback((key: PermKey) => {
    invoke(COMMANDS[key].get)
      .then((raw) => {
        const parsed = StatusSchema.safeParse(raw);
        if (parsed.success) setGranted((g) => ({ ...g, [key]: parsed.data }));
      })
      .catch((err) => console.error(`${key} status failed:`, err));
  }, []);

  const readAll = useCallback(() => PERM_KEYS.forEach(read), [read]);

  useEffect(() => {
    readAll();
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    getCurrentWindow()
      .onFocusChanged(({ payload: focused }) => {
        if (focused) readAll();
      })
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      })
      .catch((err) => console.error("focus subscribe failed:", err));
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [readAll]);

  const make = useCallback(
    (key: PermKey): PermissionState => ({
      granted: granted[key],
      request: async () => {
        try {
          const raw = await invoke(COMMANDS[key].request);
          const parsed = StatusSchema.safeParse(raw);
          if (parsed.success) setGranted((g) => ({ ...g, [key]: parsed.data }));
        } catch (err) {
          console.error(`request ${key} failed:`, err);
        }
      },
      openSettings: () => openExternal(COMMANDS[key].settingsUrl),
    }),
    [granted],
  );

  return {
    microphone: make("microphone"),
    accessibility: make("accessibility"),
    inputMonitoring: make("inputMonitoring"),
    speechRecognition: make("speechRecognition"),
  };
}
