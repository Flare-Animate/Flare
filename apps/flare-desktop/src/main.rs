//! `flare-desktop`: the desktop editor. This file only decides what to open; the window is in
//! `ui/` and everything the operating system provides is in `flare-platform`.
#![forbid(unsafe_code)]

mod ui;

use std::path::PathBuf;

use anyhow::{Context, Result};
use flare_animate::{MAX_LAYOUT_BYTES, MAX_PREFERENCES_BYTES, Workspace};
use flare_platform::{dirs, recovery};

fn main() -> Result<()> {
    let named = std::env::args_os().nth(1).map(PathBuf::from);
    let mut workspace = Workspace::untitled().map_err(anyhow::Error::msg)?;
    if let Some(path) = &named {
        let document =
            flare_storage::load(path).with_context(|| format!("cannot open {}", path.display()))?;
        workspace = Workspace::new(document, Some(path.clone()));
    }
    if let Some(path) = dirs::layout_file()
        && let Some(json) = dirs::read_layout(&path, MAX_LAYOUT_BYTES)
        && let Err(error) = workspace.restore_layout(&json)
    {
        // A damaged or outdated layout just means starting from the default.
        eprintln!("ignoring the saved layout: {error}");
    }
    if let Some(path) = dirs::preferences_file()
        && let Some(json) = dirs::read_layout(&path, MAX_PREFERENCES_BYTES)
        && let Err(error) = workspace.restore_preferences(&json)
    {
        eprintln!("ignoring the saved preferences: {error}");
    }
    if let Some(dir) = dirs::recovery_dir() {
        workspace.enable_recovery(dir.clone());
        // Only offer recovery on a plain launch, not when a project was named explicitly.
        if named.is_none()
            && let Some(newest) = flare_storage::list_recoveries(&dir).first()
            && recovery::offer(newest) == recovery::Choice::Recover
        {
            workspace.recover(&newest.path);
        }
    }
    ui::run(workspace)
}
