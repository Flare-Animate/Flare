//! Finding ffmpeg and ffprobe on this computer.

use std::path::{Path, PathBuf};

use crate::error::FfmpegError;

/// The two programs found on this computer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tools {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}

/// Finds ffmpeg and ffprobe on PATH. Both are needed; without either, [`FfmpegError::NotInstalled`].
pub fn locate() -> Result<Tools, FfmpegError> {
    Ok(Tools {
        ffmpeg: find("ffmpeg").ok_or(FfmpegError::NotInstalled)?,
        ffprobe: find("ffprobe").ok_or(FfmpegError::NotInstalled)?,
    })
}

fn find(name: &str) -> Option<PathBuf> {
    find_in(&std::env::var_os("PATH")?, name)
}

/// The first file called `name` in the directories of `path` (a PATH-style list).
pub(crate) fn find_in(path: &std::ffi::OsStr, name: &str) -> Option<PathBuf> {
    let file = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    };
    std::env::split_paths(path)
        .map(|dir| dir.join(&file))
        .find(|candidate| is_program(candidate))
}

fn is_program(path: &Path) -> bool {
    path.is_file()
}
