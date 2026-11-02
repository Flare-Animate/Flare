//! The native file pickers and message boxes. Each blocks until the person answers, so call them
//! from the UI thread only when that is what is wanted (a modal question).
//!
//! This crate has no translator: every word a box shows (a filter's name, a title, a question) is
//! passed in by the caller, who speaks the interface's language.

use std::path::PathBuf;

fn project(filter: &str) -> rfd::FileDialog {
    rfd::FileDialog::new().add_filter(filter, &["flare"])
}

pub fn pick_project(filter: &str) -> Option<PathBuf> {
    project(filter).pick_file()
}

pub fn save_project(filter: &str) -> Option<PathBuf> {
    project(filter).save_file()
}

/// An XFL file, a modern `.fla` or a Flash movie to import.
pub fn pick_import(filter: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter(filter, &["fla", "xfl", "swf"])
        .pick_file()
}

pub fn pick_sound(filter: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter(filter, &["wav", "mp3", "ogg", "flac"])
        .pick_file()
}

pub fn save_png(filter: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter(filter, &["png"])
        .set_file_name("frame.png")
        .save_file()
}

pub fn save_gif(filter: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter(filter, &["gif"])
        .set_file_name("movie.gif")
        .save_file()
}

/// Where to write a project as a Flash document.
pub fn save_fla(filter: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter(filter, &["fla"])
        .set_file_name("project.fla")
        .save_file()
}

/// Asks where to publish. The name is shared by every format written, each with its own ending.
pub fn save_publish(title: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title(title)
        .set_file_name("movie")
        .save_file()
}

pub fn message(title: &str, text: &str) {
    rfd::MessageDialog::new()
        .set_title(title)
        .set_description(text)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

/// What the box says when the program could not start, in English. See [`startup_text_in`].
pub fn startup_text(detail: &str) -> String {
    startup_text_in(
        "Flare could not start.",
        "If this is about the graphics device or surface, update your graphics drivers (Vulkan or \
         OpenGL). Running flare-desktop from a terminal prints more.",
        detail,
    )
}

/// What the box says when the program could not start. From a launcher there is no terminal, so
/// without it a failure looks like nothing happening. `detail` is cut short: it is whatever the
/// system said, and the box is not for pages of it.
pub fn startup_text_in(intro: &str, advice: &str, detail: &str) -> String {
    const LONGEST: usize = 1500;
    let detail: String = detail.chars().take(LONGEST).collect();
    format!("{intro}\n\n{detail}\n\n{advice}")
}

/// Tells the person the program could not start.
pub fn startup_failed(detail: &str) {
    startup_failed_in("Flare", &startup_text(detail));
}

/// Tells the person the program could not start, with the words already chosen.
pub fn startup_failed_in(title: &str, text: &str) {
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title(title)
        .set_description(text)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

/// Asks whether to close and lose unsaved changes.
pub fn confirm_discard(title: &str, question: &str) -> bool {
    rfd::MessageDialog::new()
        .set_title(title)
        .set_description(question)
        .set_buttons(rfd::MessageButtons::YesNo)
        .show()
        == rfd::MessageDialogResult::Yes
}
