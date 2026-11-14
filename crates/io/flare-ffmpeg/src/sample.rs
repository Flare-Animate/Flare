//! A movie as reference pictures: what ffprobe says about it before anything is read (its size,
//! rate, length and turn), then its pictures taken at a rate and size of the caller's choosing,
//! handed over one at a time as they arrive, so nothing waits for the whole movie in memory and
//! the caller can stop at any picture.

use std::ffi::OsString;
use std::path::Path;

use crate::decode::{Source, first_frame_size, parse_probe};
use crate::encode::check_path;
use crate::error::FfmpegError;
use crate::run;
use crate::tool::{Tools, locate};

/// Longest side of a sampled picture, in pixels.
pub const MAX_SAMPLE_SIDE: u32 = 8192;
/// Most picture bytes one sampling may ask for. A document holds no more than this either.
pub const MAX_SAMPLE_BYTES: u64 = 1024 * 1024 * 1024;

/// What a movie is, as ffprobe reports it, with its sides as it is shown: a phone movie stored
/// on its side with a quarter turn to play it upright has its width and height swapped here,
/// because ffmpeg turns its pictures upright as it reads them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Probe {
    pub width: u32,
    pub height: u32,
    /// Frames per second, as a numerator and a denominator.
    pub rate: (u32, u32),
    /// How long it runs, when the container says.
    pub seconds: Option<f64>,
}

/// How a movie is sampled: at `rate` pictures a second from its start, each scaled to exactly
/// `size`, and at most `max_frames` of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sampling {
    pub rate: (u32, u32),
    pub size: (u32, u32),
    pub max_frames: u32,
}

/// What ffprobe says about the movie at `path`.
pub fn probe(path: &Path) -> Result<Probe, FfmpegError> {
    probe_with(&locate()?, path)
}

/// [`probe`] with the programs given rather than those found on PATH.
pub fn probe_with(tools: &Tools, path: &Path) -> Result<Probe, FfmpegError> {
    check_path(path)?;
    let source = stream_source(tools, path)?;
    let seconds = duration(tools, path)?;
    let quarter_turned =
        rotation(tools, path)?.is_some_and(|degrees| degrees.rem_euclid(180) == 90);
    let (width, height) = if quarter_turned {
        (source.height, source.width)
    } else {
        (source.width, source.height)
    };
    Ok(Probe {
        width,
        height,
        rate: source.rate,
        seconds,
    })
}

/// The first video stream's size and rate, from its first frame when the stream has no size.
pub(crate) fn stream_source(tools: &Tools, path: &Path) -> Result<Source, FfmpegError> {
    let out = ffprobe(
        tools,
        path,
        &[
            "stream=width,height,avg_frame_rate,r_frame_rate",
            "-of",
            "csv=p=0",
        ],
    )?;
    let source = parse_probe(&String::from_utf8_lossy(&out))?;
    if source.width == 0 || source.height == 0 {
        // An animated WebP reports no size for its stream: its first frame gives it.
        return first_frame_size(&tools.ffprobe, path, source.rate);
    }
    Ok(source)
}

fn ffprobe(tools: &Tools, path: &Path, entries: &[&str]) -> Result<Vec<u8>, FfmpegError> {
    let args: Vec<OsString> = ["-v", "error", "-select_streams", "v:0", "-show_entries"]
        .iter()
        .chain(entries)
        .map(OsString::from)
        .chain(std::iter::once(path.as_os_str().to_owned()))
        .collect();
    run::capture(&tools.ffprobe, &args, "read the movie", 4096)
}

/// The container's length in seconds; `None` when it does not say (a stream, a damaged file).
fn duration(tools: &Tools, path: &Path) -> Result<Option<f64>, FfmpegError> {
    let out = ffprobe(
        tools,
        path,
        &[
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ],
    )?;
    Ok(parse_seconds(&String::from_utf8_lossy(&out)))
}

/// The turn a player gives the pictures, in degrees, from the display matrix or the older
/// `rotate` tag; `None` when there is neither.
fn rotation(tools: &Tools, path: &Path) -> Result<Option<i64>, FfmpegError> {
    let out = ffprobe(
        tools,
        path,
        &[
            "stream_side_data=rotation:stream_tags=rotate",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ],
    )?;
    Ok(parse_rotation(&String::from_utf8_lossy(&out)))
}

