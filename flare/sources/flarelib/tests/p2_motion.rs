//! P2 group 2 (Oct 15 2026): Copy/Paste Motion, Copy Motion as JS, motion presets.

use ac_core::commands::execute;
use ac_core::motion_clip::ClipSource;
use ac_core::motion_presets::{self, MotionPreset};
use ac_core::{layer_elements_at, Editor, FrameSel, Matrix, SymbolKind, Tween};
use serde_json::json;

/// A document with two layers, each holding one instance: `top` at (x0, 0), `bottom` at (x1, 100).
fn two_instances(x0: f32, x1: f32) -> Editor {
    let mut ed = Editor::new(Default::default());
    ed.layer = 0;
    execute(&mut ed, "draw.rect", &json!({"x": 0, "y": 0, "w": 20, "h": 20})).unwrap();
    execute(&mut ed, "select.all", &json!({})).unwrap();
    ed.convert_to_symbol("A", SymbolKind::MovieClip).unwrap();
    ed.modify_selected("place", |e| e.matrix = Matrix::translate(x0, 0.0));
    ed.add_layer();
    ed.layer = 0; // the new layer is on top; the first instance moves down to index 1
    execute(&mut ed, "draw.rect", &json!({"x": 0, "y": 0, "w": 30, "h": 30})).unwrap();
    execute(&mut ed, "select.all", &json!({})).unwrap();
    ed.convert_to_symbol("B", SymbolKind::MovieClip).unwrap();
    ed.modify_selected("place", |e| e.matrix = Matrix::translate(x1, 100.0));
    ed.selection.clear();
    ed
}

fn matrix_at(ed: &Editor, layer: usize, f: u32) -> Matrix {
    layer_elements_at(&ed.timeline().layers[layer], f).unwrap().0[0].matrix
}

fn alpha_at(ed: &Editor, layer: usize, f: u32) -> f32 {
    layer_elements_at(&ed.timeline().layers[layer], f).unwrap().0[0].alpha
}

fn close(a: &Matrix, b: &Matrix, tol: f32) -> bool {
    [(a.a, b.a), (a.b, b.b), (a.c, b.c), (a.d, b.d), (a.tx, b.tx), (a.ty, b.ty)].iter().all(|(x, y)| (x - y).abs() <= tol)
}

/// Layer `li` gets a classic tween from frame 0 to `end`: moves by (dx, dy), turns `deg`, fades to `alpha`.
fn classic(ed: &mut Editor, li: usize, end: u32, dx: f32, dy: f32, deg: f32, alpha: f32, ease: f32) {
    ed.select_frames(FrameSel::cell(li, end));
    ed.insert_keyframe();
    ed.layer = li;
    ed.frame = end;
    ed.selection = vec![ac_core::Selection { layer: li, element: 0 }];
    ed.modify_selected("move", |e| {
        let c = e.matrix.apply(ac_core::pt(10.0, 10.0));
        e.matrix = Matrix::translate(c.x + dx, c.y + dy).concat(&Matrix::rotate(deg.to_radians())).concat(&Matrix::translate(-10.0, -10.0));
        e.alpha = alpha;
    });
    ed.select_frames(FrameSel { layer0: li, layer1: li, f0: 0, f1: end });
    ed.set_tween(Tween::Classic { ease, rotate_turns: 0 });
}

#[test]
fn copy_classic_tween_keeps_every_frame() {
    let mut ed = two_instances(0.0, 200.0);
    classic(&mut ed, 1, 12, 120.0, 40.0, 0.0, 1.0, 0.0);
    ed.select_frames(FrameSel::cell(1, 3));
    assert_eq!(ed.copy_motion(), Ok(13), "frames 0..=12, including the end keyframe");
    let clip = ed.motion_clip.clone().unwrap();
    assert_eq!(clip.source, ClipSource::Classic);
    assert_eq!(clip.frames.len(), 13);
    for (f, x) in clip.frames.iter().enumerate() {
        assert!(close(&x.matrix, &matrix_at(&ed, 1, f as u32), 1e-4), "frame {f}");
    }
    assert!(clip.motion.is_none());
    assert!(ed.can_paste_motion());
}

