//! The Movie Scripts window: tabs for the classes and the strings of a movie, a search box, and
//! the chosen class written out beside the list.

use super::movie_scripts::{MovieScripts, ScriptsTab};
use super::*;

impl Workspace {
    pub(super) fn movie_scripts_window(&mut self, ctx: &egui::Context) {
        let Some(mut window) = self.movie_scripts.take() else {
            return;
        };
        let title = tr!(self, "scripts-title", &[("name", &window.name)]);
        let words = ScriptWords::new(self);
        let mut open = true;
        egui::Window::new(title)
            .open(&mut open)
            .default_size([760.0, 480.0])
            .resizable(true)
            .show(ctx, |ui| scripts_body(ui, &mut window, &words));
        if open {
            self.movie_scripts = Some(window);
        }
    }
}

/// The words, fetched before the window is drawn.
struct ScriptWords {
    classes: String,
    strings: String,
    find: String,
    find_hint: String,
    search: String,
    code: String,
    text_only: String,
    pick: String,
    truncated: String,
    note: String,
    uses: [String; 3],
}

impl ScriptWords {
    fn new(ws: &Workspace) -> Self {
        Self {
            classes: tr!(ws, "scripts-classes"),
            strings: tr!(ws, "scripts-strings"),
            find: tr!(ws, "scripts-find"),
            find_hint: tr!(ws, "scripts-find-hint"),
            search: tr!(ws, "scripts-search"),
            code: tr!(ws, "scripts-code"),
            text_only: tr!(ws, "scripts-text-only"),
            pick: tr!(ws, "scripts-pick-class"),
            truncated: tr!(ws, "scripts-truncated"),
            note: tr!(ws, "scripts-note"),
            uses: [
                tr!(ws, "scripts-use-text"),
                tr!(ws, "scripts-use-name"),
                tr!(ws, "scripts-use-other"),
            ],
        }
    }
}

fn scripts_body(ui: &mut egui::Ui, window: &mut MovieScripts, words: &ScriptWords) {
    let mut search = false;
    ui.horizontal(|ui| {
        ui.selectable_value(&mut window.tab, ScriptsTab::Classes, &words.classes);
        ui.selectable_value(&mut window.tab, ScriptsTab::Strings, &words.strings);
        ui.separator();
        ui.label(&words.find);
        let field = ui.add(
            egui::TextEdit::singleline(&mut window.find)
                .hint_text(&words.find_hint)
                .desired_width(200.0),
        );
        if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            search = true;
        }
        if ui.button(&words.search).clicked() {
            search = true;
        }
    });
    if window.tab == ScriptsTab::Strings
        && ui
            .checkbox(&mut window.text_only, &words.text_only)
            .changed()
    {
        search = true;
    }
    if search {
        window.search();
        window.chosen = None;
    }
    ui.separator();
    match window.tab {
        ScriptsTab::Classes => classes_tab(ui, window, words),
        ScriptsTab::Strings => strings_tab(ui, window, words),
    }
    ui.add_space(4.0);
    ui.label(egui::RichText::new(&words.note).color(flare_ui::theme::text_dim()));
}

fn classes_tab(ui: &mut egui::Ui, window: &mut MovieScripts, words: &ScriptWords) {
    if ui.checkbox(&mut window.code, &words.code).changed()
        && let Some((name, _)) = window.chosen.clone()
    {
        window.choose(&name);
    }
    let mut pick = None;
    ui.columns(2, |columns| {
        egui::ScrollArea::vertical()
            .id_salt("script_classes")
            .max_height(360.0)
            .show(&mut columns[0], |ui| {
                for class in &window.outline.classes {
                    let selected = window
                        .chosen
                        .as_ref()
                        .is_some_and(|(name, _)| *name == class.full_name);
                    if ui.selectable_label(selected, &class.full_name).clicked() {
                        pick = Some(class.full_name.clone());
                    }
                }
            });
        egui::ScrollArea::both()
            .id_salt("script_source")
            .max_height(360.0)
            .show(&mut columns[1], |ui| match &window.chosen {
                None => {
                    ui.label(&words.pick);
                }
                Some((_, Err(error))) => {
                    ui.colored_label(flare_ui::theme::text_dim(), error);
                }
                Some((_, Ok(source))) => {
                    ui.add(
                        egui::Label::new(egui::RichText::new(&source.text).monospace())
                            .selectable(true),
                    );
                    if source.truncated {
                        ui.label(&words.truncated);
                    }
                }
            });
    });
    if let Some(name) = pick {
        window.choose(&name);
    }
}

fn strings_tab(ui: &mut egui::Ui, window: &MovieScripts, words: &ScriptWords) {
    egui::ScrollArea::both()
        .id_salt("script_strings")
        .max_height(360.0)
        .show(ui, |ui| {
            egui::Grid::new("script_strings_grid")
                .striped(true)
                .show(ui, |ui| {
                    for string in &window.strings.strings {
                        ui.label(egui::RichText::new(&string.text).monospace());
                        let u = string.uses;
                        let used = [
                            (u.text, &words.uses[0]),
                            (u.name, &words.uses[1]),
                            (u.other, &words.uses[2]),
                        ]
                        .iter()
                        .filter(|(n, _)| *n > 0)
                        .map(|(n, w)| format!("{w} {n}"))
                        .collect::<Vec<_>>()
                        .join(", ");
                        ui.label(used);
                        ui.end_row();
                    }
                });
        });
}
