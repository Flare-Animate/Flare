#![allow(clippy::expect_used)]

use super::*;
use egui::epaint::Mesh;

fn stroke(points: &[(f32, f32)], half: f32) -> PreviewStroke {
    PreviewStroke {
        points: points.iter().map(|&(x, y)| Pos2::new(x, y)).collect(),
        halves: vec![half; points.len()],
        color: Color32::RED,
        sides: None,
    }
}

/// The box round the fully painted vertices, without the fading edge.
fn solid_bounds(m: &Mesh) -> Rect {
    Rect::from_points(
        &m.vertices
            .iter()
            .filter(|v| v.color.a() == 255)
            .map(|v| v.pos)
            .collect::<Vec<_>>(),
    )
}

/// The opacity a GPU paints at `p`: the colour of the triangle whose inside holds the point,
/// blended across it, as a pixel centre takes it (no anti-aliasing of its own).
fn coverage_at(m: &Mesh, p: Pos2) -> f32 {
    let d = |a: Pos2, b: Pos2, c: Pos2| (a.x - c.x) * (b.y - c.y) - (b.x - c.x) * (a.y - c.y);
    let mut best = 0.0_f32;
    for t in m.indices.chunks(3) {
        let v = [
            &m.vertices[t[0] as usize],
            &m.vertices[t[1] as usize],
            &m.vertices[t[2] as usize],
        ];
        let area = d(v[0].pos, v[1].pos, v[2].pos);
        if area.abs() < 1e-9 {
            continue;
        }
        let w = [
            d(p, v[1].pos, v[2].pos) / area,
            d(p, v[2].pos, v[0].pos) / area,
            d(p, v[0].pos, v[1].pos) / area,
        ];
        if w.iter().all(|w| *w >= 0.0) {
            let alpha: f32 = (0..3).map(|k| w[k] * f32::from(v[k].color.a())).sum();
            best = best.max(alpha / 255.0);
        }
    }
    best
}

#[test]
fn the_mesh_covers_the_stroke_end_to_end_and_fades_over_one_pixel_at_its_edge() {
    let m = mesh(&[stroke(&[(0.0, 0.0), (100.0, 0.0)], 5.0)], |p| p, 1.0);
    let solid = solid_bounds(&m);
    assert!((solid.min.x - -4.5).abs() < 0.01 && (solid.max.x - 104.5).abs() < 0.01);
    assert!((solid.min.y - -4.5).abs() < 0.01 && (solid.max.y - 4.5).abs() < 0.01);
    let all = m.calc_bounds();
    assert!((all.min.x - -5.5).abs() < 0.01 && (all.max.x - 105.5).abs() < 0.01);
    assert!((all.min.y - -5.5).abs() < 0.01 && (all.max.y - 5.5).abs() < 0.01);
    // Half way into the fade is half covered, as a stroke edge through a pixel's middle is.
    assert!((coverage_at(&m, Pos2::new(50.0, 5.0)) - 0.5).abs() < 0.02);
    assert_eq!(coverage_at(&m, Pos2::new(50.0, 0.0)), 1.0);
}

#[test]
fn a_hairline_is_one_unbroken_faint_line_not_dots() {
    // A tenth of a pixel wide: a pixel centre is inside the bare outline almost nowhere, which
    // is what drew a light touch as dashes and dots.
    let m = mesh(&[stroke(&[(0.0, 0.3), (60.0, 7.3)], 0.05)], |p| p, 1.0);
    let mut faintest = f32::MAX;
    for x in 1..60 {
        let column = (-2..12)
            .map(|y| coverage_at(&m, Pos2::new(x as f32 + 0.5, y as f32 + 0.5)))
            .fold(0.0, f32::max);
        faintest = faintest.min(column);
    }
    assert!(
        faintest > 0.03,
        "every pixel column has some paint: {faintest}"
    );
    assert!(
        m.vertices.iter().all(|v| v.color.a() <= 26),
        "no more paint than its width"
    );
}