#[test]
fn paste_classic_motion_onto_another_instance_is_relative_and_one_undo_step() {
    let mut ed = two_instances(0.0, 200.0);
    classic(&mut ed, 1, 12, 120.0, 40.0, 90.0, 0.5, 0.0);
    ed.select_frames(FrameSel::cell(1, 0));
    ed.copy_motion().unwrap();
    // paste onto the top layer's instance (starts at x = 200, y = 100)
    ed.select_frames(FrameSel::cell(0, 0));
    ed.selection = vec![ac_core::Selection { layer: 0, element: 0 }];
    let steps = ed.history().0.len();
    let before = matrix_at(&ed, 0, 0);
    ed.paste_motion(true).unwrap();
    assert_eq!(ed.history().0.len(), steps + 1, "one undo step");
    let kf = &ed.timeline().layers[0].keyframes[&0];
    assert!(matches!(kf.tween, Tween::Motion { .. }) && kf.motion.is_some(), "a new motion tween");
    assert!(close(&matrix_at(&ed, 0, 0), &before, 1e-3), "starts where the target is");
    // every frame: the target made the same change in transform as the source
    let inv0 = matrix_at(&ed, 1, 0).invert().unwrap();
    for f in 0..=12 {
        let want = matrix_at(&ed, 1, f).concat(&inv0).concat(&before);
        assert!(close(&matrix_at(&ed, 0, f), &want, 0.05), "frame {f}: {:?} vs {:?}", matrix_at(&ed, 0, f), want);
        assert!((alpha_at(&ed, 0, f) - alpha_at(&ed, 1, f)).abs() < 0.01, "alpha at {f}");
    }
    ed.undo();
    assert_eq!(ed.timeline().layers[0].keyframes[&0].tween, Tween::None);
}

#[test]
fn fitted_motion_uses_few_keys_for_straight_movement() {
    let mut ed = two_instances(0.0, 200.0);
    classic(&mut ed, 1, 24, 240.0, 0.0, 0.0, 1.0, 0.0);
    ed.select_frames(FrameSel::cell(1, 0));
    ed.copy_motion().unwrap();
    ed.select_frames(FrameSel::cell(0, 0));
    ed.paste_motion(true).unwrap();
    let m = ed.timeline().layers[0].keyframes[&0].motion.clone().unwrap();
    assert_eq!(m.path.len(), 2, "a straight line needs two anchors");
    assert!(m.rotation.keys.is_empty() && m.alpha.keys.is_empty());
}

#[test]
fn paste_absolute_takes_the_copied_transforms() {
    let mut ed = two_instances(0.0, 200.0);
    classic(&mut ed, 1, 8, 80.0, 0.0, 0.0, 1.0, 0.0);
    ed.select_frames(FrameSel::cell(1, 0));
    ed.copy_motion().unwrap();
    ed.select_frames(FrameSel::cell(0, 0));
    ed.paste_motion(false).unwrap();
    for f in 0..=8 {
        assert!(close(&matrix_at(&ed, 0, f), &matrix_at(&ed, 1, f), 0.05), "frame {f}");
    }
}

