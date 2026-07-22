use std::sync::mpsc;

use windows_sys::core::BOOL;
use windows_sys::Win32::Foundation::WPARAM;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, VK_DOWN,
    VK_ESCAPE, VK_F1, VK_F10, VK_F11, VK_F12, VK_F13, VK_F14, VK_F15, VK_F16, VK_F17, VK_F18,
    VK_F19, VK_F2, VK_F20, VK_F3, VK_F4, VK_F5, VK_F6, VK_F7, VK_F8, VK_F9, VK_LEFT, VK_RETURN,
    VK_RIGHT, VK_SPACE, VK_TAB, VK_UP,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, PeekMessageW, PostThreadMessageW, TranslateMessage, MSG,
    PM_NOREMOVE, WM_HOTKEY, WM_QUIT,
};

use crate::error::{AppError, AppResult};
use crate::platform::{HotkeyCallback, HotkeySlot};

const HOTKEY_ID: i32 = 1;
const WINDOWS_PRIMARY_FALLBACK: &str = "Ctrl+Shift+Space";

pub struct HotkeyHandle {
    thread_id: u32,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for HotkeyHandle {
    fn drop(&mut self) {
        if self.thread_id != 0 {
            // SAFETY: Category 8 — FFI boundary. `thread_id` is captured from the
            // hotkey thread after its queue exists, and WM_QUIT carries no pointers.
            unsafe {
                PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0);
            }
        }
        if let Some(thread) = self.thread.take() {
            if let Err(err) = thread.join() {
                log::warn!("join Windows hotkey thread failed: {err:?}");
            }
        }
    }
}

pub fn register(
    slot: HotkeySlot,
    combo: &str,
    callback: HotkeyCallback,
) -> AppResult<HotkeyHandle> {
    let spec = HotkeySpec::parse(slot, combo)?;
    let (tx, rx) = mpsc::channel::<Result<u32, String>>();
    let thread = std::thread::Builder::new()
        .name(format!("windows-hotkey-{slot:?}"))
        .spawn(move || run_hotkey_loop(spec, callback, tx))
        .map_err(|err| AppError::Internal(format!("spawn Windows hotkey thread: {err}")))?;

    match rx.recv() {
        Ok(Ok(thread_id)) => Ok(HotkeyHandle {
            thread_id,
            thread: Some(thread),
        }),
        Ok(Err(message)) => {
            let _ = thread.join();
            Err(AppError::Permission(message))
        }
        Err(err) => {
            let _ = thread.join();
            Err(AppError::Internal(format!(
                "start Windows hotkey thread: {err}"
            )))
        }
    }
}

fn run_hotkey_loop(
    spec: HotkeySpec,
    callback: HotkeyCallback,
    ready: mpsc::Sender<Result<u32, String>>,
) {
    let mut msg = MSG::default();
    // SAFETY: Category 8 — FFI boundary. Passing a valid, writable MSG pointer and
    // HWND null creates this thread's message queue without reading uninit memory.
    unsafe {
        PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
    }
    let thread_id = current_thread_id();
    // SAFETY: Category 8 — FFI boundary. HWND null requests a process-global
    // hotkey, HOTKEY_ID is owned by this dedicated thread, and modifiers/vk come
    // from the parser's Win32 constants.
    let registered = unsafe {
        RegisterHotKey(
            std::ptr::null_mut(),
            HOTKEY_ID,
            spec.modifiers | MOD_NOREPEAT,
            u32::from(spec.vk),
        )
    };
    if registered == 0 {
        let _ = ready.send(Err(format!(
            "注册 Windows 全局快捷键 {} 失败；请换一个未被系统占用的组合键",
            spec.label
        )));
        return;
    }
    let _ = ready.send(Ok(thread_id));

    loop {
        let mut msg = MSG::default();
        // SAFETY: Category 8 — FFI boundary. The MSG pointer is valid for writes
        // for the duration of the call; HWND null reads this thread's queue only.
        let result: BOOL = unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) };
        if result <= 0 {
            break;
        }
        if msg.message == WM_HOTKEY && msg.wParam == HOTKEY_ID as WPARAM {
            callback();
            continue;
        }
        // SAFETY: Category 8 — FFI boundary. `msg` was initialized by GetMessageW.
        unsafe {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    // SAFETY: Category 8 — FFI boundary. This thread registered HOTKEY_ID with a
    // null HWND above; unregistering the same pair releases that OS resource.
    unsafe {
        UnregisterHotKey(std::ptr::null_mut(), HOTKEY_ID);
    }
}

