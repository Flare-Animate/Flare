//! Motion and shape tweens: saved and loaded back the same, Animate's motion XML
//! written back untouched while the tween is, and both baked into SWF frames.

use std::fs;
use std::path::{Path, PathBuf};

use minnamate_core::doc::*;
use minnamate_core::hit::ElementRef;
use minnamate_core::{draw, hit, motion, render::render, swf_export, symbols, transform, xfl, xfl_write};
use swf::{PlaceObjectAction, Tag};

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn solid(r: f32, g: f32, b: f32) -> Paint {
    Paint::Solid([r, g, b, 1.0])
}

/// Two layers over 24 frames: on top, a 40 px square movie clip on a motion tween from
/// (100, 100) to (400, 200) along a curve, turning and fading; below, a shape tween
/// from a red rectangle to a blue circle over frames 1-12.
fn doc() -> Document {
    let mut doc = Document::new(550.0, 400.0, 24.0);
    symbols::new_symbol(&mut doc, "Square", SymbolKind::MovieClip).unwrap();
    let square = draw::rectangle([(-20.0, -20.0), (20.0, 20.0)], 0.0, None, Some(solid(1.0, 0.0, 0.0)));
    draw::add(&mut doc.library.get_mut("Square").unwrap().timeline.layers[0].frames[0], square);
    let inst = Instance {
        symbol: "Square".into(),
        kind: SymbolKind::MovieClip,
        matrix: Matrix::translate(100.0, 100.0),
        pivot: (0.0, 0.0),
        color: ColorTransform::IDENTITY,
        looping: Loop::Loop,
        first_frame: 0,
        extra: Extra::default(),
    };
    let tl = &mut doc.scenes[0];
    tl.layers[0].name = "Motion".into();
    tl.layers[0].frames[0].elements = vec![Element::Instance(inst)];
    assert!(tl.layers[0].create_motion_tween(0, 24.0));
    let target = ElementRef { layer: 0, keyframe: 0, element: 0 };
    motion::edit_shown(tl, target, 23, |el| hit::translate(el, 300.0, 100.0));
    motion::edit_shown(tl, target, 23, |el| transform::transform(el, &Matrix::rotate(1.0).about((400.0, 200.0))));
    let half = ColorTransform { mult: [1.0, 1.0, 1.0, 0.5], add: [0.0; 4] };
    motion::edit_shown(tl, target, 12, |el| {
        if let Element::Instance(i) = el {
            i.color = half;
        }
    });
    let Tween::Motion(m) = &mut tl.layers[0].frames[0].tween else { panic!("motion") };
    let from = (**m).clone();
    m.bend(&from, 12, (0.0, -80.0));
    (m.ease, m.orient) = (0.5, true);

    let shapes = |shape: Shape, index, duration, tween| Frame { index, duration, tween, elements: vec![Element::Shape(shape)], extra: Extra::default() };
    let rect = draw::rectangle([(50.0, 250.0), (250.0, 350.0)], 0.0, None, Some(solid(1.0, 0.0, 0.0)));
    let stroke = Stroke { width: 4.0, paint: solid(0.0, 0.0, 0.0), cap: Cap::Round, join: Join::Round };
    let circle = draw::oval([(300.0, 250.0), (400.0, 350.0)], Some(stroke), Some(solid(0.0, 0.0, 1.0)));
    let frames = vec![shapes(rect, 0, 12, Tween::Shape { ease: -0.25, angular: true, hints: vec![((50.0, 300.0), (350.0, 250.0))] }), shapes(circle, 12, 12, Tween::None)];
    let layer = Layer { name: "Shape".into(), kind: LayerKind::Normal, frames, extra: Extra::default() };
    tl.layers.push(layer);
    doc
}

#[test]
fn tweens_round_trip() {
    let made = doc();
    let dir = scratch("tweens_round_trip");
    // Saved once, the tweens read back as made (shapes come back on Animate's twip grid).
    xfl_write::save(&made, &dir.join("first")).unwrap();
    let doc = xfl::load(&dir.join("first")).unwrap();
    let tween = |d: &Document, l: usize| d.scenes[0].layers[l].frames[0].tween.clone();
    let (Tween::Motion(a), Tween::Motion(b)) = (tween(&made, 0), tween(&doc, 0)) else { panic!("motion") };
    assert_ne!(b.read, 0, "read from the file, which holds the motion XML saving writes back");
    assert_eq!(motion::Motion { read: 0, ..*b }, *a);
    assert_eq!(tween(&doc, 1), Tween::Shape { ease: -0.25, angular: true, hints: vec![((50.0, 300.0), (350.0, 250.0))] });
    let saved = fs::read_to_string(dir.join("first/DOMDocument.xml")).unwrap();
    for xml in [r#"tweenType="motion object""#, "<AnimationCore", r#"tweenType="shape""#, r#"acceleration="-25""#, r#"shapeTweenBlend="angular""#] {
        assert!(saved.contains(xml), "{xml} missing from\n{saved}");
    }
    for path in [dir.join("tweens"), dir.join("tweens.fla")] {
        xfl_write::save(&doc, &path).unwrap();
        let back = xfl::load(&path).unwrap();
        assert_eq!(back, doc, "{}", path.display());
        for f in [0, 5, 12, 18, 23] {
            let (a, b) = (render(&doc, f, 1.0).unwrap().0, render(&back, f, 1.0).unwrap().0);
            assert!(a.data() == b.data(), "{} frame {f}", path.display());
        }
    }
}

