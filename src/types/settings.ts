// Zod schema for the settings payload returned by get_settings / update_settings.
// Mirrors Rust `commands::Settings` + `HOTKEY_PRESETS` (src-tauri/src/commands.rs).
// P0.5 scope: hotkey only; microphone selection lands with the Settings page.

import { z } from "zod";

export const SettingsSchema = z.object({
  hotkey: z.enum(["Ctrl+Shift+Space", "Alt+Space", "Ctrl+Alt+Space"]),
});

export type Settings = z.infer<typeof SettingsSchema>;
export type Hotkey = Settings["hotkey"];

// Single source for the dropdown — derived from the schema so they can't drift.
export const HOTKEY_PRESETS = SettingsSchema.shape.hotkey.options;
