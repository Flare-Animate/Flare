//! `flare-desktop`: the desktop editor. This file only decides what to open; the window is in
//! `ui/` and everything the operating system provides is in `flare-platform`.
#![forbid(unsafe_code)]

mod ui;

use std::path::PathBuf;

use anyhow::Result;
use flare_animate::{MAX_LAYOUT_BYTES, MAX_PREFERENCES_BYTES, Workspace};
use flare_platform::{dirs, recovery};

fn main() -> Result<()> {
    // Every file the system hands over: a double-click, "Open with" or the command line.
    let named: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let mut workspace = Workspace::untitled().map_err(anyhow::Error::msg)?;
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
        if named.is_empty()
            && let Some(newest) = flare_storage::list_recoveries(&dir).first()
            && recovery::offer(newest) == recovery::Choice::Recover
        {
            workspace.recover(&newest.path);
        }
    }
    // A plain launch starts at Home, with the recent projects to hand; the work recovered above
    // or the project named on the command line is shown at once.
    if named.is_empty() && !workspace.is_dirty() {
        workspace.set_home(true);
    }
    for path in &named {
        workspace.open_any(path);
    }
    ui::run(workspace)
}
