use std::thread;
use std::time::Duration;

use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL, VK_V,
};

use crate::error::{AppError, AppResult};

pub(super) fn inject_text(app: &AppHandle, text: &str) -> AppResult<()> {
    app.clipboard()
        .write_text(text.to_string())
        .map_err(|err| AppError::Inject(format!("clipboard write failed: {err}")))?;
    thread::sleep(Duration::from_millis(20));
    send_ctrl_v()
}

fn send_ctrl_v() -> AppResult<()> {
    let inputs = [
        keyboard_input(VK_CONTROL, false),
        keyboard_input(VK_V, false),
        keyboard_input(VK_V, true),
        keyboard_input(VK_CONTROL, true),
    ];
    let input_size = i32::try_from(std::mem::size_of::<INPUT>())
        .map_err(|_| AppError::Internal("INPUT size does not fit i32".into()))?;
    // SAFETY: Category 8 — FFI boundary. `inputs` points to four initialized
    // INPUT records and the size argument matches the INPUT layout from windows-sys.
    let sent = unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), input_size) };
    if sent == inputs.len() as u32 {
        Ok(())
    } else {
        Err(AppError::Inject(format!(
            "SendInput Ctrl+V sent {sent}/{} events",
            inputs.len()
        )))
    }
}

fn keyboard_input(vk: u16, key_up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if key_up { KEYEVENTF_KEYUP } else { 0 },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}
