//! Bringing a video in through the MCP server, with stand-in programs in the place of ffprobe
//! and ffmpeg (shell scripts that answer as they do), so the tools are checked on a computer
//! without ffmpeg.
#![cfg(unix)]
#![allow(clippy::expect_used)]

mod common;

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use common::fixture_with_video_tools;
use serde_json::json;

fn script(dir: &Path, name: &str, body: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    path
}

/// A 64x36 movie at 30 frames a second, 2 seconds long; ffmpeg writes 24 such pictures and
/// leaves a note that it ran. The movie itself is an empty file in the export directory.
fn tools(dir: &Path) -> flare_ffmpeg::Tools {
    let exports = dir.join("exports");
    std::fs::create_dir_all(&exports).expect("exports");
    std::fs::write(exports.join("walk.mp4"), b"").expect("movie");
    flare_ffmpeg::Tools {
        ffprobe: script(
            dir,
            "ffprobe",
            r#"case "$*" in
  *format=duration*) echo "2.0" ;;
  *stream_side_data*) ;;
  *) echo "64,36,30/1,30/1" ;;
esac"#,
        ),
        ffmpeg: script(
            dir,
            "ffmpeg",
            &format!(
                "touch '{}'\nhead -c {} /dev/urandom",
                dir.join("ran").display(),
                24 * 64 * 36 * 4
            ),
        ),
    }
}

#[tokio::test]
async fn a_plan_reads_nothing_and_an_import_makes_a_layer_of_one_picture_a_frame() {
    let f = fixture_with_video_tools(tools).await;
    let plan = f
        .ok(
            "plan_video_import",
            json!({ "source": "walk.mp4", "fps": 12 }),
        )
        .await;
    assert_eq!(plan["frames"], 24);
    assert_eq!(
        plan["animation_seconds"], 1.0,
        "24 frames at the document's 24"
    );
    assert_eq!(plan["speed"], 2.0);
    assert!(!f.dir.path().join("ran").exists(), "nothing was read");

    let done = f
        .ok(
            "import_video",
            json!({ "source": "walk.mp4", "fps": 12, "frame": 5, "opacity": 25 }),
        )
        .await;
    assert_eq!(
        (done["frames"].as_u64(), done["frame"].as_u64()),
        (Some(24), Some(5))
    );
    assert_eq!(done["distinct_pictures"], 24);
    let info = f.ok("get_info", json!({})).await;
    let text = info.to_string();
    assert!(
        text.contains("walk"),
        "the layer is named for the file: {text}"
    );
}

#[tokio::test]
async fn a_movie_outside_the_export_directory_a_missing_one_and_a_bad_length_are_refused() {
    let f = fixture_with_video_tools(tools).await;
    let (is_error, message) = f
        .tool("import_video", json!({ "source": "../p.flare" }))
        .await;
    assert!(is_error, "{message}");
    let (is_error, message) = f
        .tool("import_video", json!({ "source": "missing.mp4" }))
        .await;
    assert!(
        is_error && message.to_string().contains("not a file"),
        "{message}"
    );
    // Refused by the plan, before anything is read.
    let (is_error, message) = f
        .tool(
            "import_video",
            json!({ "source": "walk.mp4", "fps": 240, "max_minutes": 0 }),
        )
        .await;
    assert!(
        is_error && message.to_string().contains("60 minutes"),
        "{message}"
    );
    assert!(!f.dir.path().join("ran").exists(), "nothing was read");
}
