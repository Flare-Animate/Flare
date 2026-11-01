//! The Properties panel when something is selected on the stage: what it is, where it is, and how
//! it is painted. A shape's fill and stroke are edited here, on the shape; an instance shows its
//! symbol and how it plays.

use crate::workspace::{ShapePaintEdit, Workspace};
use egui::{Color32, RichText, Ui};
use flare_core::Rgba8;
use flare_document::{Cap, Content, Join, Shape, Stroke as ShapeStroke, SymbolKind};
use flare_tools::ops::{self, SpanRef};
use flare_ui::{chrome, icons, theme};

/// What the selection is, when it is one thing.
enum Picked {
    Shape(SpanRef, Shape),
    Instance { name: String, kind: SymbolKind },
    Other,
}

fn rgba(color: Color32) -> Rgba8 {
    let c = color.to_array();
    Rgba8::new(c[0], c[1], c[2], c[3])
}

fn color32(color: Rgba8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}

impl Workspace {
    /// The panel for a selection on the stage.
    pub(crate) fn object_tab(&mut self, ui: &mut Ui) {
        let Some(bounds) = self.selection_bounds() else {
            chrome::empty_state(ui, "tool-selection", "Nothing selected");
            ui.vertical_centered(|ui| ui.weak("Select something on the stage."));
            return;
        };
        let count = self.selection_all().len();
        let picked = self.picked_object(count);
        let title = match &picked {
            Picked::Shape(_, shape) if shape.text().is_some() => "Text".to_owned(),
            Picked::Shape(..) => "Shape".to_owned(),
            Picked::Instance { .. } => "Symbol instance".to_owned(),
            Picked::Other if count == 1 => "Selection".to_owned(),
            Picked::Other => format!("Selection ({count})"),
        };
        ui.horizontal(|ui| {
            ui.add(icons::image("tool-selection", 18.0, theme::accent()));
            ui.label(RichText::new(title).heading());
        });
        if let Picked::Instance { name, kind } = &picked {
            self.instance_section(ui, name, *kind);
        }
        self.position_size_section(ui, bounds);
        match &picked {
            Picked::Shape(at, shape) => {
                if let Some(source) = shape.text() {
                    self.character_section(ui, Some(source));
                }
                self.shape_paint_section(ui, *at, shape);
            }
            Picked::Instance { .. } => {
                self.looping_section(ui);
                for (icon, title, label) in [
                    ("fill-color", "Color Effect", "Style"),
                    ("eye", "Display", "Blend"),
                    ("gradient", "Filters", "Filters"),
                ] {
                    chrome::section(ui, icon, title);
                    chrome::row(ui, label, |ui| {
                        ui.add_enabled(false, egui::Button::new("None"))
                            .on_disabled_hover_text(chrome::NOT_YET);
                    });
                }
            }
            Picked::Other => {}
        }
    }

    fn picked_object(&self, count: usize) -> Picked {
        if count != 1 {
            return Picked::Other;
        }
        let (Some(at), Some(scene)) = (self.selected, self.scene_id()) else {
            return Picked::Other;
        };
        match ops::find_span(self.document(), scene, at).map(|s| s.content()) {
            Some(Content::Shape(shape)) => Picked::Shape(at, shape.clone()),
            Some(Content::Instance(instance)) => {
                self.document()
                    .symbol(instance.symbol())
                    .map_or(Picked::Other, |symbol| Picked::Instance {
                        name: symbol.name().to_owned(),
                        kind: symbol.kind(),
                    })
            }
            _ => Picked::Other,
        }
    }

    /// Which symbol an instance is of, and the way to edit it.
    fn instance_section(&mut self, ui: &mut Ui, name: &str, kind: SymbolKind) {
        chrome::section(ui, "symbol-movie-clip", "Instance");
        chrome::row(ui, "Symbol", |ui| {
            ui.label(name);
        });
        chrome::row(ui, "Type", |ui| {
            ui.label(kind.label());
        });
        if ui.button("Edit Symbol").clicked() {
            self.perform(flare_menubar::MenuAction::EditSymbol);
        }
    }

