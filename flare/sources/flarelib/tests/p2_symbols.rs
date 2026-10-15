//! P2 group 2 (Oct 15 2026): Visible / Cache as Bitmap / Export as Bitmap flags,
//! 9-slice scaling grid and layer effects (the model, editing and evaluation side;
//! pixel tests are in common/tvrender/tests/p2_symbols.rs).

use ac_core::commands::execute;
use ac_core::scale9::{apply, map_path, Axis};
use ac_core::*;
use serde_json::json;
use std::sync::Arc;

fn rect(x: f32, y: f32, w: f32, h: f32, c: Color) -> Element {
    Element::shape("", Shape::new(Path::rect(x, y, w, h), Some(c), None))
}

fn doc_with_instance(m: Matrix) -> (Document, SymbolId) {
    let mut d = Document { width: 400.0, height: 300.0, ..Default::default() };
    let mut tl = Timeline::new("Panel");
    tl.layers[0].keyframes.insert(0, Keyframe::with(vec![rect(0.0, 0.0, 100.0, 100.0, Color::rgb(0, 0, 255)), rect(0.0, 0.0, 10.0, 10.0, Color::rgb(255, 0, 0)), rect(90.0, 90.0, 10.0, 10.0, Color::rgb(255, 0, 0))]));
    tl.layers[0].end = 1;
    let id = d.add_symbol("Panel", SymbolKind::MovieClip, tl);
    d.scene.layers[0].keyframes.insert(0, Keyframe::with(vec![Element::instance("p", id, m)]));
    d.scene.layers[0].end = 1;
    (d, id)
}

fn items(d: &Document) -> Vec<DrawItem> {
    evaluate_scene(d, 0)
}

fn bounds(it: &DrawItem) -> (Pt, Pt) {
    it.path.transformed(&it.matrix).bounds().unwrap()
}

// ------------------------------------------------------------------ flags

#[test]
fn invisible_objects_are_skipped_by_renders_but_shown_while_authoring() {
    let (mut d, _) = doc_with_instance(Matrix::IDENTITY);
    assert_eq!(items(&d).len(), 3);
    d.scene.layers[0].keyframes.get_mut(&0).unwrap().elements[0].visible = false;
    assert!(items(&d).is_empty(), "an invisible instance draws nothing");
    // the authoring view (Editor::evaluate_current) still shows it so it can be selected
    let ed = Editor::new(d);
    assert_eq!(ed.evaluate_current(0).len(), 3);
}

#[test]
fn invisible_works_for_every_element_kind_and_nested_groups() {
    let mut e = rect(0.0, 0.0, 5.0, 5.0, Color::rgb(0, 0, 0));
    e.visible = false;
    let mut d = Document::default();
    d.scene.layers[0].keyframes.insert(0, Keyframe::with(vec![e, Element::group(vec![rect(0.0, 0.0, 5.0, 5.0, Color::rgb(0, 0, 0)), Element { visible: false, ..rect(9.0, 9.0, 5.0, 5.0, Color::rgb(0, 0, 0)) }])]));
    assert_eq!(items(&d).len(), 1, "one visible shape in the group");
}

#[test]
fn display_flags_serialise_only_when_set() {
    let (d, _) = doc_with_instance(Matrix::IDENTITY);
    let json = d.to_json();
    assert!(!json.contains("cache_as_bitmap") && !json.contains("export_as_bitmap") && !json.contains("\"visible\": false"), "defaults stay out of the file");
    let mut d2 = d.clone();
    let e = &mut d2.scene.layers[0].keyframes.get_mut(&0).unwrap().elements[0];
    e.visible = false;
    e.cache_as_bitmap = true;
    e.export_as_bitmap = true;
    let back = Document::from_json(&d2.to_json()).unwrap();
    assert_eq!(back, d2);
    // a document from before these fields loads with the defaults
    let old = Document::from_json(&json).unwrap();
    let e = &old.scene.layers[0].keyframes[&0].elements[0];
    assert!(e.visible && !e.cache_as_bitmap && !e.export_as_bitmap);
}

