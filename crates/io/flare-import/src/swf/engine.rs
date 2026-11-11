//! Recognition of the Flash 3D engines a movie's scripts are written against.
//!
//! Flare does not run ActionScript, but a movie's compiled scripts (DoABC blocks) name the
//! packages they use, and a 3D engine's packages are distinctive. Naming the engine tells a
//! person what a movie needs to run — the first step in re-hosting or patching an old Flash
//! 3D game — and what Flare's import does not carry over. The names are recognised from
//! package prefixes the file itself carries; nothing is guessed about versions, and a movie
//! whose scripts name no known engine is reported no differently than before.

use super::Movie;
use crate::xfl::Severity;
use std::collections::{BTreeMap, BTreeSet};

/// The most DoABC blocks read per movie. A real movie has a few; a crafted file can carry
/// thousands, and reading them all reports nothing the first few do not.
const MAX_ABC_BLOCKS: usize = 32;

/// The most package names kept per movie for the report.
const MAX_PACKAGES: usize = 64;

/// The most package names listed in one message before the rest are counted.
const MAX_NAMES_IN_MESSAGE: usize = 5;

/// A string longer than this is not a package name; it is not kept.
const MAX_STRING_BYTES: usize = 128;

/// The package prefixes that name a 3D engine: (prefix, engine). The table is the whole rule.
/// Two prefixes may name one engine (an engine renamed its packages across versions).
const ENGINES: [(&str, &str); 6] = [
    ("flare.", "Flare3D"),
    ("flare3d.", "Flare3D"),
    ("alternativa.", "Alternativa3D"),
    ("alternativa3d.", "Alternativa3D"),
    ("away3d.", "Away3D"),
    ("org.papervision3d.", "Papervision3D"),
];

/// The package the Flash 3D engines render through.
const STAGE3D_PREFIX: &str = "flash.display3D";

impl Movie {
    /// Reads the movie's script blocks and reports what 3D engine they are written
    /// against. A movie's embedded binary data blocks are counted too: a 3D engine's
    /// scene and texture data usually arrives that way. Nothing is imported from either.
    pub(super) fn detect_engines(&mut self, tags: &[swf::Tag<'_>]) {
        let mut blocks = 0usize;
        let mut unreadable = 0usize;
        let mut binary = 0usize;
        let mut packages: BTreeSet<String> = BTreeSet::new();
        let mut stage3d = false;
        for tag in tags {
            let data = match tag {
                swf::Tag::DoAbc(data) => *data,
                swf::Tag::DoAbc2(abc) => abc.data,
                swf::Tag::DefineBinaryData(_) => {
                    binary += 1;
                    continue;
                }
                _ => continue,
            };
            blocks += 1;
            if blocks > MAX_ABC_BLOCKS {
                continue;
            }
            match swf::avm2::read::Reader::new(data).read() {
                Ok(abc) => {
                    collect_packages(&abc.constant_pool.strings, &mut packages, &mut stage3d)
                }
                Err(_) => unreadable += 1,
            }
        }
        if blocks == 0 {
            return;
        }
        let engines = engines_of(&packages);
        for (engine, names) in &engines {
            self.note(
                Severity::Info,
                "scripts",
                format!(
                    "the movie's scripts use the {engine} engine ({}). \
                     Scripts are not imported, so its 3D content does not appear.",
                    names_message(names),
                ),
            );
        }
        if stage3d && engines.is_empty() {
            self.note(
                Severity::Info,
                "scripts",
                format!(
                    "the movie's scripts use Stage 3D ({STAGE3D_PREFIX}.*), the GPU interface \
                     the Flash 3D engines render through. Scripts are not imported.",
                ),
            );
        }
        if !engines.is_empty() && binary > 0 {
            self.note(
                Severity::Info,
                "scripts",
                format!(
                    "the movie also carries {binary} embedded binary data block(s); \
                     a 3D engine's scene and texture data usually arrives that way",
                ),
            );
        }
        if unreadable > 0 {
            self.note(
                Severity::Info,
                "scripts",
                format!(
                    "{unreadable} script block(s) could not be read, so they were \
                     not scanned for 3D engine content",
                ),
            );
        }
        let skipped = blocks.saturating_sub(MAX_ABC_BLOCKS);
        if skipped > 0 {
            self.note(
                Severity::Info,
                "scripts",
                format!("{skipped} further script block(s) were not scanned"),
            );
        }
    }
}

/// Keeps the package names a constant pool carries: every string that names a known engine
/// package, and whether Stage 3D is used at all. Bounded so a crafted pool cannot grow the
/// report without end.
fn collect_packages(strings: &[Vec<u8>], packages: &mut BTreeSet<String>, stage3d: &mut bool) {
    for string in strings {
        if string.len() > MAX_STRING_BYTES {
            continue;
        }
        let Ok(name) = std::str::from_utf8(string) else {
            continue;
        };
        if name.starts_with(STAGE3D_PREFIX) {
            *stage3d = true;
        }
        if ENGINES.iter().any(|(prefix, _)| name.starts_with(prefix))
            && packages.len() < MAX_PACKAGES
        {
            packages.insert(name.to_owned());
        }
    }
}

/// Groups the found packages by the engine they name, in table order.
fn engines_of(packages: &BTreeSet<String>) -> BTreeMap<&'static str, Vec<String>> {
    let mut by_engine: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    for (prefix, engine) in ENGINES {
        let matched: Vec<String> = packages
            .iter()
            .filter(|name| name.starts_with(prefix))
            .cloned()
            .collect();
        if !matched.is_empty() {
            by_engine.insert(engine, matched);
        }
    }
    by_engine
}

/// The package names for one message: the first few, then how many more.
fn names_message(names: &[String]) -> String {
    let shown = names.iter().take(MAX_NAMES_IN_MESSAGE).cloned().collect::<Vec<_>>();
    let listed = shown.join(", ");
    let rest = names.len() - shown.len();
    if rest > 0 {
        format!("{listed} and {rest} more")
    } else {
        listed
    }
}
