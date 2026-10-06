//! Matrix identifiers in plain text, shared by rendering and outgoing mention metadata.
use matrix_sdk::ruma::{
    UserId,
    events::{Mentions, room::message::RoomMessageEventContent},
};
use std::ops::Range;

pub fn ranges(body: &str) -> Vec<Range<usize>> {
    let mut result = Vec::new();
    for (start, _) in body.match_indices('@') {
        if start > 0
            && !body[..start]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_whitespace() || matches!(c, '(' | '[' | '{' | '>' | '"'))
        {
            continue;
        }
        let candidate = &body[start..];
        let end = candidate
            .find(|c: char| {
                c.is_whitespace() || matches!(c, '<' | '>' | '"' | ')' | '}' | ',' | ';' | '!')
            })
            .unwrap_or(candidate.len());
        let name = candidate[..end].trim_end_matches(['.', '?']);
        if name == "@room" || (name.contains(':') && UserId::parse(name).is_ok()) {
            result.push(start..start + name.len());
            if result.len() == 64 {
                break;
            }
        }
    }
    result
}
pub(crate) fn metadata(body: &str) -> Mentions {
    let mut mentions = Mentions::new();
    for range in ranges(body) {
        let value = &body[range];
        if value == "@room" {
            mentions.room = true;
        } else if let Ok(user) = UserId::parse(value) {
            mentions.user_ids.insert(user);
        }
    }
    mentions
}
pub(crate) fn content(body: impl Into<String>) -> RoomMessageEventContent {
    let body = body.into();
    let mentions = metadata(&body);
    RoomMessageEventContent::text_plain(body).add_mentions(mentions)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mentions_are_bounded_and_not_email_addresses_or_url_fragments() {
        let body = "Hi @room, (@alice:example.org). a@b.org https://example.org/@bob:example.org @bob:example.org";
        let found: Vec<_> = ranges(body).into_iter().map(|r| &body[r]).collect();
        assert_eq!(found, ["@room", "@alice:example.org", "@bob:example.org"]);
        let m = metadata(body);
        assert!(m.room);
        assert_eq!(m.user_ids.len(), 2);
        assert!(ranges(&"@room ".repeat(100)).len() <= 64);
    }
    #[test]
    fn outgoing_content_contains_explicit_matrix_mentions() {
        let c = serde_json::to_value(content("Hi @alice:example.org @room")).unwrap();
        assert_eq!(c["m.mentions"]["user_ids"][0], "@alice:example.org");
        assert_eq!(c["m.mentions"]["room"], true);
        assert_eq!(
            serde_json::to_value(content("hello")).unwrap()["m.mentions"],
            serde_json::json!({})
        );
    }
}
