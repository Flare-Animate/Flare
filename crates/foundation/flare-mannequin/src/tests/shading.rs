use flare_core::Rgba8;

use super::front;
use crate::{Angles, Joint, Look, MAX_PICTURE_SIDE, Mannequin, Picture, Placement, Vec3, View};

fn placement() -> Placement {
    Placement::new([100.0, 120.0], 200.0).expect("placement")
}

fn picture(m: &Mannequin, look: Look) -> Picture {
    m.render(placement(), 200, 240, look).expect("picture")
}

fn facing() -> Mannequin {
    Mannequin {
        view: front(),
        ..Mannequin::default()
    }
}

fn pixel(p: &Picture, at: [f64; 2]) -> [u8; 4] {
    let i = (at[1] as usize * p.width() as usize + at[0] as usize) * 4;
    [
        p.rgba()[i],
        p.rgba()[i + 1],
        p.rgba()[i + 2],
        p.rgba()[i + 3],
    ]
}

fn light(p: &Picture, at: [f64; 2]) -> u32 {
    pixel(p, at)[..3].iter().map(|c| u32::from(*c)).sum()
}

/// Where a point of the head lands in the picture.
fn on_head(m: &Mannequin, local: Vec3) -> [f64; 2] {
    let p = m.posed().point(Joint::Head, local);
    m.projector(placement()).project(p).at
}

#[test]
fn the_picture_is_the_size_asked_and_see_through_round_the_figure() {
    let m = facing();
    let p = picture(
        &m,
        Look {
            shadow: false,
            ..Look::default()
        },
    );
    assert_eq!((p.width(), p.height()), (200, 240));
    assert_eq!(p.rgba().len(), 200 * 240 * 4);
    assert_eq!(pixel(&p, [2.0, 2.0])[3], 0);
    let chest = m.marks(placement())[Joint::Chest.index()].at;
    assert_eq!(pixel(&p, chest)[3], 255);
    assert!(
        p.rgba().chunks(4).any(|c| c[3] > 0 && c[3] < 255),
        "the figure's edge is smoothed"
    );
}

#[test]
fn a_picture_with_no_size_or_too_large_a_one_is_not_made() {
    let m = facing();
    assert!(m.render(placement(), 0, 10, Look::default()).is_none());
    assert!(
        m.render(placement(), 10, MAX_PICTURE_SIDE + 1, Look::default())
            .is_none()
    );
    // A figure wholly outside the picture is a picture of nothing.
    let away = Placement::new([-5000.0, 0.0], 100.0).expect("placement");
    let p = m.render(away, 20, 20, Look::default()).expect("picture");
    assert!(p.rgba().iter().all(|c| *c == 0));
}

#[test]
fn the_shadow_falls_on_the_floor_seen_from_above_and_not_from_below() {
    let mut m = facing();
    m.view = View::new(0.0, 30.0, 0.0);
    let p = picture(&m, Look::default());
    let foot = m.marks(placement())[Joint::LeftAnkle.index()].at;
    let (_, hi) = m.bounds(placement(), true);
    let (_, body) = m.bounds(placement(), false);
    // The light is in front of the figure and to its right, so the shadow falls behind it and
    // out to its left: up and to the right in the picture.
    assert!(hi[0] > body[0] + 2.0, "the shadow reaches past the figure");
    let shade = (foot[0] as usize..p.width() as usize)
        .map(|x| pixel(&p, [x as f64, foot[1] - 4.0]))
        .find(|c| c[3] > 0 && c[3] < 120);
    assert!(
        shade.is_some_and(|c| c[0] < 40),
        "a see-through dark shadow"
    );
    m.view = View::new(0.0, -20.0, 0.0);
    let p = picture(&m, Look::default());
    assert!(
        p.rgba()
            .chunks(4)
            .all(|c| c[3] == 0 || c[3] > 40 || c[0] > 40)
    );
}

