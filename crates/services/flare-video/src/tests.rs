#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId, SoundId};
use flare_document::{Content, FrameSpan, Layer, Scene, Shape, Sound, SoundCue, Stage, Timeline};

use super::*;

fn animation(frames: u32, stage: (u32, u32)) -> Document {
    // A red square on white, so the picture has something to get wrong.
    let shape = Shape::rect(
        ShapeId::new(),
        10.0,
        10.0,
        40.0,
        40.0,
        Rgba8::opaque(230, 30, 30),
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

/// A tone as a 16-bit mono WAV at 8 kHz, `samples` long.
fn tone_wav(samples: usize) -> Vec<u8> {
    let data: Vec<i16> = (0..samples)
        .map(|i| ((i as f64 * 440.0 * std::f64::consts::TAU / 8000.0).sin() * 12000.0) as i16)
        .collect();
    let data_len = (data.len() * 2) as u32;
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&8000u32.to_le_bytes());
    wav.extend_from_slice(&16000u32.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for sample in data {
        wav.extend_from_slice(&sample.to_le_bytes());
    }
    wav
}

/// `animation` with a sound (`bytes`) cued at frame `start`.
fn with_sound(frames: u32, start: u32, bytes: Vec<u8>) -> Document {
    let doc = animation(frames, (64, 48));
    let sound = Sound::new(SoundId::new(), "tone", bytes, 8000, 1, 4000).expect("sound");
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
fn the_video_is_even_never_larger_than_the_limit_and_its_bit_rate_stays_in_bounds() {
    assert_eq!(video_size(640, 360), (640, 360));
    assert_eq!(video_size(641, 361), (640, 360));
    assert_eq!(video_size(3840, 2160), (1920, 1080));
    assert_eq!(video_size(1, 1), (2, 2));
    assert_eq!(video_size(4000, 1), (1920, 2));
    assert_eq!(video_bitrate((64, 48), 24.0), 400_000, "a floor");
    assert_eq!(video_bitrate((1920, 1080), 60.0), 12_441_600);
    assert_eq!(video_bitrate((1920, 1080), 240.0), 16_000_000, "a ceiling");
}

#[test]
fn colors_go_through_the_weights_a_player_will_read_them_with_and_come_back() {
    let small = color::Matrix::for_size(640, 360);
    let large = color::Matrix::for_size(1280, 720);
    assert_ne!(
        small, large,
        "standard and high definition are read differently"
    );
    assert_eq!(color::Matrix::for_size(1920, 1080), large);
    for matrix in [small, large] {
        for (r, g, b) in [
            (230.0, 30.0, 30.0),
            (0.0, 0.0, 0.0),
            (255.0, 255.0, 255.0),
            (20.0, 200.0, 90.0),
            (40.0, 60.0, 250.0),
        ] {
            let (y, cb, cr) = matrix.ycc(r, g, b);
            assert!(
                (16.0..=235.0).contains(&y)
                    && (16.0..=240.0).contains(&cb)
                    && (16.0..=240.0).contains(&cr)
            );
            let (r2, g2, b2) = matrix.rgb(y, cb, cr);
            assert!((r - r2).abs() < 0.5 && (g - g2).abs() < 0.5 && (b - b2).abs() < 0.5);
        }
    }
}

#[test]
fn a_keyframe_is_found_by_its_idr_slice_with_either_start_code() {
    let sps = [0, 0, 0, 1, 0x67, 1, 2, 3];
    let idr3 = [0, 0, 1, 0x65, 9, 9];
    let idr4 = [0, 0, 0, 1, 0x65, 9, 9];
    let p = [0, 0, 0, 1, 0x41, 9, 9];
    assert!(!is_keyframe(&sps));
    assert!(!is_keyframe(&p));
    assert!(is_keyframe(&idr3) && is_keyframe(&idr4));
    let mut both = sps.to_vec();
    both.extend_from_slice(&idr4);
    assert!(is_keyframe(&both), "a parameter set before the slice");
    assert!(!is_keyframe(&[]) && !is_keyframe(&[0, 0, 1]));
}

/// Encodes `frames` of `document` the way `write_mp4` does, and decodes them again with an
/// independent decoder.
fn round_trip(
    document: &Document,
    frames: u32,
) -> (
    Vec<Vec<u8>>,
    Vec<rusty_h264::YuvFrame>,
    Vec<rusty_h264::YuvFrame>,
) {
    let stage = document.stage();
    let size = video_size(stage.width(), stage.height());
    let drawn = Frames::new(document, size).expect("frames");
    let mut encoder = picture_encoder(size, 24.0).expect("encoder");
    let (mut units, mut sources, mut stream) = (Vec::new(), Vec::new(), Vec::new());
    for n in 0..frames {
        let yuv = color::to_i420(
            &drawn.rgba(n).expect("frame"),
            size.0 as usize,
            size.1 as usize,
        );
        let unit = encoder.encode(&yuv);
        stream.extend_from_slice(&unit);
        units.push(unit);
        sources.push(yuv);
    }
    let decoded = rusty_h264::Decoder::new()
        .decode_stream(&stream)
        .expect("decodes");
    (units, sources, decoded)
}

fn psnr(a: &[u8], b: &[u8]) -> f64 {
    let mse: f64 = a
        .iter()
        .zip(b)
        .map(|(x, y)| (f64::from(*x) - f64::from(*y)).powi(2))
        .sum::<f64>()
        / a.len() as f64;
    if mse == 0.0 {
        99.0
    } else {
        10.0 * (255.0f64 * 255.0 / mse).log10()
    }
}

#[test]
fn the_picture_decodes_with_every_frame_the_right_size_and_close_to_what_was_drawn() {
    let doc = animation(60, (96, 64));
    let (units, sources, decoded) = round_trip(&doc, 60);
    assert_eq!(decoded.len(), 60, "every frame comes back");
    assert!(decoded.iter().all(|f| (f.width, f.height) == (96, 64)));
    let keys: Vec<usize> = units
        .iter()
        .enumerate()
        .filter(|(_, u)| is_keyframe(u))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        keys,
        [0, 48],
        "a keyframe to start and one every two seconds"
    );
    for (source, back) in sources.iter().zip(&decoded) {
        assert!(
            psnr(&source.y, &back.y) > 35.0,
            "luma {}",
            psnr(&source.y, &back.y)
        );
        assert!(psnr(&source.u, &back.u) > 35.0 && psnr(&source.v, &back.v) > 35.0);
    }
    // The red square is red when read back the way a player reads it.
    let frame = &decoded[30];
    let (cx, cy) = (30usize, 30usize);
    let matrix = color::Matrix::for_size(96, 64);
    let (r, g, b) = matrix.rgb(
        f32::from(frame.y[cy * 96 + cx]),
        f32::from(frame.u[(cy / 2) * 48 + cx / 2]),
        f32::from(frame.v[(cy / 2) * 48 + cx / 2]),
    );
    assert!(
        (r - 230.0).abs() < 12.0 && (g - 30.0).abs() < 12.0 && (b - 30.0).abs() < 12.0,
        "{r} {g} {b}"
    );
}

#[test]
fn a_movie_with_no_cued_sound_has_no_track_and_a_cued_one_covers_the_whole_movie() {
    assert!(
        audio::Track::new(&animation(24, (64, 48)), (24, 1), 1.0)
            .expect("track")
            .is_none()
    );
    let doc = with_sound(48, 0, tone_wav(4000));
    let mut track = audio::Track::new(&doc, (24, 1), 2.0)
        .expect("track")
        .expect("a track");
    let mut starts = Vec::new();
    while let Some(start) = track.next_start() {
        starts.push(start);
        let frame = track.next_frame().expect("frame");
        assert!(
            frame.len() > 7 && frame[0] == 0xFF && frame[1] & 0xF0 == 0xF0,
            "an ADTS frame"
        );
    }
    assert_eq!(starts.len(), (2.0 * 48_000.0 / 1024.0f64).ceil() as usize);
    assert!(starts.windows(2).all(|w| w[1] > w[0]), "in time order");
    assert_eq!(starts[0], 0.0);
}

#[test]
fn the_sound_is_where_it_was_cued_and_silent_elsewhere() {
    // The tone, half a second long, cued at frame 24 of 72: one second in, in a three second film.
    let doc = with_sound(72, 24, tone_wav(4000));
    let mut track = audio::Track::new(&doc, (24, 1), 3.0)
        .expect("track")
        .expect("a track");
    let mut adts = Vec::new();
    while track.next_start().is_some() {
        adts.extend(track.next_frame().expect("frame"));
    }
    let frames = oxideav_aac::decode::StreamDecoder::new()
        .decode_all(&adts)
        .expect("decodes");
    let pcm: Vec<i16> = frames
        .iter()
        .flat_map(|f| f.pcm.iter().step_by(2).copied())
        .collect();
    let loudest = |from: f64, to: f64| {
        pcm[(from * 48_000.0) as usize..(to * 48_000.0) as usize]
            .iter()
            .map(|s| s.unsigned_abs())
            .max()
            .unwrap_or(0)
    };
    assert!(
        loudest(0.0, 0.9) < 300,
        "silent before: {}",
        loudest(0.0, 0.9)
    );
    assert!(
        loudest(1.1, 1.4) > 3000,
        "the tone after: {}",
        loudest(1.1, 1.4)
    );
    assert!(
        loudest(1.7, 2.9) < 300,
        "and gone again: {}",
        loudest(1.7, 2.9)
    );
}

/// The names of the boxes at the top of an MP4, and how many `trak` boxes are inside `moov`.
fn boxes(bytes: &[u8]) -> (Vec<String>, usize) {
    let (mut names, mut tracks, mut at) = (Vec::new(), 0, 0);
    while at + 8 <= bytes.len() {
        let size = u32::from_be_bytes(bytes[at..at + 4].try_into().expect("four")) as usize;
        let name = String::from_utf8_lossy(&bytes[at + 4..at + 8]).into_owned();
        if name == "moov" {
            let end = (at + size).min(bytes.len());
            let mut inner = at + 8;
            while inner + 8 <= end {
                let n =
                    u32::from_be_bytes(bytes[inner..inner + 4].try_into().expect("four")) as usize;
                if &bytes[inner + 4..inner + 8] == b"trak" {
                    tracks += 1;
                }
                inner += n.max(8);
            }
        }
        names.push(name);
        at += size.max(8);
    }
    (names, tracks)
}

#[test]
fn the_file_is_a_whole_mp4_with_a_picture_track_and_a_sound_track_only_when_there_is_sound() {
    let dir = tempfile::tempdir().expect("dir");
    let silent = dir.path().join("silent.mp4");
    write_mp4(&animation(30, (64, 48)), &silent).expect("silent");
    let (names, tracks) = boxes(&std::fs::read(&silent).expect("read"));
    assert_eq!(names.first().map(String::as_str), Some("ftyp"));
    assert!(
        names.contains(&"moov".to_owned()) && names.contains(&"mdat".to_owned()),
        "{names:?}"
    );
    assert_eq!(tracks, 1);
    let loud = dir.path().join("loud.mp4");
    write_mp4(&with_sound(30, 0, tone_wav(4000)), &loud).expect("with sound");
    assert_eq!(
        boxes(&std::fs::read(&loud).expect("read")).1,
        2,
        "a picture and a sound track"
    );
    let left: Vec<String> = std::fs::read_dir(dir.path())
        .expect("ls")
        .map(|e| e.expect("e").file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(left.len(), 2, "no staging file is left: {left:?}");
}

/// With ffmpeg and ffprobe on the machine (this checks nothing, and says so, where there are none):
/// a stranger's decoder reads the file, every frame and the sound.
#[test]
fn an_independent_player_reads_the_file_when_one_is_installed() {
    use std::process::Command;
    let have = |name: &str| {
        Command::new(name)
            .arg("-version")
            .output()
            .is_ok_and(|o| o.status.success())
    };
    if !(have("ffmpeg") && have("ffprobe")) {
        eprintln!("no ffmpeg here: the independent check was not run");
        return;
    }
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("film.mp4");
    write_mp4(&with_sound(48, 0, tone_wav(4000)), &path).expect("encoded");
    let probe = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_name,codec_type,width,height,nb_frames,sample_rate,channels",
            "-of",
            "default=nw=1",
        ])
        .arg(&path)
        .output()
        .expect("probe");
    let said = String::from_utf8_lossy(&probe.stdout).into_owned();
    for wanted in [
        "codec_name=h264",
        "codec_name=aac",
        "width=64",
        "height=48",
        "nb_frames=48",
        "sample_rate=48000",
        "channels=2",
    ] {
        assert!(said.contains(wanted), "{wanted}: {said}");
    }
    let decode = Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(&path)
        .args(["-f", "null", "-"])
        .output()
        .expect("ffmpeg");
    assert!(
        decode.status.success() && decode.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&decode.stderr)
    );
}

