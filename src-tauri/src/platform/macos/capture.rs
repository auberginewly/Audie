use std::time::Instant;

use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventType, CGKeyCode, EventField,
};
use parking_lot::Mutex;
use tauri::{AppHandle, Emitter};

use super::hotkey::{
    modifier_flag_for_keycode, mods_tokens, name_for_keycode, relevant_mods, EventTapHandle,
    FN_TAP_MAX,
};
use super::permissions::{input_monitoring_granted, request_input_monitoring_access};
use crate::error::{AppError, AppResult};

use super::MacosPlatform;

const SYSTEM_BLOCKLIST: &[&str] = &["Cmd+Q", "Cmd+W", "Cmd+Tab", "Cmd+Space", "Cmd+H", "Cmd+M"];

enum CaptureEvent {
    ModifierDown(CGEventFlags),
    ModifierUp(CGEventFlags),
    Key {
        keycode: CGKeyCode,
        mods: CGEventFlags,
    },
    Ignore,
}

#[derive(Default)]
struct CaptureState {
    pending: Option<(CGEventFlags, Instant)>,
    disqualified: bool,
}

enum CaptureOutcome {
    None,
    Captured(String),
    Rejected(&'static str),
}

fn capture_step(state: &mut CaptureState, event: CaptureEvent, at: Instant) -> CaptureOutcome {
    match event {
        CaptureEvent::ModifierDown(flag) => {
            if state.pending.is_none() {
                state.pending = Some((flag, at));
                state.disqualified = false;
            } else {
                state.disqualified = true;
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
                None => return CaptureOutcome::None,
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
    pub(super) fn start_trigger_capture(&self, app: &AppHandle) -> AppResult<()> {
        if self.capture.lock().is_some() {
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
                CFRunLoop::run_current();
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

    pub(super) fn stop_trigger_capture(&self) {
        if let Some(mut handle) = self.capture.lock().take() {
            handle.runloop.stop();
            if let Some(thread) = handle.thread.take() {
                let _ = thread.join();
            }
            log::info!("trigger capture stopped");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            t + std::time::Duration::from_millis(80),
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
            t + std::time::Duration::from_millis(80),
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
            t + std::time::Duration::from_millis(50),
        );
        assert!(matches!(caps, CaptureOutcome::Rejected(_)));

        let cmd_q = capture_step(
            &mut s,
            CaptureEvent::Key {
                keycode: 12,
                mods: CGEventFlags::CGEventFlagCommand,
            },
            Instant::now(),
        );
        assert!(matches!(cmd_q, CaptureOutcome::Rejected(_)));
    }

    #[test]
    fn capture_ignores_multi_modifier_without_key() {
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
}