    /// Position and size of the selection, editable.
    fn position_size_section(&mut self, ui: &mut Ui, bounds: egui::Rect) {
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

    /// The fill and stroke of the selected shape, changed on the shape itself. A color is applied
    /// when the picker is let go, a number when it is, so each change is one undo step.
    fn shape_paint_section(&mut self, ui: &mut Ui, at: SpanRef, shape: &Shape) {
        if self.shape_paint_edit.is_some_and(|e| e.at != at) {
            self.shape_paint_edit = None;
        }
        let stroke = shape.stroke();
        let mut e = self.shape_paint_edit.unwrap_or(ShapePaintEdit {
            at,
            fill: color32(shape.fill()),
            fill_changed: false,
            outline: stroke.is_some(),
            color: stroke.map_or(self.paint.stroke_color, |s| color32(s.color())),
            width: stroke.map_or(self.paint.stroke_width, |s| s.width() as f32),
            cap: stroke.map_or(self.paint.stroke_cap, |s| s.cap()),
            join: stroke.map_or(self.paint.stroke_join, |s| s.join()),
            stroke_changed: false,
        });
        chrome::section(ui, "fill-color", "Fill and stroke");
        let (mut changed, mut immediate, mut typing) = (false, false, false);
        chrome::row(ui, "Fill", |ui| {
            if ui.color_edit_button_srgba(&mut e.fill).changed() {
                e.fill_changed = true;
                changed = true;
            }
            if shape.gradient().is_some() && !e.fill_changed {
                ui.weak("gradient");
            }
        });
        chrome::row(ui, "Stroke", |ui| {
            if ui.color_edit_button_srgba(&mut e.color).changed() {
                e.stroke_changed = true;
                changed = true;
            }
            if ui.checkbox(&mut e.outline, "Outline").changed() {
                e.stroke_changed = true;
                changed = true;
                immediate = true;
            }
        });
        chrome::row(ui, "Width", |ui| {
            let r = ui.add_enabled(
                e.outline,
                egui::DragValue::new(&mut e.width)
                    .range(0.5..=200.0)
                    .suffix(" px"),
            );
            if r.changed() {
                e.stroke_changed = true;
                changed = true;
            }
            typing |= r.has_focus() || r.dragged();
        });
        chrome::row(ui, "Cap", |ui| {
            egui::ComboBox::from_id_salt("shape_cap")
                .width(80.0)
                .selected_text(match e.cap {
                    Cap::Butt => "None",
                    Cap::Round => "Round",
                    Cap::Square => "Square",
                })
                .show_ui(ui, |ui| {
                    for (value, label) in [
                        (Cap::Butt, "None"),
                        (Cap::Round, "Round"),
                        (Cap::Square, "Square"),
                    ] {
                        if ui.selectable_value(&mut e.cap, value, label).changed() {
                            e.stroke_changed = true;
                            changed = true;
                            immediate = true;
                        }
                    }
                });
        });
        chrome::row(ui, "Join", |ui| {
            egui::ComboBox::from_id_salt("shape_join")
                .width(80.0)
                .selected_text(match e.join {
                    Join::Round => "Round",
                    Join::Bevel => "Bevel",
                    Join::Miter => "Miter",
                })
                .show_ui(ui, |ui| {
                    for (value, label) in [
                        (Join::Round, "Round"),
                        (Join::Bevel, "Bevel"),
                        (Join::Miter, "Miter"),
                    ] {
                        if ui.selectable_value(&mut e.join, value, label).changed() {
                            e.stroke_changed = true;
                            changed = true;
                            immediate = true;
                        }
                    }
                });
        });
        if changed {
            self.shape_paint_edit = Some(e);
        }
        let pointer_down = ui.input(|i| i.pointer.any_down());
        if let Some(edit) = self.shape_paint_edit
            && (immediate || (!pointer_down && !typing))
        {
            self.shape_paint_edit = None;
            self.apply_shape_paint(edit);
        }
    }

    /// Gives the shape the paint that was chosen in the panel: one undo step.
    fn apply_shape_paint(&mut self, edit: ShapePaintEdit) {
        let Some(scene) = self.scene_id() else { return };
        let stroke = if edit.stroke_changed {
            if edit.outline {
                match ShapeStroke::new(f64::from(edit.width), rgba(edit.color)) {
                    Ok(stroke) => Some(Some(stroke.with_cap(edit.cap).with_join(edit.join))),
                    Err(error) => return self.report(error.to_string()),
                }
            } else {
                Some(None)
            }
        } else {
            None
        };
        let fill = edit.fill_changed.then(|| rgba(edit.fill));
        if let Some(commands) = ops::restyle(self.document(), scene, edit.at, fill, stroke) {
            self.run_batch(commands, "change the shape's paint");
        }
    }
}
