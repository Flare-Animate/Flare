//! XDTS timing sheets (`.xdts`, "exchangeDigitalTimeSheet"): the exposure-sheet exchange format
//! written and read by OpenToonz, Clip Studio Paint and TVPaint. The file is an identifier line
//! followed by JSON with one time table: its headers name one track per column, and a track lists
//! the frames where its cell changes. The layout follows OpenToonz's `xdtsio.cpp` (BSD-3-Clause);
//! this reader and writer work on the typed [`Scene`], not on a Flare document.
//!
//! A cell is a drawing id (`12a`) or `SYMBOL_NULL_CELL` for a blank, and an entry applies from its
//! frame until the next one. [`export`] writes one track per level column, bottom column first,
//! with an entry only where the shown drawing changes. [`apply`] rebuilds the cell runs of the
//! level columns a track names. Other sheet symbols (hyphens and ticks) are refused by [`parse`]:
//! only a drawing has a frame to map.

use serde_json::{Value, json};

use crate::error::ToonzError;
use crate::limits::{MAX_FRAME_NUMBER, MAX_XDTS_ROWS};
use crate::scene::{CellRun, Column, FrameId, Scene, parse_frame};

/// The first line of every XDTS file.
pub const IDENTIFIER: &str = "exchangeDigitalTimeSheet Save Data";
/// The cell value that means an empty cell.
const NULL_CELL: &str = "SYMBOL_NULL_CELL";
/// The time table's name on export; a [`Scene`] carries no name of its own.
const DEFAULT_NAME: &str = "Scene 1";

/// A timing sheet read from an `.xdts` file: its name, its rows, and its columns.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TimingSheet {
    pub name: String,
    pub duration: u32,
    pub tracks: Vec<Track>,
}

/// One column of a sheet, named by the sheet's header for its `trackNo`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Track {
    pub name: String,
    /// The frames where the cell changes, in row order.
    pub cells: Vec<Cell>,
}

/// One exposure change: from `frame` on, the column shows `cell` (`None` for a blank).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub frame: u32,
    pub cell: Option<String>,
}

/// The XDTS text for `scene`: one track per level column, bottom column first, named after the
/// level its cells refer to (an unnamed column becomes `Column N`, counting the tracks from 1).
pub fn export(scene: &Scene) -> String {
    let duration = scene.frame_count();
    let mut names: Vec<String> = Vec::new();
    let mut tracks: Vec<Value> = Vec::new();
    for column in &scene.columns {
        let Column::Level(runs) = column else {
            continue;
        };
        let name = column_name(scene, runs, names.len());
        tracks.push(json!({
            "trackNo": tracks.len(),
            "frames": track_frames(runs, duration),
        }));
        names.push(name);
    }
    let document = json!({
        "header": { "cut": "1", "scene": "1" },
        "version": 5,
        "timeTables": [{
            "duration": duration,
            "name": DEFAULT_NAME,
            "timeTableHeaders": [{ "fieldId": 0, "names": names }],
            "fields": [{ "fieldId": 0, "tracks": tracks }],
        }],
    });
    // A `Value` has only string keys, so writing it into a string cannot fail; the fallback keeps
    // the writer total on a path that is unreachable.
    let json = serde_json::to_string_pretty(&document).unwrap_or_else(|_| String::from("{}"));
    format!("{IDENTIFIER}\n{json}\n")
}

/// Reads an `.xdts` file. The identifier line must be the first line, after an optional byte
/// order mark; the first time table is read, and its cells are sorted by frame.
pub fn parse(text: &str) -> Result<TimingSheet, ToonzError> {
    let body = text.strip_prefix('\u{feff}').unwrap_or(text);
    let (identifier, json) = body.split_once('\n').unwrap_or((body, ""));
    if identifier.trim_end_matches('\r') != IDENTIFIER {
        return Err(ToonzError::UnknownFormat("XDTS identifier line"));
    }
    let document: Value = serde_json::from_str(json.trim())
        .map_err(|error| ToonzError::malformed("xdts", format!("JSON: {error}")))?;
    let table = document
        .get("timeTables")
        .and_then(Value::as_array)
        .and_then(|tables| tables.first())
        .ok_or_else(|| ToonzError::malformed("xdts", "no time table"))?;
    let duration = read_duration(table)?;
    let name = table
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_owned();
    let names = header_names(table);
    let Some(field) = cell_field(table, "fields") else {
        return Ok(TimingSheet {
            name,
            duration,
            tracks: Vec::new(),
        });
    };
    let tracks = field
        .get("tracks")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    if tracks.len() > MAX_XDTS_ROWS as usize {
        return Err(limit("timing sheet tracks"));
    }
    let mut budget = MAX_XDTS_ROWS as usize;
    let mut out = Vec::with_capacity(tracks.len());
    for (index, track) in tracks.iter().enumerate() {
        let number = track
            .get("trackNo")
            .and_then(Value::as_u64)
            .and_then(|number| usize::try_from(number).ok())
            .unwrap_or(index);
        let name = names
            .get(number)
            .copied()
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| format!("Track {}", number.saturating_add(1)));
        let cells = read_cells(track, budget)?;
        budget -= cells.len();
        out.push(Track { name, cells });
    }
    Ok(TimingSheet {
        name,
        duration,
        tracks: out,
    })
}

