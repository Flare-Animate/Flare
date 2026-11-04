//! `flare-desktop`: the desktop editor. This file only decides what to open; the window is in
//! `ui/` and everything the operating system provides is in `flare-platform`.
// A release build on Windows is a program with a window, not a console one: opening it from the
// Start menu does not open a console beside it. What a failed start has to say is in a box (see
// `main`), and `--version` and `--help` only show when the output is redirected.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

use std::path::PathBuf;

use anyhow::Result;
use flare_animate::{MAX_LAYOUT_BYTES, MAX_PREFERENCES_BYTES, Workspace};
use flare_platform::{dialogs, dirs, recovery};

/// A start that fails is also shown in a box: launched from a menu there is no terminal to read.
fn main() -> Result<()> {
    // What a packager asks of a program it has built, answered before anything is opened.
    match std::env::args_os()
        .nth(1)
        .as_deref()
        .and_then(|a| a.to_str())
    {
        Some("--version" | "-V") => {
            println!("flare-desktop {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some("--help" | "-h") => {
            println!(
                "flare-desktop {}\nUsage: flare-desktop [FILE]...\nOpens Flare on the projects named (.flare, .fla, .xfl or .swf).",
                env!("CARGO_PKG_VERSION")
            );
            return Ok(());
        }
        _ => {}
    }
    let result = run();
    if let Err(error) = &result {
        // The workspace may not exist yet, so the box speaks the system's language, not the one
        // chosen in Preferences.
        let words = flare_i18n::Translator::new(flare_i18n::Language::from_system(
            &flare_platform::locale::system_tag(),
        ));
        let mut advice = words.text("dlg-startup-advice");
        // A Windows build has no console to read more in.
        if !cfg!(windows) {
            advice = format!("{advice} {}", words.text("dlg-startup-terminal"));
        }
        let text = dialogs::startup_text_in(
            &words.text("dlg-startup-intro"),
            &advice,
            &format!("{error:#}"),
        );
        dialogs::startup_failed_in("Flare", &text);
    }
    result
}

fn run() -> Result<()> {
    // Every file the system hands over: a double-click, "Open with" or the command line.
    let named: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let mut workspace = Workspace::untitled().map_err(anyhow::Error::msg)?;
    workspace.set_system_locale(&flare_platform::locale::system_tag());
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
    // The document made above is named before the language is known.
    workspace.localize_new_document();
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
    flare_desktop::ui::run(workspace)
}
