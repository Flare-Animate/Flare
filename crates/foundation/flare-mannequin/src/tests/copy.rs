use super::close;
use crate::{
    Angles, Board, Depth, Joint, MARKED, Mannequin, Marks, MarksError, Request, Vec3, View,
    WireError, preset,
};

const PICTURE: [f64; 2] = [900.0, 1200.0];

fn board() -> Board {
    Board::new(PICTURE).expect("board")
}

fn posed(id: &str) -> Mannequin {
    Mannequin {
        pose: preset(id).expect("preset"),
        ..Mannequin::default()
    }
}

/// Where a mark's part is in the figure's space.
fn point(figure: &Mannequin, joint: Joint) -> Vec3 {
    let at = |j| figure.posed().at(j);
    if joint == Joint::Head {
        figure.posed().point(Joint::Head, Vec3::new(0.0, 0.06, 0.0))
    } else {
        at(joint)
    }
}

fn hips(figure: &Mannequin) -> Vec3 {
    (point(figure, Joint::LeftHip) + point(figure, Joint::RightHip)).scale(0.5)
}

/// The part each mark hangs from, for saying which way it leans.
fn parent(joint: Joint) -> Option<Joint> {
    use Joint::*;
    Some(match joint {
        Head => Neck,
        LeftHip => RightHip,
        LeftShoulder => RightShoulder,
        LeftElbow => LeftShoulder,
        LeftWrist => LeftElbow,
        RightElbow => RightShoulder,
        RightWrist => RightElbow,
        LeftKnee => LeftHip,
        LeftAnkle => LeftKnee,
        RightKnee => RightHip,
        RightAnkle => RightKnee,
        _ => return None,
    })
}

/// Marks of `truth` seen from the front, with each one's lean told as the truth has it.
fn told(truth: &Mannequin) -> Marks {
    let mut marks = Marks::from_figure(truth, &board());
    for joint in MARKED {
        let from = match (joint, parent(joint)) {
            (Joint::Neck, _) => hips(truth).z,
            (_, Some(p)) => point(truth, p).z,
            (_, None) => continue,
        };
        let z = point(truth, joint).z;
        let depth = if z > from + 1e-4 {
            Depth::Nearer
        } else if z < from - 1e-4 {
            Depth::Farther
        } else {
            Depth::Auto
        };
        marks.set_depth(joint, depth);
    }
    marks
}

/// The furthest a marked part of `a` is from the same part of `b`, the two hips put together.
fn furthest(a: &Mannequin, b: &Mannequin) -> f64 {
    let shift = hips(b) - hips(a);
    MARKED
        .iter()
        .map(|j| (point(a, *j) + shift - point(b, *j)).length())
        .fold(0.0, f64::max)
}

#[test]
fn a_pose_seen_from_the_front_is_copied_back_into_depth() {
    for id in ["stand", "t_pose", "a_pose", "walk", "wave", "kneel", "sit"] {
        let truth = posed(id);
        let mut copy = Mannequin::default();
        copy.copy_pose(&told(&truth)).expect("copies");
        let off = furthest(&copy, &truth);
        assert!(off < 0.03, "{id}: a part is {off} from where it was");
    }
}

#[test]
fn which_way_a_part_leans_is_worked_out_from_how_a_body_bends() {
    // A forearm held level from an arm that hangs, and a shin held level from a thigh that
    // hangs, are seen end on from the front; an elbow bends only forward and a knee only back,
    // which settles their depth. (A thigh raised forward over a bent knee could be either: the
    // marks' depth says which.)
    let mut truth = Mannequin::default();
    truth.pose.set(Joint::LeftKnee, Angles::new(90.0, 0.0, 0.0));
    truth
        .pose
        .set(Joint::RightElbow, Angles::new(-90.0, 0.0, 0.0));
    let marks = Marks::from_figure(&truth, &board());
    let mut copy = Mannequin::default();
    copy.copy_pose(&marks).expect("copies");
    let off = furthest(&copy, &truth);
    assert!(off < 0.03, "a part is {off} from where it was");
}

