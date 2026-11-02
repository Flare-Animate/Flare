//! The native file pickers and message boxes. Each blocks until the person answers, so call them
//! from the UI thread only when that is what is wanted (a modal question).
//!
//! This crate has no translator: every word a box shows (a filter's name, a title, a question) is
//! passed in by the caller, who speaks the interface's language.

use std::path::PathBuf;

/// A project, or an Animate file or XFL folder's document (Animate's Open takes those too).
pub fn pick_open(filter: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        // `xml` is for the DOMDocument.xml of an XFL folder that has no `.xfl` beside it; any other
        // XML file is refused when it is opened.
        .add_filter(filter, &["flare", "fla", "xfl", "swf", "xml"])
        .pick_file()
}

/// Where to save the document, in `name` (with the default ending) and `folder` to start with.
/// The list offers Flash document (`.fla`, first and so the default, as in Animate), the
/// uncompressed `.xfl` and Flare's own `.flare`; the ending of the name that comes back decides
/// the format, and the caller gives one with none the default's (`flare_runtime::resolve_save_path`).
pub fn save_document(
    fla: &str,
    xfl: &str,
    flare: &str,
    name: &str,
    folder: Option<&std::path::Path>,
) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new()
        .add_filter(fla, &["fla"])
        .add_filter(xfl, &["xfl"])
        .add_filter(flare, &["flare"])
        .set_file_name(name);
    if let Some(folder) = folder.filter(|f| f.is_dir()) {
        dialog = dialog.set_directory(folder);
    }
    dialog.save_file()
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
