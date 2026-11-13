use crate::{
    Angles, Body, Joint, Mannequin, Request, View, WireError, catalog, parse_joint_setting, preset,
};

#[test]
fn each_part_of_a_request_is_laid_over_the_one_before() {
    let mut start = Mannequin::new(Body::Female);
    start.view = View::new(10.0, 0.0, 0.0);
    let request = Request {
        figure: Some(start.to_json()),
        preset: Some("walk".into()),
        joints: vec![("left_elbow".into(), [-90.0, 0.0, 0.0])],
        yaw: Some(-45.0),
        ..Request::default()
    };
    let figure = request.build().expect("builds");
    assert_eq!(figure.body, Body::Female, "from the figure file");
    assert_eq!(figure.view.yaw(), -45.0, "the turn asked for");
    assert_eq!(figure.view.pitch(), 0.0, "the figure file's tilt");
    let mut walk = preset("walk").expect("walk");
    walk.set(Joint::LeftElbow, Angles::new(-90.0, 0.0, 0.0));
    assert_eq!(figure.pose, walk);

    let posed = Request {
        pose: Some(r#"{"joints": {"neck": [20, 0, 0]}}"#.into()),
        body: Some("male".into()),
        ..Request::default()
    }
    .build()
    .expect("builds");
    assert_eq!(posed.pose.angles(Joint::Neck), Angles::new(20.0, 0.0, 0.0));
    assert_eq!(Request::default().build(), Ok(Mannequin::default()));
}

#[test]
fn a_request_with_a_wrong_name_or_number_is_refused() {
    let wrong = [
        Request {
            body: Some("robot".into()),
            ..Request::default()
        },
        Request {
            preset: Some("moonwalk".into()),
            ..Request::default()
        },
        Request {
            joints: vec![("tail".into(), [0.0; 3])],
            ..Request::default()
        },
        Request {
            joints: vec![("left_knee".into(), [-20.0, 0.0, 0.0])],
            ..Request::default()
        },
        Request {
            pitch: Some(120.0),
            ..Request::default()
        },
        Request {
            fov: Some(f64::NAN),
            ..Request::default()
        },
        Request {
            joints: vec![("neck".into(), [0.0; 3]); 65],
            ..Request::default()
        },
    ];
    for request in wrong {
        assert!(request.build().is_err(), "{request:?}");
    }
    let moonwalk = Request {
        preset: Some("moonwalk".into()),
        ..Request::default()
    };
    assert_eq!(
        moonwalk.build(),
        Err(WireError::UnknownPreset("moonwalk".into()))
    );
}

#[test]
fn a_joint_setting_reads_from_the_command_line_form() {
    assert_eq!(
        parse_joint_setting("left_knee=45, 0,-2.5"),
        Ok(("left_knee".into(), [45.0, 0.0, -2.5]))
    );
    assert!(parse_joint_setting("left_knee").is_err());
    assert!(parse_joint_setting("left_knee=1,2").is_err());
    assert!(parse_joint_setting("left_knee=a,b,c").is_err());
}

#[test]
fn the_catalog_lists_every_body_pose_proportion_and_joint() {
    let c = catalog();
    assert_eq!(c["bodies"].as_array().map(Vec::len), Some(2));
    assert_eq!(c["joints"].as_array().map(Vec::len), Some(17));
    assert_eq!(c["proportions"].as_array().map(Vec::len), Some(7));
    assert!(
        c["poses"]
            .as_array()
            .is_some_and(|p| p.iter().any(|id| id == "sit"))
    );
    assert_eq!(c["joints"][6]["id"], "left_elbow");
    assert_eq!(c["joints"][6]["bend"], serde_json::json!([-150.0, 0.0]));
}

#[test]
fn a_reach_setting_reads_from_the_command_line_form() {
    assert_eq!(
        crate::parse_reach_setting("left_wrist=0.2,-0.1,0.15"),
        Ok(("left_wrist".to_owned(), [0.2, -0.1, 0.15]))
    );
    let wrong = crate::parse_reach_setting("left_wrist").expect_err("no point");
    assert!(wrong.contains("end=x,y,z"), "{wrong}");
}
