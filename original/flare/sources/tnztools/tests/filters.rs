//! Filters and Display options: drawn, published to SWF, and saved to XFL.

use std::path::Path;

use minnamate_core::doc::*;
use minnamate_core::filters::{self, Blend, Effects, Filter, FilterKind};
use minnamate_core::tiny_skia::Pixmap;
use minnamate_core::render::{render, render_timeline};
use minnamate_core::{draw, swf_export, xfl, xfl_write};
use swf::Tag;

const RED: Rgba = [1.0, 0.0, 0.0, 1.0];
const GREY: Rgba = [128.0 / 255.0, 128.0 / 255.0, 128.0 / 255.0, 1.0];

/// A movie clip symbol drawing a `side`-pixel square from its origin.
fn square(color: Rgba, side: f32) -> Symbol {
    let mut timeline = Timeline::new("square");
    draw::add(&mut timeline.layers[0].frames[0], draw::rectangle([(0.0, 0.0), (side, side)], 0.0, None, Some(Paint::Solid(color))));
    Symbol { kind: SymbolKind::MovieClip, timeline, extra: Extra::default() }
}

fn instance(symbol: &str, (tx, ty): Point) -> Instance {
    Instance {
        symbol: symbol.into(),
        kind: SymbolKind::MovieClip,
        matrix: Matrix { tx, ty, ..Matrix::IDENTITY },
        pivot: (0.0, 0.0),
        color: ColorTransform::IDENTITY,
        looping: Loop::Loop,
        first_frame: 0,
        extra: Extra::default(),
    }
}

/// A 100 × 100 white stage holding a 40 × 40 red square clip at (30, 30), set up by `setup`,
/// over a mid-grey band across the stage's lower half.
fn doc(setup: impl FnOnce(&mut Instance)) -> Document {
    let mut doc = Document::new(100.0, 100.0, 24.0);
    doc.library.insert("Box".into(), square(RED, 40.0));
    let mut inst = instance("Box", (30.0, 30.0));
    setup(&mut inst);
    let frame = &mut doc.scenes[0].layers[0].frames[0];
    draw::add(frame, draw::rectangle([(0.0, 50.0), (100.0, 100.0)], 0.0, None, Some(Paint::Solid(GREY))));
    frame.elements.push(Element::Instance(inst));
    doc
}

fn with_filter(f: Filter) -> Document {
    doc(|i| filters::set_filters(&mut i.extra, &[f]))
}

fn frame(doc: &Document) -> Pixmap {
    let (pixmap, skipped) = render(doc, 0, 1.0).unwrap();
    assert!(skipped.is_empty(), "{skipped:?}");
    pixmap
}

fn rgb(pixmap: &Pixmap, x: u32, y: u32) -> [u8; 3] {
    let c = pixmap.pixel(x, y).unwrap().demultiply();
    [c.red(), c.green(), c.blue()]
}

#[test]
fn blur_softens_the_edges() {
    let plain = frame(&doc(|_| {}));
    let blurred = frame(&with_filter(Filter { blur: [10.0; 2], ..Filter::new(FilterKind::Blur) }));
    assert_eq!(rgb(&plain, 28, 40), [255; 3]);
    let [r, g, b] = rgb(&blurred, 28, 40);
    assert!(r == 255 && (60..230).contains(&g) && g == b, "red bleeds out past the left edge: {:?}", [r, g, b]);
    assert!(rgb(&blurred, 33, 40)[1] > 0, "and white in");
    assert_eq!(rgb(&blurred, 50, 40), [255, 0, 0], "the middle stays red");
    assert_eq!(rgb(&blurred, 20, 40), [255; 3], "5 px out it's gone");
}

#[test]
fn drop_shadow_falls_below_right() {
    let shadowed = frame(&with_filter(Filter::new(FilterKind::DropShadow)));
    // 4 px at 45°: under the bottom edge it's grey over the white, nothing above the top.
    let below = rgb(&shadowed, 40, 72);
    assert!(below[0] < 160 && below[0] == below[1] && below[1] == below[2], "{below:?}");
    assert_eq!(rgb(&shadowed, 40, 27), [255; 3]);
    assert_eq!(rgb(&shadowed, 50, 50), [255, 0, 0], "the object is on top");

    // Knockout leaves the shadow alone, the object cut out of it: the band shows through.
    let knockout = frame(&with_filter(Filter { knockout: true, ..Filter::new(FilterKind::DropShadow) }));
    assert_eq!(rgb(&knockout, 50, 60), [128; 3]);
    assert!(rgb(&knockout, 40, 72)[0] < 100, "the shadow over the band");
    // An inner shadow is cast inside, along the top and left edges.
    let inner = frame(&with_filter(Filter { side: filters::Side::Inner, ..Filter::new(FilterKind::DropShadow) }));
    assert!(rgb(&inner, 31, 50)[0] < 200, "dark inside the left edge: {:?}", rgb(&inner, 31, 50));
    assert_eq!(rgb(&inner, 68, 50), [255, 0, 0], "not the right one");
}

