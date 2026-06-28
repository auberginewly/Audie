// macOS implementation of trait Platform.
//
// P3.9: the trigger key (fn / single / combo) is driven by a CGEventTap on a
// dedicated run-loop thread — global-shortcut is gone. A clean fn "tap" or a
// matching combo keyDown fires the same HotkeyEvent::Pressed callback. Needs
// Input Monitoring.
//
// P0.4 adds clipboard-method inject (save → write → Cmd+V → restore). P1 adds
// Keychain Services for BYOK secrets.

use std::ffi::c_void;
use std::time::{Duration, Instant};

use core_foundation::base::{CFType, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::data::CFData;
use core_foundation::dictionary::CFDictionary;
use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventType, CGKeyCode, EventField,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use parking_lot::Mutex;
use security_framework_sys::access_control::kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly;
use security_framework_sys::base::{errSecDuplicateItem, errSecItemNotFound, errSecSuccess};
use security_framework_sys::item::{
    kSecAttrAccount, kSecAttrService, kSecClass, kSecClassGenericPassword, kSecReturnData,
    kSecValueData,
};
use security_framework_sys::keychain_item::{
    SecItemAdd, SecItemCopyMatching, SecItemDelete, SecItemUpdate,
};
use tauri::{AppHandle, Emitter};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_macos_permissions::{check_microphone_permission, request_microphone_permission};

use super::{HotkeyCallback, Platform};
use crate::error::{AppError, AppResult};

#[derive(Default)]
pub struct MacosPlatform {
    // The app that was frontmost when recording started. Clicking an overlay
    // button makes Audie frontmost (stealing key focus), so inject restores this
    // app first or the synthesized Cmd+V would paste into nothing. Voxt's pattern.
    focus_target_pid: Mutex<Option<i32>>,
    // P3.8 dev-only trigger-key probe: holds the run loop (to stop it cross-thread)
    // and the thread the CGEventTap runs on. None when not probing.
    probe: Mutex<Option<EventTapHandle>>,
    // P3.9 production trigger: fn / single / combo, all via one CGEventTap that
    // drives the real toggle callback. None when no trigger is registered.
    trigger: Mutex<Option<EventTapHandle>>,
    // P3.10 recorder: a listen-only capture tap, live only while the Settings
    // recorder is open. Emits trigger-captured / -rejected. None otherwise.
    capture: Mutex<Option<EventTapHandle>>,
}

impl MacosPlatform {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Platform for MacosPlatform {
    fn register_hotkey(
        &self,
        _app: &AppHandle,
        combo: &str,
        callback: HotkeyCallback,
    ) -> AppResult<()> {
        // P3.9: one CGEventTap handles every trigger shape (fn / single / combo).
        let spec = parse_trigger(combo)?;
        self.start_trigger(spec, callback)
    }

    fn unregister_all_hotkeys(&self, _app: &AppHandle) -> AppResult<()> {
        self.stop_trigger();
        Ok(())
    }

    fn inject_text(&self, app: &AppHandle, text: &str) -> AppResult<()> {
        // Clipboard method: most compatible across apps. We write the transcript to
        // the clipboard and simulate Cmd+V — and DELIBERATELY do not restore the old
        // clipboard. macOS silently drops the synthetic Cmd+V when Accessibility is
        // missing or stale (CGEvent::post is void, so there's no way to tell a
        // dropped paste from a landed one), so leaving the transcript on the
        // pasteboard guarantees the user can always manually Cmd+V — the text is
        // never lost even when the paste doesn't land. Trade-off: each dictation
        // overwrites the system clipboard (standard for dictation tools).
        app.clipboard()
            .write_text(text.to_string())
            .map_err(|err| AppError::Inject(format!("clipboard write failed: {err}")))?;

        // Preflight Accessibility BEFORE simulating Cmd+V. Without that permission
        // CGEvent::post() silently drops the keystroke and the paste never lands.
        // The transcript is already on the clipboard (above), so on a preflight miss
        // we return Permission so the user knows to grant access / paste manually
        // (§3.7). NOTE: a *stale* grant (e.g. after a code-signature change) makes
        // preflight return true while events are still dropped — undetectable here;
        // the clipboard fallback is what saves the text in that case.
        if !preflight_post_event_access() {
            // Best-effort: ask macOS to add Audie to the Accessibility list so the
            // user can flip the switch. Result is ignored — even if it returns
            // false (added but not granted) the error message tells them next steps.
            unsafe {
                let _ = CGRequestPostEventAccess();
            }
            return Err(AppError::Permission(
                "辅助功能权限未授予，文字已复制到剪贴板，可手动粘贴；请到 系统设置 → 隐私与安全性 → 辅助功能 启用 Audie".into(),
            ));
        }

        // If a panel-button click stole key focus to us, hand it back to the app
        // that was frontmost at record start, or the synthetic Cmd+V pastes into
        // nothing. Hotkey path: we're not frontmost → no-op, keep the 20ms settle.
        // When restored, give AppKit ~50ms for the activation handoff (Voxt ~40ms).
        let restored = match *self.focus_target_pid.lock() {
            Some(pid) => restore_focus_if_stolen(pid),
            None => false,
        };
        std::thread::sleep(Duration::from_millis(if restored { 50 } else { 20 }));
        simulate_cmd_v()?;

        // No clipboard restore: the transcript stays on the pasteboard as a
        // manual-paste fallback (see the method-opening comment).
        Ok(())
    }

    fn capture_focus_target(&self) {
        let pid = current_frontmost_pid();
        *self.focus_target_pid.lock() = pid;
        log::debug!("capture_focus_target: frontmost pid = {pid:?}");
    }

    fn preferred_input_device_name(&self) -> Option<String> {
        pick_reliable_input()
    }

    fn system_language(&self) -> Option<String> {
        system_language_label()
    }

    fn ensure_microphone_permission(&self) -> bool {
        // `request` triggers requestAccess(.audio): it shows the prompt only when
        // status is NotDetermined (resolving once the user answers) and is a no-op
        // when already decided — but it doesn't report the decision, so we read it
        // back with `check`. Blocks the hotkey thread, not the UI thread, so fine.
        if let Err(err) = tauri::async_runtime::block_on(request_microphone_permission()) {
            log::warn!("request microphone permission: {err}");
        }
        tauri::async_runtime::block_on(check_microphone_permission())
    }

    fn store_secret(&self, key: &str, value: &str) -> AppResult<()> {
        keychain_store_secret(key, value)
    }

    fn has_secret(&self, key: &str) -> AppResult<bool> {
        keychain_has_secret(key)
    }

    fn read_secret(&self, key: &str) -> AppResult<String> {
        keychain_read_secret(key)
    }

    fn delete_secret(&self, key: &str) -> AppResult<()> {
        keychain_delete_secret(key)
    }

    fn input_monitoring_status(&self) -> bool {
        input_monitoring_granted()
    }

    fn request_input_monitoring(&self) {
        // Shows the system prompt when undecided; no-op once decided. The grant
        // only applies after relaunch, so callers re-read status + tell the user.
        unsafe {
            IOHIDRequestAccess(K_IOHID_REQUEST_TYPE_LISTEN_EVENT);
        }
    }

    fn microphone_status(&self) -> bool {
        // Presence check only — no prompt (onboarding reads this repeatedly). Unlike
        // `ensure_microphone_permission`, this never calls `request`.
        tauri::async_runtime::block_on(check_microphone_permission())
    }

    fn request_microphone(&self) {
        // Shows the prompt only when undecided; no-op once decided. Status is read
        // back separately via `microphone_status`.
        if let Err(err) = tauri::async_runtime::block_on(request_microphone_permission()) {
            log::warn!("request microphone permission: {err}");
        }
    }

    fn accessibility_status(&self) -> bool {
        preflight_post_event_access()
    }

    fn request_accessibility(&self) {
        // Asks macOS to add Audie to the Accessibility list so the user can flip the
        // switch; the grant applies once they do (status re-read separately).
        unsafe {
            let _ = CGRequestPostEventAccess();
        }
    }

    fn speech_recognition_status(&self) -> bool {
        speech_recognition_authorized()
    }

    fn request_speech_recognition(&self) {
        // Shows the system prompt when status is NotDetermined; no-op once decided.
        // requestAuthorization reports the decision via a completion block, so we
        // block until it fires (status is re-read separately via speech_recognition_status).
        request_speech_recognition_authorization();
    }

    /// P3.10 — start a listen-only capture tap for the Settings recorder. Feeds the
    /// pure `capture_step` machine, which emits `trigger-captured` (the key the user
    /// formed) or `trigger-capture-rejected`. Needs Input Monitoring (same as the
    /// trigger). Idempotent.
    fn start_trigger_capture(&self, app: &AppHandle) -> AppResult<()> {
        if self.capture.lock().is_some() {
            return Ok(());
        }
        if !input_monitoring_granted() {
            unsafe {
                IOHIDRequestAccess(K_IOHID_REQUEST_TYPE_LISTEN_EVENT);
            }
            if !input_monitoring_granted() {
                return Err(AppError::Permission(
                    "输入监控权限未授予；请到 系统设置 → 隐私与安全性 → 输入监控 启用 Audie，然后重启 App".into(),
                ));
            }
        }

        let app = app.clone();
        let (tx, rx) = std::sync::mpsc::channel::<Option<CFRunLoop>>();
        let thread = std::thread::Builder::new()
            .name("trigger-capture".into())
            .spawn(move || {
                let state = Mutex::new(CaptureState::default());
                let tap = CGEventTap::new(
                    CGEventTapLocation::HID,
                    CGEventTapPlacement::HeadInsertEventTap,
                    CGEventTapOptions::ListenOnly,
                    vec![
                        CGEventType::KeyDown,
                        CGEventType::KeyUp,
                        CGEventType::FlagsChanged,
                    ],
                    move |_proxy, event_type, event| {
                        let captured = classify_capture_event(event_type, event);
                        match capture_step(&mut state.lock(), captured, Instant::now()) {
                            CaptureOutcome::Captured(trigger) => {
                                let _ = app.emit("trigger-captured", trigger);
                            }
                            CaptureOutcome::Rejected(reason) => {
                                let _ = app.emit("trigger-capture-rejected", reason);
                            }
                            CaptureOutcome::None => {}
                        }
                        None
                    },
                );
                let tap = match tap {
                    Ok(tap) => tap,
                    Err(()) => {
                        let _ = tx.send(None);
                        return;
                    }
                };
                let source = match tap.mach_port.create_runloop_source(0) {
                    Ok(source) => source,
                    Err(()) => {
                        let _ = tx.send(None);
                        return;
                    }
                };
                let runloop = CFRunLoop::get_current();
                unsafe {
                    runloop.add_source(&source, kCFRunLoopCommonModes);
                }
                tap.enable();
                let _ = tx.send(Some(runloop));
                CFRunLoop::run_current(); // blocks until stop_trigger_capture
            })
            .map_err(|err| AppError::Internal(format!("spawn capture thread: {err}")))?;

        match rx.recv() {
            Ok(Some(runloop)) => {
                *self.capture.lock() = Some(EventTapHandle {
                    runloop,
                    thread: Some(thread),
                });
                log::info!("trigger capture started");
                Ok(())
            }
            _ => {
                let _ = thread.join();
                Err(AppError::Internal(
                    "failed to create CGEventTap for capture".into(),
                ))
            }
        }
    }

    fn stop_trigger_capture(&self) {
        if let Some(mut handle) = self.capture.lock().take() {
            handle.runloop.stop();
            if let Some(thread) = handle.thread.take() {
                let _ = thread.join();
            }
            log::info!("trigger capture stopped");
        }
    }

    fn start_trigger_probe(&self, app: &AppHandle) -> AppResult<()> {
        // Idempotent: a second start while one is live is a no-op.
        if self.probe.lock().is_some() {
            return Ok(());
        }

        // Option B: actively prompt for Input Monitoring, then re-check. A fresh
        // grant only takes effect for an already-running process after relaunch,
        // so a denial here returns Permission with that hint (§3.7) — no panic.
        if !input_monitoring_granted() {
            unsafe {
                IOHIDRequestAccess(K_IOHID_REQUEST_TYPE_LISTEN_EVENT);
            }
            if !input_monitoring_granted() {
                return Err(AppError::Permission(
                    "输入监控权限未授予；请到 系统设置 → 隐私与安全性 → 输入监控 启用 Audie，然后重启 App".into(),
                ));
            }
        }

        // The CGEventTap must live on a thread running a CFRunLoop. We hand the run
        // loop back through a channel so stop_trigger_probe can stop it cross-thread
        // (CFRunLoop is Send and CFRunLoopStop is documented thread-safe).
        let app = app.clone();
        let (tx, rx) = std::sync::mpsc::channel::<Option<CFRunLoop>>();
        let thread = std::thread::Builder::new()
            .name("trigger-probe".into())
            .spawn(move || {
                let tap = CGEventTap::new(
                    CGEventTapLocation::HID,
                    CGEventTapPlacement::HeadInsertEventTap,
                    CGEventTapOptions::ListenOnly,
                    vec![
                        CGEventType::KeyDown,
                        CGEventType::KeyUp,
                        CGEventType::FlagsChanged,
                    ],
                    move |_proxy, event_type, event| {
                        emit_probe_key(&app, event_type, event);
                        None
                    },
                );
                let tap = match tap {
                    Ok(tap) => tap,
                    Err(()) => {
                        let _ = tx.send(None);
                        return;
                    }
                };
                let source = match tap.mach_port.create_runloop_source(0) {
                    Ok(source) => source,
                    Err(()) => {
                        let _ = tx.send(None);
                        return;
                    }
                };
                let runloop = CFRunLoop::get_current();
                unsafe {
                    runloop.add_source(&source, kCFRunLoopCommonModes);
                }
                tap.enable();
                let _ = tx.send(Some(runloop));
                CFRunLoop::run_current(); // blocks until stop_trigger_probe
            })
            .map_err(|err| AppError::Internal(format!("spawn trigger-probe thread: {err}")))?;

        match rx.recv() {
            Ok(Some(runloop)) => {
                *self.probe.lock() = Some(EventTapHandle {
                    runloop,
                    thread: Some(thread),
                });
                log::info!("trigger probe started");
                Ok(())
            }
            _ => {
                let _ = thread.join();
                Err(AppError::Internal(
                    "failed to create CGEventTap for trigger probe".into(),
                ))
            }
        }
    }

    fn stop_trigger_probe(&self) -> AppResult<()> {
        if let Some(mut probe) = self.probe.lock().take() {
            probe.runloop.stop();
            if let Some(thread) = probe.thread.take() {
                let _ = thread.join();
            }
            log::info!("trigger probe stopped");
        }
        Ok(())
    }
}

// ---- P3.9 production trigger (fn / single / combo) --------------------------
//
// One CGEventTap on a dedicated run-loop thread replaces global-shortcut. A clean
// fn "tap" or a matching combo/single keyDown fires HotkeyEvent::Pressed. The tap
// is listen-only: fn is inert once the user disables "按🌐键用来" (P3.12), and a
// combo trigger may also reach the foreground app — an accepted simplification vs
// an active tap that swallows (which would risk system-wide input jank). Needs
// Input Monitoring; a missing grant returns Permission, never panics.

/// Max fn hold that still counts as a "tap": longer presses (resting on fn) and
/// fn+key combos don't toggle. SPEC §5.8 P3.9.
const FN_TAP_MAX: Duration = Duration::from_millis(400);

#[derive(Default)]
struct FnTapState {
    fn_down_at: Option<Instant>,
    other_pressed: bool,
}

/// Mask a CGEvent's flags down to the modifier bits a trigger can require, so
/// caps-lock / numpad / coalescing bits don't break combo matching.
fn relevant_mods(flags: CGEventFlags) -> CGEventFlags {
    flags
        & (CGEventFlags::CGEventFlagControl
            | CGEventFlags::CGEventFlagShift
            | CGEventFlags::CGEventFlagAlternate
            | CGEventFlags::CGEventFlagCommand
            | CGEventFlags::CGEventFlagSecondaryFn)
}

/// A parsed trigger: a bare modifier (fn / shift / ctrl / alt / cmd — fires on a
/// clean tap) or a main key with exact modifiers (mods may be empty for a bare key).
#[derive(Clone, Copy)]
enum TriggerSpec {
    ModifierTap {
        flag: CGEventFlags,
    },
    Combo {
        keycode: CGKeyCode,
        mods: CGEventFlags,
    },
}

/// Parse a stored trigger string ("Fn" / "F13" / "Ctrl+Shift+Space") into a spec.
/// Unknown keys / no main key are `Internal` errors so validation can reject them.
fn parse_trigger(combo: &str) -> AppResult<TriggerSpec> {
    if combo.eq_ignore_ascii_case("fn") {
        return Ok(TriggerSpec::ModifierTap {
            flag: CGEventFlags::CGEventFlagSecondaryFn,
        });
    }
    let mut mods = CGEventFlags::empty();
    let mut main: Option<CGKeyCode> = None;
    for part in combo.split('+') {
        let token = part.trim();
        if token.is_empty() {
            continue;
        }
        match token.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => mods |= CGEventFlags::CGEventFlagControl,
            "alt" | "opt" | "option" => mods |= CGEventFlags::CGEventFlagAlternate,
            "shift" => mods |= CGEventFlags::CGEventFlagShift,
            "cmd" | "command" | "meta" | "super" => mods |= CGEventFlags::CGEventFlagCommand,
            // fn as a combo modifier (e.g. "Fn+Space"). Bare "Fn" is caught above.
            "fn" => mods |= CGEventFlags::CGEventFlagSecondaryFn,
            _ => {
                let keycode = keycode_for(token)
                    .ok_or_else(|| AppError::Internal(format!("unknown trigger key: {token:?}")))?;
                if main.is_some() {
                    return Err(AppError::Internal(format!(
                        "trigger {combo:?} has more than one main key"
                    )));
                }
                main = Some(keycode);
            }
        }
    }
    match main {
        Some(keycode) => Ok(TriggerSpec::Combo { keycode, mods }),
        // No main key: a single bare modifier (e.g. "Shift") is a tap trigger like
        // fn; two+ modifiers with no key is ambiguous → reject.
        None if mods.bits().count_ones() == 1 => Ok(TriggerSpec::ModifierTap { flag: mods }),
        None => Err(AppError::Internal(format!(
            "trigger {combo:?} has no main key"
        ))),
    }
}

/// Virtual keycodes (kVK_*) for the keys a trigger may use, including bare letters /
/// digits. A bare typing key also types into the focused app when pressed (the tap
/// is listen-only and doesn't swallow) — that's the user's choice; fn / other
/// modifiers / function keys are the bare triggers that don't type.
fn keycode_for(name: &str) -> Option<CGKeyCode> {
    Some(match name.to_ascii_lowercase().as_str() {
        "space" => 49,
        "return" | "enter" => 36,
        "tab" => 48,
        "escape" | "esc" => 53,
        "left" => 123,
        "right" => 124,
        "down" => 125,
        "up" => 126,
        "f1" => 122,
        "f2" => 120,
        "f3" => 99,
        "f4" => 118,
        "f5" => 96,
        "f6" => 97,
        "f7" => 98,
        "f8" => 100,
        "f9" => 101,
        "f10" => 109,
        "f11" => 103,
        "f12" => 111,
        "f13" => 105,
        "f14" => 107,
        "f15" => 113,
        "f16" => 106,
        "f17" => 64,
        "f18" => 79,
        "f19" => 80,
        "f20" => 90,
        // kVK_ANSI_* letters + digits (US layout) — combo main keys.
        "a" => 0,
        "b" => 11,
        "c" => 8,
        "d" => 2,
        "e" => 14,
        "f" => 3,
        "g" => 5,
        "h" => 4,
        "i" => 34,
        "j" => 38,
        "k" => 40,
        "l" => 37,
        "m" => 46,
        "n" => 45,
        "o" => 31,
        "p" => 35,
        "q" => 12,
        "r" => 15,
        "s" => 1,
        "t" => 17,
        "u" => 32,
        "v" => 9,
        "w" => 13,
        "x" => 7,
        "y" => 16,
        "z" => 6,
        "0" => 29,
        "1" => 18,
        "2" => 19,
        "3" => 20,
        "4" => 21,
        "5" => 23,
        "6" => 22,
        "7" => 26,
        "8" => 28,
        "9" => 25,
        _ => return None,
    })
}

/// A combo/single trigger fires on the main key's keyDown with exactly its
/// modifiers held (so Ctrl+Shift+Space doesn't fire on Ctrl+Space).
fn detect_combo(
    keycode: CGKeyCode,
    mods: CGEventFlags,
    event_type: CGEventType,
    event: &CGEvent,
) -> bool {
    if !matches!(event_type, CGEventType::KeyDown) {
        return false;
    }
    let pressed = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as CGKeyCode;
    pressed == keycode && relevant_mods(event.get_flags()) == mods
}

// ---- P3.10 recorder: native capture state machine ---------------------------
//
// During recording, the listen-only capture tap feeds raw events into this PURE
// machine, which turns them into the trigger string the user is forming (bare
// modifier / single key / combo / fn+combo) or a rejection — so the webview never
// reads KeyboardEvent (it can't see fn). The string round-trips through
// `parse_trigger`.

/// Canonical modifier order + token for building trigger strings.
const TRIGGER_MODS: [(CGEventFlags, &str); 5] = [
    (CGEventFlags::CGEventFlagControl, "Ctrl"),
    (CGEventFlags::CGEventFlagAlternate, "Alt"),
    (CGEventFlags::CGEventFlagShift, "Shift"),
    (CGEventFlags::CGEventFlagCommand, "Cmd"),
    (CGEventFlags::CGEventFlagSecondaryFn, "Fn"),
];

fn mods_tokens(mods: CGEventFlags) -> Vec<&'static str> {
    TRIGGER_MODS
        .iter()
        .filter(|(flag, _)| mods.contains(*flag))
        .map(|(_, token)| *token)
        .collect()
}

