// Dictation history data layer for the History screen. Loads list_history and
// re-fetches on the history-updated event; delete/clear go through the backend,
// which emits the event that drives the re-fetch (single source of truth).

import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { HistoryEntrySchema, EVENT_HISTORY_UPDATED, type HistoryEntry } from "../types/history";

const HistoryListSchema = HistoryEntrySchema.array();

export type UseHistory = {
  entries: HistoryEntry[];
  remove: (id: number) => Promise<void>;
  clearAll: () => Promise<void>;
  // Re-run the current LLM on a stored entry's transcript. Resolves on success
  // (the history-updated event refreshes the list); rejects so callers can toast.
  reenhance: (id: number) => Promise<void>;
};

export function useHistory(): UseHistory {
  const [entries, setEntries] = useState<HistoryEntry[]>([]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: UnlistenFn | undefined;

    const load = () => {
      invoke("list_history")
        .then((raw) => {
          const parsed = HistoryListSchema.safeParse(raw);
          if (parsed.success) {
            if (!cancelled) setEntries(parsed.data);
          } else {
            console.error("history parse failed:", parsed.error);
          }
        })
        .catch((err) => console.error("load history failed:", err));
    };

    load();
    listen(EVENT_HISTORY_UPDATED, load)
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      })
      .catch((err) => console.error("subscribe history-updated failed:", err));

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const remove = async (id: number) => {
    try {
      await invoke("delete_history_entry", { id });
    } catch (err) {
      console.error("delete history entry failed:", err);
    }
  };

  const clearAll = async () => {
    try {
      await invoke("clear_history");
    } catch (err) {
      console.error("clear history failed:", err);
    }
  };

  // Let this one reject: the History screen shows a toast on failure (network /
  // missing key), and the success path refreshes via history-updated.
  const reenhance = (id: number) => invoke<void>("reenhance_history_entry", { id });

  return { entries, remove, clearAll, reenhance };
}
