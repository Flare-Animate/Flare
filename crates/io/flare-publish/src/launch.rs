//! Test Movie: play the published SWF in an external Ruffle process.
//!
//! Running a foreign player is a trust boundary, so it is kept at arm's length: the SWF goes
//! in a private temporary directory, the player is started directly (never through a shell)
//! with exactly one argument, its standard streams are closed, and the directory is removed
//! when the movie ends or is dropped. Nothing here makes the SWF itself safe; that is the
//! player's job. The mechanism is tested with a stand-in player, not with Ruffle.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};

use flare_document::Document;
use thiserror::Error;

use crate::{PublishError, PublishOptions, publish_swf};

#[derive(Debug, Error)]
pub enum TestMovieError {
    #[error("the player {0} is not an executable file")]
    NotExecutable(PathBuf),
    #[error(transparent)]
    Publish(#[from] PublishError),
    #[error("i/o error: {0}")]
    Io(#[from] io::Error),
}

/// A running (or finished) test movie. Dropping it stops the player and deletes the files.
pub struct TestMovie {
    child: Child,
    swf: PathBuf,
    // Held so the directory outlives the player; removed on drop.
    _dir: tempfile::TempDir,
}

impl TestMovie {
    /// The SWF the player was given.
    pub fn swf_path(&self) -> &Path {
        &self.swf
    }

    /// `Some` once the player has exited.
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }

    /// Waits for the player to exit, then removes the temporary files.
    pub fn wait(mut self) -> io::Result<ExitStatus> {
        self.child.wait()
    }

    /// Stops the player.
    pub fn kill(&mut self) -> io::Result<()> {
        match self.child.kill() {
            // Already exited is fine.
            Err(error) if error.kind() == io::ErrorKind::InvalidInput => Ok(()),
            other => other,
        }?;
        self.child.wait().map(|_| ())
    }
}

impl Drop for TestMovie {
    fn drop(&mut self) {
        // Do not leave a player running against files that are about to be deleted.
        let _ = self.kill();
    }
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

/// Finds Ruffle: the `FLARE_RUFFLE` variable if it names an executable file, otherwise a
/// `ruffle` executable on `PATH`. The lookup is injectable so tests need no global state.
pub fn find_ruffle_with(env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    if let Some(explicit) = env("FLARE_RUFFLE").filter(|v| !v.is_empty()) {
        let path = PathBuf::from(explicit);
        return is_executable_file(&path).then_some(path);
    }
    let path_var = env("PATH")?;
    let names: &[&str] = if cfg!(windows) {
        &["ruffle.exe", "ruffle"]
    } else {
        &["ruffle"]
    };
    std::env::split_paths(&path_var)
        .filter(|dir| dir.is_absolute())
        .flat_map(|dir| names.iter().map(move |name| dir.join(name)))
        .find(|candidate| is_executable_file(candidate))
}

pub fn find_ruffle() -> Option<PathBuf> {
    find_ruffle_with(|name| std::env::var_os(name))
}

/// Publishes `document` into a private temporary directory and starts `player` on it.
pub fn launch_test_movie(document: &Document, player: &Path) -> Result<TestMovie, TestMovieError> {
    if !is_executable_file(player) {
        return Err(TestMovieError::NotExecutable(player.to_owned()));
    }
    let published = publish_swf(document, &PublishOptions::default())?;
    // tempfile creates the directory readable by this user only.
    let dir = tempfile::Builder::new()
        .prefix("flare-test-movie-")
        .tempdir()?;
    let swf = dir.path().join("movie.swf");
    std::fs::write(&swf, &published.bytes)?;
    let child = Command::new(player)
        .arg(&swf)
        .current_dir(dir.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(TestMovie {
        child,
        swf,
        _dir: dir,
    })
}
