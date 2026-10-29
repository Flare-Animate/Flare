//! Flare CLI: inspection, rational timing and headless project editing.
#![forbid(unsafe_code)]

mod project;

use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use flare_core::FrameRate;
use flare_flash::inspect_prefix;
use serde_json::json;

#[derive(Parser)]
#[command(
    version,
    about = "Flare starter: bounded inspection, not full format support"
)]
struct Arguments {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Identify a possible container from at most 4096 bytes, without executing it.
    Inspect { path: PathBuf },
    /// Inspect a validated rational frame rate and one zero-based frame boundary.
    Rate {
        numerator: u32,
        denominator: u32,
        #[arg(long, default_value_t = 0)]
        frame: u64,
    },
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
    },
    /// Summarize a project.
    Info { path: PathBuf },
    /// Add a Graphic symbol to the library, optionally holding one filled rectangle.
    AddSymbol {
        path: PathBuf,
        name: String,
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
        #[arg(long, default_value_t = 0)]
        start: u32,
        #[arg(long, default_value_t = 1)]
        duration: u32,
        #[arg(long)]
        layer: Option<uuid::Uuid>,
    },
    /// Render one frame of the first scene to a PNG file.
    Export {
        path: PathBuf,
        out: PathBuf,
        #[arg(long, default_value_t = 0)]
        frame: u32,
        /// Defaults to the stage width.
        #[arg(long)]
        width: Option<u32>,
        /// Defaults to the stage height.
        #[arg(long)]
        height: Option<u32>,
        /// Keep the background transparent instead of white.
        #[arg(long)]
        transparent: bool,
    },
    /// Add an empty layer below the existing ones in the first scene.
    AddLayer { path: PathBuf, name: String },
    /// Insert or remove frames and keyframes on a layer (default: the first layer).
    Frame {
        #[arg(value_enum)]
        op: project::FrameOperation,
        path: PathBuf,
        /// Zero-based frame the operation applies at.
        #[arg(long)]
        at: u32,
        #[arg(long)]
        layer: Option<uuid::Uuid>,
    },
    /// Rename, hide/show, lock/unlock, reorder or delete a layer.
    Layer {
        #[arg(value_enum)]
        op: project::LayerOperation,
        path: PathBuf,
        #[arg(long)]
        layer: Option<uuid::Uuid>,
        /// New name, for `rename`.
        #[arg(long)]
        name: Option<String>,
    },
    /// Import an XFL directory, .xfl file or modern .fla into a new project, listing what was
    /// not carried over.
    Import { source: PathBuf, out: PathBuf },
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

fn main() -> Result<()> {
    let arguments = Arguments::parse();
    let output = match arguments.command {
        Command::Inspect { path } => {
            let file =
                File::open(&path).with_context(|| format!("cannot open {}", path.display()))?;
            let metadata = file.metadata().context("cannot inspect file metadata")?;
            if !metadata.is_file() {
                bail!("inspection requires a regular file");
            }
            let mut prefix = Vec::with_capacity(4096);
            file.take(4096)
                .read_to_end(&mut prefix)
                .context("cannot read file prefix")?;
            json!({
                "path": path.to_string_lossy(),
                "file_bytes": metadata.len(),
                "inspected_bytes": prefix.len(),
                "prefix": inspect_prefix(&prefix)?,
                "validation": "prefix_only_not_full_format_validation"
            })
        }
        Command::Rate {
            numerator,
            denominator,
            frame,
        } => {
            let rate = FrameRate::new(numerator, denominator)?;
            json!({
                "rate": rate,
                "zero_based_frame": frame,
                "frame_start_nanoseconds_floor": rate.frame_start_ns(frame)?
            })
        }
        Command::New {
            path,
            numerator,
            denominator,
            width,
            height,
            background,
        } => {
            let stage =
                flare_document::Stage::new(width, height, project::parse_color(&background)?)?;
            project::new(&path, numerator, denominator, stage)?
        }
        Command::SetStage {
            path,
            width,
            height,
            background,
        } => {
            let background = background
                .as_deref()
                .map(project::parse_color)
                .transpose()?;
            project::set_stage(&path, width, height, background)?
        }
        Command::Eval { path, frame, scene } => project::eval(&path, frame, scene)?,
        Command::AddLayer { path, name } => project::add_layer(&path, &name)?,
        Command::Frame {
            op,
            path,
            at,
            layer,
        } => project::frame_op(&path, op, at, layer)?,
        Command::ExportFrames {
            path,
            directory,
            start,
            end,
            width,
            height,
            transparent,
        } => project::export_frames(&path, &directory, start, end, width, height, transparent)?,
        Command::Import { source, out } => project::import(&source, &out)?,
        Command::Layer {
            op,
            path,
            layer,
            name,
        } => project::layer_op(&path, op, layer, name.as_deref())?,
        Command::Info { path } => project::info(&path)?,
        Command::AddSymbol {
            path,
            name,
            rect,
            color,
            stroke,
            stroke_width,
        } => {
            let style = project::rect_style(&color, stroke.as_deref(), stroke_width)?;
            let rect = rect.map(|r| ([r[0], r[1], r[2], r[3]], style));
            project::add_symbol(&path, &name, rect)?
        }
        Command::DrawRect {
            path,
            rect,
            color,
            stroke,
            stroke_width,
            start,
            duration,
            layer,
        } => project::draw_rect(
            &path,
            [rect[0], rect[1], rect[2], rect[3]],
            project::rect_style(&color, stroke.as_deref(), stroke_width)?,
            start,
            duration,
            layer,
        )?,
        Command::Export {
            path,
            out,
            frame,
            width,
            height,
            transparent,
        } => project::export(&path, &out, frame, width, height, transparent)?,
        Command::Place {
            path,
            symbol,
            start,
            duration,
            x,
            y,
            tween,
            layer,
        } => project::place(
            &path,
            project::PlaceOptions {
                symbol,
                start,
                duration,
                x,
                y,
                tween,
                layer,
            },
        )?,
    };
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    serde_json::to_writer_pretty(&mut stdout, &output)?;
    writeln!(stdout)?;
    Ok(())
}
