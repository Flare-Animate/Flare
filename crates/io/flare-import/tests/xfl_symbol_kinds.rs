//! The kind of each symbol (graphic, movie clip, button) as XFL names it.
#![allow(clippy::expect_used)]

use std::collections::HashMap;

use flare_import::{FileSource, Severity, import_xfl};

#[derive(Default)]
struct Files(HashMap<String, String>);

impl Files {
    fn with(mut self, name: &str, xml: &str) -> Self {
        self.0.insert(name.to_owned(), xml.to_owned());
        self
    }
}

impl FileSource for Files {
    fn read(&self, name: &str) -> std::io::Result<Option<Vec<u8>>> {
        Ok(self.0.get(name).map(|s| s.clone().into_bytes()))
    }
}

fn messages(imported: &flare_import::Imported, severity: Severity) -> Vec<String> {
    imported
        .diagnostics
        .iter()
        .filter(|d| d.severity == severity)
        .map(|d| d.message.clone())
        .collect()
}

#[test]
fn symbols_keep_their_kind_and_the_behaviour_that_is_not_carried_out_is_reported() {
    use flare_document::SymbolKind;
    let doc = r#"<DOMDocument><symbols><Include href="S.xml"/></symbols><timelines><DOMTimeline name="S"/></timelines></DOMDocument>"#;
    // A movie clip has no symbolType in Animate's files, so none is a movie clip too.
    for (attr, kind, expected) in [
        (r#" symbolType="graphic""#, SymbolKind::Graphic, ""),
        (r#" symbolType="button""#, SymbolKind::Button, "button"),
        (
            r#" symbolType="movie clip""#,
            SymbolKind::MovieClip,
            "movie clip",
        ),
        ("", SymbolKind::MovieClip, "movie clip"),
    ] {
        let item = format!(r#"<DOMSymbolItem name="S"{attr}/>"#);
        let files = Files::default()
            .with("DOMDocument.xml", doc)
            .with("LIBRARY/S.xml", &item);
        let imported = import_xfl(&files).expect("import");
        assert_eq!(imported.document.symbols().len(), 1);
        assert_eq!(imported.document.symbols()[0].kind(), kind, "{attr}");
        let warnings = messages(&imported, Severity::Warning).join("\n");
        let said = warnings.contains(expected) && expected.is_empty() == warnings.is_empty();
        assert!(
            said,
            "{attr}: only a button or movie clip warns: {warnings}"
        );
    }
}
