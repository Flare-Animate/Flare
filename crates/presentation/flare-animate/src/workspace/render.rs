//! Getting the stage onto a texture.

use egui::{Pos2, Rect};
use flare_runtime::RenderJob;

use super::*;

impl Workspace {
    /// The part of the stage to render and at what size: what is on screen, at the resolution it
    /// is shown, as of the last layout. Before any layout it is the whole stage.
    pub(super) fn desired_texture_view(
        &self,
        pixels_per_point: f32,
        max_side: u32,
    ) -> Option<TextureView> {
        let extent = self.stage_extent();
        let (region, density) = match self.stage_view.visible_region(self.stage_panel, extent) {
            Some(region) => {
                let scale = self.stage_view.scale(self.stage_panel, extent);
                (region, (scale * pixels_per_point).max(f32::EPSILON))
            }
            None if self.stage_panel == Rect::NOTHING => {
                (Rect::from_min_size(Pos2::ZERO, extent), 1.0)
            }
            None => return None,
        };
        let side = |length: f32| ((length * density).ceil() as u32).clamp(1, max_side.max(1));
        Some(TextureView {
            region,
            size: [side(region.width()), side(region.height())],
        })
    }

    /// Keeps the stage texture matching what should be on screen. A picture that is already
    /// cached is shown at once; otherwise the frame is drawn on the render worker and the
    /// previous picture stays up (drawn where it belongs) until the new one arrives.
    pub(super) fn ensure_stage_texture(&mut self, ctx: &egui::Context) {
        self.install_rendered(ctx);
        let key = (self.context_serial, self.active().revision(), self.frame);
        let onion = self.effective_onion();
        let Some(view) = self.desired_texture_view(ctx.pixels_per_point(), texture_side_limit(ctx))
        else {
            // Nothing of the stage is on screen: keep the last texture, there is nothing to draw.
            return;
        };
        if self
            .texture
            .as_ref()
            .is_some_and(|t| t.key == key && t.onion == onion && t.view == view)
        {
            return;
        }
        if self
            .pending_render
            .as_ref()
            .is_some_and(|p| p.key == key && p.onion == onion && p.view == view)
        {
            return; // already being drawn
        }
        let Some(scene) = self.document().scenes().first().map(|s| s.id()) else {
            self.texture = None;
            return;
        };
        let ghost_frames = if onion.enabled {
            onion_ghosts(self.frame, self.scene_frames(), onion.before, onion.after)
        } else {
            Vec::new()
        };
        let (context, revision) = (key.0, key.1);
        // What the frame draws, to see whether the same picture is already around. Evaluating a
        // frame is cheap next to drawing it.
        let signature = {
            use std::hash::{Hash, Hasher};
            let document = self.document();
            let current = match flare_eval::evaluate_frame(document, scene, self.frame) {
                Ok(frame) => frame,
                Err(e) => {
                    self.report(format!("Cannot render frame: {e}"));
                    return;
                }
            };
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            current.signature().hash(&mut hasher);
            // Outlines and faded pictures of the same frames are different images.
            onion.outlines.hash(&mut hasher);
            self.display.outline_mode.hash(&mut hasher);
            for (frame, opacity) in &ghost_frames {
                if let Ok(ghost) = flare_eval::evaluate_frame(document, scene, *frame) {
                    ghost.signature().hash(&mut hasher);
                    opacity.to_bits().hash(&mut hasher);
                }
            }
            hasher.finish()
        };
        let hit = self.frame_cache.iter().position(|c| {
            c.context == context
                && c.revision == revision
                && c.signature == signature
                && c.view == view
        });
        if let Some(index) = hit {
            // Most recently used goes last.
            let entry = self.frame_cache.remove(index);
            let handle = entry.handle.clone();
            self.frame_cache.push(entry);
            self.pending_render = None;
            self.texture = Some(StageTexture {
                handle,
                view,
                key,
                onion,
            });
            return;
        }
        let id = self.background.next_id();
        self.pending_render = Some(PendingRender {
            id,
            key,
            onion,
            view,
            signature,
        });
        let stage = self.document().stage();
        self.background.submit_render(RenderJob {
            id,
            document: self.active().snapshot(),
            scene,
            frame: self.frame,
            outlines: onion.outlines,
            outline_mode: self.display.outline_mode,
            ghosts: ghost_frames,
            viewport: flare_render::Viewport {
                x: f64::from(view.region.min.x),
                y: f64::from(view.region.min.y),
                width: f64::from(view.region.width()),
                height: f64::from(view.region.height()),
            },
            size: view.size,
            background: stage.background(),
        });
        // Inline, the picture is ready already; with workers it arrives on a later pass.
        self.settle_inline();
        self.install_rendered(ctx);
    }

    /// Turns finished drawings into textures. Only the frame asked for most recently is kept:
    /// anything else was overtaken while it was being drawn.
    pub(super) fn install_rendered(&mut self, ctx: &egui::Context) {
        for (id, result) in std::mem::take(&mut self.render_inbox) {
            let Some(pending) = self.pending_render.take_if(|p| p.id == id) else {
                continue;
            };
            let color = match result {
                Ok(color) => color,
                Err(message) => {
                    self.report(format!("Cannot render frame: {message}"));
                    continue;
                }
            };
            self.frames_rendered += 1;
            let handle = ctx.load_texture("stage", color, egui::TextureOptions::LINEAR);
            let (context, revision) = (pending.key.0, pending.key.1);
            // Pictures of other document states can never be shown again.
            self.frame_cache
                .retain(|c| c.context == context && c.revision == revision);
            self.frame_cache.push(CachedFrame {
                context,
                revision,
                signature: pending.signature,
                view: pending.view,
                handle: handle.clone(),
            });
            self.trim_frame_cache();
            self.texture = Some(StageTexture {
                handle,
                view: pending.view,
                key: pending.key,
                onion: pending.onion,
            });
        }
    }

    /// Drops the least recently used frames until the cache fits its limits. The newest frame
    /// is always kept, so a single large frame is still shown.
    pub(super) fn trim_frame_cache(&mut self) {
        while self.frame_cache.len() > 1
            && (self.frame_cache.len() > FRAME_CACHE_ENTRIES
                || self
                    .frame_cache
                    .iter()
                    .map(CachedFrame::bytes)
                    .sum::<usize>()
                    > FRAME_CACHE_BYTES)
        {
            self.frame_cache.remove(0);
        }
    }
}
