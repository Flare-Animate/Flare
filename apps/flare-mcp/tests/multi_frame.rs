//! Edit Multiple Frames through the MCP server: `transform_frames`.
#![allow(clippy::expect_used)]

mod common;

use common::{Fixture, fixture};
use serde_json::json;

async fn x_at(f: &Fixture, frame: u32) -> f64 {
    f.ok("evaluate_frame", json!({ "frame": frame })).await["placed"][0]["matrix"][4]
        .as_f64()
        .expect("x")
}

#[tokio::test]
async fn transform_frames_moves_the_range_as_one_step_and_refuses_nothing_to_do() {
    let f = fixture(true, false).await;
    let sym = f.ok("add_symbol", json!({ "name": "Ball" })).await["symbol"]
        .as_str()
        .expect("id")
        .to_owned();
    for (start, x) in [(0, 10.0), (4, 20.0), (8, 30.0)] {
        f.ok(
            "place_instance",
            json!({ "symbol": sym, "start": start, "duration": 4, "x": x }),
        )
        .await;
    }
    let before = f.revision().await;
    f.ok(
        "transform_frames",
        json!({ "from": 0, "to": 5, "dx": 100.0 }),
    )
    .await;
    assert_eq!(f.revision().await, before + 1, "one step");
    assert_eq!(x_at(&f, 0).await, 110.0);
    assert_eq!(x_at(&f, 4).await, 120.0);
    assert_eq!(x_at(&f, 8).await, 30.0, "outside the range");
    for bad in [
        json!({ "from": 40, "to": 50, "dx": 1.0 }),
        json!({ "from": 0, "to": 5 }),
        json!({ "from": 0, "to": 5, "scale_x": 0.0 }),
    ] {
        let (failed, _) = f.tool("transform_frames", bad.clone()).await;
        assert!(failed, "{bad}");
    }
    assert_eq!(f.revision().await, before + 1, "a refusal changes nothing");
}
