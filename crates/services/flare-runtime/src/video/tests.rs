#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId, SoundId};
use flare_document::{Content, FrameSpan, Layer, Scene, Shape, Sound, SoundCue, Stage, Timeline};

use super::*;

fn animation(frames: u32, stage: (u32, u32)) -> Document {
    let shape = Shape::rect(
        ShapeId::new(),
        0.0,
        0.0,
        20.0,
        20.0,
        Rgba8::opaque(255, 0, 0),
    )
    .expect("rect");
    let span = FrameSpan::new(
        0,
        NonZeroU32::new(frames).expect("nonzero"),
        Content::Shape(shape),
    );
    let layer = Layer::new(LayerId::new(), "L", vec![span]).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document")
    .with_stage(Stage::new(stage.0, stage.1, Rgba8::WHITE).expect("stage"))
}

/// A half second 440 Hz tone as a 16-bit mono WAV at 8 kHz.
fn tone_wav() -> Vec<u8> {
    let samples: Vec<i16> = (0..4000)
        .map(|i| ((f64::from(i) * 440.0 * std::f64::consts::TAU / 8000.0).sin() * 12000.0) as i16)
        .collect();
    let data_len = (samples.len() * 2) as u32;
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&8000u32.to_le_bytes());
    wav.extend_from_slice(&16000u32.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        wav.extend_from_slice(&sample.to_le_bytes());
    }
    wav
}

/// `animation` with the tone cued at `start`.
fn with_sound(frames: u32, start: u32) -> Document {
    let doc = animation(frames, (64, 48));
    let sound = Sound::new(SoundId::new(), "tone", tone_wav(), 8000, 1, 4000).expect("sound");
    let id = sound.id();
    let scene = doc.scenes()[0].clone();
    let layer = scene.timeline().layers()[0]
        .clone()
        .with_cues(vec![SoundCue { start, sound: id }])
        .expect("cues");
    let scene = Scene::new(scene.id(), "S", Timeline::new(vec![layer]));
    Document::new(doc.id(), doc.frame_rate(), vec![scene], vec![])
        .expect("document")
        .with_stage(doc.stage())
        .with_sounds(vec![sound])
        .expect("sounds")
}

#[test]
fn the_video_is_even_in_both_directions_and_never_larger_than_the_limit() {
    assert_eq!(video_size(640, 360), (640, 360));
    assert_eq!(video_size(641, 361), (640, 360), "odd sizes are made even");
    assert_eq!(video_size(3840, 2160), (1920, 1080));
    assert_eq!(video_size(1, 1), (2, 2));
    assert_eq!(video_size(4000, 1), (1920, 2));
    let (w, h) = video_size(1919, 1081);
    assert!(w % 2 == 0 && h % 2 == 0 && w <= MAX_VIDEO_SIDE && h <= MAX_VIDEO_SIDE);
}

