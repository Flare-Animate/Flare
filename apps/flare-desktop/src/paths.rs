//! Where per-user files live, and reading and writing the remembered dock layout.

use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Flare's per-user data directory, given an environment lookup (injectable for tests).
/// Returns `None` rather than a relative path when the needed variables are missing.
pub fn app_dir_with(env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let var = |name: &str| env(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    let base = if cfg!(target_os = "macos") {
        var("HOME")?.join("Library").join("Application Support")
    } else if cfg!(target_os = "windows") {
        var("APPDATA")?
    } else {
        var("XDG_DATA_HOME").or_else(|| var("HOME").map(|h| h.join(".local").join("share")))?
    };
    Some(base.join("Flare"))
}

/// Where the dock layout is remembered between sessions.
pub fn layout_file() -> Option<PathBuf> {
    layout_file_with(|name| std::env::var_os(name))
}

pub fn layout_file_with(env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    app_dir_with(env).map(|dir| dir.join("layout.json"))
}

/// Reads a saved layout. A missing file is `None`; the read itself is capped so an
/// oversized file is never loaded whole.
pub fn read_layout(path: &Path) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let mut text = String::new();
    file.take(flare_ui::MAX_LAYOUT_BYTES as u64 + 1)
        .read_to_string(&mut text)
        .ok()?;
    Some(text)
}

/// Writes the layout through a temporary file and a rename, so a crash mid-write cannot
/// leave a truncated layout behind.
pub fn write_layout(path: &Path, json: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, json)?;
    std::fs::rename(&temp, path)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| OsString::from(*v))
        }
    }

    #[test]
    fn the_directory_follows_the_platform_convention() {
        let vars = [
            ("HOME", "/home/u"),
            ("XDG_DATA_HOME", "/data"),
            ("APPDATA", "C:\\Users\\u\\AppData\\Roaming"),
        ];
        let dir = app_dir_with(env(&vars)).expect("dir");
        assert!(dir.ends_with("Flare"));
        if cfg!(target_os = "macos") {
            assert_eq!(
                dir,
                PathBuf::from("/home/u/Library/Application Support/Flare")
            );
        } else if cfg!(target_os = "linux") {
            assert_eq!(dir, PathBuf::from("/data/Flare"));
        }
        assert!(
            layout_file_with(env(&vars))
                .expect("file")
                .ends_with("Flare/layout.json")
                || layout_file_with(env(&vars))
                    .expect("file")
                    .ends_with("Flare\\layout.json")
        );
    }

    #[test]
    fn missing_or_empty_variables_give_no_directory_rather_than_a_relative_path() {
        assert_eq!(app_dir_with(env(&[])), None);
        assert_eq!(
            app_dir_with(env(&[("HOME", ""), ("APPDATA", ""), ("XDG_DATA_HOME", "")])),
            None
        );
    }

    #[test]
    fn layouts_are_written_atomically_and_oversized_files_are_not_read_whole() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("a").join("layout.json");
        write_layout(&path, "{\"x\":1}").expect("write");
        write_layout(&path, "{\"x\":2}").expect("overwrite");
        assert_eq!(read_layout(&path).as_deref(), Some("{\"x\":2}"));
        assert_eq!(
            std::fs::read_dir(path.parent().expect("dir"))
                .expect("ls")
                .count(),
            1,
            "no temp file left"
        );
        assert_eq!(read_layout(&dir.path().join("missing.json")), None);
        let big = dir.path().join("big.json");
        std::fs::write(&big, "x".repeat(flare_ui::MAX_LAYOUT_BYTES * 2)).expect("write");
        let read = read_layout(&big).expect("read");
        assert!(
            read.len() <= flare_ui::MAX_LAYOUT_BYTES + 1,
            "the read itself is capped"
        );
    }
}
