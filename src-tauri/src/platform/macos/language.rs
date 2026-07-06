/// The user's main language as a coarse display label, from `NSLocale`'s first
/// preferred language. Used as the prepended language line in the enhance prompt when
/// the user hasn't picked one. objc dialect matches `current_frontmost_pid`.
#[allow(deprecated, unexpected_cfgs)]
pub(super) fn system_language_label() -> Option<String> {
    use std::ffi::CStr;
    use std::os::raw::c_char;
    use tauri_nspanel::cocoa::base::{id, nil};
    use tauri_nspanel::objc::{class, msg_send, sel, sel_impl};
    // SAFETY: read-only Foundation accessors; NSLocale is process-wide.
    let code = unsafe {
        let langs: id = msg_send![class!(NSLocale), preferredLanguages];
        if langs == nil {
            return None;
        }
        let count: usize = msg_send![langs, count];
        if count == 0 {
            return None;
        }
        let first: id = msg_send![langs, objectAtIndex: 0usize];
        if first == nil {
            return None;
        }
        let utf8: *const c_char = msg_send![first, UTF8String];
        if utf8.is_null() {
            return None;
        }
        CStr::from_ptr(utf8).to_str().ok()?.to_string()
    };
    Some(label_for_language_code(&code))
}

/// Map a BCP-47-ish code ("zh-Hans-CN" / "en-US") to a coarse display label. Pure
/// so it's unit-testable; unknown languages pass through as the raw code.
fn label_for_language_code(code: &str) -> String {
    let primary = code.split('-').next().unwrap_or(code).to_ascii_lowercase();
    let label = match primary.as_str() {
        "zh" => "中文",
        "en" => "English",
        "ja" => "日本語",
        "ko" => "한국어",
        "fr" => "Français",
        "de" => "Deutsch",
        "es" => "Español",
        "ru" => "Русский",
        _ => return code.to_string(),
    };
    label.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_code_maps_to_label() {
        assert_eq!(label_for_language_code("zh-Hans-CN"), "中文");
        assert_eq!(label_for_language_code("en-US"), "English");
        assert_eq!(label_for_language_code("ja"), "日本語");
        assert_eq!(label_for_language_code("sv-SE"), "sv-SE");
    }
}
