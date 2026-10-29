//! The Color panel.

use crate::workspace::{ColorTarget, Tool, Workspace};
use egui::{Color32, Ui, Vec2};
use flare_ui::chrome;
use flare_ui::theme;

use super::*;

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
    // ---- color -------------------------------------------------------------------------

    pub(crate) fn color_panel(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            for (target, icon, hint) in [
                (ColorTarget::Fill, "fill-color", "Edit the fill color"),
                (ColorTarget::Stroke, "stroke-color", "Edit the stroke color"),
                (
                    ColorTarget::GradientEnd,
                    "gradient",
                    "Edit the second color of the gradient",
                ),
            ] {
                if chrome::icon_button(ui, icon, BAR_ICON, self.color_target == target, true, hint)
                    .clicked()
                {
                    self.color_target = target;
                    // Choosing the gradient's color turns the gradient on.
                    if target == ColorTarget::GradientEnd {
                        self.paint.gradient_enabled = true;
                    }
                }
            }
            ui.weak(match self.color_target {
                ColorTarget::Fill => "Fill",
                ColorTarget::Stroke => "Stroke",
                ColorTarget::GradientEnd => "Gradient end",
            });
        });
        let target = match self.color_target {
            ColorTarget::Fill => &mut self.paint.fill,
            ColorTarget::Stroke => &mut self.paint.stroke_color,
            ColorTarget::GradientEnd => &mut self.paint.gradient_end,
        };
        egui::color_picker::color_picker_color32(ui, target, egui::color_picker::Alpha::OnlyBlend);
        ui.add_space(4.0);
        chrome::row(ui, "Hex", |ui| {
            let c = *target;
            let current = format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b());
            let mut text = self.hex_buffer.clone().unwrap_or_else(|| current.clone());
            let response = ui.add(egui::TextEdit::singleline(&mut text).desired_width(80.0));
            if response.changed() {
                if let Some(parsed) = parse_hex(&text) {
                    *target =
                        Color32::from_rgba_unmultiplied(parsed.r(), parsed.g(), parsed.b(), c.a());
                }
                self.hex_buffer = Some(text);
            }
            if !response.has_focus() {
                self.hex_buffer = None;
            }
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
                ColorTarget::Fill => self.paint.fill = color,
                ColorTarget::Stroke => self.paint.stroke_color = color,
                ColorTarget::GradientEnd => self.paint.gradient_end = color,
            }
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if chrome::icon_button(
                ui,
                "gradient",
                BAR_ICON,
                self.paint.gradient_enabled,
                true,
                "Fill new shapes with a gradient",
            )
            .clicked()
            {
                self.paint.gradient_enabled = !self.paint.gradient_enabled;
            }
            if chrome::icon_button(
                ui,
                "eyedropper-small",
                BAR_ICON,
                self.tool == Tool::Eyedropper,
                true,
                "Pick a color from the stage",
            )
            .clicked()
            {
                self.set_tool(Tool::Eyedropper);
            }
            if chrome::icon_button(ui, "no-color", BAR_ICON, false, true, "No color").clicked() {
                match self.color_target {
                    ColorTarget::Fill => self.paint.fill = Color32::TRANSPARENT,
                    ColorTarget::Stroke => self.paint.stroke_color = Color32::TRANSPARENT,
                    ColorTarget::GradientEnd => self.paint.gradient_end = Color32::TRANSPARENT,
                }
            }
        });
    }
}

/// `#RRGGBB` or `RRGGBB` (also three-digit `#RGB`) as an opaque color.
pub(crate) fn parse_hex(text: &str) -> Option<Color32> {
    let digits = text.trim().trim_start_matches('#');
    let expand = |s: &str| u8::from_str_radix(s, 16).ok();
    match digits.len() {
        6 => Some(Color32::from_rgb(
            expand(&digits[0..2])?,
            expand(&digits[2..4])?,
            expand(&digits[4..6])?,
        )),
        3 => {
            let d = |i: usize| expand(&digits[i..=i]).map(|v| v * 17);
            Some(Color32::from_rgb(d(0)?, d(1)?, d(2)?))
        }
        _ => None,
    }
}
