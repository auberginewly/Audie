// UI mirror of the Rust state machine. NO business logic here — the Rust side
// is the single source of truth. This store exists so React components can
// render off Zustand selectors instead of subscribing to Tauri events directly.

import { create } from "zustand";
import type { AppState, AppErrorEvent, EnhanceProgressEvent } from "../types/events";

interface RecordingStore {
  state: AppState;
  error: AppErrorEvent | null;
  enhanceProgress: EnhanceProgressEvent | null;
  // Latched true on the first SUCCESS — drives the onboarding「试一下」step's
  // persistent checkmark (survives the wizard reopening; resets on app restart).
  everSucceeded: boolean;
  setState: (next: AppState) => void;
  setError: (err: AppErrorEvent) => void;
  setEnhanceProgress: (progress: EnhanceProgressEvent) => void;
}

export const useRecordingStore = create<RecordingStore>((set) => ({
  state: "IDLE",
  error: null,
  enhanceProgress: null,
  everSucceeded: false,
  // A fresh recording clears any stale error so the capsule starts clean; a SUCCESS
  // latches everSucceeded (never reset within the session).
  setState: (next) => {
    set(
      next === "RECORDING"
        ? { state: next, error: null, enhanceProgress: null }
        : next === "SUCCESS"
          ? { state: next, everSucceeded: true }
          : { state: next },
    );
  },
  setError: (err) => {
    set({ error: err });
  },
  setEnhanceProgress: (progress) => {
    set({ enhanceProgress: progress });
  },
}));
