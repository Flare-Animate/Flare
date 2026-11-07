//! Writing a movie: the pictures go to ffmpeg as raw RGBA on its input, the sound as 16-bit stereo
//! PCM from a file, and ffmpeg encodes them into the container.

use std::ffi::OsString;
use std::path::Path;

use crate::containers::Container;
use crate::error::FfmpegError;
use crate::run;
use crate::tool::locate;

/// The sound's rate, as it is written to the PCM file.
pub const SOUND_RATE: u32 = 48_000;
/// Most frames one movie may hold.
pub const MAX_FRAMES: u32 = 20_000;
/// Longest side of a movie in pixels.
pub const MAX_SIDE: u32 = 1920;

/// What a movie is made of.
pub struct Movie<'a> {
    /// The picture's size in pixels.
    pub size: (u32, u32),
    /// Frames per second, as a numerator and a denominator.
    pub rate: (u32, u32),
    /// How many frames.
    pub frames: u32,
    /// The sound: 16-bit stereo PCM at [`SOUND_RATE`], read from this file. `None` for no sound.
    pub sound: Option<&'a Path>,
}

/// Writes `movie` to `out` in `container`. `frame(n)` gives frame `n` as RGBA of the movie's size.
/// The file is written by ffmpeg itself, so `out` must be a path of the caller's own making.
pub fn encode(
    container: &Container,
    out: &Path,
    movie: &Movie<'_>,
    mut frame: impl FnMut(u32) -> Result<Vec<u8>, String>,
) -> Result<(), FfmpegError> {
    let (w, h) = movie.size;
    if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE {
        return Err(FfmpegError::Refused(format!(
            "a movie is at most {MAX_SIDE} pixels on a side; this one is {w}x{h}"
        )));
    }
    if movie.frames == 0 || movie.frames > MAX_FRAMES {
        return Err(FfmpegError::Refused(format!(
            "a movie holds 1 to {MAX_FRAMES} frames; this one has {}",
            movie.frames
        )));
    }
    if movie.rate.0 == 0 || movie.rate.1 == 0 {
        return Err(FfmpegError::Refused(
            "the frame rate must be above zero".to_owned(),
        ));
    }
    check_path(out)?;
    let tools = locate()?;
    let args = encode_args(container, out, movie)?;
    let frame_bytes = w as usize * h as usize * 4;
    run::feed(&tools.ffmpeg, &args, "write the movie", |stdin| {
        for n in 0..movie.frames {
            let pixels = frame(n).map_err(FfmpegError::Refused)?;
            if pixels.len() != frame_bytes {
                return Err(FfmpegError::Refused(format!(
                    "frame {} has {} bytes; a {w}x{h} picture has {frame_bytes}",
                    n + 1,
                    pixels.len()
                )));
            }
            stdin.write_all(&pixels)?;
        }
        Ok(())
    })
}

/// The arguments for ffmpeg, as a list: nothing here is read by a shell.
pub(crate) fn encode_args(
    container: &Container,
    out: &Path,
    movie: &Movie<'_>,
) -> Result<Vec<OsString>, FfmpegError> {
    let s = |text: &str| OsString::from(text);
    let (w, h) = movie.size;
    let mut args: Vec<OsString> = [
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgba",
    ]
    .iter()
    .map(|a| s(a))
    .collect();
    args.push(s("-video_size"));
    args.push(s(&format!("{w}x{h}")));
    args.push(s("-framerate"));
    args.push(s(&format!("{}/{}", movie.rate.0, movie.rate.1)));
    args.push(s("-i"));
    args.push(s("pipe:0"));
    if let Some(sound) = movie.sound {
        check_path(sound)?;
        args.extend(["-f", "s16le", "-ar"].map(s));
        args.push(s(&SOUND_RATE.to_string()));
        args.extend(["-ac", "2"].map(s));
        args.push(s("-i"));
        args.push(sound.as_os_str().to_owned());
        args.extend(["-map", "0:v:0", "-map", "1:a:0"].map(s));
    } else {
        args.extend(["-map", "0:v:0"].map(s));
    }
    args.push(s("-c:v"));
    args.push(s(container.video_codec));
    if let Some(pixel_format) = container.pixel_format {
        args.extend(["-pix_fmt", pixel_format].map(s));
    }
    if let Some(bitrate) = container.bitrate {
        args.extend(["-b:v", bitrate].map(s));
    }
    if let (Some(audio), Some(_)) = (container.audio_codec, movie.sound) {
        args.push(s("-c:a"));
        args.push(s(audio));
        args.push(s("-shortest"));
    }
    args.push(s("-f"));
    args.push(s(container.muxer));
    args.push(out.as_os_str().to_owned());
    Ok(args)
}

/// A path ffmpeg is handed must not look like an option.
pub(crate) fn check_path(path: &Path) -> Result<(), FfmpegError> {
    if path.as_os_str().to_string_lossy().starts_with('-') {
        return Err(FfmpegError::Refused(format!(
            "{} starts with a dash, which ffmpeg would read as an option",
            path.display()
        )));
    }
    Ok(())
}
