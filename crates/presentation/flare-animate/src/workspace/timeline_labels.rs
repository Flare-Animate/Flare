//! The timeline's name column and ruler. Both are drawn over the frames and stay in view as the
//! frames scroll under them: the names at the left whatever frame is shown, the ruler at the top
//! however far the layers are scrolled.

use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, Vec2};
use flare_editor::timeline_view;

use super::timeline::{FOLDER_INDENT, Layout, ROW};
use super::timeline_cells;
use super::timeline_rows::{active_row, stripe};
use super::*;

impl Workspace {
    /// Paints the name column: each row's folder arrow, name, and visibility, lock and outline
    /// toggles, on the panel's own color so frames scrolled under it do not show through.
    pub(super) fn timeline_labels(
        &self,
        ui: &egui::Ui,
        painter: &Painter,
        l: &Layout,
        rows: &[(usize, usize)],
    ) {
        let column = Rect::from_min_max(
            Pos2::new(l.labels_left, l.rect.min.y),
            Pos2::new(l.label_edge, l.rect.max.y),
        );
        painter.rect_filled(column, 0.0, flare_ui::theme::panel());
        painter.vline(
            l.label_edge,
            column.y_range(),
            Stroke::new(1.0, flare_ui::theme::border()),
        );
        let Some(scene) = self.document().scenes().first() else {
            return;
        };
        let active = self.active_layer_id();
        let (ink, dim) = (flare_ui::theme::text(), flare_ui::theme::text_disabled());
        let (first, count) = visible_rows(l.rect.min.y, ROW, painter.clip_rect());
        let all_layers = scene.timeline().layers();
        for (i, (layer_index, depth)) in rows.iter().copied().enumerate().skip(first).take(count) {
            let Some(layer) = all_layers.get(layer_index) else {
                continue;
            };
            let y = l.row_top(i);
            let band = Rect::from_min_max(
                Pos2::new(l.labels_left, y),
                Pos2::new(l.label_edge, y + ROW),
            );
            if i % 2 == 1 {
                painter.rect_filled(band, 0.0, stripe());
            }
            if Some(layer.id()) == active {
                painter.rect_filled(band, 0.0, active_row());
            }
            let mut text_x = l.labels_left + 6.0 + depth as f32 * FOLDER_INDENT;
            if layer.is_folder() {
                let shut = self.collapsed_folders.contains(&layer.id());
                let arrow = if shut {
                    "chevron-right"
                } else {
                    "chevron-down"
                };
                flare_ui::icons::image(arrow, 12.0, dim).paint_at(
                    ui,
                    Rect::from_min_size(Pos2::new(text_x, y + 4.0), Vec2::splat(12.0)),
                );
                flare_ui::icons::image("folder", 14.0, ink).paint_at(
                    ui,
                    Rect::from_min_size(Pos2::new(text_x + 14.0, y + 3.0), Vec2::splat(14.0)),
                );
                text_x += 32.0;
            }
            let name = if layer.is_mask() {
                format!("Mask: {}", layer.name())
            } else if layer.is_guide() {
                format!("Guide: {}", layer.name())
            } else {
                layer.name().to_owned()
            };
            // The name stops short of the eye, lock and outline toggles.
            let name_right = l.label_edge - 58.0;
            let galley = ui.fonts_mut(|f| {
                timeline_cells::layer_name(
                    f,
                    name,
                    name_right - text_x,
                    if layer.is_visible() { ink } else { dim },
                )
            });
            painter.galley(Pos2::new(text_x, y + 4.0), galley, ink);
            let icon_at = |x: f32, name: &str, tint: Color32| {
                flare_ui::icons::image(name, 14.0, tint).paint_at(
                    ui,
                    Rect::from_min_size(Pos2::new(x, y + 4.0), Vec2::splat(14.0)),
                );
            };
            let (eye, eye_tint) = if layer.is_visible() {
                ("eye", ink)
            } else {
                ("eye-off", dim)
            };
            icon_at(l.label_edge - 52.0, eye, eye_tint);
            let (lock, lock_tint) = if layer.is_locked() {
                ("lock", ink)
            } else {
                ("lock-open", dim)
            };
            icon_at(l.label_edge - 34.0, lock, lock_tint);
            let chip = layer.outline_or_default(layer_index);
            painter.rect_filled(
                Rect::from_min_size(
                    Pos2::new(l.label_edge - 14.0, y + ROW / 2.0 - 5.0),
                    Vec2::splat(10.0),
                ),
                2.0,
                Color32::from_rgb(chip.r, chip.g, chip.b),
            );
        }
    }

