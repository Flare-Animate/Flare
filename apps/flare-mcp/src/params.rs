//! The arguments of each tool. Their doc comments become the schema descriptions agents read.

use schemars::JsonSchema;
use serde::Deserialize;

/// Arguments of a tool that changes nothing but needs a revision check.
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct Guarded {
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FrameArgs {
    /// Frame number, counting from 0.
    pub frame: u32,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExportPng {
    /// Frame number, counting from 0.
    pub frame: u32,
    /// File name only: letters, digits, '.', '_' or '-'.
    pub name: String,
    /// Output width in pixels; defaults to the stage width.
    pub width: Option<u32>,
    /// Output height in pixels; defaults to the stage height.
    pub height: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DrawRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Fill, RRGGBB or RRGGBBAA hex. Defaults to black.
    pub color: Option<String>,
    /// Outline color, RRGGBB or RRGGBBAA hex. No outline when omitted.
    pub stroke: Option<String>,
    /// Outline width; defaults to 2.
    pub stroke_width: Option<f64>,
    /// Outline pattern: solid (default), dashed, dotted, stipple, hatched, ragged, herringbone, rope or zigzag.
    pub stroke_style: Option<String>,
    /// First frame; defaults to 0.
    pub start: Option<u32>,
    /// Frames the rectangle lasts; defaults to 1.
    pub duration: Option<u32>,
    /// Layer id; defaults to the topmost layer that is not a folder or the camera. If a keyframe
    /// of the layer covers `start` the rectangle joins it and `duration` is not used.
    pub layer: Option<String>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddLayer {
    pub name: String,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddSymbol {
    /// A name the library already has is refused.
    pub name: String,
    /// "graphic" (the default), "movie clip" or "button".
    pub kind: Option<String>,
    /// Give the symbol one rectangle: its left edge.
    pub x: Option<f64>,
    pub y: Option<f64>,
    /// Setting a width makes the symbol hold a rectangle.
    pub width: Option<f64>,
    pub height: Option<f64>,
    /// Fill of that rectangle, RRGGBB or RRGGBBAA hex.
    pub color: Option<String>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PlaceInstance {
    /// Symbol id (from get_info or add_symbol).
    pub symbol: String,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub start: Option<u32>,
    pub duration: Option<u32>,
    /// Interpolate to the next exposure of the same symbol.
    pub tween: Option<bool>,
    /// Layer id; defaults to the first layer.
    pub layer: Option<String>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetStage {
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// RRGGBB or RRGGBBAA hex.
    pub background: Option<String>,
    /// Frames per second, from 0.01 to 240 (for example 24, 29.97 or 12.5).
    pub fps: Option<f64>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RenameScene {
    pub name: String,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetCamera {
    /// Frame number, counting from 0, that the camera is pointed from.
    pub frame: u32,
    /// The point of the scene at the middle of the picture. Stays as it is when omitted.
    pub x: Option<f64>,
    pub y: Option<f64>,
    /// 1 shows the stage as it is, 2 twice as large; from 0.01 to 100. Stays when omitted.
    pub zoom: Option<f64>,
    /// Degrees, clockwise. Stays when omitted.
    pub rotation: Option<f64>,
    /// "motion" moves the camera to the next keyframe's; "none" holds it until then. Keeps what
    /// the keyframe has when omitted.
    pub motion: Option<String>,
    /// The motion's ease, from -100 (starts slowly) to 100 (ends slowly).
    pub ease: Option<i8>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExportName {
    /// File name only: letters, digits, '.', '_' or '-'. The extension is added when missing.
    pub name: String,
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
}

/// The SVG export: a name and the frame to draw.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExportSvg {
    /// File name only: letters, digits, '.', '_' or '-'. The extension is added when missing.
    pub name: String,
    /// The frame to draw, from 0. Defaults to the first.
    pub frame: Option<u32>,
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
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExportSpriteSheet {
    /// File name only: letters, digits, '.', '_' or '-'. `.png` is added when missing; the atlas
    /// is written beside it with the same name and `.json`.
    pub name: String,
    /// First frame (0-based). Default 0.
    pub start: Option<u32>,
    /// Last frame, inclusive. Default: the end of the scene.
    pub end: Option<u32>,
    /// Frames per row. Default: about square.
    pub columns: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PublishSwf {
    /// File name only; `.swf` is added when missing.
    pub name: String,
    /// Write an uncompressed SWF.
    pub no_compress: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PublishHtml5 {
    /// File name only; `.html` is added when missing.
    pub name: String,
    /// Stop on the last frame instead of looping.
    pub no_loop: Option<bool>,
    /// Leave the stage at the top left instead of centering it.
    pub no_center: Option<bool>,
    /// Scale the stage to fit the browser window.
    pub responsive: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExportXfl {
    /// File name only. Ending in `.fla` writes a zipped .fla; any other name is a folder of XFL.
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ImportProject {
    /// An XFL folder, `.xfl` or `.fla` already inside the export directory (a file name only).
    pub source: String,
    /// File name for the new project; `.flare` is added when missing.
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ImportSound {
    /// WAV, MP3, Ogg Vorbis or FLAC file name inside the configured export directory.
    pub source: String,
    /// Frame at which playback starts; defaults to 0.
    pub start: Option<u32>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SoundWaveform {
    /// Only this sound, by name or id; every sound when left out.
    pub sound: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddText {
    /// The words; a newline starts a new line.
    pub text: String,
    /// Where the top-left corner of the first line goes.
    pub x: f64,
    pub y: f64,
    /// A font family, or a comma-separated list of them; list_fonts says which there are.
    /// Defaults to the system's sans-serif, else the bundled Source Sans 3.
    pub family: Option<String>,
    /// Font size in stage units; defaults to 24.
    pub size: Option<f32>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    /// RRGGBB or RRGGBBAA hex. Defaults to black.
    pub color: Option<String>,
    /// First frame; defaults to 0.
    pub start: Option<u32>,
    /// Frames the text lasts; defaults to 1.
    pub duration: Option<u32>,
    /// Layer id; defaults to the first layer.
    pub layer: Option<String>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetText {
    /// The shape id add_text reported.
    pub shape: String,
    /// What is not given stays as it was.
    pub text: Option<String>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub family: Option<String>,
    pub size: Option<f32>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    /// RRGGBB or RRGGBBAA hex.
    pub color: Option<String>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BreakText {
    /// The shape id add_text reported.
    pub shape: String,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddBone {
    /// The instance the bone starts from (an id from evaluate_frame's paths). If an armature
    /// carries it, the bone grows from its joint.
    pub from: String,
    /// The instance the bone ends at. It must be on a plain layer; it joins the armature.
    pub to: String,
    /// Frame number, counting from 0, both instances are shown at. Defaults to 0.
    pub frame: Option<u32>,
    /// Where on `from` the joint goes; its registration point when omitted.
    pub from_x: Option<f64>,
    pub from_y: Option<f64>,
    /// Where on `to` the joint goes; its registration point when omitted.
    pub to_x: Option<f64>,
    pub to_y: Option<f64>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PoseBone {
    /// The armature layer; the first scene's only one when omitted.
    pub layer: Option<String>,
    /// The joint, counting from 0 (list_bones gives them).
    pub joint: usize,
    /// Frame number, counting from 0. Defaults to 0.
    pub frame: Option<u32>,
    /// Pull the joint toward this point (inverse kinematics; a root moves the whole armature).
    pub to_x: Option<f64>,
    pub to_y: Option<f64>,
    /// Or turn the joint to this many degrees from rest, clockwise.
    pub turn: Option<f64>,
    /// Or move a root joint, and the armature with it, by this much.
    pub by_x: Option<f64>,
    pub by_y: Option<f64>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct EvalCurve {
    /// Interpolation family, as `list_easings` names them (for example "quad", "bounce", "bezier").
    pub family: String,
    /// "in", "out", "in-out" or "auto" (the default).
    pub easing: Option<String>,
    /// Handle type: "auto", "auto-clamped" (the default), "vector", "aligned" or "free".
    pub handle: Option<String>,
    /// Bezier handles [right.dt, right.dv, left.dt, left.dv] in the unit square.
    pub handles: Option<[f64; 4]>,
    /// The time, from 0 to 1.
    pub t: f64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetInterpolation {
    /// Frame the keyframe starts at.
    pub at: u32,
    /// Layer id; defaults to the first layer that is not the camera.
    pub layer: Option<String>,
    /// Family to set; omit to clear the keyframe's interpolation back to its plain ease.
    pub family: Option<String>,
    pub easing: Option<String>,
    pub handle: Option<String>,
    pub handles: Option<[f64; 4]>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BoneJoint {
    /// The armature layer; the first scene's only one when omitted.
    pub layer: Option<String>,
    /// The joint, counting from 0.
    pub joint: usize,
    /// For limit_joint: the least and the most turn from rest, in degrees. Both, or neither to
    /// let the joint turn freely.
    pub min: Option<f64>,
    pub max: Option<f64>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Inbetween {
    /// Frame the keyframe starts at; it is split to make room.
    pub at: u32,
    /// How many in-between keyframes to make, 1 to 1000; the keyframe must last more frames.
    pub count: u32,
    /// Layer id; defaults to the first layer that is not the camera.
    pub layer: Option<String>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BitmapInbetweens {
    /// Frame of the first bitmap keyframe.
    pub from: u32,
    /// Frame of the second bitmap keyframe; both must hold bitmaps of the same size.
    pub to: u32,
    /// How many pictures to make between them, 1 to 64.
    pub count: u32,
    /// File name prefix, no directory: letters, digits, '.', '_' or '-'. The files are
    /// `<name>-001.png` and so on in the export directory.
    pub name: String,
    /// Layer id; defaults to the first layer that is not the camera.
    pub layer: Option<String>,
    /// Pyramid levels for the flow estimate, 1 to 6 (default 4).
    pub levels: Option<u32>,
    /// Gauss-Newton steps per level, 1 to 16 (default 4).
    pub iterations: Option<u32>,
    /// Half-width of the solve window, 1 to 8 (default 3).
    pub window: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ArmatureLayer {
    /// The armature layer; the first scene's only one when omitted.
    pub layer: Option<String>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AnimateShortcutsParams {
    /// The text of Animate's shortcut file (`Shortcuts/<set>.xml` in its settings folder).
    pub xml: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ShortcutsParams {
    /// A set: "flare" (the default), "opentoonz", "animate" or "tahoma".
    pub set: Option<String>,
    /// The text of a shortcut file (as `export` returns it); used instead of `set`.
    pub file: Option<String>,
    /// Also return the checked file.
    pub export: Option<bool>,
    /// Only the commands this finds: words of their English name or id ("save as"), or keys
    /// ("cmd+s", "shift+f5", "option"). Each row then carries the command's name too.
    pub search: Option<String>,
}
