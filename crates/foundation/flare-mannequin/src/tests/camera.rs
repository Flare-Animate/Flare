use super::close;
use crate::{
    Camera, Facing, Joint, MAX_FOCUS, MAX_PITCH, Mannequin, Placement, Vec3, View, ZOOM_RANGE,
};

const CENTER: [f64; 2] = [300.0, 400.0];
const HEIGHT: f64 = 640.0;

fn seen(camera: &Camera, figure: &Mannequin, p: Vec3) -> [f64; 2] {
    let placement = camera.placement(CENTER, HEIGHT).expect("placement");
    figure.projector(placement).project(p).at
}

fn near(a: [f64; 2], b: [f64; 2], tolerance: f64) -> bool {
    close(a[0], b[0], tolerance) && close(a[1], b[1], tolerance)
}

#[test]
fn a_zoom_keeps_the_point_under_the_pointer_where_it_is() {
    let figure = Mannequin::default();
    let mut camera = Camera::default();
    let at = [380.0, 250.0];
    let placement = camera.placement(CENTER, HEIGHT).expect("placement");
    let under = figure.projector(placement).unproject(at, 0.0);
    camera.zoom_about(&figure.view, CENTER, HEIGHT, at, 2.5);
    assert!(close(camera.zoom(), 2.5, 1e-12));
    assert!(near(seen(&camera, &figure, under), at, 1e-6));
    // And back out again.
    camera.zoom_about(&figure.view, CENTER, HEIGHT, at, 0.5);
    assert!(near(seen(&camera, &figure, under), at, 1e-6));
}

#[test]
fn a_zoom_is_held_to_its_range_and_a_wrong_number_changes_nothing() {
    let view = View::default();
    let mut camera = Camera::default();
    camera.zoom_about(&view, CENTER, HEIGHT, CENTER, 1e9);
    assert_eq!(camera.zoom(), ZOOM_RANGE.1);
    camera.zoom_about(&view, CENTER, HEIGHT, CENTER, 1e-9);
    assert_eq!(camera.zoom(), ZOOM_RANGE.0);
    let before = camera;
    for factor in [f64::NAN, f64::INFINITY, 0.0, -2.0] {
        camera.zoom_about(&view, CENTER, HEIGHT, CENTER, factor);
    }
    camera.zoom_about(&view, CENTER, HEIGHT, [f64::NAN, 0.0], 2.0);
    camera.pan(&view, CENTER, HEIGHT, [f64::INFINITY, 0.0]);
    assert_eq!(camera, before);
}

#[test]
fn a_pan_moves_what_is_seen_with_the_pointer() {
    let figure = Mannequin::default();
    let mut camera = Camera::default();
    let p = camera.focus();
    let before = seen(&camera, &figure, p);
    camera.pan(&figure.view, CENTER, HEIGHT, [30.0, -20.0]);
    let after = seen(&camera, &figure, p);
    assert!(near(after, [before[0] + 30.0, before[1] - 20.0], 1e-6));
    // A flat view moves every point the same, near or far.
    let flat = Mannequin {
        view: View::new(40.0, 20.0, 0.0),
        ..Mannequin::default()
    };
    let hand = flat.posed().at(Joint::LeftWrist);
    let before = seen(&camera, &flat, hand);
    camera.pan(&flat.view, CENTER, HEIGHT, [-12.0, 7.0]);
    assert!(near(
        seen(&camera, &flat, hand),
        [before[0] - 12.0, before[1] + 7.0],
        1e-6
    ));
}

#[test]
fn the_view_turns_round_what_the_camera_looks_at() {
    let mut figure = Mannequin::default();
    let mut camera = Camera::default();
    let hand = figure.posed().at(Joint::LeftWrist);
    camera.look_at(hand);
    for yaw in [0.0, 45.0, 120.0, -90.0] {
        figure.view = View::new(yaw, 30.0, 30.0);
        assert!(near(seen(&camera, &figure, hand), CENTER, 1e-9), "{yaw}");
    }
}

