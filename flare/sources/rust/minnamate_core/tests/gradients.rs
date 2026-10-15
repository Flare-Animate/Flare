//! Gradient fills and strokes: read from XFL and saved back, drawn, and exported to SWF.

use std::fs;
use std::path::{Path, PathBuf};

use minnamate_core::doc::{Document, Element, Gradient, GradientKind, Matrix, Paint, Shape, Spread};
use minnamate_core::render::{render, render_timeline};
use minnamate_core::{draw, swf_export, xfl, xfl_write};
use swf::{FillStyle, GradientInterpolation, GradientSpread, Tag};

/// A 400 × 200 stage: a linear red-to-blue square at 0..100, a radial white-to-black one
/// with its focal point halfway out at 150..250, a linear-RGB black-to-white one at
/// 300..400 reflected halfway, and a line along y = 150 stroked red to half-clear green.
const DOCUMENT: &str = r##"<DOMDocument xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns="http://ns.adobe.com/xfl/2008/" width="400" height="200">
  <timelines>
    <DOMTimeline name="Scene 1">
      <layers>
        <DOMLayer name="Layer_1">
          <frames>
            <DOMFrame index="0">
              <elements>
                <DOMShape>
                  <fills>
                    <FillStyle index="1">
                      <LinearGradient>
                        <matrix><Matrix a="0.06103515625" d="0.06103515625" tx="50" ty="50"/></matrix>
                        <GradientEntry color="#FF0000" ratio="0"/>
                        <GradientEntry color="#0000FF" ratio="1"/>
                      </LinearGradient>
                    </FillStyle>
                  </fills>
                  <edges><Edge fillStyle1="1" edges="!0 0|2000 0|2000 2000|0 2000|0 0"/></edges>
                </DOMShape>
                <DOMShape>
                  <fills>
                    <FillStyle index="1">
                      <RadialGradient focalPointRatio="0.5" unknownAttr="1">
                        <matrix><Matrix a="0.06103515625" d="0.06103515625" tx="200" ty="50"/></matrix>
                        <GradientEntry color="#FFFFFF" ratio="0"/>
                        <GradientEntry color="#000000" ratio="1"/>
                        <futureChild/>
                      </RadialGradient>
                    </FillStyle>
                  </fills>
                  <edges><Edge fillStyle1="1" edges="!3000 0|5000 0|5000 2000|3000 2000|3000 0"/></edges>
                </DOMShape>
                <DOMShape>
                  <fills>
                    <FillStyle index="1">
                      <LinearGradient spreadMethod="reflect" interpolationMethod="linearRGB">
                        <matrix><Matrix a="0.030517578125" d="0.06103515625" tx="325" ty="50"/></matrix>
                        <GradientEntry color="#000000" ratio="0"/>
                        <GradientEntry color="#FFFFFF" ratio="1"/>
                      </LinearGradient>
                    </FillStyle>
                  </fills>
                  <edges><Edge fillStyle1="1" edges="!6000 0|8000 0|8000 2000|6000 2000|6000 0"/></edges>
                </DOMShape>
                <DOMShape>
                  <strokes>
                    <StrokeStyle index="1">
                      <SolidStroke scaleMode="normal" weight="10">
                        <fill>
                          <LinearGradient>
                            <matrix><Matrix a="0.244140625" d="0.006103515625" tx="200" ty="150"/></matrix>
                            <GradientEntry color="#FF0000" ratio="0"/>
                            <GradientEntry color="#00FF00" alpha="0.5" ratio="1"/>
                          </LinearGradient>
                        </fill>
                      </SolidStroke>
                    </StrokeStyle>
                  </strokes>
                  <edges><Edge strokeStyle="1" edges="!0 3000|8000 3000"/></edges>
                </DOMShape>
              </elements>
            </DOMFrame>
          </frames>
        </DOMLayer>
      </layers>
    </DOMTimeline>
  </timelines>
</DOMDocument>"##;

/// The document, loaded from a folder of its own.
fn load(name: &str) -> (Document, PathBuf) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("source")).unwrap();
    fs::write(dir.join("source/DOMDocument.xml"), DOCUMENT).unwrap();
    (xfl::load(&dir.join("source")).unwrap(), dir)
}

fn shapes(doc: &Document) -> Vec<&Shape> {
    let elements = &doc.scenes[0].layers[0].frames[0].elements;
    elements.iter().map(|el| if let Element::Shape(s) = el { s } else { panic!("{el:?}") }).collect()
}

