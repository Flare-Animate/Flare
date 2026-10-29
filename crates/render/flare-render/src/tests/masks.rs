use super::*;

#[test]
fn threaded_rendering_matches_drawing_the_frame_in_pieces() {
    // Enough shapes to take the threaded path, overlapping and translucent so that the
    // order in which layers are blended matters.
    let spans: Vec<FrameSpan> = (0..PARALLEL_MIN_SHAPES + 50)
        .map(|i| {
            let x = (i % 30) as f64 * 3.0;
            let y = (i / 30) as f64 * 3.0;
            span(rect(
                x,
                y,
                14.0,
                14.0,
                Rgba8::new((i % 250) as u8, 90, 160, 140),
            ))
        })
        .collect();
    let (doc, id) = scene_with(spans, vec![]);
    let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
    let threaded = render(&frame, 96, 96, Rgba8::WHITE).expect("threaded");
    let alone = {
        let mut one = frame.clone();
        // The same frame drawn in pieces too small to thread, then composited in order.
        one.shapes.truncate(PARALLEL_MIN_SHAPES - 1);
        let first = render(&one, 96, 96, Rgba8::WHITE).expect("first");
        let rest_frame = flare_eval::Frame {
            shapes: frame.shapes[PARALLEL_MIN_SHAPES - 1..].to_vec(),
            ..frame.clone()
        };
        let mut image = first;
        image
            .composite(&render(&rest_frame, 96, 96, Rgba8::TRANSPARENT).expect("rest"))
            .expect("composite");
        image
    };
    let differing = threaded
        .rgba()
        .iter()
        .zip(alone.rgba())
        .filter(|(a, b)| a.abs_diff(**b) > 2)
        .count();
    assert_eq!(differing, 0, "threads must not change the picture");
}

#[test]
fn a_mask_shows_the_layer_below_only_where_the_mask_has_shapes() {
    // Art covers the whole 20x20 stage; the mask is the left-top 8x8 square.
    let (doc, id) = masked_doc((0.0, 0.0, 8.0, 8.0), (0.0, 0.0, 20.0, 20.0));
    let image = draw(&doc, id, Rgba8::WHITE);
    assert_eq!(image.pixel(4, 4), Some(RED), "inside the mask");
    assert_eq!(image.pixel(12, 4), Some(Rgba8::WHITE), "outside the mask");
    assert_eq!(image.pixel(4, 12), Some(Rgba8::WHITE));
    assert_eq!(image.pixel(15, 15), Some(Rgba8::WHITE));
}

#[test]
fn the_mask_shape_itself_is_not_drawn() {
    // The mask lies where the art is not: nothing at all shows.
    let (doc, id) = masked_doc((0.0, 0.0, 8.0, 8.0), (10.0, 10.0, 8.0, 8.0));
    let image = draw(&doc, id, Rgba8::WHITE);
    for (x, y) in [(4, 4), (14, 14), (4, 14)] {
        assert_eq!(image.pixel(x, y), Some(Rgba8::WHITE), "({x}, {y})");
    }
}

#[test]
fn masked_artwork_is_only_hittable_inside_the_mask() {
    let (doc, id) = masked_doc((0.0, 0.0, 8.0, 8.0), (0.0, 0.0, 20.0, 20.0));
    let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("frame");
    assert_eq!(hit_test_all(&frame, DVec2::new(4.0, 4.0)), vec![0]);
    assert!(hit_test_all(&frame, DVec2::new(15.0, 15.0)).is_empty());
}
