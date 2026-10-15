//! P2 group 3 (drawing and rigging), core level.
use ac_core::*;

fn stroked(mode: StrokeScale, hair: bool) -> Document {
    let mut d = Document { width: 300.0, height: 200.0, ..Default::default() };
    let mut sh = Shape::new(Path::rect(10.0, 20.0, 30.0, 30.0), None, Some(Stroke { color: Color::rgb(255, 0, 0), width: 10.0 }));
    sh.stroke_style.scale = mode;
    sh.stroke_style.hairline = hair;
    let mut el = Element::shape("", sh);
    el.matrix = Matrix::scale(3.0, 1.0);
    d.scene.layers[0].keyframes.insert(0, Keyframe::with(vec![el]));
    d.scene.layers[0].end = 1;
    d
}

fn ed_with(mode: StrokeScale) -> Editor {
    let mut e = Editor::new(stroked(mode, false));
    e.select(Some(Selection { layer: 0, element: 0 }), false);
    e
}

fn style(e: &Editor) -> StrokeStyle {
    match e.element_at(Selection { layer: 0, element: 0 }).unwrap().kind {
        ElementKind::Shape(s) => s.stroke_style,
        _ => panic!(),
    }
}

#[test]
fn scale_mode_xfl_names_round_trip() {
    for m in StrokeScale::ALL {
        assert_eq!(StrokeScale::from_xfl(m.xfl()), m);
    }
    assert_eq!(StrokeScale::from_xfl("garbage"), StrokeScale::Normal);
}

#[test]
fn pen_matrices_for_each_mode() {
    let m = Matrix::scale(3.0, 2.0);
    assert_eq!(StrokeScale::None.pen(&m), Some(Matrix::IDENTITY));
    assert_eq!(StrokeScale::Horizontal.pen(&m), Some(Matrix::scale(1.0, 2.0)));
    assert_eq!(StrokeScale::Vertical.pen(&m), Some(Matrix::scale(3.0, 1.0)));
    assert_eq!(StrokeScale::Normal.pen(&m), Some(m));
}

#[test]
fn pen_handles_rotation_and_degenerate_matrices() {
    let r = Matrix::rotate(0.7).concat(&Matrix::scale(2.0, 4.0));
    let p = StrokeScale::Horizontal.pen(&r).unwrap();
    assert!((p.d - 4.0).abs() < 1e-4 && p.a == 1.0);
    assert!(StrokeScale::None.pen(&Matrix::scale(0.0, 1.0)).is_none());
    assert!(StrokeScale::Normal.pen(&Matrix { a: f32::NAN, ..Matrix::IDENTITY }).is_none());
}

