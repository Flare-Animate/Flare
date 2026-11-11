//! The tool strip and the options of the tool in hand.

use crate::workspace::{Tool, Workspace};
use egui::{Stroke, Ui};
use flare_menubar::MenuAction;
use flare_ui::{chrome, theme};

use super::tool_options_labels::{builtin_profile_key, symmetry_key};
use super::tool_slots::{SLOT_GROUPS, Slot, is_extra, name_key};
use super::*;

/// The strip is one tool wide unless it is made at least this wide: the default width, a few
/// percent of the window, stays one column at any usual window size.
const TWO_COLUMNS_FROM: f32 = 72.0;

impl Workspace {
    // ---- tools -------------------------------------------------------------------------

    pub(crate) fn tools_panel(&mut self, ui: &mut Ui) {
        // A bar of its own, as Animate's toolbar is: a shade lighter than the page around it,
        // with a thin dark line down its right edge.
        let edge = ui.max_rect();
        ui.painter().rect_filled(edge, 0.0, theme::panel_raised());
        ui.painter().vline(
            edge.right() - 0.5,
            edge.y_range(),
            Stroke::new(1.0, theme::stage_surround()),
        );
        // One column of every tool and the color box is taller than a short window: it scrolls.
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(4.0);
                ui.vertical_centered(|ui| {
                    let mut chosen = self.tool;
                    // Snug cells; two columns when the strip is wide enough for them, else one.
                    ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
                    let cell = TOOL_ICON + 6.0;
                    let columns = if ui.available_width() >= TWO_COLUMNS_FROM {
                        2
                    } else {
                        1
                    };
                    for (index, group) in SLOT_GROUPS.iter().enumerate() {
                        if index > 0 {
                            ui.add_space(3.0);
                            ui.separator();
                            ui.add_space(0.0);
                        }
                        // Tools put in the strip from the "…" list sit just above the dots.
                        let mut slots: Vec<Slot> = group.to_vec();
                        if let Some(dots) = slots.iter().position(|s| is_extra(s)) {
                            let pinned = self.pinned_slots();
                            slots.splice(dots..dots, pinned);
                        }
                        for pair in slots.chunks(columns) {
                            ui.horizontal(|ui| {
                                // Centred in the panel: a pair is two buttons wide.
                                let used = cell * pair.len() as f32
                                    + ui.spacing().item_spacing.x * (pair.len() - 1) as f32;
                                ui.add_space(((ui.available_width() - used) / 2.0).max(0.0));
                                for slot in pair {
                                    if let Some(tool) = self.slot_cell(ui, slot) {
                                        chosen = tool;
                                    }
                                }
                            });
                        }
                    }
                    self.set_tool(chosen);
                    self.tool_drag(ui, edge);
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(2.0);
                    self.color_wells(ui);
                });
            });
    }

    /// Options of the current tool, if it has any.
    pub(crate) fn tool_options(&mut self, ui: &mut Ui) {
        let icon = match self.tool {
            Tool::Select => return,
            Tool::Subselection => "tool-subselection",
            Tool::AddAnchor => "tool-anchor-add",
            Tool::DeleteAnchor => "tool-anchor-delete",
            Tool::ConvertAnchor => "tool-anchor-convert",
            Tool::FreeTransform => "tool-transform",
            Tool::Rectangle => "tool-rectangle",
            Tool::Ellipse => "tool-oval",
            Tool::Line => "tool-line",
            Tool::Pen => "tool-pen",
            Tool::Text => "tool-text",
            Tool::Hand => "tool-hand",
            Tool::Zoom => "tool-zoom",
            Tool::Eyedropper => "tool-eyedropper",
            Tool::Bucket => "tool-bucket",
            Tool::InkBottle => "tool-ink-bottle",
            Tool::Eraser => "tool-eraser",
            Tool::PolyStar => "tool-polystar",
            Tool::Lasso => "tool-lasso",
            Tool::PolygonLasso => "tool-lasso-polygon",
            Tool::Camera => "tool-camera",
            Tool::Bone => "tool-bone",
            Tool::Pencil => "tool-pencil",
            Tool::Brush => "tool-brush",
            Tool::Ruler => "tool-ruler",
            Tool::Mirror => "tool-mirror",
        };
        let title = tr!(self, "tools-title", &[("name", &self.tr(&name_key(icon)))]);
        chrome::section(ui, icon, &title);
        self.drawing_mode_mock(ui);
        match self.tool {
            Tool::Text => {
                self.character_section(ui, None);
                ui.weak(tr!(self, "tools-help-text"));
            }
            Tool::Pen => {
                ui.weak(tr!(self, "tools-help-pen"));
            }
            Tool::Rectangle => {
                chrome::row(ui, &tr!(self, "opt-corners"), |ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.rect_radius)
                            .range(0.0..=1000.0)
                            .suffix(" px"),
                    );
                });
                ui.weak(tr!(self, "tools-help-drag"));
            }
            Tool::Ellipse => {
                ui.weak(tr!(self, "tools-help-drag"));
            }
            Tool::PolyStar => {
                chrome::row(ui, &tr!(self, "opt-sides"), |ui| {
                    ui.add(egui::DragValue::new(&mut self.poly_sides).range(3..=64));
                });
                chrome::row(ui, &tr!(self, "opt-star"), |ui| {
                    ui.checkbox(&mut self.poly_star, "");
                });
                if self.poly_star {
                    chrome::row(ui, &tr!(self, "opt-depth"), |ui| {
                        ui.add(
                            egui::DragValue::new(&mut self.poly_inner)
                                .range(0.05..=0.95)
                                .speed(0.01),
                        );
                    });
                }
                ui.weak(tr!(self, "tools-help-polystar"));
            }
            Tool::Hand => {
                ui.weak(tr!(self, "tools-help-hand"));
            }
            Tool::Zoom => {
                ui.weak(tr!(self, "tools-help-zoom"));
            }
            Tool::Eyedropper => {
                ui.weak(tr!(self, "tools-help-eyedropper"));
            }
            Tool::Bucket => {
                ui.weak(tr!(self, "tools-help-bucket"));
            }
            Tool::InkBottle => {
                ui.weak(tr!(self, "tools-help-ink"));
            }
            Tool::Eraser => {
                ui.weak(tr!(self, "tools-help-eraser"));
            }
            Tool::Pencil => {
                chrome::row(ui, &tr!(self, "opt-smoothing"), |ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.smoothing)
                            .range(0.0..=20.0)
                            .speed(0.1),
                    );
                });
                self.stroke_assists(ui);
                ui.weak(tr!(self, "tools-help-pencil"));
            }
            Tool::Brush => {
                chrome::row(ui, &tr!(self, "opt-size"), |ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.brush_size)
                            .range(1.0..=200.0)
                            .suffix(" px"),
                    );
                });
                self.nib_controls(ui);
                chrome::row(ui, &tr!(self, "opt-smoothing"), |ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.smoothing)
                            .range(0.0..=20.0)
                            .speed(0.1),
                    );
                });
                chrome::row(ui, &tr!(self, "opt-light-touch"), |ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.brush_light)
                            .range(0.0..=1.0)
                            .speed(0.01),
                    );
                });
                self.brush_stroke_options(ui);
                self.stroke_assists(ui);
                ui.weak(tr!(self, "tools-help-brush"));
            }
            Tool::Ruler => {
                let reading = self.measurement().map_or_else(
                    || tr!(self, "opt-drag-on-stage"),
                    |m| {
                        tr!(
                            self,
                            "opt-ruler-reading",
                            &[
                                ("distance", &format!("{:.1}", m.distance)),
                                ("angle", &format!("{:.1}", m.angle)),
                                ("dx", &format!("{:.1}", m.dx)),
                                ("dy", &format!("{:.1}", m.dy)),
                            ],
                        )
                    },
                );
                chrome::row(ui, &tr!(self, "opt-reading"), |ui| {
                    ui.label(reading);
                });
                ui.weak(tr!(self, "tools-help-ruler"));
            }
            Tool::Mirror => {
                self.mirror_options(ui);
                ui.weak(tr!(self, "tools-help-mirror"));
            }
            Tool::Lasso => {
                ui.weak(tr!(self, "tools-help-lasso"));
            }
            Tool::Camera => self.camera_options(ui),
            Tool::Bone => self.bone_options(ui),
            Tool::PolygonLasso => {
                ui.weak(tr!(self, "tools-help-polygon"));
            }
            Tool::Line => {
                ui.weak(tr!(self, "tools-help-line"));
            }
            Tool::Select => {}
            Tool::Subselection => {
                ui.weak(tr!(self, "tools-help-subselection"));
            }
            Tool::AddAnchor => {
                ui.weak(tr!(self, "tools-help-add-anchor"));
            }
            Tool::DeleteAnchor => {
                ui.weak(tr!(self, "tools-help-delete-anchor"));
            }
            Tool::ConvertAnchor => {
                ui.weak(tr!(self, "tools-help-convert-anchor"));
            }
            Tool::FreeTransform => {
                ui.weak(tr!(self, "tools-help-free-transform"));
            }
        }
    }

    /// What the current tool does, shown when there is nothing else to say.
    pub(crate) fn tool_hint(&self) -> String {
        let key = match self.tool {
            Tool::Select => "tools-status-selection",
            Tool::Subselection => "tools-status-subselection",
            Tool::AddAnchor => "tools-status-add-anchor",
            Tool::DeleteAnchor => "tools-status-delete-anchor",
            Tool::ConvertAnchor => "tools-status-convert-anchor",
            Tool::FreeTransform => "tools-status-free-transform",
            Tool::Rectangle => "tools-status-rectangle",
            Tool::Ellipse => "tools-status-oval",
            Tool::Line => "tools-status-line",
            Tool::Pen => "tools-status-pen",
            Tool::Text => "tools-status-text",
            Tool::Hand => "tools-status-hand",
            Tool::Zoom => "tools-status-zoom",
            Tool::Eyedropper => "tools-status-eyedropper",
            Tool::Bucket => "tools-status-bucket",
            Tool::InkBottle => "tools-status-ink",
            Tool::Eraser => "tools-status-eraser",
            Tool::PolyStar => "tools-status-polystar",
            Tool::Lasso => "tools-status-lasso",
            Tool::PolygonLasso => "tools-status-polygon",
            Tool::Camera => "tools-status-camera",
            Tool::Bone => "tools-status-bone",
            Tool::Pencil => "tools-status-pencil",
            Tool::Brush => "tools-status-brush",
            Tool::Ruler => "tools-status-ruler",
            Tool::Mirror => "tools-status-mirror",
        };
        self.tr(key)
    }

    /// The brush's nib: its shape, the angle of its long axis and how flat it is.
    fn nib_controls(&mut self, ui: &mut Ui) {
        use flare_tools::brush::{BuiltinBrushProfile, Nib, NibShape};
        let (new, delete) = (tr!(self, "opt-new"), tr!(self, "opt-delete"));
        let (new_tip, delete_tip) = (
            tr!(self, "opt-new-brush-tip"),
            tr!(self, "opt-delete-brush-tip"),
        );
        let (round, square) = (tr!(self, "opt-nib-round"), tr!(self, "opt-nib-square"));
        chrome::row(ui, &tr!(self, "opt-brush"), |ui| {
            let chosen = self.brush_shelf.selected();
            let mut custom_pick = None;
            let mut builtin_pick = None;
            let builtin_name = |profile| self.tr(builtin_profile_key(profile));
            let current_name = chosen.map_or_else(
                || {
                    self.brush_shelf
                        .builtin()
                        .map_or_else(|| tr!(self, "opt-profile-custom"), builtin_name)
                },
                |i| self.brush_shelf.presets()[i].name.clone(),
            );
            egui::ComboBox::from_id_salt("brush-list")
                .selected_text(current_name)
                .show_ui(ui, |ui| {
                    for profile in BuiltinBrushProfile::ALL {
                        if ui
                            .selectable_label(
                                chosen.is_none() && self.brush_shelf.builtin() == Some(profile),
                                builtin_name(profile),
                            )
                            .clicked()
                        {
                            builtin_pick = Some(profile);
                        }
                    }
                    for (i, brush) in self.brush_shelf.presets().iter().enumerate() {
                        if ui
                            .selectable_label(chosen == Some(i), &brush.name)
                            .clicked()
                        {
                            custom_pick = Some(i);
                        }
                    }
                });
            if let Some(profile) = builtin_pick {
                self.brush_shelf.choose_builtin(profile);
            } else if let Some(index) = custom_pick {
                self.brush_shelf.choose(Some(index));
            }
            if builtin_pick.is_some() || custom_pick.is_some() {
                self.brush_light = self.brush_shelf.pressure_min();
                self.brush_pressure_gamma = self.brush_shelf.pressure_gamma();
            }
            if ui.button(&new).on_hover_text(&new_tip).clicked() {
                self.new_custom_brush();
            }
            if ui
                .add_enabled(chosen.is_some(), egui::Button::new(&delete))
                .on_hover_text(&delete_tip)
                .clicked()
            {
                self.brush_shelf.delete_selected();
            }
        });
        // Choosing a brush above may have changed the nib: read the controls from it.
        let nib = self.brush_shelf.nib();
        let (mut shape, mut angle, mut flatness) = (
            nib.shape(),
            nib.angle_degrees() as f32,
            nib.flatness_percent() as f32,
        );
        chrome::row(ui, &tr!(self, "opt-nib"), |ui| {
            egui::ComboBox::from_id_salt("brush-nib-shape")
                .selected_text(match shape {
                    NibShape::Round => round.as_str(),
                    NibShape::Square => square.as_str(),
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut shape, NibShape::Round, &round);
                    ui.selectable_value(&mut shape, NibShape::Square, &square);
                });
        });
        chrome::row(ui, &tr!(self, "opt-angle"), |ui| {
            ui.add(
                egui::DragValue::new(&mut angle)
                    .range(0.0..=359.0)
                    .suffix("\u{b0}"),
            );
        });
        chrome::row(ui, &tr!(self, "opt-flatness"), |ui| {
            ui.add(
                egui::DragValue::new(&mut flatness)
                    .range(1.0..=100.0)
                    .suffix(" %"),
            );
        });
        let edited = Nib::new(shape, f64::from(angle), f64::from(flatness));
        if edited != nib {
            self.brush_shelf.set_nib(edited);
        }
        chrome::row(ui, &tr!(self, "opt-pressure-response"), |ui| {
            ui.add(
                egui::DragValue::new(&mut self.brush_pressure_gamma)
                    .range(0.1..=4.0)
                    .speed(0.05),
            );
        });
        self.brush_shelf
            .set_pressure_curve(self.brush_light, self.brush_pressure_gamma);
    }

    /// The straight edge and symmetry options the Pencil and Brush share.
    fn stroke_assists(&mut self, ui: &mut Ui) {
        let always = tr!(self, "opt-straight-always");
        chrome::row(ui, &tr!(self, "opt-straight"), |ui| {
            ui.checkbox(&mut self.paint.straight_edge, always);
        });
        chrome::row(ui, &tr!(self, "opt-symmetry"), |ui| {
            self.symmetry_picker(ui);
        });
    }

    fn symmetry_picker(&mut self, ui: &mut Ui) {
        let names = flare_tools::mirror::Symmetry::ALL.map(|mode| self.tr(symmetry_key(mode)));
        let shown = self.tr(symmetry_key(self.paint.symmetry));
        egui::ComboBox::from_id_salt("symmetry")
            .selected_text(shown)
            .show_ui(ui, |ui| {
                for (mode, name) in flare_tools::mirror::Symmetry::ALL.into_iter().zip(&names) {
                    ui.selectable_value(&mut self.paint.symmetry, mode, name);
                }
            });
    }

    /// The Mirror tool's options: the symmetry mode, the axes, and mirror copies.
    fn mirror_options(&mut self, ui: &mut Ui) {
        chrome::row(ui, &tr!(self, "opt-symmetry"), |ui| {
            self.symmetry_picker(ui);
        });
        let extent = self.stage_extent();
        let mut center = self
            .paint
            .symmetry_center
            .unwrap_or_else(|| egui::pos2(extent.x / 2.0, extent.y / 2.0));
        let before = center;
        chrome::row(ui, &tr!(self, "opt-axes-at"), |ui| {
            ui.add(egui::DragValue::new(&mut center.x).prefix("x ").speed(1.0));
            ui.add(egui::DragValue::new(&mut center.y).prefix("y ").speed(1.0));
        });
        if center != before && center.x.is_finite() && center.y.is_finite() {
            // Far beyond any stage, and kept small enough for the geometry to stay exact.
            center = center.clamp(egui::pos2(-1.0e6, -1.0e6), egui::pos2(1.0e6, 1.0e6));
            self.paint.symmetry_center = Some(center);
        }
        if ui.small_button(tr!(self, "opt-center-on-stage")).clicked() {
            self.paint.symmetry_center = None;
        }
        let selected = self.has_selection();
        let (copy_lr, copy_tb) = (
            tr!(self, "opt-copy-left-right"),
            tr!(self, "opt-copy-top-bottom"),
        );
        let (flip_h, flip_v) = (
            tr!(self, "opt-flip-horizontal"),
            tr!(self, "opt-flip-vertical"),
        );
        ui.add_enabled_ui(selected, |ui| {
            ui.horizontal(|ui| {
                if ui.button(&copy_lr).clicked() {
                    self.mirror_copy_selection(true);
                }
                if ui.button(&copy_tb).clicked() {
                    self.mirror_copy_selection(false);
                }
            });
            ui.horizontal(|ui| {
                if ui.button(&flip_h).clicked() {
                    self.perform(MenuAction::FlipHorizontal);
                }
                if ui.button(&flip_v).clicked() {
                    self.perform(MenuAction::FlipVertical);
                }
            });
        });
    }
}
