//! Arguments of the tools that render the project to pictures and movies. Each takes
//! `transparent`, which leaves the stage colour out where the format has an alpha channel.

use schemars::JsonSchema;
use serde::Deserialize;

/// A whole-animation export with nothing to choose but its name and its background.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExportAnimation {
    /// File name only: letters, digits, '.', '_' or '-'. The extension is added when missing.
    pub name: String,
    /// Leave the stage colour out, so what is not drawn is see-through. Default false. A GIF
    /// shows each pixel or not, so edges are cut at half coverage; an MP4 has no alpha and
    /// refuses it.
    pub transparent: Option<bool>,
}

/// The movie formats `export_movie` writes: a QuickTime movie made by Flare itself, the others
/// through ffmpeg.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum MovieKind {
    Mov,
    Avi,
    Mkv,
    Webm,
    Mpeg,
    Flv,
}

/// The movie export: a name and the container.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExportMovie {
    /// File name only: letters, digits, '.', '_' or '-'. The extension for the format is added
    /// when missing.
    pub name: String,
    /// The container: mov, avi, mkv, webm, mpeg or flv. Every one but mov needs ffmpeg on the
    /// computer.
    pub format: MovieKind,
    /// Leave the stage colour out, so what is not drawn is see-through. Default false. Only mov
    /// (ProRes 4444, which then needs ffmpeg too) and webm (VP9 with alpha) can; the others
    /// refuse it.
    pub transparent: Option<bool>,
}

/// The SVG and PSD exports: a name and the frame to draw.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExportSvg {
    /// File name only: letters, digits, '.', '_' or '-'. The extension is added when missing.
    pub name: String,
    /// The frame to draw, from 0. Defaults to the first.
    pub frame: Option<u32>,
    /// Leave the stage colour out, so what is not drawn is see-through. Default false.
    pub transparent: Option<bool>,
}

/// The still-image formats `export_still` writes.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum StillKind {
    Png,
    Jpeg,
    Bmp,
    Tga,
    Tiff,
    Webp,
    Qoi,
    Pnm,
    Ico,
    Exr,
    Hdr,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExportStill {
    /// File name only: letters, digits, '.', '_' or '-'. The extension for the format is added
    /// when missing.
    pub name: String,
    /// The frame of the first scene to draw (0-based). Default 0.
    pub frame: Option<u32>,
    /// png (the default), jpeg, bmp or tga. A JPEG has no transparency and is always opaque.
    pub format: Option<StillKind>,
    /// JPEG quality from 1 to 100. Default 90. Only for jpeg.
    pub quality: Option<u8>,
    /// Leave the stage colour out, so what is not drawn is see-through. Default false. jpeg and
    /// hdr have no alpha and refuse it.
    pub transparent: Option<bool>,
}

/// The backdrop `transparent` asks for.
pub fn backdrop(transparent: Option<bool>) -> flare_runtime::Backdrop {
    flare_runtime::Backdrop::transparent_if(transparent.unwrap_or(false))
}