#[test]
fn glow_surrounds_the_object() {
    let blue = Filter { color: [0.0, 0.0, 1.0, 1.0], ..Filter::new(FilterKind::Glow) };
    let glowing = frame(&with_filter(blue.clone()));
    for (x, y) in [(28, 40), (40, 28), (71, 40)] {
        let [r, g, b] = rgb(&glowing, x, y);
        assert!(b == 255 && r < 230 && r == g, "blue just outside at ({x}, {y}): {:?}", [r, g, b]);
    }
    assert_eq!(rgb(&glowing, 50, 40), [255, 0, 0]);
    // Disabled, it doesn't draw.
    assert_eq!(frame(&with_filter(Filter { enabled: false, ..blue })).data(), frame(&doc(|_| {})).data());
}

#[test]
fn blend_modes_mix_with_whats_below() {
    let blended = |mode: &str| frame(&doc(|i| i.extra.set("blendMode", Some(mode))));
    // Over white above the band, over grey on it.
    assert_eq!(rgb(&blended("multiply"), 40, 40), [255, 0, 0]);
    assert_eq!(rgb(&blended("multiply"), 40, 60), [128, 0, 0]);
    assert_eq!(rgb(&blended("screen"), 40, 60), [255, 128, 128]);
    assert_eq!(rgb(&blended("difference"), 40, 40), [0, 255, 255]);
    assert_eq!(rgb(&blended("subtract"), 40, 60), [0, 128, 128]);
    assert_eq!(rgb(&blended("invert"), 40, 60), [127; 3]);
    assert_eq!(rgb(&blended("add"), 40, 60), [255, 128, 128]);
    // Straight onto the stage Erase draws as Normal; inside a Layer it erases what's below it.
    assert_eq!(rgb(&blended("erase"), 40, 40), [255, 0, 0]);
}

#[test]
fn blend_modes_mix_with_the_stage_in_the_editor_view() {
    // The editor draws the scene onto a clear canvas and shows it over the stage colour.
    let canvas = |doc: &Document| render_timeline(doc, &doc.scenes[0], 0, &Matrix::IDENTITY, (100.0, 100.0), 1.0).unwrap().0;
    let px = |pm: &Pixmap, x, y| {
        let c = pm.pixel(x, y).unwrap();
        [c.red(), c.green(), c.blue(), c.alpha()]
    };
    let difference = canvas(&doc(|i| i.extra.set("blendMode", Some("difference"))));
    assert_eq!(px(&difference, 40, 40), [0, 255, 255, 255], "red against the white stage, as `render` draws it");
    // Drawn as is, the canvas stays see-through where the art is: a shadow over the bare stage.
    let a = px(&canvas(&with_filter(Filter::new(FilterKind::DropShadow))), 72, 40)[3];
    assert!(a > 0 && a < 255, "{a}");
}

