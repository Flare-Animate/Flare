//! Step 3 of docs/UNIFICATION_PLAN.md: ac model -> mm -> XFL write -> XFL read -> ac keeps
//! layers, keyframes, symbols, tweens and transforms; what it cannot keep is in the report.

use std::fs;
use std::path::{Path, PathBuf};

use ac_core::geom::{pt, Matrix, Path as AcPath, Seg};
use ac_core::model::{self as ac, Color, Element, Keyframe, Layer, LayerKind, Shape, SymbolKind, Timeline, Tween};
use minnamate_core::doc as mm;
use minnamate_core::bridge::{ac_to_mm, mm_to_ac};
use minnamate_core::{xfl, xfl_write};

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn rect(x: f32, color: Color) -> Shape {
    Shape::new(AcPath::rect(x, 10.0, 40.0, 20.0), Some(color), Some(ac::Stroke { color: Color::rgb(0, 0, 0), width: 2.0 }))
}

fn sample() -> ac::Document {
    let mut doc = ac::Document::default();
    doc.width = 640.0;
    doc.height = 360.0;
    doc.fps = 30.0;
    doc.background = Color::rgb(0x20, 0x40, 0x80);
    // Library: a movie clip holding a shape, and a graphic holding an instance of it.
    let mut inner = Timeline::new("Ball");
    inner.layers[0].keyframes.insert(0, Keyframe::with(vec![Element::shape("", rect(0.0, Color::rgb(255, 0, 0)))]));
    let ball = doc.add_symbol("Art/Ball", SymbolKind::MovieClip, inner);
    let mut outer = Timeline::new("Wrap");
    outer.layers[0].keyframes.insert(0, Keyframe::with(vec![Element::instance("b", ball, Matrix::translate(5.0, 6.0))]));
    let wrap = doc.add_symbol("Wrap", SymbolKind::Graphic, outer);

    // Scene: layer "Top" has a classic tween with a rotation, a label and an instance;
    // layer "Bottom" is hidden and locked with a shape tween.
    let mut top = Layer::new("Top", 0);
    top.end = 24;
    let mut k0 = Keyframe::with(vec![Element::instance("hero", wrap, Matrix { a: 2.0, b: 0.5, c: -0.5, d: 2.0, tx: 30.0, ty: 40.0 })]);
    k0.tween = Tween::Classic { ease: -50.0, rotate_turns: -2 };
    k0.label = Some("start".into());
    let k12 = Keyframe::with(vec![Element::instance("hero", wrap, Matrix::translate(300.0, 40.0))]);
    top.keyframes.insert(0, k0);
    top.keyframes.insert(12, k12);

    let mut bottom = Layer::new("Bottom", 1);
    bottom.visible = false;
    bottom.locked = true;
    bottom.end = 24;
    let mut s0 = Keyframe::with(vec![Element::shape("", rect(0.0, Color::rgb(0, 255, 0)))]);
    s0.tween = Tween::Shape { ease: 100.0 };
    s0.shape_hints = vec![pt(1.0, 2.0)];
    let mut s1 = Keyframe::with(vec![Element::shape("", rect(100.0, Color::rgb(0, 0, 255)))]);
    s1.shape_hints = vec![pt(3.0, 4.0)];
    bottom.keyframes.insert(0, s0);
    bottom.keyframes.insert(10, s1);
    doc.scene.layers = vec![top, bottom];
    doc
}

