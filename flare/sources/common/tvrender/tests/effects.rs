//! Phase 1 rendering: groups, masks, guides, filters, blend modes, bitmaps.

use ac_core::*;
use ac_render::render_frame;

fn px(doc: &Document, frame: u32, x: u32, y: u32) -> [u8; 4] {
    let p = render_frame(doc, frame, 1.0).pixel(x, y).unwrap().demultiply();
    [p.red(), p.green(), p.blue(), p.alpha()]
}

fn rect(x: f32, y: f32, w: f32, h: f32, c: Color) -> Element {
    Element::shape("r", Shape::new(Path::rect(x, y, w, h), Some(c), None))
}

fn doc_with(layers: Vec<Layer>) -> Document {
    let mut d = Document { width: 200.0, height: 200.0, ..Default::default() };
    d.scene.layers = layers;
    d
}

fn layer(name: &str, els: Vec<Element>) -> Layer {
    let mut l = Layer::new(name, 0);
    l.keyframes.insert(0, Keyframe::with(els));
    l.end = 10;
    l
}

const RED: Color = Color::rgb(255, 0, 0);
const BLUE: Color = Color::rgb(0, 0, 255);

#[test]
fn mask_layer_clips_children() {
    let mut mask = layer("mask", vec![rect(0.0, 0.0, 100.0, 200.0, BLUE)]);
    mask.kind = LayerKind::Mask;
    let mut content = layer("content", vec![rect(0.0, 0.0, 200.0, 200.0, RED)]);
    content.parent = Some(0);
    let d = doc_with(vec![mask, content]);
    assert_eq!(px(&d, 0, 50, 50), [255, 0, 0, 255], "inside the mask: content");
    assert_eq!(px(&d, 0, 150, 50), [255, 255, 255, 255], "outside: stage colour");
    // the mask shape itself isn't drawn
    let mut d2 = d.clone();
    d2.scene.layers[1].parent = None;
    assert_eq!(px(&d2, 0, 150, 50), [255, 0, 0, 255], "unmasked when unlinked");
    // hidden folder hides its children
    let mut f = d.clone();
    f.scene.layers[0].kind = LayerKind::Folder;
    f.scene.layers[0].visible = false;
    assert_eq!(px(&f, 0, 50, 50), [255, 255, 255, 255]);
}

#[test]
fn motion_guide_moves_along_path() {
    // guide: a vertical line x=150 from y=20 to y=180
    let mut guide = layer("guide", vec![Element::shape("g", Shape::new(Path { segs: vec![Seg::M(pt(150.0, 20.0)), Seg::L(pt(150.0, 180.0))] }, None, Some(Stroke { color: BLUE, width: 1.0 })))]);
    guide.kind = LayerKind::Guide;
    let mut a = rect(-5.0, -5.0, 10.0, 10.0, RED);
    a.matrix = Matrix::translate(150.0, 20.0);
    let mut b = a.clone();
    b.matrix = Matrix::translate(150.0, 180.0);
    let mut l = Layer::new("ball", 0);
    l.keyframes.insert(0, Keyframe { tween: Tween::Classic { ease: 0.0, rotate_turns: 0 }, ..Keyframe::with(vec![a]) });
    l.keyframes.insert(8, Keyframe::with(vec![b]));
    l.end = 9;
    l.parent = Some(0);
    let d = doc_with(vec![guide, l]);
    // halfway: the ball is on the guide at y≈100; the guide line itself isn't rendered
    assert_eq!(px(&d, 4, 150, 100), [255, 0, 0, 255]);
    assert_eq!(px(&d, 4, 150, 40), [255, 255, 255, 255], "guides don't render");
}

#[test]
fn groups_render_and_round_trip() {
    let mut ed = Editor::new(doc_with(vec![layer("l", vec![rect(10.0, 10.0, 20.0, 20.0, RED), rect(50.0, 50.0, 20.0, 20.0, BLUE)])]));
    ed.select_all();
    let before = ed.selection_bounds().unwrap();
    assert_eq!(ed.group_selected(), 1);
    assert_eq!(ed.selection.len(), 1);
    assert!(matches!(ed.selected_element().unwrap().kind, ElementKind::Group(_)));
    ed.modify_selected("Move", |e| e.matrix.tx += 5.0);
    assert_eq!(px(&ed.doc, 0, 17, 20), [255, 0, 0, 255], "group content moved together");
    assert_eq!(ed.ungroup_selected(), 1);
    assert_eq!(ed.selection.len(), 2);
    let after = ed.selection_bounds().unwrap();
    assert!((after.0.x - before.0.x - 5.0).abs() < 0.01 && (after.1.y - before.1.y).abs() < 0.01);
}