/// Replaces the cells of every level column a track names. A column is named after the level its
/// first cell refers to; a column with no cells takes the first track, in order, whose name is a
/// declared level, so the sheet's column order fills a fresh scene. A track's entry applies from
/// its frame until the next one, and a null or non-drawing cell leaves the row empty. Columns
/// whose name has no track, and tracks that name no column, are left alone.
pub fn apply(sheet: &TimingSheet, scene: &mut Scene) {
    let levels = &scene.levels;
    let mut used = vec![false; sheet.tracks.len()];
    for column in &mut scene.columns {
        let Column::Level(runs) = column else {
            continue;
        };
        let Some(first) = runs.first() else {
            continue;
        };
        let level = first.level;
        let Some(name) = levels
            .iter()
            .find(|decl| decl.id == level)
            .map(|decl| decl.name.trim())
        else {
            continue;
        };
        let Some((index, track)) = sheet
            .tracks
            .iter()
            .enumerate()
            .find(|(_, track)| track.name.trim() == name)
        else {
            continue;
        };
        used[index] = true;
        *runs = rebuild(&track.cells, sheet.duration, level);
    }
    for column in &mut scene.columns {
        let Column::Level(runs) = column else {
            continue;
        };
        if !runs.is_empty() {
            continue;
        }
        let next = sheet.tracks.iter().enumerate().find_map(|(index, track)| {
            if used[index] {
                return None;
            }
            let level = levels
                .iter()
                .find(|decl| decl.name.trim() == track.name.trim())?;
            Some((index, level.id))
        });
        let Some((index, level)) = next else {
            continue;
        };
        used[index] = true;
        if let Some(track) = sheet.tracks.get(index) {
            *runs = rebuild(&track.cells, sheet.duration, level);
        }
    }
}

/// The track name for a level column: its level's name when the id is declared and named.
fn column_name(scene: &Scene, runs: &[CellRun], ordinal: usize) -> String {
    runs.first()
        .and_then(|run| scene.levels.iter().find(|level| level.id == run.level))
        .map(|level| level.name.trim())
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("Column {}", ordinal + 1))
}

/// The exposure changes of one column: an entry at every row whose drawing differs from the row
/// above (the sheet starts blank), and a null entry where content ends before the duration.
fn track_frames(runs: &[CellRun], duration: u32) -> Vec<Value> {
    let mut frames = Vec::new();
    let mut written: Option<String> = None;
    let mut row = 0u32;
    for run in runs {
        if run.row >= duration {
            break;
        }
        let start = run.row.max(row);
        let end = run.row.saturating_add(run.count).min(duration);
        if start >= end {
            continue;
        }
        if start > row {
            write_frame(&mut frames, &mut written, row, None);
        }
        if run.increment == 0 {
            write_frame(
                &mut frames,
                &mut written,
                start,
                drawing(run.frame.number, run.frame.letter),
            );
        } else {
            for at in start..end {
                let step = i64::from(at - run.row) * i64::from(run.increment);
                let number = i64::from(run.frame.number) + step;
                let cell = u32::try_from(number)
                    .ok()
                    .filter(|number| *number <= MAX_FRAME_NUMBER)
                    .and_then(|number| drawing(number, run.frame.letter));
                write_frame(&mut frames, &mut written, at, cell);
            }
        }
        row = end;
    }
    if row < duration {
        write_frame(&mut frames, &mut written, row, None);
    }
    frames
}

fn write_frame(
    frames: &mut Vec<Value>,
    written: &mut Option<String>,
    frame: u32,
    cell: Option<String>,
) {
    if *written == cell {
        return;
    }
    let text = cell.as_deref().unwrap_or(NULL_CELL);
    frames.push(json!({ "frame": frame, "cell": text }));
    *written = cell;
}

/// A drawing's cell text, or none when a frame id could not hold the number.
fn drawing(number: u32, letter: Option<char>) -> Option<String> {
    if number > MAX_FRAME_NUMBER {
        return None;
    }
    Some(match letter {
        Some(letter) => format!("{number}{letter}"),
        None => number.to_string(),
    })
}

