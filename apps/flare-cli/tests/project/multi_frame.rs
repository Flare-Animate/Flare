//! `flare transform-frames`: Edit Multiple Frames from the command line.

use super::*;

fn x_at(file: &str, frame: u32) -> f64 {
    json(&["eval", file, "--frame", &frame.to_string()])["placed"][0]["matrix"][4]
        .as_f64()
        .expect("x")
}

#[test]
fn transform_frames_moves_every_keyframe_in_the_range_and_nothing_else() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file]);
    let symbol = json(&["add-symbol", &file, "Ball"])["symbol"]
        .as_str()
        .expect("id")
        .to_owned();
    for (start, x) in [("0", "10"), ("4", "20"), ("8", "30")] {
        json(&[
            "place",
            &file,
            &symbol,
            "--start",
            start,
            "--duration",
            "4",
            "--x",
            x,
        ]);
    }
    let out = json(&[
        "transform-frames",
        &file,
        "--from",
        "0",
        "--to",
        "5",
        "--dx",
        "100",
    ]);
    assert_eq!(out["from"], 0);
    assert_eq!(x_at(&file, 0), 110.0);
    assert_eq!(x_at(&file, 4), 120.0);
    assert_eq!(x_at(&file, 8), 30.0, "outside the range");

    // Nothing in the range, or nothing to do: refused and nothing written.
    let written = std::fs::read(&file).expect("read");
    for bad in [
        vec![
            "transform-frames",
            &file,
            "--from",
            "40",
            "--to",
            "50",
            "--dx",
            "1",
        ],
        vec!["transform-frames", &file, "--from", "0", "--to", "5"],
        vec![
            "transform-frames",
            &file,
            "--from",
            "0",
            "--to",
            "5",
            "--scale-x",
            "0",
        ],
    ] {
        assert!(!run(&bad).status.success(), "{bad:?}");
    }
    assert_eq!(std::fs::read(&file).expect("read"), written);
}
