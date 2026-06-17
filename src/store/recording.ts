// UI mirror of the Rust state machine. NO business logic here — the Rust side
// is the single source of truth. This store exists so React components can
// render off Zustand selectors instead of subscribing to Tauri events directly.

import { create } from "zustand";
import type { AppState, AppErrorEvent, EnhanceProgressEvent } from "../types/events";

interface RecordingStore {
  state: AppState;
  error: AppErrorEvent | null;
  enhanceProgress: EnhanceProgressEvent | null;
  setState: (next: AppState) => void;
  setError: (err: AppErrorEvent) => void;
  setEnhanceProgress: (progress: EnhanceProgressEvent) => void;
}

export const useRecordingStore = create<RecordingStore>((set) => ({
  state: "IDLE",
  error: null,
  enhanceProgress: null,
  // A fresh recording clears any stale error so the capsule starts clean.
  setState: (next) =>
    set(
      next === "RECORDING"
        ? { state: next, error: null, enhanceProgress: null }
        : { state: next },
    ),
  setError: (err) => set({ error: err }),
  setEnhanceProgress: (progress) => set({ enhanceProgress: progress }),
}));