#[test]
fn a_movie_that_cannot_be_made_says_why_and_leaves_nothing_behind() {
    let dir = tempfile::tempdir().expect("dir");
    let target = dir.path().join("a.mp4");
    let long = animation(u32::try_from(MAX_VIDEO_FRAMES + 1).expect("fits"), (20, 20));
    assert!(
        write_mp4(&long, &target)
            .expect_err("too long")
            .contains("limited")
    );
    // An existing file is never replaced.
    std::fs::write(&target, b"mine").expect("write");
    assert!(
        write_mp4(&animation(3, (20, 20)), &target)
            .expect_err("exists")
            .contains("already exists")
    );
    assert_eq!(std::fs::read(&target).expect("read"), b"mine");
    // A sound that cannot be read names itself.
    let broken = dir.path().join("broken.mp4");
    let bad = with_sound(6, 0, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    let error = write_mp4(&bad, &broken).expect_err("a sound that is not a sound");
    assert!(error.contains("tone"), "{error}");
    assert!(!broken.exists());
    // A place that is not there.
    let nowhere = dir.path().join("no-such-folder").join("x.mp4");
    assert!(write_mp4(&animation(3, (20, 20)), &nowhere).is_err());
    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .expect("ls")
        .map(|e| e.expect("e").file_name())
        .collect();
    assert_eq!(
        entries.len(),
        1,
        "only the file that was there: {entries:?}"
    );
}

#[path = "delay_tests.rs"]
mod delay;
