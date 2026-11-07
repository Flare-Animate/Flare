//! Writing frames as still images (PNG, JPEG, BMP, TGA).

use anyhow::{Context, Result, bail};
use flare_core::Rgba8;
use flare_document::Document;
use flare_render::StillFormat;
use serde_json::{Value, json};
use std::path::Path;

use super::*;

/// The quality a JPEG gets when `--quality` is not given.
pub(super) const DEFAULT_JPEG_QUALITY: u8 = 90;

/// What `export` writes.
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum ExportFormat {
    Png,
    Jpeg,
    Bmp,
    Tga,
    Tiff,
    /// Lossless WebP (lossy WebP is not written: no pure-Rust encoder).
    Webp,
    Qoi,
    /// Netpbm arbitrary map (PAM), which keeps transparency.
    Pnm,
    /// ICO, at most 256 pixels a side.
    Ico,
    Exr,
    /// Radiance HDR. Opaque only.
    Hdr,
    Gif,
    /// An animated PNG: every frame, with full colour and alpha.
    Apng,
    /// One frame as an SVG picture: the shapes as paths, the pictures embedded.
    Svg,
    Mp4,
}

impl ExportFormat {
    /// The still format this is, or `None` for GIF and MP4, which hold every frame. A quality
    /// is only for JPEG, and a quality given for anything else is refused rather than ignored.
    pub fn still(self, quality: Option<u8>) -> Result<Option<StillFormat>> {
        if quality.is_some() && !matches!(self, Self::Jpeg) {
            bail!("--quality is for JPEG only");
        }
        Ok(match self {
            Self::Png => Some(StillFormat::Png),
            Self::Jpeg => Some(StillFormat::Jpeg {
                quality: quality.unwrap_or(DEFAULT_JPEG_QUALITY),
            }),
            Self::Bmp => Some(StillFormat::Bmp),
            Self::Tga => Some(StillFormat::Tga),
            Self::Tiff => Some(StillFormat::Tiff),
            Self::Webp => Some(StillFormat::WebpLossless),
            Self::Qoi => Some(StillFormat::Qoi),
            Self::Pnm => Some(StillFormat::Pnm),
            Self::Ico => Some(StillFormat::Ico),
            Self::Exr => Some(StillFormat::Exr),
            Self::Hdr => Some(StillFormat::Hdr),
            Self::Gif | Self::Apng | Self::Svg | Self::Mp4 => None,
        })
    }
}

pub(super) struct RenderTarget {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) background: Rgba8,
}

/// Size and background default to the document's stage.
pub(super) fn render_target(
    document: &Document,
    width: Option<u32>,
    height: Option<u32>,
    transparent: bool,
) -> RenderTarget {
    let stage = document.stage();
    RenderTarget {
        width: width.unwrap_or(stage.width()),
        height: height.unwrap_or(stage.height()),
        background: if transparent {
            Rgba8::TRANSPARENT
        } else {
            stage.background()
        },
    }
}

/// Renders one frame and writes it atomically (temp file, then rename), so a failed
/// export never leaves a partial image.
pub(super) fn write_frame(
    document: &Document,
    frame: u32,
    target: &RenderTarget,
    out: &Path,
    format: StillFormat,
) -> Result<()> {
    let scene = document
        .scenes()
        .first()
        .context("document has no scene")?
        .id();
    let evaluated = flare_eval::evaluate_frame(document, scene, frame)?;
    let image = flare_render::render(&evaluated, target.width, target.height, target.background)?;
    let directory = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut staged = tempfile::NamedTempFile::new_in(directory)?;
    image.write_still(std::io::BufWriter::new(&mut staged), format)?;
    staged.persist(out).map_err(|error| error.error)?;
    Ok(())
}

/// Renders one frame to a still image in `format`. A JPEG has no transparency, so a
/// transparent background is refused here with the reason, not left to the encoder.
pub fn export(
    path: &Path,
    out: &Path,
    frame: u32,
    width: Option<u32>,
    height: Option<u32>,
    transparent: bool,
    format: StillFormat,
) -> Result<Value> {
    if transparent && matches!(format, StillFormat::Jpeg { .. } | StillFormat::Hdr) {
        bail!("this format has no transparency: drop --transparent, or export a PNG");
    }
    let session = open(path)?;
    let target = render_target(session.document(), width, height, transparent);
    write_frame(session.document(), frame, &target, out, format)?;
    Ok(json!({ "exported": out.to_string_lossy(), "width": target.width, "height": target.height }))
}

/// Most frames one `export-frames` call will write.
pub(super) const MAX_SEQUENCE_FRAMES: u32 = 100_000;

/// Renders a frame range to numbered PNGs (`frame_00000.png`, ...) in `directory`.
/// The range defaults to the whole first scene.
pub fn export_frames(
    path: &Path,
    directory: &Path,
    start: u32,
    end: Option<u32>,
    width: Option<u32>,
    height: Option<u32>,
    transparent: bool,
) -> Result<Value> {
    let session = open(path)?;
    let document = session.document();
    let length = document
        .scenes()
        .first()
        .context("document has no scene")?
        .timeline()
        .frame_count();
    let last = end.unwrap_or_else(|| u32::try_from(length.saturating_sub(1)).unwrap_or(u32::MAX));
    if last < start {
        bail!("end frame {last} is before start frame {start}");
    }
    let count = last - start + 1;
    if count > MAX_SEQUENCE_FRAMES {
        bail!("{count} frames exceeds the limit of {MAX_SEQUENCE_FRAMES} per export");
    }
    std::fs::create_dir_all(directory)
        .with_context(|| format!("cannot create {}", directory.display()))?;
    let target = render_target(document, width, height, transparent);
    for frame in start..=last {
        write_frame(
            document,
            frame,
            &target,
            &directory.join(format!("frame_{frame:05}.png")),
            StillFormat::Png,
        )?;
    }
    Ok(
        json!({ "directory": directory.to_string_lossy(), "frames": count, "first": start, "last": last }),
    )
}
