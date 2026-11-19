//! The Google Fonts families Flare can set text in without them being installed.
//!
//! The catalog (`data/google-fonts.tsv`, written by `scripts/fonts/google_catalog.py`) names each
//! family and the files that set it in the google/fonts repository at one commit. Nothing here
//! downloads: a front end fetches [`GoogleFamily::urls`] its own way (a thread on the desktop,
//! `fetch` in a browser) and hands the bytes to [`crate::TextEngine::add_google_family`], which
//! checks them. The addresses name a commit, so they keep pointing at the same files.

use std::collections::HashMap;
use std::sync::OnceLock;

const CATALOG: &str = include_str!("../data/google-fonts.tsv");

/// Where the repository's files are served, with the CORS header a browser needs.
const RAW_BASE: &str = "https://raw.githubusercontent.com/google/fonts/";

/// The largest font file that is downloaded or accepted. The catalog leaves out larger ones.
pub const MAX_FONT_BYTES: usize = 32 * 1024 * 1024;

/// The most files one family is set with: regular, bold, italic and bold italic.
pub const MAX_FAMILY_FILES: usize = 4;

/// Which face of a family a file is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    /// Upright: every weight when the file is variable, else the regular one.
    Regular,
    Bold,
    /// Italic: every weight when the file is variable, else the regular one.
    Italic,
    BoldItalic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleFamily {
    name: &'static str,
    category: &'static str,
    folder: &'static str,
    files: Vec<(Face, &'static str)>,
}

impl GoogleFamily {
    /// The family's name, as text names it.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Google's grouping: Sans Serif, Serif, Display, Handwriting or Monospace.
    pub fn category(&self) -> &'static str {
        self.category
    }

    pub fn faces(&self) -> impl Iterator<Item = Face> + '_ {
        self.files.iter().map(|(face, _)| *face)
    }

    /// Where each file of the family is downloaded from, one for each face.
    pub fn urls(&self) -> Vec<String> {
        let commit = catalog().commit;
        self.files
            .iter()
            .map(|(_, file)| format!("{RAW_BASE}{commit}/{}/{}", self.folder, encode(file)))
            .collect()
    }

    /// A name for each file that is the same wherever it is cached and differs between
    /// families and commits: `<commit>/<folder>/<file>`, all of it from the catalog.
    pub fn cache_names(&self) -> Vec<String> {
        let commit = catalog().commit;
        self.files
            .iter()
            .map(|(_, file)| format!("{commit}/{}/{file}", self.folder))
            .collect()
    }
}

struct Catalog {
    commit: &'static str,
    families: Vec<GoogleFamily>,
    by_name: HashMap<String, usize>,
}

fn catalog() -> &'static Catalog {
    static PARSED: OnceLock<Catalog> = OnceLock::new();
    PARSED.get_or_init(|| parse(CATALOG))
}

/// Reads the catalog. It is compiled in, but a line that does not read is skipped rather than
/// trusted, and a test checks that none is.
fn parse(text: &'static str) -> Catalog {
    let mut commit = "";
    let mut families = Vec::new();
    for line in text.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        match fields.as_slice() {
            ["commit", sha] if is_commit(sha) => commit = sha,
            [name, category, folder, files] => {
                if let Some(family) = family(name, category, folder, files) {
                    families.push(family);
                }
            }
            _ => {}
        }
    }
    let by_name = families
        .iter()
        .enumerate()
        .map(|(at, family)| (family.name.to_lowercase(), at))
        .collect();
    Catalog {
        commit,
        families,
        by_name,
    }
}

fn family(
    name: &'static str,
    category: &'static str,
    folder: &'static str,
    files: &'static str,
) -> Option<GoogleFamily> {
    let plain = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_[],.".contains(c))
    };
    let (license, dir) = folder.split_once('/')?;
    if name.trim().is_empty()
        || name.chars().count() > flare_document::MAX_FAMILY_CHARS
        || !["ofl", "apache", "ufl"].contains(&license)
        || !plain(dir)
        || dir.contains("..")
    {
        return None;
    }
    let mut parsed = Vec::new();
    for entry in files.split('|') {
        let (face, file) = entry.split_once(':')?;
        let face = match face {
            "r" => Face::Regular,
            "b" => Face::Bold,
            "i" => Face::Italic,
            "bi" => Face::BoldItalic,
            _ => return None,
        };
        if !plain(file) || file.contains("..") || !file.ends_with(".ttf") {
            return None;
        }
        parsed.push((face, file));
    }
    if parsed.is_empty() || parsed.len() > MAX_FAMILY_FILES {
        return None;
    }
    Some(GoogleFamily {
        name,
        category,
        folder,
        files: parsed,
    })
}

fn is_commit(sha: &str) -> bool {
    sha.len() == 40 && sha.chars().all(|c| c.is_ascii_hexdigit())
}

/// Percent-encodes the characters of a file name a URL path cannot hold as they are (variable
/// fonts are named for their axes, as in `Roboto[wdth,wght].ttf`).
fn encode(file: &str) -> String {
    let mut out = String::with_capacity(file.len());
    for byte in file.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Every family in the catalog, by name.
pub fn families() -> &'static [GoogleFamily] {
    &catalog().families
}

/// The family named `name`, whatever its case.
pub fn family_named(name: &str) -> Option<&'static GoogleFamily> {
    let catalog = catalog();
    catalog
        .by_name
        .get(&name.trim().to_lowercase())
        .and_then(|&at| catalog.families.get(at))
}

/// The commit of google/fonts the catalog was made from.
pub fn commit() -> &'static str {
    catalog().commit
}

/// The CSS generic family names, which always resolve and are never downloaded.
const GENERIC: [&str; 13] = [
    "serif",
    "sans-serif",
    "monospace",
    "cursive",
    "fantasy",
    "system-ui",
    "ui-serif",
    "ui-sans-serif",
    "ui-monospace",
    "ui-rounded",
    "emoji",
    "math",
    "fangsong",
];

/// The names in a CSS-style family list, unquoted, in order.
pub(crate) fn names(list: &str) -> impl Iterator<Item = &str> {
    list.split(',')
        .map(|name| name.trim().trim_matches(['"', '\'']).trim())
        .filter(|name| !name.is_empty())
}

pub(crate) fn is_generic(name: &str) -> bool {
    GENERIC
        .iter()
        .any(|generic| generic.eq_ignore_ascii_case(name))
}

#[cfg(test)]
#[path = "google_tests.rs"]
mod tests;
