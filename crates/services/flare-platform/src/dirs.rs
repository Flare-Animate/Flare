//! Where per-user files live, and reading and writing the remembered dock layout.
//!
//! The places themselves are the `directories` crate's: each platform's own convention, found
//! the way that platform finds them, rather than guessed from environment variables. An
//! exception is an app the platform has no home for (an Android app is handed a data directory
//! by the system, found nowhere): the app says where its files are with [`set_app_root`] before
//! anything asks, and then everything is under it.

use directories::ProjectDirs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The per-user root an app named, when the platform's own convention has nothing to say.
static APP_ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Names where the app's per-user files are, in place of the platform's convention. Once,
/// before any file is asked for and in one process: an Android app the system does not hand a
/// home to names the data directory the system gave it. Leaving it unset keeps every platform
/// where it already is.
pub fn set_app_root(root: PathBuf) {
    let _ = APP_ROOT.set(root);
}

/// Where settings are kept: the dock layout and the preferences.
fn settings_dir() -> Option<PathBuf> {
    if let Some(root) = APP_ROOT.get() {
        return Some(root.join("config"));
    }
    project().map(|dirs| dirs.config_dir().to_owned())
}

/// Where data is kept: unsaved work written in case of a crash, and anything else the app
/// keeps between sessions.
fn application_dir() -> Option<PathBuf> {
    if let Some(root) = APP_ROOT.get() {
        return Some(root.join("data"));
    }
    project().map(|dirs| dirs.data_local_dir().to_owned())
}

fn project() -> Option<ProjectDirs> {
    ProjectDirs::from("io.github", "Flare-Animate", "Flare")
}

/// Where settings are kept: the dock layout and the preferences. `None` when the platform
/// cannot say where the person's home is, rather than a relative path.
pub fn config_dir() -> Option<PathBuf> {
    settings_dir()
}

/// The per-user recovery directory: unsaved work written in case of a crash.
pub fn recovery_dir() -> Option<PathBuf> {
    application_dir().map(|dir| dir.join("recovery"))
}

/// Where the dock layout is remembered between sessions.
pub fn layout_file() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("layout.json"))
}

/// Where the preferences are kept between sessions.
pub fn preferences_file() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("preferences.json"))
}

/// Reads a saved layout of at most `max_bytes`. A missing file is `None`; the read itself is
/// capped so an oversized file is never loaded whole.
pub fn read_layout(path: &Path, max_bytes: usize) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let mut text = String::new();
    file.take(max_bytes as u64 + 1)
        .read_to_string(&mut text)
        .ok()?;
    Some(text)
}

/// Writes the layout through a temporary file and a rename, so a crash mid-write cannot
/// leave a truncated layout behind.
pub fn write_layout(path: &Path, json: &str) -> std::io::Result<()> {
    write_file(path, json)
}

/// Writes any small settings file the same way. The staging file is the library's, with a name
/// of its own, so two windows saving at once cannot trample each other's.
pub fn write_file(path: &Path, text: &str) -> std::io::Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    staged.write_all(text.as_bytes())?;
    staged.persist(path).map(|_| ()).map_err(|e| e.error)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // The app root is a process-wide setting: these tests take this lock and so never see a
    // half-set world from each other.
    static GUARD: Mutex<()> = Mutex::new(());

    #[test]
    fn settings_and_recovery_files_have_places_of_their_own_under_flare() {
        let _guard = GUARD.lock().expect("guard");
        // The platform may have no home (a bare CI container); then there is no place, not a
        // relative one.
        let (Some(layout), Some(prefs), Some(recovery)) =
            (layout_file(), preferences_file(), recovery_dir())
        else {
            return;
        };
        for path in [&layout, &prefs, &recovery] {
            assert!(path.is_absolute(), "{path:?}");
        }
        assert_eq!(layout.parent(), prefs.parent(), "settings sit together");
        assert!(layout.ends_with("layout.json") && prefs.ends_with("preferences.json"));
        assert!(recovery.ends_with("recovery"));
        let text = layout.to_string_lossy().to_lowercase();
        assert!(text.contains("flare"), "{text}");
    }

    #[test]
    fn an_app_that_names_where_its_files_are_gets_all_of_them_there() {
        let _guard = GUARD.lock().expect("guard");
        let dir = tempfile::tempdir().expect("dir");
        set_app_root(dir.path().to_owned());
        let layout = layout_file().expect("layout");
        let prefs = preferences_file().expect("preferences");
        let recovery = recovery_dir().expect("recovery");
        assert!(layout.starts_with(dir.path()), "{layout:?}");
        assert!(prefs.starts_with(dir.path()), "{prefs:?}");
        assert!(recovery.starts_with(dir.path()), "{recovery:?}");
        assert!(layout.ends_with("layout.json"));
        assert!(prefs.ends_with("preferences.json"));
        assert!(recovery.ends_with("recovery"));
    }

    #[test]
    fn small_files_are_written_whole_and_replace_the_old_one_leaving_nothing_behind() {
        let dir = tempfile::tempdir().expect("dir");
        let file = dir.path().join("x").join("preferences.json");
        write_file(&file, "{}").expect("write");
        assert_eq!(std::fs::read_to_string(&file).expect("read"), "{}");
        write_file(&file, "{\"a\":1}").expect("overwrite");
        assert_eq!(std::fs::read_to_string(&file).expect("read"), "{\"a\":1}");
        assert_eq!(
            std::fs::read_dir(file.parent().expect("dir"))
                .expect("ls")
                .count(),
            1,
            "no staging file left"
        );
    }

    #[test]
    fn layouts_are_written_atomically_and_oversized_files_are_not_read_whole() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("a").join("layout.json");
        write_layout(&path, "{\"x\":1}").expect("write");
        write_layout(&path, "{\"x\":2}").expect("overwrite");
        assert_eq!(read_layout(&path, 1024).as_deref(), Some("{\"x\":2}"));
        assert_eq!(read_layout(&dir.path().join("missing.json"), 1024), None);
        let big = dir.path().join("big.json");
        std::fs::write(&big, "x".repeat(2048)).expect("write");
        let read = read_layout(&big, 1024).expect("read");
        assert!(read.len() <= 1024 + 1, "the read itself is capped");
    }
}
