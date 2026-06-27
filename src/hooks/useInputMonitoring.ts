// Input Monitoring permission state (P3.9). The default trigger (fn) and the
// CGEventTap need it. `request` shows the system prompt; a fresh grant only
// applies after relaunch, so the row tells the user to restart Audie.

import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";

const StatusSchema = z.boolean();

export type UseInputMonitoring = {
  granted: boolean | null; // null while loading
  request: () => Promise<void>;
};

export function useInputMonitoring(): UseInputMonitoring {
  const [granted, setGranted] = useState<boolean | null>(null);

  const read = useCallback((raw: unknown) => {
    const parsed = StatusSchema.safeParse(raw);
    if (parsed.success) setGranted(parsed.data);
  }, []);

  useEffect(() => {
    invoke("get_input_monitoring_status")
      .then(read)
      .catch((err) => console.error("input monitoring status failed:", err));
  }, [read]);

  const request = useCallback(async () => {
    try {
      read(await invoke("request_input_monitoring_permission"));
    } catch (err) {
      console.error("request input monitoring failed:", err);
    }
  }, [read]);

  return { granted, request };
}
