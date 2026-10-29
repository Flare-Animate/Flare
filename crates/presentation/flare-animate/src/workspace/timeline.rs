//! The timeline panel.

use crate::timeline_view;
use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use flare_document::{Content, Layer};

use super::*;

impl Workspace {
    pub(super) fn timeline(&mut self, ui: &mut egui::Ui) {
        self.timeline_toolbar(ui);
        ui.separator();

        let available = ui.available_width();
        if ui.rect_contains_pointer(ui.max_rect()) {
            let zoom = ui.input(|i| i.zoom_delta());
            if zoom != 1.0 {
                self.zoom_timeline(zoom);
            }
        }
        egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            let frames = self.scene_frames();
            let layer_count = self
                .document()
                .scenes()
                .first()
                .map_or(0, |s| s.timeline().layers().len());
            let row = 22.0;
            let label_w = 150.0;
            let height = row * (layer_count as f32 + 1.0);
            let per_frame = timeline_view::frame_width(
                (available - label_w).max(0.0),
                u64::from(frames),
                self.timeline_zoom,
            );
            let (rect, response) = ui.allocate_exact_size(
                Vec2::new(label_w + per_frame * frames as f32, height),
                Sense::click_and_drag(),
            );
            let painter = ui.painter_at(rect);
            let track = Rect::from_min_max(Pos2::new(rect.min.x + label_w, rect.min.y), rect.max);

            let frame_at = |x: f32| (x - track.min.x) / per_frame;
            let row_layer = |y: f32| {
                let index = ((y - rect.min.y) / row).floor() as i64 - 1;
                usize::try_from(index).ok().and_then(|i| {
                    let layers = self.document().scenes().first()?.timeline().layers();
                    layers.get(i).map(Layer::id)
                })
            };
            if response.drag_started()
                && let Some(p) = ui.input(|i| i.pointer.press_origin())
                && p.x >= track.min.x
                && let Some(id) = row_layer(p.y)
                && self.begin_span_drag(id, frame_at(p.x), 6.0 / per_frame)
            {
                self.select_layer(id);
            }
            if self.span_drag.is_some() {
                if response.dragged()
                    && let Some(p) = response.interact_pointer_pos()
                {
                    self.update_span_drag(frame_at(p.x).floor().max(0.0) as u32);
                }
                if response.drag_stopped() {
                    self.end_span_drag();
                }
            } else if (response.clicked() || response.dragged())
                && let Some(p) = response.interact_pointer_pos()
            {
                let in_track = p.x >= track.min.x;
                let f = ((p.x - track.min.x) / per_frame).floor().max(0.0) as u32;
                let extend = response.clicked() && in_track && ui.input(|i| i.modifiers.shift);
                // Clicking a row (below the ruler) makes that layer the active one.
                let index = ((p.y - rect.min.y) / row).floor() as i64 - 1;
                let picked = usize::try_from(index).ok().and_then(|i| {
                    let layers = self.document().scenes().first()?.timeline().layers();
                    layers.get(i).map(Layer::id)
                });
                if extend && let Some(id) = picked {
                    // Shift-click selects the frames between the playhead (or the range's own
                    // start) and the clicked one, without moving the playhead.
                    let anchor = self
                        .frame_range
                        .filter(|r| r.layer == id)
                        .map_or(self.frame, |r| r.from);
                    self.select_frames(id, anchor, f);
                } else if in_track {
                    self.set_frame(f);
                    if response.clicked() {
                        self.frame_range = None;
                    }
                }
                if let Some(id) = picked {
                    // The two small toggles at the right edge of the label column.
                    let eye = p.x >= track.min.x - 54.0 && p.x < track.min.x - 36.0;
                    let lock = p.x >= track.min.x - 36.0 && p.x < track.min.x - 18.0;
                    if response.clicked() && eye {
                        self.toggle_layer_visible(id);
                    } else if response.clicked() && lock {
                        self.toggle_layer_locked(id);
                    } else {
                        self.select_layer(id);
                    }
                }
            }
            let active = self.active_layer_id();

            // The ruler: a band with a tick on every frame and a number every few.
            let ink = flare_ui::theme::TEXT;
            let band = Rect::from_min_max(rect.min, Pos2::new(rect.max.x, rect.min.y + row));
            painter.rect_filled(band, 0.0, flare_ui::theme::PANEL_RAISED);
            painter.hline(
                rect.x_range(),
                band.bottom(),
                Stroke::new(1.0, flare_ui::theme::BORDER),
            );
            let step = ((40.0 / per_frame).ceil() as u32).max(1);
            for f in 0..frames {
                let x = track.min.x + f as f32 * per_frame;
                if x > painter.clip_rect().max.x {
                    break;
                }
                let major = f % step == 0;
                painter.vline(
                    x,
                    (band.bottom() - if major { 7.0 } else { 3.0 })..=band.bottom(),
                    Stroke::new(1.0, flare_ui::theme::TEXT_DISABLED),
                );
                if major {
                    painter.text(
                        Pos2::new(x + 3.0, band.top() + 2.0),
                        Align2::LEFT_TOP,
                        (f + 1).to_string(),
                        FontId::monospace(10.0),
                        flare_ui::theme::TEXT_DIM,
                    );
                }
            }
            if let Some(scene) = self.document().scenes().first() {
                // Only the rows in view are painted; with thousands of layers drawing all of
                // them every frame dominated the cost of an idle frame.
                let (first, count) = visible_rows(rect.min.y, row, painter.clip_rect());
                for (i, layer) in scene
                    .timeline()
                    .layers()
                    .iter()
                    .enumerate()
                    .skip(first)
                    .take(count)
                {
                    let y = rect.min.y + row * (i as f32 + 1.0);
                    if i % 2 == 1 {
                        painter.rect_filled(
                            Rect::from_min_max(
                                Pos2::new(rect.min.x, y),
                                Pos2::new(rect.max.x, y + row),
                            ),
                            0.0,
                            Color32::from_rgba_unmultiplied(255, 255, 255, 6),
                        );
                    }
                    // A line every five frames makes the cells easy to count.
                    for f in (0..frames).step_by(5) {
                        let x = track.min.x + f as f32 * per_frame;
                        if x > painter.clip_rect().max.x {
                            break;
                        }
                        painter.vline(
                            x,
                            y..=y + row,
                            Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 14)),
                        );
                    }
                    if Some(layer.id()) == active {
                        let band = Rect::from_min_max(
                            Pos2::new(rect.min.x, y),
                            Pos2::new(rect.max.x, y + row),
                        );
                        painter.rect_filled(
                            band,
                            0.0,
                            Color32::from_rgba_unmultiplied(80, 140, 255, 40),
                        );
                    }
                    let dim = flare_ui::theme::TEXT_DISABLED;
                    painter.text(
                        Pos2::new(rect.min.x + 6.0, y + 4.0),
                        Align2::LEFT_TOP,
                        if layer.is_mask() {
                            format!("Mask: {}", layer.name())
                        } else {
                            layer.name().to_owned()
                        },
                        FontId::proportional(12.0),
                        if layer.is_visible() { ink } else { dim },
                    );
                    // Visibility, lock and outline columns, as icons.
                    let icon_at = |x: f32, name: &str, tint: Color32| {
                        flare_ui::icons::image(name, 14.0, tint).paint_at(
                            ui,
                            Rect::from_min_size(Pos2::new(x, y + 4.0), Vec2::splat(14.0)),
                        );
                    };
                    icon_at(
                        track.min.x - 52.0,
                        if layer.is_visible() { "eye" } else { "eye-off" },
                        if layer.is_visible() {
                            flare_ui::theme::TEXT
                        } else {
                            dim
                        },
                    );
                    icon_at(
                        track.min.x - 34.0,
                        if layer.is_locked() {
                            "lock"
                        } else {
                            "lock-open"
                        },
                        if layer.is_locked() {
                            flare_ui::theme::TEXT
                        } else {
                            dim
                        },
                    );
                    icon_at(track.min.x - 16.0, "outline-square", dim);
                    for span in layer.spans() {
                        let x0 = track.min.x + span.start() as f32 * per_frame;
                        let x1 = track.min.x + span.end() as f32 * per_frame;
                        let color = match span.content() {
                            Content::Empty => Color32::from_gray(90),
                            Content::Shape(_) => Color32::from_rgb(70, 130, 180),
                            Content::Instance(_) => Color32::from_rgb(120, 170, 90),
                        };
                        let bar = Rect::from_min_max(
                            Pos2::new(x0, y + 2.0),
                            Pos2::new(x1.max(x0 + 1.0), y + row - 2.0),
                        );
                        let color = if layer.is_visible() {
                            color
                        } else {
                            color.gamma_multiply(0.35)
                        };
                        painter.rect_filled(bar, 2.0, color);
                    }
                    for cue in layer.cues() {
                        let Some(sound) = self.session.document().sound(cue.sound) else {
                            continue;
                        };
                        let fps = f64::from(self.document().frame_rate().numerator())
                            / f64::from(self.document().frame_rate().denominator());
                        let length = (sound.seconds() * fps).ceil().max(1.0) as f32;
                        let x0 = track.min.x + cue.start as f32 * per_frame;
                        let bar = Rect::from_min_max(
                            Pos2::new(x0, y + 2.0),
                            Pos2::new(x0 + length * per_frame, y + row - 2.0),
                        );
                        painter.rect_filled(bar, 2.0, Color32::from_rgb(214, 140, 60));
                        painter.text(
                            bar.left_center() + Vec2::new(3.0, 0.0),
                            Align2::LEFT_CENTER,
                            sound.name(),
                            FontId::proportional(10.0),
                            Color32::BLACK,
                        );
                    }
                    if let Some(range) = self.frame_range.filter(|r| r.layer == layer.id()) {
                        let x0 = track.min.x + range.from as f32 * per_frame;
                        let x1 = track.min.x + (range.to as f32 + 1.0) * per_frame;
                        let marked = Rect::from_min_max(
                            Pos2::new(x0, y + 1.0),
                            Pos2::new(x1, y + row - 1.0),
                        );
                        painter.rect_filled(
                            marked,
                            2.0,
                            Color32::from_rgba_unmultiplied(255, 220, 80, 70),
                        );
                        painter.rect_stroke(
                            marked,
                            2.0,
                            Stroke::new(1.0, Color32::from_rgb(255, 200, 40)),
                            StrokeKind::Inside,
                        );
                    }
                    if let Some((dragged, start, duration)) = self.span_drag_preview()
                        && dragged == layer.id()
                    {
                        let x0 = track.min.x + start as f32 * per_frame;
                        let x1 = x0 + duration as f32 * per_frame;
                        let ghost = Rect::from_min_max(
                            Pos2::new(x0, y + 2.0),
                            Pos2::new(x1, y + row - 2.0),
                        );
                        painter.rect_stroke(
                            ghost,
                            2.0,
                            Stroke::new(2.0, Color32::from_rgb(40, 120, 255)),
                            StrokeKind::Inside,
                        );
                    }
                }
            }
            let head = track.min.x + (self.frame as f32 + 0.5) * per_frame;
            painter.line_segment(
                [
                    Pos2::new(head, rect.min.y + 8.0),
                    Pos2::new(head, rect.max.y),
                ],
                Stroke::new(1.5, Color32::from_rgb(232, 69, 60)),
            );
            flare_ui::icons::image("playhead", 16.0, Color32::from_rgb(232, 69, 60)).paint_at(
                ui,
                Rect::from_center_size(Pos2::new(head, rect.min.y + 8.0), Vec2::splat(16.0)),
            );
        });
    }
}