#[test]
fn align_left_and_distribute() {
    let mut ed = Editor::new(doc_with(vec![layer("l", vec![rect(10.0, 10.0, 20.0, 20.0, RED), rect(60.0, 80.0, 30.0, 20.0, BLUE), rect(150.0, 40.0, 10.0, 10.0, RED)])]));
    ed.select_all();
    ed.align(editor::AlignOp::Left, false);
    for i in 0..3 {
        assert!((ed.bounds_of(&[Selection { layer: 0, element: i }]).unwrap().0.x - 10.0).abs() < 0.01);
    }
    ed.align(editor::AlignOp::Bottom, true);
    for i in 0..3 {
        assert!((ed.bounds_of(&[Selection { layer: 0, element: i }]).unwrap().1.y - 200.0).abs() < 0.01);
    }
    ed.align(editor::AlignOp::MatchWidth, false);
    for i in 0..3 {
        let (lo, hi) = ed.bounds_of(&[Selection { layer: 0, element: i }]).unwrap();
        assert!((hi.x - lo.x - 30.0).abs() < 0.01);
    }
}

#[test]
fn filters_and_blend_modes() {
    let mut el = rect(50.0, 50.0, 60.0, 60.0, RED);
    el.filters.push(Filter::default_of("drop shadow").unwrap());
    let d = doc_with(vec![layer("l", vec![el.clone()])]);
    // shadow appears below-right of the box (45°, 5 px), darker than the stage
    let s = px(&d, 0, 113, 113);
    assert!(s[0] < 200 && s[1] < 200, "shadow drawn: {s:?}");
    assert_eq!(px(&d, 0, 80, 80), [255, 0, 0, 255], "object on top");

    let mut blur = el.clone();
    blur.filters = vec![Filter::Blur { blur_x: 20.0, blur_y: 20.0, quality: 2 }];
    let d = doc_with(vec![layer("l", vec![blur])]);
    let e = px(&d, 0, 50, 80);
    assert!(e[0] > 200 && e[1] > 60 && e[1] < 230, "edge softened: {e:?}");

    let mut mul = rect(0.0, 0.0, 200.0, 200.0, Color::rgb(128, 128, 128));
    mul.blend = BlendMode::Multiply;
    let d = doc_with(vec![layer("top", vec![mul]), layer("bottom", vec![rect(0.0, 0.0, 100.0, 200.0, RED)])]);
    let m = px(&d, 0, 50, 50);
    assert!(m[0] > 110 && m[0] < 140 && m[1] < 10, "multiply darkens red: {m:?}");

    let mut adj = rect(0.0, 0.0, 50.0, 50.0, RED);
    adj.filters = vec![Filter::AdjustColor { brightness: 0.0, contrast: 0.0, saturation: -100.0, hue: 0.0 }];
    let d = doc_with(vec![layer("l", vec![adj])]);
    let g = px(&d, 0, 20, 20);
    assert!(g[0] == g[1] && g[1] == g[2], "desaturated to grey: {g:?}");
}

#[test]
fn bitmaps_import_render_and_serialize() {
    // 2x2: red, green / blue, transparent
    let rgba = [255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 0, 0, 0, 0];
    let asset = BitmapAsset::from_rgba(0, "t.png", 2, 2, &rgba).unwrap();
    let mut ed = Editor::new(doc_with(vec![layer("l", vec![])]));
    let id = ed.add_bitmap(asset);
    let s = ed.place_bitmap(id, pt(10.0, 10.0)).unwrap();
    ed.selection = vec![s];
    ed.modify_selected("Scale", |e| e.matrix = e.matrix.concat(&Matrix::scale(20.0, 20.0)));
    ed.doc.bitmaps[0].smoothing = false;
    assert_eq!(px(&ed.doc, 0, 15, 15), [255, 0, 0, 255]);
    assert_eq!(px(&ed.doc, 0, 35, 15), [0, 255, 0, 255]);
    assert_eq!(px(&ed.doc, 0, 15, 35), [0, 0, 255, 255]);
    assert_eq!(px(&ed.doc, 0, 35, 35), [255, 255, 255, 255], "transparent pixel shows the stage");
    let json = ed.doc.to_json();
    let back = Document::from_json(&json).unwrap();
    assert_eq!(back.bitmaps[0].png, ed.doc.bitmaps[0].png);
    assert_eq!(ed.bitmap_usage(id), 1);
    ed.delete_bitmap(id);
    assert_eq!(ed.bitmap_usage(id), 0);
    // SVG export embeds images, clips and filters without panicking
    let svg = ac_render::export_svg(&back, 0);
    assert!(svg.contains("data:image/png;base64,"));
}

