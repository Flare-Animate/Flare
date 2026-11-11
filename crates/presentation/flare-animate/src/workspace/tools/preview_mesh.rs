//! The live stroke preview as anti-aliased triangles.
//!
//! A GPU draws a mesh with no anti-aliasing: a pixel is painted only when its centre is inside a
//! triangle. A stroke narrower than a pixel or two (a light touch, a taper's tip, a small brush,
//! a zoomed-out stage) then covers some pixel centres and misses others, and shows as dashes and
//! dots until the stage's own anti-aliased picture of the committed shape replaces it. So every
//! edge here fades out over one device pixel, as egui's own shapes do, and a stroke thinner than
//! that pixel is drawn one pixel wide at a matching fraction of its opacity, so it fades rather
//! than breaks up.

use egui::epaint::{Mesh, Vertex, WHITE_UV};
use egui::{Color32, Pos2, Vec2};

use super::stroke_preview::{PreviewSides, PreviewStroke};

/// Corners in each round end of a ribbon built from points and widths.
const CAP_STEPS: usize = 8;

/// `strokes` as one mesh in screen space. `pixel` is the size of one device pixel in screen
/// units (`1 / pixels_per_point`), the width each edge fades out over.
pub(in crate::workspace) fn mesh(
    strokes: &[PreviewStroke],
    to_screen: impl Fn(Pos2) -> Pos2,
    pixel: f32,
) -> Mesh {
    let pixel = if pixel.is_finite() && pixel > 0.0 {
        pixel
    } else {
        1.0
    };
    let mut mesh = Mesh::default();
    for stroke in strokes {
        let sides = match &stroke.sides {
            Some(sides) => sides.clone(),
            None => ribbon(&stroke.points, &stroke.halves),
        };
        let screen = PreviewSides {
            centre: sides.centre.iter().map(|p| to_screen(*p)).collect(),
            left: sides.left.iter().map(|p| to_screen(*p)).collect(),
            right: sides.right.iter().map(|p| to_screen(*p)).collect(),
            end_cap: sides.end_cap.iter().map(|p| to_screen(*p)).collect(),
            start_cap: sides.start_cap.iter().map(|p| to_screen(*p)).collect(),
        };
        feathered(&mut mesh, &screen, stroke.color, pixel);
    }
    mesh
}

/// The sides of a ribbon through `points`, `halves[i]` to each side of point `i`, with round
/// ends: the shape of a line of one width, or of any stroke that has no outline of its own.
pub(in crate::workspace) fn ribbon(points: &[Pos2], halves: &[f32]) -> PreviewSides {
    let count = points.len().min(halves.len());
    let (points, halves) = (&points[..count], &halves[..count]);
    let mut sides = PreviewSides {
        centre: Vec::with_capacity(count),
        left: Vec::with_capacity(count),
        right: Vec::with_capacity(count),
        end_cap: Vec::new(),
        start_cap: Vec::new(),
    };
    if count == 0 {
        return sides;
    }
    let normals: Vec<Vec2> = (0..count)
        .map(|i| {
            let along = points[(i + 1).min(count - 1)] - points[i.saturating_sub(1)];
            let along = if along.length() > f32::EPSILON {
                along.normalized()
            } else {
                Vec2::X
            };
            Vec2::new(-along.y, along.x)
        })
        .collect();
    for i in 0..count {
        let half = halves[i].max(0.0);
        sides.centre.push(points[i]);
        sides.left.push(points[i] + normals[i] * half);
        sides.right.push(points[i] - normals[i] * half);
    }
    let last = count - 1;
    sides.end_cap = cap(points[last], normals[last], halves[last].max(0.0));
    sides.start_cap = cap(points[0], -normals[0], halves[0].max(0.0));
    sides
}

/// The half circle round `centre` from the `normal` side to the other, through the way the
/// line heads (as the brush library turns its ends), without the two side points.
fn cap(centre: Pos2, normal: Vec2, radius: f32) -> Vec<Pos2> {
    let heading = Vec2::new(normal.y, -normal.x);
    (1..CAP_STEPS)
        .map(|k| {
            let angle = std::f32::consts::PI * k as f32 / CAP_STEPS as f32;
            centre + (normal * angle.cos() + heading * angle.sin()) * radius
        })
        .collect()
}

fn vertex(pos: Pos2, color: Color32) -> Vertex {
    Vertex {
        pos,
        uv: WHITE_UV,
        color,
    }
}

