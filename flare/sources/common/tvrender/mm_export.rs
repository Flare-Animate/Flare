//! File > Export: one frame as a PNG or JPEG image, the movie as an animated GIF, as a PNG
//! sequence, or as video through ffmpeg. Frames are drawn by render.rs, numbered across the
//! scenes, at `scale` times the stage's size.

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use tiny_skia::Pixmap;

use crate::Result;
use crate::doc::Document;
use crate::encode::{self, Gif};
use crate::render::render;

/// What Export Image writes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Format {
    Png,
    /// At a quality of 1..=100.
    Jpeg(u8),
}

/// What Export Video says when ffmpeg can't be found.
pub const FFMPEG_MISSING: &str =
    "Export Video/Media encodes through ffmpeg, which isn't on the PATH. Install ffmpeg (ffmpeg.org), make sure the folder holding ffmpeg.exe is on the PATH, then export again.";

/// The movie's frames, every scene's one after another, as `render` numbers them.
pub fn frame_count(doc: &Document) -> u32 {
    doc.scenes.iter().map(|s| s.frame_count()).sum()
}

/// The size `render` draws the stage at `scale`.
pub fn size(doc: &Document, scale: f32) -> (u32, u32) {
    ((doc.width * scale).ceil() as u32, (doc.height * scale).ceil() as u32)
}

/// Frame `frame` as an image file. Transparent leaves out the stage colour where nothing is drawn
/// (a JPEG, having no alpha, keeps it).
pub fn image(doc: &Document, frame: u32, scale: f32, transparent: bool, format: Format) -> Result<(Vec<u8>, BTreeSet<String>)> {
    let doc = backdrop(doc, transparent && format == Format::Png);
    let (pixmap, skipped) = render(&doc, frame, scale)?;
    let bytes = match format {
        Format::Png => pixmap.encode_png()?,
        Format::Jpeg(quality) => {
            let (w, h) = (short(pixmap.width())?, short(pixmap.height())?);
            encode::jpeg(&straight(&pixmap), w, h, quality)
        }
    };
    Ok((bytes, skipped))
}

/// `frames` as an animated GIF timed by the frame rate, each frame with a palette of its own. It
/// loops for ever, or plays `repeat` more times; transparent, the stage colour is left out.
pub fn gif(doc: &Document, frames: Range<u32>, scale: f32, transparent: bool, repeat: Option<u16>) -> Result<(Vec<u8>, BTreeSet<String>)> {
    let doc = backdrop(doc, transparent);
    let (w, h) = size(&doc, scale);
    let mut gif = Gif::new(short(w)?, short(h)?, repeat);
    let fps = doc.frame_rate.max(0.01) as f64;
    // In hundredths of a second, rounded so the frames add up to the movie's length.
    let at = |n: u32| (n as f64 * 100.0 / fps).round() as u64;
    let mut skipped = BTreeSet::new();
    for (i, f) in frames.enumerate() {
        let (pixmap, s) = render(&doc, f, scale)?;
        skipped.extend(s);
        let delay = (at(i as u32 + 1) - at(i as u32)).min(u16::MAX as u64) as u16;
        gif.frame(&straight(&pixmap), delay);
    }
    Ok((gif.finish(), skipped))
}

/// Export Movie as a PNG sequence: a file per frame, numbered from `path`'s name: walk.png makes
/// walk0001.png, walk0002.png... Returns the files written.
pub fn png_sequence(doc: &Document, path: &Path, scale: f32, transparent: bool) -> Result<(Vec<PathBuf>, BTreeSet<String>)> {
    let doc = backdrop(doc, transparent);
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let dir = path.parent().unwrap_or(Path::new(""));
    std::fs::create_dir_all(dir)?;
    let (mut files, mut skipped) = (Vec::new(), BTreeSet::new());
    for f in 0..frame_count(&doc) {
        let (pixmap, s) = render(&doc, f, scale)?;
        skipped.extend(s);
        let file = dir.join(format!("{stem}{:04}.png", f + 1));
        pixmap.save_png(&file)?;
        files.push(file);
    }
    Ok((files, skipped))
}

/// Export Video/Media: the movie encoded by ffmpeg into `path`, its format following the file's
/// extension (H.264 for .mp4 and .mov). The frames go to ffmpeg's input as raw RGBA.
pub fn video(doc: &Document, path: &Path, scale: f32) -> Result<BTreeSet<String>> {
    let (w, h) = size(doc, scale);
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-hide_banner", "-loglevel", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgba"]);
    cmd.args(["-s", &format!("{w}x{h}"), "-r", &doc.frame_rate.to_string(), "-i", "-"]);
    // H.264 in yuv420p, which every player plays, wants an even size.
    cmd.args(["-vf", "pad=ceil(iw/2)*2:ceil(ih/2)*2", "-pix_fmt", "yuv420p"]);
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if matches!(ext.as_str(), "mp4" | "mov" | "m4v") {
        cmd.args(["-c:v", "libx264"]);
    }
    cmd.arg(path).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::piped());
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000); // CREATE_NO_WINDOW
    let mut child = cmd.spawn().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => FFMPEG_MISSING.to_string(),
        _ => format!("Couldn't start ffmpeg: {e}"),
    })?;
    // Read ffmpeg's errors meanwhile, so a full pipe can't stall it.
    let mut errors = child.stderr.take();
    let errors = std::thread::spawn(move || {
        let mut text = String::new();
        errors.as_mut().map(|e| e.read_to_string(&mut text));
        text
    });
    let mut input = child.stdin.take().ok_or("ffmpeg took no input")?;
    let mut skipped = BTreeSet::new();
    let mut fed = || -> Result<()> {
        for f in 0..frame_count(doc) {
            let (pixmap, s) = render(doc, f, scale)?;
            skipped.extend(s);
            // Opaque over the stage colour, so premultiplied is straight.
            input.write_all(pixmap.data())?;
        }
        Ok(())
    };
    let fed = fed();
    drop(input);
    let status = child.wait()?;
    let errors = errors.join().unwrap_or_default();
    if !status.success() {
        return Err(format!("ffmpeg couldn't make the video.\n\n{}", errors.trim()).into());
    }
    fed?;
    Ok(skipped)
}

