//! Documents using Flare's timeline semantics (retained tween curves, keyframe
//! playback clocks) export to .fla as Animate's model and look the same.

use ac_core::*;

/// A 6-frame graphic (a square stepping right) placed on the scene, plus a
/// classic-tweened square on a second layer with a strong ease.
fn doc() -> Document {
    let mut d = Document { width: 200.0, height: 100.0, ..Default::default() };
    let mut tl = Timeline::new("Stepper");
    let l = &mut tl.layers[0];
    for f in 0..6 {
        let sq = Element::shape("s", Shape::new(Path::rect(f as f32 * 10.0, 0.0, 8.0, 8.0), Some(Color::rgb(0, 100, 200)), None));
        l.keyframes.insert(f, Keyframe::with(vec![sq]));
    }
    l.end = 6;
    d.library.push(Symbol { id: 1, name: "Stepper".into(), kind: SymbolKind::Graphic, timeline: tl });
    let mut inst = Layer::new("inst", 0);
    inst.keyframes.insert(0, Keyframe::with(vec![Element::instance("", 1, Matrix::translate(10.0, 10.0))]));
    inst.end = 20;
    let mut tw = Layer::new("tween", 1);
    let a = Element::shape("r", Shape::new(Path::rect(0.0, 60.0, 10.0, 10.0), Some(Color::rgb(200, 0, 0)), None));
    let mut b = a.clone();
    b.matrix = Matrix::translate(150.0, 0.0);
    let mut k0 = Keyframe::with(vec![a]);
    k0.tween = Tween::Classic { ease: 100.0, rotate_turns: 0 };
    tw.keyframes.insert(0, k0);
    tw.keyframes.insert(19, Keyframe::with(vec![b]));
    tw.end = 20;
    d.scene.layers = vec![inst, tw];
    d
}

fn frames(d: &Document) -> Vec<String> {
    // world-space bounds of every item, to 0.05 px (imported shapes keep their
    // position in the path rather than the matrix)
    let r = |v: f32| (v * 20.0).round() / 20.0;
    (0..20)
        .map(|f| {
            let b: Vec<_> = evaluate_scene(d, f).iter().filter_map(|i| i.path.transformed(&i.matrix).bounds()).map(|(a, b)| [r(a.x), r(a.y), r(b.x), r(b.y)]).collect();
            format!("{f}: {b:?}")
        })
        .collect()
}

#[test]
fn retained_curves_and_clocks_round_trip_through_fla() {
    let mut d = doc();
    frames::set_retain_tween_curves(true);
    // split both layers at frame 7: the graphic keeps playing, the eased tween keeps its curve
    for l in &mut d.scene.layers {
        frames::convert_to_keyframes(l, 7, 7, false);
    }
    // and reverse the tween's second half
    frames::reverse_frames(&mut d.scene.layers[1], 7, 19);
    frames::set_retain_tween_curves(false);
    assert!(d.scene.layers[1].keyframes.values().any(|k| k.tween_boundary.is_some()));
    assert_eq!(d.scene.layers[0].keyframes[&7].playback_offset, 7);
    let before = frames(&d);

    let (pkg, warnings) = ac_xfl::write_package(&d);
    assert!(warnings.iter().any(|w| w.contains("frame by frame")), "{warnings:?}");
    let back = ac_xfl::read_package(&pkg).unwrap().doc;
    assert_eq!(frames(&back), before, "same picture on every frame after .fla");
    // the clock became the instance's First Frame
    let ElementKind::Instance(i) = &back.scene.layers[0].keyframes[&7].elements[0].kind else { panic!() };
    assert_eq!(i.first_frame, 7 % 6);
}

/// Real .fla files can carry a wrong end-of-directory record (size or start of
/// the central directory); they still open.
#[test]
fn damaged_zip_end_records_are_repaired() {
    let mut pkg = ac_xfl::Package::new();
    pkg.insert("DOMDocument.xml".into(), b"<DOMDocument/>".to_vec());
    pkg.insert("LIBRARY/a b (c).xml".into(), b"<x/>".to_vec());
    let good = ac_xfl::zip(&pkg).unwrap();
    let eocd = good.windows(4).rposition(|w| w == b"PK\x05\x06").unwrap();
    // size 54 bytes too large (as in a real file) and a wrong directory start
    let mut bad = good.clone();
    let size = u32::from_le_bytes(bad[eocd + 12..eocd + 16].try_into().unwrap());
    bad[eocd + 12..eocd + 16].copy_from_slice(&(size + 54).to_le_bytes());
    bad[eocd + 16..eocd + 20].copy_from_slice(&7u32.to_le_bytes());
    let fixed = ac_xfl::repair_end_record(&bad).unwrap();
    assert_eq!(fixed, good, "the record is rebuilt exactly");
    let back = ac_xfl::unzip(&bad).unwrap();
    assert_eq!(back.len(), 2);
    assert_eq!(back["LIBRARY/a b (c).xml"], b"<x/>");
    assert!(ac_xfl::unzip(b"not a zip at all").is_err());
}

