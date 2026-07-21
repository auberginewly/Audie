import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { z } from "zod";

import { openExternal } from "../lib/open";
import {
  effectivePermissionGranted,
  permissionAfterStatus,
  permissionAfterTimeout,
  type PermissionKey,
  type PermissionPhase,
  type PermissionSnapshot,
} from "./permissionState";

const StatusSchema = z.boolean();
const POLL_INTERVAL_MS = 500;
const POLL_ATTEMPTS = 60;

export interface PermissionState {
  readonly granted: boolean | null;
  readonly phase: PermissionPhase;
  readonly request: () => Promise<void>;
  readonly openSettings: () => void;
  readonly restart: () => Promise<void>;
}

export type UsePermissions = Record<PermissionKey, PermissionState>;

const COMMANDS: Record<PermissionKey, { get: string; request: string; settingsUrl: string }> = {
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
};

const PERMISSION_KEYS = ["microphone", "accessibility", "inputMonitoring"] as const;

type PermissionRecord = Record<PermissionKey, PermissionSnapshot & { readonly loaded: boolean }>;

const INITIAL_PERMISSION: PermissionSnapshot & { readonly loaded: boolean } = {
  granted: false,
  phase: "idle",
  loaded: false,
};

export function usePermissions(): UsePermissions {
  const [permissions, setPermissions] = useState<PermissionRecord>({
    microphone: INITIAL_PERMISSION,
    accessibility: INITIAL_PERMISSION,
    inputMonitoring: INITIAL_PERMISSION,
  });
  const pollers = useRef<Partial<Record<PermissionKey, ReturnType<typeof setInterval>>>>({});

  const stopPolling = useCallback((key: PermissionKey) => {
    const poller = pollers.current[key];
    if (poller !== undefined) clearInterval(poller);
    delete pollers.current[key];
  }, []);

  const applyStatus = useCallback((key: PermissionKey, granted: boolean) => {
    setPermissions((current) => ({
      ...current,
      [key]: { ...permissionAfterStatus(key, current[key], granted), loaded: true },
    }));
  }, []);

  const read = useCallback(
    async (key: PermissionKey): Promise<boolean | null> => {
      try {
        const parsed = StatusSchema.safeParse(await invoke(COMMANDS[key].get));
        if (!parsed.success) return null;
        applyStatus(key, parsed.data);
        return parsed.data;
      } catch (error) {
        console.error(`${key} status failed:`, error);
        return null;
      }
    },
    [applyStatus],
  );

  const startPolling = useCallback(
    (key: PermissionKey) => {
      stopPolling(key);
      let attempts = 0;
      pollers.current[key] = setInterval(() => {
        attempts += 1;
        void read(key).then((granted) => {
          if (granted === true) {
            stopPolling(key);
          } else if (attempts >= POLL_ATTEMPTS) {
            stopPolling(key);
            setPermissions((current) => ({
              ...current,
              [key]: { ...permissionAfterTimeout(current[key]), loaded: true },
            }));
          }
        });
      }, POLL_INTERVAL_MS);
    },
    [read, stopPolling],
  );

  const readAll = useCallback(() => {
    for (const key of PERMISSION_KEYS) void read(key);
  }, [read]);

  useEffect(() => {
    void Promise.resolve().then(readAll);
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void getCurrentWindow()
      .onFocusChanged(({ payload: focused }) => {
        if (focused) readAll();
      })
      .then((listener) => {
        if (cancelled) listener();
        else unlisten = listener;
      })
      .catch((error: unknown) => {
        console.error("focus subscribe failed:", error);
      });
    return () => {
      cancelled = true;
      unlisten?.();
      for (const key of PERMISSION_KEYS) stopPolling(key);
    };
  }, [readAll, stopPolling]);

  const make = useCallback(
    (key: PermissionKey): PermissionState => {
      const snapshot = permissions[key];
      return {
        granted: snapshot.loaded ? effectivePermissionGranted(snapshot) : null,
        phase: snapshot.phase,
        request: async () => {
          stopPolling(key);
          setPermissions((current) => ({
            ...current,
            [key]: { granted: false, phase: "requesting", loaded: true },
          }));
          try {
            const parsed = StatusSchema.safeParse(await invoke(COMMANDS[key].request));
            if (parsed.success) applyStatus(key, parsed.data);
            if (parsed.success && parsed.data) return;
            if (key !== "microphone") {
              setPermissions((current) => ({
                ...current,
                [key]: { granted: false, phase: "needsSettings", loaded: true },
              }));
              openExternal(COMMANDS[key].settingsUrl);
            }
            startPolling(key);
          } catch (error) {
            console.error(`request ${key} failed:`, error);
            setPermissions((current) => ({
              ...current,
              [key]: { granted: false, phase: "needsSettings", loaded: true },
            }));
          }
        },
        openSettings: () => {
          if (!snapshot.granted) {
            setPermissions((current) => ({
              ...current,
              [key]: { granted: false, phase: "needsSettings", loaded: true },
            }));
            startPolling(key);
          }
          openExternal(COMMANDS[key].settingsUrl);
        },
        restart: async () => {
          await invoke("restart_app");
        },
      };
    },
    [applyStatus, permissions, startPolling, stopPolling],
  );

  return {
    microphone: make("microphone"),
    accessibility: make("accessibility"),
    inputMonitoring: make("inputMonitoring"),
  };
}
