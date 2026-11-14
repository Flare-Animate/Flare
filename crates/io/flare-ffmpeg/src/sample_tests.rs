//! Probing and sampling against stand-in programs: small shell scripts in the place of ffprobe
//! and ffmpeg that answer as they do, so the arguments, the turn, the chunking, the stop and the
//! failures are checked on a computer without ffmpeg.
#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};

use crate::error::FfmpegError;
use crate::sample::{
    MAX_SAMPLE_SIDE, Sampling, parse_rotation, parse_seconds, probe_with, read_frames_with,
    sample_args,
};
use crate::tool::Tools;

#[cfg(unix)]
fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write script");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    path
}

/// An ffprobe that reports a 1920x1080 movie at 29.97 frames a second, 12.5 seconds long, turned
/// by `rotation` (nothing when empty).
#[cfg(unix)]
fn fake_probe(dir: &Path, rotation: &str) -> PathBuf {
    script(
        dir,
        "ffprobe",
        &format!(
            r#"case "$*" in
  *format=duration*) echo "12.500000" ;;
  *stream_side_data*) echo "{rotation}" ;;
  *) echo "1920,1080,30000/1001,30000/1001" ;;
esac"#
        ),
    )
}

/// An ffmpeg that keeps its arguments in `args` and writes `bytes` zero bytes, then exits with
/// `status` after saying `said` on its error output.
#[cfg(unix)]
fn fake_ffmpeg(dir: &Path, bytes: usize, status: u8, said: &str) -> PathBuf {
    let args = dir.join("args");
    script(
        dir,
        "ffmpeg",
        &format!(
            "printf '%s\\n' \"$@\" > '{}'\nhead -c {bytes} /dev/zero\necho '{said}' >&2\nexit {status}",
            args.display()
        ),
    )
}

#[cfg(unix)]
fn tools(dir: &Path, rotation: &str, bytes: usize, status: u8, said: &str) -> Tools {
    Tools {
        ffprobe: fake_probe(dir, rotation),
        ffmpeg: fake_ffmpeg(dir, bytes, status, said),
    }
}

const SAMPLING: Sampling = Sampling {
    rate: (12, 1),
    size: (4, 2),
    max_frames: 10,
};
/// One 4x2 RGBA picture.
const PICTURE: usize = 4 * 2 * 4;

#[cfg(unix)]
#[test]
fn a_probe_gives_the_size_rate_and_length_with_a_quarter_turn_swapping_the_sides() {
    let dir = tempfile::tempdir().expect("dir");
    let movie = dir.path().join("clip.mp4");
    let upright = probe_with(&tools(dir.path(), "", 0, 0, ""), &movie).expect("probe");
    assert_eq!((upright.width, upright.height), (1920, 1080));
    assert_eq!(upright.rate, (30000, 1001));
    assert_eq!(upright.seconds, Some(12.5));
    let turned = probe_with(&tools(dir.path(), "-90", 0, 0, ""), &movie).expect("probe");
    assert_eq!(
        (turned.width, turned.height),
        (1080, 1920),
        "a phone movie is shown upright"
    );
    let half = probe_with(&tools(dir.path(), "180", 0, 0, ""), &movie).expect("probe");
    assert_eq!(
        (half.width, half.height),
        (1920, 1080),
        "a half turn keeps the sides"
    );
}

#[cfg(unix)]
#[test]
fn pictures_arrive_one_at_a_time_and_a_piece_short_of_one_at_the_end_is_dropped() {
    let dir = tempfile::tempdir().expect("dir");
    let tools = tools(dir.path(), "", PICTURE * 2 + PICTURE / 2, 0, "");
    let mut got = Vec::new();
    let count = read_frames_with(&tools, &dir.path().join("clip.mp4"), SAMPLING, |p| {
        got.push(p.len());
        true
    })
    .expect("read");
    assert_eq!(count, 2);
    assert_eq!(got, vec![PICTURE, PICTURE]);
    let args = std::fs::read_to_string(dir.path().join("args")).expect("args");
    let args: Vec<&str> = args.lines().collect();
    assert!(
        args.windows(2).any(|w| w == ["-vf", "fps=12/1,scale=4:2"]),
        "the rate is taken before the pictures are scaled: {args:?}"
    );
    assert!(
        args.windows(2).any(|w| w == ["-frames:v", "10"]),
        "{args:?}"
    );
    assert!(
        args.windows(2).any(|w| w == ["-pix_fmt", "rgba"]),
        "{args:?}"
    );
    assert!(args.contains(&"-an"), "the sound is not read: {args:?}");
}

