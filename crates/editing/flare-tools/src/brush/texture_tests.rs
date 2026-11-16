#![allow(clippy::expect_used)]

use flare_input::PointerKind;

use super::*;

fn wave() -> Vec<InputSample> {
    (0..200)
        .map(|i| {
            let x = f64::from(i) * 2.0;
            let pressure = 0.3 + 0.7 * ((f64::from(i) / 200.0) * std::f64::consts::PI).sin();
            InputSample::new(
                x,
                (x * 0.03).sin() * 30.0,
                pressure as f32,
                PointerKind::Pen,
            )
            .expect("sample")
        })
        .collect()
}

fn textured(bristles: u8, grain: f32) -> BrushStroke {
    BrushStroke {
        texture: Texture::new(bristles, grain),
        ..BrushStroke::new(24.0, PressureCurve::new(0.2, 1.0))
    }
}

fn subpaths(path: &[PathSegment]) -> usize {
    path.iter()
        .filter(|s| matches!(s, PathSegment::MoveTo(_)))
        .count()
}

#[test]
fn a_solid_brush_is_one_piece_with_round_ends_as_before() {
    let solid = BrushStroke::new(24.0, PressureCurve::new(0.2, 1.0));
    let pieces = solid.pieces(&wave(), 0.5);
    assert_eq!(pieces.len(), 1);
    assert_eq!(Some(pieces[0].clone()), solid.sides(&wave(), 0.5));
    assert_eq!(subpaths(&solid.outline(&wave(), 0.5).expect("outline")), 1);
}

#[test]
fn bristles_without_grain_run_the_whole_stroke_side_by_side() {
    let stroke = textured(6, 0.0);
    let pieces = stroke.pieces(&wave(), 0.5);
    assert_eq!(pieces.len(), 6);
    let whole = stroke.sides(&wave(), 0.5).expect("sides");
    for piece in &pieces {
        assert_eq!(piece.centre.len(), whole.centre.len());
        assert!(piece.end_cap.is_empty() && piece.start_cap.is_empty());
        // Every strand stays inside the stroke it is part of.
        for (i, at) in piece.centre.iter().enumerate() {
            let half = (whole.left[i] - whole.right[i]).length() / 2.0;
            assert!((piece.left[i] - whole.centre[i]).length() <= half + 1e-6);
            assert!((piece.right[i] - whole.centre[i]).length() <= half + 1e-6);
            assert!((*at - whole.centre[i]).length() <= half + 1e-6);
        }
    }
    let path = stroke.outline(&wave(), 0.5).expect("outline");
    assert_eq!(subpaths(&path), 6);
    assert!(
        flare_document::Shape::new(
            flare_core::ShapeId::new(),
            path,
            flare_core::Rgba8::TRANSPARENT
        )
        .is_ok()
    );
}

#[test]
fn strands_lie_in_their_own_lanes_and_do_not_overlap() {
    let pieces = textured(8, 0.0).pieces(&wave(), 0.5);
    let whole = textured(1, 0.0).sides(&wave(), 0.5).expect("sides");
    let i = whole.centre.len() / 2;
    let across = (whole.left[i] - whole.right[i]).normalize();
    // Each strand's extent across the line at one point, in order, never runs into the next.
    let mut spans: Vec<(f64, f64)> = pieces
        .iter()
        .map(|p| {
            let (a, b) = (
                (p.left[i] - whole.centre[i]).dot(across),
                (p.right[i] - whole.centre[i]).dot(across),
            );
            (a.min(b), a.max(b))
        })
        .collect();
    spans.sort_by(|x, y| x.0.total_cmp(&y.0));
    for pair in spans.windows(2) {
        assert!(pair[0].1 <= pair[1].0 + 1e-6, "{pair:?}");
    }
}

#[test]
fn grain_breaks_strands_into_more_pieces_and_more_grain_leaves_less() {
    let some = textured(8, 0.3).pieces(&wave(), 0.5);
    let lots = textured(8, 0.8).pieces(&wave(), 0.5);
    assert!(some.len() > 8, "broken: {}", some.len());
    let painted = |pieces: &[StrokeSides]| pieces.iter().map(|p| p.centre.len()).sum::<usize>();
    assert!(painted(&lots) < painted(&some));
    // Full grain at the lightest pressure leaves nothing to paint, which is not an outline.
    let light: Vec<InputSample> = (0..50)
        .map(|i| InputSample::new(f64::from(i), 0.0, 0.0, PointerKind::Pen).expect("sample"))
        .collect();
    let gone = BrushStroke {
        texture: Texture::new(1, 1.0),
        ..BrushStroke::new(24.0, PressureCurve::new(0.0, 1.0))
    };
    assert!(gone.outline(&light, 0.5).is_none());
}

#[test]
fn the_same_drag_makes_the_same_texture_and_another_start_another() {
    let stroke = textured(10, 0.5);
    assert_eq!(stroke.pieces(&wave(), 0.5), stroke.pieces(&wave(), 0.5));
    let moved: Vec<InputSample> = wave()
        .into_iter()
        .map(|s| InputSample { x: s.x + 7.0, ..s })
        .collect();
    let shifted = stroke.pieces(&moved, 0.5);
    let lengths = |p: &[StrokeSides]| p.iter().map(|s| s.centre.len()).collect::<Vec<_>>();
    assert_ne!(lengths(&shifted), lengths(&stroke.pieces(&wave(), 0.5)));
}

#[test]
fn a_texture_keeps_its_numbers_in_range_and_a_long_stroke_inside_the_segment_limit() {
    assert_eq!(Texture::new(200, 3.0), Texture::new(MAX_BRISTLES, 1.0));
    assert_eq!(Texture::new(4, f32::NAN).grain(), 0.0);
    assert!(Texture::new(1, 0.0).is_solid() && !Texture::new(2, 0.0).is_solid());
    let long: Vec<InputSample> = (0..200_000)
        .map(|i| {
            let x = f64::from(i) * 0.7;
            InputSample::new(x, (x * 0.2).sin() * 40.0, 0.9, PointerKind::Pen).expect("sample")
        })
        .collect();
    let path = textured(MAX_BRISTLES, 0.0)
        .outline(&long, 0.0)
        .expect("outline");
    assert!(path.len() < MAX_SEGMENTS, "{}", path.len());
}

#[test]
fn a_swatch_is_a_pen_stroke_that_presses_hardest_in_the_middle() {
    let samples = swatch_samples(200.0, 40.0);
    assert!(samples.iter().all(|s| s.kind == PointerKind::Pen));
    let middle = samples[samples.len() / 2].pressure;
    assert!(middle > 0.95 && samples[0].pressure < 0.1);
    assert!(
        samples
            .iter()
            .all(|s| (0.0..=200.0).contains(&s.x) && (0.0..=40.0).contains(&s.y))
    );
    for profile in BuiltinBrushProfile::ALL {
        let pieces = swatch(&profile.preset().stroke(10.0), 200.0, 40.0);
        assert!(!pieces.is_empty(), "{profile:?} draws something");
        assert_eq!(pieces.len() > 1, profile.is_textured(), "{profile:?}");
    }
    assert!(swatch_samples(f64::NAN, 40.0).iter().all(|s| s.x == 0.0));
}
