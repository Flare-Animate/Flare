//! `flare video-import` with stand-in programs on the PATH of the child process in the place of
//! ffprobe and ffmpeg (shell scripts that answer as they do), so the command is checked on a
//! computer without ffmpeg.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;

use super::*;

fn script(dir: &Path, name: &str, body: &str) {
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
}

/// A folder holding an ffprobe that reports a 320x180 movie at 30 frames a second, `seconds`
/// long, and an ffmpeg that writes `pictures` 160x90 pictures and notes that it ran.
fn stand_ins(dir: &Path, seconds: u32, pictures: usize) -> std::path::PathBuf {
    let bin = dir.join("bin");
    std::fs::create_dir(&bin).expect("bin");
    script(
        &bin,
        "ffprobe",
        &format!(
            r#"case "$*" in
  *format=duration*) echo "{seconds}" ;;
  *stream_side_data*) ;;
  *) echo "320,180,30/1,30/1" ;;
esac"#
        ),
    );
    script(
        &bin,
        "ffmpeg",
        &format!(
            "touch '{}'\nhead -c {} /dev/urandom",
            dir.join("ran").display(),
            pictures * 160 * 90 * 4
        ),
    );
    bin
}

fn run_with(bin: &Path, args: &[&str]) -> Output {
    let path = format!("{}:/usr/bin:/bin", bin.display());
    Command::new(env!("CARGO_BIN_EXE_flare"))
        .args(args)
        .env("PATH", path)
        .output()
        .expect("run cli")
}

fn json_with(bin: &Path, args: &[&str]) -> Value {
    let output = run_with(bin, args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("json")
}

#[test]
fn a_plan_says_what_the_import_makes_and_reads_nothing() {
    let dir = tempfile::tempdir().expect("dir");
    let bin = stand_ins(dir.path(), 4, 0);
    let file = path(dir.path());
    json(&["new", &file]);
    let movie = dir.path().join("walk.mp4").to_string_lossy().into_owned();
    let plan = json_with(
        &bin,
        &[
            "video-import",
            &file,
            &movie,
            "--fps",
            "12",
            "--max-side",
            "160",
            "--plan",
        ],
    );
    assert_eq!(plan["frames"], 48, "four seconds at 12 a second");
    assert_eq!(plan["picture"]["width"], 160);
    assert_eq!(plan["document_fps"], 24.0);
    assert_eq!(plan["animation_seconds"], 2.0, "48 frames at 24 a second");
    assert_eq!(plan["keeps_timing"], false);
    assert_eq!(plan["fits"], true);
    assert!(!dir.path().join("ran").exists(), "ffmpeg did not run");
    let info = json(&["info", &file]);
    let layers = &info["scenes"][0]["layers"];
    assert_eq!(layers.as_array().map(Vec::len), Some(1), "unchanged");
}

#[test]
fn the_video_is_imported_on_a_layer_of_its_own_one_picture_a_frame() {
    let dir = tempfile::tempdir().expect("dir");
    let bin = stand_ins(dir.path(), 4, 48);
    let file = path(dir.path());
    json(&["new", &file]);
    let movie = dir.path().join("walk.mp4").to_string_lossy().into_owned();
    let done = json_with(
        &bin,
        &[
            "video-import",
            &file,
            &movie,
            "--fps",
            "12",
            "--max-side",
            "160",
            "--frame",
            "3",
            "--opacity",
            "30",
        ],
    );
    assert_eq!(
        (done["frames"].as_u64(), done["frame"].as_u64()),
        (Some(48), Some(3))
    );
    assert_eq!(done["distinct_pictures"], 48);
    let info = json(&["info", &file]);
    let layers = info["scenes"][0]["layers"]
        .as_array()
        .expect("layers")
        .clone();
    assert_eq!(layers.len(), 2);
    let bottom = layers.last().expect("bottom");
    assert_eq!(bottom["id"], done["layer"]);
    assert_eq!(
        (bottom["name"].as_str(), bottom["opacity"].as_u64()),
        (Some("walk"), Some(30))
    );
    // Shown from frame 3, the 160x90 pictures fitted to the 1920x1080 stage, 12 times as large.
    let frame = json(&["eval", &file, "--frame", "10"]);
    let placed = &frame["placed"][0];
    assert_eq!(placed["symbol"], done["symbol"]);
    assert_eq!(
        placed["matrix"],
        serde_json::json!([12.0, 0.0, 0.0, 12.0, 0.0, 0.0])
    );
    let before = json(&["eval", &file, "--frame", "2"]);
    assert_eq!(
        before["placed"].as_array().map(Vec::len),
        Some(0),
        "not before its frame"
    );
}

#[test]
fn a_video_too_large_is_refused_with_what_would_fit() {
    let dir = tempfile::tempdir().expect("dir");
    let bin = stand_ins(dir.path(), 600, 1);
    let file = path(dir.path());
    json(&["new", &file]);
    let movie = dir.path().join("walk.mp4").to_string_lossy().into_owned();
    // Ten minutes at the document's 24 a second: more pictures than a document holds.
    let output = run_with(
        &bin,
        &["video-import", &file, &movie, "--max-minutes", "10"],
    );
    assert!(!output.status.success());
    let said = String::from_utf8_lossy(&output.stderr);
    assert!(
        said.contains("too large") && said.contains("--max-minutes"),
        "{said}"
    );
    assert!(!dir.path().join("ran").exists(), "nothing was read");
    // Without --max-minutes it reads as much of the first minute as fits, which does.
    let plan = json_with(&bin, &["video-import", &file, &movie, "--plan"]);
    assert_eq!(plan["fits"], true);
    assert_eq!(plan["whole_video"], false);
}

#[test]
fn without_ffmpeg_the_command_says_it_is_needed() {
    let dir = tempfile::tempdir().expect("dir");
    let empty = dir.path().join("empty");
    std::fs::create_dir(&empty).expect("dir");
    let file = path(dir.path());
    json(&["new", &file]);
    let output = Command::new(env!("CARGO_BIN_EXE_flare"))
        .args(["video-import", &file, "walk.mp4"])
        .env("PATH", &empty)
        .output()
        .expect("run");
    assert!(!output.status.success());
    let said = String::from_utf8_lossy(&output.stderr);
    assert!(said.contains("needs ffmpeg"), "{said}");
}
