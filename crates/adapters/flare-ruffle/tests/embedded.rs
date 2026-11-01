//! Ruffle playing inside the process: a movie advances with the time it is given, and the sound
//! a published movie carries comes out of Ruffle's mixer.
//!
//! Needs the `embedded` feature and a GPU adapter wgpu can open, so they are ignored in the
//! ordinary run (a machine without a GPU would fail them): `scripts/gpu_tests.sh` runs them, and
//! must pass before a change to the player or to what we publish for it.
#![cfg(feature = "embedded")]
#![allow(clippy::expect_used)]

use std::num::NonZeroU32;
use std::time::Duration;

use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId, SoundId};
use flare_document::{
    Content, Document, FrameSpan, Layer, Scene, Shape, Sound, SoundCue, Stage, Timeline,
};
use flare_publish::{PublishOptions, publish_swf};
use flare_ruffle::embedded::{AudioBackend, EmbeddedMovie, Gpu};
use ruffle_core::backend::audio::{
    AudioMixer, AudioMixerProxy, DecodeError, RegisterError, SoundHandle, SoundInstanceHandle,
    SoundStreamInfo, SoundTransform, swf,
};
use ruffle_core::impl_audio_mixer_backend;

const WIDTH: u32 = 100;
const HEIGHT: u32 = 80;

fn full(color: Rgba8) -> Shape {
    Shape::rect(
        ShapeId::new(),
        0.0,
        0.0,
        f64::from(WIDTH),
        f64::from(HEIGHT),
        color,
    )
    .expect("rect")
}

fn span(start: u32, shape: Shape) -> FrameSpan {
    FrameSpan::new(start, NonZeroU32::MIN, Content::Shape(shape))
}

