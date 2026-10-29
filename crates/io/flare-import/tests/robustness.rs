//! Hostile and damaged input. Nothing here asserts a particular result for most inputs,
//! only that the importer returns one instead of panicking, hanging or exploding in size.
#![allow(clippy::expect_used)]

use std::collections::HashMap;

use flare_import::{FileSource, ImportError, import_xfl};

#[derive(Default, Clone)]
struct Files(HashMap<String, Vec<u8>>);

impl Files {
    fn with(mut self, name: &str, bytes: &[u8]) -> Self {
        self.0.insert(name.to_owned(), bytes.to_vec());
        self
    }
}

impl FileSource for Files {
    fn read(&self, name: &str) -> std::io::Result<Option<Vec<u8>>> {
        Ok(self.0.get(name).cloned())
    }
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
}

const MAIN: &str = r##"<DOMDocument frameRate="12" width="40" height="30" backgroundColor="#102030">
<symbols><Include href="Dot.xml"/></symbols>
<timelines><DOMTimeline name="Scene 1"><layers>
  <DOMLayer name="Main" visible="true"><frames>
    <DOMFrame index="0" duration="5" tweenType="motion"><elements>
      <DOMSymbolInstance libraryItemName="Dot"><matrix><Matrix tx="10" ty="20"/></matrix></DOMSymbolInstance>
      <DOMShape><fills><FillStyle index="1"><SolidColor color="#0000FF" alpha="0.5"/></FillStyle></fills>
        <strokes><StrokeStyle index="1"><SolidStroke weight="2"><fill><SolidColor color="#000000"/></fill></SolidStroke></StrokeStyle></strokes>
        <edges><Edge fillStyle1="1" strokeStyle="1" edges="!0 0|400 0[500 100 400 400|0 400|0 0"/></edges></DOMShape>
    </elements></DOMFrame>
    <DOMFrame index="5" duration="5"><elements/></DOMFrame>
  </frames></DOMLayer>
</layers></DOMTimeline></timelines></DOMDocument>"##;

const DOT: &str = r##"<DOMSymbolItem name="Dot" symbolType="graphic"><timeline><DOMTimeline name="Dot"><layers>
<DOMLayer name="L"><frames><DOMFrame index="0"><elements><DOMShape>
  <fills><FillStyle index="1"><SolidColor color="#FF0000"/></FillStyle></fills>
  <edges><Edge fillStyle1="1" edges="!0 0|200 0|200 200|0 200|0 0"/></edges>
</DOMShape></elements></DOMFrame></frames></DOMLayer></layers></DOMTimeline></timeline></DOMSymbolItem>"##;

fn valid() -> Files {
    Files::default()
        .with("DOMDocument.xml", MAIN.as_bytes())
        .with("LIBRARY/Dot.xml", DOT.as_bytes())
}

#[test]
fn the_unmodified_fixture_imports() {
    import_xfl(&valid())
        .expect("the baseline must be valid for the mutation tests to mean anything");
}

#[test]
fn random_byte_mutations_of_a_valid_document_never_panic() {
    let mut rng = Rng(0xF1A4E);
    let (mut ok, mut err) = (0, 0);
    for _ in 0..1_500 {
        let mut files = valid();
        let target = if rng.next().is_multiple_of(2) {
            "DOMDocument.xml"
        } else {
            "LIBRARY/Dot.xml"
        };
        let mut bytes = files.0[target].clone();
        for _ in 0..(1 + rng.next() % 4) {
            let at = (rng.next() as usize) % bytes.len();
            bytes[at] = match rng.next() % 4 {
                0 => b'<',
                1 => b'"',
                2 => (rng.next() % 256) as u8,
                _ => b'9',
            };
        }
        files.0.insert(target.to_owned(), bytes);
        match import_xfl(&files) {
            Ok(imported) => {
                ok += 1;
                // Anything accepted is a valid document with a bounded stage.
                assert!(imported.document.stage().width() <= flare_document::MAX_STAGE_SIDE);
            }
            Err(_) => err += 1,
        }
    }
    assert!(
        ok > 50 && err > 50,
        "mutations hit both outcomes ({ok} imported, {err} refused)"
    );
}

