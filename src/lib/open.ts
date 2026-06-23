import { openUrl } from "@tauri-apps/plugin-opener";

// Open an external URL in the system browser. Tauri webviews don't follow
// target="_blank" on their own — links must route through the opener plugin.
export function openExternal(url: string) {
  void openUrl(url).catch((err) => console.error("open url failed:", err));
}
