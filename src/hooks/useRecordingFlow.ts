// Subscribe to Rust `state-change` events and mirror into the Zustand store.
// Validates every payload with Zod (CLAUDE.md TS rule: no `any`).

import { useEffect } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  EVENT_STATE_CHANGE,
  EVENT_ERROR,
  EVENT_ENHANCE_PROGRESS,
  StateChangeSchema,
  AppErrorSchema,
  EnhanceProgressSchema,
  type StateChange,
  type AppErrorEvent,
  type EnhanceProgressEvent,
} from "../types/events";
import { useRecordingStore } from "../store/recording";

export function useRecordingFlow(): void {
  const setState = useRecordingStore((s) => s.setState);
  const setError = useRecordingStore((s) => s.setError);
  const setEnhanceProgress = useRecordingStore((s) => s.setEnhanceProgress);

  useEffect(() => {
    const unlisteners: UnlistenFn[] = [];
    let cancelled = false;

    const track = (fn: UnlistenFn) => {
      if (cancelled) {
        fn();
      } else {
        unlisteners.push(fn);
      }
    };

    listen<StateChange>(EVENT_STATE_CHANGE, (event) => {
      const parsed = StateChangeSchema.safeParse(event.payload);
      if (!parsed.success) {
        // Defensive: a malformed event means a Rust/TS contract drift.
        console.warn("invalid state-change payload", parsed.error, event.payload);
        return;
      }
      setState(parsed.data.to);
    })
      .then(track)
      .catch((err) => {
        console.error("failed to subscribe state-change:", err);
      });

    // The ERROR state-change and this `error` event arrive as a pair; the latter
    // carries the message the capsule renders.
    listen<AppErrorEvent>(EVENT_ERROR, (event) => {
      const parsed = AppErrorSchema.safeParse(event.payload);
      if (!parsed.success) {
        console.warn("invalid error payload", parsed.error, event.payload);
        return;
      }
      setError(parsed.data);
    })
      .then(track)
      .catch((err) => {
        console.error("failed to subscribe error:", err);
      });

    listen<EnhanceProgressEvent>(EVENT_ENHANCE_PROGRESS, (event) => {
      const parsed = EnhanceProgressSchema.safeParse(event.payload);
      if (!parsed.success) {
        console.warn("invalid enhance-progress payload", parsed.error, event.payload);
        return;
      }
      setEnhanceProgress(parsed.data);
    })
      .then(track)
      .catch((err) => {
        console.error("failed to subscribe enhance-progress:", err);
      });

    return () => {
      cancelled = true;
      unlisteners.forEach((fn) => {
        fn();
      });
    };
  }, [setState, setError, setEnhanceProgress]);
}
