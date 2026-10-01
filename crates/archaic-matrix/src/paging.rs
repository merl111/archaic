use std::collections::HashSet;
/// Every caller stages updates before committing a cursor, so failures are retryable.
#[derive(Clone, Default)]
pub(crate) struct Cursor {
    pub next: Option<String>,
    seen: HashSet<String>,
}
impl Cursor {
    pub fn advance(&mut self, next: Option<String>) -> Result<(), &'static str> {
        if next
            .as_ref()
            .is_some_and(|n| self.next.as_ref() == Some(n) || self.seen.contains(n))
        {
            return Err("page-stalled");
        }
        if self.seen.len() >= 4096 {
            return Err("tool-limit");
        }
        if let Some(old) = self.next.take() {
            self.seen.insert(old);
        }
        self.next = next;
        Ok(())
    }
    pub fn more(&self) -> bool {
        self.next.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_and_cyclic_tokens_fail_without_advancing() {
        let mut cursor = Cursor::default();
        cursor.advance(Some("one".into())).unwrap();
        assert_eq!(cursor.advance(Some("one".into())), Err("page-stalled"));
        assert_eq!(cursor.next.as_deref(), Some("one"));
        cursor.advance(Some("two".into())).unwrap();
        assert_eq!(cursor.advance(Some("one".into())), Err("page-stalled"));
        assert_eq!(cursor.next.as_deref(), Some("two"));
        cursor.advance(None).unwrap();
        assert!(!cursor.more());
    }
}
