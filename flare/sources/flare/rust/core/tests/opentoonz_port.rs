//! Features modelled on OpenToonz subsystems: Trace Bitmap, XDTS export,
//! stroke styles, deformation tools, drawing assistants.

use ac_core::commands::execute;
use ac_core::*;
use serde_json::json;

#[test]
fn trace_bitmap_replaces_the_bitmap_with_fills_in_one_step() {
    let mut ed = Editor::new(Document::default());
    // 30×20: left half red, right half blue
    let rgba: Vec<u8> = (0..20).flat_map(|_| (0..30).flat_map(|x| if x < 15 { [255, 0, 0, 255] } else { [0, 0, 255, 255] })).collect();
    let id = ed.add_bitmap(bitmap::BitmapAsset::from_rgba(0, "Flag", 30, 20, &rgba).unwrap());
    ed.place_bitmap(id, pt(100.0, 50.0)).unwrap();
    execute(&mut ed, "select.all", &json!({})).unwrap();
    let n = execute(&mut ed, "bitmap.trace", &json!({"color_threshold": 50, "curve_fit": "normal", "corners": "many"})).unwrap();
    assert_eq!(n, json!(2));
    let els = &ed.timeline().layers[0].keyframes[&0].elements;
    assert_eq!(els.len(), 1);
    let ElementKind::Canvas(c) = &els[0].kind else { panic!("a merge shape") };
    assert_eq!(c.fills.len(), 2);
    let (lo, hi) = c.bounds().unwrap();
    assert!((lo.x - 100.0).abs() < 0.6 && (hi.x - 130.0).abs() < 0.6 && (lo.y - 50.0).abs() < 0.6, "placed where the bitmap was: {lo:?} {hi:?}");
    ed.undo();
    assert!(matches!(ed.timeline().layers[0].keyframes[&0].elements[0].kind, ElementKind::Bitmap(_)));
    assert!(execute(&mut ed, "bitmap.trace", &json!({"curve_fit": "wobbly"})).is_err());
}

#[test]
fn xdts_export_is_readable_json_after_the_identifier() {
    let d = demo::demo_document();
    let text = xdts::export(&d, &d.scene);
    let (head, body) = text.split_once('\n').unwrap();
    assert_eq!(head, xdts::IDENTIFIER);
    let v: serde_json::Value = serde_json::from_str(body).unwrap();
    let names = v["timeTables"][0]["timeTableHeaders"][0]["names"].as_array().unwrap();
    assert!(!names.is_empty());
    assert_eq!(v["timeTables"][0]["fields"][0]["tracks"].as_array().unwrap().len(), names.len());
}

fn canvas_bounds(ed: &Editor, layer: usize) -> (Pt, Pt) {
    let kf = ed.timeline().layers[layer].keyframes.values().next().unwrap();
    let c = kf.elements.iter().find_map(|e| if let ElementKind::Canvas(c) = &e.kind { Some(c) } else { None }).unwrap();
    c.bounds().unwrap()
}

#[test]
fn merge_drawing_on_a_tweened_layer_lands_where_drawn() {
    // the demo's Ball layer is a classic tween of an instance; two merge
    // strokes in a row must both land where they were drawn
    let mut ed = Editor::new(demo::demo_document());
    ed.layer = ed.timeline().layers.iter().position(|l| l.name == "Ball").unwrap();
    let stroke = Some(Stroke { color: Color::rgb(0, 0, 0), width: 2.0 });
    assert!(ed.merge_draw(&Path { segs: vec![geom::Seg::M(pt(300.0, 150.0)), geom::Seg::L(pt(400.0, 150.0))] }, None, None, stroke));
    assert!(ed.merge_draw(&Path { segs: vec![geom::Seg::M(pt(300.0, 300.0)), geom::Seg::L(pt(400.0, 300.0))] }, None, None, stroke));
    let (lo, hi) = canvas_bounds(&ed, ed.layer);
    assert!((lo.y - 149.0).abs() < 2.0 && (hi.y - 301.0).abs() < 2.0 && (lo.x - 299.0).abs() < 2.0, "{lo:?} {hi:?}");
}

