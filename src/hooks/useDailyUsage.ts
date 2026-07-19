// Per-day 口述/AI 产出 series for the Home chart. Loads get_daily_usage and
// re-fetches on the history-updated event, mirroring useUsageStats. Returns
// only days that have rows — the chart component zero-fills the window.

import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { DailyUsageSchema, EVENT_HISTORY_UPDATED, type DailyUsage } from "../types/history";

const DailyUsageListSchema = DailyUsageSchema.array();

export function useDailyUsage(days: number): DailyUsage[] {
  const [rows, setRows] = useState<DailyUsage[]>([]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: UnlistenFn | undefined;

    const load = () => {
      invoke("get_daily_usage", { days })
        .then((raw) => {
          const parsed = DailyUsageListSchema.safeParse(raw);
          if (parsed.success) {
            if (!cancelled) setRows(parsed.data);
          } else {
            console.error("daily usage parse failed:", parsed.error);
          }
        })
        .catch((err: unknown) => {
          console.error("load daily usage failed:", err);
        });
    };

    load();
    listen(EVENT_HISTORY_UPDATED, load)
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      })
      .catch((err: unknown) => {
        console.error("subscribe history-updated failed:", err);
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [days]);

  return rows;
}
