//! Layers linked to others: a mask clips what it masks when published, when rendered and
//! on the editor's stage once locked; and folders, masks, guides and layer parenting load
//! back from XFL as saved.

use std::fs;
use std::path::{Path, PathBuf};

use minnamate_core::doc::*;
use minnamate_core::render::{render, render_layer, render_timeline};
use minnamate_core::tiny_skia::Pixmap;
use minnamate_core::{draw, hit, swf_export, xfl, xfl_write};
use swf::Tag;

const RED: Rgba = [1.0, 0.0, 0.0, 1.0];
const BLUE: Rgba = [0.0, 0.0, 1.0, 1.0];
/// The mask's own colour, faint: masking goes by shape, not colour or alpha.
const FAINT: Rgba = [0.0, 1.0, 0.0, 0.3];

fn square(x0: f32, y0: f32, x1: f32, y1: f32, color: Rgba) -> Element {
    Element::Shape(draw::rectangle([(x0, y0), (x1, y1)], 0.0, None, Some(Paint::Solid(color))))
}

fn layer(name: &str, kind: LayerKind, elements: Vec<Element>) -> Layer {
    let frames = vec![Frame { index: 0, duration: 1, tween: Tween::None, elements, extra: Extra::default() }];
    Layer { name: name.into(), kind, frames, extra: Extra::default() }
}

/// Top down: a mask (100..200 square), the red layer it masks (50..250), a blue strip below both.
fn masked_doc() -> Document {
    let mut doc = Document::new(300.0, 400.0, 24.0);
    let tl = &mut doc.scenes[0];
    tl.layers = vec![
        layer("Mask", LayerKind::Mask, vec![square(100.0, 100.0, 200.0, 200.0, FAINT)]),
        layer("Red", LayerKind::Normal, vec![square(50.0, 50.0, 250.0, 250.0, RED)]),
        layer("Blue", LayerKind::Normal, vec![square(0.0, 300.0, 300.0, 350.0, BLUE)]),
    ];
    tl.set_parent(1, Some(0));
    doc
}

fn rgba(p: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let c = p.pixel(x, y).unwrap().demultiply();
    [c.red(), c.green(), c.blue(), c.alpha()]
}

#[test]
fn a_mask_clips_the_layers_it_masks() {
    let doc = masked_doc();
    let (f, skipped) = render(&doc, 0, 1.0).unwrap();
    assert!(skipped.is_empty(), "{skipped:?}");
    assert_eq!(rgba(&f, 150, 150), [255, 0, 0, 255], "inside the mask: the masked layer, opaque");
    assert_eq!(rgba(&f, 60, 60), [255, 255, 255, 255], "outside it: neither the masked layer nor the mask");
    assert_eq!(rgba(&f, 150, 325), [0, 0, 255, 255], "other layers as they are");
    // Antialiased at the mask's edge, not cut a pixel early or late.
    assert_eq!(rgba(&f, 100, 150)[1], 0);
    assert_eq!(rgba(&f, 99, 150), [255, 255, 255, 255]);

    // An empty mask hides what it masks.
    let mut empty = doc.clone();
    empty.scenes[0].layers[0].frames[0].elements.clear();
    assert_eq!(rgba(&render(&empty, 0, 1.0).unwrap().0, 150, 150), [255, 255, 255, 255]);

    // Only filled areas show what's masked: Animate ignores a mask's strokes.
    let mut stroked = doc.clone();
    let stroke = Stroke { width: 20.0, paint: Paint::Solid(FAINT), cap: Cap::Round, join: Join::Round };
    let outlined = draw::rectangle([(100.0, 100.0), (200.0, 200.0)], 0.0, Some(stroke), Some(Paint::Solid(FAINT)));
    stroked.scenes[0].layers[0].frames[0].elements = vec![Element::Shape(outlined)];
    let f = render(&stroked, 0, 1.0).unwrap().0;
    assert_eq!((rgba(&f, 150, 150), rgba(&f, 95, 150)), ([255, 0, 0, 255], [255, 255, 255, 255]));
}

#[test]
fn the_stage_shows_masking_once_mask_and_masked_are_locked() {
    let mut doc = masked_doc();
    let stage = |doc: &Document| render_timeline(doc, &doc.scenes[0], 0, &Matrix::IDENTITY, (300.0, 400.0), 1.0).unwrap().0;
    // Unlocked: the mask's shape over the red layer, unclipped.
    let open = stage(&doc);
    assert_eq!(rgba(&open, 60, 60), [255, 0, 0, 255]);
    assert!((170..=190).contains(&rgba(&open, 150, 150)[0]), "the faint green mask over red: {:?}", rgba(&open, 150, 150));
    // Locked, both: clipped, the mask not drawn.
    for l in [0, 1] {
        doc.scenes[0].layers[l].set_locked(true);
    }
    let shut = stage(&doc);
    assert_eq!((rgba(&shut, 60, 60)[3], rgba(&shut, 150, 150)), (0, [255, 0, 0, 255]));
    // One layer at a time, as onion skinning draws them: the red one, still clipped; the mask, not at all.
    let one = |l| render_layer(&doc, &doc.scenes[0], 0, l, &Matrix::IDENTITY, (300.0, 400.0), 1.0).unwrap().0;
    let (red, mask) = (one(1), one(0));
    assert_eq!((rgba(&red, 150, 150), rgba(&red, 60, 60)[3], rgba(&red, 150, 325)[3]), ([255, 0, 0, 255], 0, 0));
    assert!(mask.data().iter().all(|&b| b == 0));
}

