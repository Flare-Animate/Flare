//! Toon Boom Harmony scenes (`.xstage`), read as a Flare project: the stage resolution, the
//! frame rate and the scene's columns as layers. A Harmony scene is an XML project that
//! references the drawings, palettes and sounds stored beside it in the scene's folder; those
//! files are not read, so every column becomes a layer holding one empty frame and a note says
//! so. The element names follow the Harmony 21 scene layout the `harmonyparser` project
//! documents (MIT); see [docs/INDUSTRY-FORMATS.md](../../../../docs/INDUSTRY-FORMATS.md).

use std::num::NonZeroU32;

use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId};
use flare_document::{Content, Document, FrameSpan, Layer, Scene, Stage, Timeline};
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::xfl::{Diagnostic, ImportError, Imported, Severity};

/// Most bytes of one scene that are read; a real scene is a few hundred kilobytes.
const MAX_BYTES: usize = 64 * 1024 * 1024;
/// Most XML elements one scene may hold, so a hostile file cannot exhaust memory.
const MAX_NODES: usize = 500_000;
/// Most columns one scene may hold.
const MAX_COLUMNS: usize = 1_000;
/// Most frames one column may hold.
const MAX_FRAMES: u32 = 1_000_000;
/// The longest column name kept, in characters.
const MAX_NAME_CHARS: usize = 64;

#[derive(Debug, thiserror::Error)]
enum HarmonyError {
    #[error("the scene is not XML: {0}")]
    Xml(String),
    #[error("the scene has more than {MAX_NODES} elements")]
    TooManyNodes,
    #[error("the scene has more than {MAX_COLUMNS} columns")]
    TooManyColumns,
    #[error("the scene has more than {MAX_FRAMES} frames")]
    TooManyFrames,
    #[error("the scene has no stage resolution")]
    NoResolution,
    #[error("the scene's resolution {0:?} is not a width and a height")]
    BadResolution(String),
    #[error("the scene has no frame rate")]
    NoFrameRate,
    #[error("the scene has no frame count")]
    NoFrames,
}

/// Whether `bytes` look like a Harmony scene: the XML root is a project written by Harmony,
/// or an older scene whose root element is `<xstage>`.
pub fn looks_like_xstage(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(64 * 1024)];
    let text = String::from_utf8_lossy(head).to_ascii_lowercase();
    text.contains("<xstage") || (text.contains("<project") && text.contains("harmony"))
}

/// What the scan of a scene found, before it becomes a document.
#[derive(Default)]
struct Found {
    size: Option<(u32, u32)>,
    fps: Option<f64>,
    frames: Option<u32>,
    scene_name: Option<String>,
    columns: Vec<Column>,
}

struct Column {
    name: String,
    order: i32,
}

/// Imports the scene `bytes`, named `file`.
pub fn import(file: &str, bytes: &[u8]) -> Result<Imported, ImportError> {
    if bytes.len() > MAX_BYTES {
        return Err(ImportError::TooLarge(file.to_owned()));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| foreign(file, "not UTF-8 XML"))?;
    let found = scan(text).map_err(|e| foreign(file, e))?;
    build(file, found)
}

fn foreign(file: &str, e: impl std::fmt::Display) -> ImportError {
    ImportError::Foreign(format!("{file}: {e}"))
}

