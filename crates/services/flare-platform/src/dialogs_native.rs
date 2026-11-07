//! The file pickers and message boxes, from the platform they run on (see the parent module).

/// A project, or an Animate file or XFL folder's document (Animate's Open takes those too).
pub fn pick_open(filter: &str) -> Option<std::path::PathBuf> {
    rfd::FileDialog::new()
        // `xml` is for the DOMDocument.xml of an XFL folder that has no `.xfl` beside it; any other
        // XML file is refused when it is opened.
        .add_filter(
            filter,
            &[
                "flare",
                "fla",
                "xfl",
                "swf",
                "xml",
                "moho",
                "anime",
                "mohoproj",
                "animeproj",
                "bvh",
                "svg",
                "jpg",
                "jpeg",
                "bmp",
                "tga",
                "tnz",
                "tpl",
                "plt",
                "pli",
                "tlv",
                "fc",
                "wick",
            ],
        )
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
) -> Option<std::path::PathBuf> {
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

pub fn pick_sound(filter: &str) -> Option<std::path::PathBuf> {
    rfd::FileDialog::new()
        .add_filter(filter, &["wav", "mp3", "ogg", "flac"])
        .pick_file()
}

pub fn save_png(filter: &str) -> Option<std::path::PathBuf> {
    rfd::FileDialog::new()
        .add_filter(filter, &["png"])
        .set_file_name("frame.png")
        .save_file()
}

pub fn save_gif(filter: &str) -> Option<std::path::PathBuf> {
    rfd::FileDialog::new()
        .add_filter(filter, &["gif"])
        .set_file_name("movie.gif")
        .save_file()
}

/// Where to write a project as a Flash document.
pub fn save_fla(filter: &str) -> Option<std::path::PathBuf> {
    rfd::FileDialog::new()
        .add_filter(filter, &["fla"])
        .set_file_name("project.fla")
        .save_file()
}

/// Asks where to publish. The name is shared by every format written, each with its own ending.
pub fn save_publish(title: &str) -> Option<std::path::PathBuf> {
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
