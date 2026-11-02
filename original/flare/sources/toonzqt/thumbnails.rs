//! View-only, bounded timeline previews from the production evaluator/renderer.
use super::*;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrameDisplay {
    #[default]
    Cells,
    Preview,
    Context,
}
#[derive(Default)]
pub(super) struct ThumbnailCache {
    identity: Option<(u64, u64, usize, Option<u32>)>,
    textures: BTreeMap<(usize, u32, bool), egui::TextureHandle>,
}
impl Studio {
    pub(super) fn frame_display_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button(self.i18n.text("field-frame-display"), |ui| {
            for (mode, key) in [
                (FrameDisplay::Cells, "frames-cells"),
                (FrameDisplay::Preview, "frames-preview"),
                (FrameDisplay::Context, "frames-context"),
            ] {
                if widgets::selectable(ui, self.prefs.frame_display == mode, self.i18n.text(key))
                    .clicked()
                {
                    self.prefs.frame_display = mode;
                    self.reset_timeline_gesture();
                    ui.close();
                }
            }
        });
    }
    pub(super) fn thumbnail_pixels(
        &self,
        layer: usize,
        frame: u32,
        context: bool,
    ) -> Option<tiny_skia::Pixmap> {
        let items: Vec<_> = self
            .editor
            .evaluate_current(frame)
            .into_iter()
            .filter(|it| it.src.is_some_and(|(l, _)| l == layer))
            .collect();
        let (lo, hi) = if context {
            (
                pt(0.0, 0.0),
                pt(self.editor.doc.width, self.editor.doc.height),
            )
        } else {
            let bounds = items
                .iter()
                .filter_map(|it| it.path.transformed(&it.matrix).bounds())
                .reduce(|a, b| {
                    (
                        pt(a.0.x.min(b.0.x), a.0.y.min(b.0.y)),
                        pt(a.1.x.max(b.1.x), a.1.y.max(b.1.y)),
                    )
                });
            bounds.unwrap_or((
                pt(0.0, 0.0),
                pt(self.editor.doc.width, self.editor.doc.height),
            ))
        };
        let width = (hi.x - lo.x).max(1.0);
        let height = (hi.y - lo.y).max(1.0);
        let scale = (124.0 / width).min(84.0 / height);
        if !scale.is_finite() {
            return None;
        }
        let view = Matrix::translate(
            (128.0 - width * scale) / 2.0 - lo.x * scale,
            (88.0 - height * scale) / 2.0 - lo.y * scale,
        )
        .concat(&Matrix::scale(scale, scale));
        let mut pix = tiny_skia::Pixmap::new(128, 88)?;
        let bg = self.editor.doc.background;
        pix.fill(tiny_skia::Color::from_rgba8(bg.r, bg.g, bg.b, 255));
        ac_render::draw_items(
            &mut pix.as_mut(),
            &items,
            &ac_render::RenderOpts {
                view,
                ..Default::default()
            },
        );
        Some(pix)
    }
    pub(super) fn frame_thumbnail(
        &mut self,
        ctx: &egui::Context,
        layer: usize,
        frame: u32,
    ) -> Option<egui::TextureId> {
        let identity = (
            self.session,
            self.editor.revision,
            self.editor.scene,
            self.editor.editing,
        );
        let cache = &mut self.timeline_state.thumbnails;
        if cache.identity != Some(identity) {
            cache.textures.clear();
            cache.identity = Some(identity);
        }
        let key = (
            layer,
            frame,
            self.prefs.frame_display == FrameDisplay::Context,
        );
        if let Some(texture) = cache.textures.get(&key) {
            return Some(texture.id());
        }
        let pixels = self.thumbnail_pixels(layer, frame, key.2)?;
        let cache = &mut self.timeline_state.thumbnails;
        if cache.textures.len() >= 512 {
            cache.textures.clear();
        }
        let texture = ctx.load_texture(
            "studio.frame.preview",
            egui::ColorImage::from_rgba_unmultiplied(
                [128, 88],
                &ac_render::unpremultiplied(&pixels),
            ),
            egui::TextureOptions::LINEAR,
        );
        let id = texture.id();
        cache.textures.insert(key, texture);
        Some(id)
    }
}