fn current_thread_id() -> u32 {
    // SAFETY: Category 8 — FFI boundary. GetCurrentThreadId takes no pointers and
    // returns the calling thread's numeric id.
    unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() }
}

#[derive(Clone)]
struct HotkeySpec {
    modifiers: u32,
    vk: u16,
    label: String,
}

impl HotkeySpec {
    fn parse(slot: HotkeySlot, combo: &str) -> AppResult<Self> {
        let normalized = if combo.eq_ignore_ascii_case("fn") && slot == HotkeySlot::Primary {
            WINDOWS_PRIMARY_FALLBACK
        } else {
            combo
        };
        parse_combo(normalized)
    }
}

fn parse_combo(combo: &str) -> AppResult<HotkeySpec> {
    let mut modifiers = 0;
    let mut main = None;
    for raw in combo.split('+') {
        let token = raw.trim();
        if token.is_empty() {
            continue;
        }
        match token.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= MOD_CONTROL,
            "alt" | "opt" | "option" => modifiers |= MOD_ALT,
            "shift" => modifiers |= MOD_SHIFT,
            "cmd" | "command" | "meta" | "super" | "win" | "windows" => {
                return Err(AppError::Internal(
                    "Windows MVP does not support the Win key as an Audie trigger".into(),
                ));
            }
            "fn" => {
                return Err(AppError::Internal(
                    "Windows does not expose Fn as a reliable global hotkey; use Ctrl+Shift+Space"
                        .into(),
                ));
            }
            _ => {
                let vk = virtual_key_for(token).ok_or_else(|| {
                    AppError::Internal(format!("unknown Windows trigger key: {token:?}"))
                })?;
                if main.replace(vk).is_some() {
                    return Err(AppError::Internal(format!(
                        "trigger {combo:?} has more than one main key"
                    )));
                }
            }
        }
    }
    let vk =
        main.ok_or_else(|| AppError::Internal(format!("trigger {combo:?} has no main key")))?;
    Ok(HotkeySpec {
        modifiers,
        vk,
        label: combo.to_string(),
    })
}

fn virtual_key_for(name: &str) -> Option<u16> {
    if name.len() == 1 {
        let byte = name.as_bytes()[0].to_ascii_uppercase();
        if byte.is_ascii_alphanumeric() {
            return Some(u16::from(byte));
        }
    }
    Some(match name.to_ascii_lowercase().as_str() {
        "space" => VK_SPACE,
        "return" | "enter" => VK_RETURN,
        "tab" => VK_TAB,
        "escape" | "esc" => VK_ESCAPE,
        "left" => VK_LEFT,
        "right" => VK_RIGHT,
        "down" => VK_DOWN,
        "up" => VK_UP,
        "f1" => VK_F1,
        "f2" => VK_F2,
        "f3" => VK_F3,
        "f4" => VK_F4,
        "f5" => VK_F5,
        "f6" => VK_F6,
        "f7" => VK_F7,
        "f8" => VK_F8,
        "f9" => VK_F9,
        "f10" => VK_F10,
        "f11" => VK_F11,
        "f12" => VK_F12,
        "f13" => VK_F13,
        "f14" => VK_F14,
        "f15" => VK_F15,
        "f16" => VK_F16,
        "f17" => VK_F17,
        "f18" => VK_F18,
        "f19" => VK_F19,
        "f20" => VK_F20,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ctrl_shift_space_when_combo_is_standard() {
        let spec = HotkeySpec::parse(HotkeySlot::Primary, "Ctrl+Shift+Space").unwrap();

        assert_eq!(spec.modifiers, MOD_CONTROL | MOD_SHIFT);
        assert_eq!(spec.vk, VK_SPACE);
    }

    #[test]
    fn maps_primary_fn_to_windows_default_without_persisting_it() {
        let spec = HotkeySpec::parse(HotkeySlot::Primary, "Fn").unwrap();

        assert_eq!(spec.modifiers, MOD_CONTROL | MOD_SHIFT);
        assert_eq!(spec.vk, VK_SPACE);
        assert_eq!(spec.label, "Ctrl+Shift+Space");
    }

    #[test]
    fn rejects_fn_for_compose_slot() {
        assert!(HotkeySpec::parse(HotkeySlot::Compose, "Fn").is_err());
    }
}
