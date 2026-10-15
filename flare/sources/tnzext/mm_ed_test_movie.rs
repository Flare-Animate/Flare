//! Control > Test (Ctrl+Enter): plays the whole movie, looping, in its own window.

use eframe::egui::{self, Color32, ColorImage, Frame, Rect, TextureHandle, TextureOptions, ViewportBuilder, ViewportId, pos2, vec2};
use minnamate_core::doc::{Document, Timeline};
use minnamate_core::render::render;

use crate::app::Editor;

pub struct TestMovie {
    start: f64,
    texture: Option<(TextureHandle, (u32, u32))>,
}

impl TestMovie {
    pub fn new(now: f64) -> Self {
        TestMovie { start: now, texture: None }
    }
}

pub fn show(ctx: &egui::Context, ed: &mut Editor) {
    let Editor { doc, test, title, path, .. } = ed;
    let Some(movie) = test else { return };
    let name = path.as_ref().and_then(|p| p.file_stem()).map_or_else(|| title.clone(), |s| s.to_string_lossy().into_owned());
    let builder = ViewportBuilder::default().with_title(format!("{name}.swf")).with_inner_size([doc.width, doc.height]);
    let mut close = false;
    ctx.show_viewport_immediate(ViewportId::from_hash_of("test-movie"), builder, |ui, _| {
        close = ui.input(|i| i.viewport().close_requested());
        let [r, g, b, _] = doc.background.map(|c| (c * 255.0) as u8);
        egui::CentralPanel::default().frame(Frame::NONE.fill(Color32::from_rgb(r, g, b))).show(ui, |ui| {
            let total: u32 = doc.scenes.iter().map(Timeline::frame_count).sum::<u32>().max(1);
            let fps = doc.frame_rate.max(0.01) as f64;
            let frame = ((ui.input(|i| i.time) - movie.start) * fps) as u32 % total;
            // Scale to fit the window, keeping the aspect ratio (SWF "show all").
            let view = ui.max_rect();
            let scale = (view.size() / vec2(doc.width, doc.height)).min_elem().max(0.01);
            if let Some(tex) = movie_texture(ui.ctx(), doc, movie, frame, scale * ui.ctx().pixels_per_point()) {
                let rect = Rect::from_center_size(view.center(), vec2(doc.width, doc.height) * scale);
                ui.painter().image(tex, rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            ui.ctx().request_repaint_after_secs(1.0 / fps as f32);
        });
    });
    if close {
        *test = None;
    }
}

// ponytail: movie clips show their first frame here, as on the stage; per-instance
// clip playback (or embedding Ruffle) is what makes this a true test player.
fn movie_texture(ctx: &egui::Context, doc: &Document, movie: &mut TestMovie, frame: u32, scale: f32) -> Option<egui::TextureId> {
    let scale = scale.min(4096.0 / doc.width.max(doc.height).max(1.0));
    let key = (frame, scale.to_bits());
    if movie.texture.as_ref().map(|(_, k)| *k) != Some(key) {
        let (pixmap, _) = render(doc, frame, scale).ok()?;
        let image = ColorImage::from_rgba_premultiplied([pixmap.width() as usize, pixmap.height() as usize], pixmap.data());
        match &mut movie.texture {
            Some((tex, k)) => {
                tex.set(image, TextureOptions::LINEAR);
                *k = key;
            }
            None => movie.texture = Some((ctx.load_texture("test-movie", image, TextureOptions::LINEAR), key)),
        }
    }
    movie.texture.as_ref().map(|(tex, _)| tex.id())
}
