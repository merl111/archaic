//! Plain-text HTTP(S) links projected onto CUI's native rich-text hit regions.
use cui::chat::{RichText, Span};
/// Render Matrix HTML as bounded native text spans. Ruma handles HTML parsing,
/// entities, sanitizing and reply fallbacks; no HTML or remote images execute.
pub fn formatted(html: &str, fallback: &str) -> RichText {
    if html.len() > 65536 {
        return parse(&crate::daylight::text(fallback, 65536));
    }
    let document = ruma_html::Html::parse(html);
    document.sanitize();
    let mut spans = Vec::new();
    for node in document.children() {
        html_node(&node, cui::chat::TextStyle::Body, None, &mut spans, 0);
    }
    while spans.last().is_some_and(|s: &Span| s.text == "\n") {
        spans.pop();
    }
    if spans.is_empty()
        || spans.len() > 128
        || spans.iter().map(|s| s.text.len()).sum::<usize>() > 65536
    {
        return parse(&crate::daylight::text(fallback, 65536));
    }
    RichText(spans)
}
fn html_node(
    node: &ruma_html::NodeRef,
    mut style: cui::chat::TextStyle,
    mut link: Option<String>,
    spans: &mut Vec<Span>,
    depth: usize,
) {
    use cui::chat::TextStyle;
    if depth > 100 || spans.len() > 128 {
        return;
    }
    if let Some(text) = node.as_text() {
        let text = text.borrow().to_string().replace('\0', "�");
        if !text.is_empty() {
            spans.push(Span { text, style, link });
        }
        return;
    }
    let Some(element) = node.as_element() else {
        return;
    };
    let tag = element.name.local.as_ref();
    if matches!(tag, "mx-reply" | "script" | "style") {
        return;
    }
    let block = matches!(
        tag,
        "p" | "div" | "blockquote" | "pre" | "li" | "h1" | "h2" | "h3" | "h4"
    );
    if block && !spans.is_empty() && !spans.last().unwrap().text.ends_with('\n') {
        spans.push(Span::plain("\n"));
    }
    match tag {
        "strong" | "b" | "em" | "i" | "h1" | "h2" | "h3" | "h4" => style = TextStyle::Strong,
        "code" | "pre" => style = TextStyle::Code,
        "blockquote" => {
            style = TextStyle::Muted;
            spans.push(Span::plain("│ "));
        }
        "li" => spans.push(Span::plain("• ")),
        "br" => spans.push(Span::plain("\n")),
        "a" => {
            link = element
                .attrs
                .borrow()
                .iter()
                .find(|a| a.name.local.as_ref() == "href")
                .map(|a| a.value.to_string())
                .filter(|s| s.len() <= 4096 && valid(s));
        }
        _ => {}
    }
    for child in node.children() {
        html_node(&child, style, link.clone(), spans, depth + 1);
    }
    if block && !spans.last().is_some_and(|s| s.text.ends_with('\n')) {
        spans.push(Span::plain("\n"));
    }
}
pub fn valid(s: &str) -> bool {
    url::Url::parse(s)
        .is_ok_and(|u| matches!(u.scheme(), "http" | "https") && u.host_str().is_some())
}
pub fn parse(body: &str) -> RichText {
    let mut spans = Vec::new();
    let mut rest = body;
    while spans.len() < 125 {
        let Some(start) = rest
            .find("https://")
            .into_iter()
            .chain(rest.find("http://"))
            .min()
        else {
            break;
        };
        let candidate = &rest[start..];
        let end = candidate
            .find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '\''))
            .unwrap_or(candidate.len());
        let mut link = candidate[..end].trim_end_matches(['.', ',', ';', ':', '!', '?']);
        while link.ends_with(')') && link.matches(')').count() > link.matches('(').count() {
            link = &link[..link.len() - 1];
        }
        if !valid(link) || link.len() > 4096 {
            let consumed = start + end.max(7).min(candidate.len());
            spans.push(Span::plain(&rest[..consumed]));
            rest = &rest[consumed..];
            continue;
        }
        if start > 0 {
            spans.push(Span::plain(&rest[..start]));
        }
        spans.push(Span {
            text: link.into(),
            link: Some(link.into()),
            ..Span::plain("")
        });
        rest = &rest[start + link.len()..];
    }
    if !rest.is_empty() {
        spans.push(Span::plain(rest));
    }
    let mut rich = Vec::new();
    for span in spans {
        if span.link.is_some() || rich.len() >= 125 {
            rich.push(span);
            continue;
        }
        let mut last = 0;
        for range in archaic_matrix::mentions::ranges(&span.text) {
            if rich.len() >= 125 {
                break;
            }
            if range.start > last {
                rich.push(Span::plain(&span.text[last..range.start]));
            }
            rich.push(Span {
                text: span.text[range.clone()].into(),
                style: cui::chat::TextStyle::Mention,
                link: None,
            });
            last = range.end;
        }
        if last < span.text.len() {
            rich.push(Span::plain(&span.text[last..]));
        }
    }
    // CUI accepts at most 128 spans. Preserve all remaining text when coalescing.
    if rich.len() > 128 {
        let tail = rich.drain(127..).map(|s| s.text).collect::<String>();
        rich.push(Span::plain(tail));
    }
    RichText(rich)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matrix_formatting_is_native_bounded_and_removes_reply_fallbacks() {
        use cui::chat::TextStyle;
        let rich = formatted(
            "<mx-reply>old quote</mx-reply><p>Hello <strong>world</strong> &amp; <code>code</code></p><ul><li>One</li></ul><a href='https://example.org'>safe</a><a href='javascript:alert(1)'>text</a>",
            "fallback",
        );
        assert!(!rich.plain().contains("old quote"));
        assert!(rich.plain().contains("Hello world & code"));
        assert!(rich.plain().contains("• One"));
        assert!(
            rich.0
                .iter()
                .any(|s| s.text == "world" && matches!(s.style, TextStyle::Strong))
        );
        assert!(
            rich.0
                .iter()
                .any(|s| s.text == "code" && matches!(s.style, TextStyle::Code))
        );
        assert_eq!(
            rich.0
                .iter()
                .filter_map(|s| s.link.as_deref())
                .collect::<Vec<_>>(),
            vec!["https://example.org"]
        );
        assert_eq!(
            formatted(&"<b>x</b>".repeat(200), "fallback").plain(),
            "fallback"
        );
    }
    #[test]
    fn links_preserve_text_and_punctuation_and_wrapped_urls() {
        let body = "Read (https://example.org/über_(one)). Then https://app.notion.com/p/long?source=copy_link\nThanks";
        let rich = parse(body);
        assert_eq!(
            rich.0.iter().map(|s| s.text.as_str()).collect::<String>(),
            body
        );
        let links: Vec<_> = rich.0.iter().filter_map(|s| s.link.as_deref()).collect();
        assert_eq!(
            links,
            vec![
                "https://example.org/über_(one)",
                "https://app.notion.com/p/long?source=copy_link"
            ]
        );
        assert!(!valid("javascript:alert(1)"));
        assert!(!valid("file:///tmp/test"));
    }
    #[test]
    fn many_links_respect_native_span_limit_without_losing_text() {
        let text = "https://example.org/ ".repeat(200);
        let rich = parse(&text);
        assert!(rich.0.len() <= 128);
        assert_eq!(
            rich.0.iter().map(|s| s.text.as_str()).collect::<String>(),
            text
        );
    }
}
