//! P2 group 3: XFL read/write of stroke scale modes, hairlines and hinting.
use ac_core::*;
use ac_xfl::{read_package, write_package};

fn doc(mode: StrokeScale, hair: bool, hint: bool) -> Document {
    let mut d = Document { width: 100.0, height: 100.0, ..Default::default() };
    let mut sh = Shape::new(Path::rect(10.0, 10.0, 30.0, 30.0), None, Some(Stroke { color: Color::rgb(255, 0, 0), width: 4.0 }));
    sh.stroke_style.scale = mode;
    sh.stroke_style.hairline = hair;
    sh.stroke_style.pixel_hinting = hint;
    d.scene.layers[0].keyframes.insert(0, Keyframe::with(vec![Element::shape("", sh)]));
    d.scene.layers[0].end = 1;
    d
}
fn dom(d: &Document) -> String {
    let (pkg, _) = write_package(d);
    String::from_utf8(pkg.get("DOMDocument.xml").unwrap().clone()).unwrap()
}
fn style_of(d: &Document) -> StrokeStyle {
    let (pkg, _) = write_package(d);
    let back = read_package(&pkg).unwrap().doc;
    for e in &back.scene.layers[0].keyframes[&0].elements {
        if let ElementKind::Shape(s) = &e.kind {
            return s.stroke_style;
        }
    }
    panic!("no shape")
}

#[test]
fn scale_mode_is_written_per_mode() {
    for (m, n) in [(StrokeScale::Normal, "normal"), (StrokeScale::Horizontal, "horizontal"), (StrokeScale::Vertical, "vertical"), (StrokeScale::None, "none")] {
        assert!(dom(&doc(m, false, false)).contains(&format!("scaleMode=\"{n}\"")), "{n}");
    }
}
#[test]
fn scale_modes_round_trip() {
    for m in StrokeScale::ALL {
        assert_eq!(style_of(&doc(m, false, false)).scale, m);
    }
}
#[test]
fn hairline_and_hinting_round_trip() {
    let x = dom(&doc(StrokeScale::Normal, true, true));
    assert!(x.contains("solidStyle=\"hairline\"") && x.contains("pixelHinting=\"true\""));
    let s = style_of(&doc(StrokeScale::None, true, true));
    assert!(s.hairline && s.pixel_hinting && s.scale == StrokeScale::None);
}
#[test]
fn plain_strokes_stay_plain() {
    let x = dom(&doc(StrokeScale::Normal, false, false));
    assert!(!x.contains("solidStyle") && !x.contains("pixelHinting"));
    let s = style_of(&doc(StrokeScale::Normal, false, false));
    assert!(!s.hairline && !s.pixel_hinting);
}
