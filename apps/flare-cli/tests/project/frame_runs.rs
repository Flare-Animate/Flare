//! Moving and copying runs of frames, closing gaps, and drawing on an empty frame.

use super::*;

#[test]
fn frames_move_and_copy_as_runs_gaps_close_and_a_drawing_holds_the_keyframe_before_it() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    let out = dir.path().join("f.png").to_string_lossy().into_owned();
    let spans =
        |layer: usize| json(&["info", &file])["scenes"][0]["layers"][layer]["spans"].as_u64();
    let black_at = |frame: &str| {
        json(&["export", &file, &out, "--frame", frame]);
        pixel(&read_png(&out), 2, 2) == [0, 0, 0, 255]
    };
    json(&["new", &file, "--width", "20", "--height", "20"]);
    let rect = ["--rect", "0", "0", "5", "5"];
    json(&[&["draw-rect", &file][..], &rect, &["--duration", "2"]].concat());

    json(&[
        "frame",
        "copy-frames",
        &file,
        "--at",
        "0",
        "--to",
        "1",
        "--dest",
        "5",
    ]);
    assert_eq!(spans(0), Some(2));
    assert!(
        black_at("6") && !black_at("3"),
        "a copy at 5 and 6, frames 2 to 4 empty"
    );

    // Taking the empty frames out closes the gap.
    json(&["frame", "remove-frames", &file, "--at", "2", "--to", "4"]);
    assert!(black_at("3"), "the copy moved up to frames 2 and 3");

    json(&[
        "frame",
        "move-frames",
        &file,
        "--at",
        "2",
        "--to",
        "3",
        "--dest",
        "10",
    ]);
    assert_eq!(spans(0), Some(2));
    assert!(
        !black_at("3") && black_at("11"),
        "moved, its old frames left empty"
    );

    // A drawing on an empty frame: the keyframe before holds the frames up to it.
    json(
        &[
            &["draw-rect", &file][..],
            &["--rect", "10", "10", "5", "5"],
            &["--start", "14"],
        ]
        .concat(),
    );
    assert_eq!(spans(0), Some(3));
    assert!(black_at("13"), "frames 12 and 13 show the keyframe at 10");

    let second = json(&["add-layer", &file, "Second"])["layer"]
        .as_str()
        .expect("id")
        .to_owned();
    json(&[
        "frame",
        "move-frames",
        &file,
        "--at",
        "0",
        "--to",
        "1",
        "--dest",
        "0",
        "--onto",
        &second,
    ]);
    assert_eq!((spans(0), spans(1)), (Some(2), Some(1)));

    let before = std::fs::read(&file).expect("read");
    for refused in [
        vec!["frame", "move-frames", &file, "--at", "10", "--to", "13"],
        vec![
            "frame",
            "copy-frames",
            &file,
            "--at",
            "4",
            "--to",
            "8",
            "--dest",
            "20",
        ],
        vec![
            "frame",
            "move-frames",
            &file,
            "--at",
            "10",
            "--to",
            "13",
            "--dest",
            "10",
        ],
    ] {
        assert!(!run(&refused).status.success(), "{refused:?}");
    }
    assert_eq!(std::fs::read(&file).expect("read"), before);
}
