use super::*;

#[test]
fn joins_chat_completions_endpoint() {
    assert_eq!(
        chat_completions_endpoint("https://api.deepseek.com/v1/"),
        "https://api.deepseek.com/v1/chat/completions"
    );
}

#[test]
fn classifies_openai_compatible_http_errors() {
    assert!(matches!(
        classify_http_status(401, ""),
        AppError::Provider(_)
    ));
    assert!(matches!(
        classify_http_status(403, ""),
        AppError::Network(_)
    ));
    assert!(matches!(
        classify_http_status(429, ""),
        AppError::Network(_)
    ));
    assert!(matches!(
        classify_http_status(500, ""),
        AppError::Network(_)
    ));
}