/// A length ffprobe printed, when it is one: finite, above zero, and at most a day.
pub(crate) fn parse_seconds(text: &str) -> Option<f64> {
    text.lines()
        .find_map(|line| line.trim().parse::<f64>().ok())
        .filter(|s| s.is_finite() && *s > 0.0 && *s <= 86_400.0)
}

/// The first whole number of degrees ffprobe printed.
pub(crate) fn parse_rotation(text: &str) -> Option<i64> {
    text.lines()
        .find_map(|line| line.trim().parse::<f64>().ok())
        .filter(|d| d.is_finite() && d.abs() <= 360.0)
        .map(|d| d.round() as i64)
}

/// Reads the movie at `path` as `sampling` says, handing each picture to `each` as RGBA, rows
/// top to bottom, as soon as it arrives. `each` returns false to stop, and the reading then ends
/// with [`FfmpegError::Stopped`]. Returns how many pictures were handed over.
pub fn read_frames(
    path: &Path,
    sampling: Sampling,
    each: impl FnMut(Vec<u8>) -> bool,
) -> Result<u32, FfmpegError> {
    read_frames_with(&locate()?, path, sampling, each)
}

/// [`read_frames`] with the programs given rather than those found on PATH.
pub fn read_frames_with(
    tools: &Tools,
    path: &Path,
    sampling: Sampling,
    mut each: impl FnMut(Vec<u8>) -> bool,
) -> Result<u32, FfmpegError> {
    let args = sample_args(path, sampling)?;
    let (w, h) = sampling.size;
    let frame_bytes = usize::try_from(u64::from(w) * u64::from(h) * 4)
        .map_err(|_| FfmpegError::Refused("the pictures are too large".to_owned()))?;
    let count = run::stream(
        &tools.ffmpeg,
        &args,
        "read the movie",
        frame_bytes,
        u64::from(sampling.max_frames),
        &mut each,
    )?;
    if count == 0 {
        return Err(FfmpegError::Failed {
            action: "read the movie",
            detail: "it has no pictures".to_owned(),
        });
    }
    Ok(u32::try_from(count).unwrap_or(u32::MAX))
}

/// The arguments that sample the movie: the rate first, so only the pictures kept are scaled.
pub(crate) fn sample_args(path: &Path, sampling: Sampling) -> Result<Vec<OsString>, FfmpegError> {
    check_path(path)?;
    let Sampling {
        rate: (n, d),
        size: (w, h),
        max_frames,
    } = sampling;
    if n == 0 || d == 0 {
        return Err(FfmpegError::Refused(
            "the rate to take pictures at is zero".to_owned(),
        ));
    }
    if w == 0 || h == 0 || w > MAX_SAMPLE_SIDE || h > MAX_SAMPLE_SIDE {
        return Err(FfmpegError::Refused(format!(
            "a picture is 1 to {MAX_SAMPLE_SIDE} pixels a side, not {w}x{h}"
        )));
    }
    if max_frames == 0 {
        return Err(FfmpegError::Refused(
            "no pictures were asked for".to_owned(),
        ));
    }
    let bytes = u64::from(w) * u64::from(h) * 4 * u64::from(max_frames);
    if bytes > MAX_SAMPLE_BYTES {
        return Err(FfmpegError::Refused(format!(
            "{max_frames} pictures of {w}x{h} would take {bytes} bytes; at most {MAX_SAMPLE_BYTES} are read"
        )));
    }
    let mut args: Vec<OsString> = ["-hide_banner", "-loglevel", "error", "-i"]
        .iter()
        .map(OsString::from)
        .collect();
    args.push(path.as_os_str().to_owned());
    args.extend(
        [
            "-an".to_owned(),
            "-sn".to_owned(),
            "-vf".to_owned(),
            format!("fps={n}/{d},scale={w}:{h}"),
            "-frames:v".to_owned(),
            max_frames.to_string(),
            "-f".to_owned(),
            "rawvideo".to_owned(),
            "-pix_fmt".to_owned(),
            "rgba".to_owned(),
            "pipe:1".to_owned(),
        ]
        .map(OsString::from),
    );
    Ok(args)
}
