//! Exporting the animation as an MP4 video, with its sound, through an `ffmpeg` the person has
//! installed. Flare carries no video encoder (H.264 is patent-encumbered and its good encoders
//! are GPL), so as with Ruffle it is kept at arm's length: found on `PATH` or through
//! `FLARE_FFMPEG`, started with no shell and fixed arguments, fed the frames on its input, and
//! its output kept only if it succeeds.

use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use flare_core::SoundId;
use flare_document::Document;

/// Longest side of the video in pixels; bigger stages are scaled down to it.
pub const MAX_VIDEO_SIDE: u32 = 1920;
/// Most frames a video holds (about 14 minutes at 24 frames a second).
pub const MAX_VIDEO_FRAMES: u64 = 20_000;
/// The sound is mixed at this rate, in stereo.
const AUDIO_RATE: u32 = 48_000;
/// How much of ffmpeg's complaint is kept to show.
const MAX_STDERR: u64 = 8 * 1024;

fn is_executable_file(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Finds ffmpeg: the `FLARE_FFMPEG` variable if it names an executable file, otherwise an
/// `ffmpeg` executable in one of `PATH`'s absolute directories. The lookup is injectable so
/// tests need no global state.
pub fn find_ffmpeg_with(env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    if let Some(explicit) = env("FLARE_FFMPEG").filter(|v| !v.is_empty()) {
        let path = PathBuf::from(explicit);
        return is_executable_file(&path).then_some(path);
    }
    let path_var = env("PATH")?;
    let names: &[&str] = if cfg!(windows) {
        &["ffmpeg.exe", "ffmpeg"]
    } else {
        &["ffmpeg"]
    };
    std::env::split_paths(&path_var)
        .filter(|dir| dir.is_absolute())
        .flat_map(|dir| names.iter().map(move |name| dir.join(name)))
        .find(|candidate| is_executable_file(candidate))
}

pub fn find_ffmpeg() -> Option<PathBuf> {
    find_ffmpeg_with(|name| std::env::var_os(name))
}

/// The size of the video for a stage `width` by `height`: scaled down to [`MAX_VIDEO_SIDE`] and
/// even in both directions, which the usual H.264 pixel format needs.
pub fn video_size(width: u32, height: u32) -> (u32, u32) {
    let longest = f64::from(width.max(height).max(1));
    let scale = (f64::from(MAX_VIDEO_SIDE) / longest).min(1.0);
    let even = |side: u32| {
        let scaled = (f64::from(side) * scale).round() as u32;
        (scaled.max(2) / 2) * 2
    };
    (even(width), even(height))
}

/// The arguments ffmpeg is started with: raw RGBA frames on its input, optionally raw stereo
/// sound from `audio`, and an H.264 (and AAC) video in an MP4 written to `out`.
pub fn ffmpeg_args(
    size: (u32, u32),
    rate: (u32, u32),
    audio: Option<&Path>,
    out: &Path,
) -> Vec<OsString> {
    let mut args: Vec<OsString> = Vec::new();
    let mut add = |items: &[&str]| args.extend(items.iter().map(OsString::from));
    add(&["-hide_banner", "-loglevel", "error", "-y"]);
    add(&["-f", "rawvideo", "-pix_fmt", "rgba"]);
    add(&["-s", &format!("{}x{}", size.0, size.1)]);
    add(&[
        "-framerate",
        &format!("{}/{}", rate.0, rate.1),
        "-i",
        "pipe:0",
    ]);
    if let Some(audio) = audio {
        add(&[
            "-f",
            "f32le",
            "-ar",
            &AUDIO_RATE.to_string(),
            "-ac",
            "2",
            "-i",
        ]);
        args.push(audio.as_os_str().to_owned());
    }
    let mut add = |items: &[&str]| args.extend(items.iter().map(OsString::from));
    add(&[
        "-c:v", "libx264", "-pix_fmt", "yuv420p", "-preset", "medium", "-crf", "18",
    ]);
    if audio.is_some() {
        add(&["-c:a", "aac", "-b:a", "192k", "-shortest"]);
    }
    add(&["-movflags", "+faststart", "-f", "mp4"]);
    args.push(out.as_os_str().to_owned());
    args
}

/// Mixes every sound cued in the first scene into raw stereo `f32` samples at [`AUDIO_RATE`],
/// written to `sink` a second at a time so a long film is never held whole. Returns whether
/// there was any sound to write.
fn mix_sound(
    document: &Document,
    frames: u64,
    rate: (u32, u32),
    sink: &mut impl Write,
) -> Result<bool, String> {
    let Some(scene) = document.scenes().first() else {
        return Ok(false);
    };
    let mut mixer = flare_audio::Mixer::new(AUDIO_RATE);
    let mut decoded: HashMap<SoundId, std::sync::Arc<flare_audio::Audio>> = HashMap::new();
    let seconds_per_frame = f64::from(rate.1) / f64::from(rate.0);
    let mut any = false;
    for layer in scene.timeline().layers() {
        for cue in layer.cues() {
            let Some(sound) = document.sound(cue.sound) else {
                continue;
            };
            let audio = match decoded.get(&cue.sound) {
                Some(audio) => audio.clone(),
                None => {
                    let audio = flare_audio::decode(sound.data())
                        .map_err(|e| format!("Cannot read the sound \"{}\": {e}", sound.name()))?;
                    let audio = std::sync::Arc::new(audio);
                    decoded.insert(cue.sound, audio.clone());
                    audio
                }
            };
            let delay = (f64::from(cue.start) * seconds_per_frame * f64::from(AUDIO_RATE)) as u64;
            mixer.play(audio, delay);
            any = true;
        }
    }
    if !any {
        return Ok(false);
    }
    let total = (frames as f64 * seconds_per_frame * f64::from(AUDIO_RATE)).ceil() as u64;
    let mut done = 0u64;
    let mut chunk = vec![0.0f32; AUDIO_RATE as usize * 2];
    let mut bytes = Vec::with_capacity(chunk.len() * 4);
    while done < total {
        let now = (total - done).min(u64::from(AUDIO_RATE)) as usize;
        let slice = &mut chunk[..now * 2];
        mixer.render(slice);
        bytes.clear();
        for sample in slice.iter() {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        sink.write_all(&bytes)
            .map_err(|e| format!("Cannot write the sound: {e}"))?;
        done += now as u64;
    }
    Ok(true)
}

/// Draws every frame of the first scene and has `ffmpeg` encode them, with the sound, into an MP4
/// at `path`. The file appears only when the encoder succeeded, and an existing file is never
/// replaced. Errors are worded for the person who asked for the export.
pub fn write_mp4(document: &Document, ffmpeg: &Path, path: &Path) -> Result<(), String> {
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
    if path.exists() {
        return Err(format!("{} already exists", path.display()));
    }
    let directory = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let stage = document.stage();
    let (width, height) = video_size(stage.width(), stage.height());

    // The video is encoded into a file beside the target and moved into place when it is whole.
    let staged = tempfile::Builder::new()
        .prefix(".flare-video-")
        .suffix(".part")
        .tempfile_in(directory)
        .map_err(|e| format!("Cannot write the movie: {e}"))?
        .into_temp_path();
    let mut sound_file =
        tempfile::NamedTempFile::new().map_err(|e| format!("Cannot write the movie: {e}"))?;
    let has_sound = {
        let mut sink = std::io::BufWriter::new(sound_file.as_file_mut());
        let any = mix_sound(document, frames, (num, den), &mut sink)?;
        sink.flush()
            .map_err(|e| format!("Cannot write the sound: {e}"))?;
        any
    };
    let sound_path = sound_file.into_temp_path();
    let args = ffmpeg_args(
        (width, height),
        (num, den),
        has_sound.then_some(sound_path.as_ref()),
        &staged,
    );
    let mut child = Command::new(ffmpeg)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Cannot start ffmpeg: {e}"))?;
    let mut complaint = child.stderr.take();
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(stderr) = complaint.as_mut() {
            let _ = stderr.take(MAX_STDERR).read_to_string(&mut text);
            // Whatever is left is read and dropped so ffmpeg never blocks writing it.
            let _ = std::io::copy(stderr, &mut std::io::sink());
        }
        text
    });
    let fed = feed_frames(
        document,
        &mut child,
        (width, height),
        u32::try_from(frames).unwrap_or(u32::MAX),
    );
    drop(child.stdin.take());
    let status = child.wait();
    let complaint = reader.join().unwrap_or_default();
    let status = status.map_err(|e| format!("ffmpeg did not finish: {e}"))?;
    if !status.success() {
        let said = complaint.trim();
        return Err(if said.is_empty() {
            format!("ffmpeg stopped with {status}")
        } else {
            format!("ffmpeg stopped: {said}")
        });
    }
    // A failure drawing a frame after ffmpeg took the earlier ones would leave a short video.
    fed?;
    staged.persist_noclobber(path).map(|_| ()).map_err(|e| {
        if e.error.kind() == std::io::ErrorKind::AlreadyExists {
            format!("{} already exists", path.display())
        } else {
            format!("Cannot write the movie: {}", e.error)
        }
    })
}

/// Draws the frames over the stage color and writes them to the child's input.
fn feed_frames(
    document: &Document,
    child: &mut std::process::Child,
    size: (u32, u32),
    frames: u32,
) -> Result<(), String> {
    let scene = document
        .scenes()
        .first()
        .ok_or("The document has no scene to export.")?;
    let stage = document.stage();
    let viewport = flare_render::Viewport {
        x: 0.0,
        y: 0.0,
        width: f64::from(stage.width()),
        height: f64::from(stage.height()),
    };
    let background = {
        let c = stage.background();
        flare_core::Rgba8::opaque(c.r, c.g, c.b)
    };
    let Some(input) = child.stdin.as_mut() else {
        return Err("Cannot reach ffmpeg's input.".to_owned());
    };
    for frame in 0..frames {
        let drawn = flare_eval::evaluate_frame(document, scene.id(), frame)
            .map_err(|e| format!("Cannot draw frame {}: {e}", frame + 1))?;
        let image = flare_render::render_viewport(&drawn, &viewport, size.0, size.1, background)
            .map_err(|e| format!("Cannot render frame {}: {e}", frame + 1))?;
        if input.write_all(image.rgba()).is_err() {
            // ffmpeg has stopped taking frames; its own message says why.
            return Ok(());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
