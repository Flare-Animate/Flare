//! Animate's Create New Symbol and Convert to Symbol dialog (Insert > New Symbol, Ctrl+F8, and
//! Modify > Convert to Symbol, F8): the symbol's name, its type, the library folder it goes in
//! and, for a conversion, where its own origin sits on the artwork. The checks and the edit are
//! `flare_tools::library`'s, the same the command line and the MCP server use.

use flare_document::SymbolKind;
use flare_tools::library::{self, Registration, SymbolError};
use flare_ui::{chrome, dialog};

use super::panels::library::kind_icon;
use super::*;

/// The types in the order Animate's dialog lists them.
const KINDS: [SymbolKind; 3] = [
    SymbolKind::MovieClip,
    SymbolKind::Button,
    SymbolKind::Graphic,
];
/// Room for the label column and the three type cards.
const DIALOG_WIDTH: f32 = 400.0;

/// What the dialog makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SymbolPurpose {
    /// An empty symbol, edited in place once it is made.
    New,
    /// The selected shape, which an instance of the symbol replaces.
    Convert { scene: SceneId, at: SpanRef },
}

/// What the dialog holds while it is open.
pub(super) struct SymbolDialog {
    pub(super) purpose: SymbolPurpose,
    pub(super) name: String,
    pub(super) kind: SymbolKind,
    pub(super) registration: Registration,
    /// The name field takes the keyboard, its text selected, on the dialog's first frame.
    focus_name: bool,
}

impl Workspace {
    /// Insert > New Symbol: asks for the name and type of an empty symbol.
    pub(super) fn open_new_symbol_dialog(&mut self) {
        if self.in_symbol() {
            self.report(tr!(self, "lib-leave-editing"));
            return;
        }
        self.open_symbol_dialog(SymbolPurpose::New);
    }

    /// Modify > Convert to Symbol: asks for the name, type and registration of a symbol made
    /// from the selected shape.
    pub(super) fn open_convert_symbol_dialog(&mut self) {
        let (Some(at), Some(scene)) = (self.selected, self.scene_id()) else {
            self.report(tr!(self, "msg-select-shape-for-symbol"));
            return;
        };
        if !self.can_convert_selection() {
            self.report(tr!(self, "msg-select-shape-for-symbol"));
            return;
        }
        self.open_symbol_dialog(SymbolPurpose::Convert { scene, at });
    }

    fn open_symbol_dialog(&mut self, purpose: SymbolPurpose) {
        let name = library::free_symbol_name(self.document(), |n| {
            tr!(self, "lib-symbol-name", &[("n", &n.to_string())])
        });
        self.windows.symbol = Some(SymbolDialog {
            purpose,
            name,
            kind: self.library.new_kind,
            registration: self.library.registration,
            focus_name: true,
        });
    }

    /// Why the name in the dialog cannot be used, in the language being spoken.
    fn symbol_name_problem(&self, name: &str) -> Option<String> {
        match library::check_symbol_name(self.document(), name) {
            Ok(_) => None,
            Err(SymbolError::NameEmpty) => Some(tr!(self, "sym-name-empty")),
            Err(SymbolError::NameTooLong) => Some(tr!(self, "sym-name-too-long")),
            Err(_) => Some(tr!(self, "sym-name-taken", &[("name", name.trim())])),
        }
    }

    /// Makes what the open dialog describes, as one undo step, and closes it. A name that
    /// cannot be used keeps the dialog open; any other refusal closes it and says why.
    pub(super) fn confirm_symbol_dialog(&mut self) {
        let Some(form) = self.windows.symbol.take() else {
            return;
        };
        if self.symbol_name_problem(&form.name).is_some() {
            self.windows.symbol = Some(form);
            return;
        }
        let made = match form.purpose {
            SymbolPurpose::New => library::create_symbol(self.document(), &form.name, form.kind),
            SymbolPurpose::Convert { scene, at } => library::convert_to_symbol(
                self.document(),
                scene,
                at,
                &form.name,
                form.kind,
                form.registration,
            ),
        };
        let (commands, id) = match made {
            Ok(made) => made,
            Err(SymbolError::NotAShape) => {
                self.report(tr!(self, "msg-select-shape-for-symbol"));
                return;
            }
            Err(error) => {
                self.report(error.to_string());
                return;
            }
        };
        let what = match form.purpose {
            SymbolPurpose::New => "add the symbol",
            SymbolPurpose::Convert { .. } => "convert to symbol",
        };
        self.run_batch(commands, what);
        if self.status.is_some() {
            return;
        }
        // The next dialog starts on the choices made in this one, as Animate's does.
        self.library.new_kind = form.kind;
        self.library.registration = form.registration;
        self.library.selected = Some(id);
        if form.purpose == SymbolPurpose::New {
            // Animate opens a new symbol for drawing straight away.
            self.enter_symbol(id);
        }
    }

