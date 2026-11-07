//! Movies written and read through the real ffmpeg. Where ffmpeg is not on the computer these
//! tests check nothing, and they say so.
#![allow(clippy::expect_used)]

use std::path::Path;

use flare_ffmpeg::{FfmpegError, Movie, container, decode, encode, locate};

/// A frame of one flat red level, so a lossy codec's average can be checked.
fn red(level: u8) -> Vec<u8> {
    [level, 0, 0, 255].repeat(16 * 16)
}

fn have_ffmpeg() -> bool {
    match locate() {
        Ok(_) => true,
        Err(_) => {
            eprintln!("ffmpeg is not installed here: these tests check nothing");
            false
        }
    }
}

#[test]
fn each_container_writes_a_movie_that_reads_back_with_its_frames_and_size() {
    if !have_ffmpeg() {
        return;
    }
    let dir = tempfile::tempdir().expect("dir");
    for id in ["mov", "avi", "mkv", "webm", "mpeg", "flv"] {
        let c = container(id).expect("known");
        let out = dir.path().join(format!("movie.{}", c.extension));
        let movie = Movie {
            size: (16, 16),
            rate: (12, 1),
            frames: 4,
            sound: None,
        };
        encode(c, &out, &movie, |n| Ok(red(40 + 40 * n as u8)))
            .unwrap_or_else(|e| panic!("{id}: {e}"));
        let clip = decode(&out, 100, 1920).unwrap_or_else(|e| panic!("{id}: {e}"));
        assert_eq!((clip.width, clip.height), (16, 16), "{id}");
        assert_eq!(clip.frames.len(), 4, "{id}: every frame comes back");
        // FLV keeps time in milliseconds, so its rate comes back close to 12 but not exactly.
        let fps = f64::from(clip.rate.0) / f64::from(clip.rate.1);
        assert!((fps - 12.0).abs() < 0.05, "{id}: rate {fps}");
        // A lossy codec keeps a flat color's level within a few steps.
        let first_red = clip.frames[0]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| u32::from(p[0]))
            .sum::<u32>()
            / 256;
        assert!(
            first_red.abs_diff(40) <= 12,
            "{id}: first frame red {first_red}"
        );
    }
}

#[test]
fn a_movie_with_sound_has_its_sound_track_written() {
    if !have_ffmpeg() {
        return;
    }
    let dir = tempfile::tempdir().expect("dir");
    let sound = dir.path().join("sound.pcm");
    // Half a second of a 440 Hz tone, 16-bit stereo at the sound rate.
    let samples: Vec<u8> = (0..24_000)
        .flat_map(|i: u32| {
            let v =
                ((f64::from(i) * 440.0 * std::f64::consts::TAU / 48_000.0).sin() * 8000.0) as i16;
            let b = v.to_le_bytes();
            [b[0], b[1], b[0], b[1]]
        })
        .collect();
    std::fs::write(&sound, &samples).expect("pcm");
    let out = dir.path().join("with-sound.mkv");
    let movie = Movie {
        size: (16, 16),
        rate: (12, 1),
        frames: 6,
        sound: Some(&sound),
    };
    encode(container("mkv").expect("mkv"), &out, &movie, |n| {
        Ok(red(50 + 10 * n as u8))
    })
    .expect("mkv");
    let probe = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "a",
            "-show_entries",
            "stream=codec_name",
            "-of",
            "csv=p=0",
        ])
        .arg(&out)
        .output()
        .expect("ffprobe");
    assert!(
        String::from_utf8_lossy(&probe.stdout).contains("opus")
            || String::from_utf8_lossy(&probe.stdout).contains("aac"),
        "an audio track: {:?}",
        probe.stdout
    );
}

#[test]
fn a_frame_of_the_wrong_size_stops_the_write_and_says_which() {
    if !have_ffmpeg() {
        return;
    }
    let dir = tempfile::tempdir().expect("dir");
    let out = dir.path().join("bad.webm");
    let movie = Movie {
        size: (16, 16),
        rate: (12, 1),
        frames: 2,
        sound: None,
    };
    let error = encode(container("webm").expect("webm"), &out, &movie, |n| {
        Ok(if n == 0 { red(10) } else { vec![0; 8] })
    })
    .expect_err("refused");
    assert!(
        matches!(error, FfmpegError::Refused(ref m) if m.contains("frame 2")),
        "{error}"
    );
}

#[test]
fn a_write_to_a_folder_that_does_not_exist_reports_what_ffmpeg_said() {
    if !have_ffmpeg() {
        return;
    }
    let dir = tempfile::tempdir().expect("dir");
    let out = dir.path().join("no-such-folder").join("x.mov");
    let movie = Movie {
        size: (16, 16),
        rate: (12, 1),
        frames: 1,
        sound: None,
    };
    let error =
        encode(container("mov").expect("mov"), &out, &movie, |_| Ok(red(1))).expect_err("fails");
    assert!(
        matches!(
            error,
            FfmpegError::Failed {
                action: "write the movie",
                ..
            }
        ),
        "{error}"
    );
}

#[test]
fn reading_a_file_that_is_not_a_movie_fails_and_a_frame_limit_is_kept() {
    if !have_ffmpeg() {
        return;
    }
    let dir = tempfile::tempdir().expect("dir");
    let junk = dir.path().join("junk.mov");
    std::fs::write(&junk, b"not a movie at all").expect("write");
    assert!(decode(Path::new(&junk), 10, 100).is_err());

    let out = dir.path().join("ten.webm");
    let movie = Movie {
        size: (16, 16),
        rate: (12, 1),
        frames: 10,
        sound: None,
    };
    encode(container("webm").expect("webm"), &out, &movie, |n| {
        Ok(red(n as u8 * 20))
    })
    .expect("write");
    let clip = decode(&out, 3, 1920).expect("read");
    assert_eq!(clip.frames.len(), 3, "no more than the frame limit is read");
}
