//! `flare stop-motion-project`: the session a project keeps, built from PNG files. The rules are
//! tested in `flare_editor::stop_motion_library`; here the files, the options and the refusals.

use super::*;

fn picture(dir: &Path, name: &str, shade: u8) -> String {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, 2, 2);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("header");
    writer.write_image_data(&[shade; 16]).expect("data");
    writer.finish().expect("finish");
    let file = dir.join(name);
    std::fs::write(&file, out).expect("picture");
    file.to_string_lossy().into_owned()
}

#[test]
fn a_session_is_kept_in_the_project_and_laid_on_a_layer() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file]);
    let sp = "stop-motion-project";
    json(&[sp, "start", &file, "--fps", "12", "--hold", "2"]);
    let a = picture(dir.path(), "a.png", 10);
    let b = picture(dir.path(), "b.png", 99);
    json(&[sp, "add", &file, &a]);
    let added = json(&[sp, "add", &file, &b, "--hold", "3"]);
    assert_eq!(added["frames"], 2);
    json(&[sp, "add", &file, &a]);
    let listed = json(&[sp, "list", &file]);
    assert_eq!(listed["duration"], 7, "{listed}");
    assert_eq!(
        listed["frames"][0]["bitmap"], listed["frames"][2]["bitmap"],
        "the same picture is stored once"
    );
    json(&[sp, "hold", &file, "0", "5"]);
    assert_eq!(json(&[sp, "list", &file])["duration"], 10);
    json(&[sp, "remove", &file, "1"]);
    json(&[sp, "rate", &file, "--fps", "24", "--hold", "1"]);
    json(&[sp, "lay", &file]);
    let layers = json(&["info", &file])["scenes"][0]["layers"].clone();
    assert!(layers.to_string().contains("Stop motion"), "{layers}");
    json(&[sp, "end", &file]);
    assert!(
        !run(&[sp, "list", &file]).status.success(),
        "the session ended"
    );
}

#[test]
fn refusals_name_the_reason_and_leave_the_project_alone() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file]);
    let sp = "stop-motion-project";
    let a = picture(dir.path(), "a.png", 1);
    let before = std::fs::read(&file).expect("file");
    let early = run(&[sp, "add", &file, &a]);
    assert!(String::from_utf8_lossy(&early.stderr).contains("no stop-motion session"));
    let bad_rate = run(&[sp, "start", &file, "--fps", "0"]);
    assert!(!bad_rate.status.success());
    json(&[sp, "start", &file]);
    let started = std::fs::read(&file).expect("file");
    assert_ne!(before, started);
    let twice = run(&[sp, "start", &file]);
    assert!(String::from_utf8_lossy(&twice.stderr).contains("already has"));
    let not_png = dir.path().join("x.png");
    std::fs::write(&not_png, b"not a picture").expect("file");
    let bad = run(&[sp, "add", &file, &not_png.to_string_lossy()]);
    assert!(!bad.status.success());
    let nothing = run(&[sp, "hold", &file, "4", "2"]);
    assert!(String::from_utf8_lossy(&nothing.stderr).contains("no frame 4"));
    assert_eq!(
        std::fs::read(&file).expect("file"),
        started,
        "nothing changed"
    );
}