fn gradient(p: &Paint) -> &minnamate_core::doc::Gradient {
    let Paint::Gradient(g) = p else { panic!("{p:?}") };
    g
}

#[test]
fn round_trip() {
    let (doc, dir) = load("gradients_round_trip");
    let s = shapes(&doc);
    let linear = gradient(&s[0].fills[0]);
    assert_eq!((linear.kind, linear.spread, linear.linear_rgb), (GradientKind::Linear, Spread::Pad, false));
    assert_eq!((linear.matrix.a, linear.matrix.tx), (100.0 / 1638.4, 50.0), "the 1638.4 px square spans 100 px");
    assert_eq!(linear.stops, [(0.0, [1.0, 0.0, 0.0, 1.0]), (1.0, [0.0, 0.0, 1.0, 1.0])]);
    let radial = gradient(&s[1].fills[0]);
    assert_eq!(radial.kind, GradientKind::Radial { focal: 0.5 });
    assert_eq!(radial.extra.attrs, [("unknownAttr".to_string(), "1".to_string())]);
    let reflected = gradient(&s[2].fills[0]);
    assert_eq!((reflected.spread, reflected.linear_rgb), (Spread::Reflect, true));
    assert_eq!(gradient(&s[3].strokes[0].paint).stops[1], (1.0, [0.0, 1.0, 0.0, 0.5]), "strokes carry a fill");

    for path in [dir.join("saved"), dir.join("saved.fla")] {
        xfl_write::save(&doc, &path).unwrap();
        let back = xfl::load(&path).unwrap();
        assert_eq!(back, doc, "{}", path.display());
        assert!(render(&back, 0, 1.0).unwrap().0.data() == render(&doc, 0, 1.0).unwrap().0.data());
    }
    let saved = fs::read_to_string(dir.join("saved/DOMDocument.xml")).unwrap();
    let written = [
        r##"<GradientEntry color="#FF0000" ratio="0"/>"##,
        r#"<RadialGradient focalPointRatio="0.5" unknownAttr="1">"#,
        r#"<GradientEntry ratio="1"/>"#,
        "<futureChild/>",
        r#"<LinearGradient spreadMethod="reflect" interpolationMethod="linearRGB">"#,
        r##"<GradientEntry color="#00FF00" alpha="0.5" ratio="1"/>"##,
    ];
    for xml in written {
        assert!(saved.contains(xml), "{xml} missing from\n{saved}");
    }
}

#[test]
fn rendering() {
    let (doc, _) = load("gradients_rendering");
    let (pixmap, skipped) = render(&doc, 0, 1.0).unwrap();
    assert!(skipped.is_empty(), "{skipped:?}");
    let s = shapes(&doc);
    let rgb = |x: u32, y: u32| {
        let c = pixmap.pixel(x, y).unwrap().demultiply();
        [c.red(), c.green(), c.blue()].map(f32::from)
    };
    // What the model says pixel (x, y) shows: paint `p`'s colour at its middle, over the white stage.
    let want = |p: &Paint, x: u32, y: u32| {
        let [r, g, b, a] = gradient(p).color_at_point((x as f32 + 0.5, y as f32 + 0.5));
        [r, g, b].map(|c| (c * a + 1.0 - a) * 255.0)
    };
    let near = |got: [f32; 3], want: [f32; 3], at: &str| {
        assert!(got.iter().zip(want).all(|(g, w)| (g - w).abs() <= 2.5), "{at}: drew {got:?}, want {want:?}");
    };
    // Anchors: the ramps run the way the file says.
    near(rgb(0, 50), [254.0, 0.0, 1.0], "linear's left end");
    near(rgb(99, 50), [1.0, 0.0, 254.0], "linear's right end");
    near(rgb(224, 49), [252.0; 3], "radial's focal point");
    near(rgb(151, 1), [0.0; 3], "radial's corner, past the circle");
    near(rgb(324, 50), [188.0; 3], "linear RGB's middle is half the light, not half of 255");
    near(rgb(374, 50), [188.0; 3], "reflected back");
    near(rgb(2, 150), [253.0, 2.0, 1.0], "the stroke's red end");
    near(rgb(397, 150), [128.0, 255.0, 128.0], "the stroke's half-clear green end");
    // Across each fill, the drawing matches the model's own sums.
    for (paint, xs) in [(&s[0].fills[0], 5..95), (&s[1].fills[0], 155..245), (&s[2].fills[0], 305..395)] {
        for x in xs.step_by(9) {
            for y in [5, 37, 50, 71, 94] {
                near(rgb(x, y), want(paint, x, y), &format!("({x}, {y})"));
            }
        }
    }
    for x in (10..390).step_by(20) {
        near(rgb(x, 150), want(&s[3].strokes[0].paint, x, 150), &format!("stroke at {x}"));
    }
}