/// System combos that would double-fire (we don't consume) — e.g. Cmd+Q quits the
/// frontmost app. Rejected in the recorder. best-effort; only the destructive few.
const SYSTEM_BLOCKLIST: &[&str] = &["Cmd+Q", "Cmd+W", "Cmd+Tab", "Cmd+Space", "Cmd+H", "Cmd+M"];

/// Inverse of `keycode_for`: a captured main key's virtual keycode → token, or None
/// for keys a trigger can't use (so the recorder ignores them).
fn name_for_keycode(keycode: CGKeyCode) -> Option<&'static str> {
    Some(match keycode {
        49 => "Space",
        36 => "Return",
        48 => "Tab",
        53 => "Escape",
        123 => "Left",
        124 => "Right",
        125 => "Down",
        126 => "Up",
        122 => "F1",
        120 => "F2",
        99 => "F3",
        118 => "F4",
        96 => "F5",
        97 => "F6",
        98 => "F7",
        100 => "F8",
        101 => "F9",
        109 => "F10",
        103 => "F11",
        111 => "F12",
        105 => "F13",
        107 => "F14",
        113 => "F15",
        106 => "F16",
        64 => "F17",
        79 => "F18",
        80 => "F19",
        90 => "F20",
        0 => "A",
        11 => "B",
        8 => "C",
        2 => "D",
        14 => "E",
        3 => "F",
        5 => "G",
        4 => "H",
        34 => "I",
        38 => "J",
        40 => "K",
        37 => "L",
        46 => "M",
        45 => "N",
        31 => "O",
        35 => "P",
        12 => "Q",
        15 => "R",
        1 => "S",
        17 => "T",
        32 => "U",
        9 => "V",
        13 => "W",
        7 => "X",
        16 => "Y",
        6 => "Z",
        29 => "0",
        18 => "1",
        19 => "2",
        20 => "3",
        21 => "4",
        23 => "5",
        22 => "6",
        26 => "7",
        28 => "8",
        25 => "9",
        _ => return None,
    })
}

