//! Reading a whole XFL project.

use crate::xml::{self, Node};
use flare_core::{DocumentId, Rgba8, SceneId, SymbolId};
use flare_document::{
    Anchor, Bitmap, Document, MAX_ATTACHMENTS, MAX_BITMAPS, Placement, Scene, Stage, Symbol,
    SymbolKind, Timeline,
};

use super::*;

const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
const JPEG_SIGNATURE: &[u8] = &[0xFF, 0xD8, 0xFF];

/// Imports an XFL document from `source`.
pub fn import_xfl(source: &dyn FileSource) -> Result<Imported, ImportError> {
    import_xfl_with_files(source).map(|(imported, _)| imported)
}

/// The library file each imported symbol was read from, like `LIBRARY/Symbol 1.xml`.
pub(crate) type SymbolFiles = Vec<(SymbolId, String)>;

/// [`import_xfl`], and the file each symbol came from, which the kept source needs.
pub(crate) fn import_xfl_with_files(
    source: &dyn FileSource,
) -> Result<(Imported, SymbolFiles), ImportError> {
    let read = |name: &str| -> Result<Option<Node>, ImportError> {
        let Some(bytes) = source.read(name)? else {
            return Ok(None);
        };
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err(ImportError::TooLarge(name.to_owned()));
        }
        xml::parse(&bytes)
            .map(Some)
            .map_err(|error| ImportError::Xml {
                file: name.to_owned(),
                error,
            })
    };
    let root = read("DOMDocument.xml")?
        .ok_or_else(|| ImportError::NotXfl("DOMDocument.xml was not found".into()))?;
    if root.name != "DOMDocument" {
        return Err(ImportError::NotXfl(format!(
            "the root element is <{}>, expected <DOMDocument>",
            root.name
        )));
    }

    let mut importer = Importer::new();
    importer.frame_rate = root
        .attr("frameRate")
        .and_then(|r| r.trim().parse::<f64>().ok())
        .filter(|r| r.is_finite() && *r > 0.0 && *r <= 1000.0)
        .unwrap_or(24.0);

    // Pass 1: read every library item and assign IDs, so symbols can refer to each other.
    let mut items: Vec<(String, SymbolId, Node)> = Vec::new();
    let mut symbol_files: SymbolFiles = Vec::new();
    let includes: Vec<&Node> = root
        .child("symbols")
        .into_iter()
        .flat_map(|s| s.children_named("Include"))
        .collect();
    if includes.len() > MAX_SYMBOLS {
        return Err(ImportError::TooManySymbols);
    }
    for include in includes {
        let href = include.attr("href").unwrap_or_default();
        let file = format!("LIBRARY/{href}");
        let Some(item) = read(&file)? else {
            importer.note(
                Severity::Unsupported,
                &file,
                "library item not found; instances of it will be skipped",
            );
            continue;
        };
        if item.name != "DOMSymbolItem" {
            importer.note(
                Severity::Unsupported,
                &file,
                format!("unexpected <{}> in the library", item.name),
            );
            continue;
        }
        let Some(name) = item.attr("name").map(str::to_owned) else {
            importer.note(
                Severity::Unsupported,
                &file,
                "library item without a name was skipped",
            );
            continue;
        };
        let id = SymbolId::new();
        importer.symbols.insert(name.clone(), id);
        symbol_files.push((id, file));
        items.push((name, id, item));
    }

    // Pass 2: build the symbols' timelines.
    let mut symbols = Vec::with_capacity(items.len());
    for (name, id, item) in &items {
        let place = format!("symbol \"{name}\"");
        // XFL leaves the type out for a movie clip.
        let kind = match item.attr("symbolType") {
            Some("graphic") => SymbolKind::Graphic,
            Some("button") => SymbolKind::Button,
            _ => SymbolKind::MovieClip,
        };
        match kind {
            SymbolKind::Graphic => {}
            SymbolKind::Button => importer.note(Severity::Warning, &place, "a button keeps its type, but its up, over, down and hit states and its events are not carried out"),
            SymbolKind::MovieClip => importer.note(Severity::Warning, &place, "a movie clip keeps its type, but its own clock and its scripts are not carried out"),
        }
        let timeline = item
            .child("timeline")
            .and_then(|t| t.child("DOMTimeline"))
            .map_or_else(Timeline::default, |t| {
                importer.timeline(t, &place, Anchor::Symbol(*id))
            });
        // Anything in the symbol item besides its timeline is kept as source data.
        for child in item.children.iter().filter(|c| c.name != "timeline") {
            let note = format!("{place} / <{}>", child.name);
            importer.keep(Anchor::Symbol(*id), Placement::Child, child.to_xml(), &note);
        }
        symbols.push(Symbol::new(*id, name.clone(), timeline).with_kind(kind));
    }

    // Library bitmaps: the media list names each image and its file, which sits in LIBRARY/.
    // PNG and JPEG are decoded; other formats are reported and their instances kept as source data.
    let mut bitmaps = Vec::new();
    for item in root
        .child("media")
        .into_iter()
        .flat_map(|m| m.children_named("DOMBitmapItem"))
    {
        let Some(name) = item.attr("name").map(str::to_owned) else {
            continue;
        };
        let place = format!("bitmap \"{name}\"");
        if bitmaps.len() >= MAX_BITMAPS {
            importer.note(
                Severity::Unsupported,
                &place,
                "the limit of bitmaps was reached",
            );
            break;
        }
        // Animate keeps a bitmap's pixels in `bin/` under `bitmapDataHRef`; `href` names the
        // library entry, and an image file written by hand may sit under `LIBRARY/`.
        let data_href = item.attr("bitmapDataHRef").map(|h| format!("bin/{h}"));
        let file = match (&data_href, item.attr("href")) {
            (Some(path), _) => path.clone(),
            (None, Some(href)) => format!("LIBRARY/{href}"),
            // A media entry naming no file at all is kept as source data, not a failed import.
            (None, None) => {
                importer.note(Severity::Unsupported, &place, "image file not found");
                continue;
            }
        };
        let found = match source.read(&file)? {
            Some(bytes) => Some(bytes),
            None => match item.attr("href") {
                Some(href) if data_href.is_some() => source.read(&format!("LIBRARY/{href}"))?,
                _ => None,
            },
        };
        let Some(bytes) = found else {
            importer.note(Severity::Unsupported, &place, "image file not found");
            continue;
        };
        if bytes.len() as u64 > MAX_FILE_BYTES {
            importer.note(Severity::Unsupported, &place, "image file is too large");
            continue;
        }
        let decoded = if bytes.starts_with(PNG_SIGNATURE) {
            crate::bitmap::decode_png(&file, &bytes)
        } else if bytes.starts_with(JPEG_SIGNATURE) {
            crate::bitmap::decode_jpeg(&file, &bytes)
        } else if bytes.starts_with(&crate::flash_bitmap::SIGNATURE) {
            // Animate's own lossless payload from `bin/`.
            crate::flash_bitmap::decode(&bytes)
                .map_err(|e| ImportError::Foreign(format!("{file}: {e}")))
        } else {
            importer.note(
                Severity::Unsupported,
                &place,
                "only PNG and JPEG bitmaps are imported; this image was kept as source data",
            );
            continue;
        };
        let picture = match decoded {
            Ok(picture) => picture,
            Err(error) => {
                importer.note(
                    Severity::Unsupported,
                    &place,
                    format!("could not be read: {error}"),
                );
                continue;
            }
        };
        let id = flare_core::BitmapId::new();
        match Bitmap::new(
            id,
            name.clone(),
            picture.width,
            picture.height,
            picture.rgba,
        ) {
            Ok(bitmap) => {
                importer.bitmaps.insert(name, id);
                bitmaps.push(bitmap);
            }
            Err(error) => importer.note(Severity::Unsupported, &place, format!("{error}")),
        }
    }

    let mut scenes = Vec::new();
    for node in root
        .child("timelines")
        .into_iter()
        .flat_map(|t| t.children_named("DOMTimeline"))
    {
        let name = node.attr("name").unwrap_or("Scene").to_owned();
        let scene_id = SceneId::new();
        let timeline =
            importer.timeline(node, &format!("scene \"{name}\""), Anchor::Scene(scene_id));
        scenes.push(Scene::new(scene_id, name, timeline));
    }
    if scenes.is_empty() {
        return Err(ImportError::NoTimelines);
    }

    let rate = frame_rate(root.attr("frameRate").unwrap_or("24")).map_err(|message| {
        ImportError::BadAttribute {
            what: "frameRate".into(),
            message,
        }
    })?;
    let background = root
        .attr("backgroundColor")
        .and_then(|c| color_from(c, None))
        .unwrap_or(Rgba8::WHITE);
    let stage = Stage::new(
        stage_side(&root, "width", 550)?,
        stage_side(&root, "height", 400)?,
        background,
    )
    .map_err(|e| ImportError::BadAttribute {
        what: "stage".into(),
        message: e.to_string(),
    })?;

    // Document-level data Flare has no place for (media, fonts, publish settings, ...).
    for child in root
        .children
        .iter()
        .filter(|c| !matches!(c.name.as_str(), "symbols" | "timelines"))
    {
        let note = format!("document / <{}>", child.name);
        importer.keep(Anchor::Document, Placement::Child, child.to_xml(), &note);
    }
    let document = Document::new(DocumentId::new(), rate, scenes, symbols)
        .map_err(ImportError::Invalid)?
        .with_stage(stage);
    let document = if bitmaps.is_empty() {
        document
    } else {
        document
            .with_bitmaps(bitmaps)
            .map_err(ImportError::Invalid)?
    };

    // Keep only what still hangs off something that was imported (a layer skipped for
    // overlapping frames takes its source data with it), and stay within the limits.
    let wanted = importer.kept.len();
    let mut kept = std::mem::take(&mut importer.kept);
    if kept.len() > MAX_ATTACHMENTS {
        kept.truncate(MAX_ATTACHMENTS);
    }
    let document = document.with_attachments_retained(&kept);
    let retained = document.attachments().len();
    if retained < wanted {
        importer.note(
            Severity::Warning,
            "import",
            format!(
                "{} piece(s) of source data could not be kept (their layer was skipped or the limit of {MAX_ATTACHMENTS} was reached)",
                wanted - retained
            ),
        );
    }
    if retained > 0 {
        importer.note(
            Severity::Info,
            "import",
            format!(
                "{retained} piece(s) of source data that Flare cannot edit were kept and will be written back on export"
            ),
        );
    }
    if importer.suppressed > 0 {
        let message = format!(
            "{} further diagnostics were suppressed",
            importer.suppressed
        );
        importer.diagnostics.push(Diagnostic {
            severity: Severity::Info,
            location: "import".into(),
            message,
        });
    }
    Ok((
        Imported {
            document,
            diagnostics: importer.diagnostics,
        },
        symbol_files,
    ))
}