#[test]
fn ac_mm_xfl_mm_ac_round_trip() {
    let src = sample();
    let (mm_doc, to_mm) = ac_to_mm(&src);
    assert!(to_mm.is_lossless(), "{to_mm:?}");

    let dir = scratch("bridge_round_trip");
    let path = dir.join("bridge");
    xfl_write::save(&mm_doc, &path).unwrap();
    let back_mm = xfl::load(&path).unwrap();
    assert_eq!(back_mm, mm_doc, "XFL write/read must preserve the mm document");
    let (back, from_mm) = mm_to_ac(&back_mm);
    assert!(from_mm.is_lossless(), "{from_mm:?}");

    assert_eq!((back.width, back.height, back.fps, back.background), (640.0, 360.0, 30.0, Color::rgb(0x20, 0x40, 0x80)));
    // Library: names, kinds, content (ids are renumbered by name).
    let names: Vec<_> = back.library.iter().map(|s| (s.name.as_str(), s.kind)).collect();
    assert_eq!(names, [("Art/Ball", SymbolKind::MovieClip), ("Wrap", SymbolKind::Graphic)]);
    // mm has no Z: ac closes a subpath with Z, mm with a line back to its start (same outline).
    let (ac::ElementKind::Shape(want), ac::ElementKind::Shape(got)) = (&src.library[0].timeline.layers[0].keyframes[&0].elements[0].kind, &back.library[0].timeline.layers[0].keyframes[&0].elements[0].kind) else { panic!() };
    let mut closed = want.path.segs.clone();
    *closed.last_mut().unwrap() = Seg::L(pt(0.0, 10.0));
    assert_eq!((&got.path.segs, got.fill, got.stroke), (&closed, want.fill, want.stroke));

    // Layers.
    let (top, bottom) = (&back.scene.layers[0], &back.scene.layers[1]);
    assert_eq!((top.name.as_str(), top.visible, top.locked, top.kind), ("Top", true, false, LayerKind::Normal));
    assert_eq!((bottom.name.as_str(), bottom.visible, bottom.locked), ("Bottom", false, true));
    assert_eq!((top.end, bottom.end), (24, 24));
    assert_eq!(top.color, src.scene.layers[0].color);
    // Keyframes, label, tweens.
    assert_eq!(top.keyframes.keys().copied().collect::<Vec<_>>(), [0, 12]);
    assert_eq!(top.keyframes[&0].tween, Tween::Classic { ease: -50.0, rotate_turns: -2 });
    assert_eq!(top.keyframes[&0].label.as_deref(), Some("start"));
    assert_eq!(bottom.keyframes[&0].tween, Tween::Shape { ease: 100.0 });
    // Transforms and instance data.
    let hero = &top.keyframes[&0].elements[0];
    assert_eq!(hero.matrix, Matrix { a: 2.0, b: 0.5, c: -0.5, d: 2.0, tx: 30.0, ty: 40.0 });
    assert_eq!(hero.name, "hero");
    let ac::ElementKind::Instance(inst) = &hero.kind else { panic!("instance expected") };
    assert_eq!(back.symbol(inst.symbol).unwrap().name, "Wrap");
    assert_eq!(top.keyframes[&12].elements[0].matrix, Matrix::translate(300.0, 40.0));
    // Shapes: fill, stroke, outline.
    let ac::ElementKind::Shape(sh) = &bottom.keyframes[&10].elements[0].kind else { panic!("shape expected") };
    assert_eq!(sh.fill, Some(Color::rgb(0, 0, 255)));
    assert_eq!(sh.stroke.map(|s| s.width), Some(2.0));
    assert_eq!(sh.path.segs.len(), 5, "rect: M + 3 L + closing L");
    // Shape hints pair up across the tween.
    assert_eq!(bottom.keyframes[&0].shape_hints, vec![pt(1.0, 2.0)]);
    assert_eq!(bottom.keyframes[&10].shape_hints, vec![pt(3.0, 4.0)]);
}

#[test]
fn lossy_fields_are_reported() {
    let mut doc = sample();
    doc.scene.layers[0].keyframes.get_mut(&0).unwrap().script = Some("stop();".into());
    doc.scene.layers[0].keyframes.get_mut(&0).unwrap().elements.push(Element::text("hi", 12.0, Color::rgb(0, 0, 0), pt(0.0, 0.0)));
    doc.scene.layers.push(Layer { kind: LayerKind::Camera, ..Layer::new("Cam", 2) });
    doc.scene.layers[1].keyframes.get_mut(&0).unwrap().tween = Tween::Motion { ease: 0.0 };
    let (_, rep) = ac_to_mm(&doc);
    for key in ["ac.keyframe.extras", "ac.element.text", "ac.camera_layer", "ac.tween.motion"] {
        assert_eq!(rep.count(key), 1, "{key}: {rep:?}");
    }
}

