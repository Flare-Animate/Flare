//! Portable shader graph assets for headless projects.

use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use flare_commands::Command;
use flare_core::Rgba8;
use flare_document::ShaderAsset;
use flare_render::{StillFormat, apply_shader_stack};
use serde_json::{Value, json};

use crate::project;

const MAX_SHADER_FILE_BYTES: u64 = flare_import::MAX_SHADER_GRAPH_FILE_BYTES as u64;

#[derive(Subcommand)]
pub(crate) enum ShaderCommand {
    /// List named shader graphs stored in a project.
    List { path: PathBuf },
    /// Import a bounded Flare graph JSON file as a named asset.
    Import {
        path: PathBuf,
        name: String,
        graph: PathBuf,
    },
    /// Export a named graph in Flare's versioned JSON interchange form.
    Export {
        path: PathBuf,
        name: String,
        out: PathBuf,
    },
    /// Render a project frame with an ordered stack of its named shader graphs to a PNG.
    Render {
        path: PathBuf,
        out: PathBuf,
        #[arg(long = "shader", required = true)]
        shaders: Vec<String>,
        #[arg(long, default_value_t = 0)]
        frame: u32,
        #[arg(long)]
        width: Option<u32>,
        #[arg(long)]
        height: Option<u32>,
        #[arg(long)]
        transparent: bool,
    },
}

pub(crate) fn run(command: ShaderCommand) -> Result<Value> {
    match command {
        ShaderCommand::List { path } => {
            let session = project::open_session(&path)?;
            Ok(json!({
                "shader_graphs": session.document().shader_assets().iter().map(|asset| {
                    json!({"name": asset.name(), "nodes": asset.graph().nodes().len()})
                }).collect::<Vec<_>>()
            }))
        }
        ShaderCommand::Import { path, name, graph } => import(&path, &name, &graph),
        ShaderCommand::Export { path, name, out } => export(&path, &name, &out),
        ShaderCommand::Render {
            path,
            out,
            shaders,
            frame,
            width,
            height,
            transparent,
        } => render(&path, &out, &shaders, frame, width, height, transparent),
    }
}

fn export(path: &std::path::Path, name: &str, out: &std::path::Path) -> Result<Value> {
    let session = project::open_session(path)?;
    let asset = session
        .document()
        .shader_assets()
        .iter()
        .find(|asset| asset.name() == name)
        .with_context(|| format!("shader graph {name:?} is not in this project"))?;
    let bytes = flare_import::export_shader_graph(asset.graph())?;
    let directory = out
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    let mut staged = tempfile::NamedTempFile::new_in(directory)?;
    std::io::Write::write_all(&mut staged, &bytes)?;
    staged.persist(out).map_err(|error| error.error)?;
    Ok(json!({"exported": out.to_string_lossy(), "name": name}))
}

fn render(
    path: &std::path::Path,
    out: &std::path::Path,
    shader_names: &[String],
    frame: u32,
    width: Option<u32>,
    height: Option<u32>,
    transparent: bool,
) -> Result<Value> {
    if shader_names.len() > flare_render::MAX_SHADER_STACK {
        bail!(
            "shader stacks are limited to {} graphs",
            flare_render::MAX_SHADER_STACK
        );
    }
    let session = project::open_session(path)?;
    let document = session.document();
    let graphs = shader_names
        .iter()
        .map(|name| {
            document
                .shader_assets()
                .iter()
                .find(|asset| asset.name() == name)
                .map(|asset| asset.graph())
                .with_context(|| format!("shader graph {name:?} is not in this project"))
        })
        .collect::<Result<Vec<_>>>()?;
    let scene = document
        .scenes()
        .first()
        .context("document has no scene")?
        .id();
    let evaluated = flare_eval::evaluate_frame(document, scene, frame)?;
    let stage = document.stage();
    let render_width = width.unwrap_or(stage.width());
    let render_height = height.unwrap_or(stage.height());
    let background = if transparent {
        Rgba8::TRANSPARENT
    } else {
        stage.background()
    };
    let base = flare_render::render(&evaluated, render_width, render_height, background)?;
    let time = document.frame_rate().frame_start_ns(u64::from(frame))? as f32 / 1_000_000_000.0;
    let image = apply_shader_stack(&graphs, &base, time)?;

    let directory = out
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    let mut staged = tempfile::NamedTempFile::new_in(directory)?;
    image.write_still(std::io::BufWriter::new(&mut staged), StillFormat::Png)?;
    staged.persist(out).map_err(|error| error.error)?;
    Ok(json!({
        "rendered": out.to_string_lossy(),
        "width": render_width,
        "height": render_height,
        "frame": frame,
        "shaders": shader_names,
    }))
}

fn import(path: &std::path::Path, name: &str, graph_path: &std::path::Path) -> Result<Value> {
    let file = File::open(graph_path)
        .with_context(|| format!("cannot open graph {}", graph_path.display()))?;
    let metadata = file.metadata().context("cannot inspect graph file")?;
    if !metadata.is_file() || metadata.len() > MAX_SHADER_FILE_BYTES {
        bail!(
            "shader graph file must be a regular file no larger than {MAX_SHADER_FILE_BYTES} bytes"
        );
    }
    let mut bytes = Vec::new();
    file.take(MAX_SHADER_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .context("cannot read graph file")?;
    if bytes.len() as u64 > MAX_SHADER_FILE_BYTES {
        bail!("shader graph file exceeds {MAX_SHADER_FILE_BYTES} bytes");
    }
    let graph = flare_import::import_shader_graph(&bytes).context("invalid shader graph")?;
    let asset = ShaderAsset::new(name, graph)?;
    let mut session = project::open_session(path)?;
    let mut assets = session.document().shader_assets().to_vec();
    assets.retain(|existing| existing.name() != asset.name());
    assets.push(asset);
    session.apply([Command::SetShaderAssets(assets)])?;
    project::commit_session(&session, path)?;
    Ok(json!({"imported": name.trim()}))
}

#[cfg(test)]
#[path = "shader_tests.rs"]
mod tests;
