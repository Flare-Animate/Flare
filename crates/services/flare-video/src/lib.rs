//! The movie as an MP4 video with its sound, made here, with no encoder to install: the picture
//! is H.264 from `rusty_h264`, the sound AAC-LC from OxideAV's `oxideav-aac`, and `muxide` writes
//! the container. All three are pure Rust.
//!
//! The picture is drawn a frame at a time, encoded and handed to the container at once, and the
//! sound goes in beside it in time order, so memory does not grow with the length of the film
//! beyond what the container keeps. The file appears only when it is complete, and an existing
//! file is never replaced.
#![forbid(unsafe_code)]

mod audio;
mod color;

use std::io::Write;
use std::path::Path;

use flare_document::Document;
use muxide::api::{AacProfile, AudioCodec, MuxerBuilder, VideoCodec};
use rusty_h264::{Encoder, EncoderConfig};

/// Longest side of the video in pixels; bigger stages are scaled down to it.
pub const MAX_VIDEO_SIDE: u32 = 1920;
/// Most frames a video holds (about 14 minutes at 24 frames a second).
pub const MAX_VIDEO_FRAMES: u64 = 20_000;

/// The size of the video for a stage `width` by `height`: scaled down to [`MAX_VIDEO_SIDE`] and
/// even in both directions, which H.264's 4:2:0 pictures need.
pub fn video_size(width: u32, height: u32) -> (u32, u32) {
    let longest = f64::from(width.max(height).max(1));
    let scale = (f64::from(MAX_VIDEO_SIDE) / longest).min(1.0);
    let even = |side: u32| {
        let scaled = (f64::from(side) * scale).round() as u32;
        (scaled.max(2) / 2) * 2
    };
    (even(width), even(height))
}

/// The average bit rate for a picture of this size and speed: about a tenth of a bit a pixel,
/// which is plenty for drawn animation, kept between a floor that stays watchable and a ceiling
/// that stays small.
pub fn video_bitrate(size: (u32, u32), fps: f64) -> u32 {
    let wanted = f64::from(size.0) * f64::from(size.1) * fps * 0.1;
    wanted.clamp(400_000.0, 16_000_000.0) as u32
}

/// Whether `annex_b` (one access unit) holds an IDR slice, which a player can start from.
fn is_keyframe(annex_b: &[u8]) -> bool {
    let mut i = 0;
    while i + 3 < annex_b.len() {
        let long = annex_b[i + 2] == 0 && annex_b[i + 3] == 1;
        if annex_b[i] == 0 && annex_b[i + 1] == 0 && (annex_b[i + 2] == 1 || long) {
            let start = if long { i + 4 } else { i + 3 };
            if start < annex_b.len() && annex_b[start] & 0x1f == 5 {
                return true;
            }
            i = start;
        } else {
            i += 1;
        }
    }
    false
}

/// Draws the frames of the first scene one at a time, as pictures of `size`.
pub(crate) struct Frames<'a> {
    document: &'a Document,
    scene: flare_core::SceneId,
    viewport: flare_render::Viewport,
    background: flare_core::Rgba8,
    size: (u32, u32),
}

impl<'a> Frames<'a> {
    pub(crate) fn new(document: &'a Document, size: (u32, u32)) -> Result<Self, String> {
        let scene = document
            .scenes()
            .first()
            .ok_or("The document has no scene to export.")?;
        let stage = document.stage();
        let c = stage.background();
        Ok(Self {
            document,
            scene: scene.id(),
            viewport: flare_render::Viewport {
                x: 0.0,
                y: 0.0,
                width: f64::from(stage.width()),
                height: f64::from(stage.height()),
            },
            background: flare_core::Rgba8::opaque(c.r, c.g, c.b),
            size,
        })
    }

    /// Frame `n` over the stage color, as RGBA.
    pub(crate) fn rgba(&self, n: u32) -> Result<Vec<u8>, String> {
        let drawn = flare_eval::evaluate_frame(self.document, self.scene, n)
            .map_err(|e| format!("Cannot draw frame {}: {e}", n + 1))?;
        let image = flare_render::render_viewport(
            &drawn,
            &self.viewport,
            self.size.0,
            self.size.1,
            self.background,
        )
        .map_err(|e| format!("Cannot render frame {}: {e}", n + 1))?;
        Ok(image.rgba().to_vec())
    }
}