#[test]
fn classic_tweens_tween_matching_filters() {
    // Keyframes at 0 and 10 with a classic tween between: a shadow from 0 to 20 px away, and
    // a glow at one end against a blur at the other, which can't tween.
    let shadow = |distance| Filter { distance, ..Filter::new(FilterKind::DropShadow) };
    let mut doc = doc(|i| filters::set_filters(&mut i.extra, &[shadow(0.0)]));
    let layer = &mut doc.scenes[0].layers[0];
    layer.frames[0].elements.remove(0);
    let mut end = layer.frames[0].clone();
    (layer.frames[0].duration, layer.frames[0].tween) = (10, Tween::Classic { ease: 0.0, rotate: Rotate::Auto });
    (end.index, end.tween) = (10, Tween::None);
    let Element::Instance(i) = &mut end.elements[0] else { panic!() };
    filters::set_filters(&mut i.extra, &[shadow(20.0)]);
    layer.frames.push(end);
    let shown = |doc: &Document, frame| {
        let list = minnamate_core::display::display_list(doc, &doc.scenes[0], frame, &mut Default::default());
        list[0].effects.filters.clone()
    };
    assert_eq!(shown(&doc, 5)[0].distance, 10.0, "halfway");
    assert_eq!(shown(&doc, 10)[0].distance, 20.0);
    // Inserting a keyframe there keeps what it shows.
    let mut split = doc.clone();
    split.scenes[0].layers[0].insert_keyframe(5);
    assert_eq!(shown(&split, 5)[0].distance, 10.0);

    let Element::Instance(i) = &mut doc.scenes[0].layers[0].frames[1].elements[0] else { panic!() };
    filters::set_filters(&mut i.extra, &[Filter::new(FilterKind::Blur)]);
    assert_eq!(shown(&doc, 5), [shadow(0.0)], "a list that doesn't match holds");
}

#[test]
fn absurd_sizes_from_a_file_still_draw() {
    // Far past Animate's ranges, as a hand-edited file may have them.
    let shadow = r#"<filters><DropShadowFilter blurX="1e30" distance="INF"/></filters>"#;
    let pm = frame(&doc(|i| i.extra.after.push(shadow.into())));
    assert_eq!(rgb(&pm, 50, 40), [255, 0, 0]);
}

#[test]
fn erase_cuts_through_its_layer() {
    let mut doc = doc(|i| i.symbol = "Holed".into());
    let mut holed = square(RED, 40.0);
    let mut hole = instance("Hole", (10.0, 10.0));
    hole.extra.set("blendMode", Some("erase"));
    holed.timeline.layers[0].frames[0].elements.push(Element::Instance(hole));
    doc.library.insert("Holed".into(), holed);
    doc.library.insert("Hole".into(), square([0.0, 0.0, 0.0, 1.0], 20.0));
    let set = |doc: &mut Document, mode| {
        let Element::Instance(i) = &mut doc.scenes[0].layers[0].frames[0].elements[1] else { panic!() };
        i.extra.set("blendMode", Some(mode));
    };
    set(&mut doc, "layer");
    let layered = frame(&doc);
    assert_eq!(rgb(&layered, 50, 45), [255; 3], "the stage shows through the hole");
    assert_eq!(rgb(&layered, 50, 55), [128; 3], "and so does the band");
    assert_eq!(rgb(&layered, 35, 35), [255, 0, 0]);
    set(&mut doc, "normal");
    assert_eq!(rgb(&frame(&doc), 50, 45), [0; 3], "without the layer the hole draws black");
}

#[test]
fn adjust_color_and_color_effects_apply_after_filters() {
    // A tint applies to the shadow too, as it colours the filtered object.
    let tinted = doc(|i| {
        filters::set_filters(&mut i.extra, &[Filter::new(FilterKind::DropShadow)]);
        i.color = ColorTransform { mult: [0.0, 0.0, 0.0, 1.0], add: [0.0, 0.0, 1.0, 0.0] };
    });
    let pm = frame(&tinted);
    assert_eq!(rgb(&pm, 50, 40), [0, 0, 255]);
    let right = rgb(&pm, 72, 40);
    assert!(right[2] == 255 && right[0] < 160 && right[0] == right[1], "a blue shadow: {right:?}");
    let grey = frame(&with_filter(Filter { adjust: [0.0, 0.0, -100.0, 0.0], ..Filter::new(FilterKind::AdjustColor) }));
    let [r, g, b] = rgb(&grey, 50, 40);
    assert!(r == g && g == b && r > 30 && r < 90, "red desaturated to its grey: {:?}", [r, g, b]);
}

#[test]
fn hidden_instances_publish_but_dont_draw() {
    let hidden = doc(|i| i.extra.set("isVisible", Some("false")));
    assert_eq!(rgb(&frame(&hidden), 50, 40), [255; 3]);
    let places = placements(&hidden);
    assert_eq!(places.len(), 2, "{places:?}");
    assert_eq!((places[1].version, places[1].visible), (3, Some(false)));
    assert_eq!((places[0].version, places[0].visible), (2, None), "the band needs no PlaceObject3");
}

