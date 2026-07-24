use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;
use windows_sys::Win32::System::SystemServices::LOCALE_NAME_MAX_LENGTH;

pub(super) fn system_language_label() -> Option<String> {
    let mut buffer = [0u16; LOCALE_NAME_MAX_LENGTH as usize];
    let capacity = i32::try_from(LOCALE_NAME_MAX_LENGTH).expect("locale name capacity fits in i32");
    // SAFETY: Category 8 — FFI boundary. `buffer` is valid for writes of the
    // provided length; Windows writes a NUL-terminated UTF-16 locale name.
    let len = unsafe { GetUserDefaultLocaleName(buffer.as_mut_ptr(), capacity) };
    if len <= 1 {
        return None;
    }
    let locale = String::from_utf16_lossy(&buffer[..(len as usize - 1)]);
    Some(label_for_language_code(&locale).to_string())
}

fn label_for_language_code(code: &str) -> &'static str {
    let lowered = code.to_ascii_lowercase();
    if lowered.starts_with("zh") {
        "中文"
    } else {
        "English"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_chinese_locale_as_chinese() {
        assert_eq!(label_for_language_code("zh-CN"), "中文");
        assert_eq!(label_for_language_code("zh-Hans"), "中文");
    }

    #[test]
    fn labels_non_chinese_locale_as_english() {
        assert_eq!(label_for_language_code("en-US"), "English");
        assert_eq!(label_for_language_code("ja-JP"), "English");
    }
}
