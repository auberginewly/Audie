// Subscribe to Rust `state-change` events and mirror into the Zustand store.
// Validates every payload with Zod (CLAUDE.md TS rule: no `any`).

import { useEffect } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  EVENT_STATE_CHANGE,
  StateChangeSchema,
  type StateChange,
} from "../types/events";
import { useRecordingStore } from "../store/recording";

export function useRecordingFlow(): void {
  const setState = useRecordingStore((s) => s.setState);

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let cancelled = false;

    listen<StateChange>(EVENT_STATE_CHANGE, (event) => {
      const parsed = StateChangeSchema.safeParse(event.payload);
      if (!parsed.success) {
        // Defensive: a malformed event means a Rust/TS contract drift.
        console.warn("invalid state-change payload", parsed.error, event.payload);
        return;
      }
      setState(parsed.data.to);
    })
      .then((fn) => {
        if (cancelled) {
          fn();
        } else {
          unlisten = fn;
        }
      })
      .catch((err) => {
        console.error("failed to subscribe state-change:", err);
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [setState]);
}
