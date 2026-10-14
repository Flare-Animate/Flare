//! AnimationCraft core: document model, timeline evaluation, tweens, editing
//! commands. No UI or rendering dependencies (portable to wasm).

pub mod assist;
pub mod bitmap;
pub mod centerline;
#[cfg(feature = "codecs")]
pub mod codecs;
pub mod deform;
pub mod fx;
pub mod demo;
pub mod editor;
pub mod expr;
pub mod frames;
pub mod geom;
pub mod ik;
pub mod model;
pub mod merge;
pub mod rig;
pub mod sculpt;
pub mod sound;
pub mod strokes;
pub mod motion;
pub mod paint;
pub mod particles;
pub mod text;
pub mod trace;
pub mod xdts;
pub mod tween_edit;
pub mod commands;

pub use editor::{Editor, FrameSel, LayerFlag, Selection};
pub use geom::{pt, Matrix, Path, PointRef, Pt, Seg};
pub use model::*;

#[cfg(test)]
mod flare_frame_clock_tests;
#[cfg(test)]
mod flare_tween_boundary_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn matrix_decompose_roundtrip() {
        for m in [
            Matrix::IDENTITY,
            Matrix::translate(10.0, -4.0).concat(&Matrix::rotate(0.7)).concat(&Matrix::scale(2.0, 0.5)),
            Matrix::scale(-1.0, 1.0),
            Matrix::scale(1.0, -3.0).concat(&Matrix::rotate(-2.0)),
        ] {
            let r = Matrix::compose(&m.decompose());
            for (x, y) in [(r.a, m.a), (r.b, m.b), (r.c, m.c), (r.d, m.d), (r.tx, m.tx), (r.ty, m.ty)] {
                assert!(close(x, y), "{m:?} -> {r:?}");
            }
        }
    }

    #[test]
    fn matrix_invert() {
        let m = Matrix::translate(5.0, 7.0).concat(&Matrix::rotate(1.1)).concat(&Matrix::scale(2.0, 3.0));
        let p = pt(3.0, -2.0);
        let q = m.invert().unwrap().apply(m.apply(p));
        assert!(close(p.x, q.x) && close(p.y, q.y));
    }

    #[test]
    fn classic_tween_rotation_takes_shortest_path() {
        let a = Matrix::rotate(170f32.to_radians());
        let b = Matrix::rotate(-170f32.to_radians());
        let mid = a.interpolate(&b, 0.5, 0).decompose();
        assert!(close(mid.skew_y.abs(), PI), "went the long way: {}", mid.skew_y.to_degrees());
        let spun = Matrix::IDENTITY.interpolate(&Matrix::IDENTITY, 0.5, 1).decompose();
        assert!(close(spun.skew_y.abs(), PI), "rotate CW 1x should be half-turned at t=.5");
    }

    #[test]
    fn ease_endpoints_and_direction() {
        for e in [-100.0, -30.0, 0.0, 50.0, 100.0] {
            assert!(close(apply_ease(0.0, e), 0.0) && close(apply_ease(1.0, e), 1.0));
        }
        assert!(apply_ease(0.5, 100.0) > 0.5, "ease out is fast at the start");
        assert!(apply_ease(0.5, -100.0) < 0.5, "ease in is slow at the start");
    }

    #[test]
    fn tween_hits_keyframes_exactly() {
        let doc = demo::demo_document();
        let ball = &doc.scene.layers[0];
        for (&f, kf) in &ball.keyframes {
            let (els, _) = model::layer_elements_at(ball, f).unwrap();
            assert_eq!(els[0].matrix, kf.elements[0].matrix, "frame {f}");
        }
        // mid-tween lies between the two keyframes
        let (els, _) = model::layer_elements_at(ball, 4).unwrap();
        let x = els[0].matrix.tx;
        assert!(x > 50.0 && x < 150.0, "x={x}");
    }

    #[test]
    fn shape_tween_endpoints() {
        let a = Path::rect(0.0, 0.0, 10.0, 10.0);
        let b = Path::star(5.0, 5.0, 8.0, 3.0, 5);
        let m0 = geom::morph_paths(&a, &b, 0.0);
        let (lo, hi) = m0.bounds().unwrap();
        assert!(close(lo.x, 0.0) && close(hi.x, 10.0));
        let m1 = geom::morph_paths(&a, &b, 1.0);
        let (lo1, hi1) = m1.bounds().unwrap();
        let (blo, bhi) = b.bounds().unwrap();
        assert!(close(lo1.x, blo.x) && close(hi1.y, bhi.y));
    }

    #[test]
    fn graphic_symbol_syncs_to_parent() {
        let mut doc = Document::default();
        let mut tl = Timeline::new("blink");
        tl.layers[0].keyframes.insert(0, Keyframe::with(vec![Element::shape("a", Shape { path: Path::rect(0., 0., 1., 1.), fill: Some(Color::rgb(1, 0, 0)), stroke: None, ..Default::default() })]));
        tl.layers[0].keyframes.insert(2, Keyframe::with(vec![Element::shape("b", Shape { path: Path::rect(0., 0., 1., 1.), fill: Some(Color::rgb(0, 0, 1)), stroke: None, ..Default::default() })]));
        tl.layers[0].end = 4;
        let id = doc.add_symbol("blink", SymbolKind::Graphic, tl);
        doc.scene.layers[0].keyframes.insert(0, Keyframe::with(vec![Element::instance("i", id, Matrix::IDENTITY)]));
        doc.scene.layers[0].end = 10;
        let fill = |doc: &Document, f| model::evaluate_scene(doc, f)[0].fill.unwrap();
        assert_eq!(fill(&doc, 1), Color::rgb(1, 0, 0));
        assert_eq!(fill(&doc, 2), Color::rgb(0, 0, 1));
        assert_eq!(fill(&doc, 4), Color::rgb(1, 0, 0), "loops after 4 frames");
        if let ElementKind::Instance(i) = &mut doc.scene.layers[0].keyframes.get_mut(&0).unwrap().elements[0].kind {
            i.loop_mode = LoopMode::PlayOnce;
        }
        assert_eq!(fill(&doc, 9), Color::rgb(0, 0, 1), "play once holds last frame");
    }

    #[test]
    fn editor_keyframes_tweens_undo() {
        let mut ed = Editor::new(Document::default());
        ed.add_element(Element::shape("box", Shape { path: Path::rect(0., 0., 10., 10.), fill: Some(Color::rgb(0, 0, 0)), stroke: None, ..Default::default() }));
        ed.set_frame(9);
        ed.insert_keyframe();
        assert_eq!(ed.timeline().layers[0].end, 10);
        ed.selection = vec![Selection { layer: 0, element: 0 }];
        ed.modify_selected("Move", |e| e.matrix = Matrix::translate(100.0, 0.0));
        ed.set_frame(0);
        ed.set_tween(Tween::Classic { ease: 0.0, rotate_turns: 0 });
        let mid = ed.evaluate_current(5)[0].matrix.tx;
        assert!(close(mid, 100.0 * 5.0 / 9.0), "mid={mid}");
        ed.undo();
        assert_eq!(ed.evaluate_current(5)[0].matrix.tx, 0.0, "undo removed tween");
        ed.redo();
        assert!(ed.evaluate_current(5)[0].matrix.tx > 0.0);
    }

    #[test]
    fn insert_and_remove_frame_shift_keyframes() {
        let mut ed = Editor::new(Document::default());
        ed.set_frame(4);
        ed.insert_blank_keyframe();
        ed.set_frame(2);
        ed.insert_frame();
        assert!(ed.timeline().layers[0].keyframes.contains_key(&5));
        ed.remove_frame();
        assert!(ed.timeline().layers[0].keyframes.contains_key(&4));
    }

    #[test]
    fn convert_to_symbol_keeps_visual_position() {
        let mut ed = Editor::new(Document::default());
        ed.add_element(Element::shape("box", Shape { path: Path::rect(100., 50., 20., 20.), fill: Some(Color::rgb(0, 0, 0)), stroke: None, ..Default::default() }));
        let before = ed.selection_bounds().unwrap();
        let id = ed.convert_to_symbol("Box", SymbolKind::Graphic).unwrap();
        let after = ed.selection_bounds().unwrap();
        assert!(close(before.0.x, after.0.x) && close(before.1.y, after.1.y));
        assert!(matches!(ed.selected_element().unwrap().kind, ElementKind::Instance(Instance { symbol, .. }) if symbol == id));
        assert!(ed.hit_test(pt(110.0, 60.0)).is_some());
        assert!(ed.hit_test(pt(10.0, 10.0)).is_none());
    }


    fn boxed(x: f32) -> Element {
        Element::shape("box", Shape { path: Path::rect(x, 0., 10., 10.), fill: Some(Color::rgb(0, 0, 0)), stroke: None, ..Default::default() })
    }
    fn kfs(ed: &Editor, layer: usize) -> Vec<u32> {
        ed.timeline().layers[layer].keyframes.keys().copied().collect()
    }

    #[test]
    fn delete_removes_object_from_whole_span() {
        let mut ed = Editor::new(Document::default());
        ed.add_element(boxed(0.0));
        ed.set_frame(9);
        ed.insert_frame(); // static span 1..10
        ed.set_frame(5);
        assert_eq!(ed.evaluate_current(5).len(), 1);
        ed.select(ed.hit_test(pt(5.0, 5.0)), false);
        assert_eq!(ed.delete_selected(), 1);
        for f in [0, 5, 9] {
            assert!(ed.evaluate_current(f).is_empty(), "frame {f} still shows the box");
        }
        assert_eq!(kfs(&ed, 0), vec![0], "no stray keyframe created");
    }

    #[test]
    fn delete_inside_tween_removes_both_ends() {
        let mut doc = demo::demo_document();
        doc.scene.layers[0].locked = false;
        let mut ed = Editor::new(doc);
        ed.set_frame(4);
        let x = ed.evaluate_current(4).iter().find(|i| i.src == Some((0, 0))).unwrap().matrix.tx;
        let y = ed.evaluate_current(4).iter().find(|i| i.src == Some((0, 0))).unwrap().matrix.ty;
        ed.select(ed.hit_test(pt(x, y)), false);
        assert_eq!(ed.selection, vec![Selection { layer: 0, element: 0 }]);
        ed.delete_selected();
        for f in 0..=9 {
            assert!(ed.evaluate_current(f).iter().all(|i| i.src.map(|s| s.0) != Some(0)), "ball visible at {f}");
        }
        ed.undo();
        assert!(ed.evaluate_current(4).iter().any(|i| i.src == Some((0, 0))));
    }

    #[test]
    fn multi_select_marquee_move_delete() {
        let mut ed = Editor::new(Document::default());
        ed.add_element(boxed(0.0));
        ed.add_element(boxed(50.0));
        ed.add_element(boxed(200.0));
        ed.select_in_rect(pt(-5.0, -5.0), pt(70.0, 20.0), false);
        assert_eq!(ed.selection.len(), 2);
        ed.modify_selected("Move", |e| e.matrix.ty += 30.0);
        let (lo, _) = ed.selection_bounds().unwrap();
        assert!(close(lo.y, 30.0));
        ed.delete_selected();
        assert_eq!(ed.evaluate_current(0).len(), 1);
        ed.select_all();
        assert_eq!(ed.selection.len(), 1);
    }

    #[test]
    fn copy_paste_duplicate() {
        let mut ed = Editor::new(Document::default());
        ed.add_element(boxed(0.0));
        ed.duplicate();
        assert_eq!(ed.evaluate_current(0).len(), 2);
        let (lo, _) = ed.selection_bounds().unwrap();
        assert!(close(lo.x, 10.0), "duplicate is offset");
        ed.copy_selected();
        ed.paste(true);
        assert_eq!(ed.evaluate_current(0).len(), 3);
    }

    #[test]
    fn frame_selection_ops() {
        let mut ed = Editor::new(Document::default());
        ed.add_element(boxed(0.0));
        ed.add_layer();
        // two layers, select frames 5..=8 on both and convert to keyframes
        ed.select_frames(FrameSel { layer0: 0, layer1: 1, f0: 4, f1: 7 });
        ed.insert_keyframe();
        assert_eq!(kfs(&ed, 0), vec![0, 4, 5, 6, 7]);
        assert_eq!(kfs(&ed, 1), vec![0, 4, 5, 6, 7]);
        ed.clear_keyframe();
        assert_eq!(kfs(&ed, 1), vec![0]);
        assert_eq!(ed.timeline().layers[1].end, 8);
        // remove those frames again
        ed.remove_frame();
        assert_eq!(ed.timeline().layers[1].end, 4);
    }

    #[test]
    fn clear_frames_keeps_later_content() {
        let mut ed = Editor::new(Document::default());
        ed.add_element(boxed(0.0));
        ed.set_frame(9);
        ed.insert_frame();
        ed.select_frames(FrameSel::between((0, 2), (0, 4)));
        ed.clear_frames();
        assert_eq!(ed.evaluate_current(1).len(), 1);
        assert!(ed.evaluate_current(3).is_empty());
        assert_eq!(ed.evaluate_current(6).len(), 1, "frames after the cleared block keep their content");
    }

    #[test]
    fn move_and_paste_frames() {
        let mut ed = Editor::new(Document::default());
        ed.set_frame(4);
        ed.add_element(boxed(0.0)); // creates keyframe at 4 (past end)
        assert_eq!(kfs(&ed, 0), vec![0, 4]);
        ed.select_frames(FrameSel::cell(0, 4));
        ed.move_frames(3);
        assert_eq!(kfs(&ed, 0), vec![0, 7]);
        assert!(ed.evaluate_current(4).is_empty());
        assert_eq!(ed.evaluate_current(7).len(), 1);
        ed.copy_frames();
        ed.select_frames(FrameSel::cell(0, 12));
        ed.paste_frames();
        assert_eq!(ed.evaluate_current(12).len(), 1);
        assert!(ed.timeline().layers[0].end >= 13);
    }

    #[test]
    fn dragging_a_tween_keyframe_stretches_spans() {
        let mut ed = Editor::new(demo::demo_document());
        ed.select_frames(FrameSel::cell(0, 9));
        ed.move_frames(4);
        assert_eq!(kfs(&ed, 0), vec![0, 13, 18, 27, 36, 47], "no blank or pinned keyframes left behind");
        let x = |f| ed.evaluate_current(f).iter().find(|i| i.src == Some((0, 0))).unwrap().matrix.tx;
        assert!(x(11) > x(0) && x(11) < x(13), "frames 10-13 now belong to the first tween");
        assert!(matches!(ed.timeline().layers[0].keyframes[&13].tween, Tween::Classic { .. }));
    }

    #[test]
    fn tween_on_selection() {
        let mut ed = Editor::new(Document::default());
        ed.add_element(boxed(0.0));
        ed.set_frame(10);
        ed.insert_keyframe();
        ed.select_all();
        ed.modify_selected("Move", |e| e.matrix.tx = 100.0);
        ed.select_frames(FrameSel::cell(0, 3));
        ed.set_tween(Tween::Classic { ease: 0.0, rotate_turns: 0 });
        assert!(ed.evaluate_current(5)[0].matrix.tx > 0.0);
        ed.set_tween_params(100.0, 0);
        assert!(matches!(ed.timeline().layers[0].keyframes[&0].tween, Tween::Classic { ease, .. } if ease == 100.0));
        assert!(matches!(ed.timeline().layers[0].keyframes[&10].tween, Tween::None), "closing keyframe has no tween");
    }

    #[test]
    fn library_delete_removes_instances() {
        let mut ed = Editor::new(demo::demo_document());
        let ball = ed.doc.library.iter().find(|s| s.name == "Ball").unwrap().id;
        assert!(ed.symbol_usage(ball) > 0);
        ed.delete_symbol(ball);
        assert!(ed.doc.symbol(ball).is_none());
        assert_eq!(ed.symbol_usage(ball), 0);
        ac_core_render_smoke(&ed);
    }

    fn ac_core_render_smoke(ed: &Editor) {
        for f in 0..ed.frame_count() {
            let _ = ed.evaluate_current(f);
        }
    }


    #[test]
    fn text_outlines_and_hits() {
        let t = text::text_path("Hi", 40.0, text::FontFace::Sans, text::Align::Left, 0.0);
        let (lo, hi) = t.bounds().unwrap();
        assert!(hi.x - lo.x > 15.0 && hi.y - lo.y > 20.0, "{lo:?} {hi:?}");
        let mut ed = Editor::new(Document::default());
        ed.add_element(Element::text("Hello", 40.0, Color::rgb(0, 0, 0), pt(100.0, 100.0)));
        let (lo, _) = ed.selection_bounds().unwrap();
        assert!(lo.x >= 100.0 && lo.y >= 100.0);
        assert!(!ed.evaluate_current(0)[0].path.segs.is_empty());
    }

    #[test]
    fn shape_builders() {
        let r = Path::rounded_rect(0.0, 0.0, 100.0, 50.0, 10.0);
        let (lo, hi) = r.bounds().unwrap();
        assert!(close(lo.x, 0.0) && close(hi.x, 100.0) && close(hi.y, 50.0));
        assert!(!r.hit(pt(0.5, 0.5), true, 0.0), "corner is rounded off");
        let star = Path::polystar(0.0, 0.0, 10.0, 10.0, 5, true, 0.5);
        assert_eq!(star.anchors().len(), 10);
        let hex = Path::polystar(0.0, 0.0, 10.0, 10.0, 6, false, 0.5);
        assert_eq!(hex.anchors().len(), 6);
    }

    #[test]
    fn anchor_editing() {
        let mut p = Path::rect(0.0, 0.0, 10.0, 10.0);
        assert_eq!(p.anchors().len(), 4);
        let r = p.pick_point(pt(10.2, 9.8), 1.0, true).unwrap();
        p.set_point(r, pt(20.0, 20.0));
        assert!(close(p.bounds().unwrap().1.x, 20.0));
        let i = p.insert_anchor(pt(5.0, 0.0), 1.0).unwrap();
        assert_eq!(p.anchors().len(), 5);
        assert!(p.delete_anchor(i + 1));
        assert_eq!(p.anchors().len(), 4);
        p.convert_anchor(1);
        assert!(p.segs.iter().any(|s| matches!(s, Seg::C(..))), "corner became smooth");
        p.convert_anchor(1);
        assert!(!p.segs.iter().any(|s| matches!(s, Seg::C(..))), "and back to a corner");
        // moving the start anchor of a closed shape keeps it closed
        let mut q = Path::ellipse(0.0, 0.0, 10.0, 10.0);
        q.set_point(PointRef::Anchor(0), pt(15.0, 0.0));
        assert_eq!(q.anchors().len(), 4);
        assert!(close(q.bounds().unwrap().1.x, 15.0));
    }

    #[test]
    fn command_registry() {
        use serde_json::json;
        let mut ed = Editor::new(Document::default());
        let r = commands::execute(&mut ed, "draw.rect", &json!({"x": 10, "y": 10, "w": 50, "h": 20, "fill": "#FF0000"})).unwrap();
        assert_eq!(r["layer"], 1);
        commands::execute(&mut ed, "draw.text", &json!({"x": 10, "y": 60, "text": "Hi"})).unwrap();
        assert_eq!(commands::execute(&mut ed, "select.all", &json!({})).unwrap(), 2);
        commands::execute(&mut ed, "select.at", &json!({"x": 20, "y": 20})).unwrap();
        commands::execute(&mut ed, "selection.set", &json!({"fill": "#00FF00", "alpha": 0.5})).unwrap();
        let info = commands::execute(&mut ed, "selection.info", &json!(null)).unwrap();
        assert_eq!(info[0]["props"]["fill"], "#00FF00");
        commands::execute(&mut ed, "timeline.insert_keyframe", &json!({"frame": 10})).unwrap();
        commands::execute(&mut ed, "timeline.set_tween", &json!({"kind": "classic", "frame": 1})).unwrap();
        let layers = commands::execute(&mut ed, "layer.list", &json!({})).unwrap();
        assert_eq!(layers[0]["keyframes"], json!([1, 10]));
        commands::execute(&mut ed, "timeline.set_script", &json!({"frame": 10, "script": "this.stop();"})).unwrap();
        assert!(ed.timeline().layers[0].keyframes[&9].script.is_some());
        // errors are messages, not panics
        assert!(commands::execute(&mut ed, "draw.rect", &json!({"x": 1})).unwrap_err().contains("missing"));
        assert!(commands::execute(&mut ed, "timeline.goto", &json!({"frame": 0})).is_err());
        assert!(commands::execute(&mut ed, "nope", &json!({})).is_err());
        assert!(commands::execute(&mut ed, "layer.select", &json!({"layer": "Nope"})).is_err());
        // every registered command id is dispatched
        for c in commands::COMMANDS {
            let e = commands::execute(&mut ed, c.id, &json!({}));
            assert!(!matches!(&e, Err(m) if m.starts_with("unknown command")), "{} not dispatched", c.id);
        }
    }

    #[test]
    fn undo_squash_groups_a_script() {
        let mut ed = Editor::new(Document::default());
        let mark = ed.undo_depth();
        for i in 0..5 {
            commands::execute(&mut ed, "draw.rect", &serde_json::json!({"x": i * 10, "y": 0, "w": 5, "h": 5})).unwrap();
        }
        ed.squash_undo(mark, "Run Script");
        assert_eq!(ed.undo_label(), Some("Run Script"));
        ed.undo();
        assert!(ed.evaluate_current(0).is_empty(), "one undo removes everything the script did");
    }

    #[test]
    fn old_documents_still_load() {
        // keyframes saved before `script` and text existed
        let doc = demo::demo_document();
        let json = doc.to_json().replace(",\n          \"script\": null", "");
        assert!(Document::from_json(&json).is_ok());
    }


    #[test]
    fn distort_and_envelope_identity_and_warp() {
        let p = Path::rect(0.0, 0.0, 100.0, 50.0);
        let (lo, hi) = (pt(0.0, 0.0), pt(100.0, 50.0));
        let same = deform::distort(&p, lo, hi, &[pt(0.0, 0.0), pt(100.0, 0.0), pt(100.0, 50.0), pt(0.0, 50.0)]);
        let (a, b) = same.bounds().unwrap();
        assert!(close(a.x, 0.0) && close(b.x, 100.0) && close(b.y, 50.0));
        let skewed = deform::distort(&p, lo, hi, &[pt(20.0, 0.0), pt(120.0, 0.0), pt(100.0, 50.0), pt(0.0, 50.0)]);
        assert!(close(skewed.bounds().unwrap().1.x, 120.0));
        let g = deform::envelope_grid(lo, hi);
        let env = deform::envelope(&p, lo, hi, &g);
        let (a, b) = env.bounds().unwrap();
        assert!(close(a.x, 0.0) && close(b.x, 100.0), "identity envelope keeps the shape");
        let mut g2 = g;
        g2[1] = pt(50.0, -30.0); // pull the top-middle handle up
        let bulged = deform::envelope(&p, lo, hi, &g2);
        let (blo, _) = bulged.bounds().unwrap();
        assert!(blo.y < -25.0 && blo.y > -35.0, "top edge passes through the handle: {}", blo.y);
        assert!(bulged.hit(pt(2.0, 1.0), true, 0.5), "corners stay put");
    }

    #[test]
    fn variable_width_outline() {
        let line = Path { segs: vec![Seg::M(pt(0.0, 0.0)), Seg::L(pt(100.0, 0.0))] };
        let o = deform::variable_stroke(&line, &[(0.0, 2.0), (1.0, 20.0)]);
        let (lo, hi) = o.bounds().unwrap();
        assert!(hi.y > 9.0 && lo.y < -9.0, "thick end");
        assert!(o.hit(pt(5.0, 0.0), true, 0.0) && !o.hit(pt(5.0, 5.0), true, 0.0), "thin start");
        // a shape with a width profile renders its stroke as a filled outline
        let mut ed = Editor::new(Document::default());
        let mut sh = Shape::new(line.clone(), None, Some(Stroke { color: Color::rgb(0, 0, 0), width: 4.0 }));
        sh.width_profile = Some(vec![(0.0, 0.5), (1.0, 3.0)]);
        ed.add_element(Element::shape("w", sh));
        let items = ed.evaluate_current(0);
        assert!(items.iter().any(|i| i.fill.is_some() && i.stroke.is_none()));
        // a width point in the middle of a straight segment bulges it there
        let mid = deform::variable_stroke(&line, &[(0.0, 1.0), (0.5, 30.0), (1.0, 1.0)]);
        assert!(mid.hit(pt(50.0, 12.0), true, 0.0) && !mid.hit(pt(5.0, 5.0), true, 0.0), "bulge at t=0.5");
    }

    #[test]
    fn asset_warp_pins() {
        let base = Path::rect(0.0, 0.0, 100.0, 20.0);
        let mut w = deform::Warp::new(&base);
        w.pins = vec![(pt(0.0, 10.0), pt(0.0, 10.0)), (pt(100.0, 10.0), pt(100.0, 60.0))];
        let out = w.apply();
        let (_, hi) = out.bounds().unwrap();
        assert!(hi.y > 50.0, "pulled end follows its pin");
        assert!(out.hit(pt(1.0, 10.0), true, 2.0), "pinned end stays");
        w.pins.truncate(1);
        w.pins[0].1 = pt(10.0, 10.0);
        assert!(close(w.apply().bounds().unwrap().0.x, 10.0), "one pin = move");
    }

    #[test]
    fn three_d_projection_and_tween() {
        let mut doc = Document::default();
        let mut el = Element::shape("card", Shape::new(Path::rect(-50.0, -50.0, 100.0, 100.0), Some(Color::rgb(255, 0, 0)), None));
        el.matrix = Matrix::translate(275.0, 200.0);
        let mut flipped = el.clone();
        flipped.three_d = Some(deform::Transform3D { ry: 60.0, ..Default::default() });
        doc.scene.layers[0].keyframes.insert(0, Keyframe { tween: Tween::Classic { ease: 0.0, rotate_turns: 0 }, ..Keyframe::with(vec![el]) });
        doc.scene.layers[0].keyframes.insert(10, Keyframe::with(vec![flipped]));
        doc.scene.layers[0].end = 11;
        let w = |f| { let it = &evaluate_scene(&doc, f)[0]; let (lo, hi) = it.path.transformed(&it.matrix).bounds().unwrap(); hi.x - lo.x };
        assert!(close(w(0), 100.0));
        assert!(w(10) < 60.0, "60° around Y foreshortens: {}", w(10));
        assert!(w(5) < w(0) && w(5) > w(10), "tween interpolates the 3D rotation");
        // farther away = smaller
        let mut far = evaluate_scene(&doc, 0)[0].clone();
        let t = deform::Transform3D { z: 500.0, ..Default::default() };
        far.path = deform::project_path(&far.path, &far.matrix, &t, pt(275.0, 200.0), &doc.perspective());
        let (lo, hi) = far.path.bounds().unwrap();
        assert!(hi.x - lo.x < 70.0);
    }

    #[test]
    fn bones_fk_and_ik() {
        let mut ed = Editor::new(Document::default());
        let seg = |x: f32| Element::shape("seg", Shape::new(Path::rect(x, 0.0, 100.0, 20.0), Some(Color::rgb(0, 0, 0)), None));
        ed.add_element(seg(0.0));
        ed.add_element(seg(100.0));
        ed.add_element(seg(200.0));
        ed.add_bone(0, 0, 1, pt(5.0, 10.0), pt(100.0, 10.0)).unwrap();
        ed.add_bone(0, 1, 2, pt(100.0, 10.0), pt(200.0, 10.0)).unwrap();
        assert!(ed.add_bone(0, 2, 0, pt(250.0, 10.0), pt(5.0, 10.0)).is_err(), "no loops");
        assert!(ed.add_bone(0, 0, 2, pt(5.0, 10.0), pt(200.0, 10.0)).is_err(), "one parent per object");
        // pull the tip up: the chain bends, the root stays put, joints stay connected
        ed.begin_edit("Pose");
        let tip = ed.element_at(Selection { layer: 0, element: 2 }).unwrap();
        let grab = tip.matrix.invert().unwrap().apply(pt(295.0, 10.0));
        ed.ik_drag(0, 2, grab, pt(200.0, 150.0));
        let els = ed.timeline().layers[0].keyframes[&0].elements.clone();
        let reach = els[2].matrix.apply(grab);
        assert!(reach.dist(pt(200.0, 150.0)) < 2.0, "tip reached target: {reach:?}");
        assert!(els[0].matrix.apply(pt(5.0, 10.0)).dist(pt(5.0, 10.0)) < 0.5, "root joint fixed");
        let b = ed.bones(0)[1];
        assert!(els[1].matrix.apply(b.tail_parent).dist(els[2].matrix.apply(b.tail_child)) < 0.01, "joint connected");
        // deleting a bonded object drops its bones and re-indexes the rest
        ed.selection = vec![Selection { layer: 0, element: 0 }];
        ed.delete_selected();
        assert!(ed.bones(0).iter().all(|b| b.parent < 2 && b.child < 2));
        assert_eq!(ed.bones(0).len(), 1);
    }

    #[test]
    fn camera_layer_pans_and_zooms() {
        let mut ed = Editor::new(Document::default());
        ed.add_element(Element::shape("dot", Shape::new(Path::rect(265.0, 190.0, 20.0, 20.0), Some(Color::rgb(0, 0, 0)), None)));
        ed.add_camera();
        assert_eq!(ed.doc.scene.layers[0].kind, LayerKind::Camera);
        let b = |doc: &Document| { let it = &evaluate_scene(doc, 0)[0]; it.path.transformed(&it.matrix).bounds().unwrap() };
        let (lo, hi) = b(&ed.doc);
        assert!(close(hi.x - lo.x, 20.0), "default camera = identity");
        commands::execute(&mut ed, "camera.set", &serde_json::json!({ "zoom": 2.0, "x": 275, "y": 200 })).unwrap();
        let (lo, hi) = b(&ed.doc);
        assert!(close(hi.x - lo.x, 40.0) && close((lo.x + hi.x) / 2.0, 275.0), "zoomed around the centre");
        assert!(!ed.layer_editable(0), "nothing can be drawn on the camera layer");
    }

    #[test]
    fn gradients_render_and_tween() {
        let mut ed = Editor::new(Document::default());
        ed.add_element(Element::shape("g", Shape::new(Path::rect(0.0, 0.0, 100.0, 100.0), Some(Color::rgb(0, 0, 0)), None)));
        ed.select_all();
        commands::execute(&mut ed, "selection.set_gradient", &serde_json::json!({ "kind": "radial", "colors": ["#FFFFFF", "#0000FF"] })).unwrap();
        let it = &ed.evaluate_current(0)[0];
        let g = it.gradient.as_ref().unwrap();
        assert_eq!(g.kind, GradientKind::Radial);
        assert_eq!(g.stops.len(), 2);
        let mid = g.lerp(&Gradient { stops: vec![(0.0, Color::rgb(0, 0, 0)), (1.0, Color::rgb(0, 0, 0))], ..g.clone() }, 0.5);
        assert_eq!(mid.stops[0].1, Color::rgb(128, 128, 128));
    }

    #[test]
    fn json_roundtrip() {
        let doc = demo::demo_document();
        let back = Document::from_json(&doc.to_json()).unwrap();
        assert_eq!(doc, back);
    }

    #[test]
    fn custom_ease_on_classic_tween() {
        let mut ed = Editor::new(Document::default());
        ed.add_element(Element::shape("box", Shape::new(Path::rect(0., 0., 10., 10.), Some(Color::rgb(0, 0, 0)), None)));
        ed.set_frame(10);
        ed.insert_keyframe();
        ed.selection = vec![Selection { layer: 0, element: 0 }];
        ed.modify_selected("Move", |e| e.matrix = Matrix::translate(100.0, 0.0));
        ed.set_frame(0);
        ed.set_tween(Tween::Classic { ease: 0.0, rotate_turns: 0 });
        ed.frame_sel = None;
        ed.set_ease_curve(Some(motion::Ease::preset(motion::EaseFamily::Cubic, motion::EaseDir::In)));
        let x = ed.evaluate_current(5)[0].matrix.tx;
        assert!(close(x, 100.0 * 0.125), "cubic in at t=.5: {x}");
        // survives save/load
        let json = serde_json::to_string(&ed.doc).unwrap();
        let back: Document = serde_json::from_str(&json).unwrap();
        assert_eq!(back.scene.layers[0].keyframes[&0].ease_curve, ed.doc.scene.layers[0].keyframes[&0].ease_curve);
    }

    #[test]
    fn motion_tween_workflow() {
        let mut ed = Editor::new(Document { fps: 12.0, ..Document::default() });
        ed.add_element(Element::shape("box", Shape::new(Path::rect(0., 0., 20., 20.), Some(Color::rgb(0, 0, 0)), None)));
        ed.set_tween(Tween::Motion { ease: 0.0 });
        let l = &ed.timeline().layers[0];
        assert_eq!(l.end, 12, "one-frame span grows to one second");
        assert!(l.keyframes[&0].motion.is_some());
        assert_eq!(l.keyframes.len(), 1, "no closing keyframe");
        // move at frame 11 -> property keyframes, no new timeline keyframe
        ed.set_frame(11);
        ed.selection = vec![Selection { layer: 0, element: 0 }];
        ed.modify_selected("Move", |e| e.matrix.tx += 110.0);
        assert_eq!(ed.timeline().layers[0].keyframes.len(), 1);
        let m = ed.timeline().layers[0].keyframes[&0].motion.clone().unwrap();
        assert_eq!(m.path.len(), 2);
        let center = |ed: &Editor, f| {
            let it = &ed.evaluate_current(f)[0];
            it.matrix.apply(pt(10.0, 10.0))
        };
        assert!(close(center(&ed, 0).x, 10.0));
        assert!(close(center(&ed, 11).x, 120.0));
        assert!(close(center(&ed, 5).x, 10.0 + 110.0 * 5.0 / 11.0));
        // rotate at frame 6 -> rotation keys at 0 and 6 only
        ed.set_frame(6);
        ed.selection = vec![Selection { layer: 0, element: 0 }];
        ed.modify_selected("Rotate", |e| {
            let c = e.matrix.apply(pt(10.0, 10.0));
            e.matrix = Matrix::translate(c.x, c.y).concat(&Matrix::rotate(std::f32::consts::FRAC_PI_2)).concat(&Matrix::translate(-c.x, -c.y)).concat(&e.matrix);
        });
        let m = ed.timeline().layers[0].keyframes[&0].motion.clone().unwrap();
        assert_eq!(m.rotation.keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 6]);
        assert!(close(m.rotation.keys[1].value, 90.0));
        assert_eq!(m.path.len(), 2, "rotating about the pivot adds no position key");
        // F6 keys everything; Shift+F6 clears them
        ed.set_frame(3);
        ed.insert_keyframe();
        assert!(ed.timeline().layers[0].keyframes[&0].motion.as_ref().unwrap().scale_x.has_key(3));
        ed.clear_keyframe();
        assert!(!ed.timeline().layers[0].keyframes[&0].motion.as_ref().unwrap().scale_x.has_key(3));
        // splitting the span (F7 elsewhere / pinning) keeps what each frame shows
        let before: Vec<Pt> = (0..12).map(|f| center(&ed, f)).collect();
        frames::convert_to_keyframes(&mut ed.timeline_mut().layers[0], 8, 8, false);
        let after: Vec<Pt> = (0..12).map(|f| center(&ed, f)).collect();
        for (a, b) in before.iter().zip(&after) {
            assert!(a.dist(*b) < 0.05, "{a:?} vs {b:?}");
        }
        // round trip
        let json = serde_json::to_string(&ed.doc).unwrap();
        let back: Document = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ed.doc);
    }

    #[test]
    fn reverse_frames_plays_backwards() {
        let mut ed = Editor::new(Document::default());
        ed.add_element(Element::shape("box", Shape::new(Path::rect(0., 0., 10., 10.), Some(Color::rgb(0, 0, 0)), None)));
        ed.set_frame(9);
        ed.insert_keyframe();
        ed.selection = vec![Selection { layer: 0, element: 0 }];
        ed.modify_selected("Move", |e| e.matrix = Matrix::translate(90.0, 0.0));
        ed.set_frame(0);
        ed.set_tween(Tween::Classic { ease: 50.0, rotate_turns: 0 });
        ed.set_frame(14);
        ed.insert_frame(); // a static tail after the tween: frames 10-14 hold x=90
        let x = |ed: &Editor, f| ed.evaluate_current(f)[0].matrix.tx;
        let before: Vec<f32> = (0..15).map(|f| x(&ed, f)).collect();
        ed.select_frames(FrameSel { layer0: 0, layer1: 0, f0: 0, f1: 14 });
        ed.reverse_frames();
        for f in 0..=14 {
            assert!(close(x(&ed, f), before[14 - f as usize]), "frame {f}: {} vs {}", x(&ed, f), before[14 - f as usize]);
        }
        ed.reverse_frames();
        for f in 0..=14 {
            assert!(close(x(&ed, f), before[f as usize]), "twice = original at {f}");
        }
    }

    #[test]
    fn frame_by_frame_keeps_the_animation() {
        let mut ed = Editor::new(Document::default());
        ed.add_element(Element::shape("box", Shape::new(Path::rect(0., 0., 10., 10.), Some(Color::rgb(0, 0, 0)), None)));
        ed.set_frame(8);
        ed.insert_keyframe();
        ed.selection = vec![Selection { layer: 0, element: 0 }];
        ed.modify_selected("Move", |e| e.matrix = Matrix::translate(80.0, 40.0));
        ed.set_frame(0);
        ed.set_tween(Tween::Classic { ease: -100.0, rotate_turns: 0 });
        let before: Vec<Matrix> = (0..9).map(|f| ed.evaluate_current(f)[0].matrix).collect();
        ed.select_frames(FrameSel { layer0: 0, layer1: 0, f0: 0, f1: 8 });
        ed.convert_to_frame_by_frame();
        assert_eq!(ed.timeline().layers[0].keyframes.len(), 9);
        assert!(ed.timeline().layers[0].keyframes.values().all(|k| matches!(k.tween, Tween::None)));
        for f in 0..9 {
            assert_eq!(ed.evaluate_current(f)[0].matrix, before[f as usize]);
        }
    }

    #[test]
    fn shape_hints_steer_the_morph() {
        // square -> square rotated by one corner: without hints the corners
        // pair up nearest-first; with hints a (top-left) goes to top-right.
        let sq = Path::rect(0., 0., 100., 100.);
        let mut doc = Document::default();
        let mut l = Layer::new("L", 0);
        l.end = 11;
        let mut a = Keyframe::with(vec![Element::shape("s", Shape::new(sq.clone(), Some(Color::rgb(0, 0, 0)), None))]);
        a.tween = Tween::Shape { ease: 0.0 };
        a.shape_hints = vec![pt(0.0, 0.0)];
        let mut b = Keyframe::with(vec![Element::shape("s", Shape::new(sq.clone(), Some(Color::rgb(0, 0, 0)), None))]);
        b.shape_hints = vec![pt(100.0, 0.0)];
        l.keyframes.insert(0, a);
        l.keyframes.insert(10, b);
        doc.scene.layers = vec![l];
        let (els, _) = layer_elements_at(&doc.scene.layers[0], 5).unwrap();
        let ElementKind::Shape(s) = &els[0].kind else { panic!() };
        // the hinted corner is halfway along the top edge at t = 0.5
        let anchors: Vec<Pt> = s.path.anchors().into_iter().map(|(_, p)| p).collect();
        assert!(anchors.iter().any(|p| p.dist(pt(50.0, 0.0)) < 0.5), "{anchors:?}");
        // and the hint pair maps onto itself at the ends
        let (e0, _) = layer_elements_at(&doc.scene.layers[0], 0).unwrap();
        let ElementKind::Shape(s0) = &e0[0].kind else { panic!() };
        assert_eq!(s0.path, sq);
    }

    #[test]
    fn reverse_motion_tween() {
        let mut ed = Editor::new(Document { fps: 10.0, ..Document::default() });
        ed.add_element(Element::shape("box", Shape::new(Path::rect(0., 0., 20., 20.), Some(Color::rgb(0, 0, 0)), None)));
        ed.set_tween(Tween::Motion { ease: 0.0 });
        ed.set_frame(9);
        ed.selection = vec![Selection { layer: 0, element: 0 }];
        ed.modify_selected("Move", |e| e.matrix.tx += 90.0);
        ed.set_frame(5);
        ed.selection = vec![Selection { layer: 0, element: 0 }];
        ed.modify_selected("Fade", |e| e.alpha = 0.2);
        let snap = |ed: &Editor| (0..10).map(|f| { let it = &ed.evaluate_current(f)[0]; (it.matrix.tx, it.alpha) }).collect::<Vec<_>>();
        let before = snap(&ed);
        ed.select_frames(FrameSel { layer0: 0, layer1: 0, f0: 0, f1: 9 });
        ed.reverse_frames();
        let after = snap(&ed);
        for f in 0..10 {
            let (a, b) = (after[f], before[9 - f]);
            assert!(close(a.0, b.0) && close(a.1, b.1), "frame {f}: {a:?} vs {b:?}");
        }
        // span end drag stretches the keys
        ed.set_layer_end(0, 20, Some("Span"));
        let it = &ed.evaluate_current(19)[0];
        assert!(close(it.matrix.tx, before[0].0), "stretched end shows the reversed end pose");
    }

    #[test]
    fn edit_multiple_frames_moves_every_keyframe() {
        let mut ed = Editor::new(Document::default());
        ed.add_element(Element::shape("box", Shape::new(Path::rect(0., 0., 10., 10.), Some(Color::rgb(0, 0, 0)), None)));
        for f in [4, 8] {
            ed.set_frame(f);
            ed.insert_keyframe();
            ed.selection = vec![Selection { layer: 0, element: 0 }];
            ed.modify_selected("Move", |e| e.matrix.tx += 20.0);
        }
        ed.set_frame(0);
        ed.multi_frame = Some((0, 8));
        ed.selection = vec![Selection { layer: 0, element: 0 }];
        ed.modify_selected("Move", |e| e.matrix.ty += 50.0);
        let l = &ed.timeline().layers[0];
        assert_eq!(l.keyframes.len(), 3, "no keyframes added");
        assert!(l.keyframes.values().all(|k| k.elements[0].matrix.ty == 50.0));
        assert_eq!(l.keyframes[&8].elements[0].matrix.tx, 40.0, "x offsets kept");
    }

    #[test]
    fn buttons_colour_effects_and_swap() {
        let mut doc = Document::default();
        let mut tl = Timeline::new("btn");
        let rect = |c: Color, w: f32| Element::shape("r", Shape::new(Path::rect(0., 0., w, 20.), Some(c), None));
        tl.layers[0].end = 4;
        for (f, c, w) in [(0, Color::rgb(255, 0, 0), 20.0), (1, Color::rgb(0, 255, 0), 20.0), (2, Color::rgb(0, 0, 255), 20.0), (3, Color::rgb(0, 0, 0), 60.0)] {
            tl.layers[0].keyframes.insert(f, Keyframe::with(vec![rect(c, w)]));
        }
        let btn = doc.add_symbol("Button", SymbolKind::Button, tl);
        let other = doc.add_symbol("Other", SymbolKind::Graphic, Timeline::new("o"));
        let mut inst = Element::instance("b", btn, Matrix::translate(100.0, 100.0));
        inst.color_fx = Some(ColorXform { mul: [0.5, 1.0, 1.0, 1.0], add: [0.0, 0.0, 10.0, 0.0] });
        doc.scene.layers[0].keyframes.insert(0, Keyframe::with(vec![inst]));
        doc.scene.layers[0].end = 1;
        let mut ed = Editor::new(doc);
        let fill = |ed: &Editor| ed.evaluate_current(0)[0].fill.unwrap();
        assert_eq!(fill(&ed), Color::rgb(255, 0, 0), "Up");
        ed.button_state = Some(((0, 0), 1));
        assert_eq!(fill(&ed), Color::rgb(0, 255, 0), "Over");
        ed.button_state = Some(((0, 0), 2));
        assert_eq!(fill(&ed), Color::rgb(0, 0, 255), "Down");
        ed.button_state = Some(((0, 0), 3));
        assert_eq!(fill(&ed), Color::rgb(0, 0, 255), "the Hit frame never shows");
        // the hit area (60 px wide) is bigger than the visible button (20 px)
        assert_eq!(ed.button_at(pt(150.0, 110.0)), Some((0, 0)));
        assert_eq!(ed.button_at(pt(170.0, 110.0)), None);
        // advanced colour: red × 50%, blue + 10, composed onto the item
        let it = &ed.evaluate_current(0)[0];
        assert_eq!(it.cx.unwrap().apply(Color::rgb(200, 0, 0)), Color::rgb(100, 0, 10));
        // swap keeps transform and effect
        ed.selection = vec![Selection { layer: 0, element: 0 }];
        ed.swap_symbol(other);
        let el = ed.selected_element().unwrap();
        assert!(matches!(el.kind, ElementKind::Instance(Instance { symbol, .. }) if symbol == other));
        assert_eq!(el.matrix, Matrix::translate(100.0, 100.0));
        assert!(el.color_fx.is_some());
    }

    #[test]
    fn scenes_play_in_order() {
        let mut ed = Editor::new(Document::default());
        ed.add_element(Element::shape("a", Shape::new(Path::rect(0., 0., 10., 10.), Some(Color::rgb(255, 0, 0)), None)));
        ed.set_frame(4);
        ed.insert_frame();
        assert_eq!(ed.add_scene(), 1);
        assert_eq!(ed.timeline().name, "Scene 2");
        ed.add_element(Element::shape("b", Shape::new(Path::rect(0., 0., 10., 10.), Some(Color::rgb(0, 0, 255)), None)));
        ed.set_frame(2);
        ed.insert_frame();
        assert_eq!(ed.doc.total_frames(), 8);
        assert_eq!(ed.doc.locate_frame(6), (1, 1));
        let fill = |doc: &Document, f| evaluate_scene(doc, f)[0].fill.unwrap();
        assert_eq!(fill(&ed.doc, 4), Color::rgb(255, 0, 0));
        assert_eq!(fill(&ed.doc, 5), Color::rgb(0, 0, 255));
        // reorder: scene 2 first
        ed.move_scene(1, 0);
        assert_eq!(fill(&ed.doc, 0), Color::rgb(0, 0, 255));
        assert_eq!(ed.scene, 0, "current scene follows the move");
        ed.delete_scene(0);
        assert_eq!(ed.doc.scene_count(), 1);
        assert_eq!(fill(&ed.doc, 0), Color::rgb(255, 0, 0));
        ed.undo();
        assert_eq!(ed.doc.scene_count(), 2);
        let json = serde_json::to_string(&ed.doc).unwrap();
        assert_eq!(serde_json::from_str::<Document>(&json).unwrap(), ed.doc);
    }

    #[test]
    fn layer_parenting_follows_and_keeps_place() {
        let mut ed = Editor::new(Document::default());
        // layer 0 (top): child at x=100; layer 1: parent at x=0, moves to x=50 by frame 10
        ed.add_element(Element::shape("child", Shape::new(Path::rect(100., 0., 10., 10.), Some(Color::rgb(0, 0, 0)), None)));
        ed.set_frame(10);
        ed.insert_frame();
        ed.set_frame(0);
        ed.add_layer();
        ed.layer = 0;
        ed.timeline_mut().layers.swap(0, 1);
        let parent_layer = 1;
        ed.layer = parent_layer;
        ed.add_element(Element::shape("parent", Shape::new(Path::rect(0., 0., 10., 10.), Some(Color::rgb(0, 0, 0)), None)));
        ed.set_frame(10);
        ed.insert_keyframe();
        ed.selection = vec![Selection { layer: parent_layer, element: 0 }];
        ed.modify_selected("Move", |e| e.matrix.tx += 50.0);
        ed.set_frame(0);
        ed.set_tween(Tween::Classic { ease: 0.0, rotate_turns: 0 });
        let child_x = |ed: &Editor, f| ed.evaluate_current(f).iter().find(|i| i.src == Some((0, 0))).map(|i| i.matrix.apply(pt(100.0, 0.0)).x).unwrap();
        assert!(close(child_x(&ed, 10), 100.0));
        ed.set_anim_parent(0, Some(parent_layer));
        assert!(close(child_x(&ed, 0), 100.0), "nothing jumps when parenting");
        assert!(close(child_x(&ed, 10), 150.0), "the child follows its parent");
        assert!(close(child_x(&ed, 5), 125.0));
        // moving the child on the stage moves it by exactly that much
        ed.set_frame(10);
        ed.selection = vec![Selection { layer: 0, element: 0 }];
        ed.modify_selected("Move", |e| e.matrix.tx += 7.0);
        assert!(close(child_x(&ed, 10), 157.0));
        assert!(ed.parent_candidates(parent_layer).is_empty(), "no cycles");
        let at0 = child_x(&ed, 0);
        ed.set_anim_parent(0, None);
        assert!(close(child_x(&ed, 0), at0), "unparenting keeps the place on keyframes");
    }

    #[test]
    fn merge_drawing_workflow() {
        use crate::merge::Part;
        let mut ed = Editor::new(Document::default());
        let red = Color::rgb(255, 0, 0);
        let blue = Color::rgb(0, 0, 255);
        assert!(ed.merge_draw(&Path::rect(0.0, 0.0, 100.0, 100.0), Some(red), None, None));
        assert!(ed.merge_draw(&Path::rect(50.0, 50.0, 100.0, 100.0), Some(blue), None, None));
        let c = ed.canvas(0).unwrap();
        assert_eq!((c.fills.len(), ed.timeline().layers[0].keyframes[&0].elements.len()), (2, 1), "one merge shape, two regions");
        // click the blue square and drag it away: the red square keeps its notch
        let (li, part, _) = ed.merge_part_at(pt(120.0, 120.0), 1.0).unwrap();
        assert!(matches!(part, Some(Part::Fill(_))));
        ed.merge_select(li, part.unwrap(), false, false);
        assert_eq!(ed.selection.len(), 1);
        let b = ed.selection_bounds().unwrap();
        assert!(close(b.0.x, 50.0) && close(b.1.x, 150.0), "bounds are the selected piece: {b:?}");
        ed.modify_selected("Move", |e| e.matrix.tx += 200.0);
        ed.deselect();
        let c = ed.canvas(0).unwrap();
        assert_eq!(c.fills.len(), 2);
        let red_bounds = c.fills.iter().find(|f| f.fill == Some(red)).unwrap().bounds().unwrap();
        assert!(close(red_bounds.1.x, 100.0));
        // drop red onto blue: blue gets cut
        let (li, part, _) = ed.merge_part_at(pt(10.0, 10.0), 1.0).unwrap();
        ed.merge_select(li, part.unwrap(), false, false);
        ed.modify_selected("Move", |e| e.matrix.tx += 220.0);
        ed.deselect();
        let c = ed.canvas(0).unwrap();
        assert!(c.fills.iter().any(|f| f.fill == Some(blue)));
        // undo brings the drag back
        ed.undo();
        assert!(ed.canvas(0).unwrap().float.is_some() || ed.canvas(0).unwrap().fills.iter().any(|f| f.fill == Some(red)));
        // marquee half of the shape and delete it
        let mut ed2 = Editor::new(Document::default());
        ed2.merge_draw(&Path::rect(0.0, 0.0, 100.0, 100.0), Some(red), None, None);
        assert!(ed2.merge_select_area(&[vec![pt(50.0, -10.0), pt(120.0, -10.0), pt(120.0, 120.0), pt(50.0, 120.0)]], false));
        assert_eq!(ed2.delete_selected(), 1);
        let b = ed2.canvas(0).unwrap().bounds().unwrap();
        assert!(close(b.1.x, 50.0), "{b:?}");
        // the document round-trips
        let json = serde_json::to_string(&ed2.doc).unwrap();
        assert_eq!(serde_json::from_str::<Document>(&json).unwrap(), ed2.doc);
    }

    #[test]
    fn combine_objects() {
        use crate::merge::BoolOp;
        let mk = |x: f32, c: Color| Element::shape("s", Shape::new(Path::rect(x, 0.0, 100.0, 100.0), Some(c), None));
        let area = |ed: &Editor, i: usize| -> f32 {
            let el = ed.element_at(Selection { layer: 0, element: i }).unwrap();
            let ElementKind::Shape(sh) = el.kind else { panic!() };
            let (lo, hi) = sh.path.bounds().unwrap();
            (hi.x - lo.x) * (hi.y - lo.y)
        };
        for (op, n, w) in [(BoolOp::Intersect, 1, 50.0 * 100.0), (BoolOp::Punch, 1, 50.0 * 100.0), (BoolOp::Crop, 1, 50.0 * 100.0), (BoolOp::Union, 1, 150.0 * 100.0)] {
            let mut ed = Editor::new(Document::default());
            ed.add_element(mk(0.0, Color::rgb(255, 0, 0)));
            ed.add_element(mk(50.0, Color::rgb(255, 0, 0)));
            ed.selection = vec![Selection { layer: 0, element: 0 }, Selection { layer: 0, element: 1 }];
            ed.combine_objects(op).unwrap();
            assert_eq!(ed.selection.len(), n, "{op:?}");
            assert!(close(area(&ed, 0), w), "{op:?}: {}", area(&ed, 0));
        }
        // Break Apart: a drawing object becomes merge shape
        let mut ed = Editor::new(Document::default());
        ed.add_element(mk(0.0, Color::rgb(255, 0, 0)));
        ed.selection = vec![Selection { layer: 0, element: 0 }];
        ed.break_apart();
        assert_eq!(ed.canvas(0).unwrap().fills.len(), 1);
    }

    #[test]
    fn primitives_and_stroke_styles() {
        let r = Primitive::Rect { x: 0.0, y: 0.0, w: 100.0, h: 50.0, radius: [10.0, 0.0, -10.0, 0.0] };
        let (lo, hi) = r.path().bounds().unwrap();
        assert!(close(lo.x, 0.0) && close(hi.x, 100.0) && close(hi.y, 50.0));
        // the inverted corner cuts in: the bottom-right corner point is not on the path
        assert!(!r.path().hit(pt(99.5, 49.5), true, 0.0));
        let o = Primitive::Oval { cx: 0.0, cy: 0.0, rx: 50.0, ry: 50.0, start: 0.0, end: 180.0, inner: 50.0, close: true };
        assert!(o.path().hit(pt(0.0, 37.0), true, 0.0), "inside the half ring");
        assert!(!o.path().hit(pt(0.0, -37.0), true, 0.0), "other half is open");
        assert!(!o.path().hit(pt(0.0, 10.0), true, 0.0), "the hole");
        // a default stroke style has usable dash lengths (regression: derived Default gave 0)
        let st = StrokeStyle::default();
        assert!(st.is_default() && st.dash_len > 0.0 && st.gap > 0.0);
        let json = serde_json::to_string(&Shape { primitive: Some(r), ..Shape::new(r.path(), None, None) }).unwrap();
        let back: Shape = serde_json::from_str(&json).unwrap();
        assert_eq!(back.primitive, Some(r));
    }
}
