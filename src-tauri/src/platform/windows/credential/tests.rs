use super::*;

#[test]
fn target_name_namespaces_key_under_audie_service() {
    assert_eq!(
        target_name("openai_api_key"),
        "com.audie.app.secure-storage:openai_api_key"
    );
}
