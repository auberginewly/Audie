use super::*;

#[test]
fn debug_and_release_builds_use_separate_keychain_services() {
    assert_eq!(
        keychain_service_for_build(true),
        "com.audie.app.dev.secure-storage"
    );
    assert_eq!(
        keychain_service_for_build(false),
        "com.audie.app.secure-storage"
    );
}

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
