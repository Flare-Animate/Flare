//! P2 group 2 (Oct 15 2026), pixel level: Visible flag, 9-slice scaling (vector
//! and bitmap), layer effects, and the HTML5 publish sharing all of it.

use ac_core::*;
use ac_render::html5::{publish, Html5Opts};
use ac_render::render_frame;

const RED: Color = Color::rgb(255, 0, 0);
const BLUE: Color = Color::rgb(0, 0, 255);
const GREEN: Color = Color::rgb(0, 255, 0);
const WHITE: [u8; 4] = [255, 255, 255, 255];

fn px(doc: &Document, x: u32, y: u32) -> [u8; 4] {
    let p = render_frame(doc, 0, 1.0).pixel(x, y).unwrap().demultiply();
    [p.red(), p.green(), p.blue(), p.alpha()]
}

fn rect(x: f32, y: f32, w: f32, h: f32, c: Color) -> Element {
    Element::shape("", Shape::new(Path::rect(x, y, w, h), Some(c), None))
}

fn doc(layers: Vec<Vec<Element>>) -> Document {
    let mut d = Document { width: 300.0, height: 200.0, ..Default::default() };
    d.scene.layers.clear();
    for (i, els) in layers.into_iter().enumerate() {
        let mut l = Layer::new(&format!("L{i}"), i);
        l.keyframes.insert(0, Keyframe::with(els));
        l.end = 1;
        d.scene.layers.push(l);
    }
    d
}

/// A 100 x 100 blue panel with red 10 x 10 corner squares, as a movie clip with a 10..90 grid.
fn panel_doc(m: Matrix, grid: bool) -> Document {
    let mut d = doc(vec![vec![]]);
    let mut tl = Timeline::new("Panel");
    let mut els = vec![rect(0.0, 0.0, 100.0, 100.0, BLUE)];
    for (x, y) in [(0.0, 0.0), (90.0, 0.0), (0.0, 90.0), (90.0, 90.0)] {
        els.push(rect(x, y, 10.0, 10.0, RED));
    }
    tl.layers[0].keyframes.insert(0, Keyframe::with(els));
    tl.layers[0].end = 1;
    let id = d.add_symbol("Panel", SymbolKind::MovieClip, tl);
    if grid {
        d.symbol_mut(id).unwrap().scale_grid = Some(ScaleGrid { left: 10.0, top: 10.0, right: 90.0, bottom: 90.0 });
    }
    d.scene.layers[0].keyframes.get_mut(&0).unwrap().elements = vec![Element::instance("", id, m)];
    d
}

#[test]
fn invisible_instances_are_not_rendered() {
    let mut d = doc(vec![vec![rect(10.0, 10.0, 50.0, 50.0, RED)]]);
    assert_eq!(px(&d, 30, 30), [255, 0, 0, 255]);
    d.scene.layers[0].keyframes.get_mut(&0).unwrap().elements[0].visible = false;
    assert_eq!(px(&d, 30, 30), WHITE);
}

#[test]
fn nine_slice_corners_keep_their_size_when_the_panel_is_stretched() {
    let m = Matrix::scale(2.0, 1.5);
    let sliced = panel_doc(m, true);
    let plain = panel_doc(m, false);
    // top-left corner: 10 x 10 red when sliced, 20 x 15 when scaled normally
    assert_eq!(px(&sliced, 5, 5), [255, 0, 0, 255]);
    assert_eq!(px(&sliced, 15, 5), [0, 0, 255, 255], "just past the 10 px corner: panel colour");
    assert_eq!(px(&plain, 15, 5), [255, 0, 0, 255], "plain scaling stretches the corner");
    // right edge and bottom edge corners sit in the corners of the 200 x 150 panel
    assert_eq!(px(&sliced, 195, 145), [255, 0, 0, 255]);
    assert_eq!(px(&sliced, 185, 145), [0, 0, 255, 255]);
    assert_eq!(px(&sliced, 195, 5), [255, 0, 0, 255]);
    assert_eq!(px(&sliced, 5, 145), [255, 0, 0, 255]);
    // the stretched centre reaches the full size; nothing outside the panel
    assert_eq!(px(&sliced, 100, 75), [0, 0, 255, 255]);
    assert_eq!(px(&sliced, 205, 75), WHITE);
    assert_eq!(px(&sliced, 100, 155), WHITE);
}