/// Real-file corpus: set AC_FLA_CORPUS to a folder of .fla files (not in the
/// repository: they belong to their authors). Each must open quickly, with
/// every drawn item at a sane size (a misread number shows up as a huge shape).
#[test]
fn real_fla_corpus_opens() {
    let Some(dir) = std::env::var_os("AC_FLA_CORPUS") else { return };
    let mut n = 0;
    for f in std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("fla"))) {
        let t = std::time::Instant::now();
        let im = ac_xfl::open(&f).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        let secs = t.elapsed().as_secs_f32();
        assert!(secs < 10.0, "{}: {secs:.1}s to open", f.display());
        let d = &im.doc;
        for frame in [0, d.scene.frame_count() / 2, d.scene.frame_count().saturating_sub(1)] {
            for it in ac_core::evaluate_scene(d, frame) {
                if let Some((a, b)) = it.path.transformed(&it.matrix).bounds() {
                    let lim = 20.0 * d.width.max(d.height);
                    assert!(b.x - a.x < lim && b.y - a.y < lim, "{} frame {}: an item spans {a:?}..{b:?}", f.display(), frame + 1);
                }
            }
        }
        eprintln!("{}: {} symbols, {} frames, {secs:.2}s, {} notes", f.display(), d.library.len(), d.scene.frame_count(), im.warnings.len());
        n += 1;
    }
    eprintln!("{n} files");
}

