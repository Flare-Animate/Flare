//! Text while it is typed: what the stage shows in place of the stored letters, and the caret.
//!
//! The editor's own field draws nothing. The font worker sets the words as they are typed, and the
//! stage draws that in the place of the text object being edited (or on top of the scene for new
//! text), so the old letters and the new ones are never up together and the letters are the ones
//! the text will have. The caret and the box around the text come from the same answer.

use egui::{Pos2, Rect, Stroke, StrokeKind, Vec2};
use flare_core::{DocumentPoint, Rgba8, ShapeId};
use flare_document::Shape;
use flare_runtime::{TextJob, TextPreviewRender};
use flare_text::{Caret, TextStyle};
use flare_tools::ops;

use crate::workspace::*;

/// The latest answer: the letters so far (none while there are none) and where the caret is.
pub(in crate::workspace) struct TextPreview {
    pub(in crate::workspace) shape: Option<Shape>,
    pub(in crate::workspace) caret: Caret,
}

/// What a preview was asked for, so it is asked again only when something changed.
#[derive(Clone, PartialEq)]
pub(in crate::workspace) struct PreviewAsk {
    text: String,
    cursor: usize,
    family: String,
    size: u32,
    bold: bool,
    italic: bool,
    origin: (u32, u32),
    fill: Rgba8,
}

impl Workspace {
    /// Asks the font worker for the text as typed so far, if it is not what was last asked for.
    /// `cursor` is the editor's cursor, in characters.
    pub(in crate::workspace) fn update_text_preview(&mut self, cursor: usize) {
        let Some(edit) = self.text.edit.as_ref() else {
            return;
        };
        let Ok(origin) = DocumentPoint::new(f64::from(edit.at.x), f64::from(edit.at.y)) else {
            return;
        };
        let text = edit.buffer.clone();
        let byte = text
            .char_indices()
            .nth(cursor)
            .map_or(text.len(), |(index, _)| index);
        let shape = edit
            .editing
            .and_then(|at| self.text_at(at))
            .map_or(self.text.preview_shape, |(shape, _)| shape.id());
        let fill = {
            let c = self.text_ink().to_array();
            Rgba8::new(c[0], c[1], c[2], c[3])
        };
        let ask = PreviewAsk {
            text: text.clone(),
            cursor: byte,
            family: self.text.family.clone(),
            size: self.text.size.to_bits(),
            bold: self.text.bold,
            italic: self.text.italic,
            origin: (edit.at.x.to_bits(), edit.at.y.to_bits()),
            fill,
        };
        if self.text.preview_asked.as_ref() == Some(&ask) {
            return;
        }
        self.text.preview_asked = Some(ask);
        let id = self.background.next_id();
        self.text.preview_pending = Some(id);
        self.background.submit_text(TextJob::Preview {
            id,
            text,
            style: TextStyle {
                family: self.text.family.clone(),
                size: self.text.size,
                bold: self.text.bold,
                italic: self.text.italic,
            },
            origin,
            cursor: byte,
            shape,
            fill,
        });
        self.settle_inline();
    }

    /// Keeps the answer the font worker gave, if it is the one asked for last and the text is
    /// still being typed (or is on its way to the stage).
    pub(in crate::workspace) fn text_preview_arrived(
        &mut self,
        id: u64,
        result: Result<(Option<Shape>, Caret), String>,
    ) {
        if self.text.preview_pending != Some(id) {
            return;
        }
        self.text.preview_pending = None;
        if !self.text_preview_active() {
            return;
        }
        // A request the engine refused (a size out of range, say) leaves the last picture up.
        if let Ok((shape, caret)) = result {
            self.text.preview = Some(TextPreview { shape, caret });
            self.text.preview_epoch += 1;
        }
    }

    /// Stops showing the typed text instead of the stored text.
    pub(in crate::workspace) fn clear_text_preview(&mut self) {
        self.text.preview = None;
        self.text.preview_asked = None;
        self.text.preview_pending = None;
        self.text.preview_epoch += 1;
        self.text.preview_shape = ShapeId::new();
    }

    /// Whether typed text is being shown: while it is typed, and while the finished text is being
    /// set so the stored letters do not come back for a moment.
    fn text_preview_active(&self) -> bool {
        self.text.edit.is_some() || self.text.pending.is_some()
    }

    /// The text object being edited, if one is.
    fn text_being_edited(&self) -> Option<flare_tools::ops::SpanRef> {
        let editing = self.text.edit.as_ref().and_then(|edit| edit.editing);
        editing.or(match self.text.pending {
            Some((_, TextTarget::Existing(at))) => Some(at),
            _ => None,
        })
    }

    /// A number that changes whenever the stage should be drawn again because of the typed text.
    pub(in crate::workspace) fn text_epoch(&self) -> u64 {
        self.text.preview_epoch * 2 + u64::from(self.text_preview_active())
    }

    /// What the render worker draws in place of the stored letters.
    pub(in crate::workspace) fn text_preview_render(&self) -> Option<TextPreviewRender> {
        if !self.text_preview_active() {
            return None;
        }
        let preview = self.text.preview.as_ref()?;
        let replaces = self
            .text_being_edited()
            .and_then(|at| self.text_at(at))
            .map(|(shape, _)| shape.id());
        Some(TextPreviewRender {
            replaces,
            shape: preview.shape.clone(),
        })
    }

    /// The box around the typed text and its caret, over the stage.
    pub(in crate::workspace) fn draw_text_cursor(
        &self,
        ctx: &egui::Context,
        image_rect: Rect,
        scale: f32,
    ) {
        let Some(preview) = self.text.preview.as_ref() else {
            return;
        };
        let to_screen = |p: Pos2| image_rect.min + p.to_vec2() * scale;
        let caret = preview.caret;
        let top = to_screen(Pos2::new(caret.x, caret.y));
        let stroke = Stroke::new(1.0, flare_ui::theme::accent());
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            self.text.editor_id.with("cursor"),
        ));
        // The box is the letters' bounds; with none yet it is a small one at the caret.
        let letters = preview.shape.as_ref().and_then(ops::shape_bounds);
        let around = letters.map_or_else(
            || {
                Rect::from_min_size(
                    top,
                    Vec2::new(caret.height * scale * 0.5, caret.height * scale),
                )
            },
            |bounds| Rect::from_min_max(to_screen(bounds.min), to_screen(bounds.max)),
        );
        painter.rect_stroke(around.expand(3.0), 0.0, stroke, StrokeKind::Outside);
        // The caret blinks, as a caret does.
        let on = (ctx.input(|i| i.time) * 2.0) as i64 % 2 == 0;
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
        if on {
            painter.line_segment(
                [top, top + Vec2::new(0.0, caret.height * scale)],
                Stroke::new(1.5, flare_ui::theme::accent()),
            );
        }
    }
}
