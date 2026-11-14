use crate::wire::read_presets;
use crate::{
    Angles, Body, Joint, Mannequin, Pose, Proportion, Vec3, View, WireError, preset, presets,
};

#[test]
fn every_built_in_pose_reads_and_each_is_named_once() {
    let all = presets();
    assert!(all.len() >= 10, "{}", all.len());
    let mut ids: Vec<&str> = all.iter().map(|p| p.id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), all.len());
    assert!(
        ids.iter()
            .all(|id| id.chars().all(|c| c.is_ascii_lowercase() || c == '_'))
    );
    // Read directly too, so a mistake in the file fails here rather than giving no presets.
    assert_eq!(
        read_presets(include_str!("../../data/poses.json"))
            .expect("reads")
            .len(),
        all.len()
    );
    assert!(preset("walk").is_some_and(|p| p != Pose::default()));
    assert!(preset("moonwalk").is_none());
}

#[test]
fn a_figure_comes_back_from_its_json() {
    let mut m = Mannequin::new(Body::Female);
    m.pose = preset("run").expect("run");
    m.pose.set(Joint::Head, Angles::new(12.5, -3.25, 4.0));
    m.proportions.set(Proportion::Legs, 1.2).expect("in range");
    m.view = View::new(-40.0, 15.0, 45.0);
    let json = m.to_json();
    assert_eq!(Mannequin::from_json(&json).expect("reads"), m);
    let pose = Pose::from_json(&m.pose.to_json()).expect("reads");
    assert_eq!(pose, m.pose);
    assert!(
        !Pose::default().to_json().contains("lift"),
        "a rest pose is short"
    );
}

#[test]
fn what_a_figure_file_leaves_out_is_the_default() {
    let m = Mannequin::from_json(r#"{"version": 1}"#).expect("reads");
    assert_eq!(m, Mannequin::default());
    let m =
        Mannequin::from_json(r#"{"version": 1, "pose": {"joints": {"left_knee": [45, 0, 0]}}}"#)
            .expect("reads");
    assert_eq!(m.pose.angles(Joint::LeftKnee), Angles::new(45.0, 0.0, 0.0));
}

#[test]
fn bad_figure_files_are_refused_with_the_reason() {
    type Check = fn(&WireError) -> bool;
    let cases: [(&str, Check); 9] = [
        (r#"{"version": 2}"#, |e| matches!(e, WireError::Version(2))),
        ("[", |e| matches!(e, WireError::Json(_))),
        (r#"{"version": 1, "tail": 1}"#, |e| {
            matches!(e, WireError::Json(_))
        }),
        (r#"{"version": 1, "body": "robot"}"#, |e| {
            matches!(e, WireError::UnknownBody(_))
        }),
        (r#"{"version": 1, "proportions": {"ears": 1}}"#, |e| {
            matches!(e, WireError::UnknownProportion(_))
        }),
        (r#"{"version": 1, "proportions": {"legs": 9}}"#, |e| {
            matches!(e, WireError::Body(_))
        }),
        (
            r#"{"version": 1, "pose": {"joints": {"tail": [0, 0, 0]}}}"#,
            |e| matches!(e, WireError::UnknownJoint(_)),
        ),
        (
            r#"{"version": 1, "pose": {"joints": {"left_knee": [-10, 0, 0]}}}"#,
            |e| matches!(e, WireError::Pose(_)) && e.to_string().contains("left_knee"),
        ),
        (
            r#"{"version": 1, "view": {"yaw": 0, "pitch": 95, "fov": 0}}"#,
            |e| matches!(e, WireError::View),
        ),
    ];
    for (text, expected) in cases {
        let error = Mannequin::from_json(text).expect_err(text);
        assert!(expected(&error), "{text}: {error}");
    }
    let huge = format!(r#"{{"version": 1, "body": "{}"}}"#, "m".repeat(20_000));
    assert_eq!(Mannequin::from_json(&huge), Err(WireError::TooLarge));
    assert!(Pose::from_json(r#"{"lift": [0, 5, 0]}"#).is_err());
}

#[test]
fn a_pose_file_moves_the_figure_by_its_lift() {
    let pose = Pose::from_json(r#"{"lift": [0, -0.2, 0.1]}"#).expect("reads");
    assert_eq!(pose.lift(), Vec3::new(0.0, -0.2, 0.1));
}