/// Focal gradients, document swatches (solid + gradient) and art-brush
/// strokes survive a write and read back.
#[test]
fn focal_gradients_swatches_and_art_brushes() {
    use ac_core::*;
    let mut d = Document::default();
    let mut sh = Shape::new(Path::rect(0.0, 0.0, 100.0, 100.0), None, None);
    sh.gradient = Some(Gradient { focal: 0.6, ..Gradient::fit(GradientKind::Radial, vec![(0.0, Color::rgb(255, 255, 255)), (1.0, Color::rgb(0, 0, 0))], pt(0.0, 0.0), pt(100.0, 100.0)) });
    d.scene.layers[0].keyframes.get_mut(&0).unwrap().elements.push(Element::shape("", sh));
    d.swatches = Some(vec![Swatch::solid("", Color::rgb(0x12, 0x34, 0x56)), Swatch::gradient("", GradientKind::Linear, vec![(0.0, Color::rgb(255, 0, 0)), (1.0, Color::rgb(0, 0, 255))])]);
    let (mut pkg, _) = ac_xfl::write_package(&d);
    // add an art brush and a shape drawn with it (as Animate writes them)
    pkg.insert("PaintBrushDefinitions.xml".into(), br#"<paintbrushdefinitions xmlns="http://ns.adobe.com/xfl/2008/"><paintbrushdefinition index="1" brushParam='{"FlashPaintBrushType":"ArtBrush","artDirection":"0","flipArt":"false","name":"Bar","svgData":"&lt;svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 100 20\" width=\"100\" height=\"20\"&gt;&lt;path d=\"M0,0 L100,0 L100,20 L0,20 Z\"/&gt;&lt;/svg&gt;"}'/></paintbrushdefinitions>"#.to_vec());
    let dom = String::from_utf8(pkg["DOMDocument.xml"].clone()).unwrap();
    let brush_shape = r##"<DOMShape isDrawingObject="true"><strokes><StrokeStyle index="1"><PaintBrushStroke weight="10" paintBrushDefinition="1"><fill><SolidColor color="#00FF00"/></fill></PaintBrushStroke></StrokeStyle></strokes><edges><Edge strokeStyle="1" edges="!0 0|4000 0"/></edges></DOMShape>"##;
    let dom = dom.replacen("<elements>", &format!("<elements>{brush_shape}"), 1);
    assert!(dom.contains("focalPointRatio=\"0.6\""), "focal written");
    assert!(dom.contains("<SolidSwatchItem color=\"#123456\"/>") && dom.contains("<LinearGradientSwatchItem>"), "swatches written");
    pkg.insert("DOMDocument.xml".into(), dom.into_bytes());
    let im = ac_xfl::read_package(&pkg).unwrap();
    let els: Vec<&Element> = im.doc.scene.layers[0].keyframes[&0].elements.iter().collect();
    let g = els.iter().find_map(|e| match &e.kind { ElementKind::Shape(s) => s.gradient.clone(), _ => None }).unwrap();
    assert!((g.focal - 0.6).abs() < 1e-4);
    let art = els.iter().find_map(|e| match &e.kind { ElementKind::Shape(s) if s.fill == Some(Color::rgb(0, 255, 0)) => Some(s.clone()), _ => None }).expect("art brush stroke became a green fill");
    let (lo, hi) = art.path.bounds().unwrap();
    assert!((hi.x - lo.x - 200.0).abs() < 0.5 && (hi.y - lo.y - 10.0).abs() < 0.5, "{lo:?} {hi:?}");
    assert!(!im.warnings.iter().any(|w| w.contains("solid line") || w.contains("focal")), "{:?}", im.warnings);
    let sw = im.doc.swatches.unwrap();
    assert_eq!(sw.len(), 2);
    assert_eq!(sw[0].color, Color::rgb(0x12, 0x34, 0x56));
    assert_eq!(sw[1].gradient.as_ref().unwrap().stops.len(), 2);
}

/// Text fields (dynamic/input, paragraph options, link), library folders
/// (empty ones too) and linkage survive a write and read back.
#[test]
fn text_fields_folders_and_linkage_round_trip() {
    use ac_core::text::{Align, Baseline, TextKind};
    use ac_core::*;
    let mut d = Document::default();
    let mut el = Element::text("Score: 0", 18.0, Color::rgb(10, 20, 30), pt(5.0, 6.0));
    el.name = "score".into();
    if let ElementKind::Text(t) = &mut el.kind {
        t.align = Align::Justify;
        t.para.width = Some(150.0);
        t.para.line_spacing = 4.0;
        t.para.indent = 8.0;
        t.para.margin_left = 2.0;
        t.para.baseline = Baseline::Superscript;
        t.para.no_kern = true;
        t.field.kind = TextKind::Input;
        t.field.max_chars = 12;
        t.field.border = true;
        t.field.password = true;
        t.field.selectable = true;
        t.field.link = Some("https://example.org".into());
    }
    d.scene.layers[0].keyframes.get_mut(&0).unwrap().elements.push(el);
    d.library.push(Symbol { id: 1, name: "Cast/Hero".into(), kind: SymbolKind::MovieClip, timeline: Timeline::new("Hero") });
    d.next_symbol_id = 2;
    d.folders = vec!["Cast".into(), "Empty".into()];
    d.linkage.insert(1, "HeroClip".into());
    let (pkg, _) = ac_xfl::write_package(&d);
    let dom = String::from_utf8(pkg["DOMDocument.xml"].clone()).unwrap();
    assert!(dom.contains("<DOMInputText") && dom.contains("maxCharacters=\"12\"") && dom.contains("lineType=\"password\""), "{dom}");
    assert!(dom.contains("DOMFolderItem name=\"Empty\""));
    let im = ac_xfl::read_package(&pkg).unwrap();
    let ElementKind::Text(t) = &im.doc.scene.layers[0].keyframes[&0].elements[0].kind else { panic!() };
    assert_eq!(t.field.kind, TextKind::Input);
    assert_eq!((t.field.max_chars, t.field.border, t.field.password), (12, true, true));
    assert_eq!(t.field.link.as_deref(), Some("https://example.org"));
    assert_eq!(t.align, Align::Justify);
    assert_eq!(t.para.width, Some(150.0));
    assert_eq!((t.para.line_spacing, t.para.indent, t.para.margin_left), (4.0, 8.0, 2.0));
    assert_eq!(t.para.baseline, Baseline::Superscript);
    assert!(t.para.no_kern);
    assert!(im.doc.folders.contains(&"Empty".to_string()) && im.doc.folders.contains(&"Cast".to_string()));
    assert_eq!(im.doc.library[0].name, "Cast/Hero");
    assert_eq!(im.doc.linkage.get(&1).map(|s| s.as_str()), Some("HeroClip"));
    assert!(!im.warnings.iter().any(|w| w.contains("static text")), "{:?}", im.warnings);
}
