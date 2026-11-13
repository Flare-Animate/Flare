//! Movies the command line writes: the ffmpeg containers (checked against ffmpeg itself where it is
//! installed, and checked for the "not installed" message where its directory is taken off PATH),
//! and the QuickTime movie and animated WebP that Flare writes itself.
use super::*;

/// A 10 by 10 stage with a red square, made by the command line itself.
fn square(dir: &Path) -> String {
    let file = path(dir);
    json(&["new", &file, "--width", "10", "--height", "10"]);
    json(&[
        "draw-rect",
        &file,
        "--rect",
        "0",
        "0",
        "10",
        "10",
        "--color",
        "FF0000",
    ]);
    file
}

#[test]
fn export_writes_each_ffmpeg_container_and_it_reads_back() {
    if flare_ffmpeg::locate().is_err() {
        eprintln!("ffmpeg is not installed here: this checks nothing");
        return;
    }
    let dir = tempfile::tempdir().expect("dir");
    let file = square(dir.path());
    for (format, extension) in [
        ("avi", "avi"),
        ("mkv", "mkv"),
        ("webm", "webm"),
        ("mpeg", "mpg"),
        ("flv", "flv"),
    ] {
        let out = dir
            .path()
            .join(format!("movie.{extension}"))
            .to_string_lossy()
            .into_owned();
        let info = json(&["export", &file, &out, "--format", format]);
        assert!(info["bytes"].as_u64().is_some_and(|n| n > 0), "{format}");
        let clip = flare_ffmpeg::decode(Path::new(&out), 10, 1920).expect(format);
        assert_eq!((clip.width, clip.height), (10, 10), "{format}");
        assert!(!clip.frames.is_empty(), "{format}");
    }
}

#[test]
fn a_movie_without_ffmpeg_on_the_path_says_it_is_not_installed() {
    let dir = tempfile::tempdir().expect("dir");
    let file = square(dir.path());
    let out = dir.path().join("movie.mkv").to_string_lossy().into_owned();
    let output = Command::new(env!("CARGO_BIN_EXE_flare"))
        .env("PATH", "")
        .args(["export", &file, &out, "--format", "mkv"])
        .output()
        .expect("run cli");
    assert!(!output.status.success(), "no ffmpeg, no movie");
    let err = String::from_utf8_lossy(&output.stderr);
    assert!(err.contains("ffmpeg"), "{err}");
    assert!(!Path::new(&out).exists(), "nothing is left behind");
}

#[test]
fn a_quicktime_movie_needs_no_ffmpeg() {
    let dir = tempfile::tempdir().expect("dir");
    let file = square(dir.path());
    let out = dir.path().join("movie.mov").to_string_lossy().into_owned();
    let output = Command::new(env!("CARGO_BIN_EXE_flare"))
        .env("PATH", "")
        .args(["export", &file, &out, "--format", "mov"])
        .output()
        .expect("run cli");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = std::fs::read(&out).expect("mov");
    assert_eq!(
        bytes.get(4..12),
        Some(b"ftypqt  ".as_slice()),
        "QuickTime's brand"
    );
}

#[test]
fn export_writes_an_animated_webp_that_is_a_webp_file() {
    let dir = tempfile::tempdir().expect("dir");
    let file = square(dir.path());
    let out = dir.path().join("anim.webp").to_string_lossy().into_owned();
    json(&["export", &file, &out, "--format", "animated-webp"]);
    let bytes = std::fs::read(&out).expect("webp");
    assert!(bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"));
    assert!(bytes.windows(4).any(|w| w == b"ANMF"), "an animation frame");
}