#[test]
fn tweens_play() {
    let doc = doc();
    let (start, mid) = (render(&doc, 0, 1.0).unwrap().0, render(&doc, 6, 1.0).unwrap().0);
    let rgb = |p: &minnamate_core::tiny_skia::Pixmap, x: u32, y: u32| {
        let c = p.pixel(x, y).unwrap().demultiply();
        [c.red(), c.green(), c.blue()]
    };
    assert_eq!(rgb(&start, 100, 100), [255, 0, 0], "the square, where its keyframe has it");
    assert_eq!(rgb(&start, 60, 300), [255, 0, 0], "the rectangle");
    // Halfway into the shape tween, eased in: under halfway from red to blue, between the two in place.
    let [r, _, b] = rgb(&mid, 230, 300);
    assert!(r > b && b > 60, "{r} {b}");
    assert_eq!(rgb(&mid, 60, 260), [255, 255, 255], "the rectangle's corner has moved on");
    // The square went along its path: at frame 12 it's above the straight line, at half alpha.
    let at12 = render(&doc, 12, 1.0).unwrap().0;
    let tl = &doc.scenes[0];
    let key = &tl.layers[0].frames[0];
    let Some(Element::Instance(i)) = motion::target_at(key, 12) else { panic!("an instance") };
    assert!(i.matrix.ty < 150.0 - 20.0, "{:?}", i.matrix);
    assert_eq!(i.color.mult[3], 0.5);
    let (x, y) = (i.matrix.tx as u32, i.matrix.ty as u32);
    assert!(rgb(&at12, x, y)[1] > 100, "half see-through red over white");
}