/// The encoder for pictures of `size` at `fps`: a keyframe every two seconds, the rest predicted.
pub(crate) fn picture_encoder(size: (u32, u32), fps: f64) -> Result<Encoder, String> {
    let mut config = EncoderConfig::new(size.0 as usize, size.1 as usize);
    config.framerate = fps as f32;
    config.gop_size = ((fps * 2.0).round() as u32).clamp(12, 120);
    config.bitrate = video_bitrate(size, fps);
    Encoder::new(config).map_err(|e| format!("Cannot start the video encoder: {e}"))
}

/// Makes an MP4 of the first scene at `path`, with the sound cued in it. The file appears only
/// when it is whole, and an existing file is never replaced. Errors are worded for the person
/// who asked for the export.
pub fn write_mp4(document: &Document, path: &Path) -> Result<(), String> {
    let scene = document
        .scenes()
        .first()
        .ok_or("The document has no scene to export.")?;
    let frames = scene.timeline().frame_count().max(1);
    if frames > MAX_VIDEO_FRAMES {
        return Err(format!(
            "The movie has {frames} frames; a video is limited to {MAX_VIDEO_FRAMES}."
        ));
    }
    let rate = document.frame_rate();
    let (num, den) = (rate.numerator(), rate.denominator());
    if num == 0 || den == 0 {
        return Err("The frame rate must be above zero to export a movie.".to_owned());
    }
    let fps = f64::from(num) / f64::from(den);
    if path.exists() {
        return Err(format!("{} already exists", path.display()));
    }
    let directory = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let stage = document.stage();
    let size = video_size(stage.width(), stage.height());
    let seconds = frames as f64 / fps;

    let drawn = Frames::new(document, size)?;
    let mut encoder = picture_encoder(size, fps)?;
    let mut sound = audio::Track::new(document, (num, den), seconds)?;

    // Written beside the target and moved into place when the container is closed.
    let mut staged = tempfile::Builder::new()
        .prefix(".flare-video-")
        .suffix(".part")
        .tempfile_in(directory)
        .map_err(|e| format!("Cannot write the movie: {e}"))?;
    let mut builder = MuxerBuilder::new(
        staged
            .as_file()
            .try_clone()
            .map_err(|e| format!("Cannot write the movie: {e}"))?,
    )
    .video(VideoCodec::H264, size.0, size.1, fps);
    if sound.is_some() {
        builder = builder.audio(AudioCodec::Aac(AacProfile::Lc), audio::RATE, 2);
    }
    let mut muxer = builder
        .build()
        .map_err(|e| format!("Cannot start the movie: {e}"))?;
    for n in 0..frames as u32 {
        let pts = f64::from(n) / fps;
        let yuv = color::to_i420(&drawn.rgba(n)?, size.0 as usize, size.1 as usize);
        let unit = encoder.encode(&yuv);
        muxer
            .write_video(pts, &unit, is_keyframe(&unit))
            .map_err(|e| format!("Cannot write frame {}: {e}", n + 1))?;
        // The sound up to this frame's time goes in after it: the container wants a picture
        // before any sound.
        while let Some(track) = sound.as_mut()
            && track.next_start().is_some_and(|start| start <= pts)
        {
            let start = track.next_start().unwrap_or(0.0);
            let frame = track.next_frame()?;
            muxer
                .write_audio(start, &frame)
                .map_err(|e| format!("Cannot write the sound: {e}"))?;
        }
    }
    while let Some(track) = sound.as_mut()
        && let Some(start) = track.next_start()
    {
        let frame = track.next_frame()?;
        muxer
            .write_audio(start, &frame)
            .map_err(|e| format!("Cannot write the sound: {e}"))?;
    }
    muxer
        .finish()
        .map_err(|e| format!("Cannot finish the movie: {e}"))?;
    staged
        .flush()
        .map_err(|e| format!("Cannot write the movie: {e}"))?;
    staged
        .into_temp_path()
        .persist_noclobber(path)
        .map(|_| ())
        .map_err(|e| {
            if e.error.kind() == std::io::ErrorKind::AlreadyExists {
                format!("{} already exists", path.display())
            } else {
                format!("Cannot write the movie: {}", e.error)
            }
        })
}

#[cfg(test)]
mod tests;
