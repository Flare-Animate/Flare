//! The font, size and style of text, shared by the Text tool's settings and the Properties of a
//! text object.

use crate::workspace::Workspace;
use egui::Ui;
use flare_document::TextSource;
use flare_ui::chrome;

/// Generic families every machine can set; they come first in the list.
const GENERIC: [&str; 3] = ["sans-serif", "serif", "monospace"];

/// How tall the list of families is, whatever it holds. The drop-down keeps the size it had when
/// it opened, so a list that shrank to the few names there were then (before the font worker
/// had listed the rest) cut the full list down to two or three rows.
const LIST_HEIGHT: f32 = 260.0;

impl Workspace {
    /// The Character section. With `object` it is that text object's own, and a change sets the
    /// text again; without, it is the settings new text is made with, and the text being typed.
    pub(crate) fn character_section(&mut self, ui: &mut Ui, object: Option<&TextSource>) {
        chrome::section(ui, "tool-text", &tr!(self, "opt-character"));
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
        chrome::row(ui, &tr!(self, "opt-family"), |ui| {
            change.family = self.family_picker(ui, &family);
        });
        chrome::row(ui, &tr!(self, "opt-size"), |ui| {
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
        let (bold_name, italic_name) = (tr!(self, "opt-bold"), tr!(self, "opt-italic"));
        chrome::row(ui, &tr!(self, "opt-style"), |ui| {
            let (mut now_bold, mut now_italic) = (bold, italic);
            ui.toggle_value(&mut now_bold, &bold_name);
            ui.toggle_value(&mut now_italic, &italic_name);
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
                    Err(error) => {
                        let message = tr!(self, "opt-text-error", &[("error", &error.to_string())]);
                        self.report(message);
                    }
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
        let (filter_hint, no_match, google) = (
            tr!(self, "opt-filter"),
            tr!(self, "opt-no-font"),
            tr!(self, "opt-google-fonts"),
        );
        let open = egui::ComboBox::from_id_salt("text_family")
            .selected_text(shown.clone())
            .width(150.0)
            // Room for the filter box above the list, so the drop-down never scrolls itself.
            .height(LIST_HEIGHT + 40.0)
            // egui closes a drop-down on any click by default, which would close it the moment
            // the filter box is clicked; it closes on a choice instead (below).
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show_ui(ui, |ui| {
                let field = ui.add(
                    egui::TextEdit::singleline(&mut filter)
                        .id_salt("text_family_filter")
                        .hint_text(&filter_hint)
                        .desired_width(150.0),
                );
                // Typing filters straight away when the list opens.
                if !self.text.family_filter_focused {
                    field.request_focus();
                    self.text.family_filter_focused = true;
                }
                let wanted = filter.to_lowercase();
                let matches =
                    |name: &str| wanted.is_empty() || name.to_lowercase().contains(&wanted);
                // The families here, then those Google Fonts has that are downloaded when chosen.
                let mut rows: Vec<Row> = GENERIC
                    .into_iter()
                    .chain(self.text.families.iter().map(String::as_str))
                    .filter(|name| matches(name))
                    .map(Row::Family)
                    .collect();
                let downloadable = self
                    .text
                    .google
                    .iter()
                    .copied()
                    .filter(|name| matches(name));
                let first = rows.len();
                rows.extend(downloadable.map(Row::Family));
                if rows.len() > first {
                    rows.insert(first, Row::Heading);
                }
                if rows.is_empty() {
                    ui.weak(&no_match);
                }
                // The rows are placed by this height, so it must be the one a selectable label is
                // drawn at: its text with padding, and never less than any control.
                let row_height = family_row_height(ui);
                egui::ScrollArea::vertical()
                    .min_scrolled_height(LIST_HEIGHT)
                    .max_height(LIST_HEIGHT)
                    .auto_shrink([false, false])
                    .show_rows(ui, row_height, rows.len(), |ui, range| {
                        for row in &rows[range] {
                            match row {
                                Row::Heading => {
                                    ui.weak(&google);
                                }
                                Row::Family(name) => {
                                    if ui.selectable_label(*name == shown, *name).clicked() {
                                        chosen = Some((*name).to_owned());
                                        ui.close();
                                    }
                                }
                            }
                        }
                    });
            })
            .inner
            .is_some();
        // Focused again the next time it opens.
        self.text.family_filter_focused &= open;
        self.text.family_filter = filter;
        chosen
    }
}

/// How tall a row of the family list is drawn.
pub(in crate::workspace) fn family_row_height(ui: &Ui) -> f32 {
    let text = ui.text_style_height(&egui::TextStyle::Button) + 2.0 * ui.spacing().button_padding.y;
    ui.spacing().interact_size.y.max(text)
}

/// A line of the family list: a family, or the heading over the Google Fonts ones.
enum Row<'a> {
    Family(&'a str),
    Heading,
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