#[test]
fn nine_slice_edges_stretch_along_one_axis_only() {
    // a symbol with a thin frame line on its left edge, 6 px wide, grid 10..90
    let mut d = doc(vec![vec![]]);
    let mut tl = Timeline::new("Bar");
    tl.layers[0].keyframes.insert(0, Keyframe::with(vec![rect(0.0, 0.0, 100.0, 100.0, GREEN), rect(0.0, 0.0, 6.0, 100.0, BLUE)]));
    tl.layers[0].end = 1;
    let id = d.add_symbol("Bar", SymbolKind::MovieClip, tl);
    d.symbol_mut(id).unwrap().scale_grid = Some(ScaleGrid { left: 10.0, top: 10.0, right: 90.0, bottom: 90.0 });
    d.scene.layers[0].keyframes.get_mut(&0).unwrap().elements = vec![Element::instance("", id, Matrix::scale(1.0, 2.0))];
    // stretched vertically only: the 6 px bar keeps its width along the whole 200 px height
    assert_eq!(px(&d, 3, 100), [0, 0, 255, 255]);
    assert_eq!(px(&d, 8, 100), [0, 255, 0, 255], "6 px wide, not wider");
    assert_eq!(px(&d, 3, 190), [0, 0, 255, 255]);
    assert_eq!(px(&d, 50, 199), [0, 255, 0, 255]);
    assert_eq!(px(&d, 150, 100), WHITE);
}

#[test]
fn nine_slice_bends_a_diagonal_edge_across_the_grid_lines() {
    let mut d = doc(vec![vec![]]);
    let mut tl = Timeline::new("Tri");
    let tri = Path::polygon(&[pt(0.0, 0.0), pt(100.0, 0.0), pt(0.0, 100.0)]);
    tl.layers[0].keyframes.insert(0, Keyframe::with(vec![Element::shape("", Shape::new(tri, Some(RED), None))]));
    tl.layers[0].end = 1;
    let id = d.add_symbol("Tri", SymbolKind::MovieClip, tl);
    d.scene.layers[0].keyframes.get_mut(&0).unwrap().elements = vec![Element::instance("", id, Matrix::scale(2.0, 1.0))];
    // plain scaling: the hypotenuse is the straight line (200,0)-(0,100): at x = 60 it is at y = 70
    assert_eq!(px(&d, 60, 66), [255, 0, 0, 255]);
    d.symbol_mut(id).unwrap().scale_grid = Some(ScaleGrid { left: 25.0, top: 25.0, right: 75.0, bottom: 75.0 });
    // 9-slice: corners keep size, so the edge bends through (175, 25) and (25, 75): at x = 60 it is at y = 63.3
    assert_eq!(px(&d, 60, 66), WHITE, "outside the bent edge");
    assert_eq!(px(&d, 60, 58), [255, 0, 0, 255], "inside");
    assert_eq!(px(&d, 180, 10), [255, 0, 0, 255], "the top-right corner region keeps its 45 degree edge");
    assert_eq!(px(&d, 190, 15), WHITE);
}