#[test]
fn a_part_told_to_lean_toward_the_viewer_does() {
    let truth = Mannequin::default();
    let mut marks = Marks::from_figure(&truth, &board());
    // The left hand raised level with the shoulder, but drawn short: it points at the viewer or
    // away from them.
    let shoulder = marks.get(Joint::LeftShoulder).expect("shoulder");
    marks.set(Joint::LeftElbow, Some([shoulder[0] + 30.0, shoulder[1]]));
    marks.set(Joint::LeftWrist, Some([shoulder[0] + 60.0, shoulder[1]]));
    for (depth, toward) in [(Depth::Nearer, true), (Depth::Farther, false)] {
        marks.set_depth(Joint::LeftElbow, depth);
        marks.set_depth(Joint::LeftWrist, depth);
        let mut copy = Mannequin::default();
        copy.copy_pose(&marks).expect("copies");
        let posed = copy.posed();
        let ahead = posed.at(Joint::LeftWrist).z > posed.at(Joint::LeftShoulder).z;
        assert_eq!(ahead, toward, "{depth:?}");
    }
}

#[test]
fn the_copied_figure_lies_on_its_marks_seen_from_the_front() {
    let truth = posed("walk");
    let mut marks = told(&truth);
    // The person stands smaller and elsewhere on the picture.
    for joint in MARKED {
        let at = marks.get(joint).expect("marked");
        marks.set(joint, Some([at[0] * 0.4 + 120.0, at[1] * 0.4 + 300.0]));
    }
    let mut copy = Mannequin {
        view: View::new(0.0, 0.0, 0.0),
        ..Mannequin::default()
    };
    let fit = copy.copy_pose(&marks).expect("copies");
    let fitted = Board::fitted(PICTURE, &fit).expect("fits");
    let again = Marks::from_figure(&copy, &fitted);
    for joint in MARKED {
        let (a, b) = (again.get(joint).expect("a"), marks.get(joint).expect("b"));
        let off = (a[0] - b[0]).hypot(a[1] - b[1]);
        // Within a percent and a half of the figure's height in the picture.
        assert!(off < 0.015 * fit.scale, "{joint:?} is {off} off");
    }
}

#[test]
fn the_same_marks_at_any_size_copy_the_same_pose() {
    let marks = told(&posed("wave"));
    let mut larger = marks;
    for joint in MARKED {
        let at = marks.get(joint).expect("marked");
        larger.set(joint, Some([at[0] * 3.0 - 50.0, at[1] * 3.0 + 10.0]));
    }
    let (mut a, mut b) = (Mannequin::default(), Mannequin::default());
    a.copy_pose(&marks).expect("copies");
    b.copy_pose(&larger).expect("copies");
    assert!(furthest(&a, &b) < 1e-6);
}

#[test]
fn a_copied_crouch_stands_on_the_floor() {
    let mut truth = Mannequin::default();
    for (hip, knee) in [
        (Joint::LeftHip, Joint::LeftKnee),
        (Joint::RightHip, Joint::RightKnee),
    ] {
        truth.pose.set(hip, Angles::new(-80.0, 0.0, 0.0));
        truth.pose.set(knee, Angles::new(110.0, 0.0, 0.0));
    }
    let mut copy = Mannequin::default();
    copy.copy_pose(&told(&truth)).expect("copies");
    let posed = copy.posed();
    let lowest = posed
        .at(Joint::LeftAnkle)
        .y
        .min(posed.at(Joint::RightAnkle).y);
    assert!(close(lowest, 0.04, 1e-9), "{lowest}");
    assert!(copy.pose.lift().y < -0.1, "the body came down");
}

