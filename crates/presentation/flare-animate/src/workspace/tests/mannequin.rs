use super::*;
use crate::workspace::Tool;
use crate::workspace::mannequin::PlaceInto;
use crate::workspace::panels::mannequin::{joint_key, pose_key, proportion_key};
use crate::workspace::panels::mannequin_view::{RING_RADIUS, view_placement};
use egui::PointerButton;
use flare_mannequin::{Axis, Body, Joint, Proportion};

/// Draws the panel alone for a pass with `events`.
fn panel(ws: &mut Workspace, ctx: &egui::Context, time: f64, events: Vec<Event>) {
    let mut out = ctx.run_ui(input(time, events), |ui| ws.mannequin_panel(ui));
    out.textures_delta.clear();
}

fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

/// Presses at `from`, moves to `to` in steps and lets go.
fn drag(ws: &mut Workspace, ctx: &egui::Context, time: &mut f64, from: Pos2, to: Pos2) {
    let mut step = |ws: &mut Workspace, events| {
        *time += 0.05;
        panel(ws, ctx, *time, events);
    };
    step(ws, vec![Event::PointerMoved(from)]);
    step(ws, vec![button(from, true)]);
    for i in 1..=6 {
        let p = from + (to - from) * (i as f32 / 6.0);
        step(ws, vec![Event::PointerMoved(p)]);
    }
    step(ws, vec![button(to, false)]);
    step(ws, vec![]);
}

/// The panel laid out, and where a joint's dot is in it.
fn laid_out() -> (Workspace, egui::Context, f64) {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    for step in 0..3 {
        panel(&mut ws, &ctx, f64::from(step) * 0.1, vec![]);
    }
    (ws, ctx, 0.3)
}

fn placement(ws: &Workspace) -> flare_mannequin::Placement {
    let rect = ws.mannequin.view_rect.expect("drawn");
    view_placement(rect, &ws.mannequin.camera).expect("placement")
}

fn mark(ws: &Workspace, joint: Joint) -> Pos2 {
    let at = ws.mannequin().marks(placement(ws))[joint.index()].at;
    Pos2::new(at[0] as f32, at[1] as f32)
}

fn click_with(
    ws: &mut Workspace,
    ctx: &egui::Context,
    time: &mut f64,
    p: Pos2,
    which: PointerButton,
) {
    let press = |pressed| Event::PointerButton {
        pos: p,
        button: which,
        pressed,
        modifiers: Modifiers::NONE,
    };
    for events in [
        vec![Event::PointerMoved(p)],
        vec![press(true)],
        vec![press(false)],
        vec![],
    ] {
        *time += 0.05;
        panel(ws, ctx, *time, events);
    }
}

#[test]
fn the_panel_is_in_the_strip_and_the_window_menu_brings_it_up() {
    let mut ws = workspace();
    assert!(ws.panels.layout.in_strip(Pane::Mannequin));
    ws.perform(MenuAction::ShowMannequin);
    assert!(ws.pane_is_front(Pane::Mannequin));
}

#[test]
fn every_name_the_panel_builds_has_a_message() {
    for preset in flare_mannequin::presets() {
        assert!(flare_i18n::has_key(&pose_key(&preset.id)), "{}", preset.id);
    }
    for joint in Joint::ALL {
        assert!(flare_i18n::has_key(&joint_key(joint)), "{joint:?}");
    }
    for p in Proportion::ALL {
        assert!(flare_i18n::has_key(&proportion_key(p)), "{p:?}");
    }
}

#[test]
fn presets_bodies_mirror_and_reset_change_the_figure() {
    let mut ws = workspace();
    assert!(ws.set_mannequin_pose("wave"));
    assert!(!ws.set_mannequin_pose("moonwalk"));
    let waving = ws.mannequin().pose;
    ws.mirror_mannequin();
    assert_eq!(ws.mannequin().pose, waving.mirrored());
    ws.set_mannequin_body(Body::Female);
    assert_eq!(ws.mannequin().body, Body::Female);
    ws.mannequin
        .figure
        .proportions
        .set_clamped(Proportion::Legs, 1.2);
    ws.reset_mannequin_proportions();
    assert_eq!(ws.mannequin().proportions.get(Proportion::Legs), 1.0);
    ws.reset_mannequin_pose();
    assert_eq!(ws.mannequin().pose, flare_mannequin::Pose::default());
    ws.mannequin.figure.orbit(40.0, 10.0);
    ws.mannequin.camera.set_zoom(2.0);
    ws.reset_mannequin_view();
    assert_eq!(ws.mannequin().view, flare_mannequin::View::default());
    assert_eq!(ws.mannequin.camera, flare_mannequin::Camera::default());
}