#[test]
fn stroke_style_command_sets_opentoonz_styles_and_params() {
    let mut ed = Editor::new(Document::default());
    execute(&mut ed, "drawing.mode", &json!({"merge": false})).unwrap();
    execute(&mut ed, "draw.line", &json!({"x1": 10, "y1": 10, "x2": 200, "y2": 10, "stroke_width": 8})).unwrap();
    execute(&mut ed, "select.all", &json!({})).unwrap();
    execute(&mut ed, "selection.stroke_style", &json!({"dash": "rope", "params": {"tilt": 0.9}, "colors": ["#ff0000"]})).unwrap();
    let ElementKind::Shape(s) = &ed.selected_element().unwrap().kind else { panic!() };
    assert_eq!(s.stroke_style.dash, Dash::Rope);
    assert_eq!(strokes::param(&s.stroke_style, 0), 0.9);
    assert_eq!(s.stroke_style.art.colors[0], Some(Color::rgb(255, 0, 0)));
    assert!(execute(&mut ed, "selection.stroke_style", &json!({"params": {"density": 1}})).is_err(), "Rope has no density");
    assert!(execute(&mut ed, "selection.stroke_style", &json!({"dash": "spaghetti"})).is_err());
    // the style survives a save round trip and renders as generated geometry
    let back: Document = serde_json::from_str(&serde_json::to_string(&ed.doc).unwrap()).unwrap();
    assert_eq!(back, ed.doc);
    let mut items = vec![];
    evaluate(&EvalCtx::new(&ed.doc, 0), &ed.doc.scene, 0, &Matrix::IDENTITY, 1.0, 0, &mut items);
    assert!(items.len() > 2 && items.iter().any(|i| i.fill == Some(Color::rgb(0, 0, 0))), "rope pieces + outline");
}

#[test]
fn deform_command_works_on_drawing_objects_and_merge_shapes() {
    let mut ed = Editor::new(Document::default());
    // merge shape: a filled square with an outline
    execute(&mut ed, "drawing.mode", &json!({"merge": true})).unwrap();
    execute(&mut ed, "draw.rect", &json!({"x": 100, "y": 100, "w": 100, "h": 100, "fill": "#ff0000", "stroke": "#000000"})).unwrap();
    let n = execute(&mut ed, "shape.deform", &json!({"tool": "magnet", "x": 150, "y": 100, "to_x": 150, "to_y": 60, "size": 30})).unwrap();
    assert_eq!(n, json!(1));
    let (lo, _) = canvas_bounds(&ed, 0);
    assert!((lo.y - 60.0).abs() < 2.0, "top edge pulled up: {lo:?}");
    ed.undo();
    assert!((canvas_bounds(&ed, 0).0.y - 99.0).abs() < 2.0, "one undo step");
    // pinch with a corner, then pump the outline
    execute(&mut ed, "shape.deform", &json!({"tool": "pinch", "x": 100, "y": 150, "to_x": 60, "to_y": 150, "size": 30, "sharp": true})).unwrap();
    assert!(canvas_bounds(&ed, 0).0.x < 62.0);
    execute(&mut ed, "shape.deform", &json!({"tool": "pump", "x": 200, "y": 150, "to_x": 200, "to_y": 110, "size": 20})).unwrap();
    let kf = &ed.timeline().layers[0].keyframes[&0];
    let ElementKind::Canvas(c) = &kf.elements[0].kind else { panic!() };
    assert!(c.strokes.iter().any(|s| s.width.is_some()), "pump adds a width profile");
    // drawing object: a line bent by the Bender
    execute(&mut ed, "drawing.mode", &json!({"merge": false})).unwrap();
    execute(&mut ed, "draw.line", &json!({"x1": 300, "y1": 300, "x2": 500, "y2": 300})).unwrap();
    execute(&mut ed, "shape.deform", &json!({"tool": "bender", "x": 400, "y": 280, "x2": 400, "y2": 320, "from_x": 480, "from_y": 300, "to_x": 400, "to_y": 380})).unwrap();
    let ElementKind::Shape(s) = &ed.timeline().layers[0].keyframes[&0].elements.last().unwrap().kind else { panic!() };
    let m = ed.timeline().layers[0].keyframes[&0].elements.last().unwrap().matrix;
    let (lo, hi) = s.path.transformed(&m).bounds().unwrap();
    assert!(hi.y > 390.0 && hi.x < 430.0 && lo.x < 301.0, "bent down where it crosses the segment: {lo:?} {hi:?}");
    assert!(execute(&mut ed, "shape.deform", &json!({"tool": "iron", "x": 900, "y": 900, "to_x": 910, "to_y": 900})).is_err());
}

