#![allow(clippy::expect_used)]
//! BVH reader and writer tests: a small hierarchy with an End Site, round trips, and the failure
//! paths (syntax, counts, bounds).

use flare_moho::bvh::{self, Axis, BvhError, Motion};

const SAMPLE: &str = "HIERARCHY
ROOT Hips
{
\tOFFSET 0.0 0.0 0.0
\tCHANNELS 6 Xposition Yposition Zposition Zrotation Xrotation Yrotation
\tJOINT Spine
\t{
\t\tOFFSET 0.0 5.0 0.0
\t\tCHANNELS 3 Zrotation Xrotation Yrotation
\t\tEnd Site
\t\t{
\t\t\tOFFSET 0.0 3.0 0.0
\t\t}
\t}
}
MOTION
Frames: 2
Frame Time: 0.033333
0.0 90.0 0.0 0.0 0.0 0.0 1.0 2.0 3.0
1.0 91.0 0.0 0.0 0.0 0.0 1.5 2.5 3.5
";

#[test]
fn reads_hierarchy_offsets_channels_and_motion() {
    let bvh = bvh::parse(SAMPLE).expect("parse");
    assert_eq!(bvh.joints.len(), 2);
    assert_eq!(bvh.joints[0].name, "Hips");
    assert_eq!(bvh.joints[0].parent, None);
    assert_eq!(bvh.joints[1].name, "Spine");
    assert_eq!(bvh.joints[1].parent, Some(0));
    assert_eq!(bvh.joints[1].offset, [0.0, 5.0, 0.0]);
    assert_eq!(bvh.joints[1].end_site, Some([0.0, 3.0, 0.0]));
    assert_eq!(bvh.joints[0].channels[3], Motion::Rotation(Axis::Z));
    assert_eq!(bvh.channel_count(), 9);
    assert_eq!(bvh.frame_count, 2);
    assert!((bvh.frame_time - 0.033333).abs() < 1e-12);
    assert_eq!(bvh.value(1, 0), Some(1.0));
    assert_eq!(bvh.value(0, 8), Some(3.0));
    assert_eq!(bvh.value(2, 0), None);
    assert_eq!(bvh.value(0, 9), None);
}

#[test]
fn writing_then_reading_gives_the_same_value() {
    let first = bvh::parse(SAMPLE).expect("parse");
    let text = bvh::to_text(&first).expect("write");
    assert!(text.starts_with("HIERARCHY\nROOT Hips\n"));
    assert!(text.contains("\nMOTION\nFrames: 2\n"));
    let second = bvh::parse(&text).expect("re-read");
    assert_eq!(first, second);
    assert_eq!(bvh::to_text(&second).expect("write again"), text);
}

fn syntax_at(text: &str) -> (usize, String) {
    match bvh::parse(text) {
        Err(BvhError::Syntax { line, message }) => (line, message),
        other => panic!("expected a syntax error, got {other:?}"),
    }
}

#[test]
fn missing_header_is_refused() {
    let (line, _) = syntax_at("ROOT Hips\n");
    assert_eq!(line, 1);
}

#[test]
fn unknown_channel_names_the_line() {
    let text = SAMPLE.replace("Xposition", "Wposition");
    let (line, message) = syntax_at(&text);
    assert_eq!(line, 5);
    assert!(message.contains("Wposition"));
}

#[test]
fn channel_count_must_match_its_names() {
    let text = SAMPLE.replace("CHANNELS 6", "CHANNELS 5");
    let (_, message) = syntax_at(&text);
    assert!(message.contains("CHANNELS count"));
}

#[test]
fn a_motion_row_with_too_few_values_is_refused() {
    let text = SAMPLE.replace("1.0 91.0 0.0 0.0 0.0 0.0 1.5 2.5 3.5", "1.0 91.0");
    let (line, message) = syntax_at(&text);
    assert_eq!(line, 20);
    assert!(message.contains("values where 9"));
}

#[test]
fn frame_count_must_match_the_rows() {
    let more = format!("{SAMPLE}0.0 0.0 0.0 0.0 0.0 0.0 0.0 0.0 0.0\n");
    let (_, message) = syntax_at(&more);
    assert!(message.contains("more motion rows"));
    let fewer = SAMPLE.replace("Frames: 2", "Frames: 3");
    let (_, message) = syntax_at(&fewer);
    assert!(message.contains("unexpected end of file"));
}

#[test]
fn non_finite_numbers_are_refused() {
    let text = SAMPLE.replace("Frame Time: 0.033333", "Frame Time: nan");
    let (_, message) = syntax_at(&text);
    assert!(message.contains("finite"));
}

#[test]
fn a_second_root_and_a_joint_outside_the_root_are_refused() {
    let second = SAMPLE.replace("JOINT Spine", "ROOT Other");
    let (_, message) = syntax_at(&second);
    assert!(message.contains("one ROOT"));
    let outside = "HIERARCHY\nJOINT Loose\n{\n}\nMOTION\nFrames: 0\nFrame Time: 0.1\n";
    let (_, message) = syntax_at(outside);
    assert!(message.contains("outside ROOT"));
}

#[test]
fn an_unclosed_joint_is_refused() {
    let text = SAMPLE.replacen("\t}\n}\nMOTION", "\t}\nMOTION", 1);
    let (_, message) = syntax_at(&text);
    assert!(message.contains("not closed"));
}

fn chain(joints: usize) -> String {
    let mut text = String::from("HIERARCHY\n");
    for i in 0..joints {
        let keyword = if i == 0 { "ROOT" } else { "JOINT" };
        text += &format!("{keyword} j{i}\n{{\nOFFSET 0 0 0\nCHANNELS 1 Xposition\n");
    }
    text.push_str(&"}\n".repeat(joints));
    text.push_str("MOTION\nFrames: 0\nFrame Time: 0.1\n");
    text
}

#[test]
fn nesting_deeper_than_the_limit_is_refused_and_a_shallow_chain_is_read() {
    assert!(bvh::parse(&chain(8)).is_ok());
    assert!(matches!(
        bvh::parse(&chain(70)),
        Err(BvhError::Limit {
            what: "joint depth",
            ..
        })
    ));
    assert!(matches!(
        bvh::parse(&chain(1100)),
        Err(BvhError::Limit { .. })
    ));
}

#[test]
fn oversize_text_is_refused_before_reading() {
    let big = " ".repeat(bvh::MAX_BYTES + 1);
    assert!(matches!(bvh::parse(&big), Err(BvhError::TooLarge { .. })));
}

#[test]
fn writer_refuses_an_invalid_value() {
    let mut bvh = bvh::parse(SAMPLE).expect("parse");
    bvh.joints[1].name = "Two Words".to_owned();
    assert_eq!(bvh::to_text(&bvh), Err(BvhError::BadName { index: 1 }));

    let mut bvh = bvh::parse(SAMPLE).expect("parse");
    bvh.motion.pop();
    assert!(matches!(
        bvh::to_text(&bvh),
        Err(BvhError::MotionLength {
            expected: 18,
            found: 17
        })
    ));

    let mut bvh = bvh::parse(SAMPLE).expect("parse");
    bvh.joints[1].parent = Some(5);
    assert_eq!(
        bvh::to_text(&bvh),
        Err(BvhError::Parent {
            index: 1,
            parent: 5
        })
    );
}

#[test]
fn motion_names_round_trip() {
    for name in ["Xposition", "Yrotation", "Zrotation"] {
        let motion = Motion::from_name(name).expect("known name");
        assert_eq!(motion.name(), name);
    }
    assert_eq!(Motion::from_name("Wrotation"), None);
}