#[test]
fn copy_motion_tween_keeps_keyframe_motion_and_ease() {
    let mut ed = two_instances(0.0, 200.0);
    ed.select_frames(FrameSel::cell(1, 0));
    ed.set_tween(Tween::Motion { ease: 0.0 });
    execute(&mut ed, "motion.set_key", &json!({"layer": 2, "prop": "x", "frame": 24, "value": 500})).unwrap();
    execute(&mut ed, "motion.set_key", &json!({"layer": 2, "prop": "rotation", "frame": 12, "value": 90})).unwrap();
    execute(&mut ed, "timeline.set_ease", &json!({"layer": 2, "frame": 1, "ease": "Bounce Out"})).unwrap();
    ed.select_frames(FrameSel::cell(1, 5));
    let n = ed.copy_motion().unwrap();
    let src = ed.timeline().layers[1].keyframes[&0].clone();
    let clip = ed.motion_clip.clone().unwrap();
    assert_eq!(clip.source, ClipSource::Motion);
    assert_eq!(n as u32, ed.timeline().layers[1].span_end(0));
    assert_eq!(clip.motion.as_ref(), src.motion.as_deref(), "Keyframe.motion is kept as it is");
    assert_eq!(clip.ease_curve, src.ease_curve);
    // paste relative: the path starts at the target
    ed.select_frames(FrameSel::cell(0, 0));
    ed.paste_motion(true).unwrap();
    let kf = &ed.timeline().layers[0].keyframes[&0];
    assert_eq!(kf.ease_curve, src.ease_curve, "the ease comes along");
    let m = kf.motion.as_ref().unwrap();
    assert_eq!(m.rotation.keys, src.motion.as_ref().unwrap().rotation.keys);
    let (target_start, source_start) = (ed.timeline().layers[0].keyframes[&0].elements[0].matrix.apply(m.pivot), src.motion.as_ref().unwrap().path[0].at);
    assert!((m.path[0].at.x - target_start.x).abs() < 1e-3 && (m.path[0].at.y - target_start.y).abs() < 1e-3);
    let moved = m.path.last().unwrap().at.x - m.path[0].at.x;
    let src_moved = src.motion.as_ref().unwrap().path.last().unwrap().at.x - source_start.x;
    assert!((moved - src_moved).abs() < 1e-3, "same distance travelled");
    // the pasted span is as long as the copied one
    assert!(ed.timeline().layers[0].span_end(0) >= clip.len);
}

#[test]
fn copy_and_paste_motion_report_errors() {
    let mut ed = two_instances(0.0, 200.0);
    ed.select_frames(FrameSel::cell(1, 0));
    assert!(ed.copy_motion().unwrap_err().contains("tween"), "no tween here");
    assert_eq!(ed.paste_motion(true).unwrap_err(), "Nothing copied: use Copy Motion on a tween first");
    classic(&mut ed, 1, 6, 60.0, 0.0, 0.0, 1.0, 0.0);
    ed.select_frames(FrameSel::cell(1, 0));
    ed.copy_motion().unwrap();
    // two objects on the target keyframe: refuse instead of silently grouping
    ed.select_frames(FrameSel::cell(0, 0));
    execute(&mut ed, "draw.rect", &json!({"x": 0, "y": 0, "w": 5, "h": 5})).unwrap();
    let steps = ed.history().0.len();
    assert!(ed.paste_motion(true).unwrap_err().contains("one object"));
    assert_eq!(ed.history().0.len(), steps, "a refused paste leaves no undo step");
}

#[test]
fn copy_motion_as_js_replays_to_the_same_tween() {
    let mut ed = two_instances(0.0, 200.0);
    ed.select_frames(FrameSel::cell(1, 0));
    ed.set_tween(Tween::Motion { ease: 0.0 });
    execute(&mut ed, "motion.set_key", &json!({"layer": 2, "prop": "x", "frame": 24, "value": 500})).unwrap();
    execute(&mut ed, "motion.set_key", &json!({"layer": 2, "prop": "y", "frame": 24, "value": 40})).unwrap();
    execute(&mut ed, "motion.set_key", &json!({"layer": 2, "prop": "rotation", "frame": 12, "value": 90})).unwrap();
    execute(&mut ed, "motion.set_key", &json!({"layer": 2, "prop": "alpha", "frame": 24, "value": 0.25})).unwrap();
    execute(&mut ed, "motion.set_interp", &json!({"layer": 2, "prop": "rotation", "key": 1, "type": "ease_in_out", "ease_in": 4, "ease_out": 4})).unwrap();
    execute(&mut ed, "timeline.set_ease", &json!({"layer": 2, "frame": 1, "ease": "Sine In Out"})).unwrap();
    ed.select_frames(FrameSel::cell(1, 3));
    let js = ed.copy_motion_as_js().unwrap();
    assert!(ed.motion_clip.is_none(), "Copy as JS must not fill the Copy Motion clipboard");
    ed.copy_motion().unwrap();
    assert!(js.starts_with("// Flare motion: "), "{js}");
    assert!(js.contains("ac.timeline.set_tween({ kind: \"motion\", layer: 2, frame: 1 });"), "{js}");
    assert!(js.contains("ac.motion.set_key({ prop: \"x\", layer: 2, frame: 24, value: 500 });"), "{js}");
    assert!(js.contains("ac.motion.set_interp("), "{js}");
    assert!(js.contains("ac.timeline.set_ease({ layer: 2, frame: 1, ease: \"Sine In Out\" });"), "{js}");

    // run the very calls the script makes on a fresh document and compare the tweens
    let clip = ed.motion_clip.clone().unwrap();
    let mut fresh = two_instances(0.0, 200.0);
    for (id, args) in clip.to_commands(2, 1) {
        execute(&mut fresh, &id, &args).unwrap_or_else(|e| panic!("{id} {args}: {e}"));
    }
    let (a, b) = (&ed.timeline().layers[1], &fresh.timeline().layers[1]);
    assert_eq!(a.keyframes[&0].ease_curve, b.keyframes[&0].ease_curve);
    for f in 0..clip.len {
        assert!(close(&matrix_at(&ed, 1, f), &matrix_at(&fresh, 1, f), 1e-3), "frame {f}");
        assert!((alpha_at(&ed, 1, f) - alpha_at(&fresh, 1, f)).abs() < 1e-4);
    }
}

