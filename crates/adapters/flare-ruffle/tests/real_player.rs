//! Test Movie against a real Ruffle. Ignored by default because it opens a player window and
//! needs a built player: `scripts/build_ruffle.sh`, then
//! `FLARE_RUFFLE=target/ruffle-desktop/release/ruffle_desktop cargo test -p flare-ruffle
//! --test real_player -- --ignored`.
#![allow(clippy::expect_used)]

use std::num::NonZeroU32;
use std::time::{Duration, Instant};

use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId};
use flare_document::{Content, Document, FrameSpan, Layer, Scene, Shape, Stage, Timeline};

fn movie() -> Document {
    let shape = Shape::rect(
        ShapeId::new(),
        10.0,
        10.0,
        50.0,
        40.0,
        Rgba8::opaque(220, 30, 30),
    )
    .expect("rect");
    let span = FrameSpan::new(0, NonZeroU32::new(24).expect("n"), Content::Shape(shape));
    let layer = Layer::new(LayerId::new(), "L", vec![span]).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    Document::new(
        DocumentId::new(),
        FrameRate::new(12, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document")
    .with_stage(Stage::new(120, 80, Rgba8::WHITE).expect("stage"))
}

#[test]
#[ignore = "opens a Ruffle window; needs a built player (scripts/build_ruffle.sh)"]
fn the_real_player_opens_the_movie_and_keeps_playing_until_stopped() {
    let player = flare_ruffle::find_ruffle().expect("FLARE_RUFFLE names a built Ruffle");
    let mut test_movie = flare_ruffle::launch_test_movie(&movie(), &player).expect("launch");
    // A player that cannot load the file exits at once; one that plays a looping movie does not.
    let until = Instant::now() + Duration::from_secs(4);
    while Instant::now() < until {
        assert!(
            test_movie.try_wait().expect("status").is_none(),
            "Ruffle exited while the movie should be playing"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    let swf = test_movie.swf_path().to_owned();
    assert!(swf.exists());
    test_movie.kill().expect("stop");
    drop(test_movie);
    assert!(!swf.exists(), "the files go with the player");
}
