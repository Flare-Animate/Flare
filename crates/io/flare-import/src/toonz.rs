//! Toonz files as Flare projects: a scene (`.tnz`) with the level files it names that can be
//! found beside it, a vector level (`.pli`) on its own as a one-column scene, and a palette as a
//! listing. Raster levels (`.tlv`) are not decoded yet and are refused by name.

use std::path::{Component, Path, PathBuf};

use flare_toonz::{Column, Format, LevelDecl, Scene, Severity as TzSeverity, sniff};

use crate::xfl::{Diagnostic, Imported, Severity};

/// Most bytes read for one level file.
const MAX_LEVEL_BYTES: u64 = 64 * 1024 * 1024;

pub(crate) fn import(name: &Path, bytes: &[u8]) -> Result<Imported, String> {
    let ext = name
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let dir = name.parent().filter(|d| !d.as_os_str().is_empty());
    match ext.as_str() {
        "tnz" => {
            let text =
                std::str::from_utf8(bytes).map_err(|_| "the scene is not UTF-8".to_owned())?;
            let scene = flare_toonz::parse_scene(text).map_err(|e| e.to_string())?;
            convert(&scene, &|decl| read_level(dir, decl))
        }
        "pli" => {
            let file = name
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("level.pli");
            let level = flare_toonz::decode_pli(bytes).map_err(|e| e.to_string())?;
            let frames: Vec<u32> = level.frames.iter().map(|f| f.number).collect();
            let scene = single_level_scene(file, &frames);
            let owned = bytes.to_vec();
            convert(&scene, &|_| Some(owned.clone()))
        }
        "tlv" => match sniff(bytes) {
            Format::Tlv { .. } => Err(flare_toonz::decode_tlv(bytes).err().map_or_else(
                || "raster levels are not supported".to_owned(),
                |e| e.to_string(),
            )),
            _ => Err("not a TLV raster level".to_owned()),
        },
        "tpl" | "plt" => {
            let text =
                std::str::from_utf8(bytes).map_err(|_| "the palette is not UTF-8".to_owned())?;
            let palette = flare_toonz::parse_palette(text).map_err(|e| e.to_string())?;
            Err(format!(
                "a palette is not a project: it holds {} styles on {} pages; import a scene or a vector level",
                palette.style_count,
                palette.pages.len()
            ))
        }
        _ => Err("unknown Toonz file type".to_owned()),
    }
}

/// A scene that shows each drawing of one level on one column, a frame per row.
fn single_level_scene(file: &str, frames: &[u32]) -> Scene {
    use flare_toonz::{CellRun, FrameId};
    let runs = frames
        .iter()
        .enumerate()
        .map(|(row, number)| CellRun {
            row: u32::try_from(row).unwrap_or(0),
            count: 1,
            level: 1,
            frame: FrameId {
                number: *number,
                letter: None,
            },
            increment: 0,
        })
        .collect();
    Scene {
        levels: vec![LevelDecl {
            id: 1,
            name: file.to_owned(),
            kind: "vector".to_owned(),
            path: Some(file.to_owned()),
        }],
        columns: vec![Column::Level(runs)],
        ..Scene::default()
    }
}

fn convert(
    scene: &Scene,
    read: &dyn Fn(&LevelDecl) -> Option<Vec<u8>>,
) -> Result<Imported, String> {
    let out = flare_toonz::to_flare_document(scene, read).map_err(|e| e.to_string())?;
    Ok(Imported {
        document: out.document,
        diagnostics: out
            .notes
            .into_iter()
            .map(|n| Diagnostic {
                severity: match n.severity {
                    TzSeverity::Info => Severity::Info,
                    TzSeverity::Warning => Severity::Warning,
                    TzSeverity::Unsupported => Severity::Unsupported,
                },
                location: n.location,
                message: n.message,
            })
            .collect(),
    })
}

/// A level's file: the declared path (`+drawings/A.pli` names the project's `drawings` folder)
/// looked for beside the scene, in the scene's parent folder (scenes live in `scenes/`) and in
/// the folder named by the path. A path that climbs out with `..` or is absolute is never read.
fn read_level(dir: Option<&Path>, decl: &LevelDecl) -> Option<Vec<u8>> {
    use std::io::Read;
    let declared = decl.path.as_deref()?.trim_start_matches('+');
    let relative = Path::new(declared);
    if relative
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return None;
    }
    let dir = dir?;
    let base: Vec<PathBuf> = vec![
        dir.join(relative),
        dir.join("..").join(relative),
        dir.join(relative.file_name()?),
    ];
    for candidate in base {
        if let Ok(file) = std::fs::File::open(&candidate) {
            let mut bytes = Vec::new();
            if file
                .take(MAX_LEVEL_BYTES + 1)
                .read_to_end(&mut bytes)
                .is_ok()
                && bytes.len() as u64 <= MAX_LEVEL_BYTES
            {
                return Some(bytes);
            }
        }
    }
    None
}