#[test]
fn classic_tween_also_copies_as_js() {
    let mut ed = two_instances(0.0, 200.0);
    classic(&mut ed, 1, 10, 100.0, 0.0, 0.0, 1.0, 0.0);
    ed.select_frames(FrameSel::cell(1, 0));
    let js = ed.copy_motion_as_js().unwrap();
    assert!(js.contains("kind: \"motion\""), "a classic tween is written as a motion tween");
    ed.copy_motion().unwrap();
    let clip = ed.motion_clip.clone().unwrap();
    let mut fresh = two_instances(0.0, 200.0);
    for (id, args) in clip.to_commands(2, 1) {
        execute(&mut fresh, &id, &args).unwrap();
    }
    for f in 0..=10 {
        assert!(close(&matrix_at(&ed, 1, f), &matrix_at(&fresh, 1, f), 0.05), "frame {f}");
    }
}

// ------------------------------------------------------------------ presets

#[test]
fn builtin_presets_are_our_own_set_stored_as_json() {
    let all = motion_presets::builtin();
    assert!(all.len() >= 20, "{}", all.len());
    let mut names: Vec<_> = all.iter().map(|p| p.name.as_str()).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), all.len(), "unique names");
    for want in ["Bounce Drop", "Pop In", "Fade In", "Pulse", "Shake", "Spin", "Slide Left", "Fly In From Top"] {
        assert!(all.iter().any(|p| p.name == want), "{want}");
    }
    // JSON round trip
    let again = motion_presets::from_json(&motion_presets::to_json(&all)).unwrap();
    assert_eq!(again, all);
    assert!(motion_presets::from_json("{").is_err());
    for p in &all {
        assert!(p.frames >= 2 && !p.tracks.is_empty(), "{}", p.name);
        if p.ease.is_some() {
            assert!(p.ease_for().is_some(), "{}: unknown ease", p.name);
        }
    }
}

#[test]
fn every_builtin_preset_applies_as_one_undo_step() {
    for p in motion_presets::builtin() {
        let mut ed = two_instances(40.0, 200.0);
        ed.select_frames(FrameSel::cell(0, 0));
        ed.selection = vec![ac_core::Selection { layer: 0, element: 0 }];
        let base = matrix_at(&ed, 0, 0);
        let steps = ed.history().0.len();
        ed.apply_motion_preset(&p).unwrap_or_else(|e| panic!("{}: {e}", p.name));
        assert_eq!(ed.history().0.len(), steps + 1, "{}", p.name);
        let l = &ed.timeline().layers[0];
        assert!(matches!(l.keyframes[&0].tween, Tween::Motion { .. }), "{}", p.name);
        assert!(l.span_end(0) > p.last_frame(), "{}: the span grows to the preset", p.name);
        // presets are relative: every pose is finite and the object is where it was at the start
        for f in 0..l.span_end(0) {
            let m = matrix_at(&ed, 0, f);
            assert!([m.a, m.b, m.c, m.d, m.tx, m.ty].iter().all(|v| v.is_finite()), "{} frame {f}", p.name);
        }
        let first = matrix_at(&ed, 0, 0);
        assert!(first.a.is_finite() && (first.tx - base.tx).abs() < 500.0, "{}", p.name);
        ed.undo();
        assert_eq!(ed.timeline().layers[0].keyframes[&0].tween, Tween::None, "{}", p.name);
    }
}

