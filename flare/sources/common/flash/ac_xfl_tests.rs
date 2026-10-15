use super::*;
use ac_core::geom::{pt, Matrix, Path};
use ac_core::model::*;

fn fixture() -> Package {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/handmade");
    read_dir(&dir).unwrap()
}

fn region_area(c: &ac_core::merge::FillRegion) -> f32 {
    c.contours.iter().enumerate().map(|(i, k)| edges::signed_area(k).abs() * if i == 0 { 1.0 } else { -1.0 }).sum()
}

fn canvas(kf: &Keyframe) -> &ac_core::merge::MergeShape {
    match &kf.elements[0].kind {
        ElementKind::Canvas(c) => c,
        k => panic!("expected a canvas, got {k:?}"),
    }
}

fn check_fixture(doc: &Document) {
    assert_eq!((doc.width, doc.height, doc.fps), (320.0, 240.0, 12.0));
    assert_eq!(doc.background, Color::rgb(0x33, 0x66, 0x99));
    assert_eq!(doc.library.len(), 1);
    let ball = &doc.library[0];
    assert_eq!((ball.name.as_str(), ball.kind), ("Ball", SymbolKind::Graphic));
    let ElementKind::Shape(sh) = &ball.timeline.layers[0].keyframes[&0].elements[0].kind else { panic!("ball is a drawing object") };
    let g = sh.gradient.as_ref().expect("radial gradient");
    assert_eq!(g.kind, GradientKind::Radial);
    assert!((g.matrix.a - 0.0244 * 819.2).abs() < 0.05, "gradient spans the 20 px ball: {:?}", g.matrix);
    assert_eq!(sh.stroke.unwrap().color, Color::rgb(0x66, 0, 0));
    let (lo, hi) = sh.path.bounds().unwrap();
    assert!(lo.dist(pt(-20.0, -20.0)) < 0.01 && hi.dist(pt(20.0, 20.0)) < 0.01);

    let layers = &doc.scene.layers;
    assert_eq!(layers.len(), 2);
    let l = &layers[0];
    assert_eq!(l.end, 11);
    let k0 = &l.keyframes[&0];
    assert_eq!(k0.tween, Tween::Classic { ease: -40.0, rotate_turns: 0 });
    assert_eq!(k0.label.as_deref(), Some("start"));
    assert_eq!(k0.script.as_deref(), Some("this.ready = true;"));
    let e = &k0.elements[0];
    assert_eq!(e.name, "ball");
    assert_eq!(e.matrix, Matrix::translate(40.0, 120.0));
    assert!((e.alpha - 0.5).abs() < 1e-4);
    assert!(matches!(e.filters[0], ac_core::fx::Filter::DropShadow { distance, .. } if distance == 3.0));
    assert_eq!(l.keyframes[&10].elements[0].matrix.a, 2.0);

    let g = &layers[1];
    assert!(g.locked);
    let c = canvas(&g.keyframes[&0]);
    assert_eq!(c.fills.len(), 2, "{:?}", c.fills.iter().map(|f| f.fill).collect::<Vec<_>>());
    let green = c.fills.iter().find(|f| f.fill == Some(Color::rgb(0x22, 0x88, 0x33))).unwrap();
    assert!((region_area(green) - 320.0 * 40.0).abs() < 1.0, "{}", region_area(green));
    let sun = c.fills.iter().find(|f| f.fill.map(|c| c.a) == Some(191)).unwrap();
    // a circle of radius 30 drawn with four quadratics is a bit larger than πr²
    let a = region_area(sun);
    assert!(a > 2700.0 && a < 3100.0, "{a}");
    assert_eq!(c.strokes.len(), 1);
    assert_eq!(c.strokes[0].stroke.width, 3.0);
    let ElementKind::Text(t) = &g.keyframes[&0].elements[1].kind else { panic!("text") };
    assert_eq!(t.text, "Hello\nXFL");
    assert_eq!(t.align, ac_core::text::Align::Center);
    assert_eq!(g.keyframes[&0].elements[1].matrix, Matrix::translate(12.0, 10.0));
}