#[test]
fn animates_motion_xml_is_kept_until_the_tween_changes() {
    let core = r#"<motionObjectXML>
                <AnimationCore TimeScale="24000" Version="1" duration="10000">
                  <TimeMap strength="0" type="Quadratic"/>
                  <PropertyContainer id="headContainer">
                    <PropertyContainer id="Basic_Motion">
                      <Property enabled="1" id="Motion_X" ignoreTimeMap="0" readonly="0" visible="1">
                        <Keyframe anchor="0,0" next="0,0" previous="0,0" roving="0" timevalue="0"/>
                        <Keyframe anchor="0,90" next="0,90" previous="0,90" roving="1" timevalue="9000"/>
                      </Property>
                      <Property enabled="1" id="Depth" ignoreTimeMap="0" readonly="0" visible="1">
                        <Keyframe anchor="0,7" next="0,7" previous="0,7" roving="0" timevalue="0"/>
                      </Property>
                    </PropertyContainer>
                    <PropertyContainer id="Filters"><PropertyContainer id="Blur"/></PropertyContainer>
                  </PropertyContainer>
                </AnimationCore>
              </motionObjectXML>"#;
    let source = format!(
        r#"<DOMDocument xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns="http://ns.adobe.com/xfl/2008/" width="550" height="400">
  <timelines>
    <DOMTimeline name="Scene 1">
      <layers>
        <DOMLayer name="Text">
          <frames>
            <DOMFrame index="0" duration="10" tweenType="motion object" motionTweenRotate="none" isMotionObject="true" visibleAnimationKeyframes="4194303" keyMode="8195">
              {core}
              <elements>
                <DOMStaticText width="40" height="20" autoExpand="true">
                  <matrix><Matrix tx="10" ty="10"/></matrix>
                  <textRuns/>
                </DOMStaticText>
              </elements>
            </DOMFrame>
          </frames>
        </DOMLayer>
      </layers>
    </DOMTimeline>
  </timelines>
</DOMDocument>"#
    );
    let dir = scratch("animates_motion_xml");
    fs::create_dir_all(dir.join("source")).unwrap();
    fs::write(dir.join("source/DOMDocument.xml"), source).unwrap();
    let doc = xfl::load(&dir.join("source")).unwrap();
    // The text moves 90 px right over the span.
    let key = &doc.scenes[0].layers[0].frames[0];
    assert!(motion::text_at(key, 9).is_some_and(|m| (m.tx - 100.0).abs() < 1e-4 && m.ty == 10.0));

    xfl_write::save(&doc, &dir.join("saved")).unwrap();
    let saved = fs::read_to_string(dir.join("saved/DOMDocument.xml")).unwrap();
    assert!(saved.contains(core), "the motion XML as read:\n{saved}");
    assert!(saved.contains(r#"keyMode="8195""#) && saved.contains(r#"motionTweenRotate="none""#));
    assert_eq!(xfl::load(&dir.join("saved")).unwrap(), doc);

    // Changed, the tween is written afresh, keeping what the model doesn't read.
    let mut eased = doc.clone();
    let Tween::Motion(m) = &mut eased.scenes[0].layers[0].frames[0].tween else { panic!("motion") };
    m.ease = -1.0;
    let m = m.clone();
    xfl_write::save(&eased, &dir.join("eased")).unwrap();
    let saved = fs::read_to_string(dir.join("eased/DOMDocument.xml")).unwrap();
    assert_eq!(saved.matches("<motionObjectXML").count(), 1);
    for kept in [r#"strength="-100""#, r#"id="Depth""#, r#"<PropertyContainer id="Blur"/>"#] {
        assert!(saved.contains(kept), "{kept} missing from\n{saved}");
    }
    let back = xfl::load(&dir.join("eased")).unwrap();
    let Tween::Motion(b) = &back.scenes[0].layers[0].frames[0].tween else { panic!("motion") };
    assert_eq!((b.ease, b.path.clone(), b.kept.len()), (-1.0, m.path.clone(), m.kept.len()));

    // Remove Tween: no motion XML is left behind.
    let mut removed = doc.clone();
    removed.scenes[0].layers[0].remove_tween(0);
    xfl_write::save(&removed, &dir.join("removed")).unwrap();
    assert!(!fs::read_to_string(dir.join("removed/DOMDocument.xml")).unwrap().contains("motionObjectXML"));

    // Motion XML the model can't read leaves the tween unsupported, and saved as it was.
    let odd = core.replace(r#"anchor="0,90""#, r#"anchor="0,ninety""#);
    let source = fs::read_to_string(dir.join("source/DOMDocument.xml")).unwrap().replace(core, &odd);
    fs::create_dir_all(dir.join("odd")).unwrap();
    fs::write(dir.join("odd/DOMDocument.xml"), source).unwrap();
    let doc = xfl::load(&dir.join("odd")).unwrap();
    assert_eq!(doc.scenes[0].layers[0].frames[0].tween, Tween::Unsupported("motion object".into()));
    xfl_write::save(&doc, &dir.join("odd_saved")).unwrap();
    assert!(fs::read_to_string(dir.join("odd_saved/DOMDocument.xml")).unwrap().contains(&odd));
}

#[test]
fn swf_bakes_both_tweens() {
    let (bytes, skipped) = swf_export::export(&doc()).unwrap();
    assert!(skipped.is_empty(), "{skipped:?}");
    let buf = swf::decompress_swf(&bytes[..]).unwrap();
    let movie = swf::parse_swf(&buf).unwrap();
    assert_eq!(movie.header.num_frames(), 24);
    // Depth 1 is the shape tween's layer, the bottom one; depth 2 the square.
    let (mut frame, mut square, mut shapes) = (0, Vec::new(), Vec::new());
    for tag in &movie.tags {
        match tag {
            Tag::PlaceObject(p) if p.depth == 2 => square.push((frame, p.matrix.unwrap().tx.get())),
            Tag::PlaceObject(p) if p.depth == 1 => shapes.push((frame, matches!(p.action, PlaceObjectAction::Replace(_)))),
            Tag::ShowFrame => frame += 1,
            _ => {}
        }
    }
    assert_eq!(square.len(), 24, "the square moves every frame");
    assert_eq!((square[0].1, square[23].1), (100 * 20, 400 * 20));
    // A shape of its own for each in-between, then the circle.
    let replaced: Vec<u32> = shapes.iter().filter(|s| s.1).map(|s| s.0).collect();
    assert_eq!(replaced, (1..=12).collect::<Vec<_>>());
    let defined = movie.tags.iter().filter(|t| matches!(t, Tag::DefineShape(_))).count();
    assert_eq!(defined, 1 + 11 + 1 + 1, "rectangle, in-betweens, circle, and the square in its clip");
}