#[test]
fn the_camera_looks_only_so_far() {
    let mut camera = Camera::default();
    camera.look_at(Vec3::new(1e9, -1e9, 3.0));
    assert_eq!(camera.focus(), Vec3::new(MAX_FOCUS, -MAX_FOCUS, 3.0));
    camera.look_at(Vec3::new(f64::NAN, 0.0, 0.0));
    assert_eq!(camera.focus(), Vec3::new(MAX_FOCUS, -MAX_FOCUS, 3.0));
    let placement = Placement::new([0.0, 0.0], 10.0).expect("placement");
    assert!(placement.with_focus(Vec3::new(0.0, 5.0, 0.0)).is_none());
    assert!(
        placement
            .with_focus(Vec3::new(0.0, f64::NAN, 0.0))
            .is_none()
    );
    assert!(placement.with_focus(Vec3::new(1.0, 2.0, -3.0)).is_some());
}

#[test]
fn framing_brings_a_figure_moved_away_back_into_view() {
    let mut figure = Mannequin {
        view: View::new(30.0, 10.0, 30.0),
        ..Mannequin::default()
    };
    figure.pose.set_lift(Vec3::new(2.0, 0.5, -1.0));
    let size = [400.0, 500.0];
    let mut camera = Camera::default();
    camera.zoom_about(&figure.view, CENTER, HEIGHT, CENTER, 6.0);
    camera.frame(&figure, CENTER, HEIGHT, size);
    let placement = camera.placement(CENTER, HEIGHT).expect("placement");
    let (lo, hi) = figure.bounds(placement, false);
    let (left, top) = (CENTER[0] - size[0] / 2.0, CENTER[1] - size[1] / 2.0);
    assert!(lo[0] >= left && lo[1] >= top, "{lo:?}");
    assert!(hi[0] <= left + size[0] && hi[1] <= top + size[1], "{hi:?}");
    let fills = ((hi[0] - lo[0]) / size[0]).max((hi[1] - lo[1]) / size[1]);
    assert!(fills > 0.8, "it fills the view: {fills}");
    let middle = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
    assert!(near(middle, CENTER, 2.0), "{middle:?}");
}

#[test]
fn each_side_is_seen_square_on() {
    let figure = Mannequin::default();
    let posed = figure.posed();
    let depth = |view: View, joint: Joint| {
        let m = Mannequin { view, ..figure };
        let placement = Placement::new([0.0, 0.0], 100.0).expect("placement");
        m.projector(placement).project(posed.at(joint)).depth
    };
    let start = View::new(25.0, 8.0, 12.0);
    let left = start.facing(Facing::Left);
    assert!(depth(left, Joint::LeftShoulder) > depth(left, Joint::RightShoulder));
    let right = start.facing(Facing::Right);
    assert!(depth(right, Joint::RightShoulder) > depth(right, Joint::LeftShoulder));
    let back = start.facing(Facing::Back);
    let nose = posed.point(Joint::Head, Vec3::new(0.0, 0.05, 0.1));
    let m = Mannequin {
        view: back,
        ..figure
    };
    let placement = Placement::new([0.0, 0.0], 100.0).expect("placement");
    assert!(m.projector(placement).project(nose).depth < depth(back, Joint::Head));
    assert_eq!(start.facing(Facing::Top).pitch(), MAX_PITCH);
    assert_eq!(start.facing(Facing::Bottom).pitch(), -MAX_PITCH);
    assert_eq!(start.facing(Facing::Front).yaw(), 0.0);
    assert_eq!(
        start.facing(Facing::Front).fov(),
        12.0,
        "the perspective is kept"
    );
    for facing in Facing::ALL {
        assert_eq!(Facing::from_id(facing.id()), Some(facing));
    }
    assert_eq!(Facing::from_id("sideways"), None);
}
