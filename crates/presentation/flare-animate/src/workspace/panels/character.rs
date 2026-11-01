//! The font, size and style of text, shared by the Text tool's settings and the Properties of a
//! text object.

use crate::workspace::Workspace;
use egui::Ui;
use flare_document::TextSource;
use flare_ui::chrome;

/// Generic families every machine can set; they come first in the list.
const GENERIC: [&str; 3] = ["sans-serif", "serif", "monospace"];

const ROW_HEIGHT: f32 = 20.0;

impl Workspace {
    /// The Character section. With `object` it is that text object's own, and a change sets the
    /// text again; without, it is the settings new text is made with, and the text being typed.
    pub(crate) fn character_section(&mut self, ui: &mut Ui, object: Option<&TextSource>) {
        chrome::section(ui, "tool-text", "Character");
        // While text is being typed the editor takes the settings when it finishes, so the
        // controls are the tool's whichever text it is.
        let object = object.filter(|_| self.text.edit.is_none());
        let (family, size, bold, italic) = match object {
            Some(source) => (
                source.family().to_owned(),
                source.size(),
                source.bold(),
                source.italic(),
            ),
            None => (
                self.text.family.clone(),
                self.text.size,
                self.text.bold,
                self.text.italic,
            ),
        };
        let mut change = TextSettings {
            family: None,
            size: None,
            bold: None,
            italic: None,
        };
        chrome::row(ui, "Family", |ui| {
            change.family = self.family_picker(ui, &family);
        });
        chrome::row(ui, "Size", |ui| {
            let mut shown = self.text.size_edit.unwrap_or(size);
            let response = ui.add(
                egui::DragValue::new(&mut shown)
                    .range(flare_text::MIN_SIZE..=flare_text::MAX_SIZE)
                    .suffix(" px"),
            );
            if object.is_none() {
                // The tool's own size follows at once.
                self.text.size = shown;
                self.text.size_edit = None;
            } else if response.changed() {
                self.text.size_edit = Some(shown);
            }
            // A size dragged or typed for a text object is set when it is let go.
            if (response.drag_stopped() || response.lost_focus())
                && let Some(done) = self.text.size_edit.take()
            {
                change.size = Some(done);
            }
        });
        chrome::row(ui, "Style", |ui| {
            let (mut now_bold, mut now_italic) = (bold, italic);
            ui.toggle_value(&mut now_bold, "Bold");
            ui.toggle_value(&mut now_italic, "Italic");
            change.bold = (now_bold != bold).then_some(now_bold);
            change.italic = (now_italic != italic).then_some(now_italic);
        });
        self.apply_character(object, change);
    }

    fn apply_character(&mut self, object: Option<&TextSource>, change: TextSettings) {
        if change.is_none() {
            return;
        }
        match object {
            None => {
                if let Some(family) = change.family {
                    self.text.family = family;
                }
                if let Some(size) = change.size {
                    self.text.size = size;
                }
                if let Some(bold) = change.bold {
                    self.text.bold = bold;
                }
                if let Some(italic) = change.italic {
                    self.text.italic = italic;
                }
            }
            Some(source) => {
                let made = TextSource::new(
                    source.text(),
                    change.family.unwrap_or_else(|| source.family().to_owned()),
                    change.size.unwrap_or(source.size()),
                    change.bold.unwrap_or(source.bold()),
                    change.italic.unwrap_or(source.italic()),
                    source.origin(),
                );
                match made {
                    Ok(next) => self.retext_selected(next),
                    Err(error) => self.report(format!("Cannot change the text: {error}")),
                }
            }
        }
    }

    /// A drop-down of the font families with a filter box. Returns the one chosen this frame.
    fn family_picker(&mut self, ui: &mut Ui, current: &str) -> Option<String> {
        self.ask_font_families();
        let shown = current.split(',').next().unwrap_or("").trim().to_owned();
        let mut filter = std::mem::take(&mut self.text.family_filter);
        let mut chosen = None;
        egui::ComboBox::from_id_salt("text_family")
            .selected_text(shown.clone())
            .width(150.0)
            .show_ui(ui, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut filter)
                        .hint_text("Filter")
                        .desired_width(150.0),
                );
                let wanted = filter.to_lowercase();
                let matches =
                    |name: &str| wanted.is_empty() || name.to_lowercase().contains(&wanted);
                let names: Vec<&str> = GENERIC
                    .into_iter()
                    .chain(self.text.families.iter().map(String::as_str))
                    .filter(|name| matches(name))
                    .collect();
                if names.is_empty() {
                    ui.weak("No font matches.");
                }
                egui::ScrollArea::vertical().max_height(260.0).show_rows(
                    ui,
                    ROW_HEIGHT,
                    names.len(),
                    |ui, rows| {
                        for name in &names[rows] {
                            if ui.selectable_label(*name == shown, *name).clicked() {
                                chosen = Some((*name).to_owned());
                            }
                        }
                    },
                );
            });
        self.text.family_filter = filter;
        chosen
    }
}

/// What the person changed in the Character section this frame.
struct TextSettings {
    family: Option<String>,
    size: Option<f32>,
    bold: Option<bool>,
    italic: Option<bool>,
}

impl TextSettings {
    fn is_none(&self) -> bool {
        self.family.is_none() && self.size.is_none() && self.bold.is_none() && self.italic.is_none()
    }
}
