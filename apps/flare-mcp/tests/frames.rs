//! The frame picker through the MCP server: listing frames and moving the playhead.
#![allow(clippy::expect_used)]

mod common;

use common::fixture;
use serde_json::json;

async fn two_keyframes(f: &common::Fixture) {
    f.ok(
        "draw_rect",
        json!({ "x": 0, "y": 0, "width": 5, "height": 5, "start": 0, "duration": 5 }),
    )
    .await;
    f.ok(
        "draw_rect",
        json!({ "x": 10, "y": 0, "width": 5, "height": 5, "start": 5, "duration": 3 }),
    )
    .await;
}

#[tokio::test]
async fn frames_are_listed_with_their_keyframes_and_the_playhead_moves_by_word_or_number() {
    let f = fixture(true, false).await;
    two_keyframes(&f).await;
    let listed = f.ok("list_frames", json!({ "first": 3, "count": 4 })).await;
    assert_eq!(listed["frames"], 8);
    let cells = listed["cells"].as_array().expect("cells");
    assert_eq!(cells.len(), 4);
    assert_eq!(cells[2]["frame"], 5);
    assert_eq!(cells[2]["keyframes"], 1);
    assert_eq!(cells[0]["keyframes"], 0);

    assert_eq!(
        f.ok("goto_frame", json!({ "target": "next-keyframe" }))
            .await["frame"],
        5
    );
    assert_eq!(
        f.ok("goto_frame", json!({ "target": "last" })).await["frame"],
        7
    );
    assert_eq!(
        f.ok("goto_frame", json!({ "target": "2" })).await["frame"],
        2
    );
    assert_eq!(f.ok("list_frames", json!({})).await["playhead"], 2);
}

#[tokio::test]
async fn a_frame_outside_the_scene_or_a_word_that_is_none_is_refused_and_the_playhead_stays() {
    let f = fixture(true, false).await;
    two_keyframes(&f).await;
    f.ok("goto_frame", json!({ "target": "4" })).await;
    for bad in ["99", "soon", "-1", "previous-keyframe-ish"] {
        let (is_error, message) = f.tool("goto_frame", json!({ "target": bad })).await;
        assert!(is_error, "{bad}: {message}");
    }
    let (is_error, message) = f.tool("goto_frame", json!({ "target": "99" })).await;
    assert!(
        is_error && message.to_string().contains("8 frames"),
        "{message}"
    );
    assert_eq!(f.ok("list_frames", json!({})).await["playhead"], 4);
}

#[tokio::test]
async fn drawing_with_no_start_happens_at_the_playhead_and_a_read_only_server_can_still_go() {
    let f = fixture(true, false).await;
    two_keyframes(&f).await;
    f.ok("goto_frame", json!({ "target": "6" })).await;
    // Frame 6 is inside the second keyframe: the drawing there is copy-on-write.
    f.ok(
        "draw_rect",
        json!({ "x": 20, "y": 0, "width": 5, "height": 5 }),
    )
    .await;
    let listed = f.ok("list_frames", json!({})).await;
    assert_eq!(
        listed["cells"][6]["keyframes"], 1,
        "a keyframe starts at the playhead"
    );

    // Going to a frame is not an edit of the project, so a read-only server does it.
    let layer = flare_document::Layer::new(
        flare_core::LayerId::new(),
        "L",
        vec![flare_document::FrameSpan::new(
            0,
            std::num::NonZeroU32::new(3).expect("nonzero"),
            flare_document::Content::Empty,
        )],
    )
    .expect("layer");
    let scene = flare_document::Scene::new(
        flare_core::SceneId::new(),
        "S",
        flare_document::Timeline::new(vec![layer]),
    );
    let document = flare_document::Document::new(
        flare_core::DocumentId::new(),
        flare_core::FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document");
    let read_only = common::fixture_of(&document, false, false, vec![]).await;
    let went = read_only
        .ok("goto_frame", json!({ "target": "last" }))
        .await;
    assert_eq!(went["frame"], 2);
}
