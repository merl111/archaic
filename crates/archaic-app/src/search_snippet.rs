//! Plain-text match emphasis for native result rows; offsets remain UTF-8 safe.
pub fn snippet(body: &str, query: &str, indexed: bool) -> String {
    let needle = query
        .split_whitespace()
        .filter(|term| {
            !indexed
                || !(term.starts_with("from:")
                    || term.starts_with("before:")
                    || term.starts_with("after:")
                    || *term == "has:file")
        })
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let mut folded = String::new();
    let mut source = Vec::new();
    for (start, ch) in body.char_indices() {
        let lower = ch.to_lowercase().collect::<String>();
        source.extend(std::iter::repeat_n(
            (start, start + ch.len_utf8()),
            lower.len(),
        ));
        folded.push_str(&lower);
    }
    let hit = (!needle.is_empty())
        .then(|| folded.find(&needle))
        .flatten()
        .map(|start| (source[start].0, source[start + needle.len() - 1].1));
    let chars = body
        .char_indices()
        .map(|(i, _)| i)
        .chain(std::iter::once(body.len()))
        .collect::<Vec<_>>();
    let start = hit
        .map(|(s, _)| chars.partition_point(|i| *i < s).saturating_sub(30))
        .unwrap_or(0);
    let end = (start + 110).min(chars.len() - 1);
    let mut result = String::new();
    if start > 0 {
        result.push('…');
    }
    for (offset, ch) in body[chars[start]..chars[end]].char_indices() {
        let byte = chars[start] + offset;
        if hit.is_some_and(|(a, _)| a == byte) {
            result.push('⟦');
        }
        result.push(if ch.is_control() { ' ' } else { ch });
        if hit.is_some_and(|(_, b)| b == byte + ch.len_utf8()) {
            result.push('⟧');
        }
    }
    if end < chars.len() - 1 {
        result.push('…');
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn highlights_utf8_and_case_expansion_without_splitting_characters() {
        assert_eq!(snippet("Hi CAFÉ!", "café", false), "Hi ⟦CAFÉ⟧!");
        assert_eq!(snippet("İstanbul", "i", false), "⟦İ⟧stanbul");
        assert_eq!(
            snippet("hello world", "world from:@a:local", true),
            "hello ⟦world⟧"
        );
        assert_eq!(snippet("😀\nhello", "has:file", true), "😀 hello");
        assert!(
            snippet(&format!("{} found", "😀".repeat(300)), "found", false).contains("⟦found⟧")
        );
    }
}