#[test]
fn parts_without_marks_keep_their_pose() {
    let mut start = Mannequin::default();
    start
        .pose
        .set(Joint::RightElbow, Angles::new(-60.0, 10.0, 0.0));
    let mut marks = Marks::from_figure(&posed("t_pose"), &board());
    for joint in [Joint::RightElbow, Joint::RightWrist, Joint::RightShoulder] {
        marks.set(joint, None);
    }
    let mut copy = start;
    copy.copy_pose(&marks).expect("copies");
    assert_eq!(
        copy.pose.angles(Joint::RightElbow),
        start.pose.angles(Joint::RightElbow)
    );
    let left = copy.pose.angles(Joint::LeftShoulder);
    assert!(
        close(left.side, 90.0, 2.0),
        "the marked arm is out: {left:?}"
    );
}

#[test]
fn too_few_marks_copy_nothing() {
    let full = Marks::from_figure(&Mannequin::default(), &board());
    for missing in [Joint::Neck, Joint::LeftHip, Joint::RightHip] {
        let mut marks = full;
        marks.set(missing, None);
        let mut figure = posed("wave");
        assert_eq!(figure.copy_pose(&marks), Err(MarksError::TooFew));
        assert_eq!(figure, posed("wave"), "unchanged");
    }
    let mut heap = Marks::default();
    for joint in MARKED {
        heap.set(joint, Some([10.0, 10.0]));
    }
    assert_eq!(
        Mannequin::default().copy_pose(&heap),
        Err(MarksError::TooFew)
    );
}

#[test]
fn a_mark_is_put_only_on_a_marked_part_and_at_a_number() {
    let mut marks = Marks::default();
    assert!(!marks.set(Joint::Pelvis, Some([1.0, 2.0])));
    assert!(!marks.set(Joint::Head, Some([f64::NAN, 2.0])));
    assert!(!marks.set(Joint::Head, Some([1e7, 2.0])));
    assert!(marks.set(Joint::Head, Some([1.0, 2.0])));
    assert_eq!(marks.get(Joint::Head), Some([1.0, 2.0]));
    marks.set_depth(Joint::Spine, Depth::Nearer);
    assert_eq!(marks.depth(Joint::Spine), Depth::Auto);
    assert_eq!(marks.nearest([3.0, 2.0], 2.5), Some(Joint::Head));
    assert_eq!(marks.nearest([9.0, 2.0], 2.5), None);
    assert_eq!(Depth::Auto.next().next().next(), Depth::Auto);
}

