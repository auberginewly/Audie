use super::selection_or_copy;

#[test]
fn empty_selection_must_not_copy_the_editors_current_line() {
    let selected = selection_or_copy(Some(String::new()), || {
        panic!("Cmd+C without a selection can copy the whole current line");
    });
    assert_eq!(selected, Some(String::new()));
}

#[test]
fn confirmed_selection_is_used_without_touching_the_clipboard() {
    let selected = selection_or_copy(Some("selected source".into()), || {
        panic!("a confirmed selection needs no clipboard probe");
    });
    assert_eq!(selected.as_deref(), Some("selected source"));
}

#[test]
fn unsupported_native_selection_retains_the_existing_copy_fallback() {
    assert_eq!(
        selection_or_copy(None, || Some("copied source".into())).as_deref(),
        Some("copied source")
    );
}
