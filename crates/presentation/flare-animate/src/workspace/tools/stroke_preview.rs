//! The pencil and brush stroke while it is being drawn, and until the drawing of it is on the stage.
//!
//! The preview is built from the same samples, straightening, mirroring and pressure curve as the
//! stroke that is committed, so releasing the pointer changes how it looks as little as possible
//! (PaintCraft's rule: the preview and the release share one builder). After the release the
//! preview stays up until the stage texture that includes the stroke has arrived from the render
//! worker; without that the stroke vanishes for a few frames and then reappears.

use egui::{Color32, Pos2, Rect, Vec2};

pub(in crate::workspace) use super::preview_mesh::mesh;
use flare_input::InputSample;

use crate::workspace::*;

/// Most samples one stroke keeps. A longer drag stops growing the stroke (it can be finished and
/// continued), so memory and the work per frame stay bounded.
pub(in crate::workspace) const MAX_FREEHAND_SAMPLES: usize = 8_000;

/// A point closer than this to the last one, in screen pixels, adds nothing the eye can see.
const MIN_STEP_PX: f32 = 0.5;

/// Passes a committed stroke may stay up for.
const SETTLE_PASSES: u32 = 600;

/// One stroke to show: where it goes and how wide it is at each point, in stage units. A Brush
/// stroke also has the sides of the outline it will commit, which are drawn instead of the
/// ribbon, so the shape on screen while dragging is the shape that is kept.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::workspace) struct PreviewStroke {
    pub(in crate::workspace) points: Vec<Pos2>,
    pub(in crate::workspace) halves: Vec<f32>,
    pub(in crate::workspace) color: Color32,
    pub(in crate::workspace) sides: Option<PreviewSides>,
}

/// [`flare_tools::brush::StrokeSides`] in stage units for drawing.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::workspace) struct PreviewSides {
    pub(in crate::workspace) centre: Vec<Pos2>,
    pub(in crate::workspace) left: Vec<Pos2>,
    pub(in crate::workspace) right: Vec<Pos2>,
    pub(in crate::workspace) end_cap: Vec<Pos2>,
    pub(in crate::workspace) start_cap: Vec<Pos2>,
}

impl PreviewSides {
    pub(in crate::workspace) fn of(sides: &flare_tools::brush::StrokeSides) -> Self {
        let pos = |points: &[glamx::DVec2]| -> Vec<Pos2> {
            points
                .iter()
                .map(|p| Pos2::new(p.x as f32, p.y as f32))
                .collect()
        };
        Self {
            centre: pos(&sides.centre),
            left: pos(&sides.left),
            right: pos(&sides.right),
            end_cap: pos(&sides.end_cap),
            start_cap: pos(&sides.start_cap),
        }
    }
}

/// A filled shape the geometry tools just committed, in stage units, with its outline if it has
/// one.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::workspace) enum SettleShape {
    Rect {
        rect: Rect,
        radius: f32,
        fill: Color32,
        outline: Option<(f32, Color32)>,
    },
    Ellipse {
        center: Pos2,
        radius: Vec2,
        fill: Color32,
        outline: Option<(f32, Color32)>,
    },
}

/// A committed stroke that is shown until the stage picture of the document it is in is up.
pub(in crate::workspace) struct Settling {
    pub(in crate::workspace) strokes: Vec<PreviewStroke>,
    /// Shapes the geometry tools committed (the line is a stroke above).
    pub(in crate::workspace) shapes: Vec<SettleShape>,
    pub(in crate::workspace) context: u64,
    pub(in crate::workspace) revision: u64,
    /// Passes left before it is dropped whatever happens, so a picture that never shows the
    /// stroke (a failed render, a playhead beyond it) cannot leave it up for good.
    pub(in crate::workspace) passes_left: u32,
}

impl Workspace {
    /// Adds a sample to the stroke being drawn, unless it is too close to the last one to matter
    /// at this zoom or the stroke is as long as it may get.
    pub(in crate::workspace) fn push_freehand(&mut self, sample: InputSample, scale: f32) {
        let near = self.freehand.last().is_some_and(|last| {
            let step = Vec2::new((sample.x - last.x) as f32, (sample.y - last.y) as f32);
            step.length() * scale < MIN_STEP_PX
        });
        let full = self.freehand.len() >= MAX_FREEHAND_SAMPLES;
        if (near || full) && self.freehand.len() > 1 {
            // The stroke ends where the pointer is, even if it adds no new point.
            if let Some(last) = self.freehand.last_mut() {
                *last = sample;
            }
        } else if !near && !full {
            self.freehand.push(sample);
        }
    }

    /// `sample` steadied by the Brush's stabilizer, which starts afresh with each stroke. Other
    /// tools, and a brush with the stabilizer off, take the sample as it is.
    pub(in crate::workspace) fn steady_freehand(
        &mut self,
        sample: InputSample,
        scale: f32,
    ) -> InputSample {
        let level = self.brush_shelf.stabilizer();
        if self.tool != Tool::Brush || level == 0 {
            self.stabilizer = None;
            return sample;
        }
        if self.freehand.is_empty() {
            self.stabilizer = None;
        }
        self.stabilizer
            .get_or_insert_with(|| flare_input::Stabilizer::new(level, true, f64::from(scale)))
            .push(sample)
    }