#[test]
fn defaults_serialize_away_and_old_files_load() {
    let sh = Shape::new(Path::rect(0.0, 0.0, 5.0, 5.0), None, Some(Stroke { color: Color::rgb(0, 0, 0), width: 1.0 }));
    let j = serde_json::to_string(&sh).unwrap();
    assert!(!j.contains("hairline") && !j.contains("scale"));
    // a style written before the new fields existed
    let old: StrokeStyle = serde_json::from_str(r#"{"dash":"Solid","cap":"Round","join":"Round"}"#).unwrap();
    assert_eq!((old.scale, old.hairline, old.pixel_hinting), (StrokeScale::Normal, false, false));
    let mut s2 = sh.clone();
    s2.stroke_style.hairline = true;
    s2.stroke_style.scale = StrokeScale::None;
    let back: Shape = serde_json::from_str(&serde_json::to_string(&s2).unwrap()).unwrap();
    assert!(back.stroke_style.hairline && back.stroke_style.scale == StrokeScale::None);
}

#[test]
fn set_scale_is_one_undo_step() {
    let mut e = ed_with(StrokeScale::Normal);
    let n = e.history().0.len();
    assert!(e.set_stroke_scale(StrokeScale::None));
    assert_eq!(style(&e).scale, StrokeScale::None);
    assert_eq!(e.history().0.len(), n + 1);
    e.undo();
    assert_eq!(style(&e).scale, StrokeScale::Normal);
}

#[test]
fn hairline_and_hinting_commands() {
    let mut e = ed_with(StrokeScale::Normal);
    assert!(e.set_stroke_hairline(true) && style(&e).hairline);
    assert!(e.set_stroke_hinting(true) && style(&e).pixel_hinting);
    e.undo();
    assert!(!style(&e).pixel_hinting && style(&e).hairline);
    e.undo();
    assert!(!style(&e).hairline);
}

#[test]
fn stroke_commands_noop_without_selection_or_stroke() {
    let mut e = Editor::new(stroked(StrokeScale::Normal, false));
    let n = e.history().0.len();
    assert!(!e.set_stroke_scale(StrokeScale::None) && !e.set_stroke_hairline(true) && !e.set_stroke_hinting(true));
    assert_eq!(e.history().0.len(), n);
    // a fill-only shape has nothing to change
    let mut d = Document::default();
    d.scene.layers[0].keyframes.insert(0, Keyframe::with(vec![Element::shape("", Shape::new(Path::rect(0.0, 0.0, 5.0, 5.0), Some(Color::rgb(1, 2, 3)), None))]));
    d.scene.layers[0].end = 1;
    let mut e = Editor::new(d);
    e.select(Some(Selection { layer: 0, element: 0 }), false);
    assert!(!e.set_stroke_scale(StrokeScale::Vertical));
    assert_eq!(e.history().0.len(), 0);
}

// ---------------------------------------------------------------- Auto Keyframe

fn hold_doc(tween: bool) -> Editor {
    let mut d = Document { width: 300.0, height: 200.0, ..Default::default() };
    let sh = Element::shape("", Shape::new(Path::rect(0.0, 0.0, 10.0, 10.0), Some(Color::rgb(1, 2, 3)), None));
    let mut k0 = Keyframe::with(vec![sh.clone()]);
    if tween {
        k0.tween = Tween::Classic { ease: 0.0, rotate_turns: 0 };
        let mut moved = sh;
        moved.matrix = Matrix::translate(100.0, 0.0);
        d.scene.layers[0].keyframes.insert(10, Keyframe::with(vec![moved]));
    }
    d.scene.layers[0].keyframes.insert(0, k0);
    d.scene.layers[0].end = 11;
    Editor::new(d)
}
fn keys(e: &Editor) -> Vec<u32> {
    e.doc.scene.layers[0].keyframes.keys().copied().collect()
}
fn nudge(e: &mut Editor) {
    e.select(Some(Selection { layer: 0, element: 0 }), false);
    e.modify_selected("Move", |el| el.matrix = Matrix::translate(5.0, 7.0));
}

#[test]
fn auto_keyframe_defaults_off_and_keeps_old_behaviour() {
    let mut e = hold_doc(false);
    assert!(!e.auto_keyframe);
    e.set_frame(4);
    nudge(&mut e);
    assert_eq!(keys(&e), vec![0], "editing a hold edits its keyframe");
}

#[test]
fn auto_keyframe_on_creates_a_keyframe_on_a_hold_frame() {
    let mut e = hold_doc(false);
    e.auto_keyframe = true;
    e.set_frame(4);
    let n = e.history().0.len();
    nudge(&mut e);
    assert_eq!(keys(&e), vec![0, 4]);
    assert_eq!(e.history().0.len(), n + 1, "one undo step");
    // the first keyframe keeps its old pose
    assert_eq!(e.doc.scene.layers[0].keyframes[&0].elements[0].matrix, Matrix::IDENTITY);
    e.undo();
    assert_eq!(keys(&e), vec![0]);
}

#[test]
fn auto_keyframe_on_a_keyframe_adds_nothing() {
    let mut e = hold_doc(false);
    e.auto_keyframe = true;
    nudge(&mut e);
    assert_eq!(keys(&e), vec![0]);
}

#[test]
fn editing_inside_a_tween_sets_a_pose_either_way() {
    for on in [false, true] {
        let mut e = hold_doc(true);
        e.auto_keyframe = on;
        e.set_frame(5);
        nudge(&mut e);
        assert!(keys(&e).contains(&5), "auto_keyframe={on}: {:?}", keys(&e));
    }
}

#[test]
fn auto_keyframe_with_empty_selection_changes_nothing() {
    let mut e = hold_doc(false);
    e.auto_keyframe = true;
    e.set_frame(4);
    let n = e.history().0.len();
    assert!(!e.modify_selected("Move", |el| el.matrix = Matrix::translate(1.0, 1.0)));
    assert_eq!(keys(&e), vec![0]);
    assert_eq!(e.history().0.len(), n);
}

#[test]
fn auto_keyframe_applies_per_layer_for_multi_layer_selection() {
    let mut e = hold_doc(false);
    let mut l1 = Layer::new("B", 1);
    l1.keyframes.insert(0, Keyframe::with(vec![Element::shape("", Shape::new(Path::rect(0.0, 0.0, 4.0, 4.0), Some(Color::rgb(9, 9, 9)), None))]));
    l1.end = 11;
    e.doc.scene.layers.push(l1);
    e.auto_keyframe = true;
    e.set_frame(3);
    e.select(Some(Selection { layer: 0, element: 0 }), false);
    e.select(Some(Selection { layer: 1, element: 0 }), true);
    e.modify_selected("Move", |el| el.matrix = Matrix::translate(2.0, 2.0));
    assert_eq!(e.doc.scene.layers[1].keyframes.keys().copied().collect::<Vec<_>>(), vec![0, 3]);
    assert_eq!(keys(&e), vec![0, 3]);
}

// ---------------------------------------------------------------- 3D space

use ac_core::deform::{project_3d, project_3d_in, transform_3d, Perspective, Transform3D};

fn near(a: (f32, f32, f32), b: (f32, f32, f32)) -> bool {
    (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3 && (a.2 - b.2).abs() < 1e-3
}
const O: Pt = Pt { x: 0.0, y: 0.0 };

#[test]
fn global_ry_90_sends_x_to_minus_z() {
    let t = Transform3D { ry: 90.0, ..Default::default() };
    assert!(near(transform_3d(&t, O, pt(10.0, 0.0), 0.0), (0.0, 0.0, -10.0)));
    // a point on the rotation axis does not move
    assert!(near(transform_3d(&t, O, pt(0.0, 10.0), 0.0), (0.0, 10.0, 0.0)));
}
#[test]
fn local_equals_global_for_an_unrotated_object() {
    // (depth along the normal differs once the object is rotated in 3D: tested separately)
    for t in [Transform3D { rx: 30.0, ry: 40.0, rz: 50.0, ..Default::default() }, Transform3D { z: 25.0, ..Default::default() }] {
        let l = Transform3D { local: true, ..t };
        for p in [pt(10.0, 3.0), pt(-4.0, 8.0)] {
            assert!(near(transform_3d(&t, O, p, 0.0), transform_3d(&l, O, p, 0.0)));
        }
    }
}
#[test]
fn local_axes_follow_the_objects_rotation() {
    // object turned 90 degrees: its local x axis is the stage's y axis, so a local
    // Y rotation leaves stage-y points (local x) turning about the local y axis
    let g = Transform3D { ry: 90.0, ..Default::default() };
    let l = Transform3D { local: true, ..g };
    let th = std::f32::consts::FRAC_PI_2;
    let p = pt(0.0, 10.0);
    assert!(near(transform_3d(&g, O, p, th), (0.0, 10.0, 0.0)), "global ignores the object's rotation");
    assert!(near(transform_3d(&l, O, p, th), (0.0, 0.0, -10.0)), "{:?}", transform_3d(&l, O, p, th));
}
#[test]
fn local_depth_moves_along_the_rotated_normal() {
    let g = Transform3D { ry: 90.0, z: 100.0, ..Default::default() };
    let l = Transform3D { local: true, ..g };
    // global: straight away from the viewer; local: the normal now lies along +x
    assert!(near(transform_3d(&g, O, O, 0.0), (0.0, 0.0, 100.0)));
    let r = transform_3d(&l, O, O, 0.0);
    assert!(near(r, (100.0, 0.0, 0.0)) || near(r, (-100.0, 0.0, 0.0)), "{r:?}");
}
#[test]
fn local_rotation_about_the_centre_keeps_it_fixed() {
    let l = Transform3D { rx: 20.0, ry: 33.0, rz: 70.0, local: true, ..Default::default() };
    let c = pt(50.0, 40.0);
    assert!(near(transform_3d(&l, c, c, 0.9), (50.0, 40.0, 0.0)));
}
#[test]
fn rotations_preserve_distances_from_centre() {
    let l = Transform3D { rx: 20.0, ry: 33.0, rz: 70.0, local: true, ..Default::default() };
    let p = pt(7.0, -3.0);
    let (x, y, z) = transform_3d(&l, O, p, 0.4);
    assert!(((x * x + y * y + z * z).sqrt() - (49.0f32 + 9.0).sqrt()).abs() < 1e-3);
}
#[test]
fn non_finite_3d_values_do_not_poison_the_projection() {
    let t = Transform3D { rx: f32::NAN, ry: f32::INFINITY, z: f32::NAN, local: true, ..Default::default() };
    let pp = Perspective { angle_deg: 55.0, vanishing: pt(100.0, 100.0), stage_width: 200.0 };
    let r = project_3d_in(&t, O, &pp, pt(5.0, 5.0), f32::NAN);
    assert!(r.x.is_finite() && r.y.is_finite());
    let g = project_3d(&Transform3D::default(), O, &pp, pt(5.0, 5.0));
    assert!((g.x - 5.0).abs() < 1e-3);
}
#[test]
fn transform3d_serde_defaults_to_global() {
    let t: Transform3D = serde_json::from_str(r#"{"rx":1.0,"ry":2.0,"rz":3.0,"z":4.0,"center":{"x":0.0,"y":0.0}}"#).unwrap();
    assert!(!t.local);
    let j = serde_json::to_string(&Transform3D { local: true, ..t }).unwrap();
    assert!(serde_json::from_str::<Transform3D>(&j).unwrap().local);
}
#[test]
fn lerp_keeps_the_space_flag() {
    let a = Transform3D { local: true, ry: 10.0, ..Default::default() };
    let r = Transform3D::lerp(Some(a), None, 0.25).unwrap();
    assert!(r.local && (r.ry - 7.5).abs() < 1e-4);
}

fn d3_editor() -> Editor {
    let mut e = hold_doc(false);
    e.select(Some(Selection { layer: 0, element: 0 }), false);
    e.modify_selected("3D", |el| el.three_d = Some(Transform3D { ry: 30.0, ..Default::default() }));
    e
}
#[test]
fn set_3d_local_is_one_undo_step_and_remembers_the_mode() {
    let mut e = d3_editor();
    let n = e.history().0.len();
    assert!(e.set_3d_local(true) && e.three_d_local);
    assert!(e.element_at(Selection { layer: 0, element: 0 }).unwrap().three_d.unwrap().local);
    assert_eq!(e.history().0.len(), n + 1);
    e.undo();
    assert!(!e.element_at(Selection { layer: 0, element: 0 }).unwrap().three_d.unwrap().local);
}
#[test]
fn set_3d_local_without_selection_only_sets_the_tool_mode() {
    let mut e = hold_doc(false);
    let n = e.history().0.len();
    assert!(!e.set_3d_local(true));
    assert!(e.three_d_local);
    assert_eq!(e.history().0.len(), n);
    // already in that mode: no step
    let mut e = d3_editor();
    e.set_3d_local(true);
    let n = e.history().0.len();
    assert!(!e.set_3d_local(true));
    assert_eq!(e.history().0.len(), n);
}
#[test]
fn rendering_uses_the_local_space() {
    // a rotated 100x100 square with a Y rotation: local and global project differently
    let mk = |local: bool| {
        let mut e = hold_doc(false);
        e.select(Some(Selection { layer: 0, element: 0 }), false);
        e.modify_selected("x", move |el| {
            el.matrix = Matrix::rotate(std::f32::consts::FRAC_PI_2);
            el.three_d = Some(Transform3D { ry: 60.0, center: pt(5.0, 5.0), local, ..Default::default() });
        });
        e.evaluate_current(0).iter().map(|i| i.path.bounds().unwrap()).next().unwrap()
    };
    let (g, l) = (mk(false), mk(true));
    assert!((g.1.x - g.0.x - (l.1.x - l.0.x)).abs() > 0.5 || (g.1.y - g.0.y - (l.1.y - l.0.y)).abs() > 0.5, "{g:?} vs {l:?}");
}

// ---------------------------------------------------------------- Asset Warp density

use ac_core::deform::MeshWarp;

fn warp(density: u32) -> MeshWarp {
    MeshWarp { pins: vec![(pt(0.0, 0.0), pt(0.0, 0.0)), (pt(100.0, 0.0), pt(100.0, 0.0)), (pt(50.0, 50.0), pt(60.0, 70.0))], density }
}
fn tris(w: &MeshWarp) -> usize {
    w.map(pt(0.0, 0.0), pt(100.0, 100.0), &Matrix::IDENTITY).triangles().len()
}

#[test]
fn default_density_is_unchanged() {
    // 24 cells along the longer side, as before the option existed
    let m = warp(0).map(pt(0.0, 0.0), pt(100.0, 100.0), &Matrix::IDENTITY);
    assert_eq!((m.cols, m.rows), (24, 24));
    assert_eq!(warp(0).effective_density(), 24);
    assert_eq!(tris(&warp(0)), 24 * 24 * 2);
}
#[test]
fn higher_density_means_more_triangles() {
    assert!(tris(&warp(48)) > tris(&warp(0)));
    assert!(tris(&warp(8)) < tris(&warp(0)));
    assert_eq!(tris(&warp(48)), 48 * 48 * 2);
}
#[test]
fn density_is_clamped() {
    assert_eq!(warp(1).effective_density(), MeshWarp::MIN_DENSITY);
    assert_eq!(warp(100_000).effective_density(), MeshWarp::MAX_DENSITY);
    assert_eq!(tris(&warp(u32::MAX)), 64 * 64 * 2);
}
#[test]
fn pins_stay_fixed_at_every_density() {
    for d in [0, 4, 12, 24, 64] {
        let w = warp(d);
        for (p, q) in &w.pins {
            let r = ac_core::deform::rigid_mls(&w.pins, *p);
            assert!((r.x - q.x).abs() < 1e-3 && (r.y - q.y).abs() < 1e-3, "density {d}");
        }
        // a pinned grid corner lands on its target
        let m = w.map(pt(0.0, 0.0), pt(100.0, 100.0), &Matrix::IDENTITY);
        assert!(m.dst[0].dist(pt(0.0, 0.0)) < 1e-3, "density {d}");
        assert!(m.dst[m.cols].dist(pt(100.0, 0.0)) < 1e-3, "density {d}");
    }
}
#[test]
fn finer_mesh_follows_the_exact_deformation_more_closely() {
    let w = warp(0);
    let err = |d: u32| {
        let m = MeshWarp { density: d, ..w.clone() }.map(pt(0.0, 0.0), pt(100.0, 100.0), &Matrix::IDENTITY);
        // bilinear interpolation inside each cell vs the exact deformation at the cell centre
        let mut worst = 0.0f32;
        for r in 0..m.rows {
            for c in 0..m.cols {
                let i = |c: usize, r: usize| r * (m.cols + 1) + c;
                let (a, b, e, f) = (m.dst[i(c, r)], m.dst[i(c + 1, r)], m.dst[i(c, r + 1)], m.dst[i(c + 1, r + 1)]);
                let mid = pt((a.x + b.x + e.x + f.x) / 4.0, (a.y + b.y + e.y + f.y) / 4.0);
                let s = pt((m.src[i(c, r)].x + m.src[i(c + 1, r + 1)].x) / 2.0, (m.src[i(c, r)].y + m.src[i(c + 1, r + 1)].y) / 2.0);
                worst = worst.max(mid.dist(ac_core::deform::rigid_mls(&w.pins, s)));
            }
        }
        worst
    };
    assert!(err(64) < err(6));
}
#[test]
fn old_warps_load_with_default_density_and_defaults_are_not_written() {
    let w: MeshWarp = serde_json::from_str(r#"{"pins":[[{"x":0.0,"y":0.0},{"x":1.0,"y":1.0}]]}"#).unwrap();
    assert_eq!(w.density, 0);
    assert!(!serde_json::to_string(&w).unwrap().contains("density"));
    let j = serde_json::to_string(&warp(40)).unwrap();
    assert_eq!(serde_json::from_str::<MeshWarp>(&j).unwrap().density, 40);
}

fn mesh_editor() -> Editor {
    let mut e = hold_doc(false);
    e.select(Some(Selection { layer: 0, element: 0 }), false);
    e.modify_selected("warp", |el| el.mesh = Some(warp(0)));
    e
}
fn dens(e: &Editor) -> u32 {
    e.element_at(Selection { layer: 0, element: 0 }).unwrap().mesh.unwrap().density
}
#[test]
fn set_mesh_density_one_undo_step_and_clamped() {
    let mut e = mesh_editor();
    let n = e.history().0.len();
    assert!(e.set_mesh_density(40) && dens(&e) == 40);
    assert_eq!(e.history().0.len(), n + 1);
    e.undo();
    e.select(Some(Selection { layer: 0, element: 0 }), false);
    assert_eq!(dens(&e), 0);
    assert!(e.set_mesh_density(9999) && dens(&e) == MeshWarp::MAX_DENSITY);
    assert!(e.set_mesh_density(2) && dens(&e) == MeshWarp::MIN_DENSITY);
    assert!(e.set_mesh_density(0) && dens(&e) == 0);
}
#[test]
fn set_mesh_density_noop_without_selection_or_warp_or_change() {
    let mut e = mesh_editor();
    e.select(None, false);
    let n = e.history().0.len();
    assert!(!e.set_mesh_density(40));
    let mut e2 = hold_doc(false);
    e2.select(Some(Selection { layer: 0, element: 0 }), false);
    assert!(!e2.set_mesh_density(40));
    e.select(Some(Selection { layer: 0, element: 0 }), false);
    assert!(!e.set_mesh_density(0), "already default");
    assert_eq!(e.history().0.len(), n);
}

// ---------------------------------------------------------------- Bone springiness

/// Parent (element 0) with a child (element 1); the child's key rotates from 0 to
/// `to` degrees at frame 10 (a hold, so the target is a step).
fn rig(to: f32, spring: f32, damping: f32) -> Layer {
    let mk = |x: f32, rot: f32| {
        let mut e = Element::shape("", Shape::new(Path::rect(0.0, 0.0, 10.0, 10.0), Some(Color::rgb(1, 2, 3)), None));
        e.matrix = Matrix::translate(x, 0.0).concat(&Matrix::rotate(rot.to_radians()));
        e
    };
    let mut l = Layer::new("rig", 0);
    l.keyframes.insert(0, Keyframe::with(vec![mk(0.0, 0.0), mk(20.0, 0.0)]));
    l.keyframes.insert(10, Keyframe::with(vec![mk(0.0, 0.0), mk(20.0, to)]));
    l.end = 400;
    l.bones.push(Bone { parent: 0, child: 1, head: pt(0.0, 0.0), tail_parent: pt(20.0, 0.0), tail_child: pt(0.0, 0.0), rotate: true, limit: None, rest: 0.0, pin: None, spring, damping });
    l
}
fn ang(l: &Layer, f: u32) -> f32 {
    let (els, _) = layer_elements_at(l, f).unwrap();
    els[1].matrix.b.atan2(els[1].matrix.a).to_degrees()
}

#[test]
fn spring_off_follows_the_keys_exactly() {
    let l = rig(60.0, 0.0, 1.0);
    assert!(ang(&l, 9).abs() < 1e-3 && (ang(&l, 10) - 60.0).abs() < 1e-3);
}
#[test]
fn step_response_converges_to_the_target() {
    let l = rig(60.0, 0.4, 0.5);
    assert!((ang(&l, 100) - 60.0).abs() < 0.5, "{}", ang(&l, 100));
    let l = rig(60.0, 0.4, 1.0);
    assert!((ang(&l, 100) - 60.0).abs() < 0.01);
    let l = rig(60.0, 0.4, 3.0);
    assert!((ang(&l, 300) - 60.0).abs() < 0.5);
}
#[test]
fn critical_damping_never_overshoots() {
    let l = rig(60.0, 0.5, 1.0);
    let mut prev = 0.0;
    for f in 0..200 {
        let a = ang(&l, f);
        assert!(a <= 60.0 + 1e-3, "frame {f}: {a}");
        assert!(a >= prev - 1e-3, "monotone at {f}");
        prev = a;
    }
}
#[test]
fn underdamped_overshoots_and_lags() {
    let l = rig(60.0, 0.5, 0.2);
    let peak = (10..200).map(|f| ang(&l, f)).fold(0.0f32, f32::max);
    assert!(peak > 62.0, "peak {peak}");
    assert!(ang(&l, 10) < 60.0, "the child lags the key");
}
#[test]
fn stiffer_springs_reach_the_target_sooner() {
    let soft = rig(60.0, 0.15, 1.0);
    let stiff = rig(60.0, 0.9, 1.0);
    assert!(ang(&stiff, 13) > ang(&soft, 13));
}
#[test]
fn scrubbing_is_deterministic() {
    let l = rig(60.0, 0.3, 0.4);
    let forward: Vec<f32> = (0..60).map(|f| ang(&l, f)).collect();
    let backward: Vec<f32> = (0..60).rev().map(|f| ang(&l, f)).rev().collect();
    assert_eq!(forward, backward);
    // jumping straight to a frame gives the same pose as having played up to it
    let again = rig(60.0, 0.3, 0.4);
    assert_eq!(ang(&again, 37), forward[37]);
}
#[test]
fn spring_exact_step_matches_fine_integration() {
    let (w, z) = (0.5f32, 0.3f32);
    let (e1, v1) = ac_core::model::spring_step(1.0, 0.0, w, z);
    let (mut e, mut v) = (1.0f64, 0.0f64);
    let n = 200_000;
    let h = 1.0 / n as f64;
    for _ in 0..n {
        let a = -2.0 * z as f64 * w as f64 * v - (w as f64).powi(2) * e;
        v += a * h;
        e += v * h;
    }
    assert!((e1 as f64 - e).abs() < 1e-3 && (v1 as f64 - v).abs() < 1e-3, "{e1} {e} {v1} {v}");
}
#[test]
fn spring_params_are_validated() {
    let b = |s: f32, d: f32| Bone { spring: s, damping: d, ..rig(0.0, 0.0, 1.0).bones[0] };
    assert_eq!(b(f32::NAN, 1.0).spring_params().0, 0.0);
    assert_eq!(b(1000.0, 1.0).spring_params().0, Bone::MAX_SPRING);
    assert_eq!(b(0.5, f32::INFINITY).spring_params().1, 1.0);
    assert_eq!(b(0.5, -3.0).spring_params().1, 0.05);
    assert!(!b(f32::NAN, 1.0).has_spring());
    // hostile values must not poison the evaluation
    let mut l = rig(60.0, f32::NAN, f32::NAN);
    l.bones[0].spring = f32::INFINITY;
    assert!(ang(&l, 20).is_finite());
}
#[test]
fn only_the_sprung_child_is_filtered_and_it_stays_joined() {
    let l = rig(60.0, 0.4, 0.5);
    let (els, _) = layer_elements_at(&l, 12).unwrap();
    assert_eq!(els[0].matrix, Matrix::IDENTITY);
    // the child's joint (its origin) still sits on the parent's joint
    let j = els[1].matrix.apply(pt(0.0, 0.0));
    assert!((j.x - 20.0).abs() < 1e-3 && j.y.abs() < 1e-3);
}
#[test]
fn bone_serde_back_compat() {
    let b: Bone = serde_json::from_str(r#"{"parent":0,"child":1,"head":{"x":0.0,"y":0.0},"tail_parent":{"x":1.0,"y":0.0},"tail_child":{"x":0.0,"y":0.0}}"#).unwrap();
    assert_eq!((b.spring, b.damping), (0.0, 1.0));
    let j = serde_json::to_string(&b).unwrap();
    assert!(!j.contains("spring") && !j.contains("damping"));
    let s = Bone { spring: 0.4, damping: 0.6, ..b };
    let back: Bone = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
    assert_eq!((back.spring, back.damping), (0.4, 0.6));
}
#[test]
fn set_bone_spring_validates_and_is_one_undo_step() {
    let mut e = hold_doc(false);
    e.doc.scene.layers[0].keyframes.get_mut(&0).unwrap().elements.push(Element::shape("", Shape::new(Path::rect(0.0, 0.0, 5.0, 5.0), Some(Color::rgb(0, 0, 0)), None)));
    e.doc.scene.layers[0].bones.push(Bone { parent: 0, child: 1, head: pt(0.0, 0.0), tail_parent: pt(1.0, 0.0), tail_child: pt(0.0, 0.0), rotate: true, limit: None, rest: 0.0, pin: None, spring: 0.0, damping: 1.0 });
    let n = e.history().0.len();
    assert!(!e.set_bone_spring(0, 1, f32::NAN, 1.0) && !e.set_bone_spring(0, 1, 0.5, f32::INFINITY));
    assert!(!e.set_bone_spring(0, 7, 0.5, 1.0), "no such bone");
    assert!(!e.set_bone_spring(5, 1, 0.5, 1.0), "no such layer");
    assert!(!e.set_bone_spring(0, 1, 0.0, 1.0), "unchanged");
    assert_eq!(e.history().0.len(), n);
    assert!(e.set_bone_spring(0, 1, 99.0, 0.0));
    assert_eq!((e.bones(0)[0].spring, e.bones(0)[0].damping), (Bone::MAX_SPRING, 0.05));
    assert_eq!(e.history().0.len(), n + 1);
    e.undo();
    assert_eq!(e.bones(0)[0].spring, 0.0);
}
