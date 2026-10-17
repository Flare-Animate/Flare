//! P2 group 3, pixel level: hairlines and stroke scale modes, HTML5 parity.
use ac_core::*;
use ac_render::html5::{publish, Html5Opts};
use ac_render::render_frame;

fn doc(mode: StrokeScale, hair: bool) -> Document {
    let mut d = Document { width: 300.0, height: 200.0, ..Default::default() };
    let mut sh = Shape::new(Path::rect(10.0, 20.0, 30.0, 30.0), None, Some(Stroke { color: Color::rgb(255, 0, 0), width: 10.0 }));
    sh.stroke_style.scale = mode;
    sh.stroke_style.hairline = hair;
    let mut el = Element::shape("", sh);
    el.matrix = Matrix::scale(3.0, 1.0);
    d.scene.layers[0].keyframes.insert(0, Keyframe::with(vec![el]));
    d.scene.layers[0].end = 1;
    d
}
fn red(d: &Document, s: f32, x: u32, y: u32) -> bool {
    let p = render_frame(d, 0, s).pixel(x, y).unwrap();
    p.green() < 128
}

// left edge of the rect sits at stage x = 30 after the 3x horizontal scale
#[test]
fn normal_stretches_the_pen() {
    let d = doc(StrokeScale::Normal, false);
    assert!(red(&d, 1.0, 18, 35) && red(&d, 1.0, 28, 35) && red(&d, 1.0, 42, 35));
}
#[test]
fn horizontal_keeps_vertical_edges_thin() {
    let d = doc(StrokeScale::Horizontal, false);
    assert!(red(&d, 1.0, 28, 35) && red(&d, 1.0, 32, 35));
    assert!(!red(&d, 1.0, 18, 35) && !red(&d, 1.0, 42, 35));
}
#[test]
fn none_keeps_the_pen_at_its_width() {
    let d = doc(StrokeScale::None, false);
    assert!(red(&d, 1.0, 28, 35) && !red(&d, 1.0, 18, 35));
}
#[test]
fn vertical_follows_horizontal_scale() {
    let d = doc(StrokeScale::Vertical, false);
    assert!(red(&d, 1.0, 18, 35));
    // horizontal edges keep their 10 px thickness: y 15..25 around the top edge
    assert!(red(&d, 1.0, 60, 18) && !red(&d, 1.0, 60, 10));
}
#[test]
fn scale_modes_follow_view_zoom() {
    let d = doc(StrokeScale::None, false);
    // at 2x zoom the pen is 20 device px wide: edge at device x 60
    assert!(red(&d, 2.0, 52, 70) && !red(&d, 2.0, 40, 70));
}
#[test]
fn hairline_is_one_device_pixel_at_any_zoom() {
    for z in [1.0f32, 2.0, 4.0] {
        let d = doc(StrokeScale::Normal, true);
        let ex = (30.0 * z) as u32;
        assert!(red(&d, z, ex, (35.0 * z) as u32) || red(&d, z, ex - 1, (35.0 * z) as u32));
        assert!(!red(&d, z, ex + 3, (35.0 * z) as u32) && !red(&d, z, ex.saturating_sub(4), (35.0 * z) as u32));
    }
}
#[test]
fn hairline_ignores_stroke_width_and_hostile_values() {
    let mut d = doc(StrokeScale::Normal, true);
    if let ElementKind::Shape(s) = &mut d.scene.layers[0].keyframes.get_mut(&0).unwrap().elements[0].kind {
        s.stroke = Some(Stroke { color: Color::rgb(255, 0, 0), width: f32::NAN });
    }
    let _ = render_frame(&d, 0, 1.0); // must not panic
    let z = doc(StrokeScale::None, false);
    let mut z = z;
    z.scene.layers[0].keyframes.get_mut(&0).unwrap().elements[0].matrix = Matrix::scale(0.0, 1.0);
    let _ = render_frame(&z, 0, 1.0);
}
fn movie_text(d: &Document) -> String {
    let files = publish(d, &Html5Opts::default());
    String::from_utf8(files.iter().find(|f| f.0 == "movie.js").unwrap().1.clone()).unwrap()
}
#[test]
fn html5_publish_carries_hairline_and_scale() {
    assert!(movie_text(&doc(StrokeScale::Normal, true)).contains("\"hl\":1"));
    let m = movie_text(&doc(StrokeScale::Horizontal, false));
    assert!(m.contains("\"sm\":\"h\"") && !m.contains("\"hl\""));
    assert!(movie_text(&doc(StrokeScale::None, false)).contains("\"sm\":\"n\""));
    let plain = movie_text(&doc(StrokeScale::Normal, false));
    assert!(!plain.contains("\"sm\"") && !plain.contains("\"hl\""));
}