#[test]
fn assistants_shape_scripted_strokes() {
    let mut ed = Editor::new(Document::default());
    execute(&mut ed, "drawing.mode", &json!({"merge": false})).unwrap();
    assert_eq!(execute(&mut ed, "assistant.add", &json!({"type": "star", "x": 275, "y": 200, "count": 5})).unwrap(), json!(1));
    execute(&mut ed, "draw.path", &json!({"points": [[275, 150], [285, 120]], "assist": true})).unwrap();
    assert_eq!(ed.timeline().layers[0].keyframes[&0].elements.len(), 5, "five copies round the star");
    execute(&mut ed, "assistant.set", &json!({"index": 1, "enabled": false})).unwrap();
    execute(&mut ed, "assistant.add", &json!({"type": "line", "x": 100, "y": 100, "parallel": true})).unwrap();
    execute(&mut ed, "draw.path", &json!({"points": [[50, 300], [120, 303], [200, 298], [260, 302]], "assist": true})).unwrap();
    let els = &ed.timeline().layers[0].keyframes[&0].elements;
    assert_eq!(els.len(), 6);
    let ElementKind::Shape(s) = &els[5].kind else { panic!() };
    let (lo, hi) = s.path.transformed(&els[5].matrix).bounds().unwrap();
    assert!(hi.y - lo.y < 0.5, "straightened onto a horizontal line: {lo:?} {hi:?}");
    let list = execute(&mut ed, "assistant.list", &json!({})).unwrap();
    assert_eq!(list.as_array().unwrap().len(), 2);
    execute(&mut ed, "assistant.remove", &json!({"index": "all"})).unwrap();
    assert!(ed.doc.assistants.is_empty());
}

