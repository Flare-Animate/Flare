//! The Swatches panel, as Animate's: greys and the 216 web-safe colors in twelve columns, then the
//! person's own swatches. A click makes a swatch the fill or stroke color, whichever the Color
//! panel is setting; + keeps the current one; a right click on one of the person's own deletes it.

use egui::{Color32, Rect, Sense, Ui, Vec2, vec2};
use flare_core::Rgba8;
use flare_core::swatches::{self, MAX_CUSTOM_SWATCHES};
use flare_ui::{chrome, theme};

use crate::workspace::Workspace;
use crate::workspace::types::ColorTarget;

const COLUMNS: usize = 12;
const GAP: f32 = 1.0;

fn to_egui(c: Rgba8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

fn from_egui(c: Color32) -> Rgba8 {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    Rgba8::new(r, g, b, a)
}

/// What a click on the grid asked for.
enum Pick {
    Choose(Rgba8),
    Remove(usize),
}

impl Workspace {
    /// Makes `color` the color the Color panel is setting (fill, stroke or the gradient's end).
    pub fn choose_swatch(&mut self, color: Rgba8) {
        let color = to_egui(color);
        match self.color_target {
            ColorTarget::Fill => self.paint.fill = color,
            ColorTarget::Stroke => self.paint.stroke_color = color,
            ColorTarget::GradientEnd => self.paint.gradient_end = color,
        }
    }

    /// Keeps the color being set as a swatch of the person's own. Says so when the list is full.
    pub fn add_swatch(&mut self) -> bool {
        let current = match self.color_target {
            ColorTarget::Fill => self.paint.fill,
            ColorTarget::Stroke => self.paint.stroke_color,
            ColorTarget::GradientEnd => self.paint.gradient_end,
        };
        let added = self.paint.swatches.add(from_egui(current));
        if !added && self.paint.swatches.colors().len() >= MAX_CUSTOM_SWATCHES {
            let max = MAX_CUSTOM_SWATCHES.to_string();
            self.report(tr!(self, "swatches-full", &[("max", &max)]));
        }
        added
    }

    pub fn remove_swatch(&mut self, index: usize) {
        self.paint.swatches.remove(index);
    }

    pub(crate) fn swatches_panel(&mut self, ui: &mut Ui) {
        let (add_hint, remove_label, yours) = (
            tr!(self, "swatches-add"),
            tr!(self, "swatches-remove"),
            tr!(self, "swatches-yours"),
        );
        ui.weak(self.tr(match self.color_target {
            ColorTarget::Fill => "col-fill",
            ColorTarget::Stroke => "col-stroke",
            ColorTarget::GradientEnd => "col-gradient-end",
        }));
        let mut pick = None;
        pick = pick.or(grid(ui, "greys", &swatches::greys(), None));
        ui.add_space(4.0);
        pick = pick.or(grid(ui, "web-safe", &swatches::web_safe(), None));
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.weak(&yours);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if chrome::icon_button(ui, "plus", 14.0, false, true, &add_hint).clicked() {
                    self.add_swatch();
                }
            });
        });
        let own = self.paint.swatches.colors().to_vec();
        pick = pick.or(grid(ui, "own", &own, Some(&remove_label)));
        match pick {
            Some(Pick::Choose(color)) => self.choose_swatch(color),
            Some(Pick::Remove(index)) => self.remove_swatch(index),
            None => {}
        }
    }
}

/// The colors as cells, twelve to a row, as wide as the panel allows; `salt` keeps each grid's
/// cells apart. With `remove`, a right click on a cell offers to delete it.
fn grid(ui: &mut Ui, salt: &str, colors: &[Rgba8], remove: Option<&str>) -> Option<Pick> {
    if colors.is_empty() {
        return None;
    }
    let width = ui.available_width();
    let cell = ((width - GAP * (COLUMNS as f32 - 1.0)) / COLUMNS as f32).clamp(8.0, 20.0);
    let rows = colors.len().div_ceil(COLUMNS);
    let size = vec2(
        COLUMNS as f32 * (cell + GAP) - GAP,
        rows as f32 * (cell + GAP) - GAP,
    );
    let (area, _) = ui.allocate_exact_size(size, Sense::hover());
    let mut pick = None;
    for (index, color) in colors.iter().enumerate() {
        let (row, column) = (index / COLUMNS, index % COLUMNS);
        let min = area.min + vec2(column as f32 * (cell + GAP), row as f32 * (cell + GAP));
        let rect = Rect::from_min_size(min, Vec2::splat(cell));
        let response = ui.interact(rect, ui.scope_id().with((salt, index)), Sense::click());
        ui.painter().rect_filled(rect, 1.0, to_egui(*color));
        if response.hovered() {
            ui.painter().rect_stroke(
                rect.expand(1.0),
                1.0,
                egui::Stroke::new(1.0, theme::text_strong()),
                egui::StrokeKind::Inside,
            );
        }
        let hex = format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b);
        let response = response.on_hover_text(hex);
        if response.clicked() {
            pick = Some(Pick::Choose(*color));
        }
        if let Some(label) = remove {
            response.context_menu(|ui| {
                if ui.button(label).clicked() {
                    pick = Some(Pick::Remove(index));
                    ui.close();
                }
            });
        }
    }
    pick
}
