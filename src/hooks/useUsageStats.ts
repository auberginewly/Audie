// All-time usage totals for the Home dashboard. Loads get_usage_stats and
// re-fetches on the history-updated event. Logic-free screens (CLAUDE.md §6.2).

import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { UsageStatsSchema, EVENT_HISTORY_UPDATED, type UsageStats } from "../types/history";

export function useUsageStats(): UsageStats | null {
  const [stats, setStats] = useState<UsageStats | null>(null);

  useEffect(() => {
    let cancelled = false;
    let unlisten: UnlistenFn | undefined;

    const load = () => {
      invoke("get_usage_stats")
        .then((raw) => {
          const parsed = UsageStatsSchema.safeParse(raw);
          if (parsed.success) {
            if (!cancelled) setStats(parsed.data);
          } else {
            console.error("usage stats parse failed:", parsed.error);
          }
        })
        .catch((err) => {
          console.error("load usage stats failed:", err);
        });
    };

    load();
    listen(EVENT_HISTORY_UPDATED, load)
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      })
      .catch((err) => {
        console.error("subscribe history-updated failed:", err);
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  return stats;
}
