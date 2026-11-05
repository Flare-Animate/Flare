//! A phone has no box of its own yet: the file pickers answer as a cancelled dialog, and a word
//! that would have been shown is printed instead. Nothing an unsaved start depends on is lost;
//! the gaps themselves are recorded in docs/SURFACES.md. When the phone grows its own boxes
//! (the system's picker, its dialogs), this is the whole of what changes.

/// No file comes back, the same as a dialog the person cancelled.
pub fn pick_open(_filter: &str) -> Option<std::path::PathBuf> {
    None
}

pub fn save_document(
    _fla: &str,
    _xfl: &str,
    _flare: &str,
    _name: &str,
    _folder: Option<&std::path::Path>,
) -> Option<std::path::PathBuf> {
    None
}

pub fn pick_sound(_filter: &str) -> Option<std::path::PathBuf> {
    None
}

pub fn save_png(_filter: &str) -> Option<std::path::PathBuf> {
    None
}

pub fn save_gif(_filter: &str) -> Option<std::path::PathBuf> {
    None
}

pub fn save_fla(_filter: &str) -> Option<std::path::PathBuf> {
    None
}

pub fn save_publish(_title: &str) -> Option<std::path::PathBuf> {
    None
}

/// A word that has no box is kept in the log, where the phone's development output reads it.
pub fn message(title: &str, text: &str) {
    eprintln!("{title}: {text}");
}

pub fn startup_failed_in(title: &str, text: &str) {
    eprintln!("{title}: {text}");
}

/// Nothing is discarded without an answer: the person closes the app through the system, and
/// the recovery files (which a phone keeps, under its own data directory) are the last resort.
pub fn confirm_discard(_title: &str, _question: &str) -> bool {
    false
}
