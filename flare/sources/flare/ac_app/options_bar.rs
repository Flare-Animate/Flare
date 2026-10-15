//! Tool Options bar (Window › Tool Options Bar; after Flare Studio's options
//! strip, our own code): a row under the menu bar with the current tool's
//! main settings, the stroke and fill colours and the drawing mode. The full
//! set stays in Properties › Tool.

use crate::toolbar::{FtMode, PencilMode, Tool};
use crate::App;
use eframe::egui::{self, Color32, RichText};

fn num(ui: &mut egui::Ui, label: &str, v: &mut f32, range: std::ops::RangeInclusive<f32>, suffix: &str) {
    ui.label(RichText::new(label).color(Color32::from_gray(170)));
    ui.add(egui::DragValue::new(v).range(range).speed(0.5).suffix(suffix).max_decimals(1));
}

pub(crate) fn bar(app: &mut App, ctx: &egui::Context) {
    if !app.settings.options_bar {
        return;
    }
    egui::TopBottomPanel::top("tool-options").exact_height(28.0).show(ctx, |ui| {
        ui.horizontal_centered(|ui| {
            let tool = app.tool;
            ui.label(RichText::new(tool.info().name).strong());
            ui.separator();
            ui.add_enabled_ui(app.stroke_on, |ui| ui.color_edit_button_srgba(&mut app.stroke)).response.on_hover_text("Stroke colour");
            ui.checkbox(&mut app.stroke_on, "").on_hover_text("Stroke on/off");
            ui.add(egui::DragValue::new(&mut app.stroke_w).range(0.1..=100.0).speed(0.1).suffix(" px").max_decimals(1)).on_hover_text("Stroke size");
            ui.add_enabled_ui(app.fill_on, |ui| ui.color_edit_button_srgba(&mut app.fill)).response.on_hover_text("Fill colour");
            ui.checkbox(&mut app.fill_on, "").on_hover_text("Fill on/off");
            ui.separator();
            let o = &mut app.topts;
            match tool {
                Tool::Rect | Tool::RectPrimitive => num(ui, "Corner radius", &mut o.rect_radius, 0.0..=100.0, " px"),
                Tool::Oval | Tool::OvalPrimitive => {
                    num(ui, "Start", &mut o.oval_start, 0.0..=360.0, "°");
                    num(ui, "End", &mut o.oval_end, 0.0..=360.0, "°");
                    num(ui, "Inner", &mut o.oval_inner, 0.0..=99.0, "%");
                }
                Tool::PolyStar => {
                    ui.selectable_value(&mut o.poly_star, false, "Polygon");
                    ui.selectable_value(&mut o.poly_star, true, "Star");
                    ui.label(RichText::new("Sides").color(Color32::from_gray(170)));
                    ui.add(egui::DragValue::new(&mut o.poly_sides).range(3..=32));
                }
                Tool::Pencil => {
                    ui.selectable_value(&mut o.pencil_mode, PencilMode::Straighten, "Straighten");
                    ui.selectable_value(&mut o.pencil_mode, PencilMode::Smooth, "Smooth");
                    ui.selectable_value(&mut o.pencil_mode, PencilMode::Ink, "Ink");
                }
                Tool::Brush => {
                    num(ui, "Size", &mut o.brush_size, 1.0..=200.0, " px");
                    num(ui, "Smoothing", &mut o.brush_smooth, 0.0..=100.0, "");
                    egui::ComboBox::from_id_salt("bar-brush-mode").selected_text(o.brush_mode.name(false)).show_ui(ui, |ui| {
                        for x in ac_core::merge::PaintMode::BRUSH {
                            ui.selectable_value(&mut o.brush_mode, x, x.name(false));
                        }
                    });
                }
                Tool::FluidBrush => {
                    num(ui, "Size", &mut o.fluid_size, 1.0..=200.0, " px");
                    num(ui, "Velocity", &mut o.fluid_sensitivity, 0.0..=100.0, "");
                }
                Tool::RasterBrush => {
                    num(ui, "Size", &mut o.raster_size, 1.0..=200.0, " px");
                    ui.label(RichText::new("Opacity").color(Color32::from_gray(170)));
                    ui.add(egui::Slider::new(&mut o.raster_opacity, 0.0..=1.0).show_value(false));
                    ui.checkbox(&mut o.raster_eraser, "Erase");
                }
                Tool::Eraser => {
                    num(ui, "Size", &mut o.eraser_size, 2.0..=100.0, " px");
                    egui::ComboBox::from_id_salt("bar-eraser-mode").selected_text(o.eraser_mode.name(true)).show_ui(ui, |ui| {
                        for x in ac_core::merge::PaintMode::ERASER {
                            ui.selectable_value(&mut o.eraser_mode, x, x.name(true));
                        }
                    });
                    ui.checkbox(&mut o.faucet, "Faucet");
                }
                Tool::Bucket => {
                    ui.label(RichText::new("Gap").color(Color32::from_gray(170)));
                    egui::ComboBox::from_id_salt("bar-gap").selected_text(["Don't close gaps", "Close small gaps", "Close medium gaps", "Close large gaps"][o.bucket_gap.min(3) as usize]).show_ui(ui, |ui| {
                        for (i, l) in ["Don't close gaps", "Close small gaps", "Close medium gaps", "Close large gaps"].iter().enumerate() {
                            ui.selectable_value(&mut o.bucket_gap, i as u8, *l);
                        }
                    });
                    ui.checkbox(&mut o.lock_fill, "Lock fill");
                }
                Tool::MagicWand => num(ui, "Threshold", &mut o.wand_threshold, 0.0..=200.0, ""),
                Tool::Text => {
                    num(ui, "Size", &mut o.text_size, 4.0..=200.0, " pt");
                    ui.toggle_value(&mut o.text_bold, RichText::new("B").strong());
                    ui.toggle_value(&mut o.text_italic, RichText::new("I").italics());
                }
                Tool::Magnet | Tool::Pinch | Tool::Pump | Tool::Bender | Tool::Iron => {
                    num(ui, "Size", &mut o.sculpt_size, 1.0..=400.0, " px");
                    ui.label(RichText::new("Strength").color(Color32::from_gray(170)));
                    ui.add(egui::Slider::new(&mut o.sculpt_strength, 0.0..=1.0).show_value(false));
                    if tool == Tool::Pinch {
                        ui.checkbox(&mut o.sculpt_sharp, "Corner");
                    }
                }
                Tool::FreeTransform => {
                    ui.selectable_value(&mut o.ft_mode, FtMode::Normal, "Scale / Rotate");
                    ui.selectable_value(&mut o.ft_mode, FtMode::Distort, "Distort");
                    ui.selectable_value(&mut o.ft_mode, FtMode::Envelope, "Envelope");
                }
                _ => {}
            }
            if tool.is_drawing() || matches!(tool, Tool::Brush | Tool::FluidBrush) {
                ui.separator();
                ui.toggle_value(&mut o.object_draw, "Object Drawing").on_hover_text("J: each shape its own object; off = Merge Drawing");
            }
            if matches!(tool, Tool::Select | Tool::Subselect | Tool::FreeTransform) || tool.is_drawing() {
                ui.toggle_value(&mut o.snap, "Snap").on_hover_text("Snap to objects (hold Alt to bypass)");
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("×").on_hover_text("Hide the Tool Options bar (Window menu)").clicked() {
                    app.toggle_options_bar();
                }
            });
        });
    });
}
