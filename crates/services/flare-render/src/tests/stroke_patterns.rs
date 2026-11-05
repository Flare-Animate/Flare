use super::*;
use flare_document::StrokeStyle;

const BAND: std::ops::RangeInclusive<u32> = 0..=40;

/// A red horizontal line, 4 wide, down the middle of a 100 by 40 frame, in `style`.
fn line_in(style: StrokeStyle) -> crate::Image {
    let stroke = flare_document::Stroke::new(4.0, RED)
        .expect("stroke")
        .with_cap(flare_document::Cap::Butt)
        .with_style(style);
    let shape = Shape::line(ShapeId::new(), (2.0, 20.0), (98.0, 20.0), stroke).expect("line");
    let (doc, scene) = scene_with(vec![span(Content::Shape(shape))], vec![]);
    draw_sized(&doc, scene, Rgba8::WHITE, 100, 40)
}

/// The rows (y) where red covers at least half of a pixel: edges blend with the white background.
fn red_rows(image: &crate::Image) -> Vec<u32> {
    let mut rows = Vec::new();
    for y in BAND {
        for x in 0..100 {
            if image.pixel(x, y).is_some_and(|p| p.g < 128) {
                rows.push(y);
                break;
            }
        }
    }
    rows
}

#[test]
fn every_geometric_style_draws_a_different_mark_from_solid() {
    let solid = line_in(StrokeStyle::Solid);
    for style in StrokeStyle::ALL.into_iter().filter(|s| s.is_geometric()) {
        let image = line_in(style);
        let differs = (0..100)
            .flat_map(|x| BAND.map(move |y| (x, y)))
            .any(|(x, y)| image.pixel(x, y) != solid.pixel(x, y));
        assert!(differs, "{style} is not drawn like a solid line");
        assert!(!red_rows(&image).is_empty(), "{style} draws red");
    }
}

#[test]
fn a_zigzag_reaches_both_sides_of_the_line() {
    let rows = red_rows(&line_in(StrokeStyle::Zigzag));
    assert!(rows.iter().any(|y| *y <= 17), "above the line: {rows:?}");
    assert!(rows.iter().any(|y| *y >= 23), "below the line: {rows:?}");
}

#[test]
fn a_rope_stays_within_its_band() {
    let rows = red_rows(&line_in(StrokeStyle::Rope));
    assert!(
        rows.iter().all(|y| (16..=24).contains(y)),
        "rope rows: {rows:?}"
    );
    assert!(rows.len() >= 3, "two strands cross the band: {rows:?}");
}

#[test]
fn a_hatched_line_keeps_its_spine_and_crosses_it() {
    let image = line_in(StrokeStyle::Hatched);
    assert_eq!(image.pixel(50, 20), Some(RED), "the line itself is there");
    let rows = red_rows(&image);
    assert!(
        rows.iter().any(|y| *y <= 17) && rows.iter().any(|y| *y >= 23),
        "hatch crosses: {rows:?}"
    );
}

#[test]
fn dash_styles_are_unchanged_by_the_geometric_path() {
    let dashed = line_in(StrokeStyle::Dashed);
    let covered = (2..98)
        .filter(|x| dashed.pixel(*x, 20) == Some(RED))
        .count();
    assert!(
        (45..65).contains(&covered),
        "dashed still has gaps: {covered}"
    );
}
