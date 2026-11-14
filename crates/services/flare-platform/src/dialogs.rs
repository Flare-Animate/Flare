//! The native file pickers and message boxes. Each blocks until the person answers, so call them
//! from the UI thread only when that is what is wanted (a modal question).
//!
//! This crate has no translator: every word a box shows (a filter's name, a title, a question) is
//! passed in by the caller, who speaks the interface's language.
//!
//! The rows come from the platform's own box (see [`native`]); on a platform there is no box for
//! (a phone, Android or iOS: the app has no file picker of its own yet), [`phone`] answers as a cancelled dialog
//! and prints a word instead of showing one, so the app compiles and opens the same way.

use std::path::PathBuf;

#[cfg(any(target_os = "android", target_os = "ios"))]
#[path = "dialogs_phone.rs"]
mod phone;
#[cfg(any(target_os = "android", target_os = "ios"))]
use phone as backend;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[path = "dialogs_native.rs"]
mod native;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use native as backend;

/// A project, or an Animate file or XFL folder's document (Animate's Open takes those too).
pub fn pick_open(filter: &str) -> Option<PathBuf> {
    // `xml` is for the DOMDocument.xml of an XFL folder that has no `.xfl` beside it; any other
    // XML file is refused when it is opened.
    backend::pick_open(filter)
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
    backend::save_document(fla, xfl, flare, name, folder)
}

pub fn pick_sound(filter: &str) -> Option<PathBuf> {
    backend::pick_sound(filter)
}

/// A picture to look at, of the kinds whose endings are `extensions`.
pub fn pick_image(filter: &str, extensions: &[&str]) -> Option<PathBuf> {
    backend::pick_image(filter, extensions)
}

/// A movie to bring in to draw over, of the kinds ffmpeg is asked to read.
pub fn pick_video(filter: &str) -> Option<PathBuf> {
    backend::pick_video(filter)
}

/// Animate's keyboard shortcut file (`Shortcuts/<set>.xml` in its settings folder).
pub fn pick_shortcut_xml(filter: &str) -> Option<PathBuf> {
    backend::pick_shortcut_xml(filter)
}

pub fn save_png(filter: &str) -> Option<PathBuf> {
    backend::save_png(filter)
}

/// Where to write the movie: the list offers an animated GIF (first, the default), an MP4 video
/// and a QuickTime movie, each named by its words in `filters`; the ending of the name that comes
/// back decides which is written (`flare_platform::jobs::write_movie`).
pub fn save_movie(filters: [&str; 3]) -> Option<PathBuf> {
    backend::save_movie(filters)
}

/// Where to write a project as a Flash document.
pub fn save_fla(filter: &str) -> Option<PathBuf> {
    backend::save_fla(filter)
}

/// Where to write the frame on show as an SVG picture.
pub fn save_svg(filter: &str) -> Option<PathBuf> {
    backend::save_svg(filter)
}

/// Asks where to publish, starting from `name` (the document's, with no ending) in `folder`. The
/// name is shared by every format written, each with its own ending.
pub fn save_publish(title: &str, name: &str, folder: Option<&std::path::Path>) -> Option<PathBuf> {
    backend::save_publish(title, name, folder)
}

pub fn message(title: &str, text: &str) {
    backend::message(title, text);
}

/// Asks whether to close and lose unsaved changes.
pub fn confirm_discard(title: &str, question: &str) -> bool {
    backend::confirm_discard(title, question)
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
    backend::startup_failed_in(title, text);
}