#[test]
fn keyframe_types_and_expressions_drive_motion_curves() {
    use ac_core::motion::{Interp, PropKey, Track};
    let seg = |interp: Interp| Track { keys: vec![PropKey { interp, ..PropKey::new(0, 10.0, false) }, PropKey::new(10, 40.0, false)], ease: None };
    let at = |t: &Track, f: f32| t.value_at(f).unwrap();
    assert_eq!(at(&seg(Interp::Constant), 9.0), 10.0);
    assert_eq!(at(&seg(Interp::Linear), 5.0), 25.0);
    let e = seg(Interp::EaseInOut { ease_in: 4.0, ease_out: 4.0 });
    assert!(at(&e, 1.0) < 12.0 && (at(&e, 5.0) - 25.0).abs() < 1e-3 && at(&e, 9.0) > 38.0, "slow at both ends");
    let x = seg(Interp::Exponential);
    assert!((at(&x, 5.0) - (10.0f32 * 40.0).sqrt()).abs() < 1e-3, "geometric mean halfway");
    let ex = seg(Interp::Expression { text: "v0 + 5 * sin(frame * 90)".into() });
    assert!((at(&ex, 0.0) - 15.0).abs() < 1e-3, "frame 1 → sin 90°");
    let bad = seg(Interp::Expression { text: "nonsense(".into() });
    assert_eq!(at(&bad, 5.0), 25.0, "a broken expression falls back to linear");
    let sim = seg(Interp::SimilarShape { text: "t * t".into(), offset: 0.0 });
    assert!((at(&sim, 5.0) - (10.0 + 30.0 * 0.25)).abs() < 1e-3 && (at(&sim, 10.0) - 40.0).abs() < 1e-3);

    // through the command registry, with a reference to another property
    let mut ed = Editor::new(Document::default());
    execute(&mut ed, "draw.rect", &json!({"x": 0, "y": 0, "w": 40, "h": 40, "fill": "#3366FF"})).unwrap();
    execute(&mut ed, "select.all", &json!({})).unwrap();
    execute(&mut ed, "selection.to_symbol", &json!({"name": "Box", "kind": "movieclip"})).unwrap();
    execute(&mut ed, "timeline.set_tween", &json!({"kind": "motion"})).unwrap();
    execute(&mut ed, "motion.set_key", &json!({"prop": "rotation", "frame": 11, "value": 90})).unwrap();
    execute(&mut ed, "motion.set_key", &json!({"prop": "alpha", "frame": 1, "value": 1.0})).unwrap();
    execute(&mut ed, "motion.set_key", &json!({"prop": "alpha", "frame": 11, "value": 1.0})).unwrap();
    execute(&mut ed, "motion.set_interp", &json!({"prop": "rotation", "key": 1, "type": "constant"})).unwrap();
    // alpha (displayed in %) follows rotation: 100 − rotation/2
    execute(&mut ed, "motion.set_interp", &json!({"prop": "alpha", "key": 1, "type": "expression", "expression": "100 - rotation / 2"})).unwrap();
    let l = &ed.timeline().layers[0];
    let el = &layer_elements_at(l, 5).unwrap().0[0];
    assert!((el.alpha - 1.0).abs() < 1e-3, "rotation held at 0 → alpha 100%: {}", el.alpha);
    execute(&mut ed, "motion.set_interp", &json!({"prop": "rotation", "key": 1, "type": "linear"})).unwrap();
    let l = &ed.timeline().layers[0];
    let el = &layer_elements_at(l, 5).unwrap().0[0];
    assert!((el.alpha - 0.775).abs() < 0.01, "rotation 45° → alpha 77.5%: {}", el.alpha);
    assert!(execute(&mut ed, "motion.set_interp", &json!({"prop": "alpha", "key": 1, "type": "expression", "expression": "wobble(3)"})).is_err());
    assert!(execute(&mut ed, "motion.set_interp", &json!({"prop": "alpha", "key": 4, "type": "linear"})).is_err(), "no key there");
    // survives a save
    let back: Document = serde_json::from_str(&serde_json::to_string(&ed.doc).unwrap()).unwrap();
    assert_eq!(back, ed.doc);
}

