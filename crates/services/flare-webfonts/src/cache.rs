//! Downloaded font files, kept so a family is downloaded once.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use flare_text::GoogleFamily;
use flare_text::google::commit;

/// Where downloaded fonts are kept when the front end does not say: the platform's cache
/// directory for Flare, the same one `flare_platform::dirs` names.
pub fn default_cache_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("io.github", "Flare-Animate", "Flare")
        .map(|dirs| dirs.cache_dir().join("google-fonts"))
}

/// A directory of downloaded files, under one folder per google/fonts commit. Only the
/// catalog's own commit is kept: moving to a newer catalog clears the older files.
#[derive(Debug, Clone)]
pub struct Cache {
    root: PathBuf,
}

impl Cache {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The family's files as kept, or `None` unless every one of them is there, the right size
    /// and its font.
    pub fn read(&self, family: &GoogleFamily) -> Option<Vec<Vec<u8>>> {
        let mut files = Vec::new();
        for name in family.cache_names() {
            let file = std::fs::File::open(self.root.join(name)).ok()?;
            let mut bytes = Vec::new();
            file.take(flare_text::MAX_FONT_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .ok()?;
            files.push(bytes);
        }
        flare_text::check_google_family(family, &files)
            .ok()
            .map(|()| files)
    }

    /// Keeps the family's files, checked by the caller. Each is written whole or not at all, so
    /// a crash never leaves half a font to be read next time. Failing to keep them is not an
    /// error worth reporting: the text is set all the same, and the next run downloads again.
    pub fn write(&self, family: &GoogleFamily, files: &[Vec<u8>]) -> std::io::Result<()> {
        self.forget_older_commits();
        for (name, bytes) in family.cache_names().into_iter().zip(files) {
            let path = self.root.join(name);
            let Some(dir) = path.parent() else {
                continue;
            };
            std::fs::create_dir_all(dir)?;
            let mut temp = tempfile::NamedTempFile::new_in(dir)?;
            temp.write_all(bytes)?;
            temp.persist(&path).map_err(|error| error.error)?;
        }
        Ok(())
    }

    /// Removes the folders of other commits, which only an older catalog reads.
    fn forget_older_commits(&self) {
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            let is_commit = name.len() == 40 && name.chars().all(|c| c.is_ascii_hexdigit());
            if is_commit && name != commit() {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }
}