#[test]
fn display_flags_edit_as_one_undo_step() {
    let (d, _) = doc_with_instance(Matrix::IDENTITY);
    let mut ed = Editor::new(d);
    execute(&mut ed, "select.all", &json!({})).unwrap();
    let steps = ed.history().0.len();
    ed.set_selected_display(false, true, true);
    assert_eq!(ed.history().0.len(), steps + 1);
    let e = &ed.timeline().layers[0].keyframes[&0].elements[0];
    assert!(!e.visible && e.cache_as_bitmap && e.export_as_bitmap);
    ed.undo();
    let e = &ed.timeline().layers[0].keyframes[&0].elements[0];
    assert!(e.visible && !e.cache_as_bitmap);
    // the scripting command (undo cleared the selection)
    execute(&mut ed, "select.all", &json!({})).unwrap();
    execute(&mut ed, "selection.set_display", &json!({"visible": false})).unwrap();
    assert!(!ed.timeline().layers[0].keyframes[&0].elements[0].visible);
    assert!(execute(&mut ed, "selection.set_display", &json!({})).is_err());
}

#[test]
fn cache_as_bitmap_wraps_the_object_in_a_cached_group_without_moving_it() {
    let (mut d, _) = doc_with_instance(Matrix::translate(5.0, 6.0));
    let plain = items(&d);
    assert!(plain.iter().all(|i| i.fx.is_empty()));
    d.scene.layers[0].keyframes.get_mut(&0).unwrap().elements[0].cache_as_bitmap = true;
    let cached = items(&d);
    assert_eq!(cached.len(), plain.len());
    let g = cached[0].fx.first().expect("one group");
    assert!(g.cache && g.filters.is_empty());
    assert!(cached.iter().all(|i| i.fx.len() == 1 && Arc::ptr_eq(&i.fx[0], g)), "all of the object's items share the group");
    for (a, b) in plain.iter().zip(&cached) {
        assert_eq!(bounds(a), bounds(b));
    }
    // Export as bitmap is cached the same way; the empty object makes no group
    d.scene.layers[0].keyframes.get_mut(&0).unwrap().elements[0].cache_as_bitmap = false;
    d.scene.layers[0].keyframes.get_mut(&0).unwrap().elements[0].export_as_bitmap = true;
    assert!(items(&d)[0].fx[0].cache);
}

// ------------------------------------------------------------------ 9-slice

#[test]
fn axis_map_is_continuous_and_keeps_the_corners_at_size() {
    let ax = Axis::new(0.0, 100.0, 10.0, 90.0, 2.0).unwrap();
    assert_eq!(ax.map(0.0), 0.0);
    assert!((ax.map(10.0) - 10.0).abs() < 1e-4, "left corner keeps its size");
    assert!((ax.map(90.0) - 190.0).abs() < 1e-4, "right corner starts 10 from the scaled edge");
    assert!((ax.map(100.0) - 200.0).abs() < 1e-4, "outer edge is where plain scaling puts it");
    assert!((ax.map(50.0) - 100.0).abs() < 1e-4);
    let p = ax.pieces();
    for w in p.windows(2) {
        let edge = w[0].to;
        assert!((w[0].slope * edge + w[0].offset - (w[1].slope * edge + w[1].offset)).abs() < 1e-3, "continuous at {edge}");
    }
    // no scaling, an empty band, or a scale that crushes the corners: plain scaling
    assert!(Axis::new(0.0, 100.0, 10.0, 90.0, 1.0).is_none());
    assert!(Axis::new(0.0, 100.0, 50.0, 50.0, 2.0).is_none());
    assert!(Axis::new(0.0, 100.0, 40.0, 60.0, 0.1).is_none(), "corners would not fit");
}

