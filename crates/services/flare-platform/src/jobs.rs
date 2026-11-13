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

/// Writes the whole animation as the movie `path` names by its ending: an animated GIF (`.gif`),
/// an MP4 video (`.mp4`) or a QuickTime movie (`.mov`), the last two with the sound. A name with
/// no ending is a GIF, as Export Movie wrote before it had a choice. Never overwrites; the file
/// appears only when complete.
pub fn export_movie(
    document: Document,
    path: PathBuf,
    done: impl FnOnce(Result<(), String>) + Send + 'static,
) -> io::Result<()> {
    spawn("flare-export-movie", move || {
        done(write_movie(&document, &path));
    })
}

/// [`export_movie`] on the calling thread.
pub fn write_movie(document: &Document, path: &std::path::Path) -> Result<(), String> {
    let ending = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase());
    match ending.as_deref() {
        None => write_gif(document, &path.with_extension("gif")),
        Some("gif") => write_gif(document, path),
        Some("mp4") => flare_video::write_mp4(document, path),
        Some("mov") => flare_video::write_mov(document, path),
        Some(other) => Err(format!(
            "A movie is written as .gif, .mp4 or .mov, not .{other}"
        )),
    }
}

/// Renders `document` as a GIF into `path`. Never overwrites; the file appears only when it is
/// complete.
fn write_gif(document: &Document, path: &std::path::Path) -> Result<(), String> {
    write_new(path, &flare_runtime::gif_bytes(document)?)
}

/// Writes `bytes` to a file that does not exist yet: the file appears only when complete, and an
/// existing one is never replaced.
fn write_new(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    if path.exists() {
        return Err(format!("{} already exists", path.display()));
    }
    let directory = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    let mut staged = tempfile::NamedTempFile::new_in(directory)
        .map_err(|e| format!("Cannot write the file: {e}"))?;
    std::io::Write::write_all(&mut staged, bytes)
        .map_err(|e| format!("Cannot write the file: {e}"))?;
    staged.persist_noclobber(path).map(|_| ()).map_err(|e| {
        if e.error.kind() == io::ErrorKind::AlreadyExists {
            format!("{} already exists", path.display())
        } else {
            format!("Cannot write the file: {}", e.error)
        }
    })
}

/// Writes the project as a `.fla` Animate can open, on a thread. Never overwrites.
pub fn export_fla(
    document: Document,
    path: PathBuf,
    done: impl FnOnce(Result<(), String>) + Send + 'static,
) -> io::Result<()> {
    spawn("flare-export-fla", move || {
        let result = flare_export::export_xfl(&document)
            .and_then(|output| flare_export::fla_bytes(&output))
            .map_err(|e| e.to_string())
            .and_then(|bytes| write_new(&path, &bytes));
        done(result);
    })
}

/// Writes the frame on show as an SVG picture, on a thread. Never overwrites.
pub fn export_svg(
    document: Document,
    frame: u32,
    path: PathBuf,
    done: impl FnOnce(Result<(), String>) + Send + 'static,
) -> io::Result<()> {
    spawn("flare-export-svg", move || {
        let result = flare_runtime::svg_export(&document, frame)
            .and_then(|svg| write_new(&path, svg.text.as_bytes()));
        done(result);
    })
}

/// What to publish: each format that is present is written, beside the others.
#[derive(Clone, Debug)]
pub struct PublishPlan {
    pub swf: Option<PublishOptions>,
    pub html5: Option<Html5Options>,
    /// Also write an animated GIF.
    pub gif: bool,
    /// Also write an MP4 video.
    pub mp4: bool,
    /// Also write a QuickTime movie.
    pub mov: bool,
}

/// The name a published file shares with its siblings: `path` without a `.swf` or `.html`
/// ending, which a person may well have typed.
fn publish_stem(path: &std::path::Path) -> String {
    let name = path
        .file_name()
        .map_or_else(|| "movie".to_owned(), |n| n.to_string_lossy().into_owned());
    let lower = name.to_ascii_lowercase();
    for ending in [".swf", ".html", ".htm", ".gif", ".mp4", ".mov"] {
        if lower.ends_with(ending) {
            return name[..name.len() - ending.len()].to_owned();
        }
    }
    name
}

/// Publishes the formats in `plan` next to `path` (`<name>.swf`, `<name>.html`, `<name>.gif`,
/// `<name>.mp4`, `<name>.mov`) on a thread.
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
    if plan.swf.is_none() && plan.html5.is_none() && !plan.gif && !plan.mp4 && !plan.mov {
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
    if plan.gif {
        let target = at("gif");
        if let Err(error) = write_gif(document, &target) {
            failures.push(error);
        }
    }
    if plan.mp4 {
        let target = at("mp4");
        if let Err(error) = flare_video::write_mp4(document, &target) {
            failures.push(error);
        }
    }
    if plan.mov {
        let target = at("mov");
        if let Err(error) = flare_video::write_mov(document, &target) {
            failures.push(error);
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

/// Publishes the document as a SWF on a thread, for a player that runs inside the app. `done`
/// gets the bytes, or the reason worded for the person.
pub fn publish_movie(
    document: Document,
    done: impl FnOnce(Result<Vec<u8>, String>) + Send + 'static,
) -> io::Result<()> {
    spawn("flare-publish-movie", move || {
        done(
            flare_publish::publish_swf(&document, &PublishOptions::default())
                .map(|published| published.bytes)
                .map_err(|e| e.to_string()),
        );
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
