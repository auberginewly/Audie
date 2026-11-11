use super::*;

#[test]
fn language_code_maps_to_label() {
    assert_eq!(label_for_language_code("zh-Hans-CN"), "中文");
    assert_eq!(label_for_language_code("en-US"), "English");
    assert_eq!(label_for_language_code("ja"), "日本語");
    assert_eq!(label_for_language_code("sv-SE"), "sv-SE");
}
