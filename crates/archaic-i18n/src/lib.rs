//! Embedded Fluent catalogs; no filesystem or network access during translation.
use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use unic_langid::LanguageIdentifier;

pub const ENGLISH: &str = include_str!("../../../locales/en.ftl");
pub const GERMAN: &str = include_str!("../../../locales/de.ftl");

pub struct Translator {
    primary: FluentBundle<FluentResource>,
    fallback: FluentBundle<FluentResource>,
}

fn bundle(locale: &str, source: &str) -> FluentBundle<FluentResource> {
    let id: LanguageIdentifier = locale.parse().expect("embedded locale is valid");
    let mut bundle = FluentBundle::new(vec![id]);
    let resource = FluentResource::try_new(source.into()).expect("embedded catalog is valid");
    bundle
        .add_resource(resource)
        .expect("catalog has unique keys");
    bundle
}

impl Translator {
    pub fn new(locale: &str) -> Self {
        let german = locale
            .split(['-', '_', '.'])
            .next()
            .is_some_and(|s| s.eq_ignore_ascii_case("de"));
        Self {
            primary: bundle(
                if german { "de" } else { "en" },
                if german { GERMAN } else { ENGLISH },
            ),
            fallback: bundle("en", ENGLISH),
        }
    }

    pub fn text(&self, key: &str) -> String {
        self.format(key, None)
    }

    pub fn format(&self, key: &str, args: Option<&FluentArgs<'_>>) -> String {
        for bundle in [&self.primary, &self.fallback] {
            if let Some(pattern) = bundle.get_message(key).and_then(|message| message.value()) {
                let mut errors = Vec::new();
                let text = bundle.format_pattern(pattern, args, &mut errors);
                if errors.is_empty() {
                    return text.into_owned();
                }
            }
        }
        key.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chat_templates_preserve_literal_placeholders() {
        for locale in ["en", "de"] {
            let tr = Translator::new(locale);
            assert!(tr.text("chat-thread-one").contains("{count}"));
            assert!(tr.text("chat-thread-many").contains("{count}"));
            let reply = tr.text("chat-replying");
            assert!(reply.contains("{author}") && reply.contains("{preview}"));
            assert!(!tr.text("chat-composer-help").is_empty());
        }
    }
    #[test]
    fn catalogs_match_and_every_message_formats() {
        let keys = |source: &str| {
            source
                .lines()
                .filter_map(|line| line.split_once(" = ").map(|(key, _)| key.to_owned()))
                .collect::<std::collections::BTreeSet<_>>()
        };
        assert_eq!(keys(ENGLISH), keys(GERMAN));
        for locale in ["en", "de-AT", "de_DE.UTF-8", "fr"] {
            let translator = Translator::new(locale);
            for key in keys(ENGLISH) {
                assert_ne!(translator.text(&key), key);
            }
        }
    }
    #[test]
    fn unknown_language_falls_back_and_unknown_key_is_visible() {
        assert_eq!(Translator::new("ja").text("send"), "Send");
        assert_eq!(Translator::new("de-AT").text("send"), "Senden");
        assert_eq!(Translator::new("en").text("missing-key"), "missing-key");
    }
}