#[test]
fn map_path_splits_segments_at_the_grid_lines() {
    let ax = Axis::new(0.0, 100.0, 25.0, 75.0, 2.0).unwrap();
    // a horizontal line across the grid keeps its end points and bends nowhere (y untouched)
    let line = Path { segs: vec![Seg::M(pt(0.0, 10.0)), Seg::L(pt(100.0, 10.0))] };
    let m = map_path(&line, Some(&ax), None);
    assert_eq!(m.segs.len(), 4, "split at x = 25 and x = 75: {:?}", m.segs);
    assert_eq!(m.segs.last(), Some(&Seg::L(pt(200.0, 10.0))));
    // a diagonal bends: (25, 25) stays 25 along x, (75, 25) goes to 175
    let diag = Path { segs: vec![Seg::M(pt(100.0, 0.0)), Seg::L(pt(0.0, 100.0))] };
    let ay = Axis::new(0.0, 100.0, 25.0, 75.0, 1.0);
    assert!(ay.is_none());
    let m = map_path(&diag, Some(&ax), None);
    let pts: Vec<Pt> = m.segs.iter().filter_map(|s| match s { Seg::M(p) | Seg::L(p) => Some(*p), _ => None }).collect();
    assert!(pts.iter().any(|p| (p.x - 175.0).abs() < 1e-3 && (p.y - 25.0).abs() < 1e-3), "{pts:?}");
    assert!(pts.iter().any(|p| (p.x - 25.0).abs() < 1e-3 && (p.y - 75.0).abs() < 1e-3), "{pts:?}");
    // curves are split exactly: the mapped curve still passes through the mapped middle point
    let c = Path { segs: vec![Seg::M(pt(0.0, 0.0)), Seg::C(pt(30.0, 60.0), pt(70.0, 60.0), pt(100.0, 0.0))] };
    let m = map_path(&c, Some(&ax), None);
    assert!(m.segs.len() >= 4);
    let flat: Vec<Pt> = m.flatten(16).into_iter().flat_map(|(v, _)| v).collect();
    assert!(flat.iter().any(|p| (p.x - 100.0).abs() < 1.5 && (p.y - 45.0).abs() < 1.5), "apex of the arch stays at its y, x is stretched");
    // closed shapes stay closed and the closing edge is mapped too
    let tri = Path::polygon(&[pt(0.0, 0.0), pt(100.0, 0.0), pt(0.0, 100.0)]);
    let m = map_path(&tri, Some(&ax), None);
    assert_eq!(m.segs.last(), Some(&Seg::Z));
    assert!(m.segs.len() > tri.segs.len(), "the closing edge was split");
}

#[test]
fn nine_slice_keeps_corners_and_stretches_the_middle() {
    let (mut d, id) = doc_with_instance(Matrix::scale(2.0, 1.5));
    // plain scaling first: the corner squares are 20 x 15
    let b = bounds(&items(&d)[1]);
    assert!((b.1.x - b.0.x - 20.0).abs() < 1e-3 && (b.1.y - b.0.y - 15.0).abs() < 1e-3);
    d.symbol_mut(id).unwrap().scale_grid = Some(ScaleGrid { left: 10.0, top: 10.0, right: 90.0, bottom: 90.0 });
    let its = items(&d);
    let (back, tl, br) = (bounds(&its[0]), bounds(&its[1]), bounds(&its[2]));
    assert!((back.1.x - 200.0).abs() < 1e-2 && (back.1.y - 150.0).abs() < 1e-2, "the whole panel is 200 x 150: {back:?}");
    assert!((tl.1.x - tl.0.x - 10.0).abs() < 1e-2 && (tl.1.y - tl.0.y - 10.0).abs() < 1e-2, "corners keep their size: {tl:?}");
    assert!((br.1.x - br.0.x - 10.0).abs() < 1e-2 && (br.1.y - br.0.y - 10.0).abs() < 1e-2);
    assert!((br.1.x - 200.0).abs() < 1e-2 && (br.1.y - 150.0).abs() < 1e-2, "bottom-right corner sits in the corner");
    // the instance matrix itself is unchanged: only the drawn geometry is
    assert_eq!(d.scene.layers[0].keyframes[&0].elements[0].matrix, Matrix::scale(2.0, 1.5));
}

#[test]
fn nine_slice_follows_rotation_and_position() {
    let m = Matrix::translate(50.0, 20.0).concat(&Matrix::rotate(std::f32::consts::FRAC_PI_2)).concat(&Matrix::scale(2.0, 1.0));
    let (mut d, id) = doc_with_instance(m);
    d.symbol_mut(id).unwrap().scale_grid = Some(ScaleGrid { left: 10.0, top: 10.0, right: 90.0, bottom: 90.0 });
    let its = items(&d);
    // rotated by 90 degrees: the corner squares are still 10 x 10
    let tl = bounds(&its[1]);
    assert!((tl.1.x - tl.0.x - 10.0).abs() < 1e-2 && (tl.1.y - tl.0.y - 10.0).abs() < 1e-2, "{tl:?}");
    // the panel is 200 long along the rotated x axis (which now points down)
    let back = bounds(&its[0]);
    assert!((back.1.y - back.0.y - 200.0).abs() < 1e-2 && (back.1.x - back.0.x - 100.0).abs() < 1e-2, "{back:?}");
}

