# Audie Backend Guidelines

Audie's backend follows the existing Manager + Command/Event + Platform trait
architecture. Refactors should make files easier to understand without moving
architecture decisions away from the project spec.

## File Responsibilities

- `lib.rs` owns Tauri setup, state registration, hotkey entry points, overlay
  window wiring, and the high-level pipeline orchestration.
- `commands.rs` owns Tauri command contracts, settings persistence, validation,
  and light dispatch into managers or platform helpers.
- `managers/*` owns reusable app services: audio, transcription, enhance,
  history, and injection. Do not create a new manager unless the spec calls for
  a new app service boundary.
- `platform/*` owns OS side effects behind the `Platform` trait. Managers and
  commands must not import macOS or Windows modules directly.

## macOS Platform Modules

`platform/macos.rs` is the public macOS implementation surface. As it grows,
split implementation details into private child modules:

- `keychain.rs`: SecItem generic-password storage.
- `clipboard.rs`: clipboard write/read helpers and synthetic Cmd+C/Cmd+V.
- `hotkey.rs`: production trigger taps, trigger parsing, and dev probe events.
- `capture.rs`: Settings recorder capture tap and pure capture state machine.
- `permissions.rs`: Input Monitoring and microphone permission checks/requests.
- `audio_device.rs`: CoreAudio input-device scoring and reliable-device choice.
- `language.rs`: system language lookup and language-label mapping.

The `Platform` trait stays stable unless the product spec explicitly changes.
Module splits should preserve command/event names and user-visible behavior.

## Error Handling

- Return `AppResult<T>` and map OS failures to the existing `AppError` categories.
- Do not `unwrap()` or `expect()` on hot paths.
- Keep privacy-sensitive values out of logs. API keys remain in system keychain,
  never in `settings.toml`.

## Quality Gates

- Run `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test` after
  Rust refactors.
- Prefer small behavior-preserving module splits. If a refactor starts changing
  state transitions, provider behavior, or platform semantics, stop and make it a
  separate planned slice.
