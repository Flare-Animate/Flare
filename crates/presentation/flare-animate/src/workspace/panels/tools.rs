//! The tool strip and the options of the tool in hand.

use crate::MenuAction;
use crate::workspace::{Tool, Workspace};
use egui::{Stroke, Ui};
use flare_ui::{chrome, theme};

use super::tool_slots::{SLOT_GROUPS, Slot, is_extra};
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
        let (icon, title) = match self.tool {
            Tool::Select => return,
            Tool::Subselection => ("tool-subselection", "Subselection"),
            Tool::FreeTransform => ("tool-transform", "Free Transform"),
            Tool::Rectangle => ("tool-rectangle", "Rectangle"),
            Tool::Ellipse => ("tool-oval", "Oval"),
            Tool::Line => ("tool-line", "Line"),
            Tool::Pen => ("tool-pen", "Pen"),
            Tool::Text => ("tool-text", "Text"),
            Tool::Hand => ("tool-hand", "Hand"),
            Tool::Zoom => ("tool-zoom", "Zoom"),
            Tool::Eyedropper => ("tool-eyedropper", "Eyedropper"),
            Tool::Bucket => ("tool-bucket", "Paint Bucket"),
            Tool::InkBottle => ("tool-ink-bottle", "Ink Bottle"),
            Tool::Eraser => ("tool-eraser", "Eraser"),
            Tool::PolyStar => ("tool-polystar", "PolyStar"),
            Tool::Lasso => ("tool-lasso", "Lasso"),
            Tool::PolygonLasso => ("tool-lasso-polygon", "Polygon"),
            Tool::Pencil => ("tool-pencil", "Pencil"),
            Tool::Brush => ("tool-brush", "Brush"),
            Tool::Ruler => ("tool-ruler", "Ruler"),
            Tool::Mirror => ("tool-mirror", "Mirror"),
        };
        chrome::section(ui, icon, &format!("{title} tool"));
        match self.tool {
            Tool::Text => {
                chrome::row(ui, "Family", |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.text.family)
                            .desired_width(150.0)
                            .hint_text("Helvetica, sans-serif"),
                    );
                });
                chrome::row(ui, "Size", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.text.size)
                            .range(flare_text::MIN_SIZE..=flare_text::MAX_SIZE)
                            .suffix(" px"),
                    );
                });
                chrome::row(ui, "Style", |ui| {
                    ui.toggle_value(&mut self.text.bold, "Bold");
                    ui.toggle_value(&mut self.text.italic, "Italic");
                });
                ui.weak("Click the stage, type, then Place (Ctrl+Enter).");
            }
            Tool::Pen => {
                ui.weak("Click for corners, drag for curves. Click the first point to close; Enter or double-click to finish.");
            }
            Tool::Rectangle => {
                chrome::row(ui, "Corners", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.rect_radius)
                            .range(0.0..=1000.0)
                            .suffix(" px"),
                    );
                });
                ui.weak("Drag on the stage.");
            }
            Tool::Ellipse => {
                ui.weak("Drag on the stage.");
            }
            Tool::PolyStar => {
                chrome::row(ui, "Sides", |ui| {
                    ui.add(egui::DragValue::new(&mut self.poly_sides).range(3..=64));
                });
                chrome::row(ui, "Star", |ui| {
                    ui.checkbox(&mut self.poly_star, "");
                });
                if self.poly_star {
                    chrome::row(ui, "Depth", |ui| {
                        ui.add(
                            egui::DragValue::new(&mut self.poly_inner)
                                .range(0.05..=0.95)
                                .speed(0.01),
                        );
                    });
                }
                ui.weak("Drag from the center; the drag sets size and rotation.");
            }
            Tool::Hand => {
                ui.weak("Drag the stage to pan. Scrolling and middle-drag pan with any tool.");
            }
            Tool::Zoom => {
                ui.weak("Click to zoom in, Alt-click to zoom out.");
            }
            Tool::Eyedropper => {
                ui.weak("Click a shape to take its fill; Shift-click takes its stroke.");
            }
            Tool::Bucket => {
                ui.weak("Click a shape to fill it with the current fill color.");
            }
            Tool::InkBottle => {
                ui.weak("Click a shape to outline it with the current stroke.");
            }
            Tool::Eraser => {
                ui.weak("Click a shape to delete it.");
            }
            Tool::Pencil => {
                chrome::row(ui, "Smoothing", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.smoothing)
                            .range(0.0..=20.0)
                            .speed(0.1),
                    );
                });
                self.stroke_assists(ui);
                ui.weak("Drag to draw. The line uses the stroke color and width.");
            }
            Tool::Brush => {
                chrome::row(ui, "Size", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.brush_size)
                            .range(1.0..=200.0)
                            .suffix(" px"),
                    );
                });
                self.nib_controls(ui);
                chrome::row(ui, "Smoothing", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.smoothing)
                            .range(0.0..=20.0)
                            .speed(0.1),
                    );
                });
                chrome::row(ui, "Light touch", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.brush_light)
                            .range(0.0..=1.0)
                            .speed(0.01),
                    );
                });
                self.stroke_assists(ui);
                ui.weak("Drag to paint with the fill color; a pen presses wider.");
            }
            Tool::Ruler => {
                let reading = self
                    .measurement()
                    .map_or_else(|| "Drag on the stage".to_owned(), |m| m.label());
                chrome::row(ui, "Reading", |ui| {
                    ui.label(reading);
                });
                ui.weak("Drag from one point to another. Points snap to guides, and to the grid when snapping is on.");
            }
            Tool::Mirror => {
                self.mirror_options(ui);
                ui.weak(
                    "Click the stage to put the axes there. Pencil and Brush mirror across them.",
                );
            }
            Tool::Lasso => {
                ui.weak("Drag a loop around shapes to select them; Shift adds to the selection.");
            }
            Tool::PolygonLasso => {
                ui.weak("Click the corners of a loop around shapes; double-click, Enter or the first corner closes it. Shift adds to the selection.");
            }
            Tool::Line => {
                ui.weak("Drag on the stage. A line always has a stroke.");
            }
            Tool::Select => {}
            Tool::Subselection => {
                ui.weak("Click a shape, then drag its square points to reshape it.");
            }
            Tool::FreeTransform => {
                ui.weak("Drag a handle to scale, outside a corner to rotate; Alt on an edge handle skews, Shift keeps proportions.");
            }
        }
    }

    /// What the current tool does, shown when there is nothing else to say.
    pub(crate) fn tool_hint(&self) -> &'static str {
        match self.tool {
            Tool::Select => "Selection: click to select, drag to move, Shift-click to add",
            Tool::Subselection => "Subselection: click a shape, drag its points",
            Tool::FreeTransform => "Free Transform: drag the handles to scale, rotate or skew",
            Tool::Rectangle => "Rectangle: drag on the stage",
            Tool::Ellipse => "Oval: drag on the stage",
            Tool::Line => "Line: drag on the stage",
            Tool::Pen => "Pen: click for corners, drag for curves, Enter to finish",
            Tool::Text => "Text: click the stage, type, then Place",
            Tool::Hand => "Hand: drag to pan the stage",
            Tool::Zoom => "Zoom: click to zoom in, Alt-click to zoom out",
            Tool::Eyedropper => "Eyedropper: click a shape to take its color, Shift for the stroke",
            Tool::Bucket => "Paint Bucket: click a shape to fill it",
            Tool::InkBottle => "Ink Bottle: click a shape to outline it",
            Tool::Eraser => "Eraser: click a shape to delete it",
            Tool::PolyStar => "PolyStar: drag from the center",
            Tool::Lasso => "Lasso: drag a loop around shapes to select them",
            Tool::PolygonLasso => "Polygon: click corners, double-click to close and select",
            Tool::Pencil => "Pencil: drag to draw",
            Tool::Brush => "Brush: drag to paint",
            Tool::Ruler => "Ruler: drag to measure; Pencil and Brush draw straight with Shift",
            Tool::Mirror => "Mirror: click to set the axes that Pencil and Brush mirror across",
        }
    }

    /// The brush's nib: its shape, the angle of its long axis and how flat it is.
    fn nib_controls(&mut self, ui: &mut Ui) {
        use flare_tools::brush::{Nib, NibShape};
        chrome::row(ui, "Brush", |ui| {
            let chosen = self.selected_brush();
            let mut pick = None;
            egui::ComboBox::from_id_salt("brush-list")
                .selected_text(chosen.map_or("Plain", |i| self.custom_brushes()[i].name.as_str()))
                .show_ui(ui, |ui| {
                    if ui.selectable_label(chosen.is_none(), "Plain").clicked() {
                        pick = Some(None);
                    }
                    for (i, brush) in self.custom_brushes().iter().enumerate() {
                        if ui
                            .selectable_label(chosen == Some(i), &brush.name)
                            .clicked()
                        {
                            pick = Some(Some(i));
                        }
                    }
                });
            if let Some(index) = pick {
                self.choose_brush(index);
            }
            if ui
                .button("New")
                .on_hover_text("Keep this nib as a custom brush")
                .clicked()
            {
                self.new_custom_brush();
            }
            if ui
                .add_enabled(chosen.is_some(), egui::Button::new("Delete"))
                .on_hover_text("Delete the chosen custom brush")
                .clicked()
            {
                self.delete_custom_brush();
            }
        });
        // Choosing a brush above may have changed the nib: read the controls from it.
        let nib = self.brush_nib;
        let (mut shape, mut angle, mut flatness) = (
            nib.shape(),
            nib.angle_degrees() as f32,
            nib.flatness_percent() as f32,
        );
        chrome::row(ui, "Nib", |ui| {
            egui::ComboBox::from_id_salt("brush-nib-shape")
                .selected_text(match shape {
                    NibShape::Round => "Round",
                    NibShape::Square => "Square",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut shape, NibShape::Round, "Round");
                    ui.selectable_value(&mut shape, NibShape::Square, "Square");
                });
        });
        chrome::row(ui, "Angle", |ui| {
            ui.add(
                egui::DragValue::new(&mut angle)
                    .range(0.0..=359.0)
                    .suffix("\u{b0}"),
            );
        });
        chrome::row(ui, "Flatness", |ui| {
            ui.add(
                egui::DragValue::new(&mut flatness)
                    .range(1.0..=100.0)
                    .suffix(" %"),
            );
        });
        let edited = Nib::new(shape, f64::from(angle), f64::from(flatness));
        if edited != nib {
            self.set_brush_nib(edited);
        }
    }

    /// The straight edge and symmetry options the Pencil and Brush share.
    fn stroke_assists(&mut self, ui: &mut Ui) {
        chrome::row(ui, "Straight", |ui| {
            ui.checkbox(&mut self.paint.straight_edge, "Always (or hold Shift)");
        });
        chrome::row(ui, "Symmetry", |ui| {
            self.symmetry_picker(ui);
        });
    }

    fn symmetry_picker(&mut self, ui: &mut Ui) {
        egui::ComboBox::from_id_salt("symmetry")
            .selected_text(self.paint.symmetry.label())
            .show_ui(ui, |ui| {
                for mode in flare_tools::symmetry::Symmetry::ALL {
                    ui.selectable_value(&mut self.paint.symmetry, mode, mode.label());
                }
            });
    }

    /// The Mirror tool's options: the symmetry mode, the axes, and mirror copies.
    fn mirror_options(&mut self, ui: &mut Ui) {
        chrome::row(ui, "Symmetry", |ui| {
            self.symmetry_picker(ui);
        });
        let extent = self.stage_extent();
        let mut center = self
            .paint
            .symmetry_center
            .unwrap_or_else(|| egui::pos2(extent.x / 2.0, extent.y / 2.0));
        let before = center;
        chrome::row(ui, "Axes at", |ui| {
            ui.add(egui::DragValue::new(&mut center.x).prefix("x ").speed(1.0));
            ui.add(egui::DragValue::new(&mut center.y).prefix("y ").speed(1.0));
        });
        if center != before && center.x.is_finite() && center.y.is_finite() {
            // Far beyond any stage, and kept small enough for the geometry to stay exact.
            center = center.clamp(egui::pos2(-1.0e6, -1.0e6), egui::pos2(1.0e6, 1.0e6));
            self.paint.symmetry_center = Some(center);
        }
        if ui.small_button("Center on stage").clicked() {
            self.paint.symmetry_center = None;
        }
        let selected = self.has_selection();
        ui.add_enabled_ui(selected, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Copy left-right").clicked() {
                    self.mirror_copy_selection(true);
                }
                if ui.button("Copy top-bottom").clicked() {
                    self.mirror_copy_selection(false);
                }
            });
            ui.horizontal(|ui| {
                if ui.button("Flip horizontal").clicked() {
                    self.perform(MenuAction::FlipHorizontal);
                }
                if ui.button("Flip vertical").clicked() {
                    self.perform(MenuAction::FlipVertical);
                }
            });
        });
    }
}