/// A capture-relevant event, distilled from a CGEvent so `capture_step` is pure and
/// unit-testable.
enum CaptureEvent {
    ModifierDown(CGEventFlags), // a modifier (incl caps lock) went down
    ModifierUp(CGEventFlags),   // ... went up
    Key {
        keycode: CGKeyCode,
        mods: CGEventFlags, // already masked to trigger-relevant mods (incl fn)
    },
    Ignore,
}

#[derive(Default)]
struct CaptureState {
    pending: Option<(CGEventFlags, Instant)>, // the lone modifier held as a tap candidate
    disqualified: bool,                       // a second modifier / key intervened
}

enum CaptureOutcome {
    None,
    Captured(String),
    Rejected(&'static str),
}

/// Feed one event to the capture machine. A non-modifier keyDown is a combo/single
/// (fires immediately); a lone modifier down→up within FN_TAP_MAX is a bare-modifier
/// tap. Caps Lock and the system blocklist are rejected.
fn capture_step(state: &mut CaptureState, event: CaptureEvent, at: Instant) -> CaptureOutcome {
    match event {
        CaptureEvent::ModifierDown(flag) => {
            if state.pending.is_none() {
                state.pending = Some((flag, at));
                state.disqualified = false;
            } else {
                state.disqualified = true; // a second modifier — not a single tap
            }
            CaptureOutcome::None
        }
        CaptureEvent::ModifierUp(flag) => match state.pending {
            Some((pending_flag, since)) if pending_flag == flag => {
                let clean =
                    !state.disqualified && at.saturating_duration_since(since) <= FN_TAP_MAX;
                state.pending = None;
                state.disqualified = false;
                if !clean {
                    CaptureOutcome::None
                } else if flag == CGEventFlags::CGEventFlagAlphaShift {
                    CaptureOutcome::Rejected("Caps Lock 不能用作触发键")
                } else {
                    match mods_tokens(flag).first() {
                        Some(token) => CaptureOutcome::Captured((*token).to_string()),
                        None => CaptureOutcome::None,
                    }
                }
            }
            _ => CaptureOutcome::None,
        },
        CaptureEvent::Key { keycode, mods } => {
            state.pending = None;
            state.disqualified = false;
            let name = match name_for_keycode(keycode) {
                Some(name) => name,
                None => return CaptureOutcome::None, // unmappable key — keep waiting
            };
            let mut parts = mods_tokens(mods);
            parts.push(name);
            let trigger = parts.join("+");
            if SYSTEM_BLOCKLIST.contains(&trigger.as_str()) {
                CaptureOutcome::Rejected("该组合被系统占用，换一个")
            } else {
                CaptureOutcome::Captured(trigger)
            }
        }
        CaptureEvent::Ignore => CaptureOutcome::None,
    }
}

/// CGEvent → CaptureEvent boundary.
fn classify_capture_event(event_type: CGEventType, event: &CGEvent) -> CaptureEvent {
    let keycode = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as CGKeyCode;
    match event_type {
        CGEventType::FlagsChanged => match modifier_flag_for_keycode(keycode) {
            Some(flag) => {
                if event.get_flags().contains(flag) {
                    CaptureEvent::ModifierDown(flag)
                } else {
                    CaptureEvent::ModifierUp(flag)
                }
            }
            None => CaptureEvent::Ignore,
        },
        CGEventType::KeyDown => CaptureEvent::Key {
            keycode,
            mods: relevant_mods(event.get_flags()),
        },
        _ => CaptureEvent::Ignore,
    }
}

impl MacosPlatform {
    /// Start the production trigger tap for `spec`. Idempotent; needs Input
    /// Monitoring (a denial returns Permission so startup can keep going).
    fn start_trigger(&self, spec: TriggerSpec, callback: HotkeyCallback) -> AppResult<()> {
        if self.trigger.lock().is_some() {
            return Ok(());
        }

        // Actively prompt for Input Monitoring, then re-check. A fresh grant only
        // applies after relaunch, so a denial returns Permission (§3.7); startup
        // logs and continues so the app still launches (lib.rs).
        if !input_monitoring_granted() {
            unsafe {
                IOHIDRequestAccess(K_IOHID_REQUEST_TYPE_LISTEN_EVENT);
            }
            if !input_monitoring_granted() {
                return Err(AppError::Permission(
                    "输入监控权限未授予；请到 系统设置 → 隐私与安全性 → 输入监控 启用 Audie，然后重启 App".into(),
                ));
            }
        }

        // Same tap-on-a-runloop-thread pattern as the dev probe; hand the run loop
        // back so stop_trigger can stop it cross-thread.
        let (tx, rx) = std::sync::mpsc::channel::<Option<CFRunLoop>>();
        let thread = std::thread::Builder::new()
            .name("trigger".into())
            .spawn(move || {
                let state = Mutex::new(FnTapState::default());
                let tap = CGEventTap::new(
                    CGEventTapLocation::HID,
                    CGEventTapPlacement::HeadInsertEventTap,
                    CGEventTapOptions::ListenOnly,
                    vec![
                        CGEventType::KeyDown,
                        CGEventType::KeyUp,
                        CGEventType::FlagsChanged,
                    ],
                    move |_proxy, event_type, event| {
                        let fire = match spec {
                            TriggerSpec::ModifierTap { flag } => {
                                detect_modifier_tap(&state, flag, event_type, event)
                            }
                            TriggerSpec::Combo { keycode, mods } => {
                                detect_combo(keycode, mods, event_type, event)
                            }
                        };
                        if fire {
                            callback();
                        }
                        None
                    },
                );
                let tap = match tap {
                    Ok(tap) => tap,
                    Err(()) => {
                        let _ = tx.send(None);
                        return;
                    }
                };
                let source = match tap.mach_port.create_runloop_source(0) {
                    Ok(source) => source,
                    Err(()) => {
                        let _ = tx.send(None);
                        return;
                    }
                };
                let runloop = CFRunLoop::get_current();
                unsafe {
                    runloop.add_source(&source, kCFRunLoopCommonModes);
                }
                tap.enable();
                let _ = tx.send(Some(runloop));
                CFRunLoop::run_current(); // blocks until stop_trigger
            })
            .map_err(|err| AppError::Internal(format!("spawn trigger thread: {err}")))?;

        match rx.recv() {
            Ok(Some(runloop)) => {
                *self.trigger.lock() = Some(EventTapHandle {
                    runloop,
                    thread: Some(thread),
                });
                log::info!("trigger started");
                Ok(())
            }
            _ => {
                let _ = thread.join();
                Err(AppError::Internal(
                    "failed to create CGEventTap for trigger".into(),
                ))
            }
        }
    }