#[test]
fn reads_a_handwritten_xfl_folder() {
    let im = read_package(&fixture()).unwrap();
    check_fixture(&im.doc);
    // and it evaluates/renders like any document
    let items = evaluate_scene(&im.doc, 5);
    assert!(items.len() >= 4, "{}", items.len());
}

#[test]
fn round_trips_through_a_zipped_fla() {
    let im = read_package(&fixture()).unwrap();
    let (pkg, warn) = write_package(&im.doc);
    assert!(warn.is_empty(), "{warn:?}");
    let bytes = zip(&pkg).unwrap();
    assert_eq!(&bytes[30..38], b"mimetype", "mimetype is the first, stored entry");
    let back = read_package(&unzip(&bytes).unwrap()).unwrap();
    assert!(back.warnings.is_empty(), "{:?}", back.warnings);
    check_fixture(&back.doc);
    // a second trip changes nothing
    let (pkg2, _) = write_package(&back.doc);
    assert_eq!(String::from_utf8_lossy(&pkg["DOMDocument.xml"]), String::from_utf8_lossy(&pkg2["DOMDocument.xml"]));
}

#[test]
fn round_trips_editor_features() {
    let mut doc = Document::default();
    // bitmap
    let rgba: Vec<u8> = (0..16).flat_map(|i| [i as u8 * 15, 100, 200, 255]).collect();
    let mut bmp = ac_core::bitmap::BitmapAsset::from_rgba(7, "tile.png", 4, 4, &rgba).unwrap();
    bmp.smoothing = false;
    doc.bitmaps.push(bmp);
    // symbols: a movie clip in a folder, a button
    let mut mc = Symbol { id: 3, name: "Parts/Wheel".into(), kind: SymbolKind::MovieClip, timeline: Timeline::new("Wheel") };
    let mut sq = Shape::new(Path::rect(-5.0, -5.0, 10.0, 10.0), Some(Color::rgb(10, 20, 30)), Some(Stroke { color: Color::rgb(1, 2, 3), width: 2.0 }));
    sq.stroke_style.dash = Dash::Dashed;
    sq.stroke_style.cap = Cap::None;
    mc.timeline.layers[0].keyframes.get_mut(&0).unwrap().elements.push(Element::shape("", sq));
    doc.library.push(mc);
    doc.library.push(Symbol { id: 9, name: "Btn".into(), kind: SymbolKind::Button, timeline: Timeline::new("Btn") });
    doc.next_symbol_id = 10;
    // scene: folder > mask > masked layer with a classic tween + custom ease
    let tl = &mut doc.scene;
    tl.layers.clear();
    let mut folder = Layer::new("Folder", 0);
    folder.kind = LayerKind::Folder;
    folder.collapsed = true;
    let mut mask = Layer::new("Mask", 1);
    mask.kind = LayerKind::Mask;
    mask.parent = Some(0);
    mask.keyframes.get_mut(&0).unwrap().elements.push(Element::shape("", Shape::new(Path::ellipse(50.0, 50.0, 40.0, 40.0), Some(Color::rgb(0, 0, 0)), None)));
    let mut body = Layer::new("Body", 2);
    body.parent = Some(1);
    body.visible = false;
    let mut inst = Element::instance("wheel", 3, Matrix::translate(10.0, 20.0));
    inst.tint = Some((Color::rgb(255, 0, 0), 0.25));
    inst.blend = ac_core::fx::BlendMode::Multiply;
    inst.filters = vec![Filter::default_of("gradient bevel").unwrap()];
    let mut k0 = Keyframe::with(vec![inst.clone(), Element::bitmap("", 7, Matrix::translate(100.0, 0.0))]);
    k0.tween = Tween::Classic { ease: 0.0, rotate_turns: 2 };
    k0.ease_curve = Some(ac_core::motion::Ease::linear_custom());
    k0.prop_eases = Some(Box::new(ac_core::motion::PropEases { scale: Some(ac_core::motion::Ease::preset(ac_core::motion::EaseFamily::Quad, ac_core::motion::EaseDir::In)), ..Default::default() }));
    k0.sync = true;
    k0.snap = false;
    k0.label = Some("why".into());
    k0.label_kind = LabelKind::Comment;
    body.keyframes.insert(0, k0);
    inst.matrix = Matrix::translate(200.0, 20.0);
    body.keyframes.insert(12, Keyframe::with(vec![inst]));
    body.end = 13;
    // a group and an advanced colour effect on a button
    let mut b = Element::instance("go", 9, Matrix::IDENTITY);
    b.color_fx = Some(ColorXform { mul: [0.5, 1.0, 1.0, 1.0], add: [0.0, 10.0, 0.0, 0.0] });
    body.keyframes.get_mut(&12).unwrap().elements.push(Element::group(vec![b]));
    tl.layers.extend([folder, mask, body]);
    doc.more_scenes.push(Timeline::new("Scene 2"));
    doc.sounds.push(ac_core::sound::SoundAsset { id: 4, name: "voice.wav".into(), data: b"RIFF....WAVE".to_vec() });
    doc.scene.layers[2].keyframes.get_mut(&12).unwrap().sound = Some(ac_core::sound::FrameSound { sound: 4, sync: ac_core::sound::SoundSync::Stream, repeat: 3, forever: false, effect: Default::default() });

    let (pkg, warn) = write_package(&doc);
    assert!(warn.is_empty(), "{warn:?}");
    assert!(pkg.contains_key("LIBRARY/Parts/Wheel.xml") && pkg.contains_key("LIBRARY/tile.png"));
    assert!(pkg.keys().any(|k| k.starts_with("bin/") && k.ends_with(".dat")));
    let back = read_package(&pkg).unwrap();
    assert!(back.warnings.is_empty(), "{:?}", back.warnings);
    let d = back.doc;
    assert_eq!(d.more_scenes.len(), 1);
    assert_eq!(d.bitmaps.len(), 1);
    assert!(!d.bitmaps[0].smoothing);
    let px = d.bitmaps[0].image().unwrap();
    assert_eq!((px.width, px.height), (4, 4));
    let wheel = d.library.iter().find(|s| s.name == "Parts/Wheel").unwrap();
    assert_eq!(wheel.kind, SymbolKind::MovieClip);
    let ElementKind::Shape(s) = &wheel.timeline.layers[0].keyframes[&0].elements[0].kind else { panic!() };
    assert_eq!(s.fill, Some(Color::rgb(10, 20, 30)));
    assert_eq!((s.stroke_style.dash, s.stroke_style.cap), (Dash::Dashed, Cap::None));
    assert_eq!(d.library.iter().find(|s| s.name == "Btn").unwrap().kind, SymbolKind::Button);
    let l = &d.scene.layers;
    assert_eq!((l[0].kind, l[0].collapsed), (LayerKind::Folder, true));
    assert_eq!((l[1].kind, l[1].parent), (LayerKind::Mask, Some(0)));
    assert_eq!((l[2].parent, l[2].visible, l[2].end), (Some(1), false, 13));
    let k0 = &l[2].keyframes[&0];
    assert_eq!(k0.tween, Tween::Classic { ease: 0.0, rotate_turns: 2 });
    let ease = k0.ease_curve.as_ref().unwrap();
    assert!((ease.apply(0.5) - 0.5).abs() < 0.02);
    let sc = k0.prop_eases.as_ref().and_then(|p| p.scale.as_ref()).expect("per-property ease");
    assert!((sc.apply(0.5) - 0.25).abs() < 0.03, "{}", sc.apply(0.5));
    assert!(k0.sync && !k0.snap);
    assert_eq!((k0.label.as_deref(), k0.label_kind), (Some("why"), LabelKind::Comment));
    let e = &k0.elements[0];
    assert_eq!(e.tint, Some((Color::rgb(255, 0, 0), 0.25)));
    assert_eq!(e.blend, ac_core::fx::BlendMode::Multiply);
    assert_eq!(e.filters, vec![Filter::default_of("gradient bevel").unwrap()], "gradient filter with its ramp");
    assert!(matches!(k0.elements[1].kind, ElementKind::Bitmap(_)));
    let ElementKind::Group(g) = &l[2].keyframes[&12].elements[1].kind else { panic!() };
    assert_eq!(g[0].color_fx.unwrap().add[1], 10.0);
    assert_eq!(d.sounds.len(), 1);
    assert_eq!(d.sounds[0].data, b"RIFF....WAVE");
    let fs = l[2].keyframes[&12].sound.as_ref().unwrap();
    assert_eq!((fs.sync, fs.repeat, fs.forever, fs.sound), (ac_core::sound::SoundSync::Stream, 3, false, d.sounds[0].id));
}

