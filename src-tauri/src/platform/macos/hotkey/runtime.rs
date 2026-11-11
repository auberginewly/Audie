use super::*;

impl MacosPlatform {
    fn trigger_slot(&self, slot: HotkeySlot) -> &Mutex<Option<EventTapHandle>> {
        match slot {
            HotkeySlot::Primary => &self.trigger,
            HotkeySlot::Compose => &self.compose_trigger,
        }
    }

    pub(in crate::platform::macos) fn start_trigger(
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

    pub(in crate::platform::macos) fn stop_trigger(&self, slot: HotkeySlot) {
        if let Some(mut handle) = self.trigger_slot(slot).lock().take() {
            handle.runloop.stop();
            if let Some(thread) = handle.thread.take() {
                let _ = thread.join();
            }
            log::info!("trigger stopped");
        }
    }

    pub(in crate::platform::macos) fn start_trigger_probe(&self, app: &AppHandle) -> AppResult<()> {
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

    pub(in crate::platform::macos) fn stop_trigger_probe(&self) -> AppResult<()> {
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