    fn stop_trigger(&self) {
        if let Some(mut handle) = self.trigger.lock().take() {
            handle.runloop.stop();
            if let Some(thread) = handle.thread.take() {
                let _ = thread.join();
            }
            log::info!("trigger stopped");
        }
    }
}

/// One trigger-relevant signal distilled from a CGEvent, so the tap state machine
/// (`fn_tap_transition`) stays pure and unit-testable without fabricating events.
enum FnSignal {
    FnDown,
    FnUp,
    /// Any non-fn key/modifier going down — disqualifies an in-flight tap (fn+X).
    OtherKey,
}

/// Pure fn tap state machine. A clean tap = fn down, no other key in between, fn
/// up within FN_TAP_MAX. Returns true only on the fn-up that completes a tap.
/// `at` is the event time, injected so tests don't depend on wall-clock.
fn fn_tap_transition(state: &mut FnTapState, signal: FnSignal, at: Instant) -> bool {
    match signal {
        FnSignal::FnDown => {
            state.fn_down_at = Some(at);
            state.other_pressed = false;
            false
        }
        FnSignal::OtherKey => {
            if state.fn_down_at.is_some() {
                state.other_pressed = true;
            }
            false
        }
        FnSignal::FnUp => match state.fn_down_at.take() {
            Some(down_at) => {
                !state.other_pressed && at.saturating_duration_since(down_at) <= FN_TAP_MAX
            }
            None => false,
        },
    }
}

/// Thin CGEvent boundary for a bare-modifier trigger: classify the event against the
/// `target` modifier flag, then run the pure tap state machine. The modifier (fn /
/// shift / ctrl / alt / cmd) arrives as a flagsChanged; a tap = down→up with no other
/// key in between.
fn detect_modifier_tap(
    state: &Mutex<FnTapState>,
    target: CGEventFlags,
    event_type: CGEventType,
    event: &CGEvent,
) -> bool {
    let keycode = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
    let signal = match event_type {
        CGEventType::FlagsChanged if modifier_flag_for_keycode(keycode) == Some(target) => {
            if event.get_flags().contains(target) {
                FnSignal::FnDown
            } else {
                FnSignal::FnUp
            }
        }
        CGEventType::KeyDown | CGEventType::FlagsChanged => FnSignal::OtherKey,
        _ => return false,
    };
    fn_tap_transition(&mut state.lock(), signal, Instant::now())
}

// ---- P3.8 trigger-key probe (dev-only) --------------------------------------
//
// A listen-only CGEventTap that reports every key/flags event so we can verify
// fn + custom single/combo keys reach us before P3.9 swaps the real trigger.
// IOKit drives Input Monitoring; CGEventTap captures. Both stay behind Platform
// per §6.3. SPEC §5.8.

/// fn key on macOS arrives as a `flagsChanged` event with this virtual keycode
/// (`kVK_Function`), not a keyDown — the easy thing to miss.
const KVK_FUNCTION: u16 = 63;

/// A live CGEventTap: the run loop it spins on (stoppable cross-thread) plus the
/// thread handle. Shared by the dev probe (P3.8) and the production trigger (P3.9).
struct EventTapHandle {
    runloop: CFRunLoop,
    thread: Option<std::thread::JoinHandle<()>>,
}

#[derive(Clone, serde::Serialize)]
struct TriggerProbeKey {
    key_label: String,
    keycode: u16,
    modifiers: Vec<String>,
    phase: &'static str,
    is_fn: bool,
}

fn emit_probe_key(app: &AppHandle, event_type: CGEventType, event: &CGEvent) {
    let keycode = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
    let flags = event.get_flags();
    let is_fn = matches!(event_type, CGEventType::FlagsChanged) && keycode == KVK_FUNCTION;

    // flagsChanged has no intrinsic down/up — derive it from whether the key's own
    // modifier bit is still set after the change.
    let phase = match event_type {
        CGEventType::KeyDown => "down",
        CGEventType::KeyUp => "up",
        _ => match modifier_flag_for_keycode(keycode) {
            Some(flag) if flags.contains(flag) => "down",
            _ => "up",
        },
    };

    let modifiers = collect_modifiers(flags);
    let key_label = if is_fn {
        "fn".to_string()
    } else {
        match modifier_flag_for_keycode(keycode) {
            Some(_) => modifiers
                .last()
                .cloned()
                .unwrap_or_else(|| format!("key#{keycode}")),
            None => format!("key#{keycode}"),
        }
    };

    log::info!(
        "trigger-probe-key: label={key_label} keycode={keycode} mods={modifiers:?} phase={phase} is_fn={is_fn}"
    );
    if let Err(err) = app.emit(
        "trigger-probe-key",
        TriggerProbeKey {
            key_label,
            keycode,
            modifiers,
            phase,
            is_fn,
        },
    ) {
        log::warn!("emit trigger-probe-key failed: {err}");
    }
}

fn collect_modifiers(flags: CGEventFlags) -> Vec<String> {
    let mut out = Vec::new();
    if flags.contains(CGEventFlags::CGEventFlagControl) {
        out.push("ctrl".into());
    }
    if flags.contains(CGEventFlags::CGEventFlagAlternate) {
        out.push("alt".into());
    }
    if flags.contains(CGEventFlags::CGEventFlagShift) {
        out.push("shift".into());
    }
    if flags.contains(CGEventFlags::CGEventFlagCommand) {
        out.push("cmd".into());
    }
    if flags.contains(CGEventFlags::CGEventFlagSecondaryFn) {
        out.push("fn".into());
    }
    out
}

/// Map a modifier key's virtual keycode to the flag it toggles, so a flagsChanged
/// event can be classified as press vs release. Non-modifier keys return None.
fn modifier_flag_for_keycode(keycode: u16) -> Option<CGEventFlags> {
    match keycode {
        KVK_FUNCTION => Some(CGEventFlags::CGEventFlagSecondaryFn),
        56 | 60 => Some(CGEventFlags::CGEventFlagShift), // L/R shift
        59 | 62 => Some(CGEventFlags::CGEventFlagControl), // L/R control
        58 | 61 => Some(CGEventFlags::CGEventFlagAlternate), // L/R option
        54 | 55 => Some(CGEventFlags::CGEventFlagCommand), // L/R command
        57 => Some(CGEventFlags::CGEventFlagAlphaShift), // caps lock
        _ => None,
    }
}

// IOKit HID access for Input Monitoring (§5.8 option B: actively prompt). IOKit
// is not linked by core-graphics/security-framework, so this block carries its
// own framework link — same pattern as the CGEvent externs below.
#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOHIDCheckAccess(request: u32) -> u32;
    fn IOHIDRequestAccess(request: u32) -> bool;
}

