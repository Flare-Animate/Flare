//! Writing frames as PNG files.

use anyhow::{Context, Result, bail};
use flare_core::Rgba8;
use flare_document::Document;
use serde_json::{Value, json};
use std::path::Path;

use super::*;

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
/// export never leaves a partial PNG.
pub(super) fn write_frame(
    document: &Document,
    frame: u32,
    target: &RenderTarget,
    out: &Path,
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
    image.write_png(std::io::BufWriter::new(&mut staged))?;
    staged.persist(out).map_err(|error| error.error)?;
    Ok(())
}

/// Renders one frame to a PNG file.
pub fn export(
    path: &Path,
    out: &Path,
    frame: u32,
    width: Option<u32>,
    height: Option<u32>,
    transparent: bool,
) -> Result<Value> {
    let session = open(path)?;
    let target = render_target(session.document(), width, height, transparent);
    write_frame(session.document(), frame, &target, out)?;
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
        )?;
    }
    Ok(
        json!({ "directory": directory.to_string_lossy(), "frames": count, "first": start, "last": last }),
    )
}
