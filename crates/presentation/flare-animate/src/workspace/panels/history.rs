//! The History and Output panels.

use crate::workspace::Workspace;
use egui::{RichText, Ui};
use flare_menubar::MenuAction;
use flare_ui::chrome;
use flare_ui::icons;
use flare_ui::theme;

use super::*;

impl Workspace {
    /// The English names the edits give their undo steps, with the message that says each in the
    /// language being spoken. The steps keep the English name so that one list serves every language.
    pub(crate) const HISTORY_LABELS: [(&str, &str); 52] = [
        ("Resize the stage", "hist-resize-stage"),
        ("Change the background", "hist-background"),
        ("Change the frame rate", "hist-frame-rate"),
        ("Cut frames", "hist-cut-frames"),
        ("Reverse frames", "hist-reverse-frames"),
        ("Import the sound", "hist-import-sound"),
        ("Rename the scene", "hist-rename-scene"),
        ("Paste", "hist-paste"),
        ("Delete", "hist-delete"),
        ("Change the tween", "hist-tween"),
        ("Change the tween's ease", "hist-ease"),
        ("Mirror copy", "hist-mirror"),
        ("Rename the layer", "hist-rename-layer"),
        ("Delete the layer", "hist-delete-layer"),
        ("Move the layer", "hist-move-layer"),
        ("Paste frames", "hist-paste-frames"),
        ("Add a folder", "hist-add-folder"),
        ("Move the layer into a folder", "hist-layer-into-folder"),
        ("Change how the graphic plays", "hist-graphic-plays"),
        ("Move", "hist-move"),
        ("Align", "hist-align"),
        ("Arrange", "hist-arrange"),
        ("Duplicate", "hist-duplicate"),
        ("Convert to symbol", "hist-convert-symbol"),
        ("Group", "hist-group"),
        ("Break apart", "hist-break-apart"),
        ("Paint", "hist-paint"),
        ("Transform", "hist-transform"),
        ("Edit symbol", "hist-edit-symbol"),
        ("Add the symbol", "hist-add-symbol"),
        ("Change the symbol's type", "hist-symbol-type"),
        ("Rename the symbol", "hist-rename-symbol"),
        ("Delete the symbol", "hist-delete-symbol"),
        ("Place instance", "hist-place-instance"),
        ("Delete the point", "hist-delete-point"),
        ("Make the point smooth", "hist-smooth-point"),
        ("Move the point", "hist-move-point"),
        ("Delete text", "hist-delete-text"),
        ("Edit text", "hist-edit-text"),
        ("Remove camera", "hist-remove-camera"),
        ("Change the shape's paint", "hist-shape-paint"),
        ("Add a layer", "hist-add-layer"),
        ("Edit", "hist-edit"),
        ("Draw rectangle", "hist-draw-rectangle"),
        ("Draw ellipse", "hist-draw-ellipse"),
        ("Draw line", "hist-draw-line"),
        ("Draw star", "hist-draw-star"),
        ("Draw polygon", "hist-draw-polygon"),
        ("Draw path", "hist-draw-path"),
        ("Draw brush stroke", "hist-draw-brush"),
        ("Draw pencil line", "hist-draw-pencil"),
        ("Original", "hist-original"),
    ];

    /// The name of the history step `label` (the `index`th of the list) in the language being
    /// spoken. A step this does not know, such as one with a name the person typed, is shown as it
    /// was kept.
    pub(crate) fn history_label(&self, index: usize, label: &str) -> String {
        if index == 0 {
            return tr!(self, "hist-original");
        }
        if let Some((_, key)) = Self::HISTORY_LABELS
            .iter()
            .find(|(english, _)| *english == label)
        {
            return self.tr(key);
        }
        // A shape drawn by a tool the table has no entry for: "Draw <what>".
        if let Some(what) = label.strip_prefix("Draw ") {
            return tr!(self, "hist-draw-other", &[("what", what)]);
        }
        label.to_owned()
    }

    /// Messages from earlier actions, newest at the bottom.
    pub(crate) fn output_panel(&mut self, ui: &mut Ui) {
        let clear = tr!(self, "out-clear");
        let count = tr!(self, "out-count", &[("n", &self.log.len().to_string())]);
        ui.horizontal(|ui| {
            if chrome::icon_button(ui, "delete", BAR_ICON, false, !self.log.is_empty(), &clear)
                .clicked()
            {
                self.log.clear();
            }
            ui.weak(count);
        });
        ui.separator();
        let nothing = tr!(self, "out-nothing");
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                if self.log.is_empty() {
                    chrome::empty_state(ui, "info", &nothing);
                }
                for line in &self.log {
                    ui.horizontal(|ui| {
                        ui.add(icons::image("warning", 13.0, theme::warning()));
                        ui.label(RichText::new(line).monospace());
                    });
                }
            });
    }

    // ---- history -----------------------------------------------------------------------

    pub(crate) fn history_panel(&mut self, ui: &mut Ui) {
        let (undo, redo, clear) = (
            tr!(self, "hist-undo"),
            tr!(self, "hist-redo"),
            tr!(self, "hist-clear"),
        );
        ui.horizontal(|ui| {
            if chrome::icon_button(
                ui,
                "undo",
                BAR_ICON,
                false,
                self.is_enabled(MenuAction::Undo),
                &undo,
            )
            .clicked()
            {
                self.perform(MenuAction::Undo);
            }
            if chrome::icon_button(
                ui,
                "redo",
                BAR_ICON,
                false,
                self.is_enabled(MenuAction::Redo),
                &redo,
            )
            .clicked()
            {
                self.perform(MenuAction::Redo);
            }
            let any = self.active().can_undo() || self.active().can_redo();
            if chrome::icon_button(ui, "delete", BAR_ICON, false, any, &clear).clicked() {
                self.clear_history();
            }
        });
        ui.separator();
        let (entries, current) = self.history_entries();
        // The steps are kept under the English names the edits gave them; the list shows them in
        // the language being spoken.
        let entries: Vec<String> = entries
            .iter()
            .enumerate()
            .map(|(index, label)| self.history_label(index, label))
            .collect();
        let mut jump = None;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for (index, label) in entries.iter().enumerate() {
                    // Steps after the current one are undone: shown dim, still reachable.
                    let text = if index > current {
                        RichText::new(label).color(theme::text_disabled())
                    } else {
                        RichText::new(label)
                    };
                    if ui.selectable_label(index == current, text).clicked() {
                        jump = Some(index);
                    }
                }
            });
        if let Some(index) = jump {
            self.jump_in_history(index);
        }
    }
}