#[test]
fn a_press_finds_the_part_that_is_seen_there_and_the_nearer_part_hides_the_further() {
    let mut m = facing();
    let p = picture(&m, Look::default());
    let marks = m.marks(placement());
    let knee = marks[Joint::LeftKnee.index()].at;
    assert_eq!(p.joint_at([knee[0], knee[1] + 12.0]), Some(Joint::LeftKnee));
    assert_eq!(p.joint_at([2.0, 2.0]), None);
    assert_eq!(p.joint_at([-1.0, 5.0]), None);
    assert_eq!(p.joint_at([f64::NAN, 5.0]), None);
    assert_eq!(p.joint_at([500.0, 5.0]), None);
    let chest = marks[Joint::Chest.index()].at;
    let over = [chest[0], chest[1] - 6.0];
    assert_eq!(p.joint_at(over), Some(Joint::Chest));
    // A hand brought in front of the chest is what is seen there.
    assert!(m.reach(Joint::LeftWrist, placement(), over));
    let wrist = m.posed().at(Joint::LeftWrist);
    assert!(m.reach_point(Joint::LeftWrist, Vec3::new(wrist.x, wrist.y, 0.16)));
    let p = picture(&m, Look::default());
    let seen = p.joint_at(m.marks(placement())[Joint::LeftWrist.index()].at);
    assert!(
        matches!(seen, Some(Joint::LeftWrist | Joint::LeftElbow)),
        "{seen:?}"
    );
}

#[test]
fn the_figure_is_lit_from_the_upper_left_and_lined_down_its_middle() {
    let mut m = facing();
    let p = picture(&m, Look::default());
    let chest = m.marks(placement())[Joint::Chest.index()].at;
    let (left, right) = (
        [chest[0] - 9.0, chest[1] - 8.0],
        [chest[0] + 9.0, chest[1] - 8.0],
    );
    assert!(
        light(&p, left) > light(&p, right) + 20,
        "{} then {}",
        light(&p, left),
        light(&p, right)
    );
    // The orange line down the middle of the face shows from the front, and from behind the
    // one down the back of the head shows in the same place.
    let face = on_head(&m, Vec3::new(0.0, 0.085, 0.06));
    let orange = |c: [u8; 4]| c[0] > c[2] + 40;
    assert!(orange(pixel(&p, face)), "{:?}", pixel(&p, face));
    let plain = picture(
        &m,
        Look {
            guides: false,
            ..Look::default()
        },
    );
    assert!(!orange(pixel(&plain, face)), "no guides, no line");
    m.view = View::new(180.0, 0.0, 0.0);
    let behind = picture(&m, Look::default());
    assert!(orange(pixel(&behind, face)));
}

#[test]
fn the_picked_joints_parts_are_tinted_and_no_others() {
    let m = facing();
    let bare = Look {
        guides: false,
        ..Look::default()
    };
    let plain = picture(&m, bare);
    let tint = Rgba8::opaque(20, 90, 255);
    let picked = picture(
        &m,
        Look {
            picked: Some((Joint::LeftKnee, tint)),
            ..bare
        },
    );
    let marks = m.marks(placement());
    let knee = marks[Joint::LeftKnee.index()].at;
    let shin = [knee[0], knee[1] + 12.0];
    let (was, now) = (pixel(&plain, shin), pixel(&picked, shin));
    assert!(
        now[2] > now[0] + 30 && was[2] < was[0] + 30,
        "{was:?} {now:?}"
    );
    let chest = marks[Joint::Chest.index()].at;
    assert_eq!(pixel(&plain, chest), pixel(&picked, chest));
}

#[test]
fn any_pose_from_any_side_makes_a_picture() {
    for (i, preset) in crate::presets().iter().enumerate() {
        let mut m = Mannequin {
            pose: preset.pose,
            view: View::new(i as f64 * 47.0, i as f64 * 13.0 - 60.0, i as f64 * 9.0),
            ..Mannequin::default()
        };
        m.pose
            .set(Joint::Pelvis, Angles::new(40.0, i as f64 * 30.0, 10.0));
        let small = Placement::new([32.0, 40.0], 64.0).expect("placement");
        let p = m.render(small, 64, 80, Look::default()).expect("picture");
        assert!(p.rgba().chunks(4).any(|c| c[3] == 255), "{}", preset.id);
    }
}
