//! The strings a movie's scripts carry, what uses each one, and replacing them.

use crate::movie::Movie;
use crate::pool::{section_strings, string_section, with_strings};
use crate::{MAX_REPLACEMENT_BYTES, MAX_REPLACEMENTS, ScriptError};
use serde::Serialize;
use std::collections::HashSet;
use swf::avm2::types::{AbcFile, DefaultValue, Multiname, Op, Trait, TraitKind};
use swf::extensions::ReadSwfExt;

/// How a string is used. One string can be all three: the compiler keeps one copy of each text.
#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
pub struct Uses {
    /// Pushed by code or given as a default value: text the program works with.
    pub text: usize,
    /// A class, package, namespace, property or method name the code links by.
    pub name: usize,
    /// Debug information, metadata or a parameter's name: read by tools, not by the program.
    pub other: usize,
}

/// One string of one block.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ScriptString {
    pub block: usize,
    /// Its index in the block's string pool.
    pub index: u32,
    pub text: String,
    pub uses: Uses,
}

/// Which strings a listing gives.
#[derive(Debug, Clone, Default)]
pub struct StringQuery {
    /// Only strings containing this, ignoring case.
    pub find: Option<String>,
    /// Only strings the code uses as text (not only as names).
    pub text_only: bool,
    /// The most strings listed; the rest are counted. `None` lists every one.
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Strings {
    pub matched: usize,
    pub strings: Vec<ScriptString>,
    /// Blocks that could not be read, with why.
    pub unreadable: Vec<String>,
}

/// One string to replace by another, matched whole and exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replacement {
    pub from: String,
    pub to: String,
}

/// What one replacement changed.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct StringChange {
    pub from: String,
    pub to: String,
    /// The blocks it was found in.
    pub blocks: Vec<usize>,
    /// Its uses, added over those blocks.
    pub uses: Uses,
}

/// A patched movie and what changed in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Patched {
    pub bytes: Vec<u8>,
    pub changes: Vec<StringChange>,
}

/// The strings of every block that match `query`, in block and pool order.
pub fn strings(movie: &Movie, query: &StringQuery) -> Strings {
    let find = query.find.as_ref().map(|f| f.to_lowercase());
    let limit = query.limit.unwrap_or(usize::MAX);
    let mut out = Strings {
        matched: 0,
        strings: Vec::new(),
        unreadable: Vec::new(),
    };
    for block in 0..movie.block_count() {
        let abc = match movie.abc(block) {
            Ok(abc) => abc,
            Err(error) => {
                out.unreadable.push(error.to_string());
                continue;
            }
        };
        let uses = uses_of(&abc);
        for (at, bytes) in abc.constant_pool.strings.iter().enumerate() {
            let index = at + 1;
            let uses = uses.get(index).copied().unwrap_or_default();
            let text = String::from_utf8_lossy(bytes);
            if query.text_only && uses.text == 0 {
                continue;
            }
            if find
                .as_ref()
                .is_some_and(|f| !text.to_lowercase().contains(f))
            {
                continue;
            }
            out.matched += 1;
            if out.strings.len() < limit {
                out.strings.push(ScriptString {
                    block,
                    index: index as u32,
                    text: text.into_owned(),
                    uses,
                });
            }
        }
    }
    out
}

/// The movie with each replacement made in every block that has the string. All or nothing: a
/// string found nowhere, one the code links by (unless `allow_names`), or a block that holds a
/// string to replace but cannot be read refuses the whole patch.
pub fn patch_strings(
    movie: &Movie,
    replacements: &[Replacement],
    allow_names: bool,
) -> Result<Patched, ScriptError> {
    check(replacements)?;
    let mut changes: Vec<StringChange> = replacements
        .iter()
        .map(|r| StringChange {
            from: r.from.clone(),
            to: r.to.clone(),
            blocks: Vec::new(),
            uses: Uses::default(),
        })
        .collect();
    let mut rebuilt = Vec::new();
    for block in 0..movie.block_count() {
        let unreadable = |reason: &str| ScriptError::Unreadable {
            block,
            reason: reason.to_owned(),
        };
        let bytes = movie
            .abc_bytes(block)
            .ok_or_else(|| unreadable("it is not in the movie"))?;
        let Some(section) = string_section(bytes) else {
            // A block whose strings cannot even be found is kept as it is: the player could
            // not read it either, so nothing in it runs.
            continue;
        };
        let abc = match movie.abc(block) {
            Ok(abc) => abc,
            Err(error) => {
                // Its strings are there but the rest cannot be read, so what uses them is not
                // known: replacing one could break a link. Refuse rather than guess.
                let wanted = section_strings(bytes, &section)
                    .iter()
                    .any(|s| replacements.iter().any(|r| r.from.as_bytes() == *s));
                if wanted {
                    return Err(error);
                }
                continue;
            }
        };
        if section.count != abc.constant_pool.strings.len() {
            return Err(unreadable(
                "its string pool does not read the same two ways",
            ));
        }
        let uses = uses_of(&abc);
        let mut pool = abc.constant_pool.strings.clone();
        let mut changed = false;
        for (at, string) in pool.iter_mut().enumerate() {
            let Some(i) = replacements
                .iter()
                .position(|r| r.from.as_bytes() == string)
            else {
                continue;
            };
            let found = uses.get(at + 1).copied().unwrap_or_default();
            if found.name > 0 && !allow_names {
                return Err(ScriptError::IsName(replacements[i].from.clone()));
            }
            let change = &mut changes[i];
            if !change.blocks.contains(&block) {
                change.blocks.push(block);
            }
            change.uses.text += found.text;
            change.uses.name += found.name;
            change.uses.other += found.other;
            *string = replacements[i].to.as_bytes().to_vec();
            changed = true;
        }
        if changed {
            rebuilt.push((block, with_strings(bytes, &section, &pool)));
        }
    }
    if let Some(missing) = changes.iter().find(|c| c.blocks.is_empty()) {
        return Err(ScriptError::NotFound(missing.from.clone()));
    }
    Ok(Patched {
        bytes: movie.rebuilt(&rebuilt)?,
        changes,
    })
}

