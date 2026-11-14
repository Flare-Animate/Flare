//! `flare brush-import`: another art tool's pen profiles read into Flare's brush presets.
//! What a `.abr`, `.kpp` or `.bundle` maps onto is `flare_import::brushes`'s, the same the
//! desktop app and the MCP server use; this reads the file and prints.

use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use clap::Args;
use serde_json::{Value, json};

/// What `flare brush-import` is asked.
#[derive(Args)]
pub struct BrushImportArgs {
    /// A Photoshop `.abr`, a Krita `.kpp` or a Krita `.bundle`. The ending decides the reader.
    profile: PathBuf,
    /// Also write the presets as Flare's portable brush-profile file (`BrushProfileFile`
    /// JSON), which can be kept and shared; without it the presets are only printed.
    #[arg(long)]
    out: Option<PathBuf>,
}

pub fn run(args: BrushImportArgs) -> Result<Value> {
    let bytes = std::fs::read(&args.profile)
        .with_context(|| format!("cannot read {}", args.profile.display()))?;
    if bytes.len() as u64 > flare_import::brushes::MAX_OWNED_BYTES {
        return Err(anyhow!(
            "{} is larger than {} bytes, more than a pen-profile file holds",
            args.profile.display(),
            flare_import::brushes::MAX_OWNED_BYTES
        ));
    }
    let set = flare_import::brushes::import_profile_bytes(&args.profile, &bytes)
        .map_err(|e| anyhow!(e))?;
    if let Some(out) = &args.out {
        let file = flare_tools::brush::BrushProfileFile::encode(&set.presets)
            .map_err(|e| anyhow!("{e}"))?;
        std::fs::write(out, file).with_context(|| format!("cannot write {}", out.display()))?;
    }
    let presets: Vec<Value> = set
        .presets
        .iter()
        .map(serde_json::to_value)
        .collect::<std::result::Result<_, _>>()
        .context("a preset cannot be written as JSON")?;
    Ok(json!({
        "presets": presets,
        "notes": set.notes,
        "file": if let Some(out) = &args.out {
            Value::String(out.to_string_lossy().into_owned())
        } else {
            Value::Null
        },
    }))
}
