//! Parameters of the tools that bring a video in to draw over.

use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PlanVideoImport {
    /// The movie's file name inside the configured export directory (MP4, MOV, AVI, MKV, WebM,
    /// MPEG or FLV).
    pub source: String,
    /// Pictures to take from each second of the video, each one frame of the timeline; the
    /// document's rate by default, which keeps the video's timing.
    pub fps: Option<f64>,
    /// The most of the video to read, from its start, in minutes (up to 60); as much of the first
    /// minute as fits by default.
    pub max_minutes: Option<f64>,
    /// The pictures' longest side in pixels, 16 to 3840 (960 by default); never enlarged.
    pub max_side: Option<u32>,
    /// The JPEG quality the pictures are kept at, 1 to 100 (92 by default).
    pub quality: Option<u8>,
    /// Keep the pictures exactly, not as JPEGs (about nine times the room in the file).
    pub lossless: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ImportVideo {
    /// The movie's file name inside the configured export directory.
    pub source: String,
    /// As in plan_video_import.
    pub fps: Option<f64>,
    pub max_minutes: Option<f64>,
    pub max_side: Option<u32>,
    pub quality: Option<u8>,
    pub lossless: Option<bool>,
    /// How opaque the stage draws the layer while editing, 0 to 100 (50 by default).
    pub opacity: Option<u8>,
    /// The frame the video starts on; defaults to 0.
    pub frame: Option<u32>,
    /// The layer's and the symbol's name; the file's name by default.
    pub name: Option<String>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetBitmapCompression {
    /// "photo" (a JPEG: about a ninth of the room for a frame of video, which looks the same) or
    /// "lossless" (every pixel exactly).
    pub compression: String,
    /// For photo: the JPEG quality, 1 to 100 (92 by default).
    pub quality: Option<u8>,
    /// The bitmaps to change, by id; every bitmap in the library when left out.
    pub bitmaps: Option<Vec<String>>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}