#[test]
fn the_feet_start_pinned_and_only_hands_and_feet_pin() {
    let mut ws = workspace();
    assert_eq!(ws.mannequin.held, vec![Joint::LeftAnkle, Joint::RightAnkle]);
    assert!(ws.toggle_mannequin_pin(Joint::LeftAnkle));
    assert!(ws.toggle_mannequin_pin(Joint::RightWrist));
    assert_eq!(
        ws.mannequin.held,
        vec![Joint::RightAnkle, Joint::RightWrist]
    );
    assert!(!ws.toggle_mannequin_pin(Joint::Chest));
}

#[test]
fn placing_the_figure_adds_an_unlocked_layer_on_top_as_one_undo_step() {
    let mut ws = workspace();
    let before = layer_count(&ws);
    ws.set_mannequin_pose("run");
    assert!(ws.place_mannequin());
    assert_eq!(layer_count(&ws), before + 1);
    let top = ws.document().scenes()[0].timeline().layers()[0].clone();
    assert_eq!(top.name(), "Mannequin");
    assert!(!top.is_locked());
    assert_eq!(
        top.view_opacity(),
        flare_tools::mannequin::REFERENCE_OPACITY
    );
    assert!(matches!(top.spans()[0].content(), Content::Bitmap(_)));
    assert!(ws.status().is_some_and(|s| s.contains("Mannequin")));
    ws.perform(MenuAction::Undo);
    assert_eq!(layer_count(&ws), before);
}

#[test]
fn the_placed_figure_is_selected_and_moves_and_scales_like_any_picture() {
    let mut ws = workspace();
    assert!(ws.place_mannequin());
    assert_eq!(ws.tool(), Tool::FreeTransform, "its handles show at once");
    let span = ws.selected.expect("selected");
    let top = ws.document().scenes()[0].timeline().layers()[0].id();
    assert_eq!((span.layer, span.element), (top, 0));
    let bounds = |ws: &Workspace| ws.selection_with_bounds().expect("bounds").2[0];
    let placed = bounds(&ws);

    ws.move_selection(Vec2::new(30.0, -20.0));
    let moved = bounds(&ws);
    assert!(
        (moved.min - placed.min - Vec2::new(30.0, -20.0)).length() < 0.5,
        "{moved:?}"
    );

    ws.set_selection_bounds(Rect::from_min_size(moved.min, moved.size() * 0.5));
    let scaled = bounds(&ws);
    assert!(
        (scaled.width() - moved.width() * 0.5).abs() < 1.0,
        "{scaled:?}"
    );
    assert!((scaled.height() - moved.height() * 0.5).abs() < 1.0);

    // A press on it on the stage picks it again, as the Selection tool does; the tools that
    // paint or erase shapes do not take it for one.
    ws.selected = None;
    assert_eq!(ws.pick(scaled.center()), Some(span));
    assert_eq!(ws.hit(scaled.center()), None);
    ws.select_at(scaled.center(), false);
    assert_eq!(ws.selected, Some(span));

    // Each change is its own undo step.
    ws.perform(MenuAction::Undo);
    ws.selected = Some(span);
    assert!((bounds(&ws).width() - moved.width()).abs() < 1.0);
}

#[test]
fn the_placed_figure_is_dragged_on_the_stage_with_the_selection_tool() {
    let mut ws = workspace();
    assert!(ws.place_mannequin());
    let bounds = |ws: &Workspace| ws.selection_with_bounds().expect("bounds").2[0];
    let placed = bounds(&ws);
    ws.selected = None;
    ws.set_tool(Tool::Select);
    let from = placed.center();
    ws.begin_drag(from);
    assert!(ws.selected.is_some(), "the press picked the picture");
    ws.update_drag(from + Vec2::new(40.0, 10.0));
    ws.end_drag();
    let moved = bounds(&ws);
    assert!(
        (moved.min - placed.min - Vec2::new(40.0, 10.0)).length() < 0.5,
        "{moved:?}"
    );
    assert!((moved.size() - placed.size()).length() < 0.5, "only moved");
}

/// The first layer's spans, as (start, length, elements).
fn top_spans(ws: &Workspace) -> Vec<(u32, u32, usize)> {
    ws.document().scenes()[0].timeline().layers()[0]
        .spans()
        .iter()
        .map(|s| (s.start(), s.duration().get(), s.element_count()))
        .collect()
}