#[test]
fn an_unlocked_masked_layer_shows_unclipped_over_locked_ones() {
    // The blue strip masked too, under the red layer; the mask and the strip locked, red not.
    let mut doc = masked_doc();
    let tl = &mut doc.scenes[0];
    tl.set_parent(2, Some(0));
    for l in [0, 2] {
        tl.layers[l].set_locked(true);
    }
    let stage = render_timeline(&doc, &doc.scenes[0], 0, &Matrix::IDENTITY, (300.0, 400.0), 1.0).unwrap().0;
    assert_eq!(rgba(&stage, 60, 60), [255, 0, 0, 255], "red, unlocked, as it is");
    assert_eq!(rgba(&stage, 150, 325)[3], 0, "the strip, locked, clipped away");
    // Published, both clip.
    assert_eq!(rgba(&render(&doc, 0, 1.0).unwrap().0, 60, 60), [255, 255, 255, 255]);
}

#[test]
fn guides_and_unlocked_masks_on_the_stage_have_bounds() {
    // What can be picked on the stage has a box to select and transform it by.
    let mut doc = masked_doc();
    doc.scenes[0].layers.push(layer("Guide", LayerKind::Guide, vec![square(10.0, 10.0, 20.0, 20.0, BLUE)]));
    let tl = &doc.scenes[0];
    for (at, l, (min, max)) in [((15.0, 15.0), 3, (10.0, 20.0)), ((150.0, 150.0), 0, (100.0, 200.0))] {
        let picked = hit::hit_test(&doc, tl, 0, at, 0.0).expect("on the stage");
        assert_eq!(picked.layer, l);
        let b = hit::bounds(&doc, tl, 0, &[picked]).expect("a box");
        assert!([b.min.0, b.min.1, b.max.0, b.max.1].iter().zip([min, min, max, max]).all(|(a, b)| (a - b).abs() < 0.01), "{b:?}");
    }
}

#[test]
fn the_swf_masks_with_a_clip_depth() {
    let mut doc = masked_doc();
    // A second shape on the mask layer: the mask goes out as a sprite of both.
    doc.scenes[0].layers[0].frames[0].elements.push(square(0.0, 0.0, 10.0, 10.0, FAINT));
    for (two, doc) in [(false, masked_doc()), (true, doc)] {
        let (bytes, skipped) = swf_export::export(&doc).unwrap();
        assert!(skipped.is_empty(), "{skipped:?}");
        let buf = swf::decompress_swf(&bytes[..]).unwrap();
        let movie = swf::parse_swf(&buf).unwrap();
        let placed: Vec<_> = movie.tags.iter().filter_map(|t| if let Tag::PlaceObject(p) = t { Some((p.depth, p.clip_depth)) } else { None }).collect();
        // Bottom up: the blue strip, the mask clipping depth 3, the red layer.
        assert_eq!(placed, [(1, None), (2, Some(3)), (3, None)], "two shapes in the mask: {two}");
        let sprite = movie.tags.iter().find_map(|t| if let Tag::DefineSprite(s) = t { Some(s) } else { None });
        assert_eq!(sprite.map(|s| s.tags.iter().filter(|t| matches!(t, Tag::PlaceObject(_))).count()), two.then_some(2));
    }
}

/// An empty folder for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn links_between_layers_survive_saving() {
    let mut doc = masked_doc();
    let tl = &mut doc.scenes[0];
    // A closed folder holding the mask group; a guide over a tween that turns along it; a hand parented to an arm.
    let folder = tl.insert_folder(0, "Folder");
    tl.layers[folder].set_open(false);
    tl.set_parent(1, Some(0));
    let ball = tl.insert_layer(4, "Ball");
    tl.layers[ball].frames[0].tween = Tween::Classic { ease: 0.0, rotate: Rotate::Auto };
    tl.layers[ball].frames[0].extra.set("motionTweenOrientToPath", Some("true"));
    tl.add_motion_guide(ball, "Guide: Ball");
    let arm = tl.insert_layer(6, "Arm");
    let hand = tl.insert_layer(7, "Hand");
    assert!(tl.set_rig_parent(hand, Some(arm)));
    let tree: Vec<_> = (0..tl.layers.len()).map(|l| (tl.layers[l].name.as_str(), tl.parent(l))).collect();
    assert_eq!(tree, [("Folder", None), ("Mask", Some(0)), ("Red", Some(1)), ("Blue", None), ("Guide: Ball", None), ("Ball", Some(4)), ("Arm", None), ("Hand", None)]);

    let dir = scratch("layer_links");
    for path in [dir.join("folder"), dir.join("doc.fla")] {
        xfl_write::save(&doc, &path).unwrap();
        let back = xfl::load(&path).unwrap();
        assert_eq!(back, doc, "{}", path.display());
        let tl = &back.scenes[0];
        assert!(!tl.layers[0].open() && tl.mask_of(2) == Some(1) && tl.guide_of(5) == Some(4));
        assert_eq!(tl.rig_parent(7, 0), Some(6));
    }
    let xml = fs::read_to_string(dir.join("folder/DOMDocument.xml")).unwrap();
    for kept in [r#"layerType="folder" open="false""#, r#"layerType="mask" parentLayerIndex="0""#, r#"layerType="guide""#, r#"motionTweenOrientToPath="true""#, r#"layerRiggingIndex="0""#] {
        assert!(xml.contains(kept), "{kept} missing from\n{xml}");
    }
}