fn check(replacements: &[Replacement]) -> Result<(), ScriptError> {
    if replacements.is_empty() {
        return Err(ScriptError::EmptyPatch);
    }
    if replacements.len() > MAX_REPLACEMENTS {
        return Err(ScriptError::TooManyReplacements);
    }
    let mut seen = HashSet::new();
    for r in replacements {
        if r.from.is_empty() {
            return Err(ScriptError::EmptyFrom);
        }
        if !seen.insert(r.from.as_str()) {
            return Err(ScriptError::Duplicate(r.from.clone()));
        }
        if r.to.len() > MAX_REPLACEMENT_BYTES {
            return Err(ScriptError::ReplacementTooLong(r.from.clone()));
        }
    }
    Ok(())
}

/// How each string of `abc` is used, by pool index (0 is the implied empty string).
fn uses_of(abc: &AbcFile) -> Vec<Uses> {
    let mut uses = vec![Uses::default(); abc.constant_pool.strings.len() + 1];
    let mut count = |index: u32, kind: fn(&mut Uses) -> &mut usize| {
        if let Some(u) = uses.get_mut(index as usize) {
            *kind(u) += 1;
        }
    };
    let text: fn(&mut Uses) -> &mut usize = |u| &mut u.text;
    let name: fn(&mut Uses) -> &mut usize = |u| &mut u.name;
    let other: fn(&mut Uses) -> &mut usize = |u| &mut u.other;
    let pool = &abc.constant_pool;
    for ns in &pool.namespaces {
        count(crate::names::namespace_string(ns), name);
    }
    for multiname in &pool.multinames {
        match multiname {
            Multiname::QName { name: n, .. }
            | Multiname::QNameA { name: n, .. }
            | Multiname::RTQName { name: n }
            | Multiname::RTQNameA { name: n }
            | Multiname::Multiname { name: n, .. }
            | Multiname::MultinameA { name: n, .. } => count(n.0, name),
            _ => {}
        }
    }
    for method in &abc.methods {
        count(method.name.0, other);
        for param in &method.params {
            if let Some(n) = param.name {
                count(n.0, other);
            }
            if let Some(DefaultValue::String(s)) = param.default_value {
                count(s.0, text);
            }
        }
    }
    for entry in &abc.metadata {
        count(entry.name.0, other);
        for item in &entry.items {
            count(item.key.0, other);
            count(item.value.0, other);
        }
    }
    let traits = abc
        .instances
        .iter()
        .flat_map(|i| &i.traits)
        .chain(abc.classes.iter().flat_map(|c| &c.traits))
        .chain(abc.scripts.iter().flat_map(|s| &s.traits))
        .chain(abc.method_bodies.iter().flat_map(|b| &b.traits));
    for t in traits {
        if let Some(s) = default_string(t) {
            count(s, text);
        }
    }
    for body in &abc.method_bodies {
        let code = body.code.as_slice();
        let mut reader = swf::avm2::read::Reader::new(code);
        while reader.pos(code) < code.len() {
            match reader.read_op() {
                Ok(Op::PushString { value }) => count(value.0, text),
                Ok(Op::Dxns { index }) => count(index.0, name),
                Ok(Op::DebugFile { file_name }) => count(file_name.0, other),
                Ok(Op::Debug { register_name, .. }) => count(register_name.0, other),
                Ok(_) => {}
                Err(_) => break,
            }
        }
    }
    uses
}

/// The string a variable's or constant's default value is, if it is one.
fn default_string(t: &Trait) -> Option<u32> {
    match &t.kind {
        TraitKind::Slot {
            value: Some(DefaultValue::String(s)),
            ..
        }
        | TraitKind::Const {
            value: Some(DefaultValue::String(s)),
            ..
        } => Some(s.0),
        _ => None,
    }
}
