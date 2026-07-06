// Real per-model "configured" status from keychain has_secret presence checks
// (replaces the old mock ModelMeta.status). A model counts as configured when it
// has >=1 required secret and all are present. Refreshes on window focus and via
// refresh() (e.g. after the config dialog saves a key). Presence checks are no-read
// (never unlock the keychain), mirroring ModelConfigDialog's KeyInput.

import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { MODELS, requiredSecretsForModel } from "../components/Settings/models";
import type { SecretKeyId } from "../types/settings";

export interface UseConfiguredModels {
  configured: (modelId: string) => boolean;
  refresh: () => void;
}

const ALL_SECRETS: SecretKeyId[] = Array.from(new Set(MODELS.flatMap((m) => requiredSecretsForModel(m.id))));

export function useConfiguredModels(): UseConfiguredModels {
  const [present, setPresent] = useState<Partial<Record<SecretKeyId, boolean>>>({});

  const refresh = useCallback(() => {
    Promise.all(
      ALL_SECRETS.map((keyId) =>
        invoke("has_secret", { keyId })
          .then((raw) => [keyId, raw === true] as const)
          .catch(() => [keyId, false] as const),
      ),
    )
      .then((pairs) => {
        setPresent(Object.fromEntries(pairs));
      })
      .catch((err) => {
        console.error("read secret presence failed:", err);
      });
  }, []);

  useEffect(() => {
    refresh();
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    getCurrentWindow()
      .onFocusChanged(({ payload: focused }) => {
        if (focused) refresh();
      })
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      })
      .catch((err) => {
        console.error("focus subscribe failed:", err);
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [refresh]);

  const configured = useCallback(
    (modelId: string) => {
      const required = requiredSecretsForModel(modelId);
      return required.length > 0 && required.every((k) => present[k] === true);
    },
    [present],
  );

  return { configured, refresh };
}
