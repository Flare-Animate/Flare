use super::*;
use egui::epaint::{CircleShape, Mesh, RectShape, Stroke};
use egui::{Color32, Rect, Vec2};

fn near(a: Pos2, b: Pos2) -> bool {
    (a - b).length() < 1e-3
}

const PIVOT: Pos2 = Pos2::new(10.0, 10.0);

fn quarter() -> Rot2 {
    Rot2::from_angle(std::f32::consts::FRAC_PI_2)
}

#[test]
fn points_turn_about_the_pivot() {
    let mut line = Shape::line_segment(
        [Pos2::new(20.0, 10.0), Pos2::new(10.0, 10.0)],
        Stroke::new(1.0, Color32::WHITE),
    );
    turn_shape(&mut line, PIVOT, quarter());
    let Shape::LineSegment { points, .. } = line else {
        panic!("still a line");
    };
    assert!(near(points[0], Pos2::new(10.0, 20.0)), "{points:?}");
    assert!(near(points[1], PIVOT), "the pivot stays");
    let mut circle = Shape::Circle(CircleShape::filled(Pos2::new(10.0, 0.0), 3.0, Color32::RED));
    turn_shape(&mut circle, PIVOT, quarter());
    let Shape::Circle(c) = circle else {
        panic!("still a circle");
    };
    assert!(near(c.center, Pos2::new(20.0, 10.0)), "{:?}", c.center);
}

#[test]
fn rectangles_and_text_keep_their_kind_and_gain_the_angle() {
    let rect = Rect::from_center_size(Pos2::new(30.0, 10.0), Vec2::new(8.0, 4.0));
    let mut shape = Shape::Rect(RectShape::filled(rect, 0.0, Color32::WHITE));
    turn_shape(&mut shape, PIVOT, quarter());
    let Shape::Rect(r) = shape else {
        panic!("still a rectangle");
    };
    assert!(near(r.rect.center(), Pos2::new(10.0, 30.0)), "{:?}", r.rect);
    assert_eq!(
        r.rect.size(),
        Vec2::new(8.0, 4.0),
        "the size is kept; the angle turns it"
    );
    assert!((r.angle - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
}

#[test]
fn meshes_and_nested_shapes_turn_every_corner() {
    let mut mesh = Mesh::default();
    mesh.add_colored_rect(
        Rect::from_min_max(Pos2::new(10.0, 10.0), Pos2::new(20.0, 12.0)),
        Color32::WHITE,
    );
    let mut shape = Shape::Vec(vec![Shape::mesh(mesh), Shape::Noop]);
    turn_shape(&mut shape, PIVOT, quarter());
    let Shape::Vec(shapes) = shape else {
        panic!("still a list");
    };
    let Shape::Mesh(mesh) = &shapes[0] else {
        panic!("still a mesh");
    };
    let xs: Vec<f32> = mesh.vertices.iter().map(|v| v.pos.x).collect();
    // A 10 by 2 strip right of the pivot becomes a 2 by 10 strip below it, reaching left.
    assert!(
        xs.iter().all(|x| (8.0 - 1e-3..=10.0 + 1e-3).contains(x)),
        "{xs:?}"
    );
}

#[test]
fn turning_painted_shapes_touches_only_those_after_the_mark() {
    let ctx = Context::default();
    let layer = LayerId::background();
    let mut start = ShapeIdx(0);
    let mut out = ctx.run_ui(Default::default(), |ui| {
        let painter = ui.ctx().layer_painter(layer);
        let at = Pos2::new(20.0, 10.0);
        painter.circle_filled(at, 1.0, Color32::RED);
        start = painted_so_far(ui.ctx(), layer);
        painter.circle_filled(at, 1.0, Color32::BLUE);
        turn_painted(ui.ctx(), layer, start, PIVOT, std::f32::consts::FRAC_PI_2);
        ui.ctx().graphics(|g| {
            let centers: Vec<Pos2> = g
                .get(layer)
                .expect("layer")
                .all_entries()
                .filter_map(|c| match &c.shape {
                    Shape::Circle(c) => Some(c.center),
                    _ => None,
                })
                .collect();
            assert!(near(centers[0], at), "before the mark: untouched");
            assert!(near(centers[1], Pos2::new(10.0, 20.0)), "after it: turned");
        });
    });
    out.textures_delta.clear();
    assert!(start.0 >= 1);
}