/// Whether ffmpeg runs from the PATH, for Export Video/Media.
pub fn ffmpeg_found() -> bool {
    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-version").stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000);
    cmd.status().is_ok_and(|s| s.success())
}

/// `doc`, with a stage colour that draws nothing when `transparent`.
fn backdrop(doc: &Document, transparent: bool) -> Cow<'_, Document> {
    if !transparent {
        return Cow::Borrowed(doc);
    }
    let mut clear = doc.clone();
    clear.background[3] = 0.0;
    Cow::Owned(clear)
}

/// A pixmap's pixels as straight (not premultiplied) RGBA.
fn straight(pixmap: &Pixmap) -> Vec<u8> {
    pixmap.pixels().iter().flat_map(|p| {
        let c = p.demultiply();
        [c.red(), c.green(), c.blue(), c.alpha()]
    }).collect()
}

/// A size that fits an image file's 16-bit fields.
fn short(v: u32) -> Result<u16> {
    Ok(u16::try_from(v).map_err(|_| format!("{v} pixels is too large for this format"))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::*;
    use crate::draw;

    /// 40 × 30 at 10 fps: a red square moving right a step a frame for five frames, over white.
    fn movie() -> Document {
        let mut doc = Document::new(40.0, 30.0, 10.0);
        let layer = &mut doc.scenes[0].layers[0];
        for f in 0..5u32 {
            if f > 0 {
                layer.insert_blank_keyframe(f);
            }
            let x = 5.0 * f as f32;
            draw::add(&mut layer.frames[f as usize], draw::rectangle([(x, 10.0), (x + 10.0, 20.0)], 0.0, None, Some(Paint::Solid([1.0, 0.0, 0.0, 1.0]))));
        }
        doc
    }

    #[test]
    fn images_png_and_jpeg() {
        let doc = movie();
        let (png, skipped) = image(&doc, 2, 2.0, true, Format::Png).unwrap();
        assert!(skipped.is_empty());
        let back = Pixmap::decode_png(&png).unwrap();
        assert_eq!((back.width(), back.height()), (80, 60));
        assert_eq!(back.pixel(0, 0).unwrap().alpha(), 0, "transparent where the stage is");
        assert_eq!(back.pixel(30, 30).unwrap().demultiply().red(), 255, "the square, at frame 3");
        let (jpeg, _) = image(&doc, 0, 1.0, true, Format::Jpeg(90)).unwrap();
        assert_eq!(&jpeg[..2], [0xFF, 0xD8]);
        assert!(image(&doc, 9, 1.0, false, Format::Png).is_err(), "past the last frame");
    }

    #[test]
    fn an_animated_gif_has_every_frame() {
        let doc = movie();
        let (bytes, _) = gif(&doc, 0..frame_count(&doc), 1.0, false, None).unwrap();
        let frames = crate::encode::tests::gif_frames(&bytes);
        assert_eq!(frames.len(), 5);
        // The square's left edge moves 5 px a frame: on row 15, red starts at x = 5f.
        for (f, pixels) in frames.iter().enumerate() {
            let first_red = (0..40).find(|&x| pixels[15 * 40 + x] == Some([255, 0, 0]));
            assert_eq!(first_red, Some(5 * f), "frame {f}");
        }
        // 10 fps: ten hundredths a frame, read from the first frame's Graphic Control Extension.
        let gce = bytes.windows(3).position(|w| w == [0x21, 0xF9, 4]).unwrap();
        assert_eq!(u16::from_le_bytes([bytes[gce + 4], bytes[gce + 5]]), 10);
    }

    #[test]
    fn a_png_sequence_numbers_its_files() {
        let dir = std::env::temp_dir().join(format!("minnamate-sequence-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (files, _) = png_sequence(&movie(), &dir.join("walk.png"), 0.5, false).unwrap();
        let names: Vec<_> = files.iter().map(|f| f.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["walk0001.png", "walk0002.png", "walk0003.png", "walk0004.png", "walk0005.png"]);
        assert_eq!(Pixmap::load_png(&files[4]).unwrap().width(), 20);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn video_goes_through_ffmpeg_or_says_it_cant() {
        let path = std::env::temp_dir().join(format!("minnamate-video-{}.mp4", std::process::id()));
        match video(&movie(), &path, 1.0) {
            Ok(_) => assert!(std::fs::metadata(&path).is_ok_and(|m| m.len() > 0)),
            Err(e) => assert!(!ffmpeg_found() && e.to_string() == FFMPEG_MISSING, "{e}"),
        }
        let _ = std::fs::remove_file(path);
    }
}