#[test]
fn several_figures_share_a_keyframe_or_each_take_one_on_the_same_layer() {
    let mut ws = workspace();
    let layers = layer_count(&ws);
    assert!(ws.place_mannequin());
    assert_eq!(
        layer_count(&ws),
        layers + 1,
        "the first on a layer of its own"
    );
    assert_eq!(top_spans(&ws), vec![(0, 24, 1)]);

    // A second in the same frames: it joins the keyframe and is the one selected.
    ws.set_mannequin_place_into(PlaceInto::ThisKeyframe);
    ws.set_mannequin_pose("run");
    assert!(ws.place_mannequin());
    assert_eq!(layer_count(&ws), layers + 1, "no new layer");
    assert_eq!(top_spans(&ws), vec![(0, 24, 2)]);
    assert_eq!(ws.selected.map(|s| (s.start, s.element)), Some((0, 1)));

    // Another pose from frame 10: a keyframe of its own on the same layer.
    ws.set_frame(10);
    ws.set_mannequin_place_into(PlaceInto::NewKeyframe);
    ws.set_mannequin_pose("jump");
    assert!(ws.place_mannequin());
    assert_eq!(top_spans(&ws), vec![(0, 10, 2), (10, 14, 1)]);
    assert_eq!(ws.selected.map(|s| (s.start, s.element)), Some((10, 0)));

    // Each placing is its own undo step.
    ws.perform(MenuAction::Undo);
    assert_eq!(top_spans(&ws), vec![(0, 24, 2)]);
}

#[test]
fn a_figure_is_not_placed_into_a_locked_layer_and_the_reason_is_shown() {
    let mut ws = workspace();
    let layer = ws.active_layer_id().expect("layer");
    ws.toggle_layer_locked(layer);
    ws.set_mannequin_place_into(PlaceInto::ThisKeyframe);
    let layers = layer_count(&ws);
    assert!(!ws.place_mannequin());
    assert_eq!(layer_count(&ws), layers);
    assert!(
        ws.status().is_some_and(|s| s.contains("locked")),
        "{:?}",
        ws.status()
    );
}

#[test]
fn the_figure_stands_in_the_middle_of_the_stage_unless_moved() {
    let mut ws = workspace();
    let stage = ws.document().stage();
    let (w, h) = (f64::from(stage.width()), f64::from(stage.height()));
    let at = ws.mannequin_on_stage().expect("placement");
    assert_eq!(at.center, [w / 2.0, h / 2.0]);
    assert_eq!(at.height, h * 0.8);
    ws.mannequin.stage_at = Some(([10.0, 20.0], 99.0));
    let at = ws.mannequin_on_stage().expect("placement");
    assert_eq!((at.center, at.height), ([10.0, 20.0], 99.0));
    ws.mannequin.stage_at = Some(([10.0, 20.0], -1.0));
    assert!(
        ws.mannequin_on_stage().is_none(),
        "a bad size places nothing"
    );
}

#[test]
fn the_figure_is_kept_with_the_preferences_and_a_bad_one_is_ignored() {
    let mut ws = workspace();
    ws.set_mannequin_pose("sit");
    ws.set_mannequin_body(Body::Female);
    let preferences = ws.preferences();
    let mut next = workspace();
    next.apply_preferences(&preferences);
    assert_eq!(next.mannequin(), ws.mannequin());
    assert!(
        preferences.mannequin.len() < 2048,
        "small: {}",
        preferences.mannequin.len()
    );

    let mut bad = preferences.clone();
    bad.mannequin = r#"{"version": 1, "body": "robot"}"#.into();
    let mut kept = workspace();
    let default = *kept.mannequin();
    kept.apply_preferences(&bad);
    assert_eq!(*kept.mannequin(), default);
}

#[test]
fn a_drag_on_empty_space_turns_the_view_and_a_click_picks_a_part() {
    let (mut ws, ctx, mut time) = laid_out();
    let rect = ws.mannequin.view_rect.expect("drawn");
    let yaw = ws.mannequin().view.yaw();
    let pose = ws.mannequin().pose;
    let corner = rect.min + Vec2::new(6.0, 6.0);
    drag(
        &mut ws,
        &ctx,
        &mut time,
        corner,
        corner + Vec2::new(40.0, 0.0),
    );
    assert!((ws.mannequin().view.yaw() - yaw - 20.0).abs() < 1.0);
    assert_eq!(ws.mannequin().pose, pose, "the pose is untouched");

    let head = mark(&ws, Joint::Head) + Vec2::new(0.0, -10.0);
    for (dt, events) in [
        (0.05, vec![Event::PointerMoved(head)]),
        (0.1, vec![button(head, true)]),
        (0.15, vec![button(head, false)]),
    ] {
        time += dt;
        panel(&mut ws, &ctx, time, events);
    }
    assert_eq!(ws.mannequin.selected, Some(Joint::Head));
}

