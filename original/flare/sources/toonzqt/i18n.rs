//! Stable Fluent message IDs are independent of commands and user document names.
use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use serde::{Deserialize, Serialize};
use unic_langid::LanguageIdentifier;

pub const ENGLISH: &str = include_str!("locales/en-US.ftl");
pub const FRENCH: &str = include_str!("locales/fr.ftl");

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Locale {
    #[default]
    English,
    French,
    Expanded,
    RtlPreview,
}

impl Locale {
    pub const ALL: [Self; 4] = [
        Self::English,
        Self::French,
        Self::Expanded,
        Self::RtlPreview,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::French => "Français",
            Self::Expanded => "locale-expanded",
            Self::RtlPreview => "locale-rtl",
        }
    }
    pub fn detect(tag: &str) -> Self {
        if tag
            .split(['-', '_'])
            .next()
            .is_some_and(|s| s.eq_ignore_ascii_case("fr"))
        {
            Self::French
        } else {
            Self::English
        }
    }
    pub fn rtl(self) -> bool {
        self == Self::RtlPreview
    }
}

pub struct Localizer {
    pub locale: Locale,
    english: FluentBundle<FluentResource>,
    translated: FluentBundle<FluentResource>,
}

fn bundle(tag: &str, source: &str) -> FluentBundle<FluentResource> {
    // Checked built-in catalogs; malformed resources degrade to their valid entries.
    let language = tag.parse::<LanguageIdentifier>().unwrap_or_default();
    let mut bundle = FluentBundle::new(vec![language]);
    let resource =
        FluentResource::try_new(source.to_owned()).unwrap_or_else(|(resource, _)| resource);
    let _ = bundle.add_resource(resource);
    bundle
}

impl Localizer {
    pub fn new(locale: Locale) -> Self {
        Self {
            locale,
            english: bundle("en-US", ENGLISH),
            translated: bundle("fr", FRENCH),
        }
    }
    pub fn text(&self, key: &str) -> String {
        self.format(key, &[])
    }
    pub fn format(&self, key: &str, values: &[(&str, &str)]) -> String {
        let mut args = FluentArgs::new();
        for (name, value) in values {
            args.set(*name, *value);
        }
        self.resolve(key, &args)
    }
    pub fn count(&self, key: &str, count: usize) -> String {
        let mut args = FluentArgs::new();
        args.set("count", count as i64);
        self.resolve(key, &args)
    }
    pub fn number(&self, value: f32, precision: usize) -> String {
        let n = format!("{value:.precision$}");
        if self.locale == Locale::French {
            n.replace('.', ",")
        } else {
            n
        }
    }
    fn resolve(&self, key: &str, args: &FluentArgs<'_>) -> String {
        let localized = if self.locale == Locale::French {
            Self::lookup(&self.translated, key, args)
        } else {
            None
        };
        let text = localized
            .or_else(|| Self::lookup(&self.english, key, args))
            .unwrap_or_else(|| format!("[{key}]"));
        if self.locale == Locale::Expanded || self.locale == Locale::RtlPreview {
            format!("[{text}{}]", "~".repeat(text.chars().count() / 3))
        } else {
            text
        }
    }
    fn lookup(
        bundle: &FluentBundle<FluentResource>,
        key: &str,
        args: &FluentArgs<'_>,
    ) -> Option<String> {
        let pattern = bundle.get_message(key)?.value()?;
        let mut errors = Vec::new();
        let value = bundle.format_pattern(pattern, Some(args), &mut errors);
        errors.is_empty().then(|| value.into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalogs_parse_and_all_ui_keys_resolve() {
        for source in [ENGLISH, FRENCH] {
            assert!(FluentResource::try_new(source.into()).is_ok());
        }
        // Scan UI and command source: adding a literal message key requires an English entry.
        let l = Localizer::new(Locale::English);
        for src in [
            include_str!("app.rs"),
            include_str!("commands.rs"),
            include_str!("chrome.rs"),
            include_str!("timeline.rs"),
            include_str!("timeline_ops.rs"),
            include_str!("layer_ops.rs"),
            include_str!("layer_types.rs"),
            include_str!("thumbnails.rs"),
            include_str!("frame_sets.rs"),
        ] {
            for fragment in src.split('"').skip(1).step_by(2) {
                if fragment.contains('-')
                    && !fragment.ends_with('-')
                    && fragment.chars().all(|c| c.is_ascii_lowercase() || c == '-')
                {
                    assert!(
                        l.english.has_message(fragment),
                        "missing UI message {fragment}"
                    );
                }
            }
        }
    }
    #[test]
    fn fallback_plural_interpolation_and_pseudo_are_separate_from_identity() {
        let fr = Localizer::new(Locale::French);
        assert_eq!(Locale::detect("fr-CA"), Locale::French);
        assert_eq!(Locale::detect("ja-JP"), Locale::English);
        assert!(fr.count("layer-count", 1).contains("calque"));
        assert!(fr.count("layer-count", 3).contains("calques"));
        assert!(fr
            .format("frame-position", &[("current", "7"), ("total", "24")])
            .contains('7'));
        assert_eq!(fr.number(12.5, 1), "12,5");
        let mut missing = Localizer::new(Locale::French);
        missing.translated = bundle("fr", "other = Autre");
        assert_eq!(missing.text("menu-file"), "File");
        let pseudo = Localizer::new(Locale::Expanded);
        assert!(pseudo.text("menu-file").len() > "File".len());
        assert!(Locale::RtlPreview.rtl());
    }
}
