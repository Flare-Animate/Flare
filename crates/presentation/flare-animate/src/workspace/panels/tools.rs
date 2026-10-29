//! The tool strip and the options of the tool in hand.

use crate::workspace::{Tool, Workspace};
use egui::Ui;
use flare_ui::chrome;
use flare_ui::icons;
use flare_ui::theme;

use super::*;

/// One cell of the tool grid: the tool it selects, or `None` while the tool does not exist.
const TOOL_GRID: &[(&str, &str, &str, Option<Tool>)] = &[
    ("tool-selection", "Selection", "V", Some(Tool::Select)),
    (
        "tool-subselection",
        "Subselection",
        "A",
        Some(Tool::Subselection),
    ),
    (
        "tool-transform",
        "Free Transform",
        "Q",
        Some(Tool::FreeTransform),
    ),
    ("tool-lasso", "Lasso", "L", Some(Tool::Lasso)),
    ("tool-pen", "Pen", "P", Some(Tool::Pen)),
    ("tool-text", "Text", "T", Some(Tool::Text)),
    ("tool-line", "Line", "N", Some(Tool::Line)),
    ("tool-rectangle", "Rectangle", "R", Some(Tool::Rectangle)),
    ("tool-oval", "Oval", "O", Some(Tool::Ellipse)),
    ("tool-polystar", "PolyStar", "G", Some(Tool::PolyStar)),
    ("tool-pencil", "Pencil", "Y", Some(Tool::Pencil)),
    ("tool-brush", "Brush", "B", Some(Tool::Brush)),
    ("tool-bucket", "Paint Bucket", "K", Some(Tool::Bucket)),
    ("tool-ink-bottle", "Ink Bottle", "S", Some(Tool::InkBottle)),
    ("tool-eyedropper", "Eyedropper", "I", Some(Tool::Eyedropper)),
    ("tool-eraser", "Eraser", "E", Some(Tool::Eraser)),
    ("tool-hand", "Hand", "H", Some(Tool::Hand)),
    ("tool-zoom", "Zoom", "Z", Some(Tool::Zoom)),
];

impl Workspace {
    // ---- tools -------------------------------------------------------------------------

    pub(crate) fn tools_panel(&mut self, ui: &mut Ui) {
        ui.add_space(4.0);
        ui.vertical_centered(|ui| {
            let mut chosen = self.tool;
            // Snug cells; two columns when the strip is wide enough for them, else one.
            ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
            let cell = TOOL_ICON + 6.0;
            let columns = if ui.available_width() >= cell * 2.0 + 3.0 {
                2
            } else {
                1
            };
            for pair in TOOL_GRID.chunks(columns) {
                ui.horizontal(|ui| {
                    // Centred in the panel: a pair is two buttons wide.
                    let used =
                        cell * columns as f32 + ui.spacing().item_spacing.x * (columns - 1) as f32;
                    ui.add_space(((ui.available_width() - used) / 2.0).max(0.0));
                    for (icon, name, key, tool) in pair {
                        let hint = format!("{name} ({key})");
                        match tool {
                            Some(tool) => {
                                if chrome::icon_button(
                                    ui,
                                    icon,
                                    TOOL_ICON,
                                    self.tool == *tool,
                                    true,
                                    &hint,
                                )
                                .clicked()
                                {
                                    chosen = *tool;
                                }
                            }
                            None => {
                                chrome::mock_icon_button(ui, icon, TOOL_ICON, &hint);
                            }
                        }
                    }
                });
            }
            self.set_tool(chosen);
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(2.0);
            self.color_wells(ui);
        });
    }

    /// The fill and stroke swatches with the swap and reset buttons beneath.
    pub(crate) fn color_wells(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.add_space(((ui.available_width() - 74.0) / 2.0).max(0.0));
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.add(icons::image("stroke-color", 14.0, theme::TEXT_DIM))
                        .on_hover_text("Stroke color");
                    ui.color_edit_button_srgba(&mut self.stroke_color);
                });
                ui.horizontal(|ui| {
                    ui.add(icons::image("fill-color", 14.0, theme::TEXT_DIM))
                        .on_hover_text("Fill color");
                    ui.color_edit_button_srgba(&mut self.fill);
                });
            });
        });
        ui.horizontal(|ui| {
            ui.add_space(((ui.available_width() - 70.0) / 2.0).max(0.0));
            if chrome::icon_button(ui, "default-colors", 12.0, false, true, "Black and white")
                .clicked()
            {
                self.default_colors();
            }
            if chrome::icon_button(ui, "swap-colors", 12.0, false, true, "Swap fill and stroke")
                .clicked()
            {
                self.swap_colors();
            }
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
            Tool::Pencil => ("tool-pencil", "Pencil"),
            Tool::Brush => ("tool-brush", "Brush"),
        };
        chrome::section(ui, icon, &format!("{title} tool"));
        match self.tool {
            Tool::Text => {
                chrome::row(ui, "Family", |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.text_family)
                            .desired_width(150.0)
                            .hint_text("Helvetica, sans-serif"),
                    );
                });
                chrome::row(ui, "Size", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.text_size)
                            .range(flare_text::MIN_SIZE..=flare_text::MAX_SIZE)
                            .suffix(" px"),
                    );
                });
                chrome::row(ui, "Style", |ui| {
                    ui.toggle_value(&mut self.text_bold, "Bold");
                    ui.toggle_value(&mut self.text_italic, "Italic");
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
                ui.weak("Drag to paint with the fill color; a pen presses wider.");
            }
            Tool::Lasso => {
                ui.weak("Drag a loop around shapes to select them; Shift adds to the selection.");
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
            Tool::Pencil => "Pencil: drag to draw",
            Tool::Brush => "Brush: drag to paint",
        }
    }
}