fn document(layers: Vec<Layer>, sounds: Vec<Sound>) -> Document {
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(layers));
    Document::new(
        DocumentId::new(),
        FrameRate::new(12, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document")
    .with_stage(Stage::new(WIDTH, HEIGHT, Rgba8::WHITE).expect("stage"))
    .with_sounds(sounds)
    .expect("sounds")
}

fn publish(doc: &Document) -> Vec<u8> {
    publish_swf(doc, &PublishOptions::default())
        .expect("publish")
        .bytes
}

fn centre(movie: &EmbeddedMovie) -> [u8; 3] {
    let pixels = movie.read_pixels().expect("pixels");
    let at = ((HEIGHT / 2 * WIDTH + WIDTH / 2) * 4) as usize;
    [pixels[at], pixels[at + 1], pixels[at + 2]]
}

#[test]
#[ignore = "needs a GPU adapter; scripts/gpu_tests.sh"]
fn a_movie_advances_with_the_time_it_is_given() {
    let red = Rgba8::opaque(220, 30, 30);
    let blue = Rgba8::opaque(30, 30, 220);
    let layer = Layer::new(
        LayerId::new(),
        "L",
        vec![span(0, full(red)), span(1, full(blue))],
    )
    .expect("layer");
    let swf = publish(&document(vec![layer], vec![]));
    let gpu = Gpu::headless().expect("a GPU adapter");
    let mut movie = EmbeddedMovie::start(&swf, &gpu, 1.0, false).expect("start");
    assert_eq!(movie.size(), (WIDTH, HEIGHT));
    assert!(!movie.has_sound());

    // The first tick loads the movie and shows nothing but the stage. A frame lasts 1/12 s, so
    // the first appears once that much time has passed, and the second a frame later.
    movie.tick(Duration::from_millis(1)).expect("tick");
    assert_eq!(centre(&movie), [255, 255, 255], "still loading");
    movie.tick(Duration::from_millis(90)).expect("tick");
    assert_eq!(centre(&movie), [220, 30, 30]);
    movie.tick(Duration::from_millis(30)).expect("tick");
    assert_eq!(centre(&movie), [220, 30, 30], "not yet time for frame two");
    movie.tick(Duration::from_millis(60)).expect("tick");
    assert_eq!(centre(&movie), [30, 30, 220], "frame two");

    // Paused, it holds still however much time passes.
    movie.set_playing(false).expect("pause");
    movie.tick(Duration::from_secs(2)).expect("tick");
    assert_eq!(centre(&movie), [30, 30, 220]);
}

/// A paused movie is stepped one frame at a time, and the frame it shows is told.
#[test]
#[ignore = "needs a GPU adapter; scripts/gpu_tests.sh"]
fn a_paused_movie_steps_one_frame_at_a_time() {
    let red = Rgba8::opaque(220, 30, 30);
    let blue = Rgba8::opaque(30, 30, 220);
    let layer = Layer::new(
        LayerId::new(),
        "L",
        vec![span(0, full(red)), span(1, full(blue))],
    )
    .expect("layer");
    let swf = publish(&document(vec![layer], vec![]));
    let gpu = Gpu::headless().expect("a GPU adapter");
    let mut movie = EmbeddedMovie::start(&swf, &gpu, 1.0, false).expect("start");
    movie.tick(Duration::from_millis(1)).expect("load");
    movie.set_playing(false).expect("pause");
    let first = movie.current_frame();
    movie.step_frame().expect("step");
    movie.step_frame().expect("step");
    let after_two = movie.current_frame();
    assert_ne!(first, after_two, "the stepping moved the movie on");
    assert_eq!(
        centre(&movie),
        [30, 30, 220],
        "two frames in, the second shows"
    );
    // Time passing does nothing while it is paused; a step still does.
    movie.tick(Duration::from_secs(1)).expect("tick");
    assert_eq!(movie.current_frame(), after_two);
}

/// What the Test Movie window does: a still of one frame, ticked in the small steps a window's
/// frames are, shows its picture (not the stage's white) within a fraction of a second.
#[test]
#[ignore = "needs a GPU adapter; scripts/gpu_tests.sh"]
fn a_still_of_one_frame_shows_its_picture_when_ticked_in_a_windows_small_steps() {
    let red = Rgba8::opaque(220, 30, 30);
    let layer = Layer::new(LayerId::new(), "L", vec![span(0, full(red))]).expect("layer");
    let swf = publish(&document(vec![layer], vec![]));
    let gpu = Gpu::headless().expect("a GPU adapter");
    let mut movie = EmbeddedMovie::start(&swf, &gpu, 1.0, false).expect("start");
    let mut seen = Vec::new();
    for _ in 0..30 {
        movie.tick(Duration::from_millis(16)).expect("tick");
        seen.push(centre(&movie));
    }
    assert_eq!(
        seen.last(),
        Some(&[220, 30, 30]),
        "after half a second: {:?}",
        &seen[..8]
    );
}

#[test]
#[ignore = "needs a GPU adapter; scripts/gpu_tests.sh"]
fn something_that_is_not_a_swf_is_refused_not_a_crash() {
    let gpu = Gpu::headless().expect("a GPU adapter");
    let refused = EmbeddedMovie::start(b"this is not a movie", &gpu, 1.0, false);
    assert!(refused.is_err());
}

/// An audio backend that mixes into a buffer the test reads, as the sound card would.
struct Mixing {
    mixer: AudioMixer,
}

impl AudioBackend for Mixing {
    impl_audio_mixer_backend!(mixer);

    fn play(&mut self) {}

    fn pause(&mut self) {}
}

fn tone_wav() -> Vec<u8> {
    let rate = 22050_u32;
    let samples: Vec<i16> = (0..rate / 2)
        .map(|n| {
            ((f64::from(n) * 440.0 * std::f64::consts::TAU / f64::from(rate)).sin() * 12000.0)
                as i16
        })
        .collect();
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16_u32.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2_u16.to_le_bytes());
    out.extend_from_slice(&16_u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

/// Plays a one-second movie, with a cue for the tone or without, and returns the loudest sample
/// Ruffle's mixer produced in the first half second.
fn loudest(cued: bool, muted: bool) -> f32 {
    let id = SoundId::new();
    let sound = Sound::new(id, "Tone", tone_wav(), 22050, 1, 11025).expect("sound");
    let mut layer =
        Layer::new(LayerId::new(), "L", vec![span(0, full(Rgba8::WHITE))]).expect("layer");
    if cued {
        layer = layer
            .with_cues(vec![SoundCue {
                start: 0,
                sound: id,
            }])
            .expect("cues");
    }
    let swf = publish(&document(vec![layer], vec![sound]));
    let mixer = AudioMixer::new(2, 44100);
    let proxy: AudioMixerProxy = mixer.proxy();
    let gpu = Gpu::headless().expect("a GPU adapter");
    let mut movie =
        EmbeddedMovie::start_with_audio(&swf, &gpu, 1.0, Some(Box::new(Mixing { mixer })))
            .expect("start");
    assert!(movie.has_sound());
    movie.set_muted(muted).expect("mute");
    assert_eq!(movie.is_muted(), muted);
    for ms in [1, 100] {
        movie.tick(Duration::from_millis(ms)).expect("tick");
    }
    let mut buffer = vec![0.0_f32; 44100];
    proxy.mix::<f32>(&mut buffer);
    buffer.iter().fold(0.0_f32, |loud, s| loud.max(s.abs()))
}

#[test]
#[ignore = "needs a GPU adapter; scripts/gpu_tests.sh"]
fn the_sound_we_publish_is_played_by_ruffle_and_a_movie_without_a_cue_is_silent() {
    let heard = loudest(true, false);
    assert!(
        heard > 0.2,
        "Ruffle mixed a peak of {heard} for the cued tone"
    );
    assert_eq!(
        loudest(false, false),
        0.0,
        "nothing is cued, so nothing sounds"
    );
    assert_eq!(
        loudest(true, true),
        0.0,
        "muted, the cued tone is not heard"
    );
    let _ = flare_audio::decode(&tone_wav()).expect("the test tone is a real sound");
}

/// A device made the way the editor's window makes its own (wgpu's defaults, not Ruffle's), to
/// show that Ruffle can draw on a device it did not create.
#[test]
#[ignore = "needs a GPU adapter; scripts/gpu_tests.sh"]
fn ruffle_draws_on_a_device_made_with_default_limits_like_the_editors() {
    use flare_ruffle::embedded::wgpu;
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("an adapter");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("a device");
    let gpu = Gpu {
        instance,
        adapter,
        device,
        queue,
    };
    let layer = Layer::new(
        LayerId::new(),
        "L",
        vec![span(0, full(Rgba8::opaque(220, 30, 30)))],
    )
    .expect("layer");
    let swf = publish(&document(vec![layer], vec![]));
    let mut movie = EmbeddedMovie::start(&swf, &gpu, 1.0, false).expect("start");
    for ms in [1, 100] {
        movie.tick(Duration::from_millis(ms)).expect("tick");
    }
    assert_eq!(centre(&movie), [220, 30, 30]);
}