fn text(args: &[OsString]) -> Vec<String> {
    args.iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn ffmpeg_gets_one_argument_per_item_with_the_frames_on_its_input() {
    let out = Path::new("/tmp/out.part");
    let silent = text(&ffmpeg_args((640, 360), (24000, 1001), None, out));
    let joined = silent.join(" ");
    for wanted in [
        "-f rawvideo",
        "-pix_fmt rgba",
        "-s 640x360",
        "-framerate 24000/1001",
        "-i pipe:0",
        "-c:v libx264",
        "-pix_fmt yuv420p",
        "-movflags +faststart",
        "-f mp4",
    ] {
        assert!(joined.contains(wanted), "{wanted}: {joined}");
    }
    assert_eq!(
        silent.last().map(String::as_str),
        Some("/tmp/out.part"),
        "the output is last"
    );
    assert!(
        !joined.contains("aac") && !joined.contains("f32le"),
        "no sound, no audio input"
    );
    let sound = text(&ffmpeg_args(
        (640, 360),
        (24, 1),
        Some(Path::new("/tmp/a.raw")),
        out,
    ));
    let joined = sound.join(" ");
    for wanted in [
        "-f f32le",
        "-ar 48000",
        "-ac 2",
        "-i /tmp/a.raw",
        "-c:a aac",
        "-shortest",
    ] {
        assert!(joined.contains(wanted), "{wanted}: {joined}");
    }
    assert!(
        sound
            .iter()
            .all(|a| !a.contains(' ') || a.contains('x') || a.starts_with('/')),
        "no argument is a command line in disguise"
    );
}

#[test]
fn ffmpeg_is_found_by_variable_or_on_an_absolute_path_and_a_bad_variable_does_not_fall_back() {
    let dir = tempfile::tempdir().expect("dir");
    let name = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    let program = dir.path().join(name);
    std::fs::write(&program, "x").expect("write");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    let path = std::env::join_paths([dir.path()]).expect("join");
    let by_path = find_ffmpeg_with(|k| (k == "PATH").then(|| path.clone()));
    assert_eq!(by_path.as_deref(), Some(program.as_path()));
    let by_variable = find_ffmpeg_with(|k| (k == "FLARE_FFMPEG").then(|| program.clone().into()));
    assert_eq!(by_variable.as_deref(), Some(program.as_path()));
    // A variable that names nothing is an answer, not a reason to look elsewhere.
    let missing = find_ffmpeg_with(|k| match k {
        "FLARE_FFMPEG" => Some("/no/such/ffmpeg".into()),
        "PATH" => Some(path.clone()),
        _ => None,
    });
    assert_eq!(missing, None);
    // A relative directory on PATH is ignored.
    let relative = find_ffmpeg_with(|k| (k == "PATH").then(|| "bin:.".into()));
    assert_eq!(relative, None);
}

#[test]
fn a_video_with_too_many_frames_or_an_existing_file_is_refused_before_anything_runs() {
    let dir = tempfile::tempdir().expect("dir");
    let target = dir.path().join("a.mp4");
    let long = animation(u32::try_from(MAX_VIDEO_FRAMES + 1).expect("fits"), (20, 20));
    let error = write_mp4(&long, Path::new("/no/ffmpeg"), &target).expect_err("too long");
    assert!(error.contains("limited"), "{error}");
    std::fs::write(&target, b"mine").expect("write");
    let error =
        write_mp4(&animation(3, (20, 20)), Path::new("/no/ffmpeg"), &target).expect_err("exists");
    assert!(error.contains("already exists"), "{error}");
    assert_eq!(std::fs::read(&target).expect("read"), b"mine");
    let error = write_mp4(
        &animation(3, (20, 20)),
        Path::new("/no/ffmpeg"),
        &dir.path().join("b.mp4"),
    )
    .expect_err("no program");
    assert!(error.contains("Cannot start ffmpeg"), "{error}");
}

#[test]
fn the_sound_is_mixed_in_at_its_cue_and_a_film_without_sound_writes_none() {
    let mut bytes = Vec::new();
    assert!(!mix_sound(&animation(24, (20, 20)), 24, (24, 1), &mut bytes).expect("mix"));
    assert!(bytes.is_empty());
    // 48 frames at 24 fps is two seconds; the tone is cued at frame 24, one second in.
    let doc = with_sound(48, 24);
    assert!(mix_sound(&doc, 48, (24, 1), &mut bytes).expect("mix"));
    assert_eq!(bytes.len(), 2 * 48_000 * 2 * 4, "two seconds of stereo f32");
    let at = |seconds: f64| {
        let i = ((seconds * 48_000.0) as usize) * 8;
        f32::from_le_bytes(bytes[i..i + 4].try_into().expect("four bytes"))
    };
    let loudest = |from: f64, to: f64| {
        (0..2000)
            .map(|k| at(from + (to - from) * f64::from(k) / 2000.0).abs())
            .fold(0.0f32, f32::max)
    };
    assert!(loudest(0.0, 0.9) < 1e-6, "silent before the cue");
    assert!(loudest(1.05, 1.4) > 0.1, "the tone plays after it");
    assert!(loudest(1.7, 2.0) < 1e-6, "and has ended");
}

#[cfg(unix)]
fn script(dir: &Path, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join("fake-ffmpeg");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    path
}

#[cfg(unix)]
#[test]
fn what_ffmpeg_is_fed_is_every_frame_at_the_video_size_and_its_output_is_kept_whole() {
    let dir = tempfile::tempdir().expect("dir");
    let args_file = dir.path().join("args.txt");
    // A stand-in that records its arguments and copies its input to the output (the last argument).
    let fake = script(
        dir.path(),
        &format!(
            "printf '%s\\n' \"$@\" > '{}'\nfor last; do :; done\ncat > \"$last\"",
            args_file.display()
        ),
    );
    let out_dir = tempfile::tempdir().expect("out");
    let target = out_dir.path().join("film.mp4");
    // 5 frames of an odd-sized stage: the video is made even.
    write_mp4(&animation(5, (33, 21)), &fake, &target).expect("encoded");
    let written = std::fs::read(&target).expect("the video appears");
    assert_eq!(
        written.len(),
        5 * 32 * 20 * 4,
        "five RGBA frames of 32 x 20"
    );
    let args = std::fs::read_to_string(&args_file).expect("args");
    assert!(args.contains("32x20") && args.contains("libx264"), "{args}");
    let left: Vec<_> = std::fs::read_dir(out_dir.path())
        .expect("ls")
        .map(|e| e.expect("e").file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(left, ["film.mp4"], "no staging file is left behind");
}

#[cfg(unix)]
#[test]
fn a_failing_ffmpeg_reports_why_and_leaves_no_file() {
    let dir = tempfile::tempdir().expect("dir");
    let fake = script(
        dir.path(),
        "cat > /dev/null\necho 'Unknown encoder libx264' >&2\nexit 3",
    );
    let out_dir = tempfile::tempdir().expect("out");
    let target = out_dir.path().join("film.mp4");
    let error = write_mp4(&animation(4, (20, 20)), &fake, &target).expect_err("fails");
    assert!(error.contains("Unknown encoder libx264"), "{error}");
    assert_eq!(
        std::fs::read_dir(out_dir.path()).expect("ls").count(),
        0,
        "nothing left"
    );
}

#[cfg(unix)]
#[test]
fn an_ffmpeg_that_quits_early_is_reported_and_does_not_hang_the_export() {
    let dir = tempfile::tempdir().expect("dir");
    let fake = script(dir.path(), "echo 'bad input' >&2\nexit 1");
    let out_dir = tempfile::tempdir().expect("out");
    let started = std::time::Instant::now();
    let error = write_mp4(
        &animation(200, (400, 300)),
        &fake,
        &out_dir.path().join("f.mp4"),
    )
    .expect_err("fails");
    assert!(error.contains("bad input"), "{error}");
    assert!(started.elapsed() < std::time::Duration::from_secs(20));
}

/// With a real ffmpeg (a machine that has one): the file is an MP4 with H.264, and AAC when there
/// is sound. On a machine without one this checks nothing, which the output says.
#[test]
fn with_a_real_ffmpeg_the_result_is_a_playable_mp4_with_its_sound() {
    let Some(ffmpeg) = find_ffmpeg() else {
        eprintln!("no ffmpeg here: the real-encoder check was not run");
        return;
    };
    let dir = tempfile::tempdir().expect("dir");
    let silent = dir.path().join("silent.mp4");
    write_mp4(&animation(12, (64, 48)), &ffmpeg, &silent).expect("encoded");
    let loud = dir.path().join("loud.mp4");
    write_mp4(&with_sound(12, 0), &ffmpeg, &loud).expect("encoded with sound");
    for file in [&silent, &loud] {
        let bytes = std::fs::read(file).expect("read");
        assert!(
            bytes.len() > 500 && &bytes[4..8] == b"ftyp",
            "an MP4: {file:?}"
        );
    }
    let probe = ffmpeg.with_file_name(if cfg!(windows) {
        "ffprobe.exe"
    } else {
        "ffprobe"
    });
    if probe.exists() {
        let codecs = |file: &Path| {
            let out = Command::new(&probe)
                .args([
                    "-v",
                    "error",
                    "-show_entries",
                    "stream=codec_name",
                    "-of",
                    "csv=p=0",
                ])
                .arg(file)
                .output()
                .expect("probe");
            String::from_utf8_lossy(&out.stdout).into_owned()
        };
        let silent_codecs = codecs(&silent);
        assert!(
            silent_codecs.contains("h264") && !silent_codecs.contains("aac"),
            "{silent_codecs}"
        );
        let loud_codecs = codecs(&loud);
        assert!(
            loud_codecs.contains("h264") && loud_codecs.contains("aac"),
            "{loud_codecs}"
        );
    }
}
