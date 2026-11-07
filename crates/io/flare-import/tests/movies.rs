//! Movies as projects: a WebM written by ffmpeg and an MP4 written natively come in one picture per
//! frame at their rate; a movie is refused by the bytes reader; a damaged file is refused with its
//! name. The ffmpeg cases check nothing where ffmpeg is not installed, and say so.
#![allow(clippy::expect_used)]

use std::path::Path;

use flare_ffmpeg::{Movie, container, encode};
use flare_import::{import_foreign_bytes, import_foreign_path};

fn have_ffmpeg() -> bool {
    let installed = flare_ffmpeg::locate().is_ok();
    if !installed {
        eprintln!("ffmpeg is not installed here: these tests check nothing");
    }
    installed
}

/// A red picture of `level`, 16 by 16.
fn red(level: u8) -> Vec<u8> {
    [level, 0, 0, 255].repeat(16 * 16)
}

#[test]
fn a_webm_from_ffmpeg_imports_one_picture_per_frame_at_its_rate() {
    if !have_ffmpeg() {
        return;
    }
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("clip.webm");
    let movie = Movie {
        size: (16, 16),
        rate: (12, 1),
        frames: 5,
        sound: None,
    };
    encode(container("webm").expect("webm"), &path, &movie, |n| {
        Ok(red(40 + 30 * n as u8))
    })
    .expect("write");
    let imported = import_foreign_path(&path).expect("import");
    let doc = &imported.document;
    assert_eq!((doc.stage().width(), doc.stage().height()), (16, 16));
    assert_eq!(
        doc.scenes()[0].timeline().frame_count(),
        5,
        "one frame per picture"
    );
    assert!(
        imported
            .diagnostics
            .iter()
            .any(|d| d.message.contains("sound is not imported")),
        "{:?}",
        imported.diagnostics
    );
}

#[test]
fn an_mp4_written_by_flare_imports_back_at_its_size() {
    // The MP4 writer is native: this checks that its file reads back through ffmpeg, so it needs
    // ffmpeg to read the file.
    if !have_ffmpeg() {
        return;
    }
    let dir = tempfile::tempdir().expect("dir");
    let png_bytes = {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, 16, 16);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .expect("header")
            .write_image_data(&red(200))
            .expect("data");
        out
    };
    let document = import_foreign_bytes(Path::new("still.png"), png_bytes)
        .expect("a still")
        .document;
    let path = dir.path().join("made.mp4");
    flare_video::write_mp4(&document, &path).expect("mp4");
    let imported = import_foreign_path(&path).expect("import");
    assert_eq!(imported.document.stage().width(), 16);
    assert!(imported.document.scenes()[0].timeline().frame_count() >= 1);
}

#[test]
fn a_movie_is_refused_by_the_bytes_reader_and_says_it_is_read_from_its_file() {
    let error = import_foreign_bytes(Path::new("clip.mkv"), vec![1, 2, 3]).expect_err("refused");
    assert!(error.to_string().contains("read from its file"), "{error}");
}

#[test]
fn a_damaged_movie_is_refused_with_its_name() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("broken.mov");
    std::fs::write(&path, b"this is not a movie").expect("write");
    let error = import_foreign_path(&path).expect_err("refused");
    assert!(error.to_string().contains("broken.mov"), "{error}");
}
