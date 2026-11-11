use super::*;

#[test]
fn parses_ctrl_shift_space_when_combo_is_standard() {
    let spec = HotkeySpec::parse(HotkeySlot::Primary, "Ctrl+Shift+Space").unwrap();

    assert_eq!(spec.modifiers, MOD_CONTROL | MOD_SHIFT);
    assert_eq!(spec.vk, VK_SPACE);
}

#[test]
fn rejects_fn_for_primary_slot() {
    assert!(HotkeySpec::parse(HotkeySlot::Primary, "Fn").is_err());
}

#[test]
fn rejects_fn_for_compose_slot() {
    assert!(HotkeySpec::parse(HotkeySlot::Compose, "Fn").is_err());
}
