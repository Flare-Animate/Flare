//! Reading a whole XFL project.

use crate::xml::{self, Node};
use flare_core::{DocumentId, Rgba8, SceneId, SymbolId};
use flare_document::{
    Anchor, Document, MAX_ATTACHMENTS, Placement, Scene, Stage, Symbol, Timeline,
};
use std::collections::HashMap;

use super::*;

/// Imports an XFL document from `source`.
pub fn import_xfl(source: &dyn FileSource) -> Result<Imported, ImportError> {
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

    let mut importer = Importer {
        symbols: HashMap::new(),
        diagnostics: Vec::new(),
        suppressed: 0,
        kept: Vec::new(),
    };

    // Pass 1: read every library item and assign IDs, so symbols can refer to each other.
    let mut items: Vec<(String, SymbolId, Node)> = Vec::new();
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
        items.push((name, id, item));
    }

    // Pass 2: build the symbols' timelines.
    let mut symbols = Vec::with_capacity(items.len());
    for (name, id, item) in &items {
        let place = format!("symbol \"{name}\"");
        match item.attr("symbolType") {
            Some("graphic") => {}
            Some("button") => importer.note(Severity::Warning, &place, "a button was imported as a Graphic symbol; its states and events are not preserved"),
            _ => importer.note(Severity::Warning, &place, "a movie clip was imported as a Graphic symbol; its independent clock and scripts are not preserved"),
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
        symbols.push(Symbol::new(*id, name.clone(), timeline));
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
    Ok(Imported {
        document,
        diagnostics: importer.diagnostics,
    })
}
