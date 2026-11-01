//! The text tool: text typed on the stage where it will be, and edited again later.
//!
//! Text is a shape that remembers its words and font (see `flare_document::TextSource`). The
//! editor is a box on the stage at the text's corner, in the size it will have; when it is let go
//! of, the font worker sets the words into outlines and the shape appears, or replaces the one
//! being edited.

use egui::{Color32, Key, Pos2, Rect, Stroke};
use flare_core::{DocumentPoint, Rgba8, ShapeId};
use flare_document::{Content, Shape, TextSource};
use flare_runtime::TextJob;
use flare_tools::ops::{self, SpanRef};

use crate::workspace::*;

/// The editor's text is drawn no smaller or larger than this on the screen, whatever the zoom.
const EDITOR_PX: std::ops::RangeInclusive<f32> = 8.0..=600.0;

/// How far around its letters a click still counts as on the text, in stage units.
const TEXT_BOX_MARGIN: f32 = 4.0;

fn rgba(color: Color32) -> Rgba8 {
    let c = color.to_array();
    Rgba8::new(c[0], c[1], c[2], c[3])
}

fn color32(color: Rgba8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}

impl Workspace {
    // ---- the editor on the stage ------------------------------------------------------

    /// The box where text is typed, at the text's corner on the stage and in the size it will
    /// have. A press on the stage outside it, Escape, or Ctrl/Cmd+Enter finishes the text.
    pub(in crate::workspace) fn text_editor(
        &mut self,
        ctx: &egui::Context,
        image_rect: Rect,
        scale: f32,
    ) {
        let id = self.text.editor_id;
        let px = (self.text.size * scale).clamp(*EDITOR_PX.start(), *EDITOR_PX.end());
        let Some(edit) = self.text.edit.as_mut() else {
            return;
        };
        let at = image_rect.min + edit.at.to_vec2() * scale;
        let mut finish = false;
        let mut editor = None;
        let mut cursor = 0;
        egui::Area::new(id)
            .fixed_pos(at)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                // The field only takes the typing: the letters, the caret and the box are drawn
                // from the font worker's answer (`draw_text_cursor`), in the text's real font.
                let visuals = ui.visuals_mut();
                visuals.text_cursor.stroke = Stroke::NONE;
                visuals.selection.bg_fill = Color32::TRANSPARENT;
                let output = egui::TextEdit::multiline(&mut edit.buffer)
                    .font(egui::FontId::proportional(px))
                    .text_color(Color32::TRANSPARENT)
                    .frame(egui::Frame::NONE)
                    .margin(egui::Margin::ZERO)
                    .desired_rows(1)
                    .desired_width(px * 4.0)
                    .lock_focus(true)
                    .show(ui);
                if !edit.focused {
                    output.response.request_focus();
                    edit.focused = true;
                }
                cursor = output
                    .cursor_range
                    .map_or(edit.buffer.chars().count(), |range| {
                        range.primary.index.into()
                    });
                editor = Some(output.response.rect);
            });
        let Some(rect) = editor else { return };
        self.update_text_preview(cursor);
        self.draw_text_cursor(ctx, image_rect, scale);
        let pressed_outside = ctx.input(|i| {
            i.pointer.any_pressed()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|p| self.stage_panel.contains(p) && !rect.expand(6.0).contains(p))
        });
        ctx.input(|i| {
            finish |= i.key_pressed(Key::Escape);
            finish |= i.modifiers.command && i.key_pressed(Key::Enter);
        });
        if finish || pressed_outside {
            self.place_text();
        }
    }

    /// The color the text being typed will be: its object's, or the fill for new text.
    pub(in crate::workspace) fn text_ink(&self) -> Color32 {
        let existing = self
            .text
            .edit
            .as_ref()
            .and_then(|edit| edit.editing)
            .and_then(|at| self.text_at(at));
        existing.map_or(self.paint.fill, |(shape, _)| color32(shape.fill()))
    }

    // ---- starting and finishing -------------------------------------------------------

    /// A click on the stage with the Text tool: on text, edit it; elsewhere, start new text.
    pub(in crate::workspace) fn text_click(&mut self, at: Pos2) {
        if self.text.edit.is_some() {
            return;
        }
        // Text under the click wins over whatever is drawn behind it.
        let text = self
            .text_box_hit(at)
            .or_else(|| self.hit(at).filter(|span| self.text_at(*span).is_some()));
        match text {
            Some(span) => self.open_text_edit(span),
            None => {
                self.text.edit = Some(PendingText {
                    at,
                    buffer: String::new(),
                    focused: false,
                    editing: None,
                });
            }
        }
    }

    /// Double-clicking text with the Selection tool edits it. False when the selection is not text.
    pub(in crate::workspace) fn edit_selected_text(&mut self) -> bool {
        let Some(span) = self.selected.filter(|span| self.text_at(*span).is_some()) else {
            return false;
        };
        self.set_tool(Tool::Text);
        self.open_text_edit(span);
        true
    }

    /// The topmost text object whose box (the letters' bounds, with a margin) holds `point`.
    /// Clicking between letters, or inside one, still means that text.
    pub(in crate::workspace) fn text_box_hit(&self, point: Pos2) -> Option<SpanRef> {
        let scene = self.scene_id()?;
        let layers = self.document().scenes().first()?.timeline().layers();
        layers.iter().enumerate().find_map(|(index, layer)| {
            let span = layer.span_at(self.frame)?;
            let Content::Shape(shape) = span.content() else {
                return None;
            };
            shape.text()?;
            if !layer.is_visible() || crate::workspace::selection::locked_in_folders(layers, index)
            {
                return None;
            }
            let at = SpanRef {
                layer: layer.id(),
                start: span.start(),
            };
            let bounds = ops::span_bounds(self.document(), scene, at)?;
            bounds.expand(TEXT_BOX_MARGIN).contains(point).then_some(at)
        })
    }

    /// The text object in the span `at`, and its source.
    pub(in crate::workspace) fn text_at(&self, at: SpanRef) -> Option<(Shape, TextSource)> {
        let scene = self.scene_id()?;
        let Content::Shape(shape) = ops::find_span(self.document(), scene, at)?.content() else {
            return None;
        };
        let source = shape.text()?.clone();
        Some((shape.clone(), source))
    }

    /// The text object selected, if one is.
    pub(in crate::workspace) fn selected_text(&self) -> Option<(SpanRef, Shape, TextSource)> {
        let at = self.selected?;
        let (shape, source) = self.text_at(at)?;
        Some((at, shape, source))
    }

    fn open_text_edit(&mut self, at: SpanRef) {
        let Some((_, source)) = self.text_at(at) else {
            return;
        };
        // The tool's settings show the text's own, so they can be changed while it is edited.
        self.text.family = source.family().to_owned();
        self.text.size = source.size();
        self.text.bold = source.bold();
        self.text.italic = source.italic();
        self.selected = Some(at);
        self.also_selected.clear();
        let corner = source.origin().as_vec();
        self.text.edit = Some(PendingText {
            at: Pos2::new(corner.x as f32, corner.y as f32),
            buffer: source.text().to_owned(),
            focused: false,
            editing: Some(at),
        });
    }

    /// Finishes the text being typed: the words go to the font worker, and the shape appears when
    /// it is done. Emptied text that was being edited is removed.
    pub fn place_text(&mut self) {
        let Some(edit) = self.text.edit.take() else {
            return;
        };
        if edit.buffer.trim().is_empty() {
            self.clear_text_preview();
            if let (Some(at), Some(scene)) = (edit.editing, self.scene_id()) {
                self.run_batch(ops::delete(scene, at), "delete text");
                self.selected = None;
            }
            return;
        }
        let existing = edit.editing.and_then(|at| Some((at, self.text_at(at)?.0)));
        let Ok(origin) = DocumentPoint::new(f64::from(edit.at.x), f64::from(edit.at.y)) else {
            self.clear_text_preview();
            return self.report("Cannot place text: the position is not a number");
        };
        let source = match TextSource::new(
            edit.buffer,
            self.text.family.clone(),
            self.text.size,
            self.text.bold,
            self.text.italic,
            origin,
        ) {
            Ok(source) => source,
            Err(error) => {
                self.clear_text_preview();
                return self.report(format!("Cannot place text: {error}"));
            }
        };
        self.submit_text_shape(source, existing);
    }

    /// Sets `source` into outlines off the UI thread, as a new shape or as the next version of
    /// `existing`.
    fn submit_text_shape(&mut self, source: TextSource, existing: Option<(SpanRef, Shape)>) {
        let (target, shape, fill) = match &existing {
            Some((at, old)) => (TextTarget::Existing(*at), old.id(), old.fill()),
            None => (TextTarget::New, ShapeId::new(), rgba(self.paint.fill)),
        };
        let id = self.background.next_id();
        self.text.pending = Some((id, target));
        self.background.submit_text(TextJob::Set {
            id,
            source,
            shape,
            fill,
        });
        self.settle_inline();
    }

    /// Sets the selected text object in a new `source` (a different font or size, say).
    pub(in crate::workspace) fn retext_selected(&mut self, source: TextSource) {
        if let Some((at, shape, _)) = self.selected_text() {
            self.submit_text_shape(source, Some((at, shape)));
        }
    }

    /// Puts finished text on the stage: on a layer of its own, or in place of the text it edits.
    pub(in crate::workspace) fn text_ready(&mut self, shape: Shape, target: TextTarget) {
        match target {
            TextTarget::New => self.insert_shape_layer(shape, "Text"),
            TextTarget::Existing(at) => {
                let Some(scene) = self.scene_id() else { return };
                let Some((old, _)) = self.text_at(at) else {
                    return self.report("The text changed while it was being set");
                };
                let shape = shape.painted_like(&old);
                // When nothing about the text changed there is nothing to do.
                if let Some(commands) = ops::replace_shape(self.document(), scene, at, shape) {
                    self.run_batch(commands, "edit text");
                }
            }
        }
    }

    // ---- the fonts there are ----------------------------------------------------------

    /// Asks the font worker for the families there are, once.
    pub(in crate::workspace) fn ask_font_families(&mut self) {
        if !self.text.families_asked {
            self.text.families_asked = true;
            self.background.submit_text(TextJob::Families);
            self.settle_inline();
        }
    }

    /// Finishes the text being typed when something else is chosen, as the other tools finish
    /// what they are drawing.
    pub(in crate::workspace) fn finish_text_for_tool_change(&mut self) {
        if self.text.edit.is_some() {
            self.place_text();
        }
    }
}
