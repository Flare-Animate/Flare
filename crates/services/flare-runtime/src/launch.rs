//! Starting the desktop app from another program, for `flare open`.
//!
//! The command line and the app are separate programs; the line only finds the app and hands it
//! the files, which the app opens exactly as if they had been dropped on it.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The most files one launch passes on, so a glob that expands to a whole disk cannot overflow the
/// argument list.
pub const MAX_OPEN_FILES: usize = 64;

#[cfg(windows)]
const APP: &str = "flare-desktop.exe";
#[cfg(not(windows))]
const APP: &str = "flare-desktop";

/// Finds the desktop app: `FLARE_DESKTOP` if it names an executable file, otherwise next to the
/// running program (an unpacked release keeps them together), then the macOS app bundle in the
/// usual places, then `PATH`. The lookup is injectable so tests need no global state.
pub fn find_desktop_with(
    env: impl Fn(&str) -> Option<OsString>,
    running: Option<&Path>,
) -> Option<PathBuf> {
    find_in(env, running, Path::new("/Applications"))
}

/// `find_desktop_with`, with the folder the system-wide app bundle lives in named, so a test is not
/// at the mercy of whatever is installed on the machine it runs on.
fn find_in(
    env: impl Fn(&str) -> Option<OsString>,
    running: Option<&Path>,
    applications: &Path,
) -> Option<PathBuf> {
    if let Some(explicit) = env("FLARE_DESKTOP").filter(|v| !v.is_empty()) {
        let path = PathBuf::from(explicit);
        return is_executable_file(&path).then_some(path);
    }
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(dir) = running.and_then(Path::parent) {
        candidates.push(dir.join(APP));
    }
    if cfg!(target_os = "macos") {
        let inside = Path::new("Flare.app/Contents/MacOS").join(APP);
        candidates.push(applications.join(&inside));
        if let Some(home) = env("HOME").filter(|h| !h.is_empty()) {
            candidates.push(Path::new(&home).join("Applications").join(inside));
        }
    }
    if let Some(path_var) = env("PATH") {
        candidates.extend(
            std::env::split_paths(&path_var)
                .filter(|dir| dir.is_absolute())
                .map(|dir| dir.join(APP)),
        );
    }
    candidates.into_iter().find(|c| is_executable_file(c))
}

pub fn find_desktop() -> Option<PathBuf> {
    let running = std::env::current_exe().ok();
    find_desktop_with(|name| std::env::var_os(name), running.as_deref())
}

/// Starts `app` with `files` and does not wait for it: the app outlives this program.
pub fn open_desktop(app: &Path, files: &[PathBuf]) -> io::Result<()> {
    if files.len() > MAX_OPEN_FILES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("at most {MAX_OPEN_FILES} files can be opened at once"),
        ));
    }
    if !is_executable_file(app) {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{} is not an executable file", app.display()),
        ));
    }
    // The app is given absolute paths: it does not start in this program's directory.
    let absolute = files
        .iter()
        .map(std::path::absolute)
        .collect::<io::Result<Vec<_>>>()?;
    Command::new(app)
        .args(absolute)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(drop)
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
#[path = "launch_tests.rs"]
mod tests;
