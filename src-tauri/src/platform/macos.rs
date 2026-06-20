// macOS implementation of trait Platform.
//
// P0.1: hotkey via tauri-plugin-global-shortcut. The callback is parked in the
// shared HotkeyRegistry — the plugin's `with_handler` (built in lib.rs) is the
// single entry that dispatches into the registry.
//
// P0.4 adds clipboard-method inject (save → write → Cmd+V → restore). P1 adds
// Keychain Services for BYOK secrets.

use std::ffi::c_void;
use std::sync::Arc;
use std::time::Duration;

use core_foundation::base::{CFType, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::data::CFData;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use security_framework_sys::access_control::kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly;
use security_framework_sys::base::{errSecDuplicateItem, errSecItemNotFound, errSecSuccess};
use security_framework_sys::item::{
    kSecAttrAccount, kSecAttrService, kSecClass, kSecClassGenericPassword, kSecReturnData,
    kSecValueData,
};
use security_framework_sys::keychain_item::{
    SecItemAdd, SecItemCopyMatching, SecItemDelete, SecItemUpdate,
};
use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
use tauri_plugin_macos_permissions::{check_microphone_permission, request_microphone_permission};

use super::{HotkeyCallback, HotkeyRegistry, Platform};
use crate::error::{AppError, AppResult};

pub struct MacosPlatform {
    registry: Arc<HotkeyRegistry>,
}

impl MacosPlatform {
    pub fn new(registry: Arc<HotkeyRegistry>) -> Self {
        Self { registry }
    }
}

impl Platform for MacosPlatform {
    fn register_hotkey(
        &self,
        app: &AppHandle,
        combo: &str,
        callback: HotkeyCallback,
    ) -> AppResult<()> {
        let shortcut: Shortcut = combo
            .parse()
            .map_err(|err| AppError::Internal(format!("invalid hotkey combo {combo:?}: {err}")))?;

        self.registry.insert(shortcut, callback);

        app.global_shortcut()
            .register(shortcut)
            .map_err(|err| AppError::Internal(format!("failed to register hotkey: {err}")))?;

        Ok(())
    }

    fn unregister_all_hotkeys(&self, app: &AppHandle) -> AppResult<()> {
        if let Err(err) = app.global_shortcut().unregister_all() {
            log::warn!("failed to unregister all shortcuts: {err}");
        }
        self.registry.clear();
        Ok(())
    }

    fn inject_text(&self, app: &AppHandle, text: &str) -> AppResult<()> {
        // Clipboard method: most compatible across apps. Save the user's current
        // clipboard, paste our text, then restore. `read_text` fails when the
        // clipboard holds non-text (e.g. an image) — treat that as "nothing to
        // restore" rather than an error.
        let original = app.clipboard().read_text().ok();

        app.clipboard()
            .write_text(text.to_string())
            .map_err(|err| AppError::Inject(format!("clipboard write failed: {err}")))?;

        // Preflight Accessibility BEFORE simulating Cmd+V. Without that permission
        // CGEvent::post() silently drops the keystroke — paste never lands AND the
        // text would still get clobbered by clipboard restore. SPEC §3.7 says
        // "inject failed → text stays on clipboard for manual paste", so on a
        // preflight miss we keep the text on the pasteboard and return Permission
        // instead of touching restore.
        if !preflight_post_event_access() {
            // Best-effort: ask macOS to add Audie to the Accessibility list so the
            // user can flip the switch. Result is ignored — even if it returns
            // false (added but not granted) the error message tells them next steps.
            unsafe {
                let _ = CGRequestPostEventAccess();
            }
            return Err(AppError::Permission(
                "辅助功能权限未授予，文字已复制到剪贴板，可手动粘贴；请到 系统设置 → 隐私与安全性 → 辅助功能 启用 Audie".into(),
            ));
        }

        // Give the pasteboard a beat to settle before the synthetic paste.
        std::thread::sleep(Duration::from_millis(20));
        simulate_cmd_v()?;

        // The frontmost app reads the pasteboard asynchronously on Cmd+V;
        // restoring too early clobbers our text before it lands.
        std::thread::sleep(Duration::from_millis(120));
        if let Some(prev) = original {
            if let Err(err) = app.clipboard().write_text(prev) {
                log::warn!("failed to restore clipboard after inject: {err}");
            }
        }

        Ok(())
    }

    fn preferred_input_device_name(&self) -> Option<String> {
        pick_reliable_input()
    }

    fn ensure_microphone_permission(&self) -> bool {
        // `request` triggers requestAccess(.audio): it shows the prompt only when
        // status is NotDetermined (resolving once the user answers) and is a no-op
        // when already decided — but it doesn't report the decision, so we read it
        // back with `check`. Blocks the hotkey thread, not the UI thread, so fine.
        if let Err(err) = tauri::async_runtime::block_on(request_microphone_permission()) {
            log::warn!("request microphone permission: {err}");
        }
        tauri::async_runtime::block_on(check_microphone_permission())
    }

    fn store_secret(&self, key: &str, value: &str) -> AppResult<()> {
        keychain_store_secret(key, value)
    }

    fn has_secret(&self, key: &str) -> AppResult<bool> {
        keychain_has_secret(key)
    }

    fn read_secret(&self, key: &str) -> AppResult<String> {
        keychain_read_secret(key)
    }

    fn delete_secret(&self, key: &str) -> AppResult<()> {
        keychain_delete_secret(key)
    }
}

// ---- macOS Keychain Services (P1.2) -----------------------------------------
//
// Store API keys as generic-password items using SecItem* directly (Voxt-style):
//   service = "com.audie.app.secure-storage"
//   account = key_id (e.g. "groq_api_key")
//   value   = secret bytes
//
// Presence checks never request `kSecReturnData`, so opening the settings page
// can show "已配置 key" without asking macOS to unlock and reveal the secret.
// `read_secret` exists for provider calls, but it is never exposed to the frontend.

const KEYCHAIN_SERVICE: &str = "com.audie.app.secure-storage";
fn keychain_store_secret(key: &str, value: &str) -> AppResult<()> {
    let value_data = CFData::from_buffer(value.as_bytes());
    let query = keychain_base_query(key);
    let attrs = keychain_value_attributes(&value_data);

    let status = sec_item_copy_matching_status(&query);
    if status == errSecSuccess {
        sec_item_update(&query, &attrs, "update secret")
    } else if status == errSecItemNotFound {
        let item = keychain_add_item(key, &value_data);
        let add_status = sec_item_add(&item);
        if add_status == errSecSuccess {
            Ok(())
        } else if add_status == errSecDuplicateItem {
            sec_item_update(&query, &attrs, "update duplicate secret")
        } else {
            Err(AppError::Internal(format!(
                "add secret: status {add_status}"
            )))
        }
    } else {
        Err(AppError::Internal(format!(
            "lookup secret before write: status {status}"
        )))
    }
}

fn keychain_has_secret(key: &str) -> AppResult<bool> {
    let status = sec_item_copy_matching_status(&keychain_base_query(key));
    if status == errSecSuccess {
        Ok(true)
    } else if status == errSecItemNotFound {
        Ok(false)
    } else {
        Err(AppError::Internal(format!("check secret: status {status}")))
    }
}

fn keychain_read_secret(key: &str) -> AppResult<String> {
    let query = keychain_read_query(key);
    let mut item = std::ptr::null();
    let status = unsafe { SecItemCopyMatching(query.as_concrete_TypeRef(), &mut item) };
    if status == errSecSuccess {
        if item.is_null() {
            return Err(AppError::Internal("read secret returned null data".into()));
        }
        let data = unsafe { CFData::wrap_under_create_rule(item.cast()) };
        String::from_utf8(data.bytes().to_vec())
            .map_err(|_| AppError::Internal("keychain secret is not UTF-8".into()))
    } else if status == errSecItemNotFound {
        Err(AppError::Provider("secret not found".into()))
    } else {
        Err(AppError::Internal(format!("read secret: status {status}")))
    }
}

fn keychain_delete_secret(key: &str) -> AppResult<()> {
    let status = unsafe { SecItemDelete(keychain_base_query(key).as_concrete_TypeRef()) };
    if status == errSecSuccess || status == errSecItemNotFound {
        Ok(())
    } else {
        Err(AppError::Internal(format!(
            "delete secret: status {status}"
        )))
    }
}

fn keychain_base_query(key: &str) -> CFDictionary<CFString, CFType> {
    let class_key = unsafe { CFString::wrap_under_get_rule(kSecClass) };
    let class_value = unsafe { CFString::wrap_under_get_rule(kSecClassGenericPassword) };
    let service_key = unsafe { CFString::wrap_under_get_rule(kSecAttrService) };
    let service_value = CFString::new(KEYCHAIN_SERVICE);
    let account_key = unsafe { CFString::wrap_under_get_rule(kSecAttrAccount) };
    let account_value = CFString::new(key);

    CFDictionary::from_CFType_pairs(&[
        (class_key, class_value.as_CFType()),
        (service_key, service_value.as_CFType()),
        (account_key, account_value.as_CFType()),
    ])
}

fn keychain_read_query(key: &str) -> CFDictionary<CFString, CFType> {
    let class_key = unsafe { CFString::wrap_under_get_rule(kSecClass) };
    let class_value = unsafe { CFString::wrap_under_get_rule(kSecClassGenericPassword) };
    let service_key = unsafe { CFString::wrap_under_get_rule(kSecAttrService) };
    let service_value = CFString::new(KEYCHAIN_SERVICE);
    let account_key = unsafe { CFString::wrap_under_get_rule(kSecAttrAccount) };
    let account_value = CFString::new(key);
    let return_data_key = unsafe { CFString::wrap_under_get_rule(kSecReturnData) };
    let return_data_value = CFBoolean::true_value();

    CFDictionary::from_CFType_pairs(&[
        (class_key, class_value.as_CFType()),
        (service_key, service_value.as_CFType()),
        (account_key, account_value.as_CFType()),
        (return_data_key, return_data_value.as_CFType()),
    ])
}

fn keychain_value_attributes(value: &CFData) -> CFDictionary<CFString, CFType> {
    let value_key = unsafe { CFString::wrap_under_get_rule(kSecValueData) };
    let accessible_key = unsafe { CFString::wrap_under_get_rule(kSecAttrAccessible) };
    let accessible_value =
        unsafe { CFString::wrap_under_get_rule(kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly) };

    CFDictionary::from_CFType_pairs(&[
        (value_key, value.as_CFType()),
        (accessible_key, accessible_value.as_CFType()),
    ])
}

fn keychain_add_item(key: &str, value: &CFData) -> CFDictionary<CFString, CFType> {
    let class_key = unsafe { CFString::wrap_under_get_rule(kSecClass) };
    let class_value = unsafe { CFString::wrap_under_get_rule(kSecClassGenericPassword) };
    let service_key = unsafe { CFString::wrap_under_get_rule(kSecAttrService) };
    let service_value = CFString::new(KEYCHAIN_SERVICE);
    let account_key = unsafe { CFString::wrap_under_get_rule(kSecAttrAccount) };
    let account_value = CFString::new(key);
    let value_key = unsafe { CFString::wrap_under_get_rule(kSecValueData) };
    let accessible_key = unsafe { CFString::wrap_under_get_rule(kSecAttrAccessible) };
    let accessible_value =
        unsafe { CFString::wrap_under_get_rule(kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly) };

    CFDictionary::from_CFType_pairs(&[
        (class_key, class_value.as_CFType()),
        (service_key, service_value.as_CFType()),
        (account_key, account_value.as_CFType()),
        (value_key, value.as_CFType()),
        (accessible_key, accessible_value.as_CFType()),
    ])
}

fn sec_item_copy_matching_status(query: &CFDictionary<CFString, CFType>) -> i32 {
    unsafe { SecItemCopyMatching(query.as_concrete_TypeRef(), std::ptr::null_mut()) }
}

fn sec_item_add(item: &CFDictionary<CFString, CFType>) -> i32 {
    unsafe { SecItemAdd(item.as_concrete_TypeRef(), std::ptr::null_mut()) }
}

fn sec_item_update(
    query: &CFDictionary<CFString, CFType>,
    attrs: &CFDictionary<CFString, CFType>,
    label: &str,
) -> AppResult<()> {
    let status = unsafe { SecItemUpdate(query.as_concrete_TypeRef(), attrs.as_concrete_TypeRef()) };
    if status == errSecSuccess {
        Ok(())
    } else {
        Err(AppError::Internal(format!("{label}: status {status}")))
    }
}

/// Probe Accessibility (post-event) access. Returns true when CGEvent::post is
/// allowed to actually deliver events. The symbol is part of the ApplicationServices
/// framework which `core-graphics` already links, so no extra link flag needed.
fn preflight_post_event_access() -> bool {
    // SAFETY: parameterless C function from ApplicationServices.
    unsafe { CGPreflightPostEventAccess() }
}

extern "C" {
    static kSecAttrAccessible: CFStringRef;

    fn CGPreflightPostEventAccess() -> bool;
    fn CGRequestPostEventAccess() -> bool;
}

// ---- CoreAudio HAL: pick a non-Bluetooth input device (P0.7) -----------------
//
// AirPods/Bluetooth headsets in A2DP mode read literal zeros until macOS deigns
// to swap to HFP — and HFP also drops *system* audio quality to phone-grade.
// To dodge both, when the system default input is Bluetooth we look for a wired
// alternative (built-in mic, USB, etc.) and tell `AudioManager` to prefer that
// device by name. If only Bluetooth is available we leave it alone (caller falls
// back to the system default and silence detection covers the HFP gap).
//
// All `#[cfg(target_os = "macos")]` lives behind the Platform trait per §6.3.

type AudioObjectID = u32;
type OSStatus = i32;

#[repr(C)]
struct AudioObjectPropertyAddress {
    selector: u32,
    scope: u32,
    element: u32,
}

extern "C" {
    fn AudioObjectGetPropertyDataSize(
        object_id: AudioObjectID,
        in_address: *const AudioObjectPropertyAddress,
        in_qualifier_data_size: u32,
        in_qualifier_data: *const c_void,
        out_data_size: *mut u32,
    ) -> OSStatus;

    fn AudioObjectGetPropertyData(
        object_id: AudioObjectID,
        in_address: *const AudioObjectPropertyAddress,
        in_qualifier_data_size: u32,
        in_qualifier_data: *const c_void,
        io_data_size: *mut u32,
        out_data: *mut c_void,
    ) -> OSStatus;
}

const K_AUDIO_OBJECT_SYSTEM_OBJECT: AudioObjectID = 1;
const K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN: u32 = 0;
const K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL: u32 = fourcc(b"glob");
const K_AUDIO_OBJECT_PROPERTY_SCOPE_INPUT: u32 = fourcc(b"inpt");

const K_AUDIO_HARDWARE_PROPERTY_DEVICES: u32 = fourcc(b"dev#");
const K_AUDIO_HARDWARE_PROPERTY_DEFAULT_INPUT_DEVICE: u32 = fourcc(b"dIn ");
const K_AUDIO_DEVICE_PROPERTY_TRANSPORT_TYPE: u32 = fourcc(b"tran");
const K_AUDIO_DEVICE_PROPERTY_STREAMS: u32 = fourcc(b"stm#");
const K_AUDIO_OBJECT_PROPERTY_NAME: u32 = fourcc(b"lnam");
const K_AUDIO_DEVICE_TRANSPORT_TYPE_BUILT_IN: u32 = fourcc(b"bltn");
const K_AUDIO_DEVICE_TRANSPORT_TYPE_USB: u32 = fourcc(b"usb ");
const K_AUDIO_DEVICE_TRANSPORT_TYPE_BLUETOOTH: u32 = fourcc(b"blue");
const K_AUDIO_DEVICE_TRANSPORT_TYPE_BLUETOOTH_LE: u32 = fourcc(b"blea");
const K_AUDIO_DEVICE_TRANSPORT_TYPE_AIRPLAY: u32 = fourcc(b"airp");
// 'ccwd' / 'ccwl' — iPhone Continuity Capture (Mac uses your iPhone as mic).
// Same flakiness as Bluetooth: device entry can persist even when the phone
// isn't actively serving, and cpal often fails to open its stream config.
const K_AUDIO_DEVICE_TRANSPORT_TYPE_CONTINUITY_CAPTURE_WIRED: u32 = fourcc(b"ccwd");
const K_AUDIO_DEVICE_TRANSPORT_TYPE_CONTINUITY_CAPTURE_WIRELESS: u32 = fourcc(b"ccwl");

const fn fourcc(s: &[u8; 4]) -> u32 {
    ((s[0] as u32) << 24) | ((s[1] as u32) << 16) | ((s[2] as u32) << 8) | (s[3] as u32)
}

fn pick_reliable_input() -> Option<String> {
    // Score the system default. If it's already a reliable physical input
    // (built-in / USB), leave it alone — the user may have explicitly picked
    // it in System Settings.
    let default_id = read_property_scalar::<AudioObjectID>(
        K_AUDIO_OBJECT_SYSTEM_OBJECT,
        K_AUDIO_HARDWARE_PROPERTY_DEFAULT_INPUT_DEVICE,
        K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
    )?;
    let default_score = device_score(default_id);
    if default_score == 0 {
        return None;
    }

    log::info!(
        "system default input has unreliable transport (score {default_score}); \
         looking for a more reliable alternative"
    );

    // Scan every input device and pick the one with the lowest score that beats
    // the default. Ties don't trigger overrides — we only switch when we have a
    // strictly better candidate, so we never replace one flaky device with
    // another equally flaky one.
    let devices = read_property_array::<AudioObjectID>(
        K_AUDIO_OBJECT_SYSTEM_OBJECT,
        K_AUDIO_HARDWARE_PROPERTY_DEVICES,
        K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
    )?;
    let mut best: Option<(u8, AudioObjectID)> = None;
    for id in devices {
        if id == default_id || !has_input_streams(id) {
            continue;
        }
        let s = device_score(id);
        if s >= default_score {
            continue;
        }
        match best {
            Some((bs, _)) if bs <= s => {}
            _ => best = Some((s, id)),
        }
    }

    let (score, id) = best?;
    let name = read_device_name(id)?;
    log::info!("preferring more reliable input device (score {score}): {name}");
    Some(name)
}

fn device_score(device_id: AudioObjectID) -> u8 {
    let t = read_property_scalar::<u32>(
        device_id,
        K_AUDIO_DEVICE_PROPERTY_TRANSPORT_TYPE,
        K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
    )
    .unwrap_or(0);
    transport_score(t)
}

/// 0 = physical, cold-start reliable. 2 = needs a device handshake / availability
/// (Bluetooth A2DP↔HFP swap, iPhone Continuity, AirPlay) — fine when working,
/// often returns silence or fails to open on a cold press. 1 = unknown transport,
/// treat as "OK but prefer 0 if available".
fn transport_score(transport: u32) -> u8 {
    match transport {
        K_AUDIO_DEVICE_TRANSPORT_TYPE_BUILT_IN | K_AUDIO_DEVICE_TRANSPORT_TYPE_USB => 0,
        K_AUDIO_DEVICE_TRANSPORT_TYPE_BLUETOOTH
        | K_AUDIO_DEVICE_TRANSPORT_TYPE_BLUETOOTH_LE
        | K_AUDIO_DEVICE_TRANSPORT_TYPE_AIRPLAY
        | K_AUDIO_DEVICE_TRANSPORT_TYPE_CONTINUITY_CAPTURE_WIRED
        | K_AUDIO_DEVICE_TRANSPORT_TYPE_CONTINUITY_CAPTURE_WIRELESS => 2,
        _ => 1,
    }
}

/// A device counts as an input device iff querying its Streams property under
/// the Input scope returns at least one stream. Pure-output devices (speakers)
/// return size 0 here, which is how we filter them out cheaply.
fn has_input_streams(device_id: AudioObjectID) -> bool {
    let addr = AudioObjectPropertyAddress {
        selector: K_AUDIO_DEVICE_PROPERTY_STREAMS,
        scope: K_AUDIO_OBJECT_PROPERTY_SCOPE_INPUT,
        element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    };
    let mut size: u32 = 0;
    // SAFETY: addr is a valid stack pointer; size is an out-parameter.
    let status =
        unsafe { AudioObjectGetPropertyDataSize(device_id, &addr, 0, std::ptr::null(), &mut size) };
    status == 0 && size > 0
}

fn read_device_name(device_id: AudioObjectID) -> Option<String> {
    let addr = AudioObjectPropertyAddress {
        selector: K_AUDIO_OBJECT_PROPERTY_NAME,
        scope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
        element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    };
    let mut cf_str: CFStringRef = std::ptr::null();
    let mut size: u32 = std::mem::size_of::<CFStringRef>() as u32;
    // SAFETY: outData points to a single CFStringRef slot sized correctly.
    let status = unsafe {
        AudioObjectGetPropertyData(
            device_id,
            &addr,
            0,
            std::ptr::null(),
            &mut size,
            &mut cf_str as *mut _ as *mut c_void,
        )
    };
    if status != 0 || cf_str.is_null() {
        return None;
    }
    // kAudioObjectPropertyName follows the Create Rule: caller owns the +1
    // retain. wrap_under_create_rule adopts it (Drop will CFRelease).
    let s = unsafe { CFString::wrap_under_create_rule(cf_str) }.to_string();
    Some(s)
}

fn read_property_scalar<T: Default + Copy>(
    object: AudioObjectID,
    selector: u32,
    scope: u32,
) -> Option<T> {
    let addr = AudioObjectPropertyAddress {
        selector,
        scope,
        element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    };
    let mut value: T = T::default();
    let mut size: u32 = std::mem::size_of::<T>() as u32;
    // SAFETY: T is plain-old-data via Default+Copy; outData/size match.
    let status = unsafe {
        AudioObjectGetPropertyData(
            object,
            &addr,
            0,
            std::ptr::null(),
            &mut size,
            &mut value as *mut _ as *mut c_void,
        )
    };
    if status == 0 {
        Some(value)
    } else {
        None
    }
}

fn read_property_array<T: Default + Copy>(
    object: AudioObjectID,
    selector: u32,
    scope: u32,
) -> Option<Vec<T>> {
    let addr = AudioObjectPropertyAddress {
        selector,
        scope,
        element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    };
    let mut size: u32 = 0;
    // SAFETY: out-param.
    let status =
        unsafe { AudioObjectGetPropertyDataSize(object, &addr, 0, std::ptr::null(), &mut size) };
    if status != 0 || size == 0 {
        return None;
    }
    let count = size as usize / std::mem::size_of::<T>();
    let mut buf: Vec<T> = vec![T::default(); count];
    let mut size_inout = size;
    // SAFETY: buf is sized to match size_inout; T is POD via Default+Copy.
    let status = unsafe {
        AudioObjectGetPropertyData(
            object,
            &addr,
            0,
            std::ptr::null(),
            &mut size_inout,
            buf.as_mut_ptr() as *mut c_void,
        )
    };
    if status == 0 {
        Some(buf)
    } else {
        None
    }
}

/// Post a synthetic Cmd+V. Caller MUST preflight Accessibility — without it macOS
/// silently drops the events (post() still returns no error) and paste never lands.
fn simulate_cmd_v() -> AppResult<()> {
    const KEY_V: CGKeyCode = 9; // kVK_ANSI_V

    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
        .map_err(|()| AppError::Inject("CGEventSource creation failed".into()))?;

    let key_down = CGEvent::new_keyboard_event(source.clone(), KEY_V, true)
        .map_err(|()| AppError::Inject("CGEvent key-down failed".into()))?;
    key_down.set_flags(CGEventFlags::CGEventFlagCommand);
    key_down.post(CGEventTapLocation::HID);

    let key_up = CGEvent::new_keyboard_event(source, KEY_V, false)
        .map_err(|()| AppError::Inject("CGEvent key-up failed".into()))?;
    key_up.set_flags(CGEventFlags::CGEventFlagCommand);
    key_up.post(CGEventTapLocation::HID);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "touches the user's macOS Keychain; run manually for P1.2 smoke verification"]
    fn keychain_secret_round_trip_and_delete() {
        let key = format!(
            "audie_test_keychain_round_trip_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let first = "test-secret-one";
        let second = "test-secret-two";

        let _ = keychain_delete_secret(&key);

        assert!(!keychain_has_secret(&key).unwrap());

        keychain_store_secret(&key, first).unwrap();
        assert!(keychain_has_secret(&key).unwrap());
        assert_eq!(keychain_read_secret(&key).unwrap(), first);

        keychain_store_secret(&key, second).unwrap();
        assert!(keychain_has_secret(&key).unwrap());
        assert_eq!(keychain_read_secret(&key).unwrap(), second);

        keychain_delete_secret(&key).unwrap();
        assert!(!keychain_has_secret(&key).unwrap());
        assert!(matches!(
            keychain_read_secret(&key),
            Err(AppError::Provider(_))
        ));
    }

    #[test]
    fn keychain_add_item_uses_voxt_style_accessible_policy_without_access_acl() {
        let value = CFData::from_buffer(b"secret");
        let item = keychain_add_item("test_key", &value);
        let accessible_key = unsafe { CFString::wrap_under_get_rule(kSecAttrAccessible) };
        let value_key = unsafe { CFString::wrap_under_get_rule(kSecValueData) };

        assert_eq!(item.len(), 5);
        assert!(item.find(value_key).is_some());
        assert!(item.find(accessible_key).is_some());
    }

    #[test]
    fn keychain_update_attributes_use_voxt_style_accessible_policy_without_access_acl() {
        let value = CFData::from_buffer(b"secret");
        let attrs = keychain_value_attributes(&value);
        let value_key = unsafe { CFString::wrap_under_get_rule(kSecValueData) };
        let accessible_key = unsafe { CFString::wrap_under_get_rule(kSecAttrAccessible) };

        assert_eq!(attrs.len(), 2);
        assert!(attrs.find(value_key).is_some());
        assert!(attrs.find(accessible_key).is_some());
    }
}
