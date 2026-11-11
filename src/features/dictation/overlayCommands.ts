import { invoke } from "@tauri-apps/api/core";

// Overlay → Rust. The overlay is a non-activating NSPanel, so these clicks never
// steal focus from the user's app (injection still targets it). Best-effort.
export function callOverlayCommand(cmd: string) {
  void invoke(cmd).catch((err) => {
    console.error(`${cmd} failed:`, err);
  });
}