#[cfg(unix)]
#[test]
fn a_stop_ends_the_reading_and_says_so() {
    let dir = tempfile::tempdir().expect("dir");
    let tools = tools(dir.path(), "", PICTURE * 5, 0, "");
    let mut seen = 0;
    let result = read_frames_with(&tools, &dir.path().join("clip.mp4"), SAMPLING, |_| {
        seen += 1;
        seen < 2
    });
    assert!(matches!(result, Err(FfmpegError::Stopped)), "{result:?}");
    assert_eq!(seen, 2, "nothing is handed over after the stop");
}

#[cfg(unix)]
#[test]
fn no_more_than_the_pictures_asked_for_are_taken_even_when_ffmpeg_writes_more() {
    let dir = tempfile::tempdir().expect("dir");
    let tools = tools(dir.path(), "", PICTURE * 6, 0, "");
    let sampling = Sampling {
        max_frames: 3,
        ..SAMPLING
    };
    let mut seen = 0;
    let count = read_frames_with(&tools, &dir.path().join("clip.mp4"), sampling, |_| {
        seen += 1;
        true
    })
    .expect("read");
    assert_eq!((count, seen), (3, 3));
}

#[cfg(unix)]
#[test]
fn a_failing_ffmpeg_is_reported_with_what_it_said_and_a_movie_with_no_pictures_too() {
    let dir = tempfile::tempdir().expect("dir");
    let movie = dir.path().join("clip.mp4");
    let failing = tools(dir.path(), "", 0, 1, "Invalid data found");
    let error = read_frames_with(&failing, &movie, SAMPLING, |_| true).expect_err("fails");
    assert!(error.to_string().contains("Invalid data found"), "{error}");
    let empty = tools(dir.path(), "", 0, 0, "");
    let error = read_frames_with(&empty, &movie, SAMPLING, |_| true).expect_err("fails");
    assert!(error.to_string().contains("no pictures"), "{error}");
}

#[test]
fn a_sampling_outside_the_bounds_is_refused_before_anything_runs() {
    let movie = Path::new("/tmp/clip.mp4");
    for bad in [
        Sampling {
            rate: (0, 1),
            ..SAMPLING
        },
        Sampling {
            size: (0, 2),
            ..SAMPLING
        },
        Sampling {
            size: (MAX_SAMPLE_SIDE + 1, 2),
            ..SAMPLING
        },
        Sampling {
            max_frames: 0,
            ..SAMPLING
        },
        Sampling {
            size: (4096, 4096),
            max_frames: 100,
            ..SAMPLING
        },
    ] {
        assert!(
            matches!(sample_args(movie, bad), Err(FfmpegError::Refused(_))),
            "{bad:?}"
        );
    }
    assert!(matches!(
        sample_args(Path::new("-i"), SAMPLING),
        Err(FfmpegError::Refused(_))
    ));
}

#[test]
fn a_length_and_a_turn_are_read_from_what_ffprobe_prints() {
    assert_eq!(parse_seconds("92.458333\n"), Some(92.458333));
    assert_eq!(parse_seconds("N/A\n"), None);
    assert_eq!(parse_seconds("0\n"), None);
    assert_eq!(parse_seconds("inf\n"), None);
    assert_eq!(parse_rotation("-90\n"), Some(-90));
    assert_eq!(parse_rotation("\n90\n"), Some(90));
    assert_eq!(parse_rotation(""), None);
    assert_eq!(parse_rotation("1e9"), None);
}
