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