#[test]
fn nine_slice_stretches_bitmaps_in_nine_pieces() {
    // 30 x 30: red / green / blue columns, 10 px each
    let mut rgba = vec![0u8; 30 * 30 * 4];
    for y in 0..30 {
        for x in 0..30 {
            let c = [[255, 0, 0], [0, 255, 0], [0, 0, 255]][x / 10];
            rgba[(y * 30 + x) * 4..][..4].copy_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    let mut d = doc(vec![vec![]]);
    d.bitmaps.push(ac_core::bitmap::BitmapAsset::from_rgba(1, "cols", 30, 30, &rgba).unwrap());
    let mut tl = Timeline::new("Img");
    tl.layers[0].keyframes.insert(0, Keyframe::with(vec![Element::bitmap("", 1, Matrix::IDENTITY)]));
    tl.layers[0].end = 1;
    let id = d.add_symbol("Img", SymbolKind::MovieClip, tl);
    d.scene.layers[0].keyframes.get_mut(&0).unwrap().elements = vec![Element::instance("", id, Matrix::scale(3.0, 1.0))];
    assert_eq!(px(&d, 15, 15), [255, 0, 0, 255], "plain scaling: red is 30 px wide");
    assert_eq!(px(&d, 40, 15), [0, 255, 0, 255]);
    d.symbol_mut(id).unwrap().scale_grid = Some(ScaleGrid { left: 10.0, top: 10.0, right: 20.0, bottom: 20.0 });
    assert_eq!(px(&d, 5, 15), [255, 0, 0, 255]);
    assert_eq!(px(&d, 15, 15), [0, 255, 0, 255], "red stops at 10");
    assert_eq!(px(&d, 70, 15), [0, 255, 0, 255], "the middle column took all the stretch");
    assert_eq!(px(&d, 85, 15), [0, 0, 255, 255], "blue is 10 px wide at the right edge (80..90)");
    assert_eq!(px(&d, 95, 15), WHITE);
}

#[test]
fn nine_slice_instances_hit_test_where_they_are_drawn() {
    let d = panel_doc(Matrix::scale(2.0, 1.5), true);
    let its = evaluate_scene(&d, 0);
    let hit = |p: Pt| its.iter().any(|i| i.hit(p));
    assert!(hit(pt(195.0, 145.0)));
    assert!(!hit(pt(205.0, 145.0)));
    assert!(hit(pt(100.0, 75.0)));
}

// ------------------------------------------------------------------ layer effects

#[test]
fn layer_blur_softens_only_that_layer() {
    let mut d = doc(vec![vec![rect(100.0, 80.0, 40.0, 40.0, RED)], vec![rect(10.0, 80.0, 40.0, 40.0, BLUE)]]);
    // without effects both edges are crisp
    assert_eq!(px(&d, 98, 100), WHITE);
    assert_eq!(px(&d, 8, 100), WHITE);
    d.scene.layers[0].filters = vec![Filter::Blur { blur_x: 12.0, blur_y: 12.0, quality: 2 }];
    let outside = px(&d, 96, 100);
    assert!(outside[3] == 255 && outside[1] < 250, "red bleeds past its edge: {outside:?}");
    let inside = px(&d, 120, 100);
    assert!(inside[0] == 255 && inside[1] < 40, "the centre stays red: {inside:?}");
    assert_eq!(px(&d, 8, 100), WHITE, "the other layer is untouched");
    assert_eq!(px(&d, 20, 100), [0, 0, 255, 255]);
}

#[test]
fn layer_effect_treats_the_whole_layer_as_one_picture() {
    // two overlapping half-transparent squares: a layer-level Adjust Color greys the combined result
    let mut a = rect(20.0, 20.0, 60.0, 60.0, RED);
    let mut b = rect(50.0, 20.0, 60.0, 60.0, BLUE);
    a.alpha = 1.0;
    b.alpha = 1.0;
    let mut d = doc(vec![vec![a, b]]);
    d.scene.layers[0].filters = vec![Filter::AdjustColor { brightness: 0.0, contrast: 0.0, saturation: -100.0, hue: 0.0 }];
    for (x, y) in [(30, 50), (65, 50), (100, 50)] {
        let p = px(&d, x, y);
        assert!(p[0].abs_diff(p[1]) <= 2 && p[1].abs_diff(p[2]) <= 2, "grey at ({x},{y}): {p:?}");
    }
    assert_eq!(px(&d, 150, 50), WHITE);
}

#[test]
fn layer_drop_shadow_follows_the_layer_content_and_hidden_layers_draw_nothing() {
    let mut d = doc(vec![vec![rect(40.0, 40.0, 40.0, 40.0, RED)]]);
    d.scene.layers[0].filters = vec![Filter::DropShadow { blur_x: 0.0, blur_y: 0.0, strength: 100.0, quality: 1, angle: 0.0, distance: 30.0, color: Color::rgb(0, 0, 0), knockout: false, inner: false, hide_object: false }];
    let shadow = px(&d, 100, 60);
    assert!(shadow[0] < 100 && shadow[3] == 255, "shadow 30 px to the right of the square: {shadow:?}");
    assert_eq!(px(&d, 60, 60), [255, 0, 0, 255], "the object stays on top");
    d.scene.layers[0].visible = false;
    assert_eq!(px(&d, 100, 60), WHITE);
}

#[test]
fn layer_effects_apply_inside_symbols_too() {
    let mut d = doc(vec![vec![]]);
    let mut tl = Timeline::new("Glowy");
    tl.layers[0].keyframes.insert(0, Keyframe::with(vec![rect(0.0, 0.0, 30.0, 30.0, RED)]));
    tl.layers[0].end = 1;
    tl.layers[0].filters = vec![Filter::Blur { blur_x: 10.0, blur_y: 10.0, quality: 2 }];
    let id = d.add_symbol("Glowy", SymbolKind::MovieClip, tl);
    d.scene.layers[0].keyframes.get_mut(&0).unwrap().elements = vec![Element::instance("", id, Matrix::translate(100.0, 100.0))];
    let p = px(&d, 97, 115);
    assert!(p[3] == 255 && p[1] < 250, "blurred beyond the symbol's edge: {p:?}");
}

// ------------------------------------------------------------------ HTML5

fn movie(files: &[(String, Vec<u8>)]) -> serde_json::Value {
    let page = String::from_utf8(files.iter().find(|f| f.0 == "movie.js").unwrap().1.clone()).unwrap();
    let json = page.trim_start_matches("var AC_MOVIE=").trim_end().trim_end_matches(';');
    serde_json::from_str(json).unwrap()
}

#[test]
fn html5_publish_uses_the_same_nine_slice_geometry() {
    let sliced = movie(&publish(&panel_doc(Matrix::scale(2.0, 1.5), true), &Html5Opts::default()));
    let plain = movie(&publish(&panel_doc(Matrix::scale(2.0, 1.5), false), &Html5Opts::default()));
    // the published display list holds the mapped geometry: the corner squares are 10 wide
    // when sliced and 20 wide when scaled normally
    let corner_widths = |m: &serde_json::Value| -> Vec<f32> {
        m["items"].as_array().unwrap().iter().filter(|it| it["f"].as_str().is_some_and(|f| f.contains("#ff0000"))).map(|it| {
            let p = m["paths"][it["p"].as_u64().unwrap() as usize].as_str().unwrap().to_string();
            let mat: Vec<f32> = it["m"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap() as f32).collect();
            let nums: Vec<f32> = p.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-')).filter_map(|t| t.parse().ok()).collect();
            let xs: Vec<f32> = nums.chunks(2).map(|c| c[0] * mat[0] + c[1] * mat[2] + mat[4]).collect();
            xs.iter().cloned().fold(f32::MIN, f32::max) - xs.iter().cloned().fold(f32::MAX, f32::min)
        }).collect()
    };
    let (s, p) = (corner_widths(&sliced), corner_widths(&plain));
    assert_eq!(s.len(), 4, "{sliced}");
    assert!(s.iter().all(|w| (w - 10.0).abs() < 0.05), "{s:?}");
    assert!(p.iter().all(|w| (w - 20.0).abs() < 0.05), "{p:?}");
}

#[test]
fn html5_publish_leaves_out_invisible_objects_and_carries_layer_effects() {
    let mut d = doc(vec![vec![rect(0.0, 0.0, 10.0, 10.0, RED), Element { visible: false, ..rect(20.0, 0.0, 10.0, 10.0, BLUE) }]]);
    d.scene.layers[0].filters = vec![Filter::Blur { blur_x: 6.0, blur_y: 6.0, quality: 1 }];
    let m = movie(&publish(&d, &Html5Opts::default()));
    assert_eq!(m["items"].as_array().unwrap().len(), 1, "the invisible square is not published");
    // the layer's filter runs through the same group/filter runtime as object filters
    let frame = m["frames"][0].to_string();
    assert!(frame.contains("\"x\":") && frame.contains("\"c\":["), "{frame}");
    assert!(m["fx"].as_array().unwrap().iter().any(|f| f.to_string().contains("blur(")), "{}", m["fx"]);
}

#[test]
fn html5_cache_as_bitmap_marks_the_group_and_dedupes_identical_lists() {
    let mut d = doc(vec![vec![rect(10.0, 10.0, 30.0, 30.0, RED)]]);
    d.scene.layers[0].end = 3;
    d.scene.layers[0].keyframes.get_mut(&0).unwrap().elements[0].cache_as_bitmap = true;
    let m = movie(&publish(&d, &Html5Opts::default()));
    let f0 = m["frames"][0].to_string();
    assert!(f0.contains("\"cb\":0"), "{f0}");
    assert_eq!(m["frames"][0], m["frames"][2], "the same cached object on every frame shares one cache id");
    // without the flag nothing is marked
    d.scene.layers[0].keyframes.get_mut(&0).unwrap().elements[0].cache_as_bitmap = false;
    let m = movie(&publish(&d, &Html5Opts::default()));
    assert!(!m["frames"][0].to_string().contains("\"cb\""));
    // and the runtime knows what to do with it
    let files = publish(&d, &Html5Opts::default());
    let rt = String::from_utf8(files.iter().find(|f| f.0 == "flare-runtime.js").unwrap().1.clone()).unwrap();
    assert!(rt.contains("function drawCached") && rt.contains("e.cb !== undefined"));
}