#[test]
fn joint_limits_constrain_ik() {
    // three segments in a row, bones 0→1→2
    let mut ed = Editor::new(Document::default());
    let seg = |x: f32| Element::shape("seg", Shape::new(Path::rect(x, 0.0, 100.0, 20.0), Some(Color::rgb(0, 0, 0)), None));
    for x in [0.0, 100.0, 200.0] {
        ed.add_element(seg(x));
    }
    ed.add_bone(0, 0, 1, pt(5.0, 10.0), pt(100.0, 10.0)).unwrap();
    ed.add_bone(0, 1, 2, pt(100.0, 10.0), pt(200.0, 10.0)).unwrap();
    // the elbow (object 1 → 2) may bend at most 30° either way; the shoulder is locked
    execute(&mut ed, "bone.joint", &json!({"x": 250, "y": 10, "min": -30, "max": 30})).unwrap();
    execute(&mut ed, "bone.joint", &json!({"x": 150, "y": 10, "rotate": false})).unwrap();
    assert!(execute(&mut ed, "bone.joint", &json!({"x": 250, "y": 10, "min": 10, "max": 30})).is_err(), "range must include the pose");
    execute(&mut ed, "bone.drag", &json!({"x": 295, "y": 10, "to_x": 150, "to_y": 200})).unwrap();
    let els = ed.timeline().layers[0].keyframes[&0].elements.clone();
    let rel = |p: usize, c: usize| Bone::relative_angle(&els, p, c).unwrap();
    assert!(rel(1, 2).abs() <= 30.01, "elbow within its limit: {}", rel(1, 2));
    assert!(rel(1, 2).abs() > 25.0, "and bent as far as allowed: {}", rel(1, 2));
    assert!(rel(0, 1).abs() < 0.01, "locked shoulder didn't turn: {}", rel(0, 1));
    // (the root still swings the whole arm about its head; only joints are limited)
    ed.set_joint(0, 1, true, None);
    let b = ed.bones(0).to_vec();
    assert!(b.iter().all(|b| b.rotate) && b[1].limit == Some((-30.0, 30.0)) && b[0].limit.is_none());

    // shape bones: the second bone can't pass 45°
    let mut ed = Editor::new(Document::default());
    execute(&mut ed, "drawing.mode", &json!({"merge": false})).unwrap();
    execute(&mut ed, "draw.rect", &json!({"x": 0, "y": 0, "w": 200, "h": 20, "fill": "#000000"})).unwrap();
    execute(&mut ed, "shape_bone.add", &json!({"x1": 5, "y1": 10, "x2": 100, "y2": 10})).unwrap();
    execute(&mut ed, "shape_bone.add", &json!({"x1": 100, "y1": 10, "x2": 195, "y2": 10})).unwrap();
    assert_eq!(execute(&mut ed, "shape_bone.joint", &json!({"x": 195, "y": 10, "min": -45, "max": 45})).unwrap(), json!(2));
    execute(&mut ed, "shape_bone.joint", &json!({"x": 100, "y": 10, "rotate": false})).unwrap();
    execute(&mut ed, "shape_bone.pose", &json!({"x": 195, "y": 10, "to_x": 100, "to_y": 200})).unwrap();
    let ElementKind::Shape(s) = &ed.timeline().layers[0].keyframes[&0].elements[0].kind else { panic!() };
    let r = s.rig.as_ref().unwrap();
    assert_eq!(r.bones[0].angle, 0.0, "locked");
    assert!((r.bones[1].angle.to_degrees().abs() - 45.0).abs() < 0.01, "stopped at the limit: {}", r.bones[1].angle.to_degrees());
}

#[test]
fn raster_brush_paints_frame_by_frame_into_bitmaps() {
    let mut ed = Editor::new(Document::default());
    let brushes = execute(&mut ed, "paint.brushes", &json!({})).unwrap();
    assert!(brushes.as_array().unwrap().len() >= 20);
    let id = execute(&mut ed, "paint.stroke", &json!({"brush": "classic_pen", "color": "#cc2200", "size": 4, "points": [[50, 50], [200, 60, 1.0]]})).unwrap();
    let id = id.as_u64().unwrap() as bitmap::BitmapId;
    let kf0 = &ed.timeline().layers[0].keyframes[&0];
    assert_eq!(kf0.elements.len(), 1, "a new paint bitmap");
    let img = ed.doc.bitmap(id).unwrap().image().unwrap();
    assert_eq!((img.width, img.height), (550, 400), "stage sized");
    let at = |img: &bitmap::Image, x: u32, y: u32| img.rgba[((y * img.width + x) * 4 + 3) as usize];
    assert!(at(&img, 120, 55) > 150, "painted along the stroke");
    assert_eq!(at(&img, 120, 200), 0);
    // a second stroke on the same frame paints into the same bitmap (one undo step each)
    execute(&mut ed, "paint.stroke", &json!({"points": [[50, 150], [200, 150]], "size": 3})).unwrap();
    assert_eq!(ed.timeline().layers[0].keyframes[&0].elements.len(), 1);
    assert!(at(&ed.doc.bitmap(id).unwrap().image().unwrap(), 120, 150) > 0);
    ed.undo();
    assert_eq!(at(&ed.doc.bitmap(id).unwrap().image().unwrap(), 120, 150), 0, "undo removes the stroke");
    // frame by frame: a keyframe at frame 5 that copies frame 1, then paint there
    execute(&mut ed, "timeline.insert_keyframe", &json!({"frame": 5})).unwrap();
    let id2 = execute(&mut ed, "paint.stroke", &json!({"points": [[50, 300], [200, 300]], "size": 3})).unwrap().as_u64().unwrap() as bitmap::BitmapId;
    assert_ne!(id2, id, "the shared bitmap was copied before painting");
    assert_eq!(at(&ed.doc.bitmap(id).unwrap().image().unwrap(), 120, 300), 0, "frame 1 unchanged");
    assert!(at(&ed.doc.bitmap(id2).unwrap().image().unwrap(), 120, 300) > 0);
    // erasing
    execute(&mut ed, "paint.stroke", &json!({"brush": "classic_pen", "eraser": true, "size": 12, "points": [[120, 280], [120, 320]]})).unwrap();
    assert!(at(&ed.doc.bitmap(id2).unwrap().image().unwrap(), 120, 300) < 30);
    assert!(execute(&mut ed, "paint.stroke", &json!({"brush": "crayon-of-doom", "points": [[1, 1]]})).is_err());
}

