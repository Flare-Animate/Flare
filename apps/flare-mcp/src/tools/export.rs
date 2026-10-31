//! Tools that write the project out in another form, into the export directory only. They do
//! not change the open project, so reading is enough, and none of them overwrites a file.

use std::path::PathBuf;

use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use serde_json::{Value, json};

use crate::color::{safe_file_name_with, safe_name};
use crate::params::{ExportName, ExportXfl, ImportProject, PublishHtml5, PublishSwf};
use crate::server::{FlareServer, Outcome, State};

/// The export directory, created if needed, or the reason exports are off.
fn export_dir(state: &State) -> Result<PathBuf, String> {
    let dir = state
        .permissions
        .export_dir
        .clone()
        .ok_or("exports are disabled: no export directory was configured")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// `dir/name`, refused when something is already there.
fn fresh(dir: &std::path::Path, name: &str) -> Result<PathBuf, String> {
    let out = dir.join(name);
    if out.exists() {
        return Err(format!("{name} already exists in the export directory"));
    }
    Ok(out)
}

fn diagnostics<'a, D: 'a>(
    all: impl IntoIterator<Item = &'a D>,
    describe: impl Fn(&D) -> (String, String),
) -> Vec<Value> {
    all.into_iter()
        .map(|d| {
            let (severity, message) = describe(d);
            json!({ "severity": severity, "message": message })
        })
        .collect()
}

#[tool_router(router = export_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(
        description = "Render the first scene as an endlessly looping GIF in the export directory (disabled when no export directory is configured)."
    )]
    fn export_gif(&self, Parameters(args): Parameters<ExportName>) -> Outcome {
        self.read(|state| {
            let dir = export_dir(state)?;
            let out = fresh(&dir, &safe_file_name_with(&args.name, "gif")?)?;
            let bytes = flare_runtime::gif_bytes(state.session.document())?;
            std::fs::write(&out, &bytes).map_err(|e| e.to_string())?;
            Ok(json!({ "path": out.to_string_lossy(), "bytes": bytes.len() }))
        })
    }

    #[tool(
        description = "Publish the project as a SWF in the export directory. Symbols are kept as sprites; tweens are baked into per-frame positions and shapes."
    )]
    fn publish_swf(&self, Parameters(args): Parameters<PublishSwf>) -> Outcome {
        self.read(|state| {
            let dir = export_dir(state)?;
            let out = fresh(&dir, &safe_file_name_with(&args.name, "swf")?)?;
            let options = flare_publish::PublishOptions {
                compress: !args.no_compress.unwrap_or(false),
            };
            let published =
                flare_publish::publish_to_file(state.session.document(), &options, &out)
                    .map_err(|e| e.to_string())?;
            Ok(json!({
                "path": out.to_string_lossy(),
                "frames": published.frames,
                "bytes": published.bytes.len(),
                "diagnostics": diagnostics(&published.diagnostics, |d| (
                    format!("{:?}", d.severity).to_lowercase(), d.message.clone())),
            }))
        })
    }

    #[tool(
        description = "Publish the project as one HTML5 Canvas page in the export directory. Frames are baked: tweens and symbols are flattened."
    )]
    fn publish_html5(&self, Parameters(args): Parameters<PublishHtml5>) -> Outcome {
        self.read(|state| {
            let dir = export_dir(state)?;
            let file = safe_file_name_with(&args.name, "html")?;
            let out = fresh(&dir, &file)?;
            let options = flare_publish::Html5Options {
                title: file.trim_end_matches(".html").to_owned(),
                loop_timeline: !args.no_loop.unwrap_or(false),
                center_stage: !args.no_center.unwrap_or(false),
                responsive: args.responsive.unwrap_or(false),
            };
            let published =
                flare_publish::publish_html5_to_file(state.session.document(), &options, &out)
                    .map_err(|e| e.to_string())?;
            Ok(json!({
                "path": out.to_string_lossy(),
                "frames": published.frames,
                "bytes": published.html.len(),
                "diagnostics": diagnostics(&published.diagnostics, |d| (
                    format!("{:?}", d.severity).to_lowercase(), d.message.clone())),
            }))
        })
    }

    #[tool(
        description = "Export the project as an XFL folder, or a zipped .fla when the name ends in .fla, in the export directory. Lists what the format could not hold exactly."
    )]
    fn export_xfl(&self, Parameters(args): Parameters<ExportXfl>) -> Outcome {
        self.read(|state| {
            let dir = export_dir(state)?;
            let name = safe_name(&args.name)?;
            let out = fresh(&dir, name)?;
            let output =
                flare_export::export_xfl(state.session.document()).map_err(|e| e.to_string())?;
            let zipped = name.to_ascii_lowercase().ends_with(".fla");
            if zipped {
                flare_export::write_fla(&output, &out)
            } else {
                flare_export::write_directory(&output, &out)
            }
            .map_err(|e| e.to_string())?;
            Ok(json!({
                "path": out.to_string_lossy(),
                "format": if zipped { "fla" } else { "xfl" },
                "files": output.files.len(),
                "diagnostics": diagnostics(&output.diagnostics, |d| (
                    format!("{:?}", d.severity).to_lowercase(),
                    format!("{}: {}", d.location, d.message))),
            }))
        })
    }

    #[tool(
        description = "Import an XFL folder, .xfl or .fla from the export directory into a new .flare project beside it. The open project is not changed. Lists what was not carried over."
    )]
    fn import_project(&self, Parameters(args): Parameters<ImportProject>) -> Outcome {
        self.read(|state| {
            let dir = export_dir(state)?;
            let source = dir.join(safe_name(&args.source)?);
            if !source.exists() {
                return Err(format!("{} is not in the export directory", args.source));
            }
            let out = fresh(&dir, &safe_file_name_with(&args.name, "flare")?)?;
            let imported = flare_import::import_path(&source).map_err(|e| e.to_string())?;
            flare_storage::save(&imported.document, &out).map_err(|e| e.to_string())?;
            Ok(json!({
                "path": out.to_string_lossy(),
                "scenes": imported.document.scenes().len(),
                "symbols": imported.document.symbols().len(),
                "diagnostics": diagnostics(&imported.diagnostics, |d| (
                    format!("{:?}", d.severity).to_lowercase(),
                    format!("{}: {}", d.location, d.message))),
            }))
        })
    }
}