#[test]
fn truncations_and_pure_garbage_are_refused_cleanly() {
    let mut rng = Rng(9);
    for cut in (0..MAIN.len()).step_by(37) {
        let files = Files::default().with("DOMDocument.xml", &MAIN.as_bytes()[..cut]);
        assert!(
            import_xfl(&files).is_err(),
            "a truncated document is not a document (cut {cut})"
        );
    }
    for _ in 0..300 {
        let len = 1 + (rng.next() as usize) % 400;
        let garbage: Vec<u8> = (0..len).map(|_| (rng.next() % 256) as u8).collect();
        let files = Files::default().with("DOMDocument.xml", &garbage);
        assert!(import_xfl(&files).is_err());
    }
    let nul = Files::default().with("DOMDocument.xml", b"<DOMDocument\0/>");
    assert!(import_xfl(&nul).is_err());
}

#[test]
fn pathological_but_wellformed_documents_stay_bounded() {
    // Deeply nested groups: depth is capped by the XML parser, so this is refused, not recursed.
    let mut deep = String::from(
        "<DOMDocument><timelines><DOMTimeline name=\"S\"><layers><DOMLayer name=\"L\"><frames><DOMFrame index=\"0\"><elements>",
    );
    deep.push_str(&"<DOMGroup><members>".repeat(60));
    deep.push_str(&"</members></DOMGroup>".repeat(60));
    deep.push_str("</elements></DOMFrame></frames></DOMLayer></layers></DOMTimeline></timelines></DOMDocument>");
    let refused = import_xfl(&Files::default().with("DOMDocument.xml", deep.as_bytes()));
    assert!(
        matches!(refused, Err(ImportError::Xml { .. })),
        "depth beyond the cap is rejected"
    );

    // Many frames with huge indices and durations: arithmetic must not overflow.
    let mut frames = String::new();
    for i in 0..200u32 {
        frames.push_str(&format!(
            "<DOMFrame index=\"{}\" duration=\"4294967295\"><elements/></DOMFrame>",
            i.wrapping_mul(2_000_000_000)
        ));
    }
    let xml = format!(
        "<DOMDocument><timelines><DOMTimeline name=\"S\"><layers><DOMLayer name=\"L\"><frames>{frames}</frames></DOMLayer></layers></DOMTimeline></timelines></DOMDocument>"
    );
    let _ = import_xfl(&Files::default().with("DOMDocument.xml", xml.as_bytes()));

    // A shape with a very large edge list imports in reasonable time.
    let mut edges = String::from("!0 0");
    for i in 1..=50_000_u32 {
        edges.push_str(&format!("|{} {}", (i % 400) * 10, (i / 400) * 10));
    }
    edges.push_str("|0 0");
    let shape = format!(
        "<DOMDocument><timelines><DOMTimeline name=\"S\"><layers><DOMLayer name=\"L\"><frames><DOMFrame index=\"0\"><elements><DOMShape>\
         <fills><FillStyle index=\"1\"><SolidColor color=\"#000000\"/></FillStyle></fills>\
         <edges><Edge fillStyle1=\"1\" edges=\"{edges}\"/></edges></DOMShape></elements></DOMFrame></frames></DOMLayer></layers></DOMTimeline></timelines></DOMDocument>"
    );
    import_xfl(&Files::default().with("DOMDocument.xml", shape.as_bytes()))
        .expect("a large shape imports");
}

#[test]
fn diagnostics_are_capped_so_a_hostile_file_cannot_flood_memory() {
    let mut layers = String::new();
    for i in 0..5_000 {
        layers.push_str(&format!(
            "<DOMLayer name=\"L{i}\" layerType=\"mask\"><frames/></DOMLayer>"
        ));
    }
    let xml = format!(
        "<DOMDocument><timelines><DOMTimeline name=\"S\"><layers>{layers}</layers></DOMTimeline></timelines></DOMDocument>"
    );
    let imported =
        import_xfl(&Files::default().with("DOMDocument.xml", xml.as_bytes())).expect("import");
    assert!(
        imported.diagnostics.len() <= 1_001,
        "{} diagnostics",
        imported.diagnostics.len()
    );
    assert!(
        imported
            .diagnostics
            .last()
            .is_some_and(|d| d.message.contains("suppressed"))
    );
}

#[test]
fn a_layer_color_that_is_not_plain_hex_is_ignored_not_a_panic() {
    // Six bytes, but not six characters: slicing it by byte would split a character.
    let main = MAIN.replacen(
        "<DOMLayer name=\"Main\"",
        "<DOMLayer color=\"#\u{e9}\u{e9}\u{e9}\" name=\"Main\"",
        1,
    );
    let files = valid().with("DOMDocument.xml", main.as_bytes());
    let imported = import_xfl(&files).expect("still imports");
    let layers = imported.document.scenes()[0].timeline().layers();
    assert!(layers.iter().all(|l| l.outline().is_none()));
}