#[test]
fn dragging_a_hand_moves_it_and_a_forearm_pulls_its_hand() {
    let (mut ws, ctx, mut time) = laid_out();
    let wrist = mark(&ws, Joint::LeftWrist);
    let target = wrist + Vec2::new(-12.0, -25.0);
    drag(&mut ws, &ctx, &mut time, wrist, target);
    assert!(matches!(
        ws.mannequin.selected,
        Some(Joint::LeftWrist | Joint::LeftElbow)
    ));
    let moved = mark(&ws, Joint::LeftWrist);
    assert!(
        (moved - target).length() < 2.0,
        "{moved:?} is not {target:?}"
    );

    let before = mark(&ws, Joint::RightWrist);
    let (from, to) = (mark(&ws, Joint::RightElbow), before);
    let grip = from + (to - from) * 0.5;
    drag(
        &mut ws,
        &ctx,
        &mut time,
        grip,
        grip + Vec2::new(25.0, -10.0),
    );
    assert_eq!(ws.mannequin.selected, Some(Joint::RightElbow));
    let after = mark(&ws, Joint::RightWrist);
    assert!(
        (after - before - Vec2::new(25.0, -10.0)).length() < 2.0,
        "{after:?}"
    );
}

#[test]
fn dragging_the_hips_down_over_pinned_feet_crouches() {
    let (mut ws, ctx, mut time) = laid_out();
    let feet = [mark(&ws, Joint::LeftAnkle), mark(&ws, Joint::RightAnkle)];
    let hips = mark(&ws, Joint::Pelvis);
    drag(&mut ws, &ctx, &mut time, hips, hips + Vec2::new(0.0, 40.0));
    assert_eq!(ws.mannequin.selected, Some(Joint::Pelvis));
    assert!(ws.mannequin().pose.lift().y < -0.05);
    for (joint, was) in [Joint::LeftAnkle, Joint::RightAnkle].into_iter().zip(feet) {
        assert!((mark(&ws, joint) - was).length() < 1.0, "{joint:?} stayed");
    }
    assert!(ws.mannequin().pose.angles(Joint::LeftKnee).bend > 20.0);
}

#[test]
fn a_right_click_on_a_foot_or_shin_pins_or_frees_it() {
    let (mut ws, ctx, mut time) = laid_out();
    let shin = (mark(&ws, Joint::LeftKnee) + mark(&ws, Joint::LeftAnkle).to_vec2()) / 2.0;
    click_with(&mut ws, &ctx, &mut time, shin, PointerButton::Secondary);
    assert_eq!(ws.mannequin.held, vec![Joint::RightAnkle]);
    let hand = mark(&ws, Joint::RightWrist) + Vec2::new(0.0, 4.0);
    click_with(&mut ws, &ctx, &mut time, hand, PointerButton::Secondary);
    assert!(ws.mannequin.held.contains(&Joint::RightWrist));
}

#[test]
fn a_clicked_part_shows_its_rings_and_a_drag_along_one_turns_about_that_axis() {
    let (mut ws, ctx, mut time) = laid_out();
    let arm = (mark(&ws, Joint::LeftShoulder) + mark(&ws, Joint::LeftElbow).to_vec2()) / 2.0;
    click_with(&mut ws, &ctx, &mut time, arm, PointerButton::Primary);
    assert_eq!(ws.mannequin.selected, Some(Joint::LeftShoulder));
    let rings = ws
        .mannequin()
        .rings(Joint::LeftShoulder, placement(&ws), RING_RADIUS);
    let side = rings.iter().find(|r| r.axis == Axis::Side).expect("ring");
    // A point on the near half of the ring, clear of the other two.
    let i = (0..side.points.len())
        .find(|i| {
            let p = side.points[*i];
            side.front[*i]
                && rings
                    .iter()
                    .filter(|r| r.axis != Axis::Side)
                    .flat_map(|r| r.points.iter())
                    .all(|q| (q[0] - p[0]).hypot(q[1] - p[1]) > 8.0)
        })
        .expect("a clear point");
    let n = side.points.len();
    let (p, next) = (side.points[i], side.points[(i + 1) % n]);
    let along = Vec2::new((next[0] - p[0]) as f32, (next[1] - p[1]) as f32).normalized();
    let from = Pos2::new(p[0] as f32, p[1] as f32);
    let was = ws.mannequin().pose.angles(Joint::LeftShoulder);
    drag(&mut ws, &ctx, &mut time, from, from + along * 20.0);
    let now = ws.mannequin().pose.angles(Joint::LeftShoulder);
    assert!((now.side - was.side).abs() > 10.0, "{was:?} {now:?}");
    assert_eq!((now.bend, now.twist), (was.bend, was.twist));
}

