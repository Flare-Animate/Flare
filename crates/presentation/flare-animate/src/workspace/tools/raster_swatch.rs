//! The Brush's presets on a raster layer, each shown by the mark it paints: a list of swatches
//! under the preset's name, and the brush in hand drawn under the list, redrawn as its settings
//! change. Each swatch is laid by the stroke engine itself (`flare_tools::blend::raster_swatch`)
//! in white along the vector Brush gallery's sample stroke, and is tinted with the text color as
//! that gallery's are, so the two lists look alike and the chosen entry takes the highlight.
//! Swatches are made once and kept as textures, so the list costs nothing to draw again.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use egui::{Align2, Color32, FontId, Rect, Sense, TextureHandle, Ui, Vec2};
use flare_tools::blend::{BlendSettings, MAX_SWATCH_SIDE, RasterPreset, raster_swatch};
use flare_ui::chrome;

use crate::workspace::*;

/// The size of one list entry, and of the swatch of the brush in hand, in points.
const ENTRY: Vec2 = Vec2::new(200.0, 46.0);
const CURRENT: Vec2 = Vec2::new(200.0, 34.0);
/// Space round a swatch's stroke inside its box, in points, as in the vector gallery.
const PAD: f32 = 8.0;
/// The most swatches kept: the presets and a few of the brush in hand as it is edited.
const KEPT: usize = 24;

/// Swatches already made, by what they show.
#[derive(Default)]
pub(in crate::workspace) struct Swatches {
    made: Vec<(u64, TextureHandle)>,
}

#[cfg(test)]
impl Swatches {
    pub(in crate::workspace) fn len(&self) -> usize {
        self.made.len()
    }
}

/// What a swatch shows: its settings, its size and its margin in pixels.
fn key(settings: &BlendSettings, size: [u32; 3]) -> u64 {
    let mut h = DefaultHasher::new();
    for v in [
        settings.size,
        settings.strength,
        settings.hardness,
        settings.mix,
    ] {
        v.to_bits().hash(&mut h);
    }
    (settings.pressure_size, settings.stretch).hash(&mut h);
    size.hash(&mut h);
    h.finish()
}

impl Workspace {
    /// The texture of the mark `settings` paints in a box of `size` points; made the first time
    /// it is asked for. `None` when it cannot be drawn.
    fn swatch(&mut self, ui: &Ui, settings: &BlendSettings, size: Vec2) -> Option<TextureHandle> {
        let scale = ui.ctx().pixels_per_point();
        let px = |v: f32| ((v * scale).round() as u32).clamp(8, MAX_SWATCH_SIDE);
        let pixels = [px(size.x), px(size.y), (PAD * scale).round() as u32];
        let id = key(settings, pixels);
        let swatches = &mut self.paint.blend.swatches;
        if let Some((_, texture)) = swatches.made.iter().find(|(k, _)| *k == id) {
            return Some(texture.clone());
        }
        let rgba = raster_swatch(*settings, pixels[0], pixels[1], pixels[2]).ok()?;
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [pixels[0] as usize, pixels[1] as usize],
            &rgba,
        );
        let texture = ui.ctx().load_texture(
            format!("raster swatch {id}"),
            image,
            egui::TextureOptions::LINEAR,
        );
        if swatches.made.len() >= KEPT {
            drop(swatches.made.remove(0));
        }
        swatches.made.push((id, texture.clone()));
        Some(texture)
    }

    /// The Preset row: the list of presets, each over its swatch, and the brush in hand drawn
    /// under it. Choosing one sets every setting below; once one is changed the list says
    /// Custom.
    pub(in crate::workspace) fn raster_presets(&mut self, ui: &mut Ui) {
        let current = RasterPreset::of(&self.paint.blend.raster);
        let shown = current.map_or_else(
            || tr!(self, "raster-preset-custom"),
            |p| self.tr(&format!("raster-preset-{}", p.name())),
        );
        let entries: Vec<(RasterPreset, String, Option<TextureHandle>)> = RasterPreset::ALL
            .into_iter()
            .map(|p| {
                let name = self.tr(&format!("raster-preset-{}", p.name()));
                let swatch = self.swatch(ui, &p.settings(), ENTRY - Vec2::new(0.0, 14.0));
                (p, name, swatch)
            })
            .collect();
        let mut chosen = None;
        chrome::row(ui, &tr!(self, "opt-raster-preset"), |ui| {
            egui::ComboBox::from_id_salt("raster-preset")
                .selected_text(shown)
                .width(ui.available_width().min(ENTRY.x))
                .truncate()
                .height(420.0)
                .show_ui(ui, |ui| {
                    for (preset, name, swatch) in &entries {
                        if entry(ui, name, swatch.as_ref(), current == Some(*preset)) {
                            chosen = Some(*preset);
                        }
                    }
                });
        });
        if let Some(preset) = chosen {
            self.paint.blend.raster = preset.settings();
        }
        let size = Vec2::new(ui.available_width().clamp(8.0, CURRENT.x), CURRENT.y);
        let settings = self.paint.blend.raster;
        let swatch = self.swatch(ui, &settings, size);
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let color = ui.visuals().text_color();
        paint_swatch(ui, rect, swatch.as_ref(), color);
    }
}

/// One list entry: the preset's name over its swatch, highlighted when it is the one in hand.
/// Whether it was clicked.
fn entry(ui: &mut Ui, name: &str, swatch: Option<&TextureHandle>, selected: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(ENTRY, Sense::click());
    let visuals = ui.style().interact_selectable(&response, selected);
    if selected || response.hovered() {
        ui.painter()
            .rect_filled(rect, visuals.corner_radius, visuals.weak_bg_fill);
    }
    let text = visuals.text_color();
    ui.painter().text(
        rect.left_top() + Vec2::new(PAD, 3.0),
        Align2::LEFT_TOP,
        name,
        FontId::proportional(11.0),
        text,
    );
    let below = Rect::from_min_max(rect.left_top() + Vec2::new(0.0, 14.0), rect.right_bottom());
    paint_swatch(ui, below, swatch, text);
    response.on_hover_text(name).clicked()
}

/// A swatch in `rect`, its white mark tinted `color`.
fn paint_swatch(ui: &Ui, rect: Rect, swatch: Option<&TextureHandle>, color: Color32) {
    if let Some(texture) = swatch {
        let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        ui.painter().image(texture.id(), rect, uv, color);
    }
}
