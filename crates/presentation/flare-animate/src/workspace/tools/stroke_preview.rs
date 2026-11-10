//! The pencil and brush stroke while it is being drawn, and until the drawing of it is on the stage.
//!
//! The preview is built from the same samples, straightening, mirroring and pressure curve as the
//! stroke that is committed, so releasing the pointer changes how it looks as little as possible
//! (PaintCraft's rule: the preview and the release share one builder). After the release the
//! preview stays up until the stage texture that includes the stroke has arrived from the render
//! worker; without that the stroke vanishes for a few frames and then reappears.

use egui::epaint::{Mesh, Vertex, WHITE_UV};
use egui::{Color32, Pos2, Rect, Vec2};
use flare_input::InputSample;

use crate::workspace::*;

/// Most samples one stroke keeps. A longer drag stops growing the stroke (it can be finished and
/// continued), so memory and the work per frame stay bounded.
pub(in crate::workspace) const MAX_FREEHAND_SAMPLES: usize = 8_000;

/// A point closer than this to the last one, in screen pixels, adds nothing the eye can see.
const MIN_STEP_PX: f32 = 0.5;

/// Passes a committed stroke may stay up for.
const SETTLE_PASSES: u32 = 600;

/// One stroke to show: where it goes and how wide it is at each point, in stage units.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::workspace) struct PreviewStroke {
    pub(in crate::workspace) points: Vec<Pos2>,
    pub(in crate::workspace) halves: Vec<f32>,
    pub(in crate::workspace) color: Color32,
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

/// `strokes` as one mesh in screen space: a quad along each segment and a round end at each
/// point, widths following `halves`. Nothing is allocated per point beyond the mesh itself.
pub(in crate::workspace) fn mesh(
    strokes: &[PreviewStroke],
    to_screen: impl Fn(Pos2) -> Pos2,
    scale: f32,
) -> Mesh {
    let mut mesh = Mesh::default();
    for stroke in strokes {
        let points: Vec<Pos2> = stroke.points.iter().map(|p| to_screen(*p)).collect();
        for (i, (&at, &half)) in points.iter().zip(&stroke.halves).enumerate() {
            let radius = (half * scale).max(0.0);
            disc(&mut mesh, at, radius, stroke.color);
            if let (Some(&next), Some(&next_half)) = (points.get(i + 1), stroke.halves.get(i + 1)) {
                quad(
                    &mut mesh,
                    (at, radius),
                    (next, (next_half * scale).max(0.0)),
                    stroke.color,
                );
            }
        }
    }
    mesh
}

fn vertex(pos: Pos2, color: Color32) -> Vertex {
    Vertex {
        pos,
        uv: WHITE_UV,
        color,
    }
}

/// Corners of the polygon that stands for a round end or joint: a thin line needs few.
fn corners(radius: f32) -> usize {
    match radius {
        r if r < 2.0 => 6,
        r if r < 8.0 => 10,
        _ => 14,
    }
}

fn disc(mesh: &mut Mesh, at: Pos2, radius: f32, color: Color32) {
    if !(radius.is_finite() && at.is_finite()) {
        return;
    }
    let round = corners(radius);
    let first = mesh.vertices.len() as u32;
    mesh.vertices.push(vertex(at, color));
    for k in 0..round {
        let angle = std::f32::consts::TAU * k as f32 / round as f32;
        mesh.vertices
            .push(vertex(at + Vec2::angled(angle) * radius, color));
    }
    for k in 0..round as u32 {
        mesh.indices
            .extend([first, first + 1 + k, first + 1 + (k + 1) % round as u32]);
    }
}

