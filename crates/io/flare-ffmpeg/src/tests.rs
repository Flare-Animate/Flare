//! The parts of the ffmpeg bridge that need no ffmpeg: the container table, the arguments it
//! builds, the probe's answers, the size a picture is read at, and the refusals.
#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};

use crate::containers::{CONTAINERS, container};
use crate::decode::{fit, parse_probe};
use crate::encode::{Movie, encode, encode_args};
use crate::error::FfmpegError;
use crate::tool::find_in;

fn text(args: &[std::ffi::OsString]) -> Vec<String> {
    args.iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn every_container_has_its_own_id_and_the_lookup_ignores_case() {
    let mut ids: Vec<&str> = CONTAINERS.iter().map(|c| c.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), CONTAINERS.len(), "ids are unique");
    assert_eq!(container("WEBM").map(|c| c.muxer), Some("webm"));
    assert!(
        container("animated-webp").is_none(),
        "animated WebP is written in pure Rust"
    );
    assert!(
        container("mp4").is_none(),
        "MP4 is written natively, not here"
    );
    assert!(container("").is_none());
}

#[test]
fn the_arguments_name_the_codec_muxer_and_sound_of_each_container() {
    let movie = Movie {
        size: (16, 8),
        rate: (24, 1),
        frames: 3,
        sound: None,
    };
    let webm = encode_args(
        container("webm").expect("webm"),
        Path::new("/tmp/o.webm"),
        &movie,
    )
    .expect("args");
    let webm = text(&webm);
    assert!(
        webm.windows(2).any(|w| w == ["-c:v", "libvpx-vp9"]),
        "{webm:?}"
    );
    assert!(webm.windows(2).any(|w| w == ["-f", "webm"]), "{webm:?}");
    assert!(!webm.iter().any(|a| a == "-c:a"), "no sound was asked for");
    assert_eq!(webm.last().map(String::as_str), Some("/tmp/o.webm"));

    let with_sound = Movie {
        sound: Some(Path::new("/tmp/s.pcm")),
        ..movie
    };
    let mov = text(
        &encode_args(
            container("mov").expect("mov"),
            Path::new("/tmp/o.mov"),
            &with_sound,
        )
        .expect("args"),
    );
    assert!(mov.windows(2).any(|w| w == ["-c:a", "aac"]), "{mov:?}");
    assert!(
        mov.windows(2).any(|w| w == ["-map", "1:a:0"]),
        "the sound is mapped"
    );
    assert!(mov.iter().any(|a| a == "-shortest"));
}

#[test]
fn a_file_name_that_looks_like_an_option_is_refused_before_ffmpeg_runs() {
    let movie = Movie {
        size: (2, 2),
        rate: (1, 1),
        frames: 1,
        sound: None,
    };
    let error = encode(
        container("mov").expect("mov"),
        Path::new("-evil.mov"),
        &movie,
        |_| Ok(vec![0; 16]),
    )
    .expect_err("refused");
    assert!(
        matches!(error, FfmpegError::Refused(ref m) if m.contains("dash")),
        "{error}"
    );
}

#[test]
fn a_movie_outside_the_bounds_is_refused_in_words() {
    let c = container("mov").expect("mov");
    for movie in [
        Movie {
            size: (0, 2),
            rate: (1, 1),
            frames: 1,
            sound: None,
        },
        Movie {
            size: (4000, 2),
            rate: (1, 1),
            frames: 1,
            sound: None,
        },
        Movie {
            size: (2, 2),
            rate: (1, 1),
            frames: 0,
            sound: None,
        },
        Movie {
            size: (2, 2),
            rate: (1, 1),
            frames: 1_000_000,
            sound: None,
        },
        Movie {
            size: (2, 2),
            rate: (0, 1),
            frames: 1,
            sound: None,
        },
    ] {
        let error =
            encode(c, Path::new("/tmp/x.mov"), &movie, |_| Ok(vec![0; 16])).expect_err("refused");
        assert!(matches!(error, FfmpegError::Refused(_)), "{error}");
    }
}

#[test]
fn a_probe_line_gives_the_size_and_the_average_rate_or_the_base_rate() {
    let source = parse_probe("640,480,30000/1001,30000/1001\n").expect("probe");
    assert_eq!(
        (source.width, source.height, source.rate),
        (640, 480, (30000, 1001))
    );
    let base = parse_probe("320,240,0/0,24/1").expect("probe");
    assert_eq!(base.rate, (24, 1), "0/0 falls back to the base rate");
    assert!(parse_probe("not a picture").is_err());
    // A stream with no size is passed on: the reader takes the first frame's size instead.
    let sizeless = parse_probe("0,0,25/1,25/1").expect("probe");
    assert_eq!((sizeless.width, sizeless.height), (0, 0));
    assert!(
        parse_probe("640,480,24/0,24/0").is_err(),
        "a zero denominator is no rate"
    );
}

#[test]
fn a_picture_is_read_at_its_size_or_scaled_down_to_fit_with_even_sides() {
    assert_eq!(fit(640, 480, 1920).expect("fit"), (640, 480));
    assert_eq!(fit(3840, 2160, 1920).expect("fit"), (1920, 1080));
    assert_eq!(
        fit(1001, 3, 1000).expect("fit"),
        (1000, 2),
        "odd sides become even"
    );
    assert!(fit(0, 3, 1000).is_err());
}

#[test]
fn a_missing_program_is_reported_as_not_installed() {
    let empty = std::env::temp_dir().join("flare-ffmpeg-no-such-directory-xyz");
    let path = std::env::join_paths([empty.as_path()]).expect("join");
    assert!(
        find_in(&path, "ffmpeg").is_none(),
        "nothing is found on an empty PATH"
    );
    let found: Option<PathBuf> = find_in(
        &std::env::join_paths(std::iter::empty::<PathBuf>()).expect("empty"),
        "ffmpeg",
    );
    assert!(found.is_none());
}