#[test]
fn centerline_trace_turns_line_art_into_lines() {
    let mut ed = Editor::new(Document::default());
    // a white 60×40 bitmap with a 3 px black diagonal-ish stroke
    let (w, h) = (60u32, 40u32);
    let mut rgba = vec![255u8; (w * h * 4) as usize];
    for x in 5..55u32 {
        let y = 10 + (x - 5) * 20 / 50;
        for t in 0..3 {
            let i = (((y + t) * w + x) * 4) as usize;
            rgba[i..i + 3].copy_from_slice(&[0, 0, 0]);
        }
    }
    let id = ed.add_bitmap(bitmap::BitmapAsset::from_rgba(0, "Ink", w, h, &rgba).unwrap());
    ed.place_bitmap(id, pt(100.0, 100.0)).unwrap();
    execute(&mut ed, "select.all", &json!({})).unwrap();
    let n = execute(&mut ed, "bitmap.trace", &json!({"mode": "centerline", "curve_fit": "smooth"})).unwrap();
    assert_eq!(n, json!(1), "one line");
    let ElementKind::Canvas(c) = &ed.timeline().layers[0].keyframes[&0].elements[0].kind else { panic!("a merge shape") };
    assert!(c.fills.is_empty() && c.strokes.len() == 1);
    let s = &c.strokes[0];
    assert!((s.stroke.width - 3.0).abs() <= 1.0, "{}", s.stroke.width);
    let (a, b) = (s.pts[0], *s.pts.last().unwrap());
    let (l, r) = if a.x < b.x { (a, b) } else { (b, a) };
    assert!(l.dist(pt(106.0, 111.0)) < 4.0 && r.dist(pt(154.0, 131.0)) < 4.0, "placed on the stage: {l:?} {r:?}");
    assert!(execute(&mut ed, "bitmap.trace", &json!({"mode": "wireframe"})).is_err());
}

