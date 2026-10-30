//! The Properties panel and its tabs.

use crate::actions::MenuAction;
use crate::workspace::PropsTab;
use crate::workspace::Workspace;
use egui::{RichText, Ui};
use flare_ui::chrome;
use flare_ui::icons;
use flare_ui::theme;

impl Workspace {
    // ---- properties --------------------------------------------------------------------

    pub(crate) fn properties_panel(&mut self, ui: &mut Ui) {
        const TABS: [(PropsTab, &str); 4] = [
            (PropsTab::Tool, "Tool"),
            (PropsTab::Object, "Object"),
            (PropsTab::Frame, "Frame"),
            (PropsTab::Doc, "Doc"),
        ];
        let labels = TABS.map(|(_, label)| label);
        let current = TABS.iter().position(|(tab, _)| *tab == self.props_tab);
        if let Some(index) = chrome::segmented(ui, &labels, current.unwrap_or(0)) {
            self.props_tab = TABS[index].0;
        }
        ui.add_space(4.0);
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| match self.props_tab {
                PropsTab::Tool => self.tool_tab(ui),
                PropsTab::Object => self.object_tab(ui),
                PropsTab::Frame => self.frame_tab(ui),
                PropsTab::Doc => self.doc_tab(ui),
            });
    }

    pub(crate) fn tool_tab(&mut self, ui: &mut Ui) {
        self.tool_options(ui);
        chrome::section(ui, "fill-color", "Fill and stroke");
        chrome::row(ui, "Fill", |ui| {
            ui.color_edit_button_srgba(&mut self.paint.fill);
            ui.checkbox(&mut self.paint.gradient_enabled, "Gradient");
        });
        if self.paint.gradient_enabled {
            chrome::row(ui, "To", |ui| {
                ui.color_edit_button_srgba(&mut self.paint.gradient_end);
                ui.checkbox(&mut self.paint.gradient_vertical, "Vertical");
            });
        }
        chrome::row(ui, "Stroke", |ui| {
            ui.color_edit_button_srgba(&mut self.paint.stroke_color);
            ui.checkbox(&mut self.paint.stroke_enabled, "Outline");
        });
        chrome::row(ui, "Width", |ui| {
            ui.add_enabled(
                self.paint.stroke_enabled,
                egui::DragValue::new(&mut self.paint.stroke_width)
                    .range(0.5..=200.0)
                    .suffix(" px"),
            );
        });
        chrome::row(ui, "Cap", |ui| {
            use flare_document::Cap;
            egui::ComboBox::from_id_salt("cap_menu")
                .width(80.0)
                .selected_text(match self.paint.stroke_cap {
                    Cap::Butt => "None",
                    Cap::Round => "Round",
                    Cap::Square => "Square",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.paint.stroke_cap, Cap::Butt, "None");
                    ui.selectable_value(&mut self.paint.stroke_cap, Cap::Round, "Round");
                    ui.selectable_value(&mut self.paint.stroke_cap, Cap::Square, "Square");
                });
        });
        chrome::row(ui, "Join", |ui| {
            use flare_document::Join;
            egui::ComboBox::from_id_salt("join_menu")
                .width(80.0)
                .selected_text(match self.paint.stroke_join {
                    Join::Round => "Round",
                    Join::Bevel => "Bevel",
                    Join::Miter => "Miter",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.paint.stroke_join, Join::Round, "Round");
                    ui.selectable_value(&mut self.paint.stroke_join, Join::Bevel, "Bevel");
                    ui.selectable_value(&mut self.paint.stroke_join, Join::Miter, "Miter");
                });
        });
    }

    /// Position and size of the selection, editable.
    pub(crate) fn object_tab(&mut self, ui: &mut Ui) {
        let Some(bounds) = self.selection_bounds() else {
            chrome::empty_state(ui, "tool-selection", "Nothing selected");
            ui.vertical_centered(|ui| ui.weak("Select something on the stage."));
            return;
        };
        let count = self.selection_all().len();
        ui.horizontal(|ui| {
            ui.add(icons::image("tool-selection", 18.0, theme::ACCENT));
            ui.label(
                RichText::new(if count == 1 {
                    "Selection".to_owned()
                } else {
                    format!("Selection ({count})")
                })
                .heading(),
            );
        });
        chrome::section(ui, "stage-size", "Position and size");
        let mut v = self.object_edit.unwrap_or([
            bounds.min.x,
            bounds.min.y,
            bounds.width(),
            bounds.height(),
        ]);
        let mut changed = false;
        let mut done = false;
        for (label, a, b) in [("Position", 0, 1), ("Size", 2, 3)] {
            chrome::row(ui, label, |ui| {
                for i in [a, b] {
                    let r = ui.add(egui::DragValue::new(&mut v[i]).speed(0.5).max_decimals(1));
                    changed |= r.changed();
                    done |= r.drag_stopped() || r.lost_focus();
                }
            });
        }
        if changed {
            self.object_edit = Some(v);
        }
        // Applied on release or Enter, so scrubbing a value is a single undo step.
        if done && let Some(v) = self.object_edit.take() {
            self.set_selection_bounds(egui::Rect::from_min_size(
                egui::pos2(v[0], v[1]),
                egui::vec2(v[2], v[3]),
            ));
        }
    }

    /// The current frame and layer.
    pub(crate) fn frame_tab(&mut self, ui: &mut Ui) {
        chrome::section(ui, "playhead", "Frame");
        chrome::row(ui, "Frame", |ui| {
            ui.label(format!("{} / {}", self.frame + 1, self.scene_frames()));
        });
        let info = self.active_layer().map(|layer| {
            let span = layer
                .span_at(self.frame)
                .map(|s| (s.start(), s.duration().get()));
            (layer.name().to_owned(), span)
        });
        match info {
            Some((name, span)) => {
                chrome::row(ui, "Layer", |ui| {
                    ui.label(name);
                });
                chrome::row(ui, "Exposure", |ui| match span {
                    Some((start, len)) => {
                        ui.label(format!("from frame {} for {len}", start + 1));
                    }
                    None => {
                        ui.weak("empty");
                    }
                });
            }
            None => {
                ui.weak("No layer selected.");
            }
        }
        chrome::section(ui, "frame-tween", "Tween");
        let can_tween = self.is_enabled(MenuAction::ToggleMotionTween);
        if ui
            .add_enabled(can_tween, egui::Button::new("Toggle motion tween"))
            .clicked()
        {
            self.perform(MenuAction::ToggleMotionTween);
        }
    }

    pub(crate) fn doc_tab(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.add(icons::image("document", 18.0, theme::ACCENT));
            ui.label(RichText::new("Document").heading());
        });
        self.publish_section(ui);
        self.document_section(ui);
    }

    /// The SWF publish choices, with the button for the rest.
    fn publish_section(&mut self, ui: &mut Ui) {
        chrome::section(ui, "file-publish", "Publish Settings");
        self.publish_settings_ui(ui);
    }

    /// Stage size, background and the facts about the document.
    pub(crate) fn document_section(&mut self, ui: &mut Ui) {
        chrome::section(ui, "document", "Document");
        self.stage_settings(ui);
        let rate = self.document().frame_rate();
        chrome::row(ui, "Frame rate", |ui| {
            let current = f64::from(rate.numerator()) / f64::from(rate.denominator());
            let mut fps = self.fps_edit.unwrap_or(current);
            let response = ui.add_enabled(
                !self.in_symbol(),
                egui::DragValue::new(&mut fps)
                    .range(0.01..=240.0)
                    .speed(0.1)
                    .suffix(" fps"),
            );
            if response.changed() {
                self.fps_edit = Some(fps);
            }
            // Applied on release or Enter, so scrubbing the value is one undo step.
            if (response.drag_stopped() || response.lost_focus())
                && let Some(fps) = self.fps_edit.take()
            {
                self.set_frame_rate(fps);
            }
        });
        chrome::row(ui, "Scenes", |ui| {
            ui.label(self.document().scenes().len().to_string());
        });
        chrome::row(ui, "Sounds", |ui| {
            ui.label(self.document().sounds().len().to_string());
        });
    }
}