#[test]
fn the_grip_under_the_view_makes_it_taller_and_a_double_click_puts_it_back() {
    let (mut ws, ctx, mut time) = laid_out();
    let rect = ws.mannequin.view_rect.expect("drawn");
    let grip = Pos2::new(rect.center().x, rect.bottom() + 4.0);
    drag(&mut ws, &ctx, &mut time, grip, grip + Vec2::new(0.0, 120.0));
    let taller = ws.mannequin.view_height.expect("set");
    assert!((taller - rect.height() - 120.0).abs() < 2.0, "{taller}");
    panel(&mut ws, &ctx, time + 0.1, vec![]);
    assert!((ws.mannequin.view_rect.expect("drawn").height() - taller).abs() < 1.0);
    let grip = Pos2::new(rect.center().x, rect.top() + taller + 4.0);
    for (dt, events) in [
        (0.2, vec![Event::PointerMoved(grip)]),
        (0.25, vec![button(grip, true)]),
        (0.3, vec![button(grip, false)]),
        (0.35, vec![button(grip, true)]),
        (0.4, vec![button(grip, false)]),
    ] {
        panel(&mut ws, &ctx, time + dt, events);
    }
    assert_eq!(ws.mannequin.view_height, None);
}

#[test]
fn the_panel_beside_the_strip_widens_by_its_edge_and_when_made_large() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    ws.perform(MenuAction::ShowMannequin);
    let mut time = 0.0;
    for _ in 0..3 {
        time += 0.1;
        run(&mut ws, &ctx, time);
    }
    let id = egui::Id::NULL.with("strip-flyout");
    let rect = ctx.memory(|m| m.area_rect(id)).expect("open");
    let width = ws.flyout_width(Pane::Mannequin, 1280.0);
    let edge = Pos2::new(rect.left(), rect.center().y);
    let steps = [
        vec![Event::PointerMoved(edge)],
        vec![button(edge, true)],
        vec![Event::PointerMoved(edge - Vec2::new(60.0, 0.0))],
        vec![Event::PointerMoved(edge - Vec2::new(120.0, 0.0))],
        vec![button(edge - Vec2::new(120.0, 0.0), false)],
    ];
    for events in steps {
        time += 0.05;
        pass(&mut ws, &ctx, input(time, events));
    }
    assert!(ws.pane_is_front(Pane::Mannequin), "still open");
    let wider = ws.flyout_width(Pane::Mannequin, 1280.0);
    assert!((wider - width - 120.0).abs() < 2.0, "{width} then {wider}");

    ws.set_mannequin_expanded(true);
    assert!(ws.flyout_width(Pane::Mannequin, 1280.0) >= 760.0);
    assert!(
        ws.flyout_width(Pane::Mannequin, 500.0) <= 500.0,
        "never past the window"
    );
    assert_eq!(
        ws.flyout_width(Pane::Layers, 1280.0),
        crate::workspace::rail::FLYOUT_WIDTH
    );
    ws.set_mannequin_expanded(false);
    assert_eq!(ws.flyout_width(Pane::Mannequin, 1280.0), wider);
}

#[test]
fn the_expand_button_makes_the_view_tall() {
    let (mut ws, ctx, mut time) = laid_out();
    let before = ws.mannequin.view_rect.expect("drawn").height();
    ws.set_mannequin_expanded(true);
    time += 0.1;
    panel(&mut ws, &ctx, time, vec![]);
    let after = ws.mannequin.view_rect.expect("drawn").height();
    assert!(after > before + 20.0, "{before} then {after}");
}

#[test]
fn the_figure_shows_over_the_stage_only_when_asked() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    let shapes = |ws: &mut Workspace, time| {
        let mut output = ctx.run_ui(input(time, vec![]), |ui| ws.show(ui));
        output.textures_delta.clear();
        output.shapes.len()
    };
    let without = shapes(&mut ws, 0.1);
    ws.mannequin.on_stage = true;
    let with = shapes(&mut ws, 0.2);
    assert!(with > without, "{without} then {with}");
    assert!(
        ws.mannequin.stage_shown.borrow().is_some(),
        "its picture was made"
    );
}