#[test]
fn swf_export() {
    let (doc, _) = load("gradients_swf");
    let (bytes, skipped) = swf_export::export(&doc).unwrap();
    assert!(skipped.is_empty(), "{skipped:?}");
    let buf = swf::decompress_swf(&bytes[..]).unwrap();
    let movie = swf::parse_swf(&buf).unwrap();
    let defined: Vec<_> = movie.tags.iter().filter_map(|t| if let Tag::DefineShape(s) = t { Some(s) } else { None }).collect();
    assert_eq!(defined.len(), 4);

    let FillStyle::LinearGradient(linear) = &defined[0].styles.fill_styles[0] else { panic!("{:?}", defined[0].styles) };
    // The same square in twips: the scale carries over, the offset goes to twips.
    assert_eq!((linear.matrix.a.to_f32(), linear.matrix.tx.get(), linear.matrix.ty.get()), (100.0 / 1638.4, 1000, 1000));
    let records: Vec<_> = linear.records.iter().map(|r| (r.ratio, [r.color.r, r.color.g, r.color.b, r.color.a])).collect();
    assert_eq!(records, [(0, [255, 0, 0, 255]), (255, [0, 0, 255, 255])]);
    assert_eq!((linear.spread, linear.interpolation), (GradientSpread::Pad, GradientInterpolation::Rgb));

    let FillStyle::FocalGradient { gradient, focal_point } = &defined[1].styles.fill_styles[0] else { panic!("{:?}", defined[1].styles) };
    assert_eq!((focal_point.to_f32(), gradient.matrix.tx.get()), (0.5, 4000));
    let FillStyle::LinearGradient(reflected) = &defined[2].styles.fill_styles[0] else { panic!("{:?}", defined[2].styles) };
    assert_eq!((reflected.spread, reflected.interpolation), (GradientSpread::Reflect, GradientInterpolation::LinearRgb));

    let FillStyle::LinearGradient(stroke) = defined[3].styles.line_styles[0].fill_style() else { panic!("{:?}", defined[3].styles) };
    assert_eq!(stroke.records[1].color.a, 128, "a stroke's gradient, half-clear end and all");
}

/// Linear RGB changes how the ramp mixes, not how it blends: white fading out
/// over black is half grey halfway, as any half-clear white is, and on a clear
/// canvas (the editor's stage) every pixel stays a valid premultiplied colour.
#[test]
fn linear_rgb_only_mixes_the_ramp() {
    let mut fade = Gradient::new(GradientKind::Linear);
    (fade.linear_rgb, fade.stops) = (true, vec![(0.0, [1.0; 4]), (1.0, [1.0, 1.0, 1.0, 0.0])]);
    let mut doc = Document::new(100.0, 20.0, 24.0);
    doc.background = [0.0, 0.0, 0.0, 1.0];
    let bar = draw::rectangle([(0.0, 0.0), (100.0, 20.0)], 0.0, None, Some(Paint::Gradient(fade)));
    doc.scenes[0].layers[0].frames[0].elements.push(Element::Shape(bar));
    let (pixmap, _) = render(&doc, 0, 1.0).unwrap();
    for x in [10, 50, 90] {
        let want = 255.0 * (1.0 - (x as f32 + 0.5) / 100.0);
        let got = f32::from(pixmap.pixel(x, 10).unwrap().red());
        assert!((got - want).abs() <= 2.5, "at {x}: {got}, want {want}");
    }
    let (clear, _) = render_timeline(&doc, &doc.scenes[0], 0, &Matrix::IDENTITY, (100.0, 20.0), 1.0).unwrap();
    let bad = clear.pixels().iter().position(|p| p.red().max(p.green()).max(p.blue()) > p.alpha());
    assert_eq!(bad, None, "a premultiplied colour brighter than its alpha");
}