#[test]
fn fly_in_and_fade_presets_land_on_the_object() {
    let all = motion_presets::builtin();
    let get = |n: &str| all.iter().find(|p| p.name == n).unwrap().clone();
    let mut ed = two_instances(40.0, 200.0);
    ed.select_frames(FrameSel::cell(0, 0));
    let here = matrix_at(&ed, 0, 0);
    ed.apply_motion_preset(&get("Fly In From Left")).unwrap();
    let last = ed.timeline().layers[0].span_end(0) - 1;
    assert!((matrix_at(&ed, 0, 0).tx - (here.tx - 420.0)).abs() < 1e-2, "starts off to the left");
    assert!(close(&matrix_at(&ed, 0, last), &here, 1e-2), "ends where the object was");
    ed.undo();
    ed.apply_motion_preset(&get("Fade In")).unwrap();
    let last = ed.timeline().layers[0].span_end(0) - 1;
    assert!(alpha_at(&ed, 0, 0) < 0.01 && (alpha_at(&ed, 0, last) - 1.0).abs() < 1e-3);
    // alpha is a multiplier: a half-transparent object fades to half
    ed.undo();
    ed.selection = vec![ac_core::Selection { layer: 0, element: 0 }];
    ed.modify_selected("half", |e| e.alpha = 0.5);
    ed.apply_motion_preset(&get("Fade In")).unwrap();
    let last = ed.timeline().layers[0].span_end(0) - 1;
    assert!((alpha_at(&ed, 0, last) - 0.5).abs() < 1e-3);
}

#[test]
fn preset_keys_stretch_over_an_existing_span() {
    let all = motion_presets::builtin();
    let spin = all.iter().find(|p| p.name == "Spin").unwrap();
    let mut ed = two_instances(0.0, 200.0);
    ed.select_frames(FrameSel::cell(0, 0));
    ed.set_tween(Tween::Motion { ease: 0.0 });
    let len = ed.timeline().layers[0].span_end(0);
    ed.select_frames(FrameSel::cell(0, 0));
    ed.apply_motion_preset(spin).unwrap();
    assert_eq!(ed.timeline().layers[0].span_end(0), len, "an existing span keeps its length");
    let m = ed.timeline().layers[0].keyframes[&0].motion.clone().unwrap();
    assert_eq!(m.rotation.keys.last().unwrap().frame, len - 1);
    assert!((m.rotation.keys.last().unwrap().value - 360.0).abs() < 1e-3);
}

#[test]
fn save_your_own_preset_from_a_tween_and_reuse_it() {
    let mut ed = two_instances(0.0, 200.0);
    ed.select_frames(FrameSel::cell(1, 0));
    ed.set_tween(Tween::Motion { ease: 0.0 });
    execute(&mut ed, "motion.set_key", &json!({"layer": 2, "prop": "x", "frame": 24, "value": 300})).unwrap();
    execute(&mut ed, "motion.set_key", &json!({"layer": 2, "prop": "scale_x", "frame": 12, "value": 2.0})).unwrap();
    execute(&mut ed, "timeline.set_ease", &json!({"layer": 2, "frame": 1, "ease": "Back Out"})).unwrap();
    ed.select_frames(FrameSel::cell(1, 4));
    assert!(ed.motion_preset_from_tween("x").is_ok());
    let preset = ed.motion_preset_from_tween("Lunge").unwrap();
    assert_eq!(preset.name, "Lunge");
    assert_eq!(preset.ease.as_deref(), Some("Back Out"));
    assert_eq!(preset.tracks["x"].last().unwrap().v, 300.0, "relative to where the pivot started (the symbol centre, 0, 0)");
    // saved and loaded as JSON, then applied to an object somewhere else
    let json = motion_presets::to_json(std::slice::from_ref(&preset));
    let loaded: MotionPreset = motion_presets::from_json(&json).unwrap().remove(0);
    ed.select_frames(FrameSel::cell(0, 0));
    let start = matrix_at(&ed, 0, 0);
    ed.apply_motion_preset(&loaded).unwrap();
    let last = ed.timeline().layers[0].span_end(0) - 1;
    assert!((matrix_at(&ed, 0, last).tx - (start.tx + 300.0)).abs() < 1e-2, "moves by the same offset");
    assert_eq!(ed.timeline().layers[0].keyframes[&0].ease_curve, ed.timeline().layers[1].keyframes[&0].ease_curve);
    // no motion tween under the playhead: nothing to save
    ed.select_frames(FrameSel::cell(1, 0));
    ed.set_tween(Tween::None);
    assert!(ed.motion_preset_from_tween("nope").is_err());
}

