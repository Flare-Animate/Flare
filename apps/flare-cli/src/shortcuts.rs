//! `flare shortcuts`: the keyboard shortcut sets from the command line. The rules are in
//! `flare_menubar::ShortcutMap`; this reads and writes files and prints.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use clap::Subcommand;
use flare_menubar::{MAX_SHORTCUT_FILE_BYTES, ShortcutMap};
use serde_json::{Value, json};

#[derive(Subcommand)]
pub enum ShortcutsCommand {
    /// List the shortcuts of a set, or of a shortcut file's result.
    List {
        /// A set: flare, opentoonz, animate or tahoma.
        #[arg(long, default_value = "flare", conflicts_with = "file")]
        set: String,
        /// A shortcut file made by `export`.
        #[arg(long)]
        file: Option<PathBuf>,
    },
    /// Write a set (or what a file makes of it) as a shortcut file.
    Export {
        #[arg(long, default_value = "flare", conflicts_with = "file")]
        set: String,
        #[arg(long)]
        file: Option<PathBuf>,
        /// Where to write it; without it the file is printed.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Check a shortcut file and write it back in its canonical form. A file with an unknown
    /// command, a bad key or two commands on one key is refused and nothing is written.
    Import {
        file: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

fn read_file(path: &Path) -> Result<ShortcutMap> {
    let meta =
        std::fs::metadata(path).with_context(|| format!("cannot read {}", path.display()))?;
    if meta.len() > MAX_SHORTCUT_FILE_BYTES as u64 {
        return Err(anyhow!(
            "{} is too large for a shortcut file",
            path.display()
        ));
    }
    let text =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    ShortcutMap::from_json(&text).map_err(|e| anyhow!(e))
}

fn pick(set: &str, file: Option<&Path>) -> Result<ShortcutMap> {
    match file {
        Some(path) => read_file(path),
        None => ShortcutMap::from_set(set).map_err(|e| anyhow!(e)),
    }
}

pub fn run(op: ShortcutsCommand) -> Result<Value> {
    match op {
        ShortcutsCommand::List { set, file } => Ok(pick(&set, file.as_deref())?.list_json()),
        ShortcutsCommand::Export { set, file, out } => {
            let map = pick(&set, file.as_deref())?;
            write_or_print(&map, out.as_deref())
        }
        ShortcutsCommand::Import { file, out } => {
            let map = read_file(&file)?;
            write_or_print(&map, out.as_deref())
        }
    }
}

fn write_or_print(map: &ShortcutMap, out: Option<&Path>) -> Result<Value> {
    match out {
        Some(path) => {
            std::fs::write(path, map.to_json())
                .with_context(|| format!("cannot write {}", path.display()))?;
            Ok(json!({ "written": path.display().to_string(), "set": map.set() }))
        }
        None => Ok(map.file_value()),
    }
}
