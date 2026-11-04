use super::*;

fn two_keyframes(dir: &Path) -> String {
    let file = path(dir);
    json(&["new", &file, "--width", "100", "--height", "100"]);
    for (start, x, duration) in [("0", "0", "4"), ("4", "40", "2")] {
        json(&[
            "draw-rect",
            &file,
            "--rect",
            x,
            "10",
            "10",
            "10",
            "--color",
            "FF0000",
            "--start",
            start,
            "--duration",
            duration,
        ]);
    }
    file
}

#[test]
fn list_easings_and_eval_need_no_project() {
    let list = json(&["tween", "list-easings"]);
    assert!(list["families"].as_array().expect("families").len() >= 13);
    assert!(
        list["easings"]
            .as_array()
            .expect("easings")
            .iter()
            .any(|e| e == "in-out")
    );
    let half = json(&[
        "tween", "eval", "--family", "quad", "--easing", "in", "--t", "0.5",
    ]);
    assert!(half["value"].as_f64().expect("value") < 0.5);
    let bad = run(&["tween", "eval", "--family", "wobble", "--t", "0.5"]);
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("unknown interpolation"));
    assert!(
        !run(&["tween", "eval", "--family", "quad", "--t", "2"])
            .status
            .success()
    );
}

#[test]
fn set_interpolation_and_inbetween_edit_the_file_and_refuse_bad_requests() {
    let dir = tempfile::tempdir().expect("dir");
    let file = two_keyframes(dir.path());
    let set = json(&[
        "tween",
        "set-interpolation",
        &file,
        "--at",
        "0",
        "--family",
        "bounce",
        "--easing",
        "out",
    ]);
    assert!(set["revision"].is_number());
    assert!(
        !run(&[
            "tween",
            "set-interpolation",
            &file,
            "--at",
            "1",
            "--family",
            "quad"
        ])
        .status
        .success()
    );
    assert!(
        !run(&["tween", "inbetween", &file, "--at", "0", "--count", "0"])
            .status
            .success()
    );
    assert!(
        !run(&["tween", "inbetween", &file, "--at", "0", "--count", "9"])
            .status
            .success()
    );
    let before = std::fs::read(&file).expect("read");
    assert!(
        !run(&["tween", "inbetween", &file, "--at", "4", "--count", "1"])
            .status
            .success()
    );
    assert_eq!(
        std::fs::read(&file).expect("read"),
        before,
        "a refusal changes nothing"
    );
    json(&["tween", "set-interpolation", &file, "--at", "0", "--clear"]);
    json(&["tween", "inbetween", &file, "--at", "0", "--count", "3"]);
    let info = json(&["info", &file]);
    assert!(info.to_string().len() > 2);
}
