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

#[test]
fn a_graphics_frame_setting_is_read_and_an_odd_one_loops_with_a_note() {
    use flare_document::{Content, PlayMode, Playback};
    let main = |instance_attrs: &str| {
        format!(
            r#"<DOMDocument><symbols><Include href="Walk.xml"/></symbols><timelines><DOMTimeline name="S"><layers><DOMLayer name="L"><frames><DOMFrame index="0" duration="1"><elements><DOMSymbolInstance libraryItemName="Walk" symbolType="graphic"{instance_attrs}/></elements></DOMFrame></frames></DOMLayer></layers></DOMTimeline></timelines></DOMDocument>"#
        )
    };
    let import = |attrs: &str| {
        let files = Files::default().with("DOMDocument.xml", &main(attrs)).with(
            "LIBRARY/Walk.xml",
            r#"<DOMSymbolItem name="Walk" symbolType="graphic"/>"#,
        );
        import_xfl(&files).expect("import")
    };
    let playback = |imported: &flare_import::Imported| match imported.document.scenes()[0]
        .timeline()
        .layers()[0]
        .spans()[0]
        .content()
    {
        Content::Instance(i) => i.playback(),
        other => panic!("{other:?}"),
    };
    let once = import(r#" loop="play once" firstFrame="3""#);
    assert_eq!(
        playback(&once),
        Playback {
            mode: PlayMode::PlayOnce,
            first_frame: 3
        }
    );
    let single = import(r#" loop="single frame" firstFrame="9""#);
    assert_eq!(
        playback(&single),
        Playback {
            mode: PlayMode::SingleFrame,
            first_frame: 9
        }
    );
    // Animate's two reverse modes, by the names its own files use.
    let reverse = import(r#" loop="loop reverse" firstFrame="4""#);
    assert_eq!(
        playback(&reverse),
        Playback {
            mode: PlayMode::LoopReverse,
            first_frame: 4
        }
    );
    let once_reverse = import(r#" loop="play once reverse" firstFrame="2""#);
    assert_eq!(
        playback(&once_reverse),
        Playback {
            mode: PlayMode::PlayOnceReverse,
            first_frame: 2
        }
    );
    // Neither attribute: a loop from the first frame.
    assert_eq!(playback(&import("")), Playback::default());
    // Garbage in either: the default, and the odd mode is said.
    let odd = import(r#" loop="bounce" firstFrame="-4""#);
    assert_eq!(playback(&odd), Playback::default());
    assert!(
        messages(&odd, Severity::Info)
            .iter()
            .any(|m| m.contains("bounce")),
        "{:?}",
        odd.diagnostics
    );
    assert_eq!(
        playback(&import(r#" firstFrame="abc""#)),
        Playback::default()
    );
}