#[test]
fn layer_tree_survives_reordering() {
    let mut ed = Editor::new(doc_with(vec![layer("a", vec![]), layer("b", vec![]), layer("c", vec![])]));
    ed.layer = 1;
    let f = ed.wrap_layer(LayerKind::Folder, "Folder");
    assert_eq!(f, 1);
    assert_eq!(ed.timeline().layers[2].parent, Some(1));
    assert_eq!(ed.layer_depth(2), 1);
    // move "a" to the bottom: folder and child shift up, link kept
    ed.move_layer(0, 3);
    assert_eq!(ed.timeline().layers[0].name, "Folder");
    assert_eq!(ed.timeline().layers[1].parent, Some(0));
    // deleting the folder releases its child
    ed.layer = 0;
    ed.delete_layer();
    assert_eq!(ed.timeline().layers[0].parent, None);
}

#[test]
fn dropping_layers_into_folders() {
    let mut ed = Editor::new(doc_with(vec![layer("a", vec![]), layer("b", vec![]), layer("c", vec![])]));
    ed.layer = 0;
    let f = ed.wrap_layer(LayerKind::Folder, "F"); // F, a, b, c with a inside F
    assert_eq!((f, ed.timeline().layers[1].parent), (0, Some(0)));
    // drag "c" (index 3) to just under the folder: it joins the folder
    ed.drop_layer(3, 1);
    let names: Vec<&str> = ed.timeline().layers.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["F", "c", "a", "b"]);
    assert_eq!(ed.timeline().layers[1].parent, Some(0));
    // dragging the folder moves its children with it
    ed.drop_layer(0, 3);
    let names: Vec<&str> = ed.timeline().layers.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["b", "F", "c", "a"]);
    assert_eq!(ed.timeline().layers[2].parent, Some(1));
    assert_eq!(ed.timeline().layers[3].parent, Some(1));
}

#[test]
fn gradient_flow_extends_reflects_and_repeats() {
    // a black→white linear gradient over x 0..50 on a 200 px wide rectangle
    let mk = |spread: Spread| {
        let mut sh = Shape::new(Path::rect(0.0, 0.0, 200.0, 20.0), None, None);
        sh.gradient = Some(Gradient { spread, ..Gradient::fit(GradientKind::Linear, vec![(0.0, Color::rgb(0, 0, 0)), (1.0, Color::rgb(255, 255, 255))], pt(0.0, 0.0), pt(50.0, 20.0)) });
        doc_with(vec![layer("g", vec![Element::shape("", sh)])])
    };
    // at x = 60 (just past the end): extend = white, repeat = dark again, reflect = light
    let e = px(&mk(Spread::Pad), 0, 60, 10)[0];
    let r = px(&mk(Spread::Repeat), 0, 60, 10)[0];
    let f = px(&mk(Spread::Reflect), 0, 60, 10)[0];
    assert!(e > 245, "extend {e}");
    assert!(r < 60, "repeat {r}");
    assert!(f > 190 && f < 245, "reflect {f}");
}

#[test]
fn radial_focal_point_moves_the_centre() {
    // white centre → black edge over a 200 px circle; focal 0.7 moves the white towards +x
    let mk = |focal: f32| {
        let mut sh = Shape::new(Path::rect(0.0, 0.0, 200.0, 200.0), None, None);
        sh.gradient = Some(Gradient { focal, ..Gradient::fit(GradientKind::Radial, vec![(0.0, Color::rgb(255, 255, 255)), (1.0, Color::rgb(0, 0, 0))], pt(30.0, 30.0), pt(170.0, 170.0)) });
        doc_with(vec![layer("g", vec![Element::shape("", sh)])])
    };
    let centred = mk(0.0);
    let (l, r) = (px(&centred, 0, 60, 100)[0], px(&centred, 0, 140, 100)[0]);
    assert!((l as i32 - r as i32).abs() < 4, "symmetric {l} {r}");
    let f = mk(0.7);
    let (l, r) = (px(&f, 0, 60, 100)[0], px(&f, 0, 140, 100)[0]);
    assert!(r > l + 40, "brighter towards the focal point: left {l} right {r}");
    // every export carries it
    let svg = ac_render::export_svg(&f, 0);
    assert!(svg.contains("fx=\"0.7000\""), "svg");
    let lottie = ac_render::formats::lottie(&f, &Default::default()).0;
    assert!(lottie.contains("\"h\":{\"a\":0,\"k\":70"), "lottie");
    let files = ac_render::html5::publish(&f, &Default::default());
    assert!(files.iter().any(|(_, b)| String::from_utf8_lossy(b).contains("\"fx\":0.7")), "html5");
}

