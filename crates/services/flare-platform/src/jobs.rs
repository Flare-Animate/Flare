//! Slow desktop work that runs on a thread of its own and reports back through a callback, so
//! the window never waits for it.

use std::io;
use std::path::PathBuf;

use flare_document::Document;
pub use flare_publish::{Html5Options, PublishOptions};
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
            staged.persist_noclobber(&path).map(|_| ()).map_err(|e| {
                if e.error.kind() == io::ErrorKind::AlreadyExists {
                    format!("{} already exists", path.display())
                } else {
                    format!("Cannot write the movie: {}", e.error)
                }
            })
        });
        done(result);
    })
}

/// What to publish: each format that is present is written, beside the others.
#[derive(Clone, Debug)]
pub struct PublishPlan {
    pub swf: Option<PublishOptions>,
    pub html5: Option<Html5Options>,
}

/// The name a published file shares with its siblings: `path` without a `.swf` or `.html`
/// ending, which a person may well have typed.
fn publish_stem(path: &std::path::Path) -> String {
    let name = path
        .file_name()
        .map_or_else(|| "movie".to_owned(), |n| n.to_string_lossy().into_owned());
    let lower = name.to_ascii_lowercase();
    for ending in [".swf", ".html", ".htm"] {
        if lower.ends_with(ending) {
            return name[..name.len() - ending.len()].to_owned();
        }
    }
    name
}

/// Publishes the formats in `plan` next to `path` (`<name>.swf`, `<name>.html`) on a thread.
/// Never overwrites; each file appears only when complete. The first failure is reported, but
/// the other formats are still tried.
pub fn publish(
    document: Document,
    path: PathBuf,
    plan: PublishPlan,
    done: impl FnOnce(Result<(), String>) + Send + 'static,
) -> io::Result<()> {
    spawn("flare-publish", move || {
        done(publish_now(&document, &path, &plan));
    })
}

/// [`publish`] on the calling thread.
pub fn publish_now(
    document: &Document,
    path: &std::path::Path,
    plan: &PublishPlan,
) -> Result<(), String> {
    if plan.swf.is_none() && plan.html5.is_none() {
        return Err("Choose a format in Publish Settings first".to_owned());
    }
    let stem = publish_stem(path);
    let at = |extension: &str| path.with_file_name(format!("{stem}.{extension}"));
    let mut failures = Vec::new();
    if let Some(options) = &plan.swf {
        let target = at("swf");
        if let Err(error) = flare_publish::publish_to_file(document, options, &target) {
            failures.push(format!("{}: {error}", target.display()));
        }
    }
    if let Some(options) = &plan.html5 {
        let mut options = options.clone();
        options.title = stem.clone();
        let target = at("html");
        if let Err(error) = flare_publish::publish_html5_to_file(document, &options, &target) {
            failures.push(format!("{}: {error}", target.display()));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
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
