use core_foundation::base::{CFType, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::data::CFData;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::{CFString, CFStringRef};
use security_framework_sys::access_control::kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly;
use security_framework_sys::base::{errSecDuplicateItem, errSecItemNotFound, errSecSuccess};
use security_framework_sys::item::{
    kSecAttrAccount, kSecAttrService, kSecClass, kSecClassGenericPassword, kSecReturnData,
    kSecValueData,
};
use security_framework_sys::keychain_item::{
    SecItemAdd, SecItemCopyMatching, SecItemDelete, SecItemUpdate,
};

use crate::error::{AppError, AppResult};

// Store API keys as generic-password items using SecItem* directly (Voxt-style):
// service = "com.audie.app.secure-storage"; account = key_id; value = secret bytes.
const KEYCHAIN_SERVICE: &str = "com.audie.app.secure-storage";

pub(super) fn store_secret(key: &str, value: &str) -> AppResult<()> {
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

pub(super) fn has_secret(key: &str) -> AppResult<bool> {
    let status = sec_item_copy_matching_status(&keychain_base_query(key));
    if status == errSecSuccess {
        Ok(true)
    } else if status == errSecItemNotFound {
        Ok(false)
    } else {
        Err(AppError::Internal(format!("check secret: status {status}")))
    }
}

pub(super) fn read_secret(key: &str) -> AppResult<String> {
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

pub(super) fn delete_secret(key: &str) -> AppResult<()> {
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

extern "C" {
    static kSecAttrAccessible: CFStringRef;
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

        let _ = delete_secret(&key);

        assert!(!has_secret(&key).unwrap());

        store_secret(&key, first).unwrap();
        assert!(has_secret(&key).unwrap());
        assert_eq!(read_secret(&key).unwrap(), first);

        store_secret(&key, second).unwrap();
        assert!(has_secret(&key).unwrap());
        assert_eq!(read_secret(&key).unwrap(), second);

        delete_secret(&key).unwrap();
        assert!(!has_secret(&key).unwrap());
        assert!(matches!(read_secret(&key), Err(AppError::Provider(_))));
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
