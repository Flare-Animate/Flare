//! The panels of the workspace: tools, properties, library, color, align, transform, history,
//! the timeline's toolbar and the status bar.
//!
//! Controls that act on the document are real. Controls for features that do not exist yet are
//! drawn the same way but dimmed, with a hover note, using the mock helpers in `chrome`; each
//! is a place a later change fills in.

use egui::{Align, Color32, Layout, RichText, Ui, Vec2};

use super::{
    ColorTarget, Command, Layer, LayerId, MAX_ONION_FRAMES, Mode, SymbolId, Tool, Workspace,
};
use crate::actions::MenuAction;
use crate::chrome::{self, NOT_YET};
use crate::icons;
use crate::theme;

const TOOL_ICON: f32 = 20.0;
const BAR_ICON: f32 = 16.0;

/// One cell of the tool grid: the tool it selects, or `None` while the tool does not exist.
const TOOL_GRID: &[(&str, &str, &str, Option<Tool>)] = &[
    ("tool-selection", "Selection", "V", Some(Tool::Select)),
    ("tool-subselection", "Subselection", "A", None),
    ("tool-transform", "Free Transform", "Q", None),
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

/// Swatches offered by the color panel.
const SWATCHES: [[u8; 3]; 24] = [
    [0, 0, 0],
    [64, 64, 64],
    [128, 128, 128],
    [192, 192, 192],
    [255, 255, 255],
    [232, 69, 60],
    [244, 129, 48],
    [248, 190, 52],
    [252, 238, 90],
    [160, 210, 70],
    [60, 176, 90],
    [38, 166, 154],
    [40, 190, 220],
    [60, 130, 240],
    [76, 80, 220],
    [140, 82, 220],
    [200, 80, 200],
    [236, 90, 150],
    [120, 72, 48],
    [170, 120, 80],
    [226, 190, 150],
    [24, 40, 80],
    [16, 70, 60],
    [90, 20, 40],
];

impl Workspace {
    // ---- tools -------------------------------------------------------------------------

    pub(super) fn tools_panel(&mut self, ui: &mut Ui) {
        ui.add_space(4.0);
        ui.vertical_centered(|ui| {
            let mut chosen = self.tool;
            // Two columns, as many rows as the tools need.
            for pair in TOOL_GRID.chunks(2) {
                ui.horizontal(|ui| {
                    // Centred in the panel: a pair is two buttons wide.
                    let used = (TOOL_ICON + 10.0) * 2.0 + ui.spacing().item_spacing.x;
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
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);
            self.color_wells(ui);
        });
    }

    /// The fill and stroke swatches with the swap and reset buttons beneath.
    fn color_wells(&mut self, ui: &mut Ui) {
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
            chrome::mock_icon_button(ui, "default-colors", 12.0, "Black and white");
            chrome::mock_icon_button(ui, "swap-colors", 12.0, "Swap fill and stroke");
        });
    }

    // ---- properties --------------------------------------------------------------------

    pub(super) fn properties_panel(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                let selection = self.selection_bounds();
                // What the panel is about right now.
                let (icon, title) = if selection.is_some() {
                    ("tool-selection", "Selection")
                } else {
                    ("document", "Document")
                };
                ui.horizontal(|ui| {
                    ui.add(icons::image(icon, 18.0, theme::ACCENT));
                    ui.label(RichText::new(title).heading());
                });
                if let Some(bounds) = selection {
                    let count = self.selection_all().len();
                    let title = if count == 1 {
                        "Selection".to_owned()
                    } else {
                        format!("Selection ({count})")
                    };
                    chrome::section(ui, "tool-selection", &title);
                    chrome::row(ui, "Position", |ui| {
                        chrome::mock_number(ui, &format!("{:.1}", bounds.min.x), "");
                        chrome::mock_number(ui, &format!("{:.1}", bounds.min.y), "");
                    });
                    chrome::row(ui, "Size", |ui| {
                        chrome::mock_number(ui, &format!("{:.1}", bounds.width()), "");
                        chrome::mock_number(ui, &format!("{:.1}", bounds.height()), "");
                    });
                }
                self.tool_options(ui);
                chrome::section(ui, "fill-color", "Fill and stroke");
                chrome::row(ui, "Fill", |ui| {
                    ui.color_edit_button_srgba(&mut self.fill);
                    ui.checkbox(&mut self.gradient_enabled, "Gradient");
                });
                if self.gradient_enabled {
                    chrome::row(ui, "To", |ui| {
                        ui.color_edit_button_srgba(&mut self.gradient_end);
                        ui.checkbox(&mut self.gradient_vertical, "Vertical");
                    });
                }
                chrome::row(ui, "Stroke", |ui| {
                    ui.color_edit_button_srgba(&mut self.stroke_color);
                    ui.checkbox(&mut self.stroke_enabled, "Outline");
                });
                chrome::row(ui, "Width", |ui| {
                    ui.add_enabled(
                        self.stroke_enabled,
                        egui::DragValue::new(&mut self.stroke_width)
                            .range(0.5..=200.0)
                            .suffix(" px"),
                    );
                });
                chrome::row(ui, "Cap / join", |ui| {
                    chrome::mock_number(ui, "Round", "");
                });
                self.document_section(ui);
            });
    }

    /// Options of the current tool, if it has any.
    fn tool_options(&mut self, ui: &mut Ui) {
        let (icon, title) = match self.tool {
            Tool::Select => return,
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
                    chrome::mock_number(ui, "Regular", "");
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
                ui.weak("Drag to paint with the fill color.");
            }
            Tool::Lasso => {
                ui.weak("Drag a loop around shapes to select them; Shift adds to the selection.");
            }
            Tool::Line => {
                ui.weak("Drag on the stage. A line always has a stroke.");
            }
            Tool::Select => {}
        }
    }

    /// Stage size, background and the facts about the document.
    fn document_section(&mut self, ui: &mut Ui) {
        chrome::section(ui, "document", "Document");
        self.stage_settings(ui);
        let rate = self.document().frame_rate();
        chrome::row(ui, "Frame rate", |ui| {
            let fps = f64::from(rate.numerator()) / f64::from(rate.denominator());
            chrome::mock_number(ui, &format!("{fps:.2}"), " fps");
        });
        chrome::row(ui, "Scenes", |ui| {
            ui.label(self.document().scenes().len().to_string());
        });
        chrome::row(ui, "Sounds", |ui| {
            ui.label(self.document().sounds().len().to_string());
        });
    }

    // ---- library -----------------------------------------------------------------------

    pub(super) fn library_panel(&mut self, ui: &mut Ui) {
        let entries: Vec<(SymbolId, String)> = self
            .document()
            .symbols()
            .iter()
            .map(|s| (s.id(), s.name().to_owned()))
            .collect();
        // A preview well above the list, as in other animation tools.
        let well = egui::Frame::new()
            .fill(theme::FIELD)
            .stroke(egui::Stroke::new(1.0, theme::BORDER))
            .corner_radius(4.0);
        well.show(ui, |ui| {
            ui.set_min_height(64.0);
            ui.set_width(ui.available_width());
            ui.vertical_centered(|ui| {
                ui.add_space(14.0);
                ui.add(icons::image("symbol-graphic", 28.0, theme::TEXT_DISABLED));
                ui.weak(
                    match self.selected_symbol.and_then(|id| {
                        entries
                            .iter()
                            .find(|(s, _)| *s == id)
                            .map(|(_, n)| n.clone())
                    }) {
                        Some(name) => name,
                        None => "No symbol selected".to_owned(),
                    },
                );
            });
        });
        ui.add_space(4.0);
        let mut search = String::new();
        ui.add_enabled(
            false,
            egui::TextEdit::singleline(&mut search)
                .hint_text("Search library")
                .desired_width(f32::INFINITY),
        )
        .on_disabled_hover_text(NOT_YET);
        ui.add_space(4.0);
        let list_height = (ui.available_height() - 34.0).max(40.0);
        egui::ScrollArea::vertical()
            .max_height(list_height)
            .auto_shrink(false)
            .show(ui, |ui| {
                if entries.is_empty() {
                    chrome::empty_state(ui, "panel-library", "No symbols yet");
                    ui.vertical_centered(|ui| {
                        ui.weak("Select artwork and press F8 to make one.");
                    });
                }
                for (id, name) in entries {
                    let chosen = self.selected_symbol == Some(id);
                    let response = ui
                        .horizontal(|ui| {
                            ui.add(icons::image("symbol-graphic", 16.0, theme::TEXT_DIM));
                            ui.selectable_label(chosen, name)
                        })
                        .inner;
                    if response.clicked() {
                        self.selected_symbol = Some(id);
                    }
                    if response.double_clicked() {
                        self.selected_symbol = Some(id);
                        self.place_selected_symbol();
                    }
                }
            });
        ui.separator();
        ui.horizontal(|ui| {
            if chrome::icon_button(
                ui,
                "plus",
                BAR_ICON,
                false,
                self.selected_symbol.is_some(),
                "Place on stage",
            )
            .clicked()
            {
                self.place_selected_symbol();
            }
            chrome::mock_icon_button(ui, "symbol-movie-clip", BAR_ICON, "New symbol");
            chrome::mock_icon_button(ui, "folder", BAR_ICON, "New folder");
            chrome::mock_icon_button(ui, "settings", BAR_ICON, "Symbol properties");
            chrome::mock_icon_button(ui, "delete", BAR_ICON, "Delete symbol");
        });
    }

    // ---- color -------------------------------------------------------------------------

    pub(super) fn color_panel(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            for (target, icon, hint) in [
                (ColorTarget::Fill, "fill-color", "Edit the fill color"),
                (ColorTarget::Stroke, "stroke-color", "Edit the stroke color"),
            ] {
                if chrome::icon_button(ui, icon, BAR_ICON, self.color_target == target, true, hint)
                    .clicked()
                {
                    self.color_target = target;
                }
            }
            ui.weak(match self.color_target {
                ColorTarget::Fill => "Fill",
                ColorTarget::Stroke => "Stroke",
            });
        });
        let target = match self.color_target {
            ColorTarget::Fill => &mut self.fill,
            ColorTarget::Stroke => &mut self.stroke_color,
        };
        egui::color_picker::color_picker_color32(ui, target, egui::color_picker::Alpha::OnlyBlend);
        ui.add_space(4.0);
        chrome::row(ui, "Hex", |ui| {
            let c = *target;
            chrome::mock_number(ui, &format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b()), "");
        });
        ui.add_space(4.0);
        ui.weak("Swatches");
        let chosen = ui
            .horizontal_wrapped(|ui| {
                let mut chosen = None;
                for [r, g, b] in SWATCHES {
                    let color = Color32::from_rgb(r, g, b);
                    let (rect, response) =
                        ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::click());
                    ui.painter().rect_filled(rect, 3.0, color);
                    ui.painter().rect_stroke(
                        rect,
                        3.0,
                        egui::Stroke::new(
                            1.0,
                            if response.hovered() {
                                Color32::WHITE
                            } else {
                                theme::BORDER
                            },
                        ),
                        egui::StrokeKind::Inside,
                    );
                    if response.clicked() {
                        chosen = Some(color);
                    }
                }
                chosen
            })
            .inner;
        if let Some(color) = chosen {
            match self.color_target {
                ColorTarget::Fill => self.fill = color,
                ColorTarget::Stroke => self.stroke_color = color,
            }
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            chrome::mock_icon_button(ui, "gradient", BAR_ICON, "Gradient editor");
            chrome::mock_icon_button(
                ui,
                "eyedropper-small",
                BAR_ICON,
                "Pick a color from the stage",
            );
            chrome::mock_icon_button(ui, "no-color", BAR_ICON, "No color");
        });
    }

    // ---- align -------------------------------------------------------------------------

    pub(super) fn align_panel(&mut self, ui: &mut Ui) {
        chrome::section(ui, "panel-align", "Align");
        for group in [
            [
                ("align-left", "Align left edges"),
                ("align-center-h", "Align horizontal centers"),
                ("align-right", "Align right edges"),
            ],
            [
                ("align-top", "Align top edges"),
                ("align-center-v", "Align vertical centers"),
                ("align-bottom", "Align bottom edges"),
            ],
        ] {
            ui.horizontal(|ui| {
                for (icon, hint) in group {
                    chrome::mock_icon_button(ui, icon, BAR_ICON, hint);
                }
            });
        }
        chrome::section(ui, "distribute-h", "Distribute");
        ui.horizontal(|ui| {
            chrome::mock_icon_button(ui, "distribute-h", BAR_ICON, "Distribute horizontally");
            chrome::mock_icon_button(ui, "distribute-v", BAR_ICON, "Distribute vertically");
        });
        chrome::section(ui, "bring-front", "Arrange");
        ui.horizontal(|ui| {
            chrome::mock_icon_button(ui, "bring-front", BAR_ICON, "Bring to front");
            chrome::mock_icon_button(ui, "send-back", BAR_ICON, "Send to back");
            chrome::mock_icon_button(ui, "group", BAR_ICON, "Group");
            chrome::mock_icon_button(ui, "ungroup", BAR_ICON, "Ungroup");
        });
        ui.add_space(6.0);
        let mut to_stage = self.align_to_stage;
        ui.add_enabled(false, egui::Checkbox::new(&mut to_stage, "Align to stage"))
            .on_disabled_hover_text(NOT_YET);
    }

    // ---- transform ---------------------------------------------------------------------

    pub(super) fn transform_panel(&mut self, ui: &mut Ui) {
        chrome::section(ui, "panel-transform", "Transform");
        chrome::row(ui, "Scale", |ui| {
            chrome::mock_number(ui, "100", "%");
            chrome::mock_number(ui, "100", "%");
        });
        chrome::row(ui, "Rotate", |ui| {
            chrome::mock_number(ui, "0", "\u{b0}");
        });
        chrome::row(ui, "Skew", |ui| {
            chrome::mock_number(ui, "0", "\u{b0}");
            chrome::mock_number(ui, "0", "\u{b0}");
        });
        chrome::section(ui, "rotate-cw", "Quick transforms");
        let enabled = self.has_selection();
        ui.horizontal(|ui| {
            for (icon, hint, action) in [
                (
                    "flip-horizontal",
                    "Flip horizontal",
                    MenuAction::FlipHorizontal,
                ),
                ("flip-vertical", "Flip vertical", MenuAction::FlipVertical),
                (
                    "rotate-cw",
                    "Rotate 90\u{b0} clockwise",
                    MenuAction::RotateClockwise,
                ),
                (
                    "rotate-ccw",
                    "Rotate 90\u{b0} counterclockwise",
                    MenuAction::RotateCounterclockwise,
                ),
                ("zoom-in", "Scale up", MenuAction::ScaleUp),
                ("zoom-out", "Scale down", MenuAction::ScaleDown),
            ] {
                if chrome::icon_button(ui, icon, BAR_ICON, false, enabled, hint).clicked() {
                    self.perform(action);
                }
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            chrome::mock_icon_button(ui, "break-apart", BAR_ICON, "Break apart");
            chrome::mock_icon_button(ui, "convert-symbol", BAR_ICON, "Convert to symbol (F8)");
        });
    }

    // ---- history -----------------------------------------------------------------------

    pub(super) fn history_panel(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if chrome::icon_button(
                ui,
                "undo",
                BAR_ICON,
                false,
                self.is_enabled(MenuAction::Undo),
                "Undo",
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
                "Redo",
            )
            .clicked()
            {
                self.perform(MenuAction::Redo);
            }
            chrome::mock_icon_button(ui, "delete", BAR_ICON, "Clear history");
        });
        chrome::empty_state(ui, "panel-history", "Each edit will be listed here");
        ui.vertical_centered(|ui| {
            ui.weak("Click an entry to return to that point.");
        });
    }

    // ---- timeline toolbar --------------------------------------------------------------

    /// Transport, onion skin, zoom and layer buttons above the timeline.
    pub(super) fn timeline_toolbar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let last = self.scene_frames() - 1;
            if chrome::icon_button(ui, "transport-first", BAR_ICON, false, true, "First frame")
                .clicked()
            {
                self.set_frame(0);
            }
            if chrome::icon_button(
                ui,
                "transport-prev",
                BAR_ICON,
                false,
                true,
                "Previous frame",
            )
            .clicked()
            {
                self.set_frame(self.frame.saturating_sub(1));
            }
            let (icon, hint) = if self.playing {
                ("transport-pause", "Pause")
            } else {
                ("transport-play", "Play")
            };
            if chrome::icon_button(ui, icon, BAR_ICON, self.playing, true, hint).clicked() {
                self.playing = !self.playing;
            }
            if chrome::icon_button(ui, "transport-next", BAR_ICON, false, true, "Next frame")
                .clicked()
            {
                self.set_frame(self.frame.saturating_add(1));
            }
            if chrome::icon_button(ui, "transport-last", BAR_ICON, false, true, "Last frame")
                .clicked()
            {
                self.set_frame(last);
            }
            chrome::mock_icon_button(ui, "transport-loop", BAR_ICON, "Loop playback");
            ui.separator();
            let mut frame = self.frame;
            ui.add(
                egui::DragValue::new(&mut frame)
                    .range(0..=last)
                    .prefix("Frame "),
            );
            if frame != self.frame {
                self.set_frame(frame);
            }
            let rate = self.document().frame_rate();
            let fps = f64::from(rate.numerator()) / f64::from(rate.denominator());
            ui.label(RichText::new(format!("{fps:.2} fps")).color(theme::TEXT_DIM));
            ui.separator();
            let mut onion = self.onion.enabled;
            if chrome::icon_button(ui, "onion-skin", BAR_ICON, onion, true, "Onion skin").clicked()
            {
                onion = !onion;
            }
            self.set_onion_skin(onion);
            chrome::mock_icon_button(ui, "onion-outline", BAR_ICON, "Onion skin outlines");
            ui.add_enabled_ui(onion, |ui| {
                let (mut before, mut after) = (self.onion.before, self.onion.after);
                ui.add(
                    egui::DragValue::new(&mut before)
                        .range(0..=MAX_ONION_FRAMES)
                        .prefix("\u{2190} "),
                );
                ui.add(
                    egui::DragValue::new(&mut after)
                        .range(0..=MAX_ONION_FRAMES)
                        .suffix(" \u{2192}"),
                );
                self.set_onion_range(before, after);
            });
            ui.separator();
            if chrome::icon_button(ui, "layer-new", BAR_ICON, false, true, "New layer").clicked() {
                self.add_layer();
            }
            chrome::mock_icon_button(ui, "layer-folder", BAR_ICON, "New layer folder");
            chrome::mock_icon_button(ui, "layer-motion-guide", BAR_ICON, "Add motion guide");
            if chrome::icon_button(
                ui,
                "layer-mask",
                BAR_ICON,
                self.active_layer().is_some_and(Layer::is_mask),
                self.is_enabled(MenuAction::ToggleLayerMask),
                "Mask the layer below",
            )
            .clicked()
            {
                self.perform(MenuAction::ToggleLayerMask);
            }
            if chrome::icon_button(
                ui,
                "layer-delete",
                BAR_ICON,
                false,
                self.is_enabled(MenuAction::DeleteLayer),
                "Delete layer",
            )
            .clicked()
            {
                self.perform(MenuAction::DeleteLayer);
            }
            ui.separator();
            for (icon, hint, factor) in [
                (
                    "zoom-out",
                    "Zoom timeline out",
                    1.0 / crate::timeline_view::STEP,
                ),
                ("zoom-in", "Zoom timeline in", crate::timeline_view::STEP),
            ] {
                if chrome::icon_button(ui, icon, BAR_ICON, false, true, hint).clicked() {
                    self.zoom_timeline(factor);
                }
            }
            if chrome::icon_button(ui, "zoom-fit", BAR_ICON, false, true, "Fit the scene").clicked()
            {
                self.timeline_zoom = crate::timeline_view::MIN_ZOOM;
            }
            ui.separator();
            let edit = ui.add(
                egui::TextEdit::singleline(&mut self.layer_name_edit)
                    .hint_text("Rename layer")
                    .desired_width(120.0),
            );
            let submitted = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if submitted && !self.layer_name_edit.is_empty() {
                let name = std::mem::take(&mut self.layer_name_edit);
                self.rename_active_layer(&name);
            }
        });
    }

    /// A new empty layer above the others, made the active one: one undo step.
    pub fn add_layer(&mut self) {
        let Some(scene) = self.scene_id() else {
            return;
        };
        let count = self
            .document()
            .scenes()
            .first()
            .map_or(0, |s| s.timeline().layers().len());
        let id = LayerId::new();
        let layer = match Layer::new(id, format!("Layer {}", count + 1), vec![]) {
            Ok(layer) => layer,
            Err(error) => return self.report(error.to_string()),
        };
        self.run_batch(vec![Command::AddLayer { scene, layer }], "add a layer");
        if self.status.is_none() {
            self.current_layer = Some(id);
        }
    }

    // ---- status bar --------------------------------------------------------------------

    pub(super) fn status_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let message = match &self.status {
                Some(status) => RichText::new(status.as_str()).color(theme::WARNING),
                None => RichText::new(self.tool_hint()).color(theme::TEXT_DIM),
            };
            ui.label(message);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let extent = self.stage_extent();
                let percent = self.stage_view.scale(self.stage_panel, extent).max(0.0) * 100.0;
                ui.label(RichText::new(format!("{percent:.0}%")).color(theme::TEXT_DIM));
                ui.separator();
                ui.label(
                    RichText::new(format!(
                        "Frame {} / {}",
                        self.frame + 1,
                        self.scene_frames()
                    ))
                    .color(theme::TEXT_DIM),
                );
                ui.separator();
                let at = match self.pointer_stage {
                    Some(p) => format!("{:.0}, {:.0}", p.x, p.y),
                    None => "-, -".to_owned(),
                };
                ui.label(RichText::new(at).color(theme::TEXT_DIM).monospace());
                ui.add(icons::image("tool-selection", 12.0, theme::TEXT_DISABLED));
                if self.background.mode() == Mode::Threads && self.has_pending_io() {
                    ui.separator();
                    ui.label(RichText::new("Working...").color(theme::ACCENT));
                }
            });
        });
    }

    /// What the current tool does, shown when there is nothing else to say.
    fn tool_hint(&self) -> &'static str {
        match self.tool {
            Tool::Select => "Selection: click to select, drag to move, Shift-click to add",
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

    // ---- document tabs -----------------------------------------------------------------

    /// The strip above the stage with the open document as a tab.
    pub(super) fn document_tabs(&mut self, ui: &mut Ui) {
        let name = self
            .path()
            .and_then(std::path::Path::file_name)
            .map_or_else(
                || "Untitled".to_owned(),
                |n| n.to_string_lossy().into_owned(),
            );
        let dirty = self.is_dirty();
        ui.horizontal(|ui| {
            egui::Frame::new()
                .fill(theme::PANEL)
                .stroke(egui::Stroke::new(1.0, theme::BORDER))
                .corner_radius(egui::CornerRadius {
                    nw: 6,
                    ne: 6,
                    sw: 0,
                    se: 0,
                })
                .inner_margin(egui::Margin::symmetric(8, 3))
                .show(ui, |ui| {
                    ui.add(icons::image("document", 14.0, theme::TEXT_DIM));
                    ui.label(RichText::new(name).color(theme::TEXT));
                    if dirty {
                        ui.label(RichText::new("\u{25cf}").color(theme::ACCENT).small());
                    }
                    chrome::mock_icon_button(ui, "close", 10.0, "Close document");
                });
            chrome::mock_icon_button(ui, "plus", 12.0, "New document");
        });
    }

    // ---- stage bar ---------------------------------------------------------------------

    /// The bar above the stage: where you are, and zoom.
    pub(super) fn stage_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let crumbs = self.breadcrumb();
            ui.add(icons::image("scene", 16.0, theme::TEXT_DIM));
            for (depth, name) in crumbs.iter().enumerate() {
                if depth > 0 {
                    ui.add(icons::image("chevron-right", 12.0, theme::TEXT_DISABLED));
                }
                let last = depth + 1 == crumbs.len();
                let label = if last {
                    RichText::new(name).strong()
                } else {
                    RichText::new(name).color(theme::TEXT_DIM)
                };
                if ui.link(label).clicked() && !last {
                    self.exit_to_depth(depth);
                    break;
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if chrome::icon_button(ui, "zoom-in", BAR_ICON, false, true, "Zoom in").clicked() {
                    self.perform(MenuAction::ZoomIn);
                }
                let extent = self.stage_extent();
                let mut percent = self.stage_view.scale(self.stage_panel, extent) * 100.0;
                let before = percent;
                ui.add(
                    egui::DragValue::new(&mut percent)
                        .range(5.0..=3200.0)
                        .speed(2.0)
                        .suffix("%")
                        .fixed_decimals(0),
                );
                if (percent - before).abs() > f32::EPSILON {
                    self.set_stage_percent(percent);
                }
                if chrome::icon_button(ui, "zoom-out", BAR_ICON, false, true, "Zoom out").clicked()
                {
                    self.perform(MenuAction::ZoomOut);
                }
                if chrome::icon_button(ui, "zoom-fit", BAR_ICON, false, true, "Fit stage in window")
                    .clicked()
                {
                    self.perform(MenuAction::ZoomFit);
                }
                chrome::mock_icon_button(ui, "grid", BAR_ICON, "Show grid");
                chrome::mock_icon_button(ui, "ruler", BAR_ICON, "Show rulers");
                chrome::mock_icon_button(ui, "snap", BAR_ICON, "Snap to objects");
            });
        });
    }

    /// Shows the stage at `percent` of its real size, about the center of the view.
    pub fn set_stage_percent(&mut self, percent: f32) {
        let extent = self.stage_extent();
        let current = self.stage_view.scale(self.stage_panel, extent);
        let wanted = (percent / 100.0).max(0.001);
        self.stage_view
            .zoom_center(wanted / current.max(f32::EPSILON), self.stage_panel, extent);
    }
}