#[test]
fn artistic_strokes_render_as_geometry() {
    let ink = |dash: Dash| {
        let mut sh = Shape::new(Path { segs: vec![Seg::M(pt(20.0, 100.0)), Seg::L(pt(180.0, 100.0))] }, None, Some(Stroke { color: Color::rgb(0, 0, 0), width: 6.0 }));
        sh.stroke_style.dash = dash;
        let doc = doc_with(vec![layer("s", vec![Element::shape("", sh)])]);
        let img = render_frame(&doc, 0, 1.0);
        let mut n = 0;
        for y in 85..116 {
            for x in 0..200 {
                let p = img.pixel(x, y).unwrap().demultiply();
                if p.alpha() > 0 && p.red() < 128 {
                    n += 1;
                }
            }
        }
        n
    };
    let solid = ink(Dash::Solid);
    assert!(solid > 800, "solid {solid}");
    for d in [Dash::Ragged, Dash::Stipple, Dash::Hatched] {
        let n = ink(d);
        assert!(n > solid / 10 && n != solid, "{d:?}: {n} px vs solid {solid}");
    }
}

#[test]
fn still_images_in_three_formats() {
    use ac_render::{still_bytes, StillFormat};
    let sh = Shape::new(Path::rect(0.0, 0.0, 100.0, 100.0), Some(Color::rgb(200, 30, 30)), None);
    let doc = doc_with(vec![layer("a", vec![Element::shape("", sh)])]);
    let png = still_bytes(&doc, 0, 0.5, StillFormat::Png { transparent: true }).unwrap();
    assert_eq!(&png[1..4], b"PNG");
    let jpg = still_bytes(&doc, 0, 0.5, StillFormat::Jpeg { quality: 90 }).unwrap();
    assert_eq!(&jpg[..2], &[0xFF, 0xD8]);
    let small = still_bytes(&doc, 0, 0.5, StillFormat::Jpeg { quality: 10 }).unwrap();
    assert!(small.len() < jpg.len(), "lower quality, smaller file");
    let gif = still_bytes(&doc, 0, 0.5, StillFormat::Gif { colors: 4, transparent: true }).unwrap();
    assert_eq!(&gif[..3], b"GIF");
    let d = gif::DecodeOptions::new().read_info(std::io::Cursor::new(&gif)).unwrap();
    assert!(d.global_palette().unwrap().len() <= 4 * 3, "palette limited to 4 colours");
}

#[test]
fn gradient_glow_and_bevel_use_their_ramp() {
    let mk = |f: Filter| {
        let mut e = Element::shape("", Shape::new(Path::rect(60.0, 60.0, 80.0, 80.0), Some(Color::rgb(128, 128, 128)), None));
        e.filters = vec![f];
        doc_with(vec![layer("a", vec![e])])
    };
    // outer glow, transparent → pure green ramp, no offset: just outside the edge is greenish
    let glow = Filter::GradientGlow { blur_x: 10.0, blur_y: 10.0, strength: 200.0, quality: 1, angle: 0.0, distance: 0.0, knockout: false, kind: BevelType::Outer, stops: vec![(0.0, Color { r: 0, g: 255, b: 0, a: 0 }), (1.0, Color::rgb(0, 255, 0))] };
    let p = px(&mk(glow), 0, 57, 100);
    assert!(p[1] as i32 - p[0] as i32 > 80 && p[1] as i32 - p[2] as i32 > 80, "green glow outside (over the white stage): {p:?}");
    let inside = px(&mk(Filter::default_of("gradient glow").unwrap()), 0, 100, 100);
    assert_eq!(&inside[..3], &[128, 128, 128], "outer glow leaves the object alone");
    // inner bevel lit from the top-left (angle 45): top-left edge light, bottom-right dark
    let bevel = Filter::GradientBevel { blur_x: 6.0, blur_y: 6.0, strength: 200.0, quality: 1, angle: 45.0, distance: 4.0, knockout: false, kind: BevelType::Inner, stops: vec![(0.0, Color::rgb(255, 255, 255)), (0.5, Color { r: 0, g: 0, b: 0, a: 0 }), (1.0, Color::rgb(0, 0, 0))] };
    let d = mk(bevel);
    let (tl, br) = (px(&d, 0, 62, 62), px(&d, 0, 137, 137));
    assert!(tl[0] > 150 && br[0] < 100, "light {tl:?} dark {br:?}");
}
