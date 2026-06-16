// UI mirror of the Rust state machine. NO business logic here — the Rust side
// is the single source of truth. This store exists so React components can
// render off Zustand selectors instead of subscribing to Tauri events directly.

import { create } from "zustand";
import type { AppState } from "../types/events";

interface RecordingStore {
  state: AppState;
  setState: (next: AppState) => void;
}

export const useRecordingStore = create<RecordingStore>((set) => ({
  state: "IDLE",
  setState: (next) => set({ state: next }),
}));