#[test]
fn unscaled_instances_and_one_to_one_symbols_are_untouched() {
    let (mut d, id) = doc_with_instance(Matrix::translate(30.0, 30.0));
    let before = items(&d);
    d.symbol_mut(id).unwrap().scale_grid = Some(ScaleGrid { left: 10.0, top: 10.0, right: 90.0, bottom: 90.0 });
    let after = items(&d);
    for (a, b) in before.iter().zip(&after) {
        assert_eq!((a.path.clone(), a.matrix), (b.path.clone(), b.matrix));
    }
    // a degenerate grid is ignored
    d.symbol_mut(id).unwrap().scale_grid = Some(ScaleGrid { left: 50.0, top: 10.0, right: 50.0, bottom: 90.0 });
    d.scene.layers[0].keyframes.get_mut(&0).unwrap().elements[0].matrix = Matrix::scale(2.0, 2.0);
    let b = bounds(&items(&d)[1]);
    assert!((b.1.x - b.0.x - 20.0).abs() < 1e-3, "plain scaling");
}

#[test]
fn apply_handles_an_empty_symbol_and_strokes() {
    let mut v: Vec<DrawItem> = vec![];
    assert!(!apply(&mut v, 0, &Matrix::IDENTITY, &ScaleGrid { left: 1.0, top: 1.0, right: 2.0, bottom: 2.0 }, 2.0, 2.0));
    // stroke width does not stretch
    let mut tl = Timeline::new("S");
    tl.layers[0].keyframes.insert(0, Keyframe::with(vec![Element::shape("", Shape::new(Path::rect(0.0, 0.0, 100.0, 100.0), None, Some(Stroke { color: Color::rgb(0, 0, 0), width: 4.0 })))]));
    tl.layers[0].end = 1;
    let mut d = Document::default();
    let id = d.add_symbol("S", SymbolKind::MovieClip, tl);
    d.symbol_mut(id).unwrap().scale_grid = Some(ScaleGrid { left: 20.0, top: 20.0, right: 80.0, bottom: 80.0 });
    d.scene.layers[0].keyframes.insert(0, Keyframe::with(vec![Element::instance("", id, Matrix::scale(3.0, 3.0))]));
    d.scene.layers[0].end = 1;
    let its = evaluate_scene(&d, 0);
    assert_eq!(its[0].stroke.unwrap().width, 4.0, "an outline keeps its thickness");
    let b = bounds(&its[0]);
    assert!((b.1.x - 300.0).abs() < 1e-2);
}

#[test]
fn scale_grid_editing_is_validated_and_undoable() {
    let (d, id) = doc_with_instance(Matrix::IDENTITY);
    let mut ed = Editor::new(d);
    assert!(ed.set_symbol_scale_grid(id, Some(ScaleGrid { left: 50.0, top: 0.0, right: 40.0, bottom: 10.0 })).is_err());
    assert!(ed.set_symbol_scale_grid(99, None).is_err());
    let g = ed.default_scale_grid(id).unwrap();
    assert_eq!((g.left, g.top, g.right, g.bottom), (25.0, 25.0, 75.0, 75.0), "the middle half of the 100 x 100 panel");
    let steps = ed.history().0.len();
    ed.set_symbol_scale_grid(id, Some(g)).unwrap();
    ed.set_symbol_scale_grid(id, Some(g)).unwrap();
    assert_eq!(ed.history().0.len(), steps + 1, "one step, none for a no-op");
    assert_eq!(ed.doc.symbol(id).unwrap().scale_grid, Some(g));
    ed.undo();
    assert_eq!(ed.doc.symbol(id).unwrap().scale_grid, None);
    // commands
    execute(&mut ed, "symbol.set_scale_grid", &json!({"symbol": "Panel", "left": 10, "top": 12, "right": 90, "bottom": 88})).unwrap();
    assert_eq!(ed.doc.symbol(id).unwrap().scale_grid, Some(ScaleGrid { left: 10.0, top: 12.0, right: 90.0, bottom: 88.0 }));
    assert!(execute(&mut ed, "symbol.set_scale_grid", &json!({"symbol": "Panel", "left": 10})).is_err());
    execute(&mut ed, "symbol.set_scale_grid", &json!({"symbol": "Panel"})).unwrap();
    assert_eq!(ed.doc.symbol(id).unwrap().scale_grid, None);
    // saved with the document, absent when unused
    ed.set_symbol_scale_grid(id, Some(g)).unwrap();
    assert_eq!(Document::from_json(&ed.doc.to_json()).unwrap(), ed.doc);
    ed.set_symbol_scale_grid(id, None).unwrap();
    assert!(!ed.doc.to_json().contains("scale_grid"));
}

