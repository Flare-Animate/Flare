use super::*;

#[test]
fn fill_styles_sit_on_the_correct_side_of_every_edge() {
    let cases: Vec<(&str, Shape)> = vec![
        ("ring", ring(false)),
        ("ring wound the other way", ring(true)),
        (
            "square",
            rect(10.0, 10.0, 30.0, 20.0, Rgba8::opaque(1, 2, 3)),
        ),
        (
            "triangle wound the other way",
            Shape::new(
                ShapeId::new(),
                vec![
                    PathSegment::MoveTo(pt(10.0, 10.0)),
                    PathSegment::LineTo(pt(10.0, 50.0)),
                    PathSegment::LineTo(pt(50.0, 10.0)),
                    PathSegment::Close,
                ],
                Rgba8::opaque(9, 9, 9),
            )
            .expect("triangle"),
        ),
    ];
    for (name, shape) in cases {
        let doc = document(
            80,
            60,
            vec![one_frame_scene("S", vec![shape.clone()], 1)],
            vec![],
        );
        let parsed = parse(
            &publish_swf(&doc, &PublishOptions::default())
                .expect("publish")
                .bytes,
        );
        let Some(Tag::DefineShape(defined)) = parsed
            .tags
            .iter()
            .find(|t| matches!(t, Tag::DefineShape(_)))
        else {
            panic!("no shape was published");
        };
        let segs = segments(defined);
        let (mut checked, mut filled) = (0, 0);
        for gy in 0..40 {
            for gx in 0..55 {
                let p = (f64::from(gx) * 1.5 + 0.37, f64::from(gy) * 1.5 + 0.41);
                let Some(style) = swf_fill_at(&segs, p, 0.75) else {
                    continue;
                };
                let expected = original_contains(&shape, p);
                // Style 1 is the fill; style 0 means "no fill on that side".
                assert_eq!(
                    style == 1,
                    expected,
                    "{name}: at {p:?} SWF says style {style}, the shape says {expected}"
                );
                checked += 1;
                filled += usize::from(expected);
            }
        }
        assert!(
            checked > 300 && filled > 20 && filled < checked,
            "{name}: a meaningful sample ({filled}/{checked})"
        );
    }
}