#[test]
fn pinned_joints_stay_put_while_posing() {
    // a four-segment chain; pin the second joint, then pull the tip around
    let mut ed = Editor::new(Document::default());
    let seg = |x: f32| Element::shape("seg", Shape::new(Path::rect(x, 0.0, 80.0, 16.0), Some(Color::rgb(0, 0, 0)), None));
    for k in 0..4 {
        ed.add_element(seg(k as f32 * 80.0));
    }
    for k in 0..3 {
        ed.add_bone(0, k, k + 1, pt(k as f32 * 80.0 + 4.0, 8.0), pt((k + 1) as f32 * 80.0, 8.0)).unwrap();
    }
    execute(&mut ed, "bone.joint", &json!({"x": 200, "y": 8, "pin": true})).unwrap(); // object 2 attaches at (160, 8)
    let joint = |ed: &Editor| {
        let els = &ed.timeline().layers[0].keyframes[&0].elements;
        let b = ed.bones(0).iter().find(|b| b.child == 2).copied().unwrap();
        els[2].matrix.apply(b.tail_child)
    };
    assert_eq!(ed.bones(0).iter().find(|b| b.child == 2).unwrap().pin, Some(pt(160.0, 8.0)));
    execute(&mut ed, "bone.drag", &json!({"x": 316, "y": 8, "to_x": 200, "to_y": 120})).unwrap();
    assert!(joint(&ed).dist(pt(160.0, 8.0)) < 1.5, "pinned joint held: {:?}", joint(&ed));
    let els = &ed.timeline().layers[0].keyframes[&0].elements;
    let tip = els[3].matrix.apply(pt(316.0, 8.0));
    assert!(tip.dist(pt(200.0, 120.0)) < 3.0, "the tip got there with the joints after the pin: {tip:?}");
    execute(&mut ed, "bone.joint", &json!({"x": tip.x, "y": tip.y, "pin": false})).ok();

    // shape bones: pin the first bone's tail
    let mut ed = Editor::new(Document::default());
    execute(&mut ed, "drawing.mode", &json!({"merge": false})).unwrap();
    execute(&mut ed, "draw.rect", &json!({"x": 0, "y": 0, "w": 300, "h": 20, "fill": "#000000"})).unwrap();
    for (a, b) in [(5, 100), (100, 200), (200, 295)] {
        execute(&mut ed, "shape_bone.add", &json!({"x1": a, "y1": 10, "x2": b, "y2": 10})).unwrap();
    }
    execute(&mut ed, "shape_bone.joint", &json!({"x": 100, "y": 10, "pin": true})).unwrap();
    execute(&mut ed, "shape_bone.pose", &json!({"x": 295, "y": 10, "to_x": 150, "to_y": 120})).unwrap();
    let ElementKind::Shape(s) = &ed.timeline().layers[0].keyframes[&0].elements[0].kind else { panic!() };
    let posed = s.rig.as_ref().unwrap().posed();
    assert!(posed[0].1.dist(pt(100.0, 10.0)) < 1.5, "pinned tail held: {:?}", posed[0].1);
    assert!(posed[2].1.dist(pt(150.0, 120.0)) < 3.0, "tip reached: {:?}", posed[2].1);
}

#[test]
fn xdts_import_reads_sheets_and_retimes_layers() {
    // round trip: our export reads back as the same timing
    let d = demo::demo_document();
    let sheet = xdts::parse(&xdts::export(&d, &d.scene)).unwrap();
    assert_eq!(sheet.duration, d.scene.frame_count());
    assert!(!sheet.tracks.is_empty());
    // a hand-written sheet: negative frame, ticks, hyphen, null cell, numbers with zeros
    let text = format!(
        "{}\n{}",
        xdts::IDENTIFIER,
        json!({"version": 5, "header": {"cut": "1", "scene": "1"}, "timeTables": [{"duration": 10, "name": "c1",
            "timeTableHeaders": [{"fieldId": 0, "names": ["Walk", "Fx"]}],
            "fields": [{"fieldId": 0, "tracks": [
                {"trackNo": 0, "frames": [
                    {"frame": 0, "data": [{"id": 0, "values": ["001"]}]},
                    {"frame": 2, "data": [{"id": 0, "values": ["2"]}]},
                    {"frame": 3, "data": [{"id": 0, "values": ["SYMBOL_TICK_1"]}]},
                    {"frame": 5, "data": [{"id": 0, "values": ["SYMBOL_NULL_CELL"]}]},
                    {"frame": 6, "data": [{"id": 0, "values": ["1"]}]}]},
                {"trackNo": 1, "frames": [
                    {"frame": -1, "data": [{"id": 0, "values": ["3"]}]},
                    {"frame": 4, "data": [{"id": 0, "values": ["SYMBOL_HYPHEN"]}]},
                    {"frame": 7, "data": [{"id": 0, "values": ["4"]}]}]}]}]}]})
    );
    let s = xdts::parse(&text).unwrap();
    let c = |t: usize| s.tracks[t].cells.iter().map(|c| c.clone().unwrap_or("-".into())).collect::<Vec<_>>().join("");
    assert_eq!(c(0), "11222-1111");
    assert_eq!(c(1), "3333333444");
    assert!(xdts::parse("{}").is_err());

    // apply: "Walk" re-times the layer of that name; "Fx" has no layer or images → labelled blanks
    let mut ed = Editor::new(Document::default());
    ed.rename_layer(0, "walk");
    let shape = |x: f32| Element::shape("", Shape::new(Path::rect(x, 0.0, 10.0, 10.0), Some(Color::rgb(0, 0, 0)), None));
    {
        let l = &mut ed.timeline_mut().layers[0];
        l.keyframes.insert(0, Keyframe::with(vec![shape(0.0)]));
        l.keyframes.insert(3, Keyframe::with(vec![shape(50.0)]));
        l.end = 6;
    }
    let rep = xdts::apply(&mut ed, &s, &[]);
    assert_eq!((rep.retimed.clone(), rep.timing_only.clone()), (vec!["Walk".to_string()], vec!["Fx".to_string()]));
    let tl = ed.timeline();
    assert_eq!(tl.layers.len(), 2);
    assert_eq!(tl.layers[0].name, "Fx");
    let walk = &tl.layers[1];
    assert_eq!(walk.keyframes.keys().copied().collect::<Vec<_>>(), [0, 2, 5, 6]);
    assert!(walk.keyframes[&5].elements.is_empty());
    assert_eq!(walk.keyframes[&2].elements[0].kind, shape(50.0).kind, "drawing 2 = second drawing");
    assert_eq!(walk.end, 10);
    assert_eq!(tl.layers[0].keyframes[&7].label.as_deref(), Some("4"));
    ed.undo();
    assert_eq!(ed.timeline().layers.len(), 1);
}

