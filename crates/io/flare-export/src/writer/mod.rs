use flare_document::{Anchor, Document, Placement};
use std::collections::HashSet;
use std::path::PathBuf;
use thiserror::Error;

mod files;
mod shapes;
mod text;
mod timeline;
mod xml;

#[allow(unused_imports)]
use files::*;
pub use files::{fla_bytes, write_directory, write_fla};
#[allow(unused_imports)]
use shapes::*;
#[allow(unused_imports)]
use text::*;
#[allow(unused_imports)]
use timeline::*;
#[allow(unused_imports)]
use xml::{Xml, a};

/// Curve approximation tolerance, in pixels.
const CURVE_TOLERANCE_PX: f64 = 0.05;

const TWIPS_PER_PX: f64 = 20.0;

const NAMESPACE: &str = "http://ns.adobe.com/xfl/2008/";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Written faithfully but worth knowing about.
    Info,
    /// Written with a change in meaning.
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub location: String,
    pub message: String,
}

/// The files of an exported document, by path relative to its root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    /// Name used for the `.xfl` stub and, with `.fla`, the archive.
    pub name: String,
    pub files: Vec<(String, String)>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("{0} already exists")]
    AlreadyExists(PathBuf),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
}

/// Builds the files of an XFL document. Nothing is written to disk.
pub fn export_xfl(document: &Document) -> Result<Output, ExportError> {
    let mut notes: Vec<Diagnostic> = Vec::new();
    if !document.sounds().is_empty() {
        notes.push(Diagnostic {
            severity: Severity::Warning,
            location: "sounds".into(),
            message: format!(
                "{} sound(s) are not exported yet; the sound layers were written without them",
                document.sounds().len()
            ),
        });
    }

    // Library names must be unique and file names must be safe and distinct.
    let mut used_names: HashSet<String> = HashSet::new();
    let mut used_files: HashSet<String> = HashSet::new();
    let mut library = Vec::new();
    for symbol in document.symbols() {
        let base = if symbol.name().is_empty() {
            "Symbol"
        } else {
            symbol.name()
        };
        let mut name = base.to_owned();
        let mut n = 2;
        while !used_names.insert(name.clone()) {
            name = format!("{base} {n}");
            n += 1;
        }
        if name != base {
            notes.push(Diagnostic {
                severity: Severity::Warning,
                location: format!("symbol \"{base}\""),
                message: format!(
                    "the library needs unique names, so one copy was renamed \"{name}\""
                ),
            });
        }
        let stem = safe_stem(&name);
        let mut file = format!("{stem}.xml");
        let mut k = 2;
        while !used_files.insert(file.to_lowercase()) {
            file = format!("{stem} {k}.xml");
            k += 1;
        }
        library.push((symbol.id(), name, file));
    }
    let kinds = document
        .symbols()
        .iter()
        .map(|s| (s.id(), s.kind()))
        .collect();
    let names = Names { library, kinds };
    let mut ctx = Ctx {
        names: &names,
        notes: Vec::new(),
        approximated: false,
        attachments: document.attachments(),
        used: vec![false; document.attachments().len()],
    };

    let stage = document.stage();
    if stage.background().a != 255 {
        notes.push(Diagnostic {
            severity: Severity::Warning,
            location: "stage".into(),
            message: "the background's transparency cannot be stored; it was written opaque".into(),
        });
    }

    let mut main = Xml::new();
    main.open(
        "DOMDocument",
        [
            a("xmlns", NAMESPACE),
            a("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
            a("frameRate", frame_rate_text(document.frame_rate())),
            a("width", stage.width()),
            a("height", stage.height()),
            a("backgroundColor", hex(stage.background())),
            a("currentTimeline", 1),
            a("xflVersion", "2.2"),
        ],
    );
    if !names.library.is_empty() {
        main.open("symbols", []);
        for (_, _, file) in &names.library {
            main.empty("Include", [a("href", file)]);
        }
        main.close("symbols");
    }
    main.open("timelines", []);
    for scene in document.scenes() {
        write_timeline(
            &mut main,
            scene.name(),
            scene.timeline(),
            &format!("scene \"{}\"", scene.name()),
            Anchor::Scene(scene.id()),
            &mut ctx,
        );
    }
    main.close("timelines");
    for xml in ctx.take(Anchor::Document, |p| p == Placement::Child, "document") {
        main.raw(xml);
    }
    main.close("DOMDocument");
    main.raw("\n");

    let mut files = vec![("DOMDocument.xml".to_owned(), main.finish())];
    for (symbol, (_, name, file)) in document.symbols().iter().zip(&names.library) {
        let mut item = Xml::new();
        let mut attrs = vec![a("xmlns", NAMESPACE), a("name", name)];
        if let Some(kind) = xfl_symbol_type(symbol.kind()) {
            attrs.push(a("symbolType", kind));
        }
        item.open("DOMSymbolItem", attrs);
        item.open("timeline", []);
        write_timeline(
            &mut item,
            name,
            symbol.timeline(),
            &format!("symbol \"{name}\""),
            Anchor::Symbol(symbol.id()),
            &mut ctx,
        );
        item.close("timeline");
        for xml in ctx.take(
            Anchor::Symbol(symbol.id()),
            |p| p == Placement::Child,
            &format!("symbol \"{name}\""),
        ) {
            item.raw(xml);
        }
        item.close("DOMSymbolItem");
        item.raw("\n");
        files.push((format!("LIBRARY/{file}"), item.finish()));
    }

    // Source data whose keyframe no longer exists on its layer cannot be placed honestly.
    for (attachment, used) in ctx.attachments.iter().zip(&ctx.used) {
        if !used {
            ctx.notes.push(Diagnostic {
                severity: Severity::Warning,
                location: "source data".into(),
                message: format!(
                    "preserved source data ({:?}) could not be placed because its keyframe no longer exists; it was not written",
                    attachment.placement()
                ),
            });
        }
    }
    notes.append(&mut ctx.notes);
    if ctx.approximated {
        notes.push(Diagnostic {
            severity: Severity::Info,
            location: "shapes".into(),
            message: format!(
                "XFL edges cannot hold cubic curves, so they were approximated by quadratic segments within {CURVE_TOLERANCE_PX} px"
            ),
        });
    }
    let mut seen = HashSet::new();
    notes.retain(|d| seen.insert((d.severity as u8, d.location.clone(), d.message.clone())));
    let name = safe_stem(document.scenes().first().map_or("Document", |s| s.name()));
    Ok(Output {
        name,
        files,
        diagnostics: notes,
    })
}
