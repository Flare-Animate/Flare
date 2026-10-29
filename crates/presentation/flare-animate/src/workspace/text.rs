//! The text tool.

use egui::{Key, Pos2, Rect};
use flare_core::{Rgba8, ShapeId};
use flare_document::Shape;
use flare_runtime::TextJob;

use super::*;

impl Workspace {
    // ---- text -------------------------------------------------------------------------

    /// The small editor shown where the user clicked with the text tool. Ctrl/Cmd+Enter or the
    /// Place button hands the text over; Escape or Cancel drops it.
    pub(super) fn text_editor(&mut self, ctx: &egui::Context, image_rect: Rect, scale: f32) {
        let id = self.text_editor_id;
        let Some(edit) = self.text_edit.as_mut() else {
            return;
        };
        let at = image_rect.min + edit.at.to_vec2() * scale;
        let (mut place, mut cancel) = (false, false);
        egui::Area::new(id)
            .fixed_pos(at)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    let field = ui.add(
                        egui::TextEdit::multiline(&mut edit.buffer)
                            .desired_rows(2)
                            .desired_width(240.0)
                            .hint_text("Type text"),
                    );
                    if !edit.focused {
                        field.request_focus();
                        edit.focused = true;
                    }
                    ui.horizontal(|ui| {
                        place |= ui.button("Place").clicked();
                        cancel |= ui.button("Cancel").clicked();
                    });
                    ui.input(|i| {
                        place |= i.modifiers.command && i.key_pressed(Key::Enter);
                        cancel |= i.key_pressed(Key::Escape);
                    });
                });
            });
        if place {
            self.place_text();
        } else if cancel {
            self.text_edit = None;
        }
    }

    /// Starts the text being typed becoming shapes. The font work happens off the UI thread; the
    /// shapes appear when it is done.
    pub fn place_text(&mut self) {
        let Some(edit) = self.text_edit.take() else {
            return;
        };
        if edit.buffer.trim().is_empty() {
            return;
        }
        let id = self.background.next_id();
        self.text_pending = Some((id, edit.at));
        self.background.submit_text(TextJob {
            id,
            text: edit.buffer,
            style: flare_text::TextStyle {
                family: self.text_family.clone(),
                size: self.text_size,
            },
        });
        self.settle_inline();
    }

    /// Makes the shape for finished text outlines, with its corner at `at`, on a new layer.
    pub(super) fn place_outline(&mut self, outline: flare_text::TextOutline, at: Pos2) {
        let shift = |p: &flare_core::DocumentPoint| {
            flare_core::DocumentPoint::new(
                p.as_vec().x + f64::from(at.x),
                p.as_vec().y + f64::from(at.y),
            )
            .ok()
        };
        let moved: Option<Vec<flare_document::PathSegment>> = outline
            .path
            .iter()
            .map(|segment| {
                use flare_document::PathSegment as S;
                Some(match segment {
                    S::MoveTo(p) => S::MoveTo(shift(p)?),
                    S::LineTo(p) => S::LineTo(shift(p)?),
                    S::CubicTo(a, b, c) => S::CubicTo(shift(a)?, shift(b)?, shift(c)?),
                    S::Close => S::Close,
                })
            })
            .collect();
        let fill = {
            let c = self.fill.to_array();
            Rgba8::new(c[0], c[1], c[2], c[3])
        };
        let Some(path) = moved else {
            self.report("Cannot place text: it would leave the drawable range");
            return;
        };
        match Shape::new(ShapeId::new(), path, fill) {
            Ok(shape) => self.insert_shape_layer(shape, "Text"),
            Err(error) => self.report(format!("Cannot place text: {error}")),
        }
    }
}
