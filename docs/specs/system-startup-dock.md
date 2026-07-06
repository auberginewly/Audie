# System Startup + Dock

## Goal

Make Settings -> General -> System real:

- Launch at login registers with the OS using the official Tauri autostart plugin.
- Show in Dock persists as an Audie setting and applies at startup and when changed.
- Permission row leading icons stay neutral gray; the status badge carries permission state.

## Decisions

- Autostart is not stored in `settings.toml`; the UI reads `isEnabled()` from the system registration and calls `enable()` / `disable()`.
- Dock visibility is stored as `show_in_dock` in `settings.toml` because it is an Audie UI preference.
- macOS Dock visibility goes through `Platform::set_dock_visible`, implemented with AppKit activation policy. Non-macOS uses the trait default no-op until P4.
- Autostart uses `tauri-plugin-autostart` with macOS `LaunchAgent`; no custom LaunchAgent or ServiceManagement code.

## Acceptance

- Settings -> General -> System shows the real launch-at-login state on open.
- Toggling launch-at-login updates the system registration; reopening Settings shows the same state.
- Toggling Show in Dock hides/restores Audie in the macOS Dock while leaving the Settings window usable.
- Restarting Audie applies the saved Dock visibility.
- Permission row leading icons are gray while granted/denied/pending badges keep their own colors.

## Verification

- `pnpm typecheck`
- `pnpm build`
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
