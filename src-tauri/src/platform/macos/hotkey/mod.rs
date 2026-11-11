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

/// A live CGEventTap: the run loop it spins on (stoppable cross-thread) plus the
/// thread handle. Shared by the dev probe, production trigger, and capture tap.
pub(super) struct EventTapHandle {
    pub(super) runloop: CFRunLoop,
    pub(super) thread: Option<std::thread::JoinHandle<()>>,
}

/// fn key on macOS arrives as a `flagsChanged` event with this virtual keycode.
const KVK_FUNCTION: u16 = 63;

mod gesture;
mod keymap;
mod parse;
mod runtime;
pub(super) use gesture::FN_TAP_MAX;
use gesture::{detect_combo, detect_modifier_tap, FnTapState};
#[cfg(test)]
use gesture::{fn_tap_transition, FnSignal};
pub(super) use keymap::{keycode_for, modifier_flag_for_keycode, name_for_keycode};
pub(super) use parse::{mods_tokens, parse_trigger, relevant_mods, TriggerSpec};
#[cfg(test)]
mod tests;