    /// Brings a steadied stroke, which trails the pointer, to where the pointer was let go.
    pub(in crate::workspace) fn catch_up_freehand(&mut self, scale: f32) {
        if let Some(mut stabilizer) = self.stabilizer.take() {
            for sample in stabilizer.finish() {
                self.push_freehand(sample, scale);
            }
        }
    }

    /// `samples` as the strokes that will be committed: straightened if that is asked for, then
    /// with its mirror images. The preview and the commit both start here.
    pub(in crate::workspace) fn stroke_set(
        &self,
        mut samples: Vec<InputSample>,
    ) -> Vec<Vec<InputSample>> {
        if self.paint.straight_edge || self.paint.straight_held {
            samples = flare_tools::ruler::straighten(&samples);
        }
        let about = self.symmetry_center();
        let mirrored: Vec<Vec<InputSample>> = self
            .paint
            .symmetry
            .reflections()
            .iter()
            .map(|r| r.samples(&samples, about))
            .collect();
        let mut strokes = vec![samples];
        strokes.extend(mirrored);
        strokes
    }

    /// `strokes` at the width and in the color of the tool in hand.
    ///
    /// `scale` is the stage's zoom, which the Smoothing is judged at, as it is on release.
    pub(in crate::workspace) fn previews_of(
        &self,
        strokes: &[Vec<InputSample>],
        scale: f32,
    ) -> Vec<PreviewStroke> {
        if self.tool == Tool::Eraser {
            return self.eraser_previews(strokes);
        }
        let tolerance = self.smoothing / scale.max(f32::EPSILON);
        let to_points = |stroke: &[InputSample]| -> Vec<Pos2> {
            stroke
                .iter()
                .map(|p| Pos2::new(p.x as f32, p.y as f32))
                .collect()
        };
        if self.tool != Tool::Brush {
            // The curve the Pencil commits, not the points the pointer passed through.
            let half = self.paint.stroke_width / 2.0;
            return strokes
                .iter()
                .map(|stroke| {
                    let points =
                        flare_tools::pencil::smoothed_points(&to_points(stroke), tolerance);
                    PreviewStroke {
                        halves: vec![half; points.len()],
                        points,
                        color: self.paint.stroke_color,
                        sides: None,
                    }
                })
                .collect();
        }
        // Flow is how opaque the paint is at the first touch; the commit stamps the same alpha
        // into the shape.
        let fill = self.paint.fill;
        let color = Color32::from_rgba_unmultiplied(
            fill.r(),
            fill.g(),
            fill.b(),
            (f32::from(fill.a()) * self.brush_shelf.flow()) as u8,
        );
        // The widths come from the brush library, as the committed outline's do: pressure,
        // speed, tapers and the reach of the nib across the line.
        let brush = self.brush_shelf.stroke(f64::from(self.brush_size));
        strokes
            .iter()
            .map(|stroke| {
                let spine = flare_tools::brush::BrushStroke::spine(stroke);
                PreviewStroke {
                    points: to_points(&spine),
                    halves: brush
                        .half_widths(&spine)
                        .into_iter()
                        .map(|h| h as f32)
                        .collect(),
                    color,
                    // The very outline the release commits: smoothed, shaped by the nib, tapered.
                    sides: brush
                        .sides(stroke, f64::from(tolerance))
                        .as_ref()
                        .map(PreviewSides::of),
                }
            })
            .collect()
    }

    /// The stroke being drawn, as it will look when committed.
    pub(in crate::workspace) fn freehand_preview(&self, scale: f32) -> Vec<PreviewStroke> {
        self.previews_of(&self.stroke_set(self.freehand.clone()), scale)
    }

    /// Remembers what was just committed so it stays on screen until the stage shows it.
    pub(in crate::workspace) fn settle(&mut self, strokes: Vec<PreviewStroke>) {
        // Overlapping parts of a see-through preview would show darker than the stroke does.
        if strokes.iter().any(|s| s.color.a() < 255) {
            return;
        }
        self.paint.settling = Some(Settling {
            strokes,
            shapes: Vec::new(),
            context: self.context_serial,
            revision: self.active().revision(),
            passes_left: SETTLE_PASSES,
        });
    }

    /// Remembers a shape or line the geometry tools just committed, like [`Self::settle`], so
    /// that it stays on screen from the release until the stage picture shows it.
    pub(in crate::workspace) fn settle_geometry(
        &mut self,
        strokes: Vec<PreviewStroke>,
        shapes: Vec<SettleShape>,
    ) {
        self.paint.settling = Some(Settling {
            strokes,
            shapes,
            context: self.context_serial,
            revision: self.active().revision(),
            passes_left: SETTLE_PASSES,
        });
    }

    /// Drops the settling stroke once the stage texture includes it, or once the document has
    /// moved on (an undo, another edit, another tab's context) so it would be wrong to show.
    pub(in crate::workspace) fn retire_settled(&mut self) {
        let Some(settling) = &self.paint.settling else {
            return;
        };
        let drawn = self
            .texture
            .as_ref()
            .is_some_and(|t| t.key.0 == settling.context && t.key.1 >= settling.revision);
        let current = settling.context == self.context_serial
            && settling.revision == self.active().revision();
        let spent = settling.passes_left == 0;
        if drawn || !current || spent {
            self.paint.settling = None;
        } else if let Some(settling) = &mut self.paint.settling {
            settling.passes_left -= 1;
        }
    }
}

#[cfg(test)]
#[path = "stroke_preview_tests.rs"]
mod tests;
