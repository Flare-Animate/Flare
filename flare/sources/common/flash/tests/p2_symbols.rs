//! P2 group 2 (Oct 15 2026): XFL read/write of the Visible / Cache as Bitmap /
//! Export as Bitmap flags, 9-slice scale grids and layer effects.

use ac_core::*;
use ac_xfl::{read_package, write_package};

fn doc() -> Document {
    let mut d = Document { width: 200.0, height: 100.0, ..Default::default() };
    let mut tl = Timeline::new("Panel");
    tl.layers[0].keyframes.insert(0, Keyframe::with(vec![Element::shape("", Shape::new(Path::rect(0.0, 0.0, 100.0, 50.0), Some(Color::rgb(0, 0, 255)), None))]));
    tl.layers[0].end = 1;
    let id = d.add_symbol("Panel", SymbolKind::MovieClip, tl);
    d.symbol_mut(id).unwrap().scale_grid = Some(ScaleGrid { left: 12.5, top: 8.0, right: 88.0, bottom: 41.25 });
    let mut plain = Timeline::new("Plain");
    plain.layers[0].end = 1;
    d.add_symbol("Plain", SymbolKind::Graphic, plain);
    let mut els = vec![Element::instance("a", id, Matrix::translate(10.0, 10.0)), Element::instance("b", id, Matrix::translate(20.0, 20.0)), Element::instance("c", id, Matrix::translate(30.0, 30.0))];
    els[0].visible = false;
    els[1].cache_as_bitmap = true;
    els[2].export_as_bitmap = true;
    els[2].cache_as_bitmap = true;
    d.scene.layers[0].keyframes.insert(0, Keyframe::with(els));
    d.scene.layers[0].end = 1;
    d
}

fn xml(pkg: &ac_xfl::Package, name: &str) -> String {
    String::from_utf8(pkg.get(name).unwrap_or_else(|| panic!("{name}: {:?}", pkg.keys().collect::<Vec<_>>())).clone()).unwrap()
}

