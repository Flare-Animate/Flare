//! Moving round the mannequin in its panel, and posing it from a picture.

use super::*;
use crate::workspace::mannequin_reference::{Loading, Picked, read};
use crate::workspace::panels::mannequin_view::view_placement;
use egui::PointerButton;
use flare_mannequin::{Camera, Depth, Facing, Joint, MARKED, Vec3};

fn panel(ws: &mut Workspace, ctx: &egui::Context, time: f64, events: Vec<Event>) {
    let mut out = ctx.run_ui(input(time, events), |ui| ws.mannequin_panel(ui));
    out.textures_delta.clear();
}

/// The panel laid out a few times, and the time it got to.
fn laid_out(ws: &mut Workspace) -> (egui::Context, f64) {
    let ctx = egui::Context::default();
    for step in 0..3 {
        panel(ws, &ctx, f64::from(step) * 0.1, vec![]);
    }
    (ctx, 0.3)
}

fn steps(ws: &mut Workspace, ctx: &egui::Context, time: &mut f64, events: Vec<Vec<Event>>) {
    for events in events {
        *time += 0.05;
        panel(ws, ctx, *time, events);
    }
}

fn press(p: Pos2, button: PointerButton, pressed: bool) -> Event {
    Event::PointerButton {
        pos: p,
        button,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

fn drag(
    ws: &mut Workspace,
    ctx: &egui::Context,
    time: &mut f64,
    from: Pos2,
    to: Pos2,
    button: PointerButton,
) {
    let mut events = vec![
        vec![Event::PointerMoved(from)],
        vec![press(from, button, true)],
    ];
    for i in 1..=6 {
        events.push(vec![Event::PointerMoved(
            from + (to - from) * (i as f32 / 6.0),
        )]);
    }
    events.push(vec![press(to, button, false)]);
    events.push(vec![]);
    steps(ws, ctx, time, events);
}

fn double_click(ws: &mut Workspace, ctx: &egui::Context, time: &mut f64, p: Pos2) {
    let primary = PointerButton::Primary;
    steps(
        ws,
        ctx,
        time,
        vec![
            vec![Event::PointerMoved(p)],
            vec![press(p, primary, true)],
            vec![press(p, primary, false)],
            vec![press(p, primary, true)],
            vec![press(p, primary, false)],
            vec![],
        ],
    );
}

fn view(ws: &Workspace) -> Rect {
    ws.mannequin.view_rect.expect("drawn")
}

fn placement(ws: &Workspace) -> flare_mannequin::Placement {
    view_placement(view(ws), &ws.mannequin.camera).expect("placement")
}

fn seen(ws: &Workspace, p: Vec3) -> Pos2 {
    let at = ws.mannequin().projector(placement(ws)).project(p).at;
    Pos2::new(at[0] as f32, at[1] as f32)
}

/// A blank spot of the view, away from the figure.
fn empty(ws: &Workspace) -> Pos2 {
    view(ws).left_top() + Vec2::new(12.0, 60.0)
}

fn picture(width: u32, height: u32) -> Vec<u8> {
    let rgba = vec![200u8; (width * height * 4) as usize];
    flare_runtime::rgba_png(width, height, &rgba).expect("png")
}

fn with_picture() -> (Workspace, egui::Context, f64) {
    let mut ws = workspace();
    ws.mannequin.figure.view = flare_mannequin::View::new(40.0, 20.0, 30.0);
    ws.import_mannequin_reference_bytes("person.png", picture(300, 400));
    let (ctx, time) = laid_out(&mut ws);
    (ws, ctx, time)
}

fn mark_seen(ws: &Workspace, joint: Joint) -> Pos2 {
    let reference = ws.mannequin.reference.as_ref().expect("picture");
    seen(
        ws,
        reference
            .board
            .point(reference.marks.get(joint).expect("marked")),
    )
}

#[test]
fn scrolling_over_the_view_zooms_toward_the_pointer_and_leaves_the_panel_where_it_is() {
    let mut ws = workspace();
    let (ctx, mut time) = laid_out(&mut ws);
    let top = view(&ws).top();
    let pointer = view(&ws).center() + Vec2::new(30.0, -60.0);
    let under = ws
        .mannequin()
        .projector(placement(&ws))
        .unproject([f64::from(pointer.x), f64::from(pointer.y)], 0.0);
    let wheel = Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: Vec2::new(0.0, 200.0),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
        modifiers: Modifiers::NONE,
    };
    let mut events = vec![vec![Event::PointerMoved(pointer)], vec![wheel]];
    events.extend((0..8).map(|_| vec![]));
    steps(&mut ws, &ctx, &mut time, events);
    assert!(
        ws.mannequin.camera.zoom() > 1.2,
        "{}",
        ws.mannequin.camera.zoom()
    );
    assert!(
        seen(&ws, under).distance(pointer) < 0.5,
        "the point stays under the pointer"
    );
    assert_eq!(view(&ws).top(), top, "the panel did not scroll");

    let zoom = ws.mannequin.camera.zoom();
    steps(
        &mut ws,
        &ctx,
        &mut time,
        vec![vec![Event::Zoom(1.5)], vec![]],
    );
    assert!(
        (ws.mannequin.camera.zoom() - zoom * 1.5).abs() < 1e-3,
        "a pinch zooms"
    );
}

#[test]
fn a_right_drag_slides_the_view_and_a_left_drag_turns_it_round_the_figure() {
    let mut ws = workspace();
    let (ctx, mut time) = laid_out(&mut ws);
    let from = empty(&ws);
    let focus = ws.mannequin.camera.focus();
    let before = seen(&ws, focus);
    drag(
        &mut ws,
        &ctx,
        &mut time,
        from,
        from + Vec2::new(40.0, 25.0),
        PointerButton::Secondary,
    );
    let moved = seen(&ws, focus) - before;
    assert!((moved - Vec2::new(40.0, 25.0)).length() < 2.0, "{moved:?}");

    let yaw = ws.mannequin().view.yaw();
    drag(
        &mut ws,
        &ctx,
        &mut time,
        from,
        from + Vec2::new(60.0, 0.0),
        PointerButton::Primary,
    );
    assert!(ws.mannequin().view.yaw() > yaw + 10.0, "the view turned");
    assert_eq!(
        ws.mannequin().pose,
        flare_mannequin::preset("stand").expect("stand")
    );
}

#[test]
fn a_double_click_turns_the_view_round_a_part_or_frames_the_figure() {
    let mut ws = workspace();
    let (ctx, mut time) = laid_out(&mut ws);
    let head = ws.mannequin().posed().at(Joint::Head);
    let on_head = seen(&ws, head) + Vec2::new(0.0, -8.0);
    double_click(&mut ws, &ctx, &mut time, on_head);
    assert_eq!(ws.mannequin.camera.focus(), head, "it looks at the head");

    ws.mannequin.camera.set_zoom(8.0);
    // Long enough after, so this is not a third click.
    time += 2.0;
    let corner = empty(&ws);
    double_click(&mut ws, &ctx, &mut time, corner);
    let zoom = ws.mannequin.camera.zoom();
    assert!(zoom < 2.0 && zoom > 0.5, "framed: {zoom}");
}

#[test]
fn the_view_buttons_zoom_frame_and_turn_it_to_a_side() {
    let mut ws = workspace();
    ws.zoom_mannequin(2.0);
    assert!((ws.mannequin.camera.zoom() - 2.0).abs() < 1e-9);
    ws.face_mannequin(Facing::Left);
    assert_eq!(ws.mannequin().view.yaw(), -90.0);
    ws.face_mannequin(Facing::Top);
    assert_eq!(ws.mannequin().view.pitch(), flare_mannequin::MAX_PITCH);
    ws.mannequin.figure.pose.set_lift(Vec3::new(1.5, 0.0, 0.0));
    ws.frame_mannequin();
    assert!(
        ws.mannequin.camera.focus().x > 1.0,
        "it looks where the figure went"
    );
    ws.reset_mannequin_view();
    assert_eq!(ws.mannequin.camera, Camera::default());
}

/// A picture of the mannequin itself waving, drawn from the front.
fn waving() -> Vec<u8> {
    let figure = flare_mannequin::Mannequin {
        pose: flare_mannequin::preset("wave").expect("wave"),
        view: flare_mannequin::View::new(0.0, 0.0, 0.0),
        ..flare_mannequin::Mannequin::default()
    };
    let placement = flare_mannequin::Placement::new([240.0, 320.0], 560.0).expect("placement");
    let drawn = figure
        .render(placement, 480, 640, flare_mannequin::Look::default())
        .expect("drawn");
    flare_runtime::rgba_png(480, 640, drawn.rgba()).expect("png")
}

#[test]
fn the_pose_of_the_person_in_a_picture_is_found_and_copied_by_itself() {
    let mut ws = workspace();
    ws.import_mannequin_reference_bytes("wave.png", waving());
    let figure = ws.mannequin();
    let posed = figure.posed();
    assert!(
        posed.at(Joint::RightWrist).y > posed.at(Joint::RightShoulder).y,
        "the right hand is raised"
    );
    assert_eq!(figure.view.yaw(), 0.0, "seen as the picture is");
    let reference = ws.mannequin.reference.as_ref().expect("picture");
    let wrist = reference.marks.get(Joint::RightWrist).expect("found");
    assert!(
        wrist[1] < 200.0,
        "the mark is on the raised hand: {wrist:?}"
    );
    assert!(
        ws.log
            .iter()
            .any(|l| l.contains("wave.png") && l.contains("copied")),
        "{:?}",
        ws.log
    );
}

#[test]
fn a_picture_brought_in_stands_behind_the_figure_with_marks_on_it() {
    let mut ws = workspace();
    ws.ask_mannequin_reference();
    assert_eq!(
        ws.take_dialog(),
        Some(DialogRequest::ImportMannequinReference)
    );
    let (ws, _, _) = with_picture();
    let reference = ws.mannequin.reference.as_ref().expect("picture");
    assert_eq!(reference.board.size(), [300.0, 400.0]);
    assert!(reference.texture.is_some(), "the view made its texture");
    assert!(MARKED.iter().all(|j| reference.marks.get(*j).is_some()));
    assert_eq!(ws.mannequin().view.yaw(), 0.0, "seen from the front");
    assert_eq!(ws.mannequin().view.pitch(), 0.0);
    assert!(
        ws.log
            .iter()
            .any(|l| l.contains("person.png") && l.contains("No person")),
        "a blank picture has no one in it: {:?}",
        ws.log
    );
}

#[test]
fn a_mark_is_dragged_onto_the_picture_and_a_right_click_says_its_depth() {
    let (mut ws, ctx, mut time) = with_picture();
    let from = mark_seen(&ws, Joint::LeftWrist);
    let to = from + Vec2::new(30.0, -40.0);
    drag(&mut ws, &ctx, &mut time, from, to, PointerButton::Primary);
    assert!(mark_seen(&ws, Joint::LeftWrist).distance(to) < 1.0);
    assert_eq!(
        ws.mannequin().pose,
        flare_mannequin::preset("stand").expect("stand"),
        "the figure under the mark is not posed"
    );
    let at = mark_seen(&ws, Joint::LeftWrist);
    let secondary = PointerButton::Secondary;
    steps(
        &mut ws,
        &ctx,
        &mut time,
        vec![
            vec![Event::PointerMoved(at)],
            vec![press(at, secondary, true)],
            vec![press(at, secondary, false)],
            vec![],
        ],
    );
    assert_eq!(ws.mannequin_mark_depth(Joint::LeftWrist), Depth::Nearer);
    assert!(!ws.set_mannequin_mark(Joint::Pelvis, Some([1.0, 1.0])));
}

#[test]
fn copy_pose_poses_the_figure_from_the_marks_and_stands_the_picture_behind_it() {
    let (mut ws, _, _) = with_picture();
    let marks = ws.mannequin.reference.as_ref().expect("picture").marks;
    let shoulder = marks.get(Joint::LeftShoulder).expect("shoulder");
    let wrist = marks.get(Joint::LeftWrist).expect("wrist");
    let reach = (wrist[1] - shoulder[1]).abs();
    ws.set_mannequin_mark(
        Joint::LeftElbow,
        Some([shoulder[0] + reach * 0.53, shoulder[1]]),
    );
    ws.set_mannequin_mark(
        Joint::LeftWrist,
        Some([shoulder[0] + reach * 1.06, shoulder[1]]),
    );
    let board = ws.mannequin.reference.as_ref().expect("picture").board;
    assert!(ws.copy_mannequin_pose());
    let side = ws.mannequin().pose.angles(Joint::LeftShoulder).side;
    assert!((side - 90.0).abs() < 3.0, "the arm is out: {side}");
    assert_ne!(
        ws.mannequin.reference.as_ref().expect("picture").board,
        board,
        "moved to fit"
    );

    ws.reset_mannequin_marks();
    let reset = ws.mannequin.reference.as_ref().expect("picture").marks;
    assert_ne!(reset.get(Joint::LeftKnee), None);

    for joint in [Joint::Neck, Joint::LeftHip] {
        ws.set_mannequin_mark(joint, None);
    }
    let pose = ws.mannequin().pose;
    assert!(!ws.copy_mannequin_pose(), "too few marks");
    assert_eq!(ws.mannequin().pose, pose, "nothing changed");
    assert!(
        ws.log.iter().any(|l| l.contains("hips and the neck")),
        "{:?}",
        ws.log
    );
    ws.remove_mannequin_reference();
    assert!(ws.mannequin.reference.is_none());
    assert!(!ws.copy_mannequin_pose());
}

#[test]
fn a_picture_that_does_not_read_is_reported_and_brings_nothing_in() {
    let mut ws = workspace();
    ws.import_mannequin_reference_bytes("person.png", b"not a picture".to_vec());
    assert!(ws.mannequin.reference.is_none());
    assert!(
        ws.log.iter().any(|l| l.contains("person.png")),
        "{:?}",
        ws.log
    );
    ws.import_mannequin_reference(std::path::Path::new("/no/such/picture.png"));
    assert!(ws.mannequin.reference.is_none());
}

#[test]
fn a_picture_read_on_another_thread_is_taken_when_it_is_ready() {
    let mut ws = workspace();
    let (send, answer) = std::sync::mpsc::channel();
    ws.mannequin.loading = Some(Loading {
        name: "late.png".into(),
        answer,
    });
    assert!(ws.poll_mannequin_reference(), "still reading");
    let picked = read("late.png", &picture(20, 30)).expect("reads");
    send.send(Ok(picked)).expect("sent");
    assert!(!ws.poll_mannequin_reference());
    assert!(ws.mannequin.loading.is_none());
    assert!(ws.mannequin.reference.is_some());

    let (send, answer) = std::sync::mpsc::channel::<Result<Picked, String>>();
    ws.mannequin.loading = Some(Loading {
        name: "lost.png".into(),
        answer,
    });
    drop(send);
    assert!(!ws.poll_mannequin_reference());
    assert!(
        ws.log.iter().any(|l| l.contains("lost.png")),
        "{:?}",
        ws.log
    );
}
