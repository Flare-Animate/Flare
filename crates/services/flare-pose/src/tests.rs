//! The detector run on pictures of the mannequin itself, posed and drawn by `flare-mannequin`,
//! where it is known where every joint is.
#![allow(clippy::expect_used)]

use flare_mannequin::{Joint, Look, MARKED, Mannequin, Placement, Vec3, View, preset};

use crate::{Found, Pixels, PoseError, find_pose};

struct Drawn {
    rgba: Vec<u8>,
    size: [u32; 2],
    figure: Mannequin,
    placement: Placement,
}

/// `pose` drawn from the front, flat, the figure `tall` pixels tall in a picture `size`.
fn drawn(pose: &str, size: [u32; 2], tall: f64, at: [f64; 2]) -> Drawn {
    let figure = Mannequin {
        pose: preset(pose).expect("preset"),
        view: View::new(0.0, 0.0, 0.0),
        ..Mannequin::default()
    };
    let placement = Placement::new(at, tall).expect("placement");
    let picture = figure
        .render(placement, size[0], size[1], Look::default())
        .expect("drawn");
    Drawn {
        rgba: picture.rgba().to_vec(),
        size,
        figure,
        placement,
    }
}

fn find(d: &Drawn) -> Option<Found> {
    let picture = [f64::from(d.size[0]), f64::from(d.size[1])];
    find_pose(
        Pixels {
            rgba: &d.rgba,
            size: d.size,
        },
        picture,
    )
    .expect("runs")
}

/// Where `joint`'s mark belongs on the drawn picture.
fn truth(d: &Drawn, joint: Joint) -> [f64; 2] {
    let posed = d.figure.posed();
    let p = if joint == Joint::Head {
        posed.point(Joint::Head, Vec3::new(0.0, 0.06, 0.0))
    } else {
        posed.at(joint)
    };
    d.figure.projector(d.placement).project(p).at
}

/// The furthest a found mark is from where it belongs, as a share of the figure's height, and
/// how many marks were found.
fn off(d: &Drawn, found: &Found) -> (f64, usize) {
    let mut worst: f64 = 0.0;
    let mut count = 0;
    for joint in MARKED {
        if let Some(at) = found.marks.get(joint) {
            let want = truth(d, joint);
            worst = worst.max((at[0] - want[0]).hypot(at[1] - want[1]) / d.placement.height);
            count += 1;
        }
    }
    (worst, count)
}

#[test]
fn the_joints_of_a_figure_are_found_where_they_are() {
    for pose in ["stand", "t_pose", "wave", "a_pose", "walk"] {
        let d = drawn(pose, [480, 640], 560.0, [240.0, 320.0]);
        let found = find(&d).expect("someone is there");
        let (worst, count) = off(&d, &found);
        assert!(count >= 13, "{pose}: {count} marks");
        assert!(worst < 0.06, "{pose}: a mark is {worst} of the height off");
        assert!(found.confidence > 0.5, "{pose}: {}", found.confidence);
    }
}

#[test]
fn a_small_figure_in_a_large_picture_is_found_by_looking_closer() {
    let d = drawn("wave", [1200, 900], 220.0, [900.0, 600.0]);
    let found = find(&d).expect("someone is there");
    let (worst, count) = off(&d, &found);
    assert!(count >= 12, "{count} marks");
    assert!(worst < 0.08, "a mark is {worst} of the height off");
}

#[test]
fn the_marks_are_in_the_units_of_the_picture_the_pixels_were_made_from() {
    let d = drawn("stand", [240, 320], 280.0, [120.0, 160.0]);
    let pixels = Pixels {
        rgba: &d.rgba,
        size: d.size,
    };
    let small = find_pose(pixels, [240.0, 320.0])
        .expect("runs")
        .expect("found");
    let large = find_pose(pixels, [960.0, 1280.0])
        .expect("runs")
        .expect("found");
    for joint in MARKED {
        if let (Some(a), Some(b)) = (small.marks.get(joint), large.marks.get(joint)) {
            assert!((a[0] * 4.0 - b[0]).abs() < 1e-6 && (a[1] * 4.0 - b[1]).abs() < 1e-6);
        }
    }
}

#[test]
fn a_picture_with_no_one_in_it_finds_nothing() {
    for color in [[0u8, 0, 0, 255], [255, 255, 255, 255], [40, 120, 60, 255]] {
        let rgba: Vec<u8> = color.repeat(300 * 200);
        let found = find_pose(
            Pixels {
                rgba: &rgba,
                size: [300, 200],
            },
            [300.0, 200.0],
        )
        .expect("runs");
        assert!(found.is_none(), "{color:?}: {found:?}");
    }
}

#[test]
fn pixels_that_do_not_match_their_size_are_refused() {
    let rgba = vec![0u8; 10 * 10 * 4];
    let wrong = [
        (
            Pixels {
                rgba: &rgba,
                size: [11, 10],
            },
            [10.0, 10.0],
        ),
        (
            Pixels {
                rgba: &rgba,
                size: [0, 10],
            },
            [10.0, 10.0],
        ),
        (
            Pixels {
                rgba: &rgba,
                size: [10, 10],
            },
            [0.0, 10.0],
        ),
        (
            Pixels {
                rgba: &rgba,
                size: [10, 10],
            },
            [f64::NAN, 10.0],
        ),
    ];
    for (pixels, picture) in wrong {
        assert_eq!(find_pose(pixels, picture), Err(PoseError::Pixels));
    }
}

#[test]
fn a_pose_found_in_a_picture_is_copied_onto_the_figure() {
    // Waving: the right arm is raised high, the left hangs.
    let d = drawn("wave", [480, 640], 560.0, [240.0, 320.0]);
    let found = find(&d).expect("someone is there");
    let mut copy = Mannequin::default();
    copy.copy_pose(&found.marks).expect("copies");
    let posed = copy.posed();
    let (right, left) = (posed.at(Joint::RightWrist), posed.at(Joint::LeftWrist));
    assert!(
        right.y > posed.at(Joint::RightShoulder).y,
        "the right hand is above the shoulder"
    );
    assert!(
        left.y < posed.at(Joint::LeftHip).y + 0.05,
        "the left hand hangs by the hip"
    );
}