    /// Paints the ruler: a band with a tick on every frame and a number every few, over the
    /// rows scrolled up under it. Only the frames in view are ticked.
    pub(super) fn timeline_ruler(&self, painter: &Painter, l: &Layout, shown: u32) {
        let view = painter.clip_rect();
        let band = Rect::from_min_max(
            Pos2::new(l.rect.min.x, l.ruler_top),
            Pos2::new(l.rect.max.x, l.ruler_top + ROW),
        );
        painter.rect_filled(band, 0.0, flare_ui::theme::panel_raised());
        painter.hline(
            band.x_range(),
            band.bottom(),
            Stroke::new(1.0, flare_ui::theme::border()),
        );
        self.onion_markers(painter, l, band);
        self.loop_bar(painter, l, band);
        // Frame 1, then every fifth (or further apart when the frames are narrow), as Animate
        // numbers them; each fifth frame has a taller tick so it can be counted to.
        let step = timeline_view::ruler_step(l.per_frame);
        let first = l.frame_at(l.label_edge).floor().max(0.0) as u32;
        for f in first..shown {
            let x = l.frame_x(f as f32);
            if x > view.max.x {
                break;
            }
            if x < l.label_edge {
                continue;
            }
            let major = timeline_view::is_numbered(f, step);
            let (tick, color) = if major {
                (7.0, flare_ui::theme::text_dim())
            } else if timeline_view::is_fifth(f) {
                (5.0, flare_ui::theme::text_dim())
            } else {
                (3.0, flare_ui::theme::text_disabled())
            };
            painter.vline(
                x,
                (band.bottom() - tick)..=band.bottom(),
                Stroke::new(1.0, color),
            );
            if major {
                painter.text(
                    Pos2::new(x + 3.0, band.top() + 2.0),
                    Align2::LEFT_TOP,
                    (f + 1).to_string(),
                    FontId::monospace(10.0),
                    flare_ui::theme::text_dim(),
                );
            }
        }
    }

    /// Onion skin's range on the ruler, as Animate draws it: a band under the frame numbers from
    /// the earliest ghost to the latest, with a handle at each end in that side's color, to drag.
    fn onion_markers(&self, painter: &Painter, l: &Layout, band: Rect) {
        let onion = self.onion;
        if !onion.enabled {
            return;
        }
        let (start, end) = l.onion_markers(self.frame, onion.before, onion.after);
        let strip = Rect::from_min_max(
            Pos2::new(start.max(l.label_edge), band.bottom() - 5.0),
            Pos2::new(end, band.bottom() - 1.0),
        );
        if strip.width() > 0.0 {
            painter.rect_filled(strip, 1.0, flare_ui::theme::accent().gamma_multiply(0.35));
        }
        let (early, late) = match onion.tints() {
            Some([before, after]) => (before, after),
            None => {
                let dim = flare_ui::theme::text_dim();
                let [r, g, b, _] = dim.to_array();
                let same = flare_core::Rgba8::opaque(r, g, b);
                (same, same)
            }
        };
        for (x, color) in [(start, early), (end, late)] {
            if x < l.label_edge {
                continue;
            }
            let handle =
                Rect::from_center_size(Pos2::new(x, band.bottom() - 6.0), Vec2::new(4.0, 11.0));
            painter.rect_filled(handle, 1.5, Color32::from_rgb(color.r, color.g, color.b));
        }
    }
}

#[path = "timeline_loop.rs"]
mod timeline_loop;
pub(super) use timeline_loop::Looping;

#[path = "timeline_multi.rs"]
mod timeline_multi;
pub(super) use timeline_multi::MultiFrames;
