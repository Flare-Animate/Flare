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
    },
    /// Draw a filled rectangle on the first layer of the first scene.
    DrawRect {
        path: PathBuf,
        #[arg(long, num_args = 4, value_names = ["X", "Y", "W", "H"], allow_negative_numbers = true, required = true)]
        rect: Vec<f64>,
        #[arg(long, default_value = "000000")]
        color: String,
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
        #[arg(long, default_value_t = 1920)]
        width: u32,
        #[arg(long, default_value_t = 1080)]
        height: u32,
        /// Keep the background transparent instead of white.
        #[arg(long)]
        transparent: bool,
    },
    /// Add an empty layer below the existing ones in the first scene.
    AddLayer { path: PathBuf, name: String },
    /// Evaluate one frame and print the placed instances.
    Eval {
        path: PathBuf,
        #[arg(long, default_value_t = 0)]
        frame: u32,
        #[arg(long)]
        scene: Option<uuid::Uuid>,
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
        } => project::new(&path, numerator, denominator)?,
        Command::Eval { path, frame, scene } => project::eval(&path, frame, scene)?,
        Command::AddLayer { path, name } => project::add_layer(&path, &name)?,
        Command::Info { path } => project::info(&path)?,
        Command::AddSymbol {
            path,
            name,
            rect,
            color,
        } => {
            let rect = rect
                .map(|r| {
                    Ok::<_, anyhow::Error>((
                        [r[0], r[1], r[2], r[3]],
                        project::parse_color(&color)?,
                    ))
                })
                .transpose()?;
            project::add_symbol(&path, &name, rect)?
        }
        Command::DrawRect {
            path,
            rect,
            color,
            start,
            duration,
            layer,
        } => project::draw_rect(
            &path,
            [rect[0], rect[1], rect[2], rect[3]],
            project::parse_color(&color)?,
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
