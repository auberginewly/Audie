use super::*;

/// Max fn hold that still counts as a "tap": longer presses (resting on fn) and
/// fn+key combos don't toggle. SPEC §5.8 P3.9.
pub(in crate::platform::macos) const FN_TAP_MAX: Duration = Duration::from_millis(400);

#[derive(Default)]
pub(super) struct FnTapState {
    fn_down_at: Option<Instant>,
    other_pressed: bool,
}

pub(super) fn detect_combo(
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

pub(super) enum FnSignal {
    FnDown,
    FnUp,
    OtherKey,
}

pub(super) fn fn_tap_transition(state: &mut FnTapState, signal: FnSignal, at: Instant) -> bool {
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

pub(super) fn detect_modifier_tap(
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