// <IOKit/hid/IOHIDLib.h>: kIOHIDRequestTypeListenEvent = 1; kIOHIDAccessTypeGranted = 0.
const K_IOHID_REQUEST_TYPE_LISTEN_EVENT: u32 = 1;
const K_IOHID_ACCESS_TYPE_GRANTED: u32 = 0;

fn input_monitoring_granted() -> bool {
    // SAFETY: C call from IOKit with a constant request type, returns an enum int.
    unsafe { IOHIDCheckAccess(K_IOHID_REQUEST_TYPE_LISTEN_EVENT) == K_IOHID_ACCESS_TYPE_GRANTED }
}

// ---- macOS Keychain Services (P1.2) -----------------------------------------
//
// Store API keys as generic-password items using SecItem* directly (Voxt-style):
//   service = "com.audie.app.secure-storage"
//   account = key_id (e.g. "groq_api_key")
//   value   = secret bytes
//
// Presence checks never request `kSecReturnData`, so opening the settings page
// can show "已配置 key" without asking macOS to unlock and reveal the secret.
// `read_secret` exists for provider calls, but it is never exposed to the frontend.

const KEYCHAIN_SERVICE: &str = "com.audie.app.secure-storage";
fn keychain_store_secret(key: &str, value: &str) -> AppResult<()> {
    let value_data = CFData::from_buffer(value.as_bytes());
    let query = keychain_base_query(key);
    let attrs = keychain_value_attributes(&value_data);

    let status = sec_item_copy_matching_status(&query);
    if status == errSecSuccess {
        sec_item_update(&query, &attrs, "update secret")
    } else if status == errSecItemNotFound {
        let item = keychain_add_item(key, &value_data);
        let add_status = sec_item_add(&item);
        if add_status == errSecSuccess {
            Ok(())
        } else if add_status == errSecDuplicateItem {
            sec_item_update(&query, &attrs, "update duplicate secret")
        } else {
            Err(AppError::Internal(format!(
                "add secret: status {add_status}"
            )))
        }
    } else {
        Err(AppError::Internal(format!(
            "lookup secret before write: status {status}"
        )))
    }
}

fn keychain_has_secret(key: &str) -> AppResult<bool> {
    let status = sec_item_copy_matching_status(&keychain_base_query(key));
    if status == errSecSuccess {
        Ok(true)
    } else if status == errSecItemNotFound {
        Ok(false)
    } else {
        Err(AppError::Internal(format!("check secret: status {status}")))
    }
}

fn keychain_read_secret(key: &str) -> AppResult<String> {
    let query = keychain_read_query(key);
    let mut item = std::ptr::null();
    let status = unsafe { SecItemCopyMatching(query.as_concrete_TypeRef(), &mut item) };
    if status == errSecSuccess {
        if item.is_null() {
            return Err(AppError::Internal("read secret returned null data".into()));
        }
        let data = unsafe { CFData::wrap_under_create_rule(item.cast()) };
        String::from_utf8(data.bytes().to_vec())
            .map_err(|_| AppError::Internal("keychain secret is not UTF-8".into()))
    } else if status == errSecItemNotFound {
        Err(AppError::Provider("secret not found".into()))
    } else {
        Err(AppError::Internal(format!("read secret: status {status}")))
    }
}

fn keychain_delete_secret(key: &str) -> AppResult<()> {
    let status = unsafe { SecItemDelete(keychain_base_query(key).as_concrete_TypeRef()) };
    if status == errSecSuccess || status == errSecItemNotFound {
        Ok(())
    } else {
        Err(AppError::Internal(format!(
            "delete secret: status {status}"
        )))
    }
}

fn keychain_base_query(key: &str) -> CFDictionary<CFString, CFType> {
    let class_key = unsafe { CFString::wrap_under_get_rule(kSecClass) };
    let class_value = unsafe { CFString::wrap_under_get_rule(kSecClassGenericPassword) };
    let service_key = unsafe { CFString::wrap_under_get_rule(kSecAttrService) };
    let service_value = CFString::new(KEYCHAIN_SERVICE);
    let account_key = unsafe { CFString::wrap_under_get_rule(kSecAttrAccount) };
    let account_value = CFString::new(key);

    CFDictionary::from_CFType_pairs(&[
        (class_key, class_value.as_CFType()),
        (service_key, service_value.as_CFType()),
        (account_key, account_value.as_CFType()),
    ])
}

fn keychain_read_query(key: &str) -> CFDictionary<CFString, CFType> {
    let class_key = unsafe { CFString::wrap_under_get_rule(kSecClass) };
    let class_value = unsafe { CFString::wrap_under_get_rule(kSecClassGenericPassword) };
    let service_key = unsafe { CFString::wrap_under_get_rule(kSecAttrService) };
    let service_value = CFString::new(KEYCHAIN_SERVICE);
    let account_key = unsafe { CFString::wrap_under_get_rule(kSecAttrAccount) };
    let account_value = CFString::new(key);
    let return_data_key = unsafe { CFString::wrap_under_get_rule(kSecReturnData) };
    let return_data_value = CFBoolean::true_value();

    CFDictionary::from_CFType_pairs(&[
        (class_key, class_value.as_CFType()),
        (service_key, service_value.as_CFType()),
        (account_key, account_value.as_CFType()),
        (return_data_key, return_data_value.as_CFType()),
    ])
}

fn keychain_value_attributes(value: &CFData) -> CFDictionary<CFString, CFType> {
    let value_key = unsafe { CFString::wrap_under_get_rule(kSecValueData) };
    let accessible_key = unsafe { CFString::wrap_under_get_rule(kSecAttrAccessible) };
    let accessible_value =
        unsafe { CFString::wrap_under_get_rule(kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly) };

    CFDictionary::from_CFType_pairs(&[
        (value_key, value.as_CFType()),
        (accessible_key, accessible_value.as_CFType()),
    ])
}

fn keychain_add_item(key: &str, value: &CFData) -> CFDictionary<CFString, CFType> {
    let class_key = unsafe { CFString::wrap_under_get_rule(kSecClass) };
    let class_value = unsafe { CFString::wrap_under_get_rule(kSecClassGenericPassword) };
    let service_key = unsafe { CFString::wrap_under_get_rule(kSecAttrService) };
    let service_value = CFString::new(KEYCHAIN_SERVICE);
    let account_key = unsafe { CFString::wrap_under_get_rule(kSecAttrAccount) };
    let account_value = CFString::new(key);
    let value_key = unsafe { CFString::wrap_under_get_rule(kSecValueData) };
    let accessible_key = unsafe { CFString::wrap_under_get_rule(kSecAttrAccessible) };
    let accessible_value =
        unsafe { CFString::wrap_under_get_rule(kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly) };

    CFDictionary::from_CFType_pairs(&[
        (class_key, class_value.as_CFType()),
        (service_key, service_value.as_CFType()),
        (account_key, account_value.as_CFType()),
        (value_key, value.as_CFType()),
        (accessible_key, accessible_value.as_CFType()),
    ])
}

fn sec_item_copy_matching_status(query: &CFDictionary<CFString, CFType>) -> i32 {
    unsafe { SecItemCopyMatching(query.as_concrete_TypeRef(), std::ptr::null_mut()) }
}

fn sec_item_add(item: &CFDictionary<CFString, CFType>) -> i32 {
    unsafe { SecItemAdd(item.as_concrete_TypeRef(), std::ptr::null_mut()) }
}

fn sec_item_update(
    query: &CFDictionary<CFString, CFType>,
    attrs: &CFDictionary<CFString, CFType>,
    label: &str,
) -> AppResult<()> {
    let status = unsafe { SecItemUpdate(query.as_concrete_TypeRef(), attrs.as_concrete_TypeRef()) };
    if status == errSecSuccess {
        Ok(())
    } else {
        Err(AppError::Internal(format!("{label}: status {status}")))
    }
}

/// Probe Accessibility (post-event) access. Returns true when CGEvent::post is
/// allowed to actually deliver events. The symbol is part of the ApplicationServices
/// framework which `core-graphics` already links, so no extra link flag needed.
fn preflight_post_event_access() -> bool {
    // SAFETY: parameterless C function from ApplicationServices.
    unsafe { CGPreflightPostEventAccess() }
}

// ---- Speech Recognition authorization (P4, SFSpeechRecognizer) --------------
//
// The macOS-native ASR provider needs Speech authorization. `status` reads it
// without prompting (onboarding/settings poll it); `request` shows the system
// prompt (NSSpeechRecognitionUsageDescription) and resolves the user's choice
// before returning, so a follow-up status read reflects the decision.

/// True only when Speech recognition is explicitly Authorized (not NotDetermined /
/// Denied / Restricted).
fn speech_recognition_authorized() -> bool {
    // SAFETY: parameterless Objective-C class method returning a plain enum.
    let status = unsafe { objc2_speech::SFSpeechRecognizer::authorizationStatus() };
    status == objc2_speech::SFSpeechRecognizerAuthorizationStatus::Authorized
}

/// Trigger the Speech authorization prompt (when undecided) and block until the
/// completion block fires, so the caller can re-read the resolved status. A no-op
/// once the user has already decided.
fn request_speech_recognition_authorization() {
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let handler = block2::RcBlock::new(
        move |_status: objc2_speech::SFSpeechRecognizerAuthorizationStatus| {
            let _ = tx.send(());
        },
    );
    // SAFETY: class method taking a completion block; the block is retained for the
    // duration of the async request via RcBlock.
    unsafe {
        objc2_speech::SFSpeechRecognizer::requestAuthorization(&handler);
    }
    // The block fires on a system queue. Bound the wait so a missing
    // NSSpeechRecognitionUsageDescription (which suppresses the callback) can't hang.
    let _ = rx.recv_timeout(Duration::from_secs(60));
}

