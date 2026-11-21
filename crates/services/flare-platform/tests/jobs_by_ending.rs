//! The picture and movie jobs name their format by the ending of the file's name.
#![allow(clippy::expect_used)]

use flare_core::{DocumentId, FrameRate, LayerId, SceneId};
use flare_document::{Document, Layer, Scene, Timeline};
use flare_platform::jobs;

fn document() -> Document {
    let layer = Layer::new(LayerId::new(), "L", vec![]).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document")
}

#[test]
fn an_image_is_written_in_the_format_its_ending_names_and_a_movie_ending_is_refused() {
    let dir = tempfile::tempdir().expect("dir");
    jobs::write_image(
        &document(),
        0,
        &dir.path().join("a.JPG"),
        jobs::Backdrop::Stage,
    )
    .expect("jpeg");
    let jpeg = std::fs::read(dir.path().join("a.JPG")).expect("jpeg");
    assert_eq!(&jpeg[..2], &[0xFF, 0xD8], "a JPEG starts with its marker");

    let error = jobs::write_image(
        &document(),
        0,
        &dir.path().join("b.gif"),
        jobs::Backdrop::Stage,
    )
    .expect_err("a GIF is a movie, not an image");
    assert!(error.contains(".gif"), "{error}");
    assert!(
        !dir.path().join("b.gif").exists(),
        "a refused image writes no file"
    );
}

#[test]
fn an_image_with_no_ending_is_a_png_and_a_jpeg_over_a_transparent_backdrop_is_refused_clear() {
    let dir = tempfile::tempdir().expect("dir");
    jobs::write_image(
        &document(),
        0,
        &dir.path().join("plain"),
        jobs::Backdrop::Stage,
    )
    .expect("png");
    let bytes = std::fs::read(dir.path().join("plain")).expect("png");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "no ending writes a PNG");

    let error = jobs::write_image(
        &document(),
        0,
        &dir.path().join("see.jpg"),
        jobs::Backdrop::Transparent,
    )
    .expect_err("a JPEG has no alpha");
    assert!(error.contains("JPEG"), "{error}");
    assert!(!dir.path().join("see.jpg").exists());
}

#[test]
fn an_apng_movie_is_written_and_an_existing_one_is_never_replaced() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.apng");
    jobs::write_movie(&document(), &path, jobs::Backdrop::Stage).expect("apng");
    let bytes = std::fs::read(&path).expect("apng");
    assert!(
        bytes.windows(4).any(|w| w == b"acTL"),
        "an APNG has its animation chunk"
    );
    let error = jobs::write_movie(&document(), &path, jobs::Backdrop::Stage).expect_err("exists");
    assert!(error.contains("already exists"), "{error}");
}

#[test]
fn a_movie_ending_nothing_writes_is_refused_with_the_endings_it_takes() {
    let dir = tempfile::tempdir().expect("dir");
    let error = jobs::write_movie(
        &document(),
        &dir.path().join("a.svg"),
        jobs::Backdrop::Stage,
    )
    .expect_err("not a movie");
    for ending in ["gif", "mp4", "mov", "webm", "mkv", "avi", "flv"] {
        assert!(
            error.contains(ending),
            "the refusal lists .{ending}: {error}"
        );
    }
    assert!(!dir.path().join("a.svg").exists());
}

#[test]
fn the_dialog_lists_name_only_endings_the_writers_take() {
    for ending in jobs::IMAGE_ENDINGS {
        // Every offered ending is written, except that an ICO holds at most 256 pixels a side,
        // so the stage of this test is refused for it by that reason and no other.
        if let Err(error) =
            flare_runtime::picture_bytes(&document(), 0, ending, jobs::Backdrop::Stage)
        {
            assert!(
                ending == "ico" && error.contains("ICO holds at most 256 pixels a side"),
                "{ending} is offered but refused: {error}"
            );
        }
    }
    for ending in jobs::MOVIE_ENDINGS
        .iter()
        .chain(jobs::OTHER_MOVIE_ENDINGS.iter())
    {
        assert!(
            ["gif", "apng", "mp4", "mov"].contains(ending)
                || flare_ffmpeg::CONTAINERS
                    .iter()
                    .any(|c| { c.extension == *ending || (*ending == "mpeg" && c.id == "mpeg") }),
            "{ending} is offered but nothing writes it"
        );
    }
}
