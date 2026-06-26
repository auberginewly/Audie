// Drives the Settings mic-preview meter. While `active`, asks Rust to monitor
// `device` ("" = automatic) and returns the live input level [0, 1] from
// `mic-monitor-level` so the user can confirm the picked mic is hearing them.
// Restarts when the device changes; stops on unmount / when inactive. Recording
// also stops the monitor server-side (it owns the mic).

import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { AudioLevelSchema, EVENT_MIC_MONITOR_LEVEL, type AudioLevel } from "../types/events";

export function useMicMonitor(device: string, active: boolean): number {
  const [level, setLevel] = useState(0);

  useEffect(() => {
    if (!active) return;
    let unlisten: UnlistenFn | undefined;
    let cancelled = false;

    listen<AudioLevel>(EVENT_MIC_MONITOR_LEVEL, (event) => {
      const parsed = AudioLevelSchema.safeParse(event.payload);
      if (parsed.success) setLevel(parsed.data.level);
    })
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      })
      .catch((err) => console.error("subscribe mic-monitor-level failed:", err));

    invoke("start_mic_monitor", { device: device || null }).catch((err) =>
      console.error("start_mic_monitor failed:", err),
    );

    return () => {
      cancelled = true;
      unlisten?.();
      setLevel(0);
      void invoke("stop_mic_monitor").catch(() => {});
    };
  }, [device, active]);

  return level;
}
