use std::time::{Duration, Instant};

use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventType, CGKeyCode, EventField,
};
use parking_lot::Mutex;
use tauri::{AppHandle, Emitter};

use super::MacosPlatform;
use crate::error::{AppError, AppResult};
use crate::platform::{HotkeyCallback, HotkeySlot};

use super::permissions::{input_monitoring_granted, request_input_monitoring_access};

/// Max fn hold that still counts as a "tap": longer presses (resting on fn) and
/// fn+key combos don't toggle. SPEC §5.8 P3.9.
pub(super) const FN_TAP_MAX: Duration = Duration::from_millis(400);

#[derive(Default)]
struct FnTapState {
    fn_down_at: Option<Instant>,
    other_pressed: bool,
}

/// A live CGEventTap: the run loop it spins on (stoppable cross-thread) plus the
/// thread handle. Shared by the dev probe, production trigger, and capture tap.
pub(super) struct EventTapHandle {
    pub(super) runloop: CFRunLoop,
    pub(super) thread: Option<std::thread::JoinHandle<()>>,
}

/// fn key on macOS arrives as a `flagsChanged` event with this virtual keycode.
const KVK_FUNCTION: u16 = 63;

pub(super) fn relevant_mods(flags: CGEventFlags) -> CGEventFlags {
    flags
        & (CGEventFlags::CGEventFlagControl
            | CGEventFlags::CGEventFlagShift
            | CGEventFlags::CGEventFlagAlternate
            | CGEventFlags::CGEventFlagCommand
            | CGEventFlags::CGEventFlagSecondaryFn)
}

#[derive(Clone, Copy)]
pub(super) enum TriggerSpec {
    ModifierTap {
        flag: CGEventFlags,
    },
    Combo {
        keycode: CGKeyCode,
        mods: CGEventFlags,
    },
}

pub(super) fn parse_trigger(combo: &str) -> AppResult<TriggerSpec> {
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
        None if mods.bits().count_ones() == 1 => Ok(TriggerSpec::ModifierTap { flag: mods }),
        None => Err(AppError::Internal(format!(
            "trigger {combo:?} has no main key"
        ))),
    }
}

pub(super) fn keycode_for(name: &str) -> Option<CGKeyCode> {
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

pub(super) fn name_for_keycode(keycode: CGKeyCode) -> Option<&'static str> {
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

pub(super) fn modifier_flag_for_keycode(keycode: u16) -> Option<CGEventFlags> {
    match keycode {
        KVK_FUNCTION => Some(CGEventFlags::CGEventFlagSecondaryFn),
        56 | 60 => Some(CGEventFlags::CGEventFlagShift),
        59 | 62 => Some(CGEventFlags::CGEventFlagControl),
        58 | 61 => Some(CGEventFlags::CGEventFlagAlternate),
        54 | 55 => Some(CGEventFlags::CGEventFlagCommand),
        57 => Some(CGEventFlags::CGEventFlagAlphaShift),
        _ => None,
    }
}

pub(super) const TRIGGER_MODS: [(CGEventFlags, &str); 5] = [
    (CGEventFlags::CGEventFlagControl, "Ctrl"),
    (CGEventFlags::CGEventFlagAlternate, "Alt"),
    (CGEventFlags::CGEventFlagShift, "Shift"),
    (CGEventFlags::CGEventFlagCommand, "Cmd"),
    (CGEventFlags::CGEventFlagSecondaryFn, "Fn"),
];

pub(super) fn mods_tokens(mods: CGEventFlags) -> Vec<&'static str> {
    TRIGGER_MODS
        .iter()
        .filter(|(flag, _)| mods.contains(*flag))
        .map(|(_, token)| *token)
        .collect()
}

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

impl MacosPlatform {
    fn trigger_slot(&self, slot: HotkeySlot) -> &Mutex<Option<EventTapHandle>> {
        match slot {
            HotkeySlot::Primary => &self.trigger,
            HotkeySlot::Compose => &self.compose_trigger,
        }
    }