#[test]
fn motion_tweens_are_baked_and_merge_drawings_survive() {
    let mut doc = Document::default();
    let mut c = ac_core::merge::MergeShape::default();
    c.add_path(&Path::rect(0.0, 0.0, 100.0, 100.0), Some(Color::rgb(255, 0, 0)), None, Some(Stroke { color: Color::rgb(0, 0, 0), width: 2.0 }));
    c.add_path(&Path::rect(40.0, 40.0, 20.0, 20.0), Some(Color::rgb(0, 0, 255)), None, None);
    let l = &mut doc.scene.layers[0];
    l.keyframes.get_mut(&0).unwrap().elements.push(Element { kind: ElementKind::Canvas(c), ..Element::group(vec![]) });
    let mut l2 = Layer::new("Moving", 1);
    let mut k = Keyframe::with(vec![Element::instance("", 1, Matrix::IDENTITY)]);
    k.tween = Tween::Motion { ease: 0.0 };
    let mut m = ac_core::motion::Motion::new(pt(0.0, 0.0));
    m.path = vec![ac_core::motion::PathKey { frame: 0, at: pt(0.0, 0.0), h_in: pt(0.0, 0.0), h_out: pt(0.0, 0.0) }, ac_core::motion::PathKey { frame: 4, at: pt(40.0, 0.0), h_in: pt(0.0, 0.0), h_out: pt(0.0, 0.0) }];
    k.motion = Some(Box::new(m));
    l2.keyframes.insert(0, k);
    l2.end = 5;
    doc.scene.layers.push(l2);
    doc.library.push(Symbol { id: 1, name: "Dot".into(), kind: SymbolKind::Graphic, timeline: Timeline::new("Dot") });
    doc.next_symbol_id = 2;
    let (pkg, warn) = write_package(&doc);
    assert!(warn.iter().any(|w| w.contains("frame by frame")));
    let d = read_package(&pkg).unwrap().doc;
    let c = canvas(&d.scene.layers[0].keyframes[&0]);
    let red = c.fills.iter().find(|f| f.fill == Some(Color::rgb(255, 0, 0))).unwrap();
    assert!((region_area(red) - (10000.0 - 400.0)).abs() < 1.0, "the red square keeps its hole: {}", region_area(red));
    assert!(c.fills.iter().any(|f| f.fill == Some(Color::rgb(0, 0, 255))));
    assert!(!c.strokes.is_empty());
    let moving = &d.scene.layers[1];
    assert_eq!(moving.keyframes.len(), 5);
    assert!((moving.keyframes[&2].elements[0].matrix.tx - 20.0).abs() < 0.01);
}

