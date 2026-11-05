//! The start of the app before the shell opens the window: a workspace brought back to what
//! the last run saved, so the dock layout, the preferences and the language survive a reboot
//! the way they do on the desktop. Platform-independent, so its own tests drive it anywhere;
//! reading the files themselves is the activity's (`activity`).

use anyhow::Result;
use flare_animate::Workspace;

/// A new workspace, restored to the layout and preferences the app was given. A saved file
/// that cannot be restored is ignored with a line in the log and the start goes on, as on the
/// desktop: a piece of saved state must never stop a start.
///
/// `Err` is what the shell cannot show, a start with no document at all (`Workspace::untitled`
/// refused to make one); the app quits instead of showing an empty window.
pub fn start(layout: Option<String>, preferences: Option<String>) -> Result<Workspace> {
    let mut workspace =
        Workspace::untitled().map_err(|error| anyhow::Error::msg(error.to_string()))?;
    if let Some(json) = layout
        && let Err(error) = workspace.restore_layout(&json)
    {
        // A damaged or outdated layout just means starting from the default.
        eprintln!("ignoring the saved layout: {error}");
    }
    if let Some(json) = preferences
        && let Err(error) = workspace.restore_preferences(&json)
    {
        eprintln!("ignoring the saved preferences: {error}");
    }
    // The document made above is named before the language is known.
    workspace.localize_new_document();
    // A start with no work in it opens at Home, with the recent projects of past runs to hand.
    if !workspace.is_dirty() {
        workspace.set_home(true);
    }
    Ok(workspace)
}

#[cfg(test)]
#[path = "open_tests.rs"]
mod open_tests;
