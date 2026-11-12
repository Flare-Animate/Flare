//! Turning what was painted. egui's layer transforms move and scale but cannot rotate, so the
//! stage is painted upright and each of its shapes is then turned about the panel's center in the
//! paint list. Shapes keep their kind: a rectangle stays a rectangle with an angle, text keeps its
//! glyphs and turns with them, a mesh (the stage picture, an icon) has its corners turned.

use std::sync::Arc;

use egui::emath::Rot2;
use egui::epaint::{ClippedShape, Shape};
use egui::layers::ShapeIdx;
use egui::{Context, LayerId, Pos2};

/// Turns `shape` by `rot` about `pivot`.
pub fn turn_shape(shape: &mut Shape, pivot: Pos2, rot: Rot2) {
    let turn = |p: &mut Pos2| *p = pivot + rot * (*p - pivot);
    match shape {
        // A band (a plot's filled range) turns only about the origin, and the stage paints none,
        // so it stays as it is; a callback paints itself.
        Shape::Noop | Shape::Callback(_) | Shape::Band(_) => {}
        Shape::Vec(shapes) => {
            for s in shapes {
                turn_shape(s, pivot, rot);
            }
        }
        Shape::Circle(c) => turn(&mut c.center),
        Shape::Ellipse(e) => {
            turn(&mut e.center);
            e.angle += rot.angle();
        }
        Shape::LineSegment { points, .. } => points.iter_mut().for_each(turn),
        Shape::Path(p) => p.points.iter_mut().for_each(turn),
        Shape::QuadraticBezier(b) => b.points.iter_mut().for_each(turn),
        Shape::CubicBezier(b) => b.points.iter_mut().for_each(turn),
        Shape::Rect(r) => {
            // A rectangle's angle turns it about its own center: move the center, add the turn.
            let mut center = r.rect.center();
            turn(&mut center);
            r.rect = egui::Rect::from_center_size(center, r.rect.size());
            r.angle += rot.angle();
        }
        Shape::Text(t) => {
            // Text turns about its position, which is where it is placed from.
            turn(&mut t.pos);
            t.angle += rot.angle();
        }
        Shape::Mesh(mesh) => {
            for v in &mut Arc::make_mut(mesh).vertices {
                turn(&mut v.pos);
            }
        }
    }
}

/// Turns every shape painted on `layer` from `from` on by `angle` radians clockwise about
/// `pivot`. Their clip rectangles stay: the stage clips to its panel whichever way it is turned.
pub fn turn_painted(ctx: &Context, layer: LayerId, from: ShapeIdx, pivot: Pos2, angle: f32) {
    if angle == 0.0 || !angle.is_finite() || !pivot.is_finite() {
        return;
    }
    let rot = Rot2::from_angle(angle);
    ctx.graphics_mut(|g| {
        let list = g.entry(layer);
        let end = list.next_idx().0;
        for i in from.0..end {
            list.mutate_shape(ShapeIdx(i), |c: &mut ClippedShape| {
                turn_shape(&mut c.shape, pivot, rot);
            });
        }
    });
}

/// Where painting on `layer` has got to, for [`turn_painted`].
pub fn painted_so_far(ctx: &Context, layer: LayerId) -> ShapeIdx {
    ctx.graphics_mut(|g| g.entry(layer).next_idx())
}

#[cfg(test)]
#[allow(clippy::expect_used)]
#[path = "turn_tests.rs"]
mod tests;
