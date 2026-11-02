//! Affine transforms for the Free Transform tool and Modify > Transform: the
//! matrices, made to turn about any point, and applying one to an element.

use crate::doc::{BitmapInstance, Element, Instance, Matrix, Point, Shape, Text};

impl Matrix {
    pub fn translate(dx: f32, dy: f32) -> Matrix {
        Matrix { tx: dx, ty: dy, ..Matrix::IDENTITY }
    }

    pub fn scale(sx: f32, sy: f32) -> Matrix {
        Matrix { a: sx, d: sy, ..Matrix::IDENTITY }
    }

    /// Turns by `angle` radians, clockwise on screen (y points down).
    pub fn rotate(angle: f32) -> Matrix {
        let (sin, cos) = angle.sin_cos();
        Matrix { a: cos, b: sin, c: -sin, d: cos, tx: 0.0, ty: 0.0 }
    }

    /// Shears x by `kx`·y and y by `ky`·x: skews of atan(kx) and atan(ky).
    pub fn skew(kx: f32, ky: f32) -> Matrix {
        Matrix { b: ky, c: kx, ..Matrix::IDENTITY }
    }

    /// The same transform with `pivot` as its origin, so `pivot` stays put.
    pub fn about(self, (x, y): Point) -> Matrix {
        Matrix::translate(x, y) * self * Matrix::translate(-x, -y)
    }

    /// None if the matrix flattens the plane onto a line or a point.
    pub fn inverse(&self) -> Option<Matrix> {
        let det = self.a * self.d - self.b * self.c;
        if !det.is_normal() {
            return None;
        }
        let (a, b, c, d) = (self.d / det, -self.b / det, -self.c / det, self.a / det);
        Some(Matrix { a, b, c, d, tx: -(a * self.tx + c * self.ty), ty: -(b * self.tx + d * self.ty) })
    }

    /// The Transform panel's numbers: the x and y scales, and how far the x
    /// and y axes are skewed, in radians clockwise. Equal skews are a rotation.
    pub fn parts(&self) -> (f32, f32, f32, f32) {
        // Adding 0 turns -0 (from -c when c is 0) into 0, which shows as "0.0°" rather than "-0.0°".
        (self.a.hypot(self.b), self.c.hypot(self.d), (-self.c).atan2(self.d) + 0.0, self.b.atan2(self.a) + 0.0)
    }

    /// The matrix with these `parts`, with no translation.
    pub fn from_parts((sx, sy, skew_x, skew_y): (f32, f32, f32, f32)) -> Matrix {
        Matrix { a: sx * skew_y.cos(), b: sx * skew_y.sin(), c: -sy * skew_x.sin(), d: sy * skew_x.cos(), tx: 0.0, ty: 0.0 }
    }
}

/// Remove Transform: a shape, instance or text without its scale, rotation and
/// skew, as drawn or as in its symbol, the point `fixed` of its own
/// coordinates staying where it shows.
pub fn remove_transform(el: &mut Element, fixed: Point) {
    // ponytail: a group has no matrix of its own here (its members carry it), so it stays as it is.
    if let Element::Shape(Shape { matrix, .. }) | Element::Instance(Instance { matrix, .. }) | Element::Text(Text { matrix, .. }) | Element::Bitmap(BitmapInstance { matrix, .. }) = el {
        let (x, y) = matrix.apply(fixed);
        *matrix = Matrix::translate(x - fixed.0, y - fixed.1);
    }
}

