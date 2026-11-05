//! Reader for scene files (`.tnz`). The XML is read into a bounded tree and then into the typed
//! [`Scene`]; the references between cells, sound columns and level declarations are checked.

mod cells;
mod model;

use std::collections::HashSet;

use crate::error::ToonzError;
use crate::limits::{MAX_CELL_RUNS, MAX_COLUMNS, MAX_FPS, MAX_LEVELS};
use crate::tree::{self, Node};

pub use model::{CellRun, Column, FrameId, LevelDecl, Scene, SoundCells};

use cells::parse_cell_run;

/// Parses the text of a `.tnz` file.
pub fn parse_scene(input: &str) -> Result<Scene, ToonzError> {
    let root = tree::parse(input)?;
    if root.name != "tnz" {
        return Err(ToonzError::UnknownFormat("root element is not <tnz>"));
    }
    let properties = root.child("properties");
    let mut scene = Scene {
        version: root.attr("version").map(str::to_owned),
        generator: root.child("generator").map(|n| n.text.clone()),
        frame_rate: None,
        camera_count: 0,
        camera_res: None,
        camera_size: None,
        levels: Vec::new(),
        columns: Vec::new(),
    };
    if let Some(properties) = properties {
        scene.frame_rate = main_frame_rate(properties)?;
        scene.camera_count = properties
            .child("cameras")
            .map_or(0, |cameras| cameras.children_named("camera").count());
        scene.camera_res = properties
            .child("cameras")
            .and_then(|c| c.children_named("camera").next())
            .and_then(|c| c.child("cameraRes"))
            .and_then(|r| {
                let mut parts = r.text.split_whitespace().map(|v| v.parse::<u32>().ok());
                Some((parts.next()??, parts.next()??))
            })
            .filter(|(w, h)| *w > 0 && *h > 0);
        scene.camera_size = properties
            .child("cameras")
            .and_then(|c| c.children_named("camera").next())
            .and_then(|c| c.child("cameraSize"))
            .and_then(|r| {
                let mut parts = r.text.split_whitespace().map(|v| v.parse::<f64>().ok());
                Some((parts.next()??, parts.next()??))
            })
            .filter(|(w, h)| w.is_finite() && h.is_finite() && *w > 0.0 && *h > 0.0);
    }
    if let Some(level_set) = root.child("levelSet") {
        collect_levels(level_set, &mut scene.levels)?;
    }
    if let Some(xsheet) = root.child("xsheet") {
        scene.columns = read_columns(xsheet)?;
    }
    check_references(&scene)?;
    Ok(scene)
}

/// The `fps` of the output named `main`, or of the first output when none is named so.
fn main_frame_rate(properties: &Node) -> Result<Option<f64>, ToonzError> {
    let Some(outputs) = properties.child("outputs") else {
        return Ok(None);
    };
    let first = outputs.children_named("output").next();
    let main = outputs
        .children_named("output")
        .find(|o| o.attr("name") == Some("main"))
        .or(first);
    let Some(fps) = main.and_then(|o| o.child("fps")) else {
        return Ok(None);
    };
    let value: f64 =
        fps.text.trim().parse().map_err(|_| {
            ToonzError::malformed("tnz", format!("fps `{}` is not a number", fps.text))
        })?;
    if !value.is_finite() || value <= 0.0 || value > MAX_FPS {
        return Err(ToonzError::malformed(
            "tnz",
            format!("fps {value} is outside (0, {MAX_FPS}]"),
        ));
    }
    Ok(Some(value))
}

