//! Flare CLI: inspection, rational timing and headless project editing.
#![forbid(unsafe_code)]

mod commands;
mod game;
mod project;
mod shortcuts;
mod stopmotion;
mod symbol;
mod tween;

use std::fs::File;
use std::io::{Read, Write};

use anyhow::{Context, Result, bail};
use clap::Parser;
use commands::Command;
use flare_core::FrameRate;
use flare_flash::inspect_prefix;
use serde_json::json;

#[derive(Parser)]
#[command(
    name = "flare",
    version,
    about = "Flare starter: bounded inspection, not full format support"
)]
struct Arguments {
    #[command(subcommand)]
    command: Command,
}

/// Windows gives the main thread 1 MiB (Linux 8 MiB); the command tree and the
/// export paths overflow it, so the work runs on a thread with a known stack.
const STACK_BYTES: usize = 16 * 1024 * 1024;

fn main() -> Result<()> {
    std::thread::Builder::new()
        .name("flare-cli".into())
        .stack_size(STACK_BYTES)
        .spawn(run)
        .context("cannot start the worker thread")?
        .join()
        .map_err(|_| anyhow::anyhow!("the command panicked"))?
}

fn run() -> Result<()> {
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
        Command::RenameScene { path, name } => project::rename_scene(&path, &name)?,
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
        Command::Camera {
            path,
            frame,
            x,
            y,
            zoom,
            rotation,
            motion,
            ease,
        } => project::camera(&path, frame, (x, y), zoom, rotation, motion, ease)?,
        Command::Bone { action } => project::bone(action)?,
        Command::Bones { path, frame } => project::bones_list(&path, frame)?,
        Command::Tween { op } => tween::run(op)?,
        Command::Game { op } => game::run(op)?,
        Command::Shortcuts { op } => shortcuts::run(op)?,
        Command::Symbol { op } => symbol::run_symbol(op)?,
        Command::Rig { op } => symbol::run_rig(op)?,
        Command::AddLayer { path, name } => project::add_layer(&path, &name)?,
        Command::Frame {
            op,
            path,
            at,
            to,
            layer,
            ease,
        } => project::frame_op(&path, op, at, to, layer, ease)?,
        Command::ExportFrames {
            path,
            directory,
            start,
            end,
            width,
            height,
            transparent,
        } => project::export_frames(&path, &directory, start, end, width, height, transparent)?,
        Command::ExportSprites {
            path,
            out,
            start,
            end,
            columns,
        } => project::export_sprites(&path, &out, start, end, columns)?,
        Command::Import { source, out } => project::import(&source, &out)?,
        Command::Convert { source, out, force } => project::convert(&source, &out, force)?,
        Command::ExportXfl { path, out } => project::export_xfl(&path, &out)?,
        Command::PublishSwf {
            path,
            out,
            no_compress,
        } => project::publish_swf(&path, &out, !no_compress)?,
        Command::PublishHtml5 {
            path,
            out,
            no_loop,
            no_center,
            responsive,
        } => project::publish_html5(&path, &out, !no_loop, !no_center, responsive)?,
        Command::TestMovie { path, ruffle } => project::test_movie(&path, ruffle)?,
        Command::Open { paths } => project::open_app(&paths)?,
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
            let style = project::rect_style(&color, stroke.as_deref(), stroke_width, None)?;
            let rect = rect.map(|r| ([r[0], r[1], r[2], r[3]], style));
            project::add_symbol(&path, &name, rect)?
        }
        Command::DrawRect {
            path,
            rect,
            color,
            stroke,
            stroke_width,
            stroke_style,
            start,
            duration,
            layer,
        } => project::draw_rect(
            &path,
            [rect[0], rect[1], rect[2], rect[3]],
            project::rect_style(
                &color,
                stroke.as_deref(),
                stroke_width,
                stroke_style.as_deref(),
            )?,
            start,
            duration,
            layer,
        )?,
        Command::AddText {
            path,
            text,
            x,
            y,
            family,
            size,
            bold,
            italic,
            color,
            start,
            duration,
            layer,
            font_files,
        } => project::add_text(
            &path,
            project::TextOptions {
                text,
                x,
                y,
                family,
                size,
                bold,
                italic,
                color,
                start,
                duration,
                layer,
                font_files,
            },
        )?,
        Command::SetText {
            path,
            shape,
            text,
            x,
            y,
            family,
            size,
            bold,
            italic,
            color,
            font_files,
        } => project::set_text(
            &path,
            shape,
            project::TextChanges {
                text,
                x,
                y,
                family,
                size,
                bold,
                italic,
                color,
                font_files,
            },
        )?,
        Command::BreakText { path, shape } => project::break_text(&path, shape)?,
        Command::StopMotionNew {
            fps,
            hold,
            onion_before,
            onion_after,
            onion_opacity,
        } => stopmotion::new_session(fps, hold, onion_before, onion_after, onion_opacity)?,
        Command::StopMotionCapture => stopmotion::capture()?,
        Command::Fonts { font_files } => project::font_families(&font_files)?,
        Command::Export {
            path,
            out,
            format,
            frame,
            width,
            height,
            transparent,
            quality,
        } => match format.still(quality)? {
            Some(still) => project::export(
                &path,
                &out,
                frame.unwrap_or(0),
                width,
                height,
                transparent,
                still,
            )?,
            None => match format {
                project::ExportFormat::Svg => {
                    if width.is_some() || height.is_some() || transparent {
                        bail!(
                            "SVG is a vector picture at the stage size: --width, --height and --transparent do not apply"
                        );
                    }
                    project::export_svg(&path, &out, frame.unwrap_or(0))?
                }
                project::ExportFormat::Psd => {
                    if width.is_some() || height.is_some() || transparent {
                        bail!(
                            "a PSD is one picture at the stage size: --width, --height and --transparent do not apply"
                        );
                    }
                    project::export_psd(&path, &out, frame.unwrap_or(0))?
                }
                project::ExportFormat::Gif
                | project::ExportFormat::Apng
                | project::ExportFormat::Mp4 => {
                    if frame.is_some() || width.is_some() || height.is_some() || transparent {
                        bail!(
                            "a GIF or a video holds every frame at the stage size: --frame, --width, --height and --transparent are for PNG and the other still formats"
                        );
                    }
                    match format {
                        project::ExportFormat::Gif => project::export_gif(&path, &out)?,
                        project::ExportFormat::Apng => project::export_apng(&path, &out)?,
                        _ => project::export_mp4(&path, &out)?,
                    }
                }
                _ => bail!("unreachable: this format is a still image"),
            },
        },
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