/// Transforms `el` by `m`, given in its timeline's coordinates: what it draws
/// lands where `m` maps it. Like `hit::translate`, it changes matrices, never
/// edges. An instance's transformation point is in its symbol's coordinates,
/// so it goes along.
pub fn transform(el: &mut Element, m: &Matrix) {
    match el {
        // ponytail: strokes go through the matrix too, so a stretched or skewed shape's strokes
        // stretch with it, as inside a symbol. Transform the edges instead to keep widths even.
        Element::Shape(Shape { matrix, .. }) | Element::Instance(Instance { matrix, .. }) | Element::Text(Text { matrix, .. }) | Element::Bitmap(BitmapInstance { matrix, .. }) => {
            *matrix = *m * *matrix
        }
        Element::Group(members) => members.iter_mut().for_each(|el| transform(el, m)),
        // ponytail: like `hit::translate`, unsupported elements (kept as raw XML) stay as they are.
        // Parse their matrix to transform them.
        Element::Unsupported { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::FRAC_PI_2;

    use super::*;
    use crate::doc::{ColorTransform, Document, Extra, Loop, Paint, SymbolKind};
    use crate::{draw, hit};

    fn close(a: Point, b: Point) -> bool {
        (a.0 - b.0).abs() < 1e-4 && (a.1 - b.1).abs() < 1e-4
    }

    #[test]
    fn about_a_pivot() {
        let quarter = Matrix::rotate(FRAC_PI_2).about((10.0, 10.0));
        assert!(close(quarter.apply((10.0, 10.0)), (10.0, 10.0)), "the pivot stays");
        assert!(close(quarter.apply((20.0, 10.0)), (10.0, 20.0)), "right of it turns to below: clockwise on screen");
        assert!(close(Matrix::scale(2.0, 3.0).about((1.0, 1.0)).apply((2.0, 2.0)), (3.0, 4.0)));
        let lean = Matrix::skew(0.5, 0.0).about((0.0, 10.0));
        assert!(close(lean.apply((0.0, 0.0)), (-5.0, 0.0)) && close(lean.apply((4.0, 10.0)), (4.0, 10.0)));
        assert!(close(Matrix::skew(0.0, 2.0).apply((1.0, 1.0)), (1.0, 3.0)));
        assert!(close(Matrix::translate(3.0, -4.0).apply((1.0, 1.0)), (4.0, -3.0)));
    }

    #[test]
    fn inverse_undoes() {
        let m = Matrix { a: 2.0, b: 1.0, c: -0.5, d: 3.0, tx: 7.0, ty: -2.0 };
        let inv = m.inverse().unwrap();
        for p in [(0.0, 0.0), (3.0, -4.0), (100.0, 25.0)] {
            assert!(close(inv.apply(m.apply(p)), p) && close(m.apply(inv.apply(p)), p), "{p:?}");
        }
        assert_eq!(Matrix::scale(0.0, 1.0).inverse(), None);
        assert_eq!(Matrix { a: 1.0, b: 2.0, c: 2.0, d: 4.0, tx: 0.0, ty: 0.0 }.inverse(), None, "flat onto a line");
    }

    #[test]
    fn transforms_matrices_through_groups() {
        let square = draw::rectangle([(0.0, 0.0), (10.0, 10.0)], 0.0, None, Some(Paint::Solid([1.0; 4])));
        let instance = Instance {
            symbol: "Square".into(),
            kind: SymbolKind::Graphic,
            matrix: Matrix::translate(20.0, 0.0),
            pivot: (5.0, 5.0),
            color: ColorTransform::IDENTITY,
            looping: Loop::Loop,
            first_frame: 0,
            extra: Extra::default(),
        };
        let mut group = Element::Group(vec![Element::Shape(square.clone()), Element::Instance(instance.clone())]);
        let m = Matrix::rotate(FRAC_PI_2).about((15.0, 5.0));
        transform(&mut group, &m);
        let Element::Group(members) = &group else { panic!("not a group") };
        assert_eq!(members[0], Element::Shape(Shape { matrix: m, ..square }));
        assert_eq!(members[1], Element::Instance(Instance { matrix: m * instance.matrix, ..instance }), "the pivot stays local");

        let video = Element::Unsupported { name: "DOMVideoInstance".into(), xml: "<DOMVideoInstance/>".into() };
        let mut unknown = video.clone();
        transform(&mut unknown, &m);
        assert_eq!(unknown, video);
    }

    #[test]
    fn parts_round_trip_and_removing_a_transform() {
        let m = Matrix::rotate(0.5) * Matrix::skew(0.3, 0.0) * Matrix::scale(2.0, 0.5);
        let back = Matrix::from_parts(m.parts());
        for (x, y) in [(m.a, back.a), (m.b, back.b), (m.c, back.c), (m.d, back.d)] {
            assert!((x - y).abs() < 1e-5, "{m:?} came back as {back:?}");
        }
        let (_, _, kx, ky) = Matrix::IDENTITY.parts();
        assert!(kx.is_sign_positive() && ky.is_sign_positive(), "no -0.0° in the Transform panel");
        let (sx, sy, kx, ky) = Matrix::rotate(0.5).parts();
        assert!((sx - 1.0).abs() < 1e-6 && (sy - 1.0).abs() < 1e-6 && (kx - 0.5).abs() < 1e-6 && (ky - 0.5).abs() < 1e-6);

        let square = draw::rectangle([(0.0, 0.0), (10.0, 10.0)], 0.0, None, Some(Paint::Solid([1.0; 4])));
        let turned = Matrix::translate(50.0, 20.0) * m;
        let mut el = Element::Shape(Shape { matrix: turned, ..square });
        remove_transform(&mut el, (5.0, 5.0));
        let Element::Shape(s) = &el else { panic!("not a shape") };
        assert!(close(s.matrix.apply((5.0, 5.0)), turned.apply((5.0, 5.0))), "the middle stays");
        assert_eq!([s.matrix.a, s.matrix.b, s.matrix.c, s.matrix.d], [1.0, 0.0, 0.0, 1.0]);
    }

    /// Seen through its inverse matrix, a turned shape's box is the one it was drawn with.
    #[test]
    fn local_bounds_through_the_inverse() {
        let mut shape = draw::rectangle([(0.0, 0.0), (40.0, 10.0)], 0.0, None, Some(Paint::Solid([1.0; 4])));
        let turn = Matrix::rotate(0.5).about((20.0, 5.0));
        shape.matrix = turn;
        let mut doc = Document::new(100.0, 100.0, 24.0);
        doc.scenes[0].layers[0].frames[0].elements.push(Element::Shape(shape));
        let r = [hit::ElementRef { layer: 0, keyframe: 0, element: 0 }];
        let local = hit::bounds_through(&doc, &doc.scenes[0], 0, &r, turn.inverse().unwrap()).unwrap();
        assert!(close(local.min, (0.0, 0.0)) && close(local.max, (40.0, 10.0)), "{local:?}");
        let drawn = hit::bounds(&doc, &doc.scenes[0], 0, &r).unwrap();
        assert!(drawn.max.1 - drawn.min.1 > 20.0, "on stage it's turned: {drawn:?}");
    }
}
