//! Renaming a layer where its name is, and the menu on a right click in the layer column.

use egui::{Pos2, Rect, Ui};

use super::*;
use flare_menubar::MenuAction;

impl Workspace {
    /// Starts editing the name of `layer` in place.
    pub(super) fn begin_layer_rename(&mut self, layer: LayerId) {
        let Some(name) = self.layer(layer).map(|l| l.name().to_owned()) else {
            return;
        };
        self.select_layer(layer);
        // A box with a new id each time, so one that was left with Escape does not report that
        // it has lost focus the moment the next edit appears.
        self.layer_rename_serial = self.layer_rename_serial.wrapping_add(1);
        self.layer_renaming = Some((layer, name));
    }

    /// The text box over the name of the layer being renamed: Enter or leaving the box keeps the
    /// name, Escape drops it. `labels` is where the names start, row by row, from `first_row`.
    pub(super) fn layer_name_editor(
        &mut self,
        ui: &mut Ui,
        rows: &[(usize, usize)],
        first_row_top: f32,
        row: f32,
        left: f32,
        right: f32,
    ) {
        let Some((id, _)) = &self.layer_renaming else {
            return;
        };
        let id = *id;
        let Some(at) = self.document().scenes().first().and_then(|s| {
            let layers = s.timeline().layers();
            rows.iter()
                .position(|(i, _)| layers.get(*i).map(|l| l.id()) == Some(id))
        }) else {
            // The layer is gone (undo, another document) or folded away.
            self.layer_renaming = None;
            return;
        };
        let top = first_row_top + row * at as f32;
        let area = Rect::from_min_max(Pos2::new(left, top), Pos2::new(right, top + row));
        let Some((_, text)) = &mut self.layer_renaming else {
            return;
        };
        let edit = ui.put(
            area,
            egui::TextEdit::singleline(text)
                .id(ui.make_persistent_id(("layer_name", self.layer_rename_serial)))
                .desired_width(area.width()),
        );
        if !edit.has_focus() && !edit.lost_focus() {
            edit.request_focus();
        }
        let escaped = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if escaped {
            self.layer_renaming = None;
        } else if edit.lost_focus()
            && let Some((_, name)) = self.layer_renaming.take()
        {
            self.select_layer(id);
            self.rename_active_layer(&name);
        }
    }

    /// The layer drawn at height `y` in the timeline, if one is, given where the rows start.
    pub(super) fn layer_at(
        &self,
        rows: &[(usize, usize)],
        first_row_top: f32,
        row: f32,
        y: f32,
    ) -> Option<LayerId> {
        let at = usize::try_from(((y - first_row_top) / row).floor() as i64).ok()?;
        let layers = self.document().scenes().first()?.timeline().layers();
        rows.get(at)
            .and_then(|(i, _)| layers.get(*i))
            .map(|l| l.id())
    }

    /// The menu for the layer under a right click.
    pub(super) fn layer_menu(&mut self, ui: &mut Ui, layer: LayerId) {
        let (visible, locked) = self
            .layer(layer)
            .map_or((true, false), |l| (l.is_visible(), l.is_locked()));
        if ui.button(tr!(self, "msg-layer-rename")).clicked() {
            self.begin_layer_rename(layer);
            ui.close();
        }
        ui.separator();
        if ui
            .button(self.tr(if visible {
                "msg-layer-hide"
            } else {
                "msg-layer-show"
            }))
            .clicked()
        {
            self.toggle_layer_visible(layer);
            ui.close();
        }
        if ui
            .button(self.tr(if locked {
                "msg-layer-unlock"
            } else {
                "msg-layer-lock"
            }))
            .clicked()
        {
            self.toggle_layer_locked(layer);
            ui.close();
        }
        ui.separator();
        for (label, action) in [
            ("msg-layer-mask", MenuAction::ToggleLayerMask),
            ("msg-layer-guide", MenuAction::ToggleLayerGuide),
            ("msg-layer-move-up", MenuAction::MoveLayerUp),
            ("msg-layer-move-down", MenuAction::MoveLayerDown),
            ("msg-layer-new", MenuAction::NewLayer),
            ("msg-layer-new-folder", MenuAction::NewLayerFolder),
        ] {
            if ui
                .add_enabled(self.is_enabled(action), egui::Button::new(self.tr(label)))
                .clicked()
            {
                self.select_layer(layer);
                self.perform(action);
                ui.close();
            }
        }
        ui.separator();
        if ui
            .add_enabled(
                self.is_enabled(MenuAction::DeleteLayer),
                egui::Button::new(tr!(self, "msg-layer-delete")),
            )
            .clicked()
        {
            self.select_layer(layer);
            self.perform(MenuAction::DeleteLayer);
            ui.close();
        }
    }
}

/// Whether `x` is over the name part of the layer column (left of the eye, lock and outline
/// toggles) and not over the track.
pub(super) fn on_name(x: f32, track_left: f32) -> bool {
    x < track_left - 54.0
}
