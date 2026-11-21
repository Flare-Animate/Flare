//! `flare import-sequence`: numbered PNG files, or a folder of them, as a new project with one
//! layer and a keyframe per picture. The reading is `flare_import`'s, the same the desktop app's
//! File > Import > Image Sequence and the MCP server's `import_image_sequence` use.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::Args;
use serde_json::{Value, json};

#[derive(Args)]
pub struct SequenceArgs {
    /// PNG files, or one folder holding them; ordered by their names, `2.png` before `10.png`.
    #[arg(required = true)]
    sources: Vec<PathBuf>,
    /// The project to write; it must not exist.
    #[arg(long)]
    out: PathBuf,
}

pub fn run(args: &SequenceArgs) -> Result<Value> {
    if args.out.exists() {
        bail!("{} already exists", args.out.display());
    }
    let files =
        flare_import::read_sequence_files(&args.sources).context("cannot read the sequence")?;
    let imported = flare_import::import_image_sequence(files).context("cannot import it")?;
    flare_storage::save(&imported.document, &args.out)
        .with_context(|| format!("cannot save {}", args.out.display()))?;
    let layer = imported
        .document
        .scenes()
        .first()
        .and_then(|s| s.timeline().layers().first());
    Ok(json!({
        "imported": args.out.to_string_lossy(),
        "pictures": layer.map_or(0, |l| l.spans().len()),
        "bitmaps": imported.document.bitmaps().len(),
    }))
}