#[test]
fn saves_and_opens_files_and_folders() {
    let im = read_package(&fixture()).unwrap();
    let dir = std::env::temp_dir().join(format!("ac-xfl-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let fla = dir.join("t.fla");
    save(&im.doc, &fla).unwrap();
    check_fixture(&open(&fla).unwrap().doc);
    let xfl = dir.join("t.xfl");
    save(&im.doc, &xfl).unwrap();
    assert!(dir.join("t/t.xfl").exists() && dir.join("t/DOMDocument.xml").exists());
    check_fixture(&open(&dir.join("t/t.xfl")).unwrap().doc);
    check_fixture(&open(&dir.join("t")).unwrap().doc);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn adjacent_fills_and_lines_share_edges_on_export() {
    let mut doc = Document::default();
    let mut c = ac_core::merge::MergeShape::default();
    c.add_path(&Path::rect(0.0, 0.0, 100.0, 50.0), Some(Color::rgb(255, 0, 0)), None, None);
    c.add_path(&Path::rect(0.0, 50.0, 100.0, 50.0), Some(Color::rgb(0, 0, 255)), None, None);
    c.add_stroke(&[pt(0.0, 50.0), pt(100.0, 50.0)], false, Stroke { color: Color::rgb(0, 0, 0), width: 2.0 });
    doc.scene.layers[0].keyframes.get_mut(&0).unwrap().elements.push(Element { kind: ElementKind::Canvas(c), ..Element::group(vec![]) });
    let (pkg, _) = write_package(&doc);
    let xml = String::from_utf8(pkg["DOMDocument.xml"].clone()).unwrap();
    // the middle line: one edge with both fills and the stroke
    assert!(xml.lines().any(|l| l.contains("fillStyle0") && l.contains("fillStyle1") && l.contains("strokeStyle")), "{xml}");
    // and the shapes still come back intact
    let d = read_package(&pkg).unwrap().doc;
    let c = canvas(&d.scene.layers[0].keyframes[&0]);
    let areas: Vec<f32> = c.fills.iter().map(region_area).collect();
    assert_eq!(areas.len(), 2, "{areas:?}");
    assert!(areas.iter().all(|a| (a - 5000.0).abs() < 1.0), "{areas:?}");
    assert!(!c.strokes.is_empty());
}

#[test]
fn reads_motion_object_tweens() {
    let mut pkg = fixture();
    let xml = String::from_utf8(pkg["DOMDocument.xml"].clone()).unwrap();
    // turn the ball's classic tween into a motion-object tween (hand-written)
    let motion = r#"<DOMFrame index="0" duration="10" tweenType="motion object" name="start" labelType="name">
              <motionObjectXML>
                <AnimationCore TimeScale="12000" Version="1" duration="120000">
                  <PropertyContainer id="headContainer">
                    <PropertyContainer id="Basic_Motion">
                      <Property enabled="1" id="Motion_X"><Keyframe anchor="0,0" timevalue="0"/><Keyframe anchor="0,200" timevalue="9000"/></Property>
                      <Property enabled="1" id="Motion_Y"><Keyframe anchor="0,0" timevalue="0"/><Keyframe anchor="0,-50" timevalue="9000"/></Property>
                      <Property enabled="1" id="Rotation_Z"><Keyframe anchor="0,0" timevalue="0"/><Keyframe anchor="0,90" timevalue="9000"/></Property>
                    </PropertyContainer>
                    <PropertyContainer id="Transformation">
                      <Property enabled="1" id="Scale_X"><Keyframe anchor="0,100" timevalue="0"/><Keyframe anchor="0,200" timevalue="9000"/></Property>
                    </PropertyContainer>
                  </PropertyContainer>
                </AnimationCore>
              </motionObjectXML>"#;
    let xml = xml.replacen(r#"<DOMFrame index="0" duration="10" tweenType="motion" acceleration="-40" name="start" labelType="name">"#, motion, 1);
    pkg.insert("DOMDocument.xml".into(), xml.into_bytes());
    let im = read_package(&pkg).unwrap();
    let k = &im.doc.scene.layers[0].keyframes[&0];
    assert!(k.tween.is_motion(), "{:?} {:?}", k.tween, im.warnings);
    let m = k.motion.as_ref().unwrap();
    assert_eq!(m.path.len(), 2);
    assert_eq!(m.path[1].frame, 9);
    assert!(m.path[1].at.dist(pt(240.0, 70.0)) < 1e-3, "{:?}", m.path[1].at);
    // frame 9 of the span: moved, turned 90°, twice as wide
    let items = evaluate_scene(&im.doc, 9);
    let ball = items.iter().find(|i| i.gradient.is_some()).unwrap();
    assert!((ball.matrix.tx - 240.0).abs() < 0.5 && (ball.matrix.ty - 70.0).abs() < 0.5, "{:?}", ball.matrix);
    let d = ball.matrix.decompose();
    assert!((d.skew_y.to_degrees() - 90.0).abs() < 0.5 && (d.scale_x - 2.0).abs() < 0.01, "{d:?}");
}

#[test]
fn text_faces_split_into_family_weight_and_italic() {
    use crate::read::split_face;
    assert_eq!(split_face("Arial-BoldItalicMT"), (Some("Arial".into()), 700, true));
    assert_eq!(split_face("TimesNewRomanPSMT"), (Some("Times New Roman".into()), 400, false));
    assert_eq!(split_face("Ubuntu Light"), (Some("Ubuntu Light".into()), 400, false));
    assert_eq!(split_face("OpenSans-Semibold"), (Some("Open Sans".into()), 600, false));
}