/// The fixture is an mm-side XFL: it must survive mm -> ac -> mm with its layers and tween.
#[test]
fn fixture_mm_to_ac_and_back() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tween");
    let mm_doc = xfl::load(&fixture).unwrap();
    let (a, rep) = mm_to_ac(&mm_doc);
    let (b, _) = ac_to_mm(&a);
    assert_eq!(b.scenes[0].layers.len(), mm_doc.scenes[0].layers.len(), "{rep:?}");
    for (x, y) in b.scenes[0].layers.iter().zip(&mm_doc.scenes[0].layers) {
        assert_eq!(x.name, y.name);
        assert_eq!(x.frames.iter().map(|f| (f.index, f.duration)).collect::<Vec<_>>(), y.frames.iter().map(|f| (f.index, f.duration)).collect::<Vec<_>>());
    }
}

#[test]
fn silent_loss_cases_are_reported() {
    // Same-named symbols stay distinct and instances keep pointing at their own.
    let mut doc = ac::Document::default();
    let mut ta = Timeline::new("A");
    ta.layers[0].keyframes.insert(0, Keyframe::with(vec![Element::shape("", rect(0.0, Color::rgb(255, 0, 0)))]));
    let mut tb = Timeline::new("B");
    tb.layers[0].keyframes.insert(0, Keyframe::with(vec![Element::shape("", rect(0.0, Color::rgb(0, 0, 255)))]));
    let a = doc.add_symbol("Dup", SymbolKind::MovieClip, ta);
    let b = doc.add_symbol("Dup", SymbolKind::MovieClip, tb);
    doc.scene.layers[0].keyframes.insert(0, Keyframe::with(vec![Element::instance("x", a, Matrix::translate(0.0, 0.0)), Element::instance("y", b, Matrix::translate(0.0, 0.0))]));
    let (m, rep) = ac_to_mm(&doc);
    assert_eq!((m.library.len(), rep.count("ac.symbol.renamed")), (2, 1), "{rep:?}");
    let mm::Element::Instance(i0) = &m.scenes[0].layers[0].frames[0].elements[0] else { panic!() };
    let mm::Element::Instance(i1) = &m.scenes[0].layers[0].frames[0].elements[1] else { panic!() };
    assert_ne!(i0.symbol, i1.symbol);
    assert!(m.library.contains_key(&i0.symbol) && m.library.contains_key(&i1.symbol));

    // Shape extras, named non-instance element, layer alpha, leading blank keyframe.
    let mut d = sample();
    let k = d.scene.layers[0].keyframes.get_mut(&0).unwrap();
    k.elements.push(Element::shape("named", { let mut s = rect(0.0, Color::rgb(1, 2, 3)); s.stroke_style.dash = ac::Dash::Dashed; s }));
    d.scene.layers[1].color.a = 128;
    let first = d.scene.layers[1].keyframes.remove(&0).unwrap();
    d.scene.layers[1].keyframes.insert(3, first);
    let (_, rep) = ac_to_mm(&d);
    assert_eq!(rep.count("ac.shape.extras"), 1, "{rep:?}");
    assert!(rep.count("ac.element.extras") >= 1 && rep.count("ac.layer.extras") >= 1, "{rep:?}");
    assert_eq!(rep.count("ac.keyframe.leading_blank"), 1, "{rep:?}");

    // Dangling parent is dropped and counted; huge indices do not overflow.
    let (mut m2, _) = ac_to_mm(&sample());
    m2.scenes[0].layers[0].extra.set("parentLayerIndex", Some("9"));
    m2.scenes[0].layers[1].extra.set("parentLayerIndex", Some("0"));
    m2.scenes[0].layers[1].frames[0].index = u32::MAX;
    m2.scenes[0].layers[1].frames[0].duration = 5;
    let (back, rep) = mm_to_ac(&m2);
    assert_eq!((back.scene.layers[0].parent, back.scene.layers[1].parent, rep.count("mm.layer.dangling_parent")), (None, Some(0), 1), "{rep:?}");
}