fn scan(text: &str) -> Result<Found, HarmonyError> {
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut found = Found::default();
    let mut stack: Vec<String> = Vec::new();
    let mut nodes = 0usize;
    let mut scene_seen = false;
    loop {
        match reader
            .read_event()
            .map_err(|e| HarmonyError::Xml(e.to_string()))?
        {
            Event::Start(start) => {
                stack.push(local_name(&start));
                nodes += 1;
                if nodes > MAX_NODES {
                    return Err(HarmonyError::TooManyNodes);
                }
                inspect(&stack, &start, &mut found, &mut scene_seen)?;
            }
            Event::Empty(start) => {
                stack.push(local_name(&start));
                nodes += 1;
                if nodes > MAX_NODES {
                    return Err(HarmonyError::TooManyNodes);
                }
                inspect(&stack, &start, &mut found, &mut scene_seen)?;
                stack.pop();
            }
            Event::End(_) => {
                stack.pop();
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(found)
}

/// Reads the elements that carry the scene's settings. Only the elements in their Harmony
/// place are taken: a `resolution` inside a nested `<options>` is a node's, not the stage's.
fn inspect(
    stack: &[String],
    element: &BytesStart<'_>,
    found: &mut Found,
    scene_seen: &mut bool,
) -> Result<(), HarmonyError> {
    let Some(name) = stack.last().map(String::as_str) else {
        return Ok(());
    };
    let parent = stack
        .len()
        .checked_sub(2)
        .and_then(|index| stack.get(index))
        .map(String::as_str);
    let under_project = stack.first().map(String::as_str) == Some("project");
    match (name, parent) {
        ("resolution", Some("options")) if under_project && stack.len() == 3 => {
            if let Some(size) = attr(element, "size") {
                found.size = Some(parse_size(&size)?);
            } else if let (Some(width), Some(height)) =
                (attr(element, "resX"), attr(element, "resY"))
            {
                found.size = Some(parse_size(&format!("{width},{height}"))?);
            }
        }
        ("framerate", Some("options")) if under_project && stack.len() == 3 => {
            if let Some(value) = attr(element, "val") {
                found.fps = value.trim().parse().ok();
            }
        }
        ("scene", Some("scenes")) if !*scene_seen => {
            *scene_seen = true;
            found.scene_name = attr(element, "name");
            found.frames = attr(element, "nbframes")
                .and_then(|value| value.trim().parse().ok())
                .or_else(|| frame_span(element));
        }
        ("column", Some("columns")) => {
            if found.columns.len() >= MAX_COLUMNS {
                return Err(HarmonyError::TooManyColumns);
            }
            let order = attr(element, "displayOrder")
                .and_then(|value| value.trim().parse().ok())
                .unwrap_or(found.columns.len() as i32);
            found.columns.push(Column {
                name: attr(element, "name").unwrap_or_default(),
                order,
            });
        }
        _ => {}
    }
    Ok(())
}

/// The frame count a scene's `startFrame` and `stopFrame` describe, when it has no `nbframes`.
fn frame_span(element: &BytesStart<'_>) -> Option<u32> {
    let start = attr(element, "startFrame")?.trim().parse::<u32>().ok()?;
    let stop = attr(element, "stopFrame")?.trim().parse::<u32>().ok()?;
    stop.checked_sub(start)
        .map(|length| length.saturating_add(1))
}

fn parse_size(text: &str) -> Result<(u32, u32), HarmonyError> {
    let mut parts = text
        .split([',', 'x', 'X', ' '])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| part.parse::<u32>().ok());
    match (parts.next(), parts.next(), parts.next()) {
        (Some(Some(width)), Some(Some(height)), None) if width > 0 && height > 0 => {
            Ok((width, height))
        }
        _ => Err(HarmonyError::BadResolution(text.to_owned())),
    }
}

fn local_name(element: &BytesStart<'_>) -> String {
    let name = element.name();
    let raw: &str = name.as_ref();
    raw.rsplit(':').next().unwrap_or(raw).to_owned()
}

fn attr(element: &BytesStart<'_>, key: &str) -> Option<String> {
    for attribute in element.attributes().flatten() {
        let name: &str = attribute.key.as_ref();
        let local = name.rsplit(':').next().unwrap_or(name);
        if !local.eq_ignore_ascii_case(key) {
            continue;
        }
        let raw: &str = &attribute.value;
        return Some(
            quick_xml::escape::unescape(raw)
                .map_or_else(|_| raw.to_owned(), |text| text.into_owned()),
        );
    }
    None
}

fn build(file: &str, found: Found) -> Result<Imported, ImportError> {
    let (width, height) = found
        .size
        .ok_or_else(|| foreign(file, HarmonyError::NoResolution))?;
    let fps = found
        .fps
        .ok_or_else(|| foreign(file, HarmonyError::NoFrameRate))?;
    let rate = FrameRate::from_per_second(fps).map_err(|e| foreign(file, e))?;
    let frames = found
        .frames
        .filter(|frames| *frames > 0)
        .ok_or_else(|| foreign(file, HarmonyError::NoFrames))?;
    if frames > MAX_FRAMES {
        return Err(foreign(file, HarmonyError::TooManyFrames));
    }
    let duration = NonZeroU32::new(frames).ok_or_else(|| foreign(file, HarmonyError::NoFrames))?;

    let mut columns = found.columns;
    columns.sort_by_key(|column| column.order);
    let column_count = columns.len();
    let mut layers = Vec::with_capacity(column_count.max(1));
    if columns.is_empty() {
        layers.push(empty_layer("Layer 1", duration, file)?);
    } else {
        for (index, column) in columns.iter().enumerate() {
            let name = column.name.trim();
            let name = if name.is_empty() {
                format!("Layer {}", index + 1)
            } else {
                name.chars().take(MAX_NAME_CHARS).collect()
            };
            layers.push(empty_layer(&name, duration, file)?);
        }
    }

    let scene_name = found
        .scene_name
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "Scene".to_owned());
    let scene = Scene::new(SceneId::new(), scene_name, Timeline::new(layers));
    let stage = Stage::new(width, height, Rgba8::WHITE).map_err(|e| foreign(file, e))?;
    let document = Document::new(DocumentId::new(), rate, vec![scene], Vec::new())
        .map_err(ImportError::Invalid)?
        .with_stage(stage);

    let diagnostics = vec![Diagnostic {
        severity: Severity::Unsupported,
        location: file.to_owned(),
        message: format!(
            "the scene's drawings, palettes and sounds stay in its Harmony folder and are not \
             read; {column_count} column(s) became layers with one empty frame each"
        ),
    }];
    Ok(Imported {
        document,
        diagnostics,
    })
}

fn empty_layer(name: &str, duration: NonZeroU32, file: &str) -> Result<Layer, ImportError> {
    let span = FrameSpan::new(0, duration, Content::Empty);
    Layer::new(LayerId::new(), name, vec![span]).map_err(|e| foreign(file, e))
}
