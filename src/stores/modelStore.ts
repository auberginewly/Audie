// Local-ASR model store — the catalog + on-disk state from the backend
// ModelManager, plus live download progress. Mirrors Handy's modelStore.ts pattern
// (load get_available_models, listen to the download events, auto-refresh) but in
// Audie's style: hand-written Zod validation (no specta bindings), no immer (not a
// dep) — plain spread updates. Components read off selectors; this owns the IPC.

import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import {
  DownloadProgressSchema,
  ModelInfoSchema,
  type DownloadProgress,
  type ModelInfo,
} from "../types/settings";

const ModelListSchema = ModelInfoSchema.array();

// model-download-failed payload (Rust commands.rs emits { model_id, error }).
type DownloadFailed = { model_id: string; error: string };

interface ModelStore {
  models: ModelInfo[];
  // Currently selected local-ASR model id ("" = none → manual whisper path).
  currentModel: string;
  // model_id → live progress while downloading.
  downloadProgress: Record<string, DownloadProgress>;
  loaded: boolean;
  // Backend probe / download error surfaced to the picker (cleared on next action).
  error: string | null;
  // Guards the one-time event-listener setup so re-mounts don't double-subscribe.
  initialized: boolean;

  init: () => Promise<void>;
  loadModels: () => Promise<void>;
  loadCurrentModel: () => Promise<void>;
  selectModel: (modelId: string) => Promise<void>;
  downloadModel: (modelId: string) => Promise<void>;
  cancelDownload: (modelId: string) => Promise<void>;
  deleteModel: (modelId: string) => Promise<void>;
}

// Drop a model's in-flight progress entry. Plain object copy (no immer dep).
function withoutProgress(
  progress: Record<string, DownloadProgress>,
  modelId: string,
): Record<string, DownloadProgress> {
  const next = { ...progress };
  delete next[modelId];
  return next;
}

export const useModelStore = create<ModelStore>((set, get) => ({
  models: [],
  currentModel: "",
  downloadProgress: {},
  loaded: false,
  error: null,
  initialized: false,

  loadModels: async () => {
    try {
      const raw = await invoke("get_available_models");
      const parsed = ModelListSchema.safeParse(raw);
      if (parsed.success) set({ models: parsed.data, loaded: true });
      else console.error("models parse failed:", parsed.error);
    } catch (err) {
      console.error("load models failed:", err);
    }
  },

  loadCurrentModel: async () => {
    try {
      const raw = await invoke("get_current_local_asr_model");
      if (typeof raw === "string") set({ currentModel: raw });
    } catch (err) {
      console.error("load current model failed:", err);
    }
  },

  selectModel: async (modelId) => {
    try {
      set({ error: null });
      await invoke("set_active_local_asr_model", { modelId });
      set({ currentModel: modelId });
    } catch (err) {
      set({ error: messageOf(err) });
    }
  },

  downloadModel: async (modelId) => {
    set({ error: null });
    // Optimistic row so the progress bar shows immediately, before the first event.
    set((s) => ({
      downloadProgress: {
        ...s.downloadProgress,
        [modelId]: { model_id: modelId, downloaded: 0, total: 0, percentage: 0 },
      },
    }));
    try {
      // Resolves when the download finishes; the -complete event refreshes the list.
      await invoke("download_model", { modelId });
    } catch (err) {
      // model-download-failed normally clears the row, but clean up here too in case
      // it never arrived (e.g. an IPC error before the backend emitted).
      set((s) => ({
        error: messageOf(err),
        downloadProgress: withoutProgress(s.downloadProgress, modelId),
      }));
    }
  },

  cancelDownload: async (modelId) => {
    try {
      await invoke("cancel_download", { modelId });
      set((s) => ({ downloadProgress: withoutProgress(s.downloadProgress, modelId) }));
      await get().loadModels();
    } catch (err) {
      set({ error: messageOf(err) });
    }
  },

  deleteModel: async (modelId) => {
    try {
      set({ error: null });
      await invoke("delete_model", { modelId });
      await get().loadModels();
      await get().loadCurrentModel(); // selection may have been cleared backend-side
    } catch (err) {
      set({ error: messageOf(err) });
    }
  },

  init: async () => {
    if (get().initialized) return;
    set({ initialized: true });

    await Promise.all([get().loadModels(), get().loadCurrentModel()]);

    // Listen to the P2 download events; the store auto-refreshes (NO manual scan).
    // Unlisteners are intentionally not torn down: the store is app-lived (a single
    // create()), so the listeners outlive any one mounted picker.
    const subs: Promise<UnlistenFn>[] = [
      listen<DownloadProgress>("model-download-progress", (event) => {
        const parsed = DownloadProgressSchema.safeParse(event.payload);
        if (!parsed.success) return;
        set((s) => ({
          downloadProgress: { ...s.downloadProgress, [parsed.data.model_id]: parsed.data },
        }));
      }),
      listen<string>("model-download-complete", (event) => {
        set((s) => ({ downloadProgress: withoutProgress(s.downloadProgress, event.payload) }));
        get().loadModels();
      }),
      listen<DownloadFailed>("model-download-failed", (event) => {
        const { model_id: modelId, error } = event.payload;
        set((s) => ({ error, downloadProgress: withoutProgress(s.downloadProgress, modelId) }));
        get().loadModels();
      }),
      listen<string>("model-download-cancelled", (event) => {
        set((s) => ({ downloadProgress: withoutProgress(s.downloadProgress, event.payload) }));
        get().loadModels();
      }),
      listen<string>("model-deleted", () => {
        get().loadModels();
        get().loadCurrentModel();
      }),
    ];
    await Promise.all(subs).catch((err) => console.error("model event listen failed:", err));
  },
}));

// Tauri serializes AppError as { code, message }; a rejected command surfaces its
// user-facing message. Falls back to the raw value for plain-string rejections.
function messageOf(err: unknown): string {
  if (err && typeof err === "object" && "message" in err) {
    const m = (err as { message: unknown }).message;
    if (typeof m === "string" && m) return m;
  }
  return typeof err === "string" && err ? err : "操作失败，请查看日志";
}
