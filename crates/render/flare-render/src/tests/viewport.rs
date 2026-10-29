use super::*;

#[test]
fn strokes_are_hittable_where_they_are_drawn_including_lines_and_outlines() {
    use flare_document::Stroke as DocStroke;
    let ink = Rgba8::opaque(0, 0, 0);
    let line = Shape::line(
        ShapeId::new(),
        (10.0, 10.0),
        (90.0, 10.0),
        DocStroke::new(6.0, ink).expect("stroke"),
    )
    .expect("line");
    let outline = Shape::rect(ShapeId::new(), 10.0, 40.0, 40.0, 40.0, Rgba8::TRANSPARENT)
        .expect("rect")
        .with_stroke(DocStroke::new(4.0, ink).expect("stroke"));
    let (doc, id) = scene_with(
        vec![span(Content::Shape(line)), span(Content::Shape(outline))],
        vec![],
    );
    let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
    let at = |x: f64, y: f64| hit_test_all(&frame, DVec2::new(x, y));
    // Paint order is bottom layer first, so the outline is shape 0 and the line shape 1.
    // On the line (within half the width) and just past it, including the round cap.
    assert_eq!(at(50.0, 10.0), vec![1]);
    assert_eq!(at(50.0, 12.9), vec![1]);
    assert!(at(50.0, 13.5).is_empty(), "outside the 3px half-width");
    assert_eq!(
        at(92.0, 10.0),
        vec![1],
        "the round cap extends past the end"
    );
    assert!(at(95.0, 10.0).is_empty());
    // An outline-only rectangle: on its edge yes, in its empty middle no.
    assert_eq!(at(10.0, 60.0), vec![0]);
    assert_eq!(at(31.0, 41.0), vec![0]);
    assert!(
        at(30.0, 60.0).is_empty(),
        "transparent fill has nothing to click inside"
    );
}

#[test]
fn a_stroked_filled_shape_is_hit_on_its_fill_and_just_outside_on_the_stroke() {
    use flare_document::Stroke as DocStroke;
    let both = Shape::rect(ShapeId::new(), 20.0, 20.0, 20.0, 20.0, RED)
        .expect("rect")
        .with_stroke(DocStroke::new(8.0, Rgba8::opaque(0, 0, 0)).expect("stroke"));
    let (doc, id) = scene_with(vec![span(Content::Shape(both))], vec![]);
    let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
    let at = |x: f64, y: f64| hit_test_all(&frame, DVec2::new(x, y));
    assert_eq!(at(30.0, 30.0), vec![0], "inside the fill");
    assert_eq!(
        at(18.0, 30.0),
        vec![0],
        "on the stroke outside the fill edge"
    );
    assert!(at(14.0, 30.0).is_empty(), "beyond the stroke");
}

#[test]
fn stroke_width_follows_the_instances_scale() {
    use flare_document::Stroke as DocStroke;
    let art = SymbolId::new();
    let line = Shape::line(
        ShapeId::new(),
        (0.0, 0.0),
        (20.0, 0.0),
        DocStroke::new(2.0, Rgba8::opaque(0, 0, 0)).expect("stroke"),
    )
    .expect("line");
    let art_tl = Timeline::new(vec![
        Layer::new(LayerId::new(), "L", vec![span(Content::Shape(line))]).expect("l"),
    ]);
    let scale = Transform2D::new(
        DAffine2::from_scale(DVec2::new(3.0, 3.0))
            * DAffine2::from_translation(DVec2::new(10.0, 10.0)),
    )
    .expect("finite");
    let instance = Instance::new(InstanceId::new(), art, scale);
    let (doc, id) = scene_with(
        vec![span(Content::Instance(instance))],
        vec![Symbol::new(art, "Art", art_tl)],
    );
    let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
    // The 2-unit stroke is 6px wide at 3x, so 2.9px from the line's centre is still on it.
    assert_eq!(hit_test_all(&frame, DVec2::new(60.0, 32.9)), vec![0]);
    assert!(hit_test_all(&frame, DVec2::new(60.0, 33.5)).is_empty());
}