/// What a PlaceObject tag of the main timeline sets.
#[derive(Debug)]
struct Placement {
    version: u8,
    visible: Option<bool>,
    blend: Option<swf::BlendMode>,
    cached: Option<bool>,
    filters: Option<Vec<swf::Filter>>,
}

fn placements(doc: &Document) -> Vec<Placement> {
    let (bytes, skipped) = swf_export::export(doc).unwrap();
    assert!(skipped.is_empty(), "{skipped:?}");
    let buf = swf::decompress_swf(&bytes[..]).unwrap();
    let movie = swf::parse_swf(&buf).unwrap();
    assert_eq!(movie.header.version(), 11);
    let places = movie.tags.iter().filter_map(|t| if let Tag::PlaceObject(p) = t { Some(p) } else { None });
    let placement = |p: &swf::PlaceObject| Placement {
        version: p.version,
        visible: p.is_visible,
        blend: p.blend_mode,
        cached: p.is_bitmap_cached,
        filters: p.filters.clone(),
    };
    places.map(|p| placement(p)).collect()
}

#[test]
fn swf_carries_filters_blend_and_caching() {
    let off = Filter { enabled: false, ..Filter::new(FilterKind::Blur) };
    let shadow = Filter { quality: 3, distance: 6.0, strength: 0.5, ..Filter::new(FilterKind::DropShadow) };
    let bevel = Filter { side: filters::Side::Full, ..Filter::new(FilterKind::Bevel) };
    let list = [shadow, off, bevel, Filter::new(FilterKind::GradientGlow), Filter::new(FilterKind::AdjustColor)];
    let doc = doc(|i| {
        filters::set_filters(&mut i.extra, &list);
        i.extra.set("blendMode", Some("multiply"));
        i.extra.set(filters::CACHE_AS_BITMAP, Some("true"));
    });
    let places = placements(&doc);
    let p = &places[1];
    assert_eq!((p.version, p.blend, p.cached, p.visible), (3, Some(swf::BlendMode::Multiply), Some(true), Some(true)));
    let filters = p.filters.as_ref().unwrap();
    let [swf::Filter::DropShadowFilter(s), swf::Filter::BevelFilter(b), swf::Filter::GradientGlowFilter(g), swf::Filter::ColorMatrixFilter(m)] = &filters[..]
    else {
        panic!("the disabled blur is left out: {filters:?}");
    };
    assert_eq!((s.distance.to_f32(), s.strength.to_f32(), s.num_passes(), s.hide_object()), (6.0, 0.5, 3, false));
    assert!((s.angle.to_f32() - std::f32::consts::FRAC_PI_4).abs() < 1e-4, "radians");
    assert!(b.is_on_top() && !b.is_inner());
    assert_eq!(g.colors.iter().map(|c| (c.ratio, c.color.a)).collect::<Vec<_>>(), [(0, 0), (255, 255)]);
    let identity = swf::ColorMatrixFilter::default().matrix;
    assert!(m.matrix.iter().zip(identity).all(|(a, b)| (a - b).abs() < 1e-5), "Adjust Color at zero changes nothing: {:?}", m.matrix);
}

#[test]
fn filters_and_display_survive_saving() {
    let list = [Filter { blur: [7.5, 3.0], color: [0.2, 0.4, 0.6, 0.5], ..Filter::new(FilterKind::Glow) }, Filter::new(FilterKind::GradientBevel)];
    let doc = doc(|i| {
        filters::set_filters(&mut i.extra, &list);
        i.extra.set("blendMode", Some("overlay"));
        i.extra.set("isVisible", Some("false"));
    });
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("filters");
    let _ = std::fs::remove_dir_all(&dir);
    for path in [dir.join("folder"), dir.join("doc.fla")] {
        xfl_write::save(&doc, &path).unwrap();
        let back = xfl::load(&path).unwrap();
        assert_eq!(back, doc, "{}", path.display());
        let Element::Instance(i) = &back.scenes[0].layers[0].frames[0].elements[1] else { panic!() };
        assert_eq!(filters::filters(&i.extra), list);
        let fx = Effects::of(&i.extra);
        assert_eq!((fx.blend, fx.hidden), (Blend::Overlay, true));
    }
    let xml = std::fs::read_to_string(dir.join("folder/DOMDocument.xml")).unwrap();
    assert!(xml.contains(r#"<GlowFilter blurX="7.5" blurY="3""#) && xml.contains(r#"blendMode="overlay""#), "{xml}");
}