/// How far in from an edge at `half` the solid paint stops, how far out the fade ends, and how
/// opaque the solid paint is: an edge fades over one `pixel`, and a stroke narrower than that is
/// a pixel wide at the fraction of opacity its width is of the pixel.
fn feather(half: f32, pixel: f32) -> (f32, f32, f32) {
    let inner = (half - pixel / 2.0).max(0.0);
    let outer = (half + pixel / 2.0).max(pixel);
    let opacity = (2.0 * half / pixel).clamp(0.0, 1.0);
    (inner, outer, opacity)
}

/// The outline in `sides` as triangles with feathered edges: along the line, four vertices
/// across at each point (faded, solid, solid, faded), and at each end a fan from the end's centre
/// with a faded rim.
fn feathered(mesh: &mut Mesh, sides: &PreviewSides, color: Color32, pixel: f32) {
    let count = sides
        .left
        .len()
        .min(sides.right.len())
        .min(sides.centre.len());
    if count == 0
        || !sides
            .left
            .iter()
            .chain(&sides.right)
            .chain(&sides.centre)
            .all(|p| p.is_finite())
    {
        return;
    }
    let clear = Color32::TRANSPARENT;
    let heading = |i: usize| {
        let along = sides.centre[(i + 1).min(count - 1)] - sides.centre[i.saturating_sub(1)];
        if along.length() > f32::EPSILON {
            along.normalized()
        } else {
            Vec2::X
        }
    };
    // The way from the right side to the left at a point: across the line where it has width,
    // square to the heading where it has none.
    let across = |i: usize| {
        let span = sides.left[i] - sides.right[i];
        if span.length() > 1e-4 {
            span.normalized()
        } else {
            let h = heading(i);
            Vec2::new(-h.y, h.x)
        }
    };
    let first = mesh.vertices.len() as u32;
    for i in 0..count {
        let middle = sides.left[i].lerp(sides.right[i], 0.5);
        let half = (sides.left[i] - sides.right[i]).length() / 2.0;
        let (inner, outer, opacity) = feather(half, pixel);
        let solid = color.gamma_multiply(opacity);
        let u = across(i);
        mesh.vertices.push(vertex(middle + u * outer, clear));
        mesh.vertices.push(vertex(middle + u * inner, solid));
        mesh.vertices.push(vertex(middle - u * inner, solid));
        mesh.vertices.push(vertex(middle - u * outer, clear));
    }
    for i in 0..count as u32 - 1 {
        let (a, b) = (first + 4 * i, first + 4 * (i + 1));
        for k in 0..3 {
            mesh.indices
                .extend([a + k, a + k + 1, b + k + 1, a + k, b + k + 1, b + k]);
        }
    }
    let last = count - 1;
    let ends = [
        (
            sides.centre[last],
            sides.left[last],
            &sides.end_cap,
            sides.right[last],
            heading(last),
        ),
        (
            sides.centre[0],
            sides.right[0],
            &sides.start_cap,
            sides.left[0],
            -heading(0),
        ),
    ];
    for (centre, from, rim, to, outward) in ends {
        let rim: Vec<Pos2> = std::iter::once(from)
            .chain(rim.iter().copied())
            .chain(std::iter::once(to))
            .collect();
        if !rim.iter().all(|p| p.is_finite()) {
            continue;
        }
        // Each rim point fades out along the way from the centre to it; a tip with no width
        // takes the directions of a half circle round its heading instead.
        let start = Vec2::new(outward.y, -outward.x);
        let steps = rim.len() - 1;
        let hub = mesh.vertices.len() as u32;
        let radius = rim
            .iter()
            .map(|p| (*p - centre).length())
            .fold(0.0, f32::max);
        let (_, _, opacity) = feather(radius, pixel);
        mesh.vertices
            .push(vertex(centre, color.gamma_multiply(opacity)));
        for (k, p) in rim.iter().enumerate() {
            let reach = *p - centre;
            let direction = if reach.length() > 1e-4 {
                reach.normalized()
            } else {
                let angle = std::f32::consts::PI * k as f32 / steps as f32;
                start * angle.cos() + outward * angle.sin()
            };
            let (inner, outer, opacity) = feather(reach.length(), pixel);
            mesh.vertices.push(vertex(
                centre + direction * inner,
                color.gamma_multiply(opacity),
            ));
            mesh.vertices
                .push(vertex(centre + direction * outer, clear));
        }
        for k in 0..steps as u32 {
            let (i0, o0, i1, o1) = (
                hub + 1 + 2 * k,
                hub + 2 + 2 * k,
                hub + 3 + 2 * k,
                hub + 4 + 2 * k,
            );
            mesh.indices.extend([hub, i0, i1, i0, o0, o1, i0, o1, i1]);
        }
    }
}
