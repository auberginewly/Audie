// TODO P0.1 — `trait Platform` (register_hotkey / inject_text / store_secret / read_secret)
// + factory that picks macos / windows impl. PROJECT_SPEC.md §3.4 / §6.3.
//
// Strict rule: all `#[cfg(target_os)]` lives behind this trait — never in managers/.