#[test]
fn applying_a_preset_without_a_single_object_is_refused() {
    let p = motion_presets::builtin().remove(0);
    let mut ed = Editor::new(Default::default());
    assert!(ed.apply_motion_preset(&p).is_err(), "empty frame");
    let mut ed = two_instances(0.0, 0.0);
    ed.select_frames(FrameSel::cell(0, 0));
    execute(&mut ed, "draw.rect", &json!({"x": 0, "y": 0, "w": 5, "h": 5})).unwrap();
    let steps = ed.history().0.len();
    assert!(ed.apply_motion_preset(&p).unwrap_err().contains("one object"));
    assert_eq!(ed.history().0.len(), steps);
}

#[test]
fn copy_motion_as_js_keeps_existing_clipboard() {
    let mut ed = two_instances(0.0, 200.0);
    // neither a clip nor a tween: error, clipboard untouched
    ed.select_frames(FrameSel::cell(1, 0));
    assert!(ed.copy_motion_as_js().is_err());
    assert!(ed.motion_clip.is_none());
    ed.set_tween(Tween::Motion { ease: 0.0 });
    execute(&mut ed, "motion.set_key", &json!({"layer": 2, "prop": "x", "frame": 24, "value": 500})).unwrap();
    ed.select_frames(FrameSel::cell(1, 3));
    ed.copy_motion().unwrap();
    let before = ed.motion_clip.clone().unwrap();
    // selection moves somewhere without a tween: JS still comes from the clip
    ed.select_frames(FrameSel::cell(0, 0));
    let js = ed.copy_motion_as_js().unwrap();
    assert!(js.contains("ac.motion.set_key("), "{js}");
    let after = ed.motion_clip.clone().unwrap();
    assert_eq!(before.len, after.len);
    assert_eq!(before.frames.len(), after.frames.len());
}

#[test]
fn hostile_preset_json_is_sorted_clamped_and_never_panics() {
    let json = r#"[{"name":"Evil","frames":4000000000,"tracks":{
        "x":[{"f":4000000000,"v":9},{"f":10,"v":5},{"f":0,"v":-5}],
        "alpha":[{"f":7,"v":1},{"f":3,"v":0}]}}]"#;
    let p = motion_presets::from_json(json).unwrap().remove(0);
    assert!(p.frames <= motion_presets::MAX_PRESET_FRAME);
    let xs = &p.tracks["x"];
    assert!(xs.windows(2).all(|w| w[0].f <= w[1].f), "sorted");
    assert!(xs.iter().all(|k| k.f <= motion_presets::MAX_PRESET_FRAME));
    assert!(p.last_frame() <= motion_presets::MAX_PRESET_FRAME);
    let base = ac_core::motion::Pose { pos: ac_core::geom::pt(0.0, 0.0), rotation: 0.0, skew: 0.0, scale_x: 1.0, scale_y: 1.0, alpha: 1.0, brightness: 0.0 };
    let _ = p.motion_for(&base, ac_core::geom::pt(0.0, 0.0), 24);
    // unsorted keys built directly (not through from_json) are handled too
    let raw: MotionPreset = serde_json::from_str::<Vec<MotionPreset>>(json).unwrap().remove(0);
    let _ = raw.motion_for(&base, ac_core::geom::pt(0.0, 0.0), 24);
}