    pub(super) fn start_trigger(
        &self,
        slot: HotkeySlot,
        spec: TriggerSpec,
        callback: HotkeyCallback,
    ) -> AppResult<()> {
        if self.trigger_slot(slot).lock().is_some() {
            return Ok(());
        }

        if !input_monitoring_granted() {
            request_input_monitoring_access();
            if !input_monitoring_granted() {
                return Err(AppError::Permission(
                    "输入监控权限未授予；请到 系统设置 → 隐私与安全性 → 输入监控 启用 Audie，然后重启 App".into(),
                ));
            }
        }

        let (tx, rx) = std::sync::mpsc::channel::<Option<CFRunLoop>>();
        let thread_name = match slot {
            HotkeySlot::Primary => "trigger",
            HotkeySlot::Compose => "trigger-compose",
        };
        let thread = std::thread::Builder::new()
            .name(thread_name.into())
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
                CFRunLoop::run_current();
            })
            .map_err(|err| AppError::Internal(format!("spawn trigger thread: {err}")))?;

        match rx.recv() {
            Ok(Some(runloop)) => {
                *self.trigger_slot(slot).lock() = Some(EventTapHandle {
                    runloop,
                    thread: Some(thread),
                });
                log::info!("trigger started ({thread_name})");
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

    pub(super) fn stop_trigger(&self, slot: HotkeySlot) {
        if let Some(mut handle) = self.trigger_slot(slot).lock().take() {
            handle.runloop.stop();
            if let Some(thread) = handle.thread.take() {
                let _ = thread.join();
            }
            log::info!("trigger stopped");
        }
    }

    pub(super) fn start_trigger_probe(&self, app: &AppHandle) -> AppResult<()> {
        if self.probe.lock().is_some() {
            return Ok(());
        }

        if !input_monitoring_granted() {
            request_input_monitoring_access();
            if !input_monitoring_granted() {
                return Err(AppError::Permission(
                    "输入监控权限未授予；请到 系统设置 → 隐私与安全性 → 输入监控 启用 Audie，然后重启 App".into(),
                ));
            }
        }

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
                CFRunLoop::run_current();
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

    pub(super) fn stop_trigger_probe(&self) -> AppResult<()> {
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

enum FnSignal {
    FnDown,
    FnUp,
    OtherKey,
}

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

#[cfg(test)]
mod tests {
    use super::*;

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
                assert_eq!(keycode, 49);
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
                assert_eq!(keycode, 105);
                assert_eq!(mods, CGEventFlags::empty());
            }
            TriggerSpec::ModifierTap { .. } => panic!("expected combo"),
        }
    }

    #[test]
    fn parse_rejects_unknown_key_and_missing_main() {
        assert!(parse_trigger("Ctrl+F21").is_err());
        assert!(parse_trigger("Ctrl+Shift").is_err());
    }

    #[test]
    fn parse_accepts_letter_combo() {
        match parse_trigger("Ctrl+Shift+D").unwrap() {
            TriggerSpec::Combo { keycode, mods } => {
                assert_eq!(keycode, 2);
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
        match parse_trigger("Fn+Space").unwrap() {
            TriggerSpec::Combo { keycode, mods } => {
                assert_eq!(keycode, 49);
                assert_eq!(mods, CGEventFlags::CGEventFlagSecondaryFn);
            }
            TriggerSpec::ModifierTap { .. } => panic!("expected combo"),
        }
    }

    #[test]
    fn relevant_mods_masks_capslock_noise() {
        let noisy = CGEventFlags::CGEventFlagControl
            | CGEventFlags::CGEventFlagShift
            | CGEventFlags::CGEventFlagAlphaShift;
        assert_eq!(
            relevant_mods(noisy),
            CGEventFlags::CGEventFlagControl | CGEventFlags::CGEventFlagShift
        );
    }
}