/// A column's density over the sheet: each entry holds until the next, and consecutive rows with
/// the same drawing collapse into one run of increment 0.
fn rebuild(cells: &[Cell], duration: u32, level: u32) -> Vec<CellRun> {
    if duration == 0 {
        return Vec::new();
    }
    let mut entries: Vec<(u32, Option<FrameId>)> = cells
        .iter()
        .filter(|cell| cell.frame < duration)
        .map(|cell| (cell.frame, frame_of(cell.cell.as_deref())))
        .collect();
    entries.sort_by_key(|(frame, _)| *frame);
    // Two entries on one frame: the first wins, as OpenToonz reads them.
    entries.dedup_by_key(|(frame, _)| *frame);
    let mut runs = Vec::new();
    let mut row = 0u32;
    let mut current: Option<FrameId> = None;
    for (frame, value) in entries {
        if frame > row {
            push_run(&mut runs, row, frame - row, current, level);
            row = frame;
        }
        current = value;
    }
    if row < duration {
        push_run(&mut runs, row, duration - row, current, level);
    }
    runs
}

fn push_run(runs: &mut Vec<CellRun>, row: u32, count: u32, frame: Option<FrameId>, level: u32) {
    let Some(frame) = frame else {
        return;
    };
    if let Some(last) = runs.last_mut()
        && last.frame == frame
        && last.level == level
        && last.increment == 0
        && last.row + last.count == row
    {
        last.count += count;
        return;
    }
    runs.push(CellRun {
        row,
        count,
        level,
        frame,
        increment: 0,
    });
}

/// The frame a cell names, or none for the null cell and for a symbol this sheet does not map.
fn frame_of(cell: Option<&str>) -> Option<FrameId> {
    match cell {
        None | Some(NULL_CELL) => None,
        Some(text) => parse_cell_frame(text.trim()).ok(),
    }
}

fn read_duration(table: &Value) -> Result<u32, ToonzError> {
    let duration = table
        .get("duration")
        .and_then(Value::as_u64)
        .ok_or_else(|| ToonzError::malformed("xdts", "time table has no duration"))?;
    if duration > u64::from(MAX_XDTS_ROWS) {
        return Err(limit("timing sheet rows"));
    }
    Ok(duration as u32)
}

fn header_names(table: &Value) -> Vec<&str> {
    cell_field(table, "timeTableHeaders")
        .and_then(|header| header.get("names"))
        .and_then(Value::as_array)
        .map(|names| {
            names
                .iter()
                .map(|name| name.as_str().unwrap_or_default().trim())
                .collect()
        })
        .unwrap_or_default()
}

/// The field with `fieldId` 0 (the cell field), or the first one when the file numbers it
/// differently.
fn cell_field<'a>(table: &'a Value, key: &str) -> Option<&'a Value> {
    let fields = table.get(key)?.as_array()?;
    fields
        .iter()
        .find(|field| field.get("fieldId").and_then(Value::as_u64) == Some(0))
        .or_else(|| fields.first())
}

fn read_cells(track: &Value, budget: usize) -> Result<Vec<Cell>, ToonzError> {
    let frames = track
        .get("frames")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    if frames.len() > budget {
        return Err(limit("timing sheet cells"));
    }
    let mut cells = Vec::with_capacity(frames.len());
    for entry in frames {
        let frame = entry
            .get("frame")
            .and_then(Value::as_u64)
            .ok_or_else(|| ToonzError::malformed("xdts", "a frame entry has no frame number"))?;
        if frame > u64::from(MAX_XDTS_ROWS) {
            return Err(limit("timing sheet rows"));
        }
        let text = cell_text(entry)
            .ok_or_else(|| ToonzError::malformed("xdts", "a frame entry has no cell value"))?
            .trim();
        let cell = if text == NULL_CELL {
            None
        } else {
            parse_cell_frame(text)?;
            Some(text.to_owned())
        };
        cells.push(Cell {
            frame: frame as u32,
            cell,
        });
    }
    cells.sort_by_key(|cell| cell.frame);
    Ok(cells)
}

/// The cell text of one entry: this crate's `cell`, or the `data` array OpenToonz writes.
fn cell_text(entry: &Value) -> Option<&str> {
    if let Some(cell) = entry.get("cell").and_then(Value::as_str) {
        return Some(cell);
    }
    entry.get("data")?.get(0)?.get("values")?.get(0)?.as_str()
}

fn parse_cell_frame(text: &str) -> Result<FrameId, ToonzError> {
    parse_frame(text).map_err(|error| match error {
        ToonzError::Malformed { detail, .. } => ToonzError::malformed("xdts", detail),
        other => other,
    })
}

fn limit(what: &'static str) -> ToonzError {
    ToonzError::Limit {
        what,
        max: MAX_XDTS_ROWS as usize,
    }
}