extern "C" {
    static kSecAttrAccessible: CFStringRef;

    fn CGPreflightPostEventAccess() -> bool;
    fn CGRequestPostEventAccess() -> bool;
}

// ---- CoreAudio HAL: pick a non-Bluetooth input device (P0.7) -----------------
//
// AirPods/Bluetooth headsets in A2DP mode read literal zeros until macOS deigns
// to swap to HFP — and HFP also drops *system* audio quality to phone-grade.
// To dodge both, when the system default input is Bluetooth we look for a wired
// alternative (built-in mic, USB, etc.) and tell `AudioManager` to prefer that
// device by name. If only Bluetooth is available we leave it alone (caller falls
// back to the system default and silence detection covers the HFP gap).
//
// All `#[cfg(target_os = "macos")]` lives behind the Platform trait per §6.3.

type AudioObjectID = u32;
type OSStatus = i32;

#[repr(C)]
struct AudioObjectPropertyAddress {
    selector: u32,
    scope: u32,
    element: u32,
}

extern "C" {
    fn AudioObjectGetPropertyDataSize(
        object_id: AudioObjectID,
        in_address: *const AudioObjectPropertyAddress,
        in_qualifier_data_size: u32,
        in_qualifier_data: *const c_void,
        out_data_size: *mut u32,
    ) -> OSStatus;

    fn AudioObjectGetPropertyData(
        object_id: AudioObjectID,
        in_address: *const AudioObjectPropertyAddress,
        in_qualifier_data_size: u32,
        in_qualifier_data: *const c_void,
        io_data_size: *mut u32,
        out_data: *mut c_void,
    ) -> OSStatus;
}

const K_AUDIO_OBJECT_SYSTEM_OBJECT: AudioObjectID = 1;
const K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN: u32 = 0;
const K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL: u32 = fourcc(b"glob");
const K_AUDIO_OBJECT_PROPERTY_SCOPE_INPUT: u32 = fourcc(b"inpt");

const K_AUDIO_HARDWARE_PROPERTY_DEVICES: u32 = fourcc(b"dev#");
const K_AUDIO_HARDWARE_PROPERTY_DEFAULT_INPUT_DEVICE: u32 = fourcc(b"dIn ");
const K_AUDIO_DEVICE_PROPERTY_TRANSPORT_TYPE: u32 = fourcc(b"tran");
const K_AUDIO_DEVICE_PROPERTY_STREAMS: u32 = fourcc(b"stm#");
const K_AUDIO_OBJECT_PROPERTY_NAME: u32 = fourcc(b"lnam");
const K_AUDIO_DEVICE_TRANSPORT_TYPE_BUILT_IN: u32 = fourcc(b"bltn");
const K_AUDIO_DEVICE_TRANSPORT_TYPE_USB: u32 = fourcc(b"usb ");
const K_AUDIO_DEVICE_TRANSPORT_TYPE_BLUETOOTH: u32 = fourcc(b"blue");
const K_AUDIO_DEVICE_TRANSPORT_TYPE_BLUETOOTH_LE: u32 = fourcc(b"blea");
const K_AUDIO_DEVICE_TRANSPORT_TYPE_AIRPLAY: u32 = fourcc(b"airp");
// 'ccwd' / 'ccwl' — iPhone Continuity Capture (Mac uses your iPhone as mic).
// Same flakiness as Bluetooth: device entry can persist even when the phone
// isn't actively serving, and cpal often fails to open its stream config.
const K_AUDIO_DEVICE_TRANSPORT_TYPE_CONTINUITY_CAPTURE_WIRED: u32 = fourcc(b"ccwd");
const K_AUDIO_DEVICE_TRANSPORT_TYPE_CONTINUITY_CAPTURE_WIRELESS: u32 = fourcc(b"ccwl");

const fn fourcc(s: &[u8; 4]) -> u32 {
    ((s[0] as u32) << 24) | ((s[1] as u32) << 16) | ((s[2] as u32) << 8) | (s[3] as u32)
}

fn pick_reliable_input() -> Option<String> {
    // Score the system default. If it's already a reliable physical input
    // (built-in / USB), leave it alone — the user may have explicitly picked
    // it in System Settings.
    let default_id = read_property_scalar::<AudioObjectID>(
        K_AUDIO_OBJECT_SYSTEM_OBJECT,
        K_AUDIO_HARDWARE_PROPERTY_DEFAULT_INPUT_DEVICE,
        K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
    )?;
    let default_score = device_score(default_id);
    if default_score == 0 {
        return None;
    }

    log::info!(
        "system default input has unreliable transport (score {default_score}); \
         looking for a more reliable alternative"
    );

    // Scan every input device and pick the one with the lowest score that beats
    // the default. Ties don't trigger overrides — we only switch when we have a
    // strictly better candidate, so we never replace one flaky device with
    // another equally flaky one.
    let devices = read_property_array::<AudioObjectID>(
        K_AUDIO_OBJECT_SYSTEM_OBJECT,
        K_AUDIO_HARDWARE_PROPERTY_DEVICES,
        K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
    )?;
    let mut best: Option<(u8, AudioObjectID)> = None;
    for id in devices {
        if id == default_id || !has_input_streams(id) {
            continue;
        }
        let s = device_score(id);
        if s >= default_score {
            continue;
        }
        match best {
            Some((bs, _)) if bs <= s => {}
            _ => best = Some((s, id)),
        }
    }

    let (score, id) = best?;
    let name = read_device_name(id)?;
    log::info!("preferring more reliable input device (score {score}): {name}");
    Some(name)
}

fn device_score(device_id: AudioObjectID) -> u8 {
    let t = read_property_scalar::<u32>(
        device_id,
        K_AUDIO_DEVICE_PROPERTY_TRANSPORT_TYPE,
        K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
    )
    .unwrap_or(0);
    transport_score(t)
}

/// 0 = physical, cold-start reliable. 2 = needs a device handshake / availability
/// (Bluetooth A2DP↔HFP swap, iPhone Continuity, AirPlay) — fine when working,
/// often returns silence or fails to open on a cold press. 1 = unknown transport,
/// treat as "OK but prefer 0 if available".
fn transport_score(transport: u32) -> u8 {
    match transport {
        K_AUDIO_DEVICE_TRANSPORT_TYPE_BUILT_IN | K_AUDIO_DEVICE_TRANSPORT_TYPE_USB => 0,
        K_AUDIO_DEVICE_TRANSPORT_TYPE_BLUETOOTH
        | K_AUDIO_DEVICE_TRANSPORT_TYPE_BLUETOOTH_LE
        | K_AUDIO_DEVICE_TRANSPORT_TYPE_AIRPLAY
        | K_AUDIO_DEVICE_TRANSPORT_TYPE_CONTINUITY_CAPTURE_WIRED
        | K_AUDIO_DEVICE_TRANSPORT_TYPE_CONTINUITY_CAPTURE_WIRELESS => 2,
        _ => 1,
    }
}

/// A device counts as an input device iff querying its Streams property under
/// the Input scope returns at least one stream. Pure-output devices (speakers)
/// return size 0 here, which is how we filter them out cheaply.
fn has_input_streams(device_id: AudioObjectID) -> bool {
    let addr = AudioObjectPropertyAddress {
        selector: K_AUDIO_DEVICE_PROPERTY_STREAMS,
        scope: K_AUDIO_OBJECT_PROPERTY_SCOPE_INPUT,
        element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    };
    let mut size: u32 = 0;
    // SAFETY: addr is a valid stack pointer; size is an out-parameter.
    let status =
        unsafe { AudioObjectGetPropertyDataSize(device_id, &addr, 0, std::ptr::null(), &mut size) };
    status == 0 && size > 0
}

fn read_device_name(device_id: AudioObjectID) -> Option<String> {
    let addr = AudioObjectPropertyAddress {
        selector: K_AUDIO_OBJECT_PROPERTY_NAME,
        scope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
        element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    };
    let mut cf_str: CFStringRef = std::ptr::null();
    let mut size: u32 = std::mem::size_of::<CFStringRef>() as u32;
    // SAFETY: outData points to a single CFStringRef slot sized correctly.
    let status = unsafe {
        AudioObjectGetPropertyData(
            device_id,
            &addr,
            0,
            std::ptr::null(),
            &mut size,
            &mut cf_str as *mut _ as *mut c_void,
        )
    };
    if status != 0 || cf_str.is_null() {
        return None;
    }
    // kAudioObjectPropertyName follows the Create Rule: caller owns the +1
    // retain. wrap_under_create_rule adopts it (Drop will CFRelease).
    let s = unsafe { CFString::wrap_under_create_rule(cf_str) }.to_string();
    Some(s)
}

