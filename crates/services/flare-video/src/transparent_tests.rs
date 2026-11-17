//! Movies over a transparent backdrop: the formats with an alpha channel keep it, and the others
//! refuse it with the reason and write nothing. The ffmpeg ones check nothing where ffmpeg is not
//! on the computer, and say so.

use flare_render::Backdrop;

use super::*;

fn ffmpeg() -> Option<flare_ffmpeg::Tools> {
    match flare_ffmpeg::locate() {
        Ok(tools) => Some(tools),
        Err(_) => {
            eprintln!("ffmpeg is not installed here: this test checks nothing");
            None
        }
    }
}

#[test]
fn an_mp4_and_the_containers_without_alpha_refuse_a_transparent_backdrop() {
    let dir = tempfile::tempdir().expect("dir");
    let document = animation(2, (64, 64));
    let mp4 = dir.path().join("a.mp4");
    let error = write_mp4_over(&document, &mp4, Backdrop::Transparent).expect_err("no alpha");
    assert!(
        error.starts_with("MP4 videos cannot have a transparent background"),
        "{error}"
    );
    assert!(!mp4.exists(), "nothing is written");
    for id in ["avi", "mkv", "mpeg", "flv"] {
        let container = flare_ffmpeg::container(id).expect("known");
        let out = dir.path().join(format!("a.{}", container.extension));
        let error = write_with_ffmpeg(&document, &out, container, Backdrop::Transparent)
            .expect_err("no alpha");
        assert!(
            error.starts_with(&format!("{}s cannot", container.label)),
            "{id}: {error}"
        );
        assert!(!out.exists(), "{id}: nothing is written");
    }
    write_mp4_over(&document, &mp4, Backdrop::Stage).expect("over the stage it is made");
}

#[test]
fn a_transparent_quicktime_movie_is_prores_4444_with_the_stage_left_clear() {
    if ffmpeg().is_none() {
        return;
    }
    let dir = tempfile::tempdir().expect("dir");
    let out = dir.path().join("a.mov");
    write_mov_over(&animation(3, (64, 64)), &out, Backdrop::Transparent).expect("mov");
    let clip = flare_ffmpeg::decode(&out, 10, 64).expect("reads back");
    assert_eq!(clip.frames.len(), 3);
    let alpha = |x: usize, y: usize| clip.frames[0][(y * 64 + x) * 4 + 3];
    assert!(
        alpha(30, 30) > 250,
        "the square is opaque: {}",
        alpha(30, 30)
    );
    assert!(
        alpha(60, 60) < 5,
        "the empty stage is clear: {}",
        alpha(60, 60)
    );
}

#[test]
fn a_transparent_webm_is_vp9_with_its_alpha() {
    let Some(tools) = ffmpeg() else {
        return;
    };
    let dir = tempfile::tempdir().expect("dir");
    let out = dir.path().join("a.webm");
    let webm = flare_ffmpeg::container("webm").expect("webm");
    write_with_ffmpeg(&animation(3, (64, 64)), &out, webm, Backdrop::Transparent).expect("webm");
    // A VP9 picture keeps its alpha beside it, which the WebM marks on the track.
    let probe = std::process::Command::new(&tools.ffprobe)
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_name:stream_tags",
        ])
        .arg(&out)
        .output()
        .expect("ffprobe runs");
    let text = String::from_utf8_lossy(&probe.stdout).to_ascii_lowercase();
    assert!(text.contains("codec_name=vp9"), "{text}");
    assert!(text.contains("alpha_mode=1"), "{text}");
}
