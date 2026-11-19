//! A Flash movie's ActionScript 3, read and patched from a file in the export directory: the same
//! `flare_as3` reading and patch `flare scripts` uses (issue #230, keeping old Flash 3D games
//! running). The open project is never changed; a patch writes a new movie beside the old one.

use std::io::Write;

use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use serde_json::{Value, json};

use super::export::{export_dir, export_file, fresh};
use crate::color::safe_file_name_with;
use crate::params_scripts::{ListScripts, PatchStrings, ScriptClass, ScriptStrings};
use crate::server::{FlareServer, Outcome, State};
use flare_as3::{Movie, OutlineQuery, Replacement, SourceOptions, StringQuery};

/// Classes returned when the caller sets no limit, so one call cannot return a whole engine.
const DEFAULT_CLASSES: usize = 200;
/// Strings returned when the caller sets no limit.
const DEFAULT_STRINGS: usize = 500;

fn movie(state: &State, source: &str) -> Result<Movie, String> {
    let path = export_file(state, source, "the movie")?;
    let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    if meta.len() > flare_as3::MAX_MOVIE_BYTES {
        return Err(format!(
            "{source} is larger than {} bytes, more than a movie is read at",
            flare_as3::MAX_MOVIE_BYTES
        ));
    }
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    Movie::read(&bytes).map_err(|e| e.to_string())
}

fn to_json(value: impl serde::Serialize) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|e| e.to_string())
}

#[tool_router(router = script_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(
        description = "List the ActionScript 3 classes a Flash movie (.swf) in the export directory defines, as its compiled scripts record them: each with its package, base class, interfaces and members (variables and constants with type and value, methods with parameters and return type). For reading an old Flash game's code, such as one built on the Alternativa3D or Flare3D engine. Returns `blocks`, `matched`, `classes` and any `unreadable` blocks. The open project is not changed."
    )]
    fn movie_scripts(&self, Parameters(args): Parameters<ListScripts>) -> Outcome {
        self.read(|state| {
            let query = OutlineQuery {
                find: args.find,
                limit: Some(args.limit.unwrap_or(DEFAULT_CLASSES)),
            };
            to_json(flare_as3::outline(&movie(state, &args.source)?, &query))
        })
    }

    #[tool(
        description = "Write one ActionScript 3 class of a Flash movie in the export directory out as a declaration: package, class, members with types and values, method signatures. Method bodies are compiled code: with `code` each is listed as instructions (pushstring, callproperty, branches to labels) in comments, otherwise only its size is given. It is not source that compiles. Returns `class`, `block`, `text` and `truncated`."
    )]
    fn movie_script_class(&self, Parameters(args): Parameters<ScriptClass>) -> Outcome {
        self.read(|state| {
            let options = SourceOptions {
                code: args.code.unwrap_or(false),
                member: args.member,
            };
            let source =
                flare_as3::class_source(&movie(state, &args.source)?, &args.class, &options)
                    .map_err(|e| e.to_string())?;
            Ok(json!({
                "class": source.class.full_name,
                "block": source.class.block,
                "text": source.text,
                "truncated": source.truncated,
            }))
        })
    }

    #[tool(
        description = "List the strings a Flash movie's ActionScript 3 carries (server addresses, URLs, messages, names), each with its block, its pool index and its uses: `text` (pushed by code or a default value), `name` (a class, package or property name the code links by) and `other` (debug information). Use it to find what patch_movie_strings should replace. Returns `matched`, `strings` and any `unreadable` blocks."
    )]
    fn movie_script_strings(&self, Parameters(args): Parameters<ScriptStrings>) -> Outcome {
        self.read(|state| {
            let query = StringQuery {
                find: args.find,
                text_only: args.text_only.unwrap_or(false),
                limit: Some(args.limit.unwrap_or(DEFAULT_STRINGS)),
            };
            to_json(flare_as3::strings(&movie(state, &args.source)?, &query))
        })
    }

    #[tool(
        description = "Replace strings in a Flash movie's ActionScript 3 (an old game's server address, a URL, a message) and write the patched movie as a new .swf in the export directory; the original and the open project are not changed. Each `from` is matched whole and exactly. Only the strings change: every other byte of the scripts and every other tag is kept, in the movie's own compression. All or nothing: a string found nowhere, a string the code links by as a name (unless `rename_names`), or an existing file of that name refuses the whole patch. Returns `path`, `bytes` and `changes` (the blocks each string was found in and its uses)."
    )]
    fn patch_movie_strings(&self, Parameters(args): Parameters<PatchStrings>) -> Outcome {
        self.read(|state| {
            let dir = export_dir(state)?;
            let out = fresh(&dir, &safe_file_name_with(&args.name, "swf")?)?;
            let replacements: Vec<Replacement> = args
                .replacements
                .into_iter()
                .map(|r| Replacement {
                    from: r.from,
                    to: r.to,
                })
                .collect();
            let patched = flare_as3::patch_strings(
                &movie(state, &args.source)?,
                &replacements,
                args.rename_names.unwrap_or(false),
            )
            .map_err(|e| e.to_string())?;
            // The movie appears whole or not at all, and never over a file that arrived since.
            let mut file = tempfile::NamedTempFile::new_in(&dir).map_err(|e| e.to_string())?;
            file.write_all(&patched.bytes).map_err(|e| e.to_string())?;
            file.persist_noclobber(&out)
                .map_err(|e| format!("cannot write {}: {}", args.name, e.error))?;
            Ok(json!({
                "path": out.to_string_lossy(),
                "bytes": patched.bytes.len(),
                "changes": to_json(&patched.changes)?,
            }))
        })
    }
}
