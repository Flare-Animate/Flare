//! Reading a movie: ffprobe gives its picture size and rate, and ffmpeg hands its pictures back as
//! raw RGBA, at most a count of frames and within a size, so memory is bounded before it is used.

use std::ffi::OsString;
use std::path::Path;

use crate::encode::check_path;
use crate::error::FfmpegError;
use crate::run;
use crate::tool::locate;

/// Most decoded picture bytes one read may hold.
pub const MAX_CLIP_BYTES: u64 = 512 * 1024 * 1024;

/// The pictures of a movie: each frame as RGBA at `width` by `height`, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clip {
    pub width: u32,
    pub height: u32,
    /// Frames per second, as a numerator and a denominator.
    pub rate: (u32, u32),
    pub frames: Vec<Vec<u8>>,
    /// True when the read stopped at its frame or byte limit, so the movie may have more.
    pub may_be_truncated: bool,
}

/// Reads at most `max_frames` pictures of the movie at `path`, scaled down to fit `max_side`.
pub fn decode(path: &Path, max_frames: u32, max_side: u32) -> Result<Clip, FfmpegError> {
    check_path(path)?;
    let tools = locate()?;
    let probe_args: Vec<OsString> = [
        "-v",
        "error",
        "-select_streams",
        "v:0",
        "-show_entries",
        "stream=width,height,avg_frame_rate,r_frame_rate",
        "-of",
        "csv=p=0",
    ]
    .iter()
    .map(OsString::from)
    .chain(std::iter::once(path.as_os_str().to_owned()))
    .collect();
    let probe = run::capture(&tools.ffprobe, &probe_args, "read the movie", 4096)?;
    eprintln!("DBG stream probe {:?}", String::from_utf8_lossy(&probe));
    let mut source = parse_probe(&String::from_utf8_lossy(&probe))?;
    if source.width == 0 || source.height == 0 {
        // An animated WebP reports no size for its stream: its first frame gives it.
        source = first_frame_size(&tools.ffprobe, path, source.rate)?;
    }
    let (w, h) = fit(source.width, source.height, max_side)?;
    let frame_bytes = u64::from(w) * u64::from(h) * 4;
    let allowed = (MAX_CLIP_BYTES / frame_bytes).min(u64::from(max_frames));
    if allowed == 0 {
        return Err(FfmpegError::Refused(format!(
            "a {w}x{h} picture is too large to read frames of"
        )));
    }
    let mut args: Vec<OsString> = ["-hide_banner", "-loglevel", "error", "-i"]
        .iter()
        .map(OsString::from)
        .collect();
    args.push(path.as_os_str().to_owned());
    args.push(OsString::from("-an"));
    args.push(OsString::from("-frames:v"));
    args.push(OsString::from(allowed.to_string()));
    if (w, h) != (source.width, source.height) {
        args.push(OsString::from("-vf"));
        args.push(OsString::from(format!("scale={w}:{h}")));
    }
    args.extend(["-f", "rawvideo", "-pix_fmt", "rgba", "pipe:1"].map(OsString::from));
    let bytes = run::capture(
        &tools.ffmpeg,
        &args,
        "read the movie",
        frame_bytes * allowed,
    )?;
    let frames: Vec<Vec<u8>> = bytes
        .chunks_exact(frame_bytes as usize)
        .map(<[u8]>::to_vec)
        .collect();
    if frames.is_empty() {
        return Err(FfmpegError::Failed {
            action: "read the movie",
            detail: "it has no pictures".to_owned(),
        });
    }
    let may_be_truncated = frames.len() as u64 == allowed;
    Ok(Clip {
        width: w,
        height: h,
        rate: source.rate,
        frames,
        may_be_truncated,
    })
}

/// What ffprobe reported about the first video stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Source {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rate: (u32, u32),
}

/// The size of the first frame, for a stream that reports none.
fn first_frame_size(ffprobe: &Path, path: &Path, rate: (u32, u32)) -> Result<Source, FfmpegError> {
    let args: Vec<OsString> = [
        "-v",
        "error",
        "-select_streams",
        "v:0",
        "-show_entries",
        "frame=width,height",
        "-read_intervals",
        "%+#1",
        "-of",
        "csv=p=0",
    ]
    .iter()
    .map(OsString::from)
    .chain(std::iter::once(path.as_os_str().to_owned()))
    .collect();
    let out = run::capture(ffprobe, &args, "read the movie", 4096)?;
    let text = String::from_utf8_lossy(&out);
    eprintln!("DBG frame probe {:?}", text);
    let fields: Vec<&str> = text.trim().trim_end_matches(',').split(',').collect();
    let (width, height) = match fields.as_slice() {
        [w, h, ..] => (w.parse::<u32>().ok(), h.parse::<u32>().ok()),
        _ => (None, None),
    };
    match (width, height) {
        (Some(width), Some(height)) if width > 0 && height > 0 => Ok(Source {
            width,
            height,
            rate,
        }),
        _ => Err(FfmpegError::Failed {
            action: "read the movie",
            detail: "its pictures have no size".to_owned(),
        }),
    }
}

/// Reads ffprobe's `width,height,avg_frame_rate,r_frame_rate` line. The average rate is used
/// unless it is 0/0, which some containers report; then the base rate is.
pub(crate) fn parse_probe(line: &str) -> Result<Source, FfmpegError> {
    let bad = || FfmpegError::Failed {
        action: "read the movie",
        detail: format!(
            "ffprobe said {:?}, not a picture size and rate",
            line.trim()
        ),
    };
    let fields: Vec<&str> = line.trim().trim_end_matches(',').split(',').collect();
    let [width, height, average, base] = fields.as_slice() else {
        return Err(bad());
    };
    let width: u32 = width.parse().map_err(|_| bad())?;
    let height: u32 = height.parse().map_err(|_| bad())?;
    let rate = parse_rate(average)
        .filter(|(n, _)| *n > 0)
        .or_else(|| parse_rate(base).filter(|(n, _)| *n > 0))
        .ok_or_else(bad)?;
    // A stream may report no size (an animated WebP does); the caller reads the first frame's.
    Ok(Source {
        width,
        height,
        rate,
    })
}

/// A rate written `n/d` (or a bare `n`), with a denominator of at least 1.
fn parse_rate(text: &str) -> Option<(u32, u32)> {
    match text.split_once('/') {
        Some((n, d)) => {
            let (n, d): (u32, u32) = (n.parse().ok()?, d.parse().ok()?);
            (d > 0).then_some((n, d))
        }
        None => text.parse().ok().map(|n| (n, 1)),
    }
}

/// The size a picture is read at: the source, scaled down so its longest side is `max_side` at
/// most, with both sides even.
pub(crate) fn fit(width: u32, height: u32, max_side: u32) -> Result<(u32, u32), FfmpegError> {
    if width == 0 || height == 0 || max_side < 2 {
        return Err(FfmpegError::Refused("the picture has no size".to_owned()));
    }
    let longest = width.max(height);
    if longest <= max_side {
        return Ok((even(width), even(height)));
    }
    let scale = f64::from(max_side) / f64::from(longest);
    let w = even((f64::from(width) * scale).round() as u32);
    let h = even((f64::from(height) * scale).round() as u32);
    Ok((w, h))
}

fn even(n: u32) -> u32 {
    (n & !1).max(2)
}