#[test]
fn a_linear_gradient_blends_between_its_two_points() {
    let (doc, id) = scene_with(
        vec![span(Content::Shape(gradient_rect(100.0, 20.0)))],
        vec![],
    );
    let image = draw_sized(&doc, id, Rgba8::WHITE, 100, 20);
    let at = |x| image.pixel(x, 10).expect("pixel");
    assert!(at(2).r > 230 && at(2).b < 25, "red end: {:?}", at(2));
    assert!(at(97).b > 230 && at(97).r < 25, "blue end: {:?}", at(97));
    let mid = at(50);
    assert!(
        mid.r.abs_diff(127) < 20 && mid.b.abs_diff(127) < 20,
        "middle: {mid:?}"
    );
    assert!(at(30).r > at(70).r, "red falls off left to right");
}

#[test]
fn the_gradient_follows_the_instance_transform() {
    // The same 100-wide gradient, scaled to half size by the instance: the blue end now
    // sits at x = 50, not 100.
    let art = SymbolId::new();
    let art_tl = Timeline::new(vec![
        Layer::new(
            LayerId::new(),
            "L",
            vec![span(Content::Shape(gradient_rect(100.0, 20.0)))],
        )
        .expect("l"),
    ]);
    let half = Transform2D::new(DAffine2::from_scale(DVec2::splat(0.5))).expect("finite");
    let instance = Instance::new(InstanceId::new(), art, half);
    let (doc, id) = scene_with(
        vec![span(Content::Instance(instance))],
        vec![Symbol::new(art, "Art", art_tl)],
    );
    let image = draw_sized(&doc, id, Rgba8::WHITE, 100, 20);
    let end = image.pixel(48, 5).expect("pixel");
    assert!(end.b > 220 && end.r < 35, "blue at the scaled end: {end:?}");
    assert_eq!(
        image.pixel(70, 5),
        Some(Rgba8::WHITE),
        "nothing beyond the shape"
    );
}

#[test]
fn a_gradient_over_a_clear_fill_is_hittable_and_a_clear_one_is_not() {
    use flare_core::DocumentPoint;
    let visible = gradient_rect(40.0, 40.0);
    let p = |x| DocumentPoint::new(x, 0.0).expect("finite");
    let clear = Shape::rect(ShapeId::new(), 0.0, 0.0, 40.0, 40.0, Rgba8::WHITE)
        .expect("rect")
        .with_gradient(
            flare_document::Gradient::linear(
                p(0.0),
                p(40.0),
                Rgba8::TRANSPARENT,
                Rgba8::TRANSPARENT,
            )
            .expect("gradient"),
        );
    for (shape, hit) in [(visible, true), (clear, false)] {
        let (doc, id) = scene_with(vec![span(Content::Shape(shape))], vec![]);
        let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("frame");
        assert_eq!(
            hit_test(&frame, glamx::DVec2::new(20.0, 20.0)).is_some(),
            hit
        );
    }
}

#[test]
fn a_viewport_draws_the_chosen_part_of_the_stage_at_the_chosen_resolution() {
    // Stage 0..100: a red square at 40..60 and a blue one at 80..100.
    let blue = Rgba8::opaque(0, 0, 255);
    let (doc, id) = scene_with(
        vec![
            span(rect(40.0, 40.0, 20.0, 20.0, RED)),
            span(rect(80.0, 80.0, 20.0, 20.0, blue)),
        ],
        vec![],
    );
    let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");

    // The right-bottom quarter (50..100) shown at 2 pixels per unit: 100 x 100 pixels.
    let view = Viewport {
        x: 50.0,
        y: 50.0,
        width: 50.0,
        height: 50.0,
    };
    let image = render_viewport(&frame, &view, 100, 100, Rgba8::WHITE).expect("render");
    assert_eq!((image.width(), image.height()), (100, 100));
    assert_eq!(
        image.pixel(5, 5),
        Some(RED),
        "stage (52, 52) is inside the red square"
    );
    assert_eq!(
        image.pixel(25, 25),
        Some(Rgba8::WHITE),
        "stage (62, 62) is between the squares"
    );
    assert_eq!(image.pixel(80, 80), Some(blue), "stage (90, 90)");
    assert_eq!(
        image.pixel(19, 5),
        Some(RED),
        "the red square ends at stage 60, which is pixel 20"
    );
    assert_eq!(image.pixel(20, 5), Some(Rgba8::WHITE));

    // The whole-stage viewport is exactly what `render` draws.
    let full =
        render_viewport(&frame, &Viewport::full(100, 100), 100, 100, Rgba8::WHITE).expect("full");
    assert_eq!(
        full,
        render(&frame, 100, 100, Rgba8::WHITE).expect("render")
    );
}

