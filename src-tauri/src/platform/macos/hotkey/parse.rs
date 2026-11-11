use super::*;

pub(in crate::platform::macos) fn relevant_mods(flags: CGEventFlags) -> CGEventFlags {
    flags
        & (CGEventFlags::CGEventFlagControl
            | CGEventFlags::CGEventFlagShift
            | CGEventFlags::CGEventFlagAlternate
            | CGEventFlags::CGEventFlagCommand
            | CGEventFlags::CGEventFlagSecondaryFn)
}

#[derive(Clone, Copy)]
pub(in crate::platform::macos) enum TriggerSpec {
    ModifierTap {
        flag: CGEventFlags,
    },
    Combo {
        keycode: CGKeyCode,
        mods: CGEventFlags,
    },
}

pub(in crate::platform::macos) fn parse_trigger(combo: &str) -> AppResult<TriggerSpec> {
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

pub(in crate::platform::macos) const TRIGGER_MODS: [(CGEventFlags, &str); 5] = [
    (CGEventFlags::CGEventFlagControl, "Ctrl"),
    (CGEventFlags::CGEventFlagAlternate, "Alt"),
    (CGEventFlags::CGEventFlagShift, "Shift"),
    (CGEventFlags::CGEventFlagCommand, "Cmd"),
    (CGEventFlags::CGEventFlagSecondaryFn, "Fn"),
];

pub(in crate::platform::macos) fn mods_tokens(mods: CGEventFlags) -> Vec<&'static str> {
    TRIGGER_MODS
        .iter()
        .filter(|(flag, _)| mods.contains(*flag))
        .map(|(_, token)| *token)
        .collect()
}