fn read_property_scalar<T: Default + Copy>(
    object: AudioObjectID,
    selector: u32,
    scope: u32,
) -> Option<T> {
    let addr = AudioObjectPropertyAddress {
        selector,
        scope,
        element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    };
    let mut value: T = T::default();
    let mut size: u32 = std::mem::size_of::<T>() as u32;
    // SAFETY: T is plain-old-data via Default+Copy; outData/size match.
    let status = unsafe {
        AudioObjectGetPropertyData(
            object,
            &addr,
            0,
            std::ptr::null(),
            &mut size,
            &mut value as *mut _ as *mut c_void,
        )
    };
    if status == 0 {
        Some(value)
    } else {
        None
    }
}

fn read_property_array<T: Default + Copy>(
    object: AudioObjectID,
    selector: u32,
    scope: u32,
) -> Option<Vec<T>> {
    let addr = AudioObjectPropertyAddress {
        selector,
        scope,
        element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    };
    let mut size: u32 = 0;
    // SAFETY: out-param.
    let status =
        unsafe { AudioObjectGetPropertyDataSize(object, &addr, 0, std::ptr::null(), &mut size) };
    if status != 0 || size == 0 {
        return None;
    }
    let count = size as usize / std::mem::size_of::<T>();
    let mut buf: Vec<T> = vec![T::default(); count];
    let mut size_inout = size;
    // SAFETY: buf is sized to match size_inout; T is POD via Default+Copy.
    let status = unsafe {
        AudioObjectGetPropertyData(
            object,
            &addr,
            0,
            std::ptr::null(),
            &mut size_inout,
            buf.as_mut_ptr() as *mut c_void,
        )
    };
    if status == 0 {
        Some(buf)
    } else {
        None
    }
}

/// Post a synthetic Cmd+V. Caller MUST preflight Accessibility — without it macOS
/// silently drops the events (post() still returns no error) and paste never lands.
fn simulate_cmd_v() -> AppResult<()> {
    const KEY_V: CGKeyCode = 9; // kVK_ANSI_V

    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
        .map_err(|()| AppError::Inject("CGEventSource creation failed".into()))?;

    let key_down = CGEvent::new_keyboard_event(source.clone(), KEY_V, true)
        .map_err(|()| AppError::Inject("CGEvent key-down failed".into()))?;
    key_down.set_flags(CGEventFlags::CGEventFlagCommand);
    key_down.post(CGEventTapLocation::HID);

    let key_up = CGEvent::new_keyboard_event(source, KEY_V, false)
        .map_err(|()| AppError::Inject("CGEvent key-up failed".into()))?;
    key_up.set_flags(CGEventFlags::CGEventFlagCommand);
    key_up.post(CGEventTapLocation::HID);

    Ok(())
}

/// PID of the frontmost application right now, via NSWorkspace. Captured at record
/// start so inject can restore focus to the user's app. `objc`/`cocoa` dialect to
/// match lib.rs (NSWorkspace/NSRunningApplication aren't in the cocoa bindings).
#[allow(deprecated, unexpected_cfgs)]
fn current_frontmost_pid() -> Option<i32> {
    use tauri_nspanel::cocoa::base::{id, nil};
    use tauri_nspanel::objc::{class, msg_send, sel, sel_impl};
    // SAFETY: read-only AppKit class accessors; NSWorkspace is process-wide and
    // safe to query off the main thread.
    unsafe {
        let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace == nil {
            return None;
        }
        let app: id = msg_send![workspace, frontmostApplication];
        if app == nil {
            return None;
        }
        let pid: i32 = msg_send![app, processIdentifier]; // pid_t == i32
        Some(pid)
    }
}

/// The user's main language as a coarse display label, from `NSLocale`'s first
/// preferred language. Used as the prepended language line in the enhance prompt when
/// the user hasn't picked one. objc dialect matches `current_frontmost_pid`.
#[allow(deprecated, unexpected_cfgs)]
fn system_language_label() -> Option<String> {
    use std::ffi::CStr;
    use std::os::raw::c_char;
    use tauri_nspanel::cocoa::base::{id, nil};
    use tauri_nspanel::objc::{class, msg_send, sel, sel_impl};
    // SAFETY: read-only Foundation accessors; NSLocale is process-wide.
    let code = unsafe {
        let langs: id = msg_send![class!(NSLocale), preferredLanguages];
        if langs == nil {
            return None;
        }
        let count: usize = msg_send![langs, count];
        if count == 0 {
            return None;
        }
        let first: id = msg_send![langs, objectAtIndex: 0usize];
        if first == nil {
            return None;
        }
        let utf8: *const c_char = msg_send![first, UTF8String];
        if utf8.is_null() {
            return None;
        }
        CStr::from_ptr(utf8).to_str().ok()?.to_string()
    };
    Some(label_for_language_code(&code))
}

/// Map a BCP-47-ish code ("zh-Hans-CN" / "en-US") to a coarse display label. Pure
/// so it's unit-testable; unknown languages pass through as the raw code.
fn label_for_language_code(code: &str) -> String {
    let primary = code.split('-').next().unwrap_or(code).to_ascii_lowercase();
    let label = match primary.as_str() {
        "zh" => "中文",
        "en" => "English",
        "ja" => "日本語",
        "ko" => "한국어",
        "fr" => "Français",
        "de" => "Deutsch",
        "es" => "Español",
        "ru" => "Русский",
        _ => return code.to_string(),
    };
    label.to_string()
}

