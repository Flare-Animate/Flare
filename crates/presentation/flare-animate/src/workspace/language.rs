//! The interface's language: what the person chose, what the system speaks, and the words.

use flare_i18n::{Choice, Language, Translator};

use super::*;

impl Workspace {
    /// Tells the workspace what language the system is set to (a tag such as `de-DE`), for a
    /// choice of "System locale".
    pub fn set_system_locale(&mut self, tag: &str) {
        // A tag from outside is only compared, but keep it short all the same.
        self.system_locale = tag.chars().take(32).collect();
        self.rebuild_translator();
    }

    /// What was chosen for the language.
    pub fn language_choice(&self) -> Choice {
        self.language_choice
    }

    /// The language being spoken now.
    pub fn language(&self) -> Language {
        self.translator.language()
    }

    pub(super) fn set_language_choice(&mut self, choice: Choice) {
        if choice != self.language_choice {
            self.language_choice = choice;
            self.rebuild_translator();
        }
    }

    fn rebuild_translator(&mut self) {
        let language = self.language_choice.resolve(&self.system_locale);
        if language != self.translator.language() {
            self.translator = Translator::new(language);
        }
    }

    /// The message called `key` in the interface's language.
    pub(super) fn tr(&self, key: &str) -> String {
        self.translator.text(key)
    }

    /// The message called `key` with its `{ $name }` placeholders filled in.
    pub(super) fn tr_with(&self, key: &str, args: &[(&str, &str)]) -> String {
        self.translator.text_with(key, args)
    }

    /// The message called `key` for `count` things, named `{ $n }`; the message picks its plural
    /// form by the language's rules.
    pub(super) fn tr_count(&self, key: &str, count: u64) -> String {
        self.translator.text_count(key, count, &[])
    }

    /// [`Self::tr_count`] with other `{ $name }` placeholders to fill too.
    pub(super) fn tr_count_with(&self, key: &str, count: u64, args: &[(&str, &str)]) -> String {
        self.translator.text_count(key, count, args)
    }

    /// The name "System locale" carries in a list: the language it stands for.
    pub(super) fn system_choice_label(&self) -> String {
        let system = Language::from_system(&self.system_locale);
        self.translator.text_with(
            "prefs-language-system",
            &[("language", system.native_name())],
        )
    }

    /// Draws with the palette the person chose: the colors are read from it as they are used,
    /// and egui's own widgets are restyled when it changes.
    pub(super) fn ensure_palette(&mut self, ctx: &egui::Context) {
        flare_ui::theme::set_mode(self.theme_mode);
        if self.styled_for != Some(self.theme_mode) {
            self.styled_for = Some(self.theme_mode);
            flare_ui::theme::apply_palette(ctx);
        }
    }

    /// Puts the font of the interface's language first among the CJK ones the interface carries,
    /// when the language changes.
    pub(super) fn ensure_language_font(&mut self, ctx: &egui::Context) {
        let language = self.translator.language();
        if self.cjk_font_for == Some(language) {
            return;
        }
        self.cjk_font_for = Some(language);
        flare_ui::theme::set_cjk_priority(ctx, language.code());
    }
}