fn quad(mesh: &mut Mesh, a: (Pos2, f32), b: (Pos2, f32), color: Color32) {
    let along = b.0 - a.0;
    if !(a.0.is_finite() && b.0.is_finite()) || along.length() < f32::EPSILON {
        return;
    }
    let normal = Vec2::new(-along.y, along.x).normalized();
    let first = mesh.vertices.len() as u32;
    for (p, r, sign) in [
        (a.0, a.1, 1.0),
        (a.0, a.1, -1.0),
        (b.0, b.1, -1.0),
        (b.0, b.1, 1.0),
    ] {
        mesh.vertices.push(vertex(p + normal * r * sign, color));
    }
    mesh.indices
        .extend([first, first + 1, first + 2, first, first + 2, first + 3]);
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
    pub(in crate::workspace) fn previews_of(
        &self,
        strokes: &[Vec<InputSample>],
    ) -> Vec<PreviewStroke> {
        let curve = self.brush_shelf.pressure_curve();
        let (fill, size, brush) = match self.tool {
            Tool::Brush => (self.paint.fill, self.brush_size, true),
            _ => (self.paint.stroke_color, self.paint.stroke_width, false),
        };
        let nib = self.brush_shelf.nib();
        let velocity = self.brush_shelf.velocity();
        // Flow is how opaque the paint is at the first touch; the commit stamps the same alpha
        // into the shape.
        let mut color = fill;
        if brush {
            color = Color32::from_rgba_unmultiplied(
                fill.r(),
                fill.g(),
                fill.b(),
                (f32::from(fill.a()) * self.brush_shelf.flow()) as u8,
            );
        }
        // A flat nib paints thick where the line crosses its long axis, so the preview ribbon
        // takes the same reach from the direction the line is heading.
        let half = |stroke: &[InputSample], i: usize| -> f32 {
            let s = &stroke[i];
            if !brush {
                return size / 2.0;
            }
            let (a, b) = (
                &stroke[i.saturating_sub(1)],
                &stroke[(i + 1).min(stroke.len() - 1)],
            );
            let reach = nib.reach_across(glamx::DVec2::new(b.x - a.x, b.y - a.y)) as f32;
            let pressure = curve.width_factor(s.pressure);
            if velocity <= 0.0 {
                return size / 2.0 * pressure * reach;
            }
            // The same width the stroke will keep: speed against pressure, as far as the brush
            // asks, over half the step to each side.
            let speed = (b.x - a.x).hypot(b.y - a.y) / 2.0 / flare_tools::brush::SPEED_REACH;
            let speed_factor = 1.0 - (1.0 - f64::from(curve.min())) * speed.clamp(0.0, 1.0);
            let factor =
                f64::from(pressure) + (speed_factor - f64::from(pressure)) * f64::from(velocity);
            size / 2.0 * factor as f32 * reach
        };
        strokes
            .iter()
            .map(|stroke| PreviewStroke {
                points: stroke
                    .iter()
                    .map(|p| Pos2::new(p.x as f32, p.y as f32))
                    .collect(),
                halves: (0..stroke.len()).map(|i| half(stroke, i)).collect(),
                color,
            })
            .collect()
    }

    /// The stroke being drawn, as it will look when committed.
    pub(in crate::workspace) fn freehand_preview(&self) -> Vec<PreviewStroke> {
        self.previews_of(&self.stroke_set(self.freehand.clone()))
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
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn stroke(points: &[(f32, f32)], half: f32) -> PreviewStroke {
        PreviewStroke {
            points: points.iter().map(|&(x, y)| Pos2::new(x, y)).collect(),
            halves: vec![half; points.len()],
            color: Color32::RED,
        }
    }

    #[test]
    fn the_mesh_covers_the_stroke_end_to_end_and_its_width() {
        let m = mesh(&[stroke(&[(0.0, 0.0), (100.0, 0.0)], 5.0)], |p| p, 1.0);
        let bounds = m.calc_bounds();
        assert!((bounds.min.x - -5.0).abs() < 0.01 && (bounds.max.x - 105.0).abs() < 0.01);
        assert!((bounds.min.y - -5.0).abs() < 0.01 && (bounds.max.y - 5.0).abs() < 0.01);
    }

    #[test]
    fn zoom_scales_the_width_and_the_mesh_indexes_only_vertices_it_has() {
        let m = mesh(
            &[stroke(&[(0.0, 0.0), (10.0, 10.0), (20.0, 0.0)], 2.0)],
            |p| p,
            3.0,
        );
        assert!(m.calc_bounds().height() > 6.0 * 3.0 - 0.5 - 1.0);
        let top = m.vertices.len() as u32;
        assert!(m.indices.iter().all(|i| *i < top));
        assert_eq!(m.indices.len() % 3, 0);
    }

    #[test]
    fn non_finite_points_make_no_geometry_and_an_empty_stroke_makes_none() {
        let m = mesh(&[stroke(&[(f32::NAN, 0.0), (1.0, 1.0)], 2.0)], |p| p, 1.0);
        assert!(
            m.vertices
                .iter()
                .all(|v| v.pos.is_finite() || v.pos.x.is_nan())
        );
        assert!(m.indices.iter().all(|i| (*i as usize) < m.vertices.len()));
        assert!(mesh(&[stroke(&[], 2.0)], |p| p, 1.0).is_empty());
    }

    #[test]
    fn wider_points_make_a_wider_stroke_so_pressure_shows() {
        let mut s = stroke(&[(0.0, 0.0), (50.0, 0.0)], 1.0);
        s.halves[1] = 9.0;
        let m = mesh(&[s], |p| p, 1.0);
        assert!((m.calc_bounds().max.y - 9.0).abs() < 0.01);
    }
}