/// If Audie is the current frontmost app (a panel-button click stole key focus),
/// reactivate `target_pid` so the upcoming Cmd+V lands there. Returns true only
/// when it actually reactivated, so the caller can wait for the handoff. No-op
/// when we were never frontmost (hotkey path) or the target app is gone.
#[allow(deprecated, unexpected_cfgs)]
fn restore_focus_if_stolen(target_pid: i32) -> bool {
    use tauri_nspanel::cocoa::base::{id, nil, BOOL};
    use tauri_nspanel::objc::{class, msg_send, sel, sel_impl};
    let own_pid = std::process::id() as i32;
    // SAFETY: read-only accessors + activateWithOptions:0 (a cross-process request,
    // == Voxt's activate(options: [])); none mutate our own view hierarchy.
    unsafe {
        let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace == nil {
            return false;
        }
        let frontmost: id = msg_send![workspace, frontmostApplication];
        if frontmost == nil {
            return false;
        }
        let frontmost_pid: i32 = msg_send![frontmost, processIdentifier];
        if frontmost_pid != own_pid {
            return false; // focus wasn't stolen — leave the user's app alone
        }
        let target: id = msg_send![
            class!(NSRunningApplication),
            runningApplicationWithProcessIdentifier: target_pid
        ];
        if target == nil {
            return false;
        }
        // cocoa's BOOL is a Rust bool on this target — return it directly.
        let activated: BOOL = msg_send![target, activateWithOptions: 0u64];
        activated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_code_maps_to_label() {
        assert_eq!(label_for_language_code("zh-Hans-CN"), "中文");
        assert_eq!(label_for_language_code("en-US"), "English");
        assert_eq!(label_for_language_code("ja"), "日本語");
        // Unknown languages pass through as the raw code.
        assert_eq!(label_for_language_code("sv-SE"), "sv-SE");
    }

    // ---- P3.9 fn tap-toggle detection (pure state machine) ------------------

    #[test]
    fn fn_plain_tap_toggles() {
        let mut s = FnTapState::default();
        let down = Instant::now();
        assert!(!fn_tap_transition(&mut s, FnSignal::FnDown, down));
        assert!(fn_tap_transition(
            &mut s,
            FnSignal::FnUp,
            down + Duration::from_millis(120)
        ));
    }

    #[test]
    fn fn_plus_key_does_not_toggle() {
        // fn+arrow (Home/End/PageUp) must never fire the trigger.
        let mut s = FnTapState::default();
        let t = Instant::now();
        fn_tap_transition(&mut s, FnSignal::FnDown, t);
        fn_tap_transition(&mut s, FnSignal::OtherKey, t);
        assert!(!fn_tap_transition(
            &mut s,
            FnSignal::FnUp,
            t + Duration::from_millis(80)
        ));
    }

    #[test]
    fn fn_held_past_window_does_not_toggle() {
        let mut s = FnTapState::default();
        let down = Instant::now();
        fn_tap_transition(&mut s, FnSignal::FnDown, down);
        assert!(!fn_tap_transition(
            &mut s,
            FnSignal::FnUp,
            down + FN_TAP_MAX + Duration::from_millis(50)
        ));
    }

    #[test]
    fn fn_up_without_down_is_ignored() {
        let mut s = FnTapState::default();
        assert!(!fn_tap_transition(&mut s, FnSignal::FnUp, Instant::now()));
    }

    #[test]
    fn fn_tap_after_combo_still_toggles() {
        // other_pressed must reset on the next FnDown, or one fn+key combo would
        // poison every later tap.
        let mut s = FnTapState::default();
        let t = Instant::now();
        fn_tap_transition(&mut s, FnSignal::FnDown, t);
        fn_tap_transition(&mut s, FnSignal::OtherKey, t);
        fn_tap_transition(&mut s, FnSignal::FnUp, t + Duration::from_millis(50));
        let t2 = t + Duration::from_millis(1000);
        fn_tap_transition(&mut s, FnSignal::FnDown, t2);
        assert!(fn_tap_transition(
            &mut s,
            FnSignal::FnUp,
            t2 + Duration::from_millis(80)
        ));
    }

    // ---- P3.9 trigger string parsing ---------------------------------------

    #[test]
    fn parse_fn_is_case_insensitive() {
        for s in ["fn", "Fn", "FN"] {
            match parse_trigger(s).unwrap() {
                TriggerSpec::ModifierTap { flag } => {
                    assert_eq!(flag, CGEventFlags::CGEventFlagSecondaryFn)
                }
                TriggerSpec::Combo { .. } => panic!("expected fn modifier tap"),
            }
        }
    }

    #[test]
    fn parse_bare_modifier_is_a_tap() {
        match parse_trigger("Shift").unwrap() {
            TriggerSpec::ModifierTap { flag } => assert_eq!(flag, CGEventFlags::CGEventFlagShift),
            TriggerSpec::Combo { .. } => panic!("expected shift modifier tap"),
        }
    }

    #[test]
    fn parse_combo_collects_mods_and_main_key() {
        match parse_trigger("Ctrl+Shift+Space").unwrap() {
            TriggerSpec::Combo { keycode, mods } => {
                assert_eq!(keycode, 49); // kVK_Space
                assert_eq!(
                    mods,
                    CGEventFlags::CGEventFlagControl | CGEventFlags::CGEventFlagShift
                );
            }
            TriggerSpec::ModifierTap { .. } => panic!("expected combo"),
        }
    }

    #[test]
    fn parse_single_function_key_has_no_mods() {
        match parse_trigger("F13").unwrap() {
            TriggerSpec::Combo { keycode, mods } => {
                assert_eq!(keycode, 105); // kVK_F13
                assert_eq!(mods, CGEventFlags::empty());
            }
            TriggerSpec::ModifierTap { .. } => panic!("expected combo"),
        }
    }

    #[test]
    fn parse_rejects_unknown_key_and_missing_main() {
        assert!(parse_trigger("Ctrl+F21").is_err()); // F21 is out of the supported range
        assert!(parse_trigger("Ctrl+Shift").is_err()); // no main key
    }

    #[test]
    fn parse_accepts_letter_combo() {
        // Letters are valid as a combo main key (the recorder requires a modifier).
        match parse_trigger("Ctrl+Shift+D").unwrap() {
            TriggerSpec::Combo { keycode, mods } => {
                assert_eq!(keycode, 2); // kVK_ANSI_D
                assert_eq!(
                    mods,
                    CGEventFlags::CGEventFlagControl | CGEventFlags::CGEventFlagShift
                );
            }
            TriggerSpec::ModifierTap { .. } => panic!("expected combo"),
        }
    }

    #[test]
    fn parse_fn_combo() {
        // "Fn+Space" = fn as a combo modifier → Combo{Space, SecondaryFn}.
        match parse_trigger("Fn+Space").unwrap() {
            TriggerSpec::Combo { keycode, mods } => {
                assert_eq!(keycode, 49); // kVK_Space
                assert_eq!(mods, CGEventFlags::CGEventFlagSecondaryFn);
            }
            TriggerSpec::ModifierTap { .. } => panic!("expected combo"),
        }
    }

    // ---- P3.10 capture state machine ----------------------------------------

    fn captured(out: CaptureOutcome) -> Option<String> {
        match out {
            CaptureOutcome::Captured(s) => Some(s),
            _ => None,
        }
    }

    #[test]
    fn capture_bare_modifier_tap() {
        let mut s = CaptureState::default();
        let t = Instant::now();
        let down = capture_step(
            &mut s,
            CaptureEvent::ModifierDown(CGEventFlags::CGEventFlagShift),
            t,
        );
        assert!(matches!(down, CaptureOutcome::None));
        let up = capture_step(
            &mut s,
            CaptureEvent::ModifierUp(CGEventFlags::CGEventFlagShift),
            t + Duration::from_millis(80),
        );
        assert_eq!(captured(up).as_deref(), Some("Shift"));
    }

    #[test]
    fn capture_fn_tap() {
        let mut s = CaptureState::default();
        let t = Instant::now();
        capture_step(
            &mut s,
            CaptureEvent::ModifierDown(CGEventFlags::CGEventFlagSecondaryFn),
            t,
        );
        let up = capture_step(
            &mut s,
            CaptureEvent::ModifierUp(CGEventFlags::CGEventFlagSecondaryFn),
            t + Duration::from_millis(80),
        );
        assert_eq!(captured(up).as_deref(), Some("Fn"));
    }

    #[test]
    fn capture_combo_fn_combo_and_single() {
        let mut s = CaptureState::default();
        let combo = capture_step(
            &mut s,
            CaptureEvent::Key {
                keycode: 2,
                mods: CGEventFlags::CGEventFlagControl | CGEventFlags::CGEventFlagShift,
            },
            Instant::now(),
        );
        assert_eq!(captured(combo).as_deref(), Some("Ctrl+Shift+D"));

        let fn_combo = capture_step(
            &mut s,
            CaptureEvent::Key {
                keycode: 49,
                mods: CGEventFlags::CGEventFlagSecondaryFn,
            },
            Instant::now(),
        );
        assert_eq!(captured(fn_combo).as_deref(), Some("Fn+Space"));

        let single = capture_step(
            &mut s,
            CaptureEvent::Key {
                keycode: 105,
                mods: CGEventFlags::empty(),
            },
            Instant::now(),
        );
        assert_eq!(captured(single).as_deref(), Some("F13"));
    }

    #[test]
    fn capture_rejects_capslock_and_system_combo() {
        let mut s = CaptureState::default();
        let t = Instant::now();
        capture_step(
            &mut s,
            CaptureEvent::ModifierDown(CGEventFlags::CGEventFlagAlphaShift),
            t,
        );
        let caps = capture_step(
            &mut s,
            CaptureEvent::ModifierUp(CGEventFlags::CGEventFlagAlphaShift),
            t + Duration::from_millis(50),
        );
        assert!(matches!(caps, CaptureOutcome::Rejected(_)));

        let cmd_q = capture_step(
            &mut s,
            CaptureEvent::Key {
                keycode: 12, // Q
                mods: CGEventFlags::CGEventFlagCommand,
            },
            Instant::now(),
        );
        assert!(matches!(cmd_q, CaptureOutcome::Rejected(_)));
    }

    #[test]
    fn capture_ignores_multi_modifier_without_key() {
        // Ctrl down, Shift down, Shift up, Ctrl up — ambiguous, captures nothing.
        let mut s = CaptureState::default();
        let t = Instant::now();
        capture_step(
            &mut s,
            CaptureEvent::ModifierDown(CGEventFlags::CGEventFlagControl),
            t,
        );
        capture_step(
            &mut s,
            CaptureEvent::ModifierDown(CGEventFlags::CGEventFlagShift),
            t,
        );
        let up_shift = capture_step(
            &mut s,
            CaptureEvent::ModifierUp(CGEventFlags::CGEventFlagShift),
            t,
        );
        let up_ctrl = capture_step(
            &mut s,
            CaptureEvent::ModifierUp(CGEventFlags::CGEventFlagControl),
            t,
        );
        assert!(captured(up_shift).is_none());
        assert!(captured(up_ctrl).is_none());
    }

    #[test]
    fn relevant_mods_masks_capslock_noise() {
        // capslock (AlphaShift) must not leak into combo matching.
        let noisy = CGEventFlags::CGEventFlagControl
            | CGEventFlags::CGEventFlagShift
            | CGEventFlags::CGEventFlagAlphaShift;
        assert_eq!(
            relevant_mods(noisy),
            CGEventFlags::CGEventFlagControl | CGEventFlags::CGEventFlagShift
        );
    }

    #[test]
    #[ignore = "touches the user's macOS Keychain; run manually for P1.2 smoke verification"]
    fn keychain_secret_round_trip_and_delete() {
        let key = format!(
            "audie_test_keychain_round_trip_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let first = "test-secret-one";
        let second = "test-secret-two";

        let _ = keychain_delete_secret(&key);

        assert!(!keychain_has_secret(&key).unwrap());

        keychain_store_secret(&key, first).unwrap();
        assert!(keychain_has_secret(&key).unwrap());
        assert_eq!(keychain_read_secret(&key).unwrap(), first);

        keychain_store_secret(&key, second).unwrap();
        assert!(keychain_has_secret(&key).unwrap());
        assert_eq!(keychain_read_secret(&key).unwrap(), second);

        keychain_delete_secret(&key).unwrap();
        assert!(!keychain_has_secret(&key).unwrap());
        assert!(matches!(
            keychain_read_secret(&key),
            Err(AppError::Provider(_))
        ));
    }

    #[test]
    fn keychain_add_item_uses_voxt_style_accessible_policy_without_access_acl() {
        let value = CFData::from_buffer(b"secret");
        let item = keychain_add_item("test_key", &value);
        let accessible_key = unsafe { CFString::wrap_under_get_rule(kSecAttrAccessible) };
        let value_key = unsafe { CFString::wrap_under_get_rule(kSecValueData) };

        assert_eq!(item.len(), 5);
        assert!(item.find(value_key).is_some());
        assert!(item.find(accessible_key).is_some());
    }

    #[test]
    fn keychain_update_attributes_use_voxt_style_accessible_policy_without_access_acl() {
        let value = CFData::from_buffer(b"secret");
        let attrs = keychain_value_attributes(&value);
        let value_key = unsafe { CFString::wrap_under_get_rule(kSecValueData) };
        let accessible_key = unsafe { CFString::wrap_under_get_rule(kSecAttrAccessible) };

        assert_eq!(attrs.len(), 2);
        assert!(attrs.find(value_key).is_some());
        assert!(attrs.find(accessible_key).is_some());
    }
}
