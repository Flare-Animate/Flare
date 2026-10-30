//! The native file pickers and message boxes. Each blocks until the person answers, so call them
//! from the UI thread only when that is what is wanted (a modal question).

use std::path::PathBuf;

fn project() -> rfd::FileDialog {
    rfd::FileDialog::new().add_filter("Flare project", &["flare"])
}

pub fn pick_project() -> Option<PathBuf> {
    project().pick_file()
}

pub fn save_project() -> Option<PathBuf> {
    project().save_file()
}

/// An XFL file or a modern `.fla` to import.
pub fn pick_import() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("Flash document", &["fla", "xfl"])
        .pick_file()
}

pub fn pick_sound() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("Sound", &["wav", "mp3", "ogg", "flac"])
        .pick_file()
}

pub fn save_png() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("PNG image", &["png"])
        .set_file_name("frame.png")
        .save_file()
}

pub fn save_gif() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("GIF animation", &["gif"])
        .set_file_name("movie.gif")
        .save_file()
}

/// Asks where to publish. The name is shared by every format written, each with its own ending.
pub fn save_publish() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Publish")
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

/// Asks whether to close and lose unsaved changes.
pub fn confirm_discard() -> bool {
    rfd::MessageDialog::new()
        .set_title("Unsaved changes")
        .set_description("Close without saving your changes?")
        .set_buttons(rfd::MessageButtons::YesNo)
        .show()
        == rfd::MessageDialogResult::Yes
}