/// Walks the level set (it nests `folder` elements) and collects each `level` declaration.
fn collect_levels(node: &Node, out: &mut Vec<LevelDecl>) -> Result<(), ToonzError> {
    for child in &node.children {
        if child.name == "level" {
            if out.len() >= MAX_LEVELS {
                return Err(ToonzError::Limit {
                    what: "level count",
                    max: MAX_LEVELS,
                });
            }
            let id_text = child
                .attr("id")
                .ok_or_else(|| ToonzError::malformed("tnz", "level has no id"))?;
            let id: u32 = id_text
                .parse()
                .map_err(|_| ToonzError::malformed("tnz", format!("level id `{id_text}`")))?;
            if out.iter().any(|l| l.id == id) {
                return Err(ToonzError::malformed(
                    "tnz",
                    format!("level id {id} declared twice"),
                ));
            }
            out.push(LevelDecl {
                id,
                name: child.attr("name").unwrap_or_default().to_owned(),
                kind: child.attr("type").unwrap_or_default().to_owned(),
                path: child.child("path").map(|p| p.text.clone()),
            });
        } else {
            collect_levels(child, out)?;
        }
    }
    Ok(())
}

fn read_columns(xsheet: &Node) -> Result<Vec<Column>, ToonzError> {
    let Some(columns) = xsheet.child("columns") else {
        return Ok(Vec::new());
    };
    if columns.children.len() > MAX_COLUMNS {
        return Err(ToonzError::Limit {
            what: "column count",
            max: MAX_COLUMNS,
        });
    }
    let mut total_runs = 0usize;
    let mut out = Vec::with_capacity(columns.children.len());
    for column in &columns.children {
        let kind = match column.name.as_str() {
            "levelColumn" => Column::Level(read_runs(column, &mut total_runs)?),
            "paletteColumn" => Column::Palette(read_runs(column, &mut total_runs)?),
            "soundColumn" => match column.child("SoundCells") {
                Some(cells) => Column::Sound(parse_sound(&cells.text)?),
                None => Column::Other {
                    tag: column.name.clone(),
                },
            },
            other => Column::Other {
                tag: other.to_owned(),
            },
        };
        out.push(kind);
    }
    Ok(out)
}

fn read_runs(column: &Node, total: &mut usize) -> Result<Vec<CellRun>, ToonzError> {
    let Some(cells) = column.child("cells") else {
        return Ok(Vec::new());
    };
    let mut runs = Vec::new();
    for cell in cells.children_named("cell") {
        *total += 1;
        if *total > MAX_CELL_RUNS {
            return Err(ToonzError::Limit {
                what: "cell runs",
                max: MAX_CELL_RUNS,
            });
        }
        runs.push(parse_cell_run(&cell.text)?);
    }
    Ok(runs)
}

/// `SoundCells`: start offset, end offset, start frame, then the sound level's id.
fn parse_sound(text: &str) -> Result<SoundCells, ToonzError> {
    let fields: Vec<u32> = text
        .split_whitespace()
        .map(|f| {
            f.parse::<u32>()
                .map_err(|_| ToonzError::malformed("tnz", format!("sound field `{f}`")))
        })
        .collect::<Result<_, _>>()?;
    match fields.as_slice() {
        [start_offset, end_offset, start_frame, level] => Ok(SoundCells {
            start_offset: *start_offset,
            end_offset: *end_offset,
            start_frame: *start_frame,
            level: *level,
        }),
        _ => Err(ToonzError::malformed("tnz", "SoundCells needs four fields")),
    }
}

/// Every cell and sound column must refer to a declared level, and the runs of one column must
/// be in row order without overlap (the writer never produces anything else).
fn check_references(scene: &Scene) -> Result<(), ToonzError> {
    let declared: HashSet<u32> = scene.levels.iter().map(|l| l.id).collect();
    let require = |level: u32| {
        if declared.contains(&level) {
            Ok(())
        } else {
            Err(ToonzError::malformed(
                "tnz",
                format!("cell refers to undeclared level {level}"),
            ))
        }
    };
    for column in &scene.columns {
        match column {
            Column::Level(runs) | Column::Palette(runs) => {
                let mut next_row = 0u32;
                for run in runs {
                    require(run.level)?;
                    if run.row < next_row {
                        return Err(ToonzError::malformed(
                            "tnz",
                            format!("cell at row {} overlaps the previous run", run.row),
                        ));
                    }
                    next_row = run.row + run.count;
                }
            }
            Column::Sound(sound) => require(sound.level)?,
            Column::Other { .. } => {}
        }
    }
    Ok(())
}
