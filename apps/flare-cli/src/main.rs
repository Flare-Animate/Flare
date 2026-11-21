//! Flare CLI: inspection, rational timing and headless project editing.
#![forbid(unsafe_code)]

mod brush;
mod commands;
mod fx;
mod game;
mod lipsync;
mod magic_wand;
mod magnet;
mod mannequin;
mod project;
mod scripts;
mod shader;
mod shortcuts;
mod stopmotion;
mod symbol;
mod tween;
mod video;

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
        Command::SoundImport { path, sound, at } => project::sound_import(&path, &sound, at)?,
        Command::SoundWaveform { path, sound } => project::sound_waveform(&path, sound.as_deref())?,
        Command::SetStage {
            path,
            width,
            height,
            background,
            fps,
        } => {
            let background = background
                .as_deref()
                .map(project::parse_color)
                .transpose()?;
            project::set_stage(&path, width, height, background, fps)?
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
        Command::Frames { path, first, count } => project::frames_list(&path, first, count)?,
        Command::Goto { path, target, from } => project::goto(&path, &target, from)?,
        Command::Bone { action } => project::bone(action)?,
        Command::Bones { path, frame } => project::bones_list(&path, frame)?,
        Command::Tween { op } => tween::run(op)?,
        Command::Game { op } => game::run(op)?,
        Command::Scripts { op } => scripts::run(op)?,
        Command::Fx { op } => fx::run(op)?,
        Command::Shortcuts { op } => shortcuts::run(op)?,
        Command::Mannequin { op } => mannequin::run(*op)?,
        Command::VideoImport(args) => video::run(args)?,
        Command::LipSync(args) => lipsync::run(args)?,
        Command::BrushImport(args) => brush::run(args)?,
        Command::Shader { op } => shader::run(op)?,
        Command::Symbol { op } => symbol::run_symbol(op)?,
        Command::Rig { op } => symbol::run_rig(op)?,
        Command::AddLayer { path, name } => project::add_layer(&path, &name)?,
        Command::TransformFrames {
            path,
            from,
            to,
            dx,
            dy,
            scale_x,
            scale_y,
            rotate,
        } => project::transform_frames(
            &path,
            from,
            to,
            flare_tools::multi_frame::RangeTransform {
                dx,
                dy,
                scale_x,
                scale_y,
                rotate_degrees: rotate,
            },
        )?,
        Command::Frame {
            op,
            path,
            at,
            to,
            dest,
            layer,
            ease,
            onto,
        } => project::frame_op(
            &path,
            op,
            project::FrameArgs {
                at,
                to,
                dest,
                layer,
                ease,
                onto,
            },
        )?,
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
            op: project::LayerOperation::Opacity,
            path,
            layer,
            percent,
            ..
        } => project::layer_opacity(&path, layer, percent)?,
        Command::Layer {
            op,
            path,
            layer,
            name,
            before,
            folder,
            ..
        } => project::layer_op(
            &path,
            op,
            layer,
            name.as_deref(),
            project::LayerPlace { before, folder },
        )?,
        Command::Info { path } => project::info(&path)?,
        Command::AddSymbol {
            path,
            name,
            kind,
            rect,
            color,
            stroke,
            stroke_width,
        } => {
            let style = project::rect_style(&color, stroke.as_deref(), stroke_width, None)?;
            let rect = rect.map(|r| ([r[0], r[1], r[2], r[3]], style));
            project::add_symbol(&path, &name, kind, rect)?
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
        Command::MagicWand(args) => magic_wand::run(args)?,
        Command::Magnet(args) => magnet::run(args)?,
        Command::Brush(args) => project::brush_strokes(&args)?,
        Command::Blur(args) => project::blend_stroke(flare_tools::blend::BlendKind::Blur, &args)?,
        Command::Smudge(args) => {
            project::blend_stroke(flare_tools::blend::BlendKind::Smudge, &args)?
        }
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
            no_google_fonts,
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
                google_fonts: !no_google_fonts,
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
            no_google_fonts,
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
                google_fonts: !no_google_fonts,
            },
        )?,
        Command::BreakText { path, shape } => project::break_text(&path, shape)?,
        Command::StopMotion(sub) => match sub {
            stopmotion::Commands::StopMotionNew {
                fps,
                hold,
                onion_before,
                onion_after,
                onion_opacity,
            } => stopmotion::new_session(fps, hold, onion_before, onion_after, onion_opacity)?,
            stopmotion::Commands::StopMotionCapture { simulated } => {
                stopmotion::capture(simulated)?
            }
            stopmotion::Commands::StopMotionTimeline { fps, hold, frames } => {
                stopmotion::timeline(fps, hold, frames)?
            }
            stopmotion::Commands::StopMotionOnion {
                before,
                after,
                opacity,
                frames,
                frame,
            } => stopmotion::onion(before, after, opacity, frames, frame)?,
        },
        Command::Fonts {
            font_files,
            no_google_fonts,
        } => project::font_families(&font_files, !no_google_fonts)?,
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
                    if width.is_some() || height.is_some() {
                        bail!(
                            "SVG is a vector picture at the stage size: --width and --height do not apply"
                        );
                    }
                    project::export_svg(
                        &path,
                        &out,
                        frame.unwrap_or(0),
                        flare_runtime::Backdrop::transparent_if(transparent),
                    )?
                }
                project::ExportFormat::Psd => {
                    if width.is_some() || height.is_some() {
                        bail!(
                            "a PSD is one picture at the stage size: --width and --height do not apply"
                        );
                    }
                    project::export_psd(
                        &path,
                        &out,
                        frame.unwrap_or(0),
                        flare_runtime::Backdrop::transparent_if(transparent),
                    )?
                }
                project::ExportFormat::Gif
                | project::ExportFormat::Apng
                | project::ExportFormat::AnimatedWebp
                | project::ExportFormat::Mov
                | project::ExportFormat::Avi
                | project::ExportFormat::Mkv
                | project::ExportFormat::Webm
                | project::ExportFormat::Mpeg
                | project::ExportFormat::Flv
                | project::ExportFormat::Mp4 => {
                    if frame.is_some() || width.is_some() || height.is_some() {
                        bail!(
                            "a GIF or a video holds every frame at the stage size: --frame, --width and --height are for PNG and the other still formats"
                        );
                    }
                    let backdrop = flare_runtime::Backdrop::transparent_if(transparent);
                    match format {
                        project::ExportFormat::Gif => project::export_gif(&path, &out, backdrop)?,
                        project::ExportFormat::Apng => project::export_apng(&path, &out, backdrop)?,
                        project::ExportFormat::AnimatedWebp => {
                            project::export_animated_webp(&path, &out, backdrop)?
                        }
                        project::ExportFormat::Mov => project::export_mov(&path, &out, backdrop)?,
                        project::ExportFormat::Avi => {
                            project::export_movie(&path, &out, "avi", backdrop)?
                        }
                        project::ExportFormat::Mkv => {
                            project::export_movie(&path, &out, "mkv", backdrop)?
                        }
                        project::ExportFormat::Webm => {
                            project::export_movie(&path, &out, "webm", backdrop)?
                        }
                        project::ExportFormat::Mpeg => {
                            project::export_movie(&path, &out, "mpeg", backdrop)?
                        }
                        project::ExportFormat::Flv => {
                            project::export_movie(&path, &out, "flv", backdrop)?
                        }
                        _ => project::export_mp4(&path, &out, backdrop)?,
                    }
                }
                _ => bail!("unreachable: this format is a still image"),
            },
        },
        Command::Place(args) => project::place(&args)?,
    };
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    serde_json::to_writer_pretty(&mut stdout, &output)?;
    writeln!(stdout)?;
    Ok(())
}
