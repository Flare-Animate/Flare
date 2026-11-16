//! A Flash project file (`.flp`): the list of files beside it, of which the one Flash opens by
//! default is imported.

use super::*;

/// The most text a Flash project file (.flp) may hold.
const MAX_FLP_BYTES: usize = 4 * 1024 * 1024;

/// The kinds of file a Flash project names that Flare imports itself. Anything else in the list
/// (scripts, pictures, sounds) is counted and named, not opened.
const FLP_TARGETS: [&str; 4] = ["fla", "xfl", "swf", "swc"];

/// Imports the Flash project file at `path`: the file Flash opens by default out of the files it
/// lists beside itself, or its first listed Flash document. The project XML itself is not kept —
/// Flare imports one document, and the project's other files are named in a note.
pub(super) fn import_flp_path(path: &Path) -> Result<Imported, ImportError> {
    let file = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .to_owned();
    let refused = |reason: &str| ImportError::Foreign(format!("{file}: {reason}"));
    let mut text = String::new();
    File::open(path)?
        .take(MAX_FLP_BYTES as u64 + 1)
        .read_to_string(&mut text)
        .map_err(|_| refused("not UTF-8 text"))?;
    if text.len() > MAX_FLP_BYTES {
        return Err(ImportError::TooLarge("the Flash project".into()));
    }
    if !text.to_ascii_lowercase().contains("flash_project") {
        return Err(refused(
            "not a Flash project file: it has no <flash_project> header",
        ));
    }
    // Flash 8 marks the file its Project panel opens first; without one, the first listed
    // Flash document is the project's movie, as a person opening the project would take it.
    let default_file = attr_value(&text, "flashDefaultFile")
        .or_else(|| attr_value(&text, "flashdefaultfile"))
        .filter(|name| names_a_target(name));
    let listed: Vec<String> = attr_values(&text, "filename")
        .into_iter()
        .chain(attr_values(&text, "name"))
        .filter(|name| names_a_target(name))
        .collect();
    let chosen = match default_file.or(listed.first().cloned()) {
        Some(name) => name,
        None => {
            return Err(refused(
                "the Flash project lists no .fla, .xfl, .swf or .swc file to import",
            ));
        }
    };
    let name = safe_project_reference(&chosen)
        .ok_or_else(|| refused("the file it names does not stay beside the project"))?;
    let folder = match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.to_path_buf(),
        _ => std::path::PathBuf::from("."),
    };
    let target = folder.join(&name);
    let others = listed
        .iter()
        .filter(|candidate| **candidate != chosen)
        .count();
    let mut imported = import_path(&target)?;
    if others > 0 {
        imported.diagnostics.push(Diagnostic {
            severity: Severity::Info,
            location: file.clone(),
            message: format!(
                "the Flash project lists {others} more file(s) that were not imported"
            ),
        });
    }
    imported.diagnostics.push(Diagnostic {
        severity: Severity::Info,
        location: file,
        message: format!("the Flash project's default file is {name}"),
    });
    Ok(imported)
}

/// Whether a name from a Flash project ends in a kind Flare imports.
fn names_a_target(name: &str) -> bool {
    let lowered = name.replace('\\', "/").to_ascii_lowercase();
    FLP_TARGETS
        .iter()
        .any(|ext| lowered.rsplit('.').next().is_some_and(|e| e == *ext))
}

/// A file reference out of a Flash project, normalized to `/` and checked to stay beside the
/// project file: relative, no `..`, no empty parts, and no drive letter. `None` for anything else.
fn safe_project_reference(name: &str) -> Option<String> {
    let normalized = name.replace('\\', "/");
    if normalized.contains(':') || !is_safe_name(&normalized) {
        return None;
    }
    Some(normalized)
}

/// The values of the first attribute `name` in XML `text`, if it has one.
fn attr_value(text: &str, name: &str) -> Option<String> {
    attr_values(text, name).into_iter().next()
}

/// Every value of every attribute `name` in XML `text`, in order. A tolerant scan, like the
/// rest of the tolerant readers: Flash's project file is XML, but its shapes differ by version.
fn attr_values(text: &str, name: &str) -> Vec<String> {
    let marker = format!("{name}=\"");
    let mut values = Vec::new();
    let mut from = 0;
    while from < text.len() {
        let Some(found) = text.get(from..).and_then(|rest| rest.find(&marker)) else {
            break;
        };
        let start = from + found;
        // `name="` also sits inside `filename="`; an attribute begins after a space or a tag,
        // so one whose preceding byte is a letter belongs to the longer name.
        if start > 0 && text.as_bytes()[start - 1].is_ascii_alphabetic() {
            from = start + marker.len();
            continue;
        }
        let at = start + marker.len();
        let Some(after) = text.get(at..) else { break };
        let Some(end) = after.find('"') else { break };
        values.push(after.get(..end).unwrap_or("").to_owned());
        from = at + end + 1;
    }
    values
}
