//! Slow desktop work that runs on a thread of its own and reports back through a callback, so
//! the window never waits for it.

use std::io;
use std::path::PathBuf;

use flare_document::Document;
use flare_publish::PublishOptions;
pub use flare_ruffle::{TestMovie, TestMovieError};

fn spawn(name: &str, job: impl FnOnce() + Send + 'static) -> io::Result<()> {
    std::thread::Builder::new()
        .name(name.into())
        .spawn(job)
        .map(|_| ())
}

/// Writes one frame as a PNG. `done` gets the outcome, worded for the person.
pub fn export_png(
    document: Document,
    frame: u32,
    path: PathBuf,
    done: impl FnOnce(Result<(), String>) + Send + 'static,
) -> io::Result<()> {
    spawn("flare-export-image", move || {
        done(flare_runtime::export_png(&document, frame, &path));
    })
}

/// Renders the whole animation as a GIF and writes it. Never overwrites; the file appears only
/// when complete.
pub fn export_gif(
    document: Document,
    path: PathBuf,
    done: impl FnOnce(Result<(), String>) + Send + 'static,
) -> io::Result<()> {
    spawn("flare-export-movie", move || {
        let result = flare_runtime::gif_bytes(&document).and_then(|bytes| {
            if path.exists() {
                return Err(format!("{} already exists", path.display()));
            }
            let directory = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(std::path::Path::new("."));
            let mut staged = tempfile::NamedTempFile::new_in(directory)
                .map_err(|e| format!("Cannot write the movie: {e}"))?;
            std::io::Write::write_all(&mut staged, &bytes)
                .map_err(|e| format!("Cannot write the movie: {e}"))?;
            staged
                .persist(&path)
                .map(|_| ())
                .map_err(|e| format!("Cannot write the movie: {}", e.error))
        });
        done(result);
    })
}

/// Publishes a SWF. Never overwrites; the file appears only when complete.
pub fn publish_swf(
    document: Document,
    path: PathBuf,
    done: impl FnOnce(Result<(), String>) + Send + 'static,
) -> io::Result<()> {
    spawn("flare-publish", move || {
        let options = PublishOptions { compress: true };
        let result = flare_publish::publish_to_file(&document, &options, &path)
            .map(|_| ())
            .map_err(|e| e.to_string());
        done(result);
    })
}

/// Why a test movie could not be started.
pub enum TestMovieStart {
    /// Ruffle is not installed; the message tells the person what to do.
    NoPlayer(&'static str),
}

const NO_RUFFLE: &str = "Test Movie plays your project in Ruffle, which is not installed or not on your PATH. Install Ruffle, or set FLARE_RUFFLE to its location, then try again.";

/// Publishes the document and opens it in Ruffle, on a thread. The player is optional, so a
/// missing one is an answer rather than an error. `done` gets the running movie (dropping it
/// stops the player and removes its files).
pub fn test_movie(
    document: Document,
    done: impl FnOnce(Result<TestMovie, TestMovieError>) + Send + 'static,
) -> io::Result<Result<(), TestMovieStart>> {
    let Some(player) = flare_ruffle::find_ruffle() else {
        return Ok(Err(TestMovieStart::NoPlayer(NO_RUFFLE)));
    };
    spawn("flare-test-movie", move || {
        done(flare_ruffle::launch_test_movie(&document, &player));
    })?;
    Ok(Ok(()))
}