#[test]
fn instance_flags_use_the_dom_symbol_instance_attributes() {
    let (pkg, _) = write_package(&doc());
    let dom = xml(&pkg, "DOMDocument.xml");
    assert!(dom.contains(r#"isVisible="false""#) && dom.contains(r#"cacheAsBitmap="true""#) && dom.contains(r#"exportAsBitmap="true""#), "{dom}");
    // exactly one invisible instance, two cached
    assert_eq!(dom.matches("isVisible=").count(), 1);
    assert_eq!(dom.matches("cacheAsBitmap=").count(), 2);
    // default instances carry none of them
    let mut d = doc();
    for e in &mut d.scene.layers[0].keyframes.get_mut(&0).unwrap().elements {
        e.visible = true;
        e.cache_as_bitmap = false;
        e.export_as_bitmap = false;
    }
    let dom = xml(&write_package(&d).0, "DOMDocument.xml");
    assert!(!dom.contains("isVisible") && !dom.contains("cacheAsBitmap") && !dom.contains("exportAsBitmap"));
}

#[test]
fn instance_flags_round_trip() {
    let back = read_package(&write_package(&doc()).0).unwrap().doc;
    let els = &back.scene.layers[0].keyframes[&0].elements;
    assert_eq!(els.len(), 3);
    assert!(!els[0].visible && !els[0].cache_as_bitmap);
    assert!(els[1].visible && els[1].cache_as_bitmap && !els[1].export_as_bitmap);
    assert!(els[2].visible && els[2].cache_as_bitmap && els[2].export_as_bitmap);
}

#[test]
fn files_without_the_attributes_read_as_visible_and_uncached() {
    let (mut pkg, _) = write_package(&doc());
    let dom = xml(&pkg, "DOMDocument.xml").replace(r#" isVisible="false""#, "").replace(r#" cacheAsBitmap="true""#, "").replace(r#" exportAsBitmap="true""#, "");
    pkg.insert("DOMDocument.xml".into(), dom.into_bytes());
    let back = read_package(&pkg).unwrap().doc;
    assert!(back.scene.layers[0].keyframes[&0].elements.iter().all(|e| e.visible && !e.cache_as_bitmap && !e.export_as_bitmap));
}

#[test]
fn scale_grid_uses_the_dom_symbol_item_attributes() {
    let (pkg, _) = write_package(&doc());
    let lib = xml(&pkg, "LIBRARY/Panel.xml");
    assert!(lib.contains(r#"scaleGridLeft="12.5""#) && lib.contains(r#"scaleGridRight="88""#) && lib.contains(r#"scaleGridTop="8""#) && lib.contains(r#"scaleGridBottom="41.25""#), "{lib}");
    assert!(!xml(&pkg, "LIBRARY/Plain.xml").contains("scaleGrid"));
    let back = read_package(&pkg).unwrap().doc;
    let g = back.library.iter().find(|s| s.name == "Panel").unwrap().scale_grid.unwrap();
    assert_eq!((g.left, g.top, g.right, g.bottom), (12.5, 8.0, 88.0, 41.25));
    assert!(back.library.iter().find(|s| s.name == "Plain").unwrap().scale_grid.is_none());
}

#[test]
fn a_nonsense_scale_grid_in_a_file_is_ignored() {
    let (mut pkg, _) = write_package(&doc());
    let lib = xml(&pkg, "LIBRARY/Panel.xml").replace(r#"scaleGridRight="88""#, r#"scaleGridRight="5""#);
    pkg.insert("LIBRARY/Panel.xml".into(), lib.into_bytes());
    let back = read_package(&pkg).unwrap().doc;
    assert!(back.library.iter().find(|s| s.name == "Panel").unwrap().scale_grid.is_none());
}

#[test]
fn nine_slice_renders_the_same_after_a_round_trip() {
    let mut d = doc();
    d.scene.layers[0].keyframes.get_mut(&0).unwrap().elements.truncate(2);
    d.scene.layers[0].keyframes.get_mut(&0).unwrap().elements[0].visible = true;
    d.scene.layers[0].keyframes.get_mut(&0).unwrap().elements[1].matrix = Matrix::scale(2.0, 3.0);
    let back = read_package(&write_package(&d).0).unwrap().doc;
    let b = |doc: &Document| -> Vec<[f32; 4]> { evaluate_scene(doc, 0).iter().filter_map(|i| i.path.transformed(&i.matrix).bounds()).map(|(lo, hi)| [lo.x, lo.y, hi.x, hi.y]).collect() };
    let (x, y) = (b(&d), b(&back));
    assert_eq!(x.len(), y.len());
    for (p, q) in x.iter().zip(&y) {
        assert!(p.iter().zip(q).all(|(a, b)| (a - b).abs() < 0.05), "{p:?} vs {q:?}");
    }
}

#[test]
fn layer_effects_round_trip_through_the_layer_element() {
    let mut d = doc();
    d.scene.layers[0].filters = vec![
        Filter::Blur { blur_x: 6.0, blur_y: 4.0, quality: 2 },
        Filter::DropShadow { blur_x: 4.0, blur_y: 4.0, strength: 100.0, quality: 1, angle: 45.0, distance: 8.0, color: Color::rgb(0, 0, 0), knockout: false, inner: false, hide_object: false },
    ];
    let (pkg, warn) = write_package(&d);
    let dom = xml(&pkg, "DOMDocument.xml");
    let layer_start = dom.find("<DOMLayer").unwrap();
    let filters_at = dom[layer_start..].find("<filters>").unwrap();
    let frames_at = dom[layer_start..].find("<frames>").unwrap();
    assert!(filters_at < frames_at, "the layer's filters come before its frames");
    assert!(warn.iter().all(|w| !w.contains("layer")), "{warn:?}");
    let back = read_package(&pkg).unwrap().doc;
    assert_eq!(back.scene.layers[0].filters.len(), 2);
    assert!(matches!(back.scene.layers[0].filters[0], Filter::Blur { blur_x, blur_y, quality: 2 } if blur_x == 6.0 && blur_y == 4.0));
    assert!(matches!(back.scene.layers[0].filters[1], Filter::DropShadow { distance, .. } if distance == 8.0));
    // object filters on the layer's instances are not mixed up with the layer's
    assert!(back.scene.layers[0].keyframes[&0].elements.iter().all(|e| e.filters.is_empty()));
    // layers without effects write no <filters>
    assert!(!xml(&write_package(&doc()).0, "DOMDocument.xml").contains("<filters>"));
}

#[test]
fn layer_effects_with_no_xfl_form_are_reported() {
    let mut d = doc();
    d.scene.layers[0].filters = vec![Filter::MotionBlur { shutter: 180.0, samples: 8 }];
    let (_, warn) = write_package(&d);
    assert!(warn.iter().any(|w| w.contains("layer") && w.contains("Motion Blur")), "{warn:?}");
}
