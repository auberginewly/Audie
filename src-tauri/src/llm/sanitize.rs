// ── Output sanitizer ─────────────────────────────────────────────────────────
// Local thinking models (Qwen3) emit <think>…</think>, and some models wrap the
// answer in a preamble or code fence. Left in, they'd be injected at the cursor.
// We strip them and, when only reasoning is left, fall back to the raw transcript
// rather than inject garbage.

const PREAMBLE_MARKERS: [&str; 5] = [
    "Final Output:",
    "Result:",
    "Cleaned Text:",
    "输出：",
    "以下是",
];

pub(super) fn sanitize_enhance_output(raw: &str, fallback: &str) -> String {
    let cleaned = strip_think_blocks(raw);
    let cleaned = unwrap_code_fence(&cleaned);
    let cleaned = extract_after_preamble(&cleaned);
    let cleaned = cleaned.trim();
    if cleaned.is_empty() || looks_like_reasoning(cleaned) {
        return fallback.trim().to_string();
    }
    cleaned.to_string()
}

/// Case-insensitive byte position of an ASCII `needle`, scanning on char boundaries
/// so the returned index is always valid for slicing `haystack` (never mid-char).
/// Deliberately avoids the classic bug of indexing the original string with offsets
/// taken from a `to_lowercase()` copy, whose byte length can differ (e.g. 'ẞ', 'İ').
fn find_ci(haystack: &str, needle: &str) -> Option<usize> {
    let nlen = needle.len();
    haystack.char_indices().find_map(|(i, _)| {
        haystack
            .get(i..i + nlen)
            .filter(|slice| slice.eq_ignore_ascii_case(needle))
            .map(|_| i)
    })
}

fn strip_think_blocks(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = find_ci(rest, "<think>") {
        out.push_str(&rest[..open]);
        let after_open = &rest[open + "<think>".len()..];
        match find_ci(after_open, "</think>") {
            Some(close) => rest = &after_open[close + "</think>".len()..],
            None => rest = "", // unclosed tag: drop the remainder
        }
    }
    out.push_str(rest);
    // Drop any stray tags left behind.
    let out = strip_all(&out, "<think>");
    strip_all(&out, "</think>")
}

fn strip_all(text: &str, tag: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = find_ci(rest, tag) {
        out.push_str(&rest[..at]);
        rest = &rest[at + tag.len()..];
    }
    out.push_str(rest);
    out
}

fn unwrap_code_fence(text: &str) -> String {
    let trimmed = text.trim();
    if let Some(inner) = trimmed
        .strip_prefix("```")
        .and_then(|s| s.strip_suffix("```"))
    {
        // Drop an optional language tag on the fence's first line.
        let body = match inner.split_once('\n') {
            Some((first, rest)) if !first.contains(' ') && !first.contains('`') => rest,
            _ => inner,
        };
        return body.trim().to_string();
    }
    text.to_string()
}

fn extract_after_preamble(text: &str) -> String {
    let body = PREAMBLE_MARKERS
        .iter()
        .filter_map(|marker| rfind_ci_end(text, marker))
        .max()
        .map_or(text, |end| &text[end..]);
    body.trim()
        .trim_matches(|c| matches!(c, '"' | '\'' | '`' | '“' | '”' | '「' | '」'))
        .trim()
        .to_string()
}

/// Byte index just past the LAST case-insensitive match of an ASCII `needle`.
fn rfind_ci_end(haystack: &str, needle: &str) -> Option<usize> {
    let nlen = needle.len();
    haystack.char_indices().rev().find_map(|(i, _)| {
        haystack
            .get(i..i + nlen)
            .filter(|slice| slice.eq_ignore_ascii_case(needle))
            .map(|_| i + nlen)
    })
}

fn looks_like_reasoning(text: &str) -> bool {
    text.lines().take(3).any(|line| {
        let line = line.trim_start();
        // to_ascii_lowercase is length-preserving and only feeds starts_with, so no
        // char-boundary hazard (unlike full Unicode to_lowercase).
        let lower = line.to_ascii_lowercase();
        lower.starts_with("reasoning")
            || lower.starts_with("thinking")
            || starts_with_numbered(&lower, "step ")
            || starts_with_numbered(&lower, "rule ")
            || line.starts_with("思考过程")
            || line.starts_with("推理")
            || line.starts_with("分析：")
    })
}

fn starts_with_numbered(lower: &str, kw: &str) -> bool {
    lower
        .strip_prefix(kw)
        .and_then(|rest| rest.chars().next())
        .is_some_and(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests;