#[test]
fn xdts_import_loads_numbered_images_next_to_the_sheet() {
    let dir = std::env::temp_dir().join(format!("ac-xdts-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("B")).unwrap();
    let png = |r: u8| {
        let a = bitmap::BitmapAsset::from_rgba(0, "x", 4, 4, &[r, 0, 0, 255].repeat(16)).unwrap();
        a.png
    };
    std::fs::write(dir.join("A_0001.png"), png(10)).unwrap();
    std::fs::write(dir.join("A_0002.png"), png(20)).unwrap();
    std::fs::write(dir.join("B").join("0001.png"), png(30)).unwrap();
    let text = format!(
        "{}\n{}",
        xdts::IDENTIFIER,
        json!({"version": 5, "timeTables": [{"duration": 4, "timeTableHeaders": [{"fieldId": 0, "names": ["A", "B"]}],
            "fields": [{"fieldId": 0, "tracks": [
                {"trackNo": 0, "frames": [{"frame": 0, "data": [{"id": 0, "values": ["1"]}]}, {"frame": 2, "data": [{"id": 0, "values": ["2"]}]}]},
                {"trackNo": 1, "frames": [{"frame": 1, "data": [{"id": 0, "values": ["1"]}]}]}]}]}]})
    );
    std::fs::write(dir.join("cut.xdts"), text).unwrap();
    let mut ed = Editor::new(Document::default());
    let r = execute(&mut ed, "import.xdts", &json!({"path": dir.join("cut.xdts").to_string_lossy()})).unwrap();
    assert_eq!(r["from_images"], json!(["A", "B"]), "{r}");
    assert_eq!(ed.doc.bitmaps.len(), 3);
    let tl = ed.timeline();
    assert_eq!(tl.layers[0].name, "B");
    assert_eq!(tl.layers[1].keyframes.keys().copied().collect::<Vec<_>>(), [0, 2]);
    assert!(matches!(tl.layers[1].keyframes[&2].elements[0].kind, ElementKind::Bitmap(_)));
    assert!(tl.layers[0].keyframes[&0].elements.is_empty() && !tl.layers[0].keyframes[&1].elements.is_empty());
    ed.undo();
    assert_eq!(ed.timeline().layers.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}