#[test]
fn viewports_work_with_strokes_and_gradients_and_reject_bad_requests() {
    let shape = gradient_rect(100.0, 20.0);
    let (doc, id) = scene_with(vec![span(Content::Shape(shape))], vec![]);
    let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
    // Zoom into the right half: the blue end of the gradient fills the view.
    let right = Viewport {
        x: 90.0,
        y: 0.0,
        width: 10.0,
        height: 20.0,
    };
    let image = render_viewport(&frame, &right, 50, 100, Rgba8::WHITE).expect("render");
    let end = image.pixel(45, 50).expect("pixel");
    assert!(end.b > 220 && end.r < 40, "{end:?}");
    for bad in [
        Viewport {
            x: f64::NAN,
            ..right
        },
        Viewport {
            width: 0.0,
            ..right
        },
        Viewport {
            height: -1.0,
            ..right
        },
        Viewport {
            y: f64::INFINITY,
            ..right
        },
    ] {
        assert!(
            render_viewport(&frame, &bad, 10, 10, Rgba8::WHITE).is_err(),
            "{bad:?}"
        );
    }
}

#[test]
fn premultiplied_output_matches_the_straight_image_within_rounding() {
    let half = Rgba8::new(200, 100, 50, 128);
    let (doc, id) = scene_with(vec![span(rect(2.0, 2.0, 10.0, 10.0, half))], vec![]);
    let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
    let view = Viewport::full(20, 20);
    let straight = render_viewport(&frame, &view, 20, 20, Rgba8::TRANSPARENT).expect("straight");
    let premul = render_viewport_premultiplied(&frame, &view, 20, 20, Rgba8::TRANSPARENT)
        .expect("premultiplied");
    assert_eq!((premul.width(), premul.height()), (20, 20));
    let s = straight.pixel(5, 5).expect("pixel");
    let i = (5 * 20 + 5) * 4;
    let p = &premul.rgba()[i..i + 4];
    assert_eq!(p[3], s.a);
    for (premultiplied, straight_channel) in p[..3].iter().zip([s.r, s.g, s.b]) {
        let expected = (u32::from(straight_channel) * u32::from(s.a) + 127) / 255;
        assert!(
            u32::from(*premultiplied).abs_diff(expected) <= 1,
            "{p:?} vs {s:?}"
        );
    }
    // Outside the square both are fully transparent black.
    assert_eq!(&premul.rgba()[..4], &[0, 0, 0, 0]);
    // An opaque render is the same bytes either way.
    let opaque_a = render_viewport(&frame, &view, 20, 20, Rgba8::WHITE).expect("opaque");
    let opaque_b = render_viewport_premultiplied(&frame, &view, 20, 20, Rgba8::WHITE)
        .expect("opaque premultiplied");
    assert_eq!(opaque_a.rgba(), opaque_b.rgba());
}

#[test]
fn small_frames_and_small_machines_stay_on_one_thread() {
    assert_eq!(render_threads(0, 16), 0);
    assert_eq!(
        render_threads(PARALLEL_MIN_SHAPES - 1, 16),
        0,
        "too little work"
    );
    assert_eq!(render_threads(PARALLEL_MIN_SHAPES, 16), 4, "capped");
    assert_eq!(render_threads(100_000, 64), MAX_RENDER_THREADS);
    assert_eq!(
        render_threads(5_000, 4),
        3,
        "leaves one core for the caller"
    );
    assert_eq!(
        render_threads(5_000, 2),
        0,
        "two cores are not worth splitting"
    );
    assert_eq!(render_threads(5_000, 1), 0);
    assert_eq!(render_threads(5_000, 0), 0);
}
