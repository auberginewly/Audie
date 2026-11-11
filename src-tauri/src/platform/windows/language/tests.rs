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
