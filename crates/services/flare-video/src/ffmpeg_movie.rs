//! A movie written by ffmpeg, for the containers Flare writes no other way (MOV, AVI, Matroska,
//! WebM, MPEG, Flash video). The pictures and the sound are handed to ffmpeg, which encodes them;
//! the sound is the same mix the MP4 export makes, written as 16-bit PCM beside the output.

use std::io::Write;
use std::path::Path;

use flare_document::Document;
use flare_ffmpeg::{Container, FfmpegError, Movie};
use flare_render::Backdrop;

use crate::audio;
use crate::{Frames, MAX_VIDEO_FRAMES, video_size};

/// Writes the first scene of `document` to `path` in `container` over `backdrop`, through ffmpeg.
/// A transparent backdrop takes the container's see-through pictures (ProRes 4444 in a QuickTime
/// movie, VP9 with alpha in a WebM) and is refused by a container that has none. The file appears
/// only when whole, and an existing file is never replaced.
pub fn write_with_ffmpeg(
    document: &Document,
    path: &Path,
    container: &Container,
    backdrop: Backdrop,
) -> Result<(), String> {
    let see_through;
    let container = if backdrop.is_transparent() {
        see_through = container
            .with_alpha()
            .ok_or_else(|| flare_render::no_transparency(&format!("{}s", container.label)))?;
        &see_through
    } else {
        container
    };
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
    let size = video_size(stage.width(), stage.height());
    let fps = f64::from(num) / f64::from(den);
    let seconds = frames as f64 / fps;

    let drawn = Frames::new(document, size, backdrop)?;
    let mut track = audio::Track::new(document, (num, den), seconds)?;
    // The sound is written to a file beside the output, and removed with the staging files.
    let sound = match track.as_mut() {
        Some(track) => {
            let mut file = tempfile::Builder::new()
                .prefix(".flare-sound-")
                .suffix(".pcm")
                .tempfile_in(directory)
                .map_err(|e| format!("Cannot write the movie: {e}"))?;
            while track.next_start().is_some() {
                for sample in track.next_pcm() {
                    file.write_all(&sample.to_le_bytes())
                        .map_err(|e| format!("Cannot write the movie: {e}"))?;
                }
            }
            Some(file)
        }
        None => None,
    };
    let staged = tempfile::Builder::new()
        .prefix(".flare-movie-")
        .suffix(".part")
        .tempfile_in(directory)
        .map_err(|e| format!("Cannot write the movie: {e}"))?;
    let movie = Movie {
        size,
        rate: (num, den),
        frames: u32::try_from(frames).map_err(|_| "The movie is too long.".to_owned())?,
        sound: sound.as_ref().map(tempfile::NamedTempFile::path),
    };
    flare_ffmpeg::encode(container, staged.path(), &movie, |n| drawn.rgba(n))
        .map_err(|error| describe(error, container))?;
    staged
        .persist(path)
        .map_err(|e| format!("Cannot write the movie: {}", e.error))?;
    Ok(())
}

/// What a failure means to the person who asked for the movie.
fn describe(error: FfmpegError, container: &Container) -> String {
    match error {
        FfmpegError::NotInstalled => format!(
            "Writing a {} needs ffmpeg, which is not on this computer: install it from ffmpeg.org and put it on the PATH.",
            container.label
        ),
        other => other.to_string(),
    }
}