#[test]
fn marks_read_back_as_written_and_a_wrong_file_is_refused() {
    let mut marks = Marks::from_figure(&posed("walk"), &board());
    marks.set_depth(Joint::LeftWrist, Depth::Farther);
    marks.set(Joint::RightAnkle, None);
    let read = Marks::from_json(&marks.to_json()).expect("reads");
    for joint in MARKED {
        assert_eq!(read.depth(joint), marks.depth(joint));
        match (read.get(joint), marks.get(joint)) {
            (Some(a), Some(b)) => assert!(close(a[0], b[0], 0.01) && close(a[1], b[1], 0.01)),
            (a, b) => assert_eq!(a, b),
        }
    }
    let wrong = [
        r#"{"marks": {"pelvis": [1, 2]}}"#,
        r#"{"marks": {"elbow": [1, 2]}}"#,
        r#"{"marks": {"head": [1e9, 2]}}"#,
        r#"{"marks": {"head": [1, 2]}, "depth": {"head": "sideways"}}"#,
        r#"{"marks": {"head": [1, 2]}, "extra": 1}"#,
        r#"{"marks": {"head": [1]}}"#,
    ];
    for text in wrong {
        assert!(Marks::from_json(text).is_err(), "{text}");
    }
    let huge = format!(r#"{{"marks": {{}}, "pad": "{}"}}"#, " ".repeat(9000));
    assert_eq!(Marks::from_json(&huge), Err(MarksError::TooLarge));
}

#[test]
fn the_point_of_the_picture_under_the_pointer_is_found_from_any_side_it_faces() {
    let board = board();
    let figure = Mannequin {
        view: View::new(20.0, 15.0, 30.0),
        ..Mannequin::default()
    };
    let placement = crate::Placement::new([400.0, 300.0], 500.0).expect("placement");
    let projector = figure.projector(placement);
    let at = [250.0, 700.0];
    let seen = projector.project(board.point(at)).at;
    let found = board.under(&projector, seen).expect("seen");
    assert!(close(found[0], at[0], 1e-6) && close(found[1], at[1], 1e-6));
    let edge_on = Mannequin {
        view: View::new(90.0, 0.0, 0.0),
        ..figure
    };
    assert_eq!(board.under(&edge_on.projector(placement), seen), None);
    // Looking ahead from a point past the picture, with a wide lens: it is behind the camera.
    let past = placement
        .with_focus(crate::Vec3::new(0.0, 0.5, -2.0))
        .expect("focus");
    let wide = Mannequin {
        view: View::new(0.0, 0.0, 90.0),
        ..figure
    };
    for at in [[400.0, 300.0], [250.0, 700.0], [10.0, 10.0]] {
        assert_eq!(board.under(&wide.projector(past), at), None, "{at:?}");
    }
    assert!(Board::new([0.0, 10.0]).is_none());
    assert!(Board::new([f64::NAN, 10.0]).is_none());
}

#[test]
fn a_request_copies_a_pose_from_marks_and_turns_to_a_side() {
    let truth = posed("wave");
    let marks = told(&truth).to_json();
    let figure = Request {
        marks: Some(marks.clone()),
        facing: Some("front".into()),
        ..Request::default()
    }
    .build()
    .expect("builds");
    assert!(furthest(&figure, &truth) < 0.03);
    assert_eq!(figure.view.yaw(), 0.0);
    assert_eq!(figure.view.pitch(), 0.0);
    let wrong = Request {
        facing: Some("sideways".into()),
        ..Request::default()
    };
    assert_eq!(
        wrong.build(),
        Err(WireError::UnknownFacing("sideways".into()))
    );
    let few = Request {
        marks: Some(r#"{"marks": {"head": [1, 2]}}"#.into()),
        ..Request::default()
    };
    assert_eq!(few.build(), Err(WireError::Marks(MarksError::TooFew)));
}

#[test]
fn a_part_that_only_looks_a_little_short_stays_upright_and_a_much_shorter_one_leans() {
    // A person a little shorter in the body than the figure, standing straight: the neck is
    // marked a little low.
    let mut marks = Marks::from_figure(&Mannequin::default(), &board());
    let neck = marks.get(Joint::Neck).expect("neck");
    let hips = marks.get(Joint::LeftHip).expect("hip");
    marks.set(
        Joint::Neck,
        Some([neck[0], neck[1] + (hips[1] - neck[1]) * 0.08]),
    );
    let mut copy = Mannequin::default();
    copy.copy_pose(&marks).expect("copies");
    assert!(copy.pose.angles(Joint::Pelvis).bend.abs() < 1.0, "upright");
    // Told, the same mark leans exactly as far as it looks.
    marks.set_depth(Joint::Neck, Depth::Nearer);
    let mut told = Mannequin::default();
    told.copy_pose(&marks).expect("copies");
    assert!(
        told.pose.angles(Joint::Pelvis).bend > 15.0,
        "{:?}",
        told.pose.angles(Joint::Pelvis)
    );
    // A thigh drawn half as long as it is points well out of the picture.
    let mut marks = Marks::from_figure(&Mannequin::default(), &board());
    let (hip, knee) = (
        marks.get(Joint::LeftHip).expect("hip"),
        marks.get(Joint::LeftKnee).expect("knee"),
    );
    let half = [hip[0], hip[1] + (knee[1] - hip[1]) * 0.5];
    marks.set(Joint::LeftKnee, Some(half));
    marks.set(
        Joint::LeftAnkle,
        Some([half[0], half[1] + (knee[1] - hip[1])]),
    );
    let mut copy = Mannequin::default();
    copy.copy_pose(&marks).expect("copies");
    let thigh = copy.pose.angles(Joint::LeftHip).bend.abs();
    assert!(thigh > 40.0, "the thigh leans: {thigh}");
}