// ------------------------------------------------------------------ layer effects

#[test]
fn layer_effects_group_everything_the_layer_draws() {
    let mut d = Document { width: 100.0, height: 100.0, ..Default::default() };
    let l = &mut d.scene.layers[0];
    l.keyframes.insert(0, Keyframe::with(vec![rect(0.0, 0.0, 10.0, 10.0, Color::rgb(255, 0, 0)), rect(20.0, 0.0, 10.0, 10.0, Color::rgb(0, 0, 255))]));
    l.end = 1;
    l.filters = vec![Filter::Blur { blur_x: 4.0, blur_y: 4.0, quality: 1 }];
    let its = items(&d);
    assert_eq!(its.len(), 2);
    assert!(Arc::ptr_eq(&its[0].fx[0], &its[1].fx[0]), "both objects are one picture for the effect");
    assert_eq!(its[0].fx[0].filters, l_filters(&d));
    // element filters sit inside the layer's
    d.scene.layers[0].keyframes.get_mut(&0).unwrap().elements[0].filters = vec![Filter::Blur { blur_x: 1.0, blur_y: 1.0, quality: 1 }];
    let its = items(&d);
    assert_eq!(its[0].fx.len(), 2);
    assert_eq!(its[0].fx[0].filters, l_filters(&d), "the layer's effect is outermost");
    assert_eq!(its[1].fx.len(), 1);
    // layers without effects, hidden layers and outline view add no group
    let mut d2 = d.clone();
    d2.scene.layers[0].filters.clear();
    assert_eq!(items(&d2)[1].fx.len(), 0);
    d2.scene.layers[0].filters = l_filters(&d);
    d2.scene.layers[0].outline = true;
    assert!(items(&d2)[1].fx.is_empty(), "outline view draws plain outlines");
}

fn l_filters(d: &Document) -> Vec<Filter> {
    d.scene.layers[0].filters.clone()
}

#[test]
fn layer_effects_edit_save_and_script() {
    let mut ed = Editor::new(Document::default());
    ed.set_layer_filters(0, vec![]);
    assert!(ed.history().0.is_empty(), "no change, no undo step");
    let f = Filter::Glow { blur_x: 6.0, blur_y: 6.0, strength: 100.0, quality: 1, color: Color::rgb(255, 255, 0), knockout: false, inner: false };
    ed.set_layer_filters(0, vec![f.clone()]);
    assert_eq!(ed.history().0.len(), 1);
    assert_eq!(ed.timeline().layers[0].filters, vec![f.clone()]);
    assert_eq!(Document::from_json(&ed.doc.to_json()).unwrap(), ed.doc);
    ed.undo();
    assert!(ed.timeline().layers[0].filters.is_empty());
    assert!(!ed.doc.to_json().contains("\"filters\""), "no effects, nothing written");
    execute(&mut ed, "layer.add_filter", &json!({"type": "blur", "blur_x": 9, "blur_y": 3})).unwrap();
    execute(&mut ed, "layer.add_filter", &json!({"type": "drop_shadow", "distance": 7, "layer": 1})).unwrap();
    assert_eq!(ed.timeline().layers[0].filters.len(), 2);
    assert!(matches!(ed.timeline().layers[0].filters[0], Filter::Blur { blur_x, blur_y, .. } if blur_x == 9.0 && blur_y == 3.0));
    assert!(execute(&mut ed, "layer.add_filter", &json!({"type": "nope"})).is_err());
    execute(&mut ed, "layer.clear_filters", &json!({})).unwrap();
    assert!(ed.timeline().layers[0].filters.is_empty());
}

#[test]
fn an_invisible_object_still_has_a_centre_for_tweens() {
    let mut e = rect(100.0, 100.0, 20.0, 20.0, Color::rgb(0, 0, 0));
    e.visible = false;
    let d = Document::default();
    let c = element_local_center(&EvalCtx::new(&d, 0), &e);
    assert!((c.x - 110.0).abs() < 1e-3 && (c.y - 110.0).abs() < 1e-3, "{c:?}");
}