    /// The dialog, while it is open.
    pub(super) fn symbol_dialog_window(&mut self, ctx: &egui::Context) {
        let Some(mut form) = self.windows.symbol.take() else {
            return;
        };
        let convert = matches!(form.purpose, SymbolPurpose::Convert { .. });
        let title = if convert {
            tr!(self, "sym-convert-title")
        } else {
            tr!(self, "sym-new-title")
        };
        let (name_label, type_label, folder_label, root, registration_label, ok, cancel) = (
            tr!(self, "sym-name"),
            tr!(self, "sym-type"),
            tr!(self, "sym-folder"),
            tr!(self, "sym-library-root"),
            tr!(self, "sym-registration"),
            tr!(self, "sym-ok"),
            tr!(self, "start-cancel"),
        );
        let kinds: Vec<(&str, String)> = KINDS
            .iter()
            .map(|&kind| (kind_icon(kind), self.kind_label(kind)))
            .collect();
        let problem = self.symbol_name_problem(&form.name);
        let (mut confirm, mut cancelled) = (false, false);
        let modal = dialog::modal(ctx, egui::Id::unique("symbol_dialog")).show(ctx, |ui| {
            ui.set_width(DIALOG_WIDTH);
            dialog::title(ui, &title);
            let edit = dialog::field(ui, &name_label, |ui| {
                egui::TextEdit::singleline(&mut form.name)
                    .desired_width(ui.available_width())
                    .char_limit(library::MAX_SYMBOL_NAME_CHARS)
                    .show(ui)
            });
            if form.focus_name {
                form.focus_name = false;
                edit.response.request_focus();
                let all = egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(form.name.chars().count()),
                );
                let mut state = edit.state;
                state.cursor.set_char_range(Some(all));
                state.store(ui.ctx(), edit.response.id);
            }
            if let Some(problem) = &problem {
                ui.horizontal(|ui| {
                    ui.add_space(dialog::LABEL_WIDTH + ui.spacing().item_spacing.x);
                    ui.label(egui::RichText::new(problem).color(flare_ui::theme::danger()));
                });
            }
            ui.add_space(6.0);
            dialog::field(ui, &type_label, |ui| {
                let cards: Vec<(&str, &str)> = kinds
                    .iter()
                    .map(|(icon, name)| (*icon, name.as_str()))
                    .collect();
                let chosen = KINDS.iter().position(|k| *k == form.kind).unwrap_or(0);
                if let Some(index) = dialog::choice_cards(ui, &cards, chosen)
                    && let Some(kind) = KINDS.get(index)
                {
                    form.kind = *kind;
                }
            });
            ui.add_space(6.0);
            // Library folders are not built yet: the one place a symbol goes is shown, dimmed.
            dialog::field(ui, &folder_label, |ui| {
                ui.add_enabled(
                    false,
                    egui::Button::new(&root).min_size(egui::vec2(160.0, 24.0)),
                )
                .on_disabled_hover_text(chrome::NOT_YET);
            });
            if convert {
                ui.add_space(6.0);
                dialog::field(ui, &registration_label, |ui| {
                    let chosen = Registration::ALL
                        .iter()
                        .position(|r| *r == form.registration)
                        .unwrap_or(0);
                    if let Some(index) = dialog::point_grid(ui, chosen)
                        && let Some(point) = Registration::ALL.get(index)
                    {
                        form.registration = *point;
                    }
                });
            }
            dialog::footer(
                ui,
                |_| {},
                |ui| {
                    if dialog::primary_button(ui, &ok, problem.is_none()).clicked() {
                        confirm = true;
                    }
                    if dialog::button(ui, &cancel).clicked() {
                        cancelled = true;
                    }
                },
            );
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) && problem.is_none() {
                confirm = true;
            }
        });
        if modal.should_close() {
            cancelled = true;
        }
        if cancelled {
            return;
        }
        self.windows.symbol = Some(form);
        if confirm {
            self.confirm_symbol_dialog();
        }
    }

    /// The open dialog, for the tests.
    #[cfg(test)]
    pub(super) fn symbol_dialog(&mut self) -> Option<&mut SymbolDialog> {
        self.windows.symbol.as_mut()
    }
}
