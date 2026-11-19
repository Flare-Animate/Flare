//! `flare scripts`: a Flash movie's ActionScript 3 read as data, and its strings patched, for
//! someone keeping an old Flash 3D game running (issue #230). The reading and the patch are
//! `flare_as3`'s; this reads the arguments and the movie, writes what is asked and prints.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use flare_as3::{Movie, OutlineQuery, Replacement, SourceOptions, StringQuery};
use serde_json::{Value, json};

#[derive(Subcommand)]
pub enum ScriptsCommand {
    /// List the classes a movie's scripts define: package, base class, interfaces, and each
    /// member with its type, value or signature, as the bytecode records them.
    List {
        movie: PathBuf,
        /// Only classes whose full name contains this (ignoring case).
        #[arg(long)]
        find: Option<String>,
        /// The most classes listed; `matched` still counts them all.
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Write one class out as an ActionScript declaration. Method bodies are compiled code:
    /// with `--code` each is listed as instructions in comments, otherwise only its size is
    /// given. It is not source that compiles.
    Class {
        movie: PathBuf,
        /// The class's full name (`alternativa.tanks.Main`), or its short name when only one
        /// class has it.
        class: String,
        /// Only this member (and the constructor when it is the class's name).
        #[arg(long)]
        member: Option<String>,
        /// List each method body as instructions.
        #[arg(long)]
        code: bool,
        /// Write the text to this new file rather than printing it.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// List the strings the scripts carry, each with what uses it: `text` (pushed by code or a
    /// default value), `name` (a name the code links by) or `other` (debug information).
    Strings {
        movie: PathBuf,
        /// Only strings containing this (ignoring case).
        #[arg(long)]
        find: Option<String>,
        /// Only strings the code uses as text.
        #[arg(long)]
        text: bool,
        /// The most strings listed; `matched` still counts them all.
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Replace strings in the scripts (a server's address, a URL, a message) and write the
    /// patched movie to a new file. Each `--from` is matched whole and exactly and replaced by
    /// the `--to` in the same place. Only the strings change; every other byte of the scripts
    /// and every other tag is kept. A string found nowhere refuses the whole patch, and so does
    /// one the code links by unless `--rename-names` is given.
    Patch {
        movie: PathBuf,
        /// The patched movie; refused if a file is there already.
        #[arg(long)]
        out: PathBuf,
        /// A string to replace (repeat with one `--to` for each).
        #[arg(long = "from", required = true)]
        from: Vec<String>,
        /// What the `--from` in the same place becomes.
        #[arg(long = "to", required = true)]
        to: Vec<String>,
        /// Also replace strings the code links by (class, property and method names). That
        /// breaks every link to the old name that is not renamed with it.
        #[arg(long)]
        rename_names: bool,
    },
}

pub fn run(op: ScriptsCommand) -> Result<Value> {
    match op {
        ScriptsCommand::List { movie, find, limit } => {
            let outline = flare_as3::outline(&read(&movie)?, &OutlineQuery { find, limit });
            Ok(serde_json::to_value(outline)?)
        }
        ScriptsCommand::Class {
            movie,
            class,
            member,
            code,
            out,
        } => {
            let source =
                flare_as3::class_source(&read(&movie)?, &class, &SourceOptions { code, member })?;
            let mut answer = json!({
                "class": source.class.full_name,
                "block": source.class.block,
                "truncated": source.truncated,
            });
            match out {
                Some(path) => {
                    write_new(&path, source.text.as_bytes())?;
                    answer["path"] = json!(path.to_string_lossy());
                }
                None => answer["text"] = json!(source.text),
            }
            Ok(answer)
        }
        ScriptsCommand::Strings {
            movie,
            find,
            text,
            limit,
        } => {
            let query = StringQuery {
                find,
                text_only: text,
                limit,
            };
            Ok(serde_json::to_value(flare_as3::strings(
                &read(&movie)?,
                &query,
            ))?)
        }
        ScriptsCommand::Patch {
            movie,
            out,
            from,
            to,
            rename_names,
        } => {
            if from.len() != to.len() {
                bail!(
                    "each --from needs one --to: {} --from and {} --to were given",
                    from.len(),
                    to.len()
                );
            }
            let replacements: Vec<Replacement> = from
                .into_iter()
                .zip(to)
                .map(|(from, to)| Replacement { from, to })
                .collect();
            let patched = flare_as3::patch_strings(&read(&movie)?, &replacements, rename_names)?;
            write_new(&out, &patched.bytes)?;
            Ok(json!({
                "path": out.to_string_lossy(),
                "bytes": patched.bytes.len(),
                "changes": patched.changes,
            }))
        }
    }
}

fn read(path: &Path) -> Result<Movie> {
    let meta =
        std::fs::metadata(path).with_context(|| format!("cannot open {}", path.display()))?;
    if !meta.is_file() {
        bail!("{} is not a file", path.display());
    }
    if meta.len() > flare_as3::MAX_MOVIE_BYTES {
        bail!(
            "{} is larger than {} bytes, more than a movie is read at",
            path.display(),
            flare_as3::MAX_MOVIE_BYTES
        );
    }
    let bytes = std::fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
    Ok(Movie::read(&bytes)?)
}

/// Writes `bytes` to `path`, which must not exist yet. The file appears whole or not at all.
fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    if path.exists() {
        bail!("{} already exists; give a new file", path.display());
    }
    let dir = match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        _ => Path::new("."),
    };
    let mut file = tempfile::NamedTempFile::new_in(dir)
        .with_context(|| format!("cannot write in {}", dir.display()))?;
    file.write_all(bytes)?;
    file.persist_noclobber(path)
        .map_err(|e| e.error)
        .with_context(|| format!("cannot write {}", path.display()))?;
    Ok(())
}
