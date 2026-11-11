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
