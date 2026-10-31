//! The words of the interface in each language it speaks.
//!
//! Every language has a folder under `localizations/` at the root of the repository with one
//! Fluent file per section of the interface (`localizations/es/preferences.ftl`). The files are
//! compiled in. A message missing from a language is shown in English, and one missing from
//! English is shown as its key, so an unfinished translation never breaks a screen. Sections are
//! added as the interface is moved over to them.

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

/// A language the interface is written in, in the order a list shows them: English, then the
/// languages written in Latin letters alphabetically by their own names, then the others by the
/// order of their language tags.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Language {
    English,
    German,
    Spanish,
    PortugueseBrazil,
    Japanese,
    Korean,
    ChineseSimplified,
}

/// The sections of the interface that have files, in every language.
pub const SECTIONS: [&str; 3] = ["common", "preferences", "publish"];

macro_rules! sections {
    ($lang:literal) => {
        [
            include_str!(concat!("../../../../localizations/", $lang, "/common.ftl")),
            include_str!(concat!(
                "../../../../localizations/",
                $lang,
                "/preferences.ftl"
            )),
            include_str!(concat!("../../../../localizations/", $lang, "/publish.ftl")),
        ]
    };
}

impl Language {
    /// In list order (see the type's description).
    pub const ALL: [Self; 7] = [
        Self::English,
        Self::German,
        Self::Spanish,
        Self::PortugueseBrazil,
        Self::Japanese,
        Self::Korean,
        Self::ChineseSimplified,
    ];

    /// The language's tag, which is also the name of its folder.
    pub fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::German => "de",
            Self::Spanish => "es",
            Self::PortugueseBrazil => "pt-BR",
            Self::Japanese => "ja",
            Self::Korean => "ko",
            Self::ChineseSimplified => "zh-CN",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| l.code() == code)
    }

    /// The language's name in English.
    pub fn english_name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::German => "German",
            Self::Spanish => "Spanish",
            Self::PortugueseBrazil => "Portuguese (Brazil)",
            Self::Japanese => "Japanese",
            Self::Korean => "Korean",
            Self::ChineseSimplified => "Chinese (Simplified)",
        }
    }

    /// The language's name in itself, as a language list shows it.
    pub fn native_name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::German => "Deutsch",
            Self::Spanish => "Español",
            Self::PortugueseBrazil => "Português (Brasil)",
            Self::Japanese => "日本語",
            Self::Korean => "한국어",
            Self::ChineseSimplified => "简体中文",
        }
    }

    /// The language that fits a system locale tag such as `de-AT`, `zh_Hans_CN` or `pt-PT`:
    /// the nearest one this interface speaks, English when there is none.
    pub fn from_system(tag: &str) -> Self {
        let tag = tag.trim().replace('_', "-").to_ascii_lowercase();
        let mut parts = tag.split(['-', '.', '@']);
        match parts.next().unwrap_or("") {
            "de" => Self::German,
            "es" => Self::Spanish,
            "pt" => Self::PortugueseBrazil,
            "ja" => Self::Japanese,
            "ko" => Self::Korean,
            // Traditional Chinese is not here yet, and simplified text is the nearer of the two
            // for a reader who knows either.
            "zh" => Self::ChineseSimplified,
            _ => Self::English,
        }
    }

    fn sources(self) -> [&'static str; SECTIONS.len()] {
        match self {
            Self::English => sections!("en"),
            Self::German => sections!("de"),
            Self::Spanish => sections!("es"),
            Self::PortugueseBrazil => sections!("pt-BR"),
            Self::Japanese => sections!("ja"),
            Self::Korean => sections!("ko"),
            Self::ChineseSimplified => sections!("zh-CN"),
        }
    }

    /// The text of section number `index` of [`SECTIONS`].
    pub fn section_source(self, index: usize) -> Option<&'static str> {
        self.sources().get(index).copied()
    }
}

/// What the person chose in Preferences: follow the system, or one language.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Choice {
    #[default]
    System,
    Language(Language),
}

impl Choice {
    /// How it is kept in the preferences file.
    pub fn code(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Language(language) => language.code(),
        }
    }

    /// A kept value back; anything not understood follows the system.
    pub fn from_code(code: &str) -> Self {
        Language::from_code(code).map_or(Self::System, Self::Language)
    }

    /// The language to speak given the system's locale tag.
    pub fn resolve(self, system_tag: &str) -> Language {
        match self {
            Self::System => Language::from_system(system_tag),
            Self::Language(language) => language,
        }
    }
}

fn bundle(language: Language) -> FluentBundle<FluentResource> {
    let id: LanguageIdentifier = language
        .code()
        .parse()
        .unwrap_or_else(|_| LanguageIdentifier::default());
    let mut bundle = FluentBundle::new_concurrent(vec![id]);
    // Isolation marks would show up as stray characters in egui text.
    bundle.set_use_isolating(false);
    for source in language.sources() {
        // A file with a broken message keeps the messages that did parse.
        let resource =
            FluentResource::try_new(source.to_owned()).unwrap_or_else(|(partial, _)| partial);
        // A key defined twice keeps its first meaning; it is a mistake the tests look for.
        let _ = bundle.add_resource(resource);
    }
    bundle
}

/// Looks messages up in one language, falling back to English and then to the key.
pub struct Translator {
    language: Language,
    bundle: FluentBundle<FluentResource>,
    english: Option<FluentBundle<FluentResource>>,
}

impl Translator {
    pub fn new(language: Language) -> Self {
        Self {
            language,
            bundle: bundle(language),
            english: (language != Language::English).then(|| bundle(Language::English)),
        }
    }

    pub fn language(&self) -> Language {
        self.language
    }

    /// The message called `key`.
    pub fn text(&self, key: &str) -> String {
        self.text_with(key, &[])
    }

    /// The message called `key` with its `{ $name }` placeholders filled from `args`.
    pub fn text_with(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut fluent_args = FluentArgs::new();
        for (name, value) in args {
            fluent_args.set(*name, FluentValue::from(*value));
        }
        for bundle in std::iter::once(&self.bundle).chain(self.english.as_ref()) {
            if let Some(pattern) = bundle.get_message(key).and_then(|m| m.value()) {
                let mut errors = Vec::new();
                return bundle
                    .format_pattern(pattern, Some(&fluent_args), &mut errors)
                    .into_owned();
            }
        }
        key.to_owned()
    }

    /// Whether the language itself, not the English behind it, has `key`.
    pub fn has(&self, key: &str) -> bool {
        self.bundle.has_message(key)
    }
}

#[cfg(test)]
mod tests;