#[test]
fn zoom_scales_the_width_and_the_mesh_indexes_only_vertices_it_has() {
    let m = mesh(
        &[stroke(&[(0.0, 0.0), (10.0, 10.0), (20.0, 0.0)], 2.0)],
        |p| Pos2::new(p.x * 3.0, p.y * 3.0),
        1.0,
    );
    assert!(m.calc_bounds().height() > 6.0 * 3.0 - 0.5 - 1.0);
    let top = m.vertices.len() as u32;
    assert!(m.indices.iter().all(|i| *i < top));
    assert_eq!(m.indices.len() % 3, 0);
}

#[test]
fn non_finite_points_make_no_geometry_and_an_empty_stroke_makes_none() {
    let m = mesh(&[stroke(&[(f32::NAN, 0.0), (1.0, 1.0)], 2.0)], |p| p, 1.0);
    assert!(m.vertices.iter().all(|v| v.pos.is_finite()));
    assert!(m.indices.iter().all(|i| (*i as usize) < m.vertices.len()));
    assert!(mesh(&[stroke(&[], 2.0)], |p| p, 1.0).is_empty());
    let odd_pixel = mesh(&[stroke(&[(0.0, 0.0), (9.0, 0.0)], 2.0)], |p| p, f32::NAN);
    assert!(odd_pixel.vertices.iter().all(|v| v.pos.is_finite()));
}

#[test]
fn wider_points_make_a_wider_stroke_so_pressure_shows() {
    let mut s = stroke(&[(0.0, 0.0), (50.0, 0.0)], 1.0);
    s.halves[1] = 9.0;
    let m = mesh(&[s], |p| p, 1.0);
    assert!((solid_bounds(&m).max.y - 8.5).abs() < 0.01);
    assert!((m.calc_bounds().max.y - 9.5).abs() < 0.01);
}

#[test]
fn a_brush_preview_draws_the_outline_it_will_commit() {
    let samples: Vec<flare_input::InputSample> = (0..=40)
        .map(|i| {
            let x = f64::from(i) * 3.0;
            flare_input::InputSample::new(x, (x * 0.05).sin() * 20.0, 0.8, Default::default())
                .expect("sample")
        })
        .collect();
    let brush = flare_tools::brush::BrushStroke {
        taper: flare_tools::brush::Taper::new(15.0, 15.0),
        ..flare_tools::brush::BrushStroke::new(14.0, flare_input::PressureCurve::new(0.1, 1.0))
    };
    let sides = brush.sides(&samples, 1.0).expect("sides");
    let mut s = stroke(&[], 0.0);
    s.sides = Some(PreviewSides::of(&sides));
    let m = mesh(&[s], |p| p, 1.0);
    assert!(m.indices.iter().all(|i| (*i as usize) < m.vertices.len()));
    assert_eq!(m.indices.len() % 3, 0);
    // Every point of the committed outline is painted at least half way (it lies in the middle
    // of the fading edge), and nothing reaches further out than the fade.
    let corners: Vec<Pos2> = sides
        .left
        .iter()
        .chain(&sides.right)
        .chain(&sides.end_cap)
        .chain(&sides.start_cap)
        .map(|p| Pos2::new(p.x as f32, p.y as f32))
        .collect();
    let along = sides.left.len() / 2;
    let middle = Pos2::new(sides.left[along].x as f32, sides.left[along].y as f32);
    assert!(coverage_at(&m, middle) > 0.4, "{}", coverage_at(&m, middle));
    let outline = Rect::from_points(&corners);
    let drawn = m.calc_bounds();
    assert!(drawn.min.x >= outline.min.x - 1.0 && drawn.max.x <= outline.max.x + 1.0);
    assert!(drawn.min.y >= outline.min.y - 1.0 && drawn.max.y <= outline.max.y + 1.0);
    assert!(outline.shrink(0.6).min.x >= drawn.min.x && outline.shrink(0.6).max.y <= drawn.max.y);
}
