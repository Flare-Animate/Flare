//! The commands the line understands, and their arguments.

use std::path::PathBuf;

use clap::Subcommand;

use crate::project;

#[derive(Subcommand)]
pub(crate) enum Command {
    /// Identify a possible container from at most 4096 bytes, without executing it.
    Inspect { path: PathBuf },
    /// Inspect a validated rational frame rate and one zero-based frame boundary.
    Rate {
        numerator: u32,
        denominator: u32,
        #[arg(long, default_value_t = 0)]
        frame: u64,
    },
    /// Validate a stop-motion session's rate, hold and onion skin and print its settings. Sessions
    /// are not saved in a project yet, so this writes no file.
    StopMotionNew {
        #[arg(long, default_value_t = 12)]
        fps: u32,
        /// Timeline frames each captured frame is held for.
        #[arg(long, default_value_t = 1)]
        hold: u32,
        /// Earlier frames shown as ghosts, 0 to 5.
        #[arg(long, default_value_t = 0)]
        onion_before: u8,
        /// Later frames shown as ghosts, 0 to 5.
        #[arg(long, default_value_t = 0)]
        onion_after: u8,
        /// Ghost opacity in percent, 0 to 100.
        #[arg(long, default_value_t = 50)]
        onion_opacity: u8,
    },
    /// Capture a frame from a camera. Not built yet: fails with the named error.
    StopMotionCapture,
    /// Create an empty project with one scene and one layer.
    New {
        path: PathBuf,
        #[arg(long, default_value_t = 24)]
        numerator: u32,
        #[arg(long, default_value_t = 1)]
        denominator: u32,
        /// Stage width in pixels.
        #[arg(long, default_value_t = 1920)]
        width: u32,
        /// Stage height in pixels.
        #[arg(long, default_value_t = 1080)]
        height: u32,
        /// Stage background color RRGGBB or RRGGBBAA.
        #[arg(long, default_value = "FFFFFF")]
        background: String,
    },
    /// Change the stage size and/or background of a project.
    SetStage {
        path: PathBuf,
        #[arg(long)]
        width: Option<u32>,
        #[arg(long)]
        height: Option<u32>,
        #[arg(long)]
        background: Option<String>,
        /// Frames per second, from 0.01 to 240 (for example 24, 29.97 or 12.5).
        #[arg(long)]
        fps: Option<f64>,
    },
    /// Rename the first scene of a project.
    RenameScene { path: PathBuf, name: String },
    /// Add a sound file (WAV, MP3, Ogg Vorbis or FLAC) on a new layer.
    SoundImport {
        path: PathBuf,
        sound: PathBuf,
        /// Frame at which playback starts (zero based).
        #[arg(long, default_value_t = 0)]
        at: u32,
    },
    /// Bring a video in to draw over: its pictures, taken at --fps, one to a frame of the
    /// timeline from --frame, in one Graphic symbol on a new see-through layer at the bottom of
    /// the first scene. Move or scale that instance and every picture follows. Prints what the
    /// import makes (frames, memory, the animation's length at the document's rate) and refuses
    /// one too large for the document, saying what would fit. Needs ffmpeg on the PATH.
    VideoImport(crate::video::VideoImportArgs),
    /// How loud each sound is on every frame from its cue (`[low, high]` samples), to animate to.
    SoundWaveform {
        path: PathBuf,
        /// Only this sound, by name or id.
        #[arg(long)]
        sound: Option<String>,
    },
    /// Summarize a project.
    Info { path: PathBuf },
    /// Add a symbol to the library, optionally holding one filled rectangle. A name the library
    /// already has is refused.
    AddSymbol {
        path: PathBuf,
        name: String,
        /// graphic, movie clip or button.
        #[arg(long, value_parser = crate::symbol::kind_arg, default_value = "graphic")]
        kind: flare_document::SymbolKind,
        /// Rectangle as X Y WIDTH HEIGHT in document units.
        #[arg(long, num_args = 4, value_names = ["X", "Y", "W", "H"], allow_negative_numbers = true)]
        rect: Option<Vec<f64>>,
        /// Fill color RRGGBB or RRGGBBAA.
        #[arg(long, default_value = "000000")]
        color: String,
        /// Outline color RRGGBB or RRGGBBAA (no outline when omitted).
        #[arg(long)]
        stroke: Option<String>,
        /// Outline width in document units.
        #[arg(long, default_value_t = 2.0)]
        stroke_width: f64,
    },
    /// Draw a filled rectangle on the first layer of the first scene.
    DrawRect {
        path: PathBuf,
        #[arg(long, num_args = 4, value_names = ["X", "Y", "W", "H"], allow_negative_numbers = true, required = true)]
        rect: Vec<f64>,
        #[arg(long, default_value = "000000")]
        color: String,
        /// Outline color RRGGBB or RRGGBBAA (no outline when omitted).
        #[arg(long)]
        stroke: Option<String>,
        /// Outline width in document units.
        #[arg(long, default_value_t = 2.0)]
        stroke_width: f64,
        /// Outline pattern: solid, dashed, dotted, stipple, hatched, ragged, herringbone, rope or zigzag.
        #[arg(long)]
        stroke_style: Option<String>,
        #[arg(long, default_value_t = 0)]
        start: u32,
        #[arg(long, default_value_t = 1)]
        duration: u32,
        #[arg(long)]
        layer: Option<uuid::Uuid>,
    },
    /// Magic Wand: pick the pixels of a bitmap whose color is like the one at each point (stage
    /// coordinates, in the first scene) and clear them, or with --keep clear all the others.
    MagicWand {
        path: PathBuf,
        /// A point on the bitmap; repeat to add the pixels picked from more points.
        #[arg(long, num_args = 2, value_names = ["X", "Y"], allow_negative_numbers = true, required = true, action = clap::ArgAction::Append)]
        at: Vec<f64>,
        #[arg(long, default_value_t = 0)]
        frame: u32,
        /// How far a color may be from the clicked one, 0 to 200.
        #[arg(long, default_value_t = magic_wand_default_threshold(), value_parser = clap::value_parser!(u8).range(0..=200))]
        threshold: u8,
        /// Pick every pixel of a like color in the picture, not only those joined to the point.
        #[arg(long)]
        global: bool,
        /// How soft the edge left behind is: pixels, rough, normal or smooth.
        #[arg(long, default_value = "normal")]
        smoothing: String,
        /// Clear every pixel except the picked ones.
        #[arg(long)]
        keep: bool,
        /// Say how many pixels would be picked and change nothing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Blur: soften the bitmap under the first point along a stroke of stage points.
    Blur(project::BlendArgs),
    /// Smudge: push the colors of the bitmap under the first point along a stroke of stage points.
    Smudge(project::BlendArgs),
    /// Add editable text to a layer of the first scene.
    AddText {
        path: PathBuf,
        /// The words; \n starts a new line.
        text: String,
        #[arg(long, default_value_t = 0.0, allow_negative_numbers = true)]
        x: f64,
        #[arg(long, default_value_t = 0.0, allow_negative_numbers = true)]
        y: f64,
        /// A font family, or a comma-separated list; `flare fonts` lists the ones there are.
        #[arg(long, default_value = flare_text::DEFAULT_FAMILY)]
        family: String,
        #[arg(long, default_value_t = 24.0)]
        size: f32,
        #[arg(long)]
        bold: bool,
        #[arg(long)]
        italic: bool,
        #[arg(long, default_value = "000000")]
        color: String,
        #[arg(long, default_value_t = 0)]
        start: u32,
        #[arg(long, default_value_t = 1)]
        duration: u32,
        #[arg(long)]
        layer: Option<uuid::Uuid>,
        /// A font file (.ttf, .otf) to make known by its family name; repeatable.
        #[arg(long = "font-file")]
        font_files: Vec<PathBuf>,
    },
    /// Change a text object: its words, font, place or color.
    SetText {
        path: PathBuf,
        /// The shape `add-text` reported.
        shape: uuid::Uuid,
        #[arg(long)]
        text: Option<String>,
        #[arg(long, allow_negative_numbers = true)]
        x: Option<f64>,
        #[arg(long, allow_negative_numbers = true)]
        y: Option<f64>,
        #[arg(long)]
        family: Option<String>,
        #[arg(long)]
        size: Option<f32>,
        #[arg(long)]
        bold: Option<bool>,
        #[arg(long)]
        italic: Option<bool>,
        #[arg(long)]
        color: Option<String>,
        /// A font file (.ttf, .otf) to make known by its family name; repeatable.
        #[arg(long = "font-file")]
        font_files: Vec<PathBuf>,
    },
    /// Take text apart into plain outlines.
    BreakText { path: PathBuf, shape: uuid::Uuid },
    /// List the font families text can be set in.
    Fonts {
        /// A font file (.ttf, .otf) to make known by its family name; repeatable.
        #[arg(long = "font-file")]
        font_files: Vec<PathBuf>,
    },
    /// Render the first scene: one frame as a PNG (the default), a JPEG, a BMP or a TGA, every frame
    /// as a looping GIF, or a video with its sound as an MP4 or a QuickTime movie (`mov`).
    Export {
        path: PathBuf,
        out: PathBuf,
        #[arg(long, value_enum, default_value_t = project::ExportFormat::Png)]
        format: project::ExportFormat,
        /// The frame to render (a still image only; default 0).
        #[arg(long)]
        frame: Option<u32>,
        /// Defaults to the stage width.
        #[arg(long)]
        width: Option<u32>,
        /// Defaults to the stage height.
        #[arg(long)]
        height: Option<u32>,
        /// Keep the background transparent instead of white (not for JPEG).
        #[arg(long)]
        transparent: bool,
        /// JPEG quality from 1 to 100 (JPEG only; default 90).
        #[arg(long, value_name = "1-100")]
        quality: Option<u8>,
    },
    /// Point the first scene's camera from a frame on (the camera layer is made when there is
    /// none). What is not given stays what the camera has at that frame.
    Camera {
        path: PathBuf,
        #[arg(long, default_value_t = 0)]
        frame: u32,
        /// The point of the scene at the middle of the picture.
        #[arg(long, allow_hyphen_values = true)]
        x: Option<f64>,
        #[arg(long, allow_hyphen_values = true)]
        y: Option<f64>,
        /// 1 shows the stage as it is, 2 twice as large; from 0.01 to 100.
        #[arg(long)]
        zoom: Option<f64>,
        /// Degrees, clockwise.
        #[arg(long, allow_hyphen_values = true)]
        rotation: Option<f64>,
        /// Hold the camera until the next keyframe, or move it to the next one.
        #[arg(long, value_enum)]
        motion: Option<project::CameraMotion>,
        /// The motion's ease, from -100 (starts slowly) to 100 (ends slowly); an ease alone makes it a
        /// motion.
        #[arg(long, allow_hyphen_values = true)]
        ease: Option<i8>,
    },
    /// Link instances with bones and pose the armature, as Animate's Bone tool and posing do.
    Bone {
        #[command(subcommand)]
        action: project::BoneAction,
    },
    /// List the armatures of the first scene at a frame: each joint, where it is posed, how far it
    /// is turned and the instance it carries.
    Bones {
        path: PathBuf,
        #[arg(long, default_value_t = 0)]
        frame: u32,
    },
    /// Keyframe interpolation: list the easings, evaluate a curve, set a keyframe's
    /// interpolation, make in-betweens.
    Tween {
        #[command(subcommand)]
        op: crate::tween::TweenCommand,
    },
    /// The game description: entities and components, gravity, runs under scripted input, export.
    Game {
        #[command(subcommand)]
        op: crate::game::GameCommand,
    },
    /// The reference mannequin: list what a figure can be, print one, place one on a layer.
    Mannequin {
        #[command(subcommand)]
        // Boxed: a figure's many options would make every command as large.
        op: Box<crate::mannequin::MannequinCommand>,
    },
    /// Keyboard shortcuts: list a set, export one as a file, check and import a file.
    Shortcuts {
        #[command(subcommand)]
        op: crate::shortcuts::ShortcutsCommand,
    },
    /// Add an empty layer below the existing ones in the first scene.
    AddLayer { path: PathBuf, name: String },
    /// Edit Multiple Frames: move, scale and turn everything in frames `--from` to `--to` on the
    /// shown, unlocked layers together, about the middle of all of it, as one undo step.
    TransformFrames {
        path: PathBuf,
        #[arg(long)]
        from: u32,
        #[arg(long)]
        to: u32,
        #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
        dx: f64,
        #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
        dy: f64,
        #[arg(long, default_value_t = 1.0, allow_hyphen_values = true)]
        scale_x: f64,
        #[arg(long, default_value_t = 1.0, allow_hyphen_values = true)]
        scale_y: f64,
        /// Clockwise, in degrees.
        #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
        rotate: f64,
    },
    /// Insert or remove frames and keyframes on a layer (default: the first layer).
    Frame {
        #[arg(value_enum)]
        op: project::FrameOperation,
        path: PathBuf,
        /// Zero-based frame the operation applies at (the first, for a range).
        #[arg(long)]
        at: u32,
        /// The last frame of the range, for `remove-frames` and `reverse-frames`; for
        /// `move-keyframe`, the frame the keyframe goes to.
        #[arg(long)]
        to: Option<u32>,
        #[arg(long)]
        layer: Option<uuid::Uuid>,
        /// Easing for `set-ease`, from -100 (starts slowly) to 100 (ends slowly).
        #[arg(long, allow_hyphen_values = true)]
        ease: Option<i8>,
        /// For `move-keyframe`: the layer it goes to (default: its own).
        #[arg(long)]
        onto: Option<uuid::Uuid>,
    },
    /// Rename, hide/show, lock/unlock, reorder, duplicate, delete or make transparent a layer;
    /// show all layers, or lock or hide all but one.
    Layer {
        #[arg(value_enum)]
        op: project::LayerOperation,
        path: PathBuf,
        #[arg(long)]
        layer: Option<uuid::Uuid>,
        /// New name, for `rename`.
        #[arg(long)]
        name: Option<String>,
        /// For `move`: the layer to go just above; without it, the layer goes to the bottom.
        #[arg(long)]
        before: Option<uuid::Uuid>,
        /// For `move`: the folder to go into (a folder above the new place); without it, the
        /// layer is taken out of any folder.
        #[arg(long)]
        folder: Option<uuid::Uuid>,
        /// For `opacity`: how opaque the stage draws the layer while editing, 0 to 100 (100 is
        /// opaque; exports always draw it whole).
        #[arg(long)]
        percent: Option<u8>,
    },
    /// Import an XFL directory, .xfl, .fla, .swf, Moho, BVH or Toonz file into a new project, listing what was
    /// not carried over.
    Import { source: PathBuf, out: PathBuf },
    /// Convert a project to the format OUT's ending says: .fla, .xfl (a folder with the .xfl stub
    /// inside) or .flare, as Save As does. The source can be any of them (or a .swf to read). An
    /// Animate document that was not edited is written back as the original.
    Convert {
        source: PathBuf,
        out: PathBuf,
        /// Replace OUT if it exists (the old file is kept until the new one is whole).
        #[arg(long)]
        force: bool,
    },
    /// Export a project as an XFL folder, or a zipped .fla when OUT ends in .fla.
    ExportXfl { path: PathBuf, out: PathBuf },
    /// A symbol's kind and how an instance plays it.
    Symbol {
        #[command(subcommand)]
        op: crate::symbol::SymbolCommand,
    },
    /// Skeleton rigs for symbols: bones, keyed poses, IK, evaluation.
    Rig {
        #[command(subcommand)]
        op: crate::symbol::RigCmd,
    },
    /// Publish a project as a SWF. Symbols are kept as sprites; tweens are baked into per-frame
    /// positions and shapes.
    PublishSwf {
        path: PathBuf,
        out: PathBuf,
        /// Write an uncompressed SWF.
        #[arg(long)]
        no_compress: bool,
    },
    /// Publish a project as one HTML5 Canvas page that plays it in a browser. Frames are baked
    /// (tweens and symbols are flattened). The options are Animate's Publish Settings.
    PublishHtml5 {
        path: PathBuf,
        out: PathBuf,
        /// Stop on the last frame instead of looping the timeline.
        #[arg(long)]
        no_loop: bool,
        /// Leave the stage at the top left instead of centering it.
        #[arg(long)]
        no_center: bool,
        /// Scale the stage to fit the browser window.
        #[arg(long)]
        responsive: bool,
    },
    /// Play the project in an external Ruffle (Test Movie) and wait for it to close.
    TestMovie {
        path: PathBuf,
        /// The Ruffle executable (default: $FLARE_RUFFLE, then `ruffle` on PATH).
        #[arg(long)]
        ruffle: Option<PathBuf>,
    },
    /// Open the desktop app, on the given projects if any (the app is found next to this program,
    /// on PATH, or at $FLARE_DESKTOP).
    Open { paths: Vec<PathBuf> },
    /// Evaluate one frame and print the placed instances.
    Eval {
        path: PathBuf,
        #[arg(long, default_value_t = 0)]
        frame: u32,
        #[arg(long)]
        scene: Option<uuid::Uuid>,
    },
    /// Render a frame range to numbered PNGs (default: the whole first scene).
    ExportFrames {
        path: PathBuf,
        directory: PathBuf,
        #[arg(long, default_value_t = 0)]
        start: u32,
        /// Last frame, inclusive.
        #[arg(long)]
        end: Option<u32>,
        #[arg(long)]
        width: Option<u32>,
        #[arg(long)]
        height: Option<u32>,
        #[arg(long)]
        transparent: bool,
    },
    /// Lay a frame range out as a transparent PNG sprite sheet with a JSON atlas beside it.
    ExportSprites {
        path: PathBuf,
        /// The sheet (`.png`); the atlas is written next to it as `.json`.
        out: PathBuf,
        #[arg(long, default_value_t = 0)]
        start: u32,
        /// Last frame, inclusive.
        #[arg(long)]
        end: Option<u32>,
        /// Frames per row (default: about square).
        #[arg(long)]
        columns: Option<u32>,
    },
    /// Place a symbol instance on the first layer of the first scene.
    Place {
        path: PathBuf,
        symbol: uuid::Uuid,
        #[arg(long, default_value_t = 0)]
        start: u32,
        #[arg(long, default_value_t = 1)]
        duration: u32,
        #[arg(long, default_value_t = 0.0)]
        x: f64,
        #[arg(long, default_value_t = 0.0)]
        y: f64,
        /// Tween linearly to the next adjacent span of the same symbol.
        #[arg(long)]
        tween: bool,
        /// Layer id from `add-layer` or `info` (default: the first layer).
        #[arg(long)]
        layer: Option<uuid::Uuid>,
    },
}

const fn magic_wand_default_threshold() -> u8 {
    flare_tools::magic_wand::DEFAULT_THRESHOLD
}
