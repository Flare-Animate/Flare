#![allow(clippy::expect_used)]

use std::collections::HashMap;
use std::io::Write;

use flare_core::Rgba8;
use flare_document::{Content, Document, Tween};
use flare_import::{
    DirSource, FileSource, ImportError, Severity, ZipSource, frame_rate, import_path, import_xfl,
};

/// An in-memory XFL document.
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

const DOT: &str = r##"<DOMSymbolItem name="Dot" symbolType="graphic"><timeline><DOMTimeline name="Dot"><layers>
<DOMLayer name="L"><frames><DOMFrame index="0"><elements><DOMShape>
  <fills><FillStyle index="1"><SolidColor color="#FF0000"/></FillStyle></fills>
  <edges><Edge fillStyle1="1" edges="!0 0|200 0|200 200|0 200|0 0"/></edges>
</DOMShape></elements></DOMFrame></frames></DOMLayer></layers></DOMTimeline></timeline></DOMSymbolItem>"##;

const MAIN: &str = r##"<DOMDocument frameRate="12" width="400" height="300" backgroundColor="#102030">
<symbols><Include href="Dot.xml"/></symbols>
<timelines><DOMTimeline name="Scene 1"><layers>
  <DOMLayer name="Main" visible="true" locked="true"><frames>
    <DOMFrame index="0" duration="5" tweenType="motion"><elements>
      <DOMSymbolInstance libraryItemName="Dot"><matrix><Matrix tx="10" ty="20"/></matrix></DOMSymbolInstance>
    </elements></DOMFrame>
    <DOMFrame index="5" duration="5"><elements>
      <DOMSymbolInstance libraryItemName="Dot"><matrix><Matrix tx="110" ty="20"/></matrix></DOMSymbolInstance>
    </elements></DOMFrame>
  </frames></DOMLayer>
  <DOMLayer name="Art"><frames><DOMFrame index="0" duration="10"><elements>
    <DOMShape>
      <fills><FillStyle index="1"><SolidColor color="#0000FF" alpha="0.5"/></FillStyle></fills>
      <strokes><StrokeStyle index="1"><SolidStroke weight="2"><fill><SolidColor color="#000000"/></fill></SolidStroke></StrokeStyle></strokes>
      <edges><Edge fillStyle1="1" strokeStyle="1" edges="!4000 2000|4800 2000|4800 2400|4000 2400|4000 2000"/></edges>
    </DOMShape>
    <DOMShape>
      <fills><FillStyle index="1"><SolidColor color="#FFFF00"/></FillStyle></fills>
      <edges><Edge fillStyle1="1" edges="!6000 5000|6200 5000|6200 5200|6000 5200|6000 5000"/></edges>
    </DOMShape>
    <DOMStaticText/>
  </elements></DOMFrame></frames></DOMLayer>
  <DOMLayer name="Cut" layerType="mask"><frames/></DOMLayer>
  <DOMLayer name="Folder" layerType="folder"><frames/></DOMLayer>
</layers></DOMTimeline></timelines></DOMDocument>"##;

fn sample() -> Files {
    Files::default()
        .with("DOMDocument.xml", MAIN)
        .with("LIBRARY/Dot.xml", DOT)
}

fn messages(imported: &flare_import::Imported, severity: Severity) -> Vec<String> {
    imported
        .diagnostics
        .iter()
        .filter(|d| d.severity == severity)
        .map(|d| format!("{}: {}", d.location, d.message))
        .collect()
}

fn pixel(document: &Document, frame: u32, x: u32, y: u32) -> Rgba8 {
    let scene = document.scenes()[0].id();
    let evaluated = flare_eval::evaluate_frame(document, scene, frame).expect("eval");
    let stage = document.stage();
    let image = flare_render::render(
        &evaluated,
        stage.width(),
        stage.height(),
        stage.background(),
    )
    .expect("render");
    image.pixel(x, y).expect("pixel")
}

#[test]
fn imports_stage_scenes_layers_symbols_and_tweens() {
    let imported = import_xfl(&sample()).expect("import");
    let doc = &imported.document;
    let stage = doc.stage();
    assert_eq!(
        (stage.width(), stage.height(), stage.background()),
        (400, 300, Rgba8::opaque(0x10, 0x20, 0x30))
    );
    assert_eq!(
        (doc.frame_rate().numerator(), doc.frame_rate().denominator()),
        (12, 1)
    );
    assert_eq!(doc.symbols().len(), 1);
    assert_eq!(doc.symbols()[0].name(), "Dot");

    let layers = doc.scenes()[0].timeline().layers();
    let names: Vec<&str> = layers.iter().map(|l| l.name()).collect();
    assert_eq!(
        names,
        vec!["Main", "Art (2)", "Art", "Folder"],
        "mask layers are skipped, folders are kept; a second shape in a keyframe goes on a layer above"
    );
    assert!(layers[0].is_locked() && layers[0].is_visible());
    assert_eq!(layers[0].spans().len(), 2);
    assert_eq!(layers[0].spans()[0].tween(), Tween::Linear);
    assert_eq!(layers[0].spans()[1].tween(), Tween::None);
    assert_eq!(
        (
            layers[0].spans()[1].start(),
            layers[0].spans()[1].duration().get()
        ),
        (5, 5)
    );
    assert!(matches!(
        layers[0].spans()[0].content(),
        Content::Instance(_)
    ));
}

#[test]
fn imported_artwork_renders_where_the_source_put_it() {
    let doc = import_xfl(&sample()).expect("import").document;
    let bg = doc.stage().background();
    // The red 10x10 square, instanced at (10, 20), then tweened toward (110, 20).
    assert_eq!(pixel(&doc, 0, 15, 25), Rgba8::opaque(255, 0, 0));
    assert_eq!(pixel(&doc, 0, 50, 25), bg);
    assert_eq!(
        pixel(&doc, 2, 15 + 40, 25),
        Rgba8::opaque(255, 0, 0),
        "40% of the way: x = 10 + 0.4 * 100"
    );
    assert_eq!(pixel(&doc, 5, 115, 25), Rgba8::opaque(255, 0, 0));
    // The half-transparent blue rectangle at (200, 100) 40x20 with a 2px black stroke on top of it.
    assert_eq!(
        pixel(&doc, 0, 200, 110),
        Rgba8::opaque(0, 0, 0),
        "stroke on the left edge"
    );
    let inside = pixel(&doc, 0, 220, 110);
    assert!(
        inside.b > inside.r && inside.b > 100,
        "blue fill blended over the background: {inside:?}"
    );
    assert_eq!(pixel(&doc, 0, 220, 140), bg, "below the rectangle");
}

#[test]
fn unsupported_features_are_reported_not_hidden() {
    let imported = import_xfl(&sample()).expect("import");
    let unsupported = messages(&imported, Severity::Unsupported).join("\n");
    assert!(
        unsupported.contains("mask layers are not imported"),
        "{unsupported}"
    );
    assert!(
        !unsupported.contains("folder"),
        "folders are imported now, so they are not reported: {unsupported}"
    );
    assert!(
        unsupported.contains("DOMStaticText is not imported"),
        "{unsupported}"
    );
    let info = messages(&imported, Severity::Info).join("\n");
    assert!(info.contains("share a keyframe"), "{info}");
    // Dot is a graphic symbol, so nothing about it changes meaning on import.
    assert!(messages(&imported, Severity::Warning).is_empty());
}

#[test]
fn shapes_with_holes_use_opposite_orientations_and_render_as_holes() {
    let xml = r##"<DOMDocument frameRate="24" width="30" height="30"><timelines><DOMTimeline name="S"><layers>
    <DOMLayer name="L"><frames><DOMFrame index="0"><elements><DOMShape>
      <fills><FillStyle index="1"><SolidColor color="#00FF00"/></FillStyle></fills>
      <edges>
        <Edge fillStyle1="1" edges="!0 0|400 0|400 400|0 400|0 0"/>
        <Edge fillStyle0="1" edges="!100 100|300 100|300 300|100 300|100 100"/>
      </edges>
    </DOMShape></elements></DOMFrame></frames></DOMLayer></layers></DOMTimeline></timelines></DOMDocument>"##;
    let doc = import_xfl(&Files::default().with("DOMDocument.xml", xml))
        .expect("import")
        .document;
    assert_eq!(
        pixel(&doc, 0, 2, 2),
        Rgba8::opaque(0, 255, 0),
        "the ring is filled"
    );
    assert_eq!(pixel(&doc, 0, 10, 10), Rgba8::WHITE, "the middle is a hole");
    assert_eq!(pixel(&doc, 0, 25, 25), Rgba8::WHITE, "outside the shape");
}

#[test]
fn curves_become_cubics_and_stay_inside_their_control_points() {
    let xml = r##"<DOMDocument frameRate="24" width="40" height="40"><timelines><DOMTimeline name="S"><layers>
    <DOMLayer name="L"><frames><DOMFrame index="0"><elements><DOMShape>
      <fills><FillStyle index="1"><SolidColor color="#000000"/></FillStyle></fills>
      <edges><Edge fillStyle1="1" edges="!0 0[400 0 400 400|0 400|0 0"/></edges>
    </DOMShape></elements></DOMFrame></frames></DOMLayer></layers></DOMTimeline></timelines></DOMDocument>"##;
    let doc = import_xfl(&Files::default().with("DOMDocument.xml", xml))
        .expect("import")
        .document;
    let Content::Shape(shape) = doc.scenes()[0].timeline().layers()[0].spans()[0].content() else {
        panic!("shape expected");
    };
    assert!(
        shape
            .path()
            .iter()
            .any(|s| matches!(s, flare_document::PathSegment::CubicTo(..)))
    );
    // Inside the curved corner is filled; the far corner beyond the curve is not.
    assert_eq!(pixel(&doc, 0, 3, 17), Rgba8::opaque(0, 0, 0));
    assert_eq!(pixel(&doc, 0, 19, 3), Rgba8::WHITE);
}

#[test]
fn bad_documents_fail_with_clear_errors() {
    let err = |files: Files| import_xfl(&files).expect_err("should fail");
    assert!(matches!(err(Files::default()), ImportError::NotXfl(_)));
    assert!(matches!(
        err(Files::default().with("DOMDocument.xml", "<Other/>")),
        ImportError::NotXfl(_)
    ));
    assert!(matches!(
        err(Files::default().with("DOMDocument.xml", "<DOMDocument")),
        ImportError::Xml { .. }
    ));
    assert!(matches!(
        err(Files::default().with("DOMDocument.xml", "<DOMDocument/>")),
        ImportError::NoTimelines
    ));
    let doc = |attrs: &str| {
        format!(
            r#"<DOMDocument {attrs}><timelines><DOMTimeline name="S"/></timelines></DOMDocument>"#
        )
    };
    for attrs in [
        "width=\"0\"",
        "height=\"99999\"",
        "width=\"abc\"",
        "frameRate=\"0\"",
        "frameRate=\"fast\"",
    ] {
        let e = err(Files::default().with("DOMDocument.xml", &doc(attrs)));
        assert!(
            matches!(e, ImportError::BadAttribute { .. }),
            "{attrs}: {e}"
        );
    }
    let bomb = r#"<!DOCTYPE a [<!ENTITY x "xxxx">]><DOMDocument/>"#;
    assert!(matches!(
        err(Files::default().with("DOMDocument.xml", bomb)),
        ImportError::Xml { .. }
    ));
}

#[test]
fn missing_library_items_and_instances_are_reported_and_skipped() {
    let files = Files::default().with("DOMDocument.xml", MAIN); // no LIBRARY/Dot.xml
    let imported = import_xfl(&files).expect("still imports");
    assert!(imported.document.symbols().is_empty());
    let unsupported = messages(&imported, Severity::Unsupported).join("\n");
    assert!(
        unsupported.contains("library item not found"),
        "{unsupported}"
    );
    assert!(
        unsupported.contains("missing symbol \"Dot\""),
        "{unsupported}"
    );
    // The instances are gone but their (now empty) frames keep the layer's timing.
    let main = &imported.document.scenes()[0].timeline().layers()[0];
    assert!(
        main.spans()
            .iter()
            .all(|s| matches!(s.content(), Content::Empty))
    );
}

#[test]
fn symbol_cycles_in_the_library_are_rejected_as_an_invalid_document() {
    let item = |name: &str, other: &str| {
        format!(
            r#"<DOMSymbolItem name="{name}" symbolType="graphic"><timeline><DOMTimeline name="t"><layers><DOMLayer name="L"><frames>
            <DOMFrame index="0"><elements><DOMSymbolInstance libraryItemName="{other}"/></elements></DOMFrame></frames></DOMLayer></layers></DOMTimeline></timeline></DOMSymbolItem>"#
        )
    };
    let doc = r#"<DOMDocument><symbols><Include href="A.xml"/><Include href="B.xml"/></symbols>
        <timelines><DOMTimeline name="S"/></timelines></DOMDocument>"#;
    let files = Files::default()
        .with("DOMDocument.xml", doc)
        .with("LIBRARY/A.xml", &item("A", "B"))
        .with("LIBRARY/B.xml", &item("B", "A"));
    assert!(matches!(import_xfl(&files), Err(ImportError::Invalid(_))));
}

#[test]
fn awkward_frames_are_skipped_with_a_reason() {
    let xml = r##"<DOMDocument><timelines><DOMTimeline name="S"><layers>
      <DOMLayer name="Bad"><frames>
        <DOMFrame duration="2"><elements/></DOMFrame>
        <DOMFrame index="3" duration="0"><elements/></DOMFrame>
        <DOMFrame index="x"><elements/></DOMFrame>
      </frames></DOMLayer>
      <DOMLayer name="Overlap"><frames>
        <DOMFrame index="0" duration="5"><elements/></DOMFrame>
        <DOMFrame index="2" duration="5"><elements/></DOMFrame>
      </frames></DOMLayer>
      <DOMLayer name="Fine"><frames><DOMFrame index="0" duration="2" tweenType="shape"><elements/></DOMFrame></frames></DOMLayer>
    </layers></DOMTimeline></timelines></DOMDocument>"##;
    let imported = import_xfl(&Files::default().with("DOMDocument.xml", xml)).expect("import");
    let unsupported = messages(&imported, Severity::Unsupported).join("\n");
    assert!(
        unsupported.contains("without a valid index"),
        "{unsupported}"
    );
    assert!(unsupported.contains("invalid duration"), "{unsupported}");
    assert!(unsupported.contains("frames overlap"), "{unsupported}");
    assert!(
        unsupported.contains("a shape tween needs a drawn shape"),
        "{unsupported}"
    );
    let names: Vec<&str> = imported.document.scenes()[0]
        .timeline()
        .layers()
        .iter()
        .map(|l| l.name())
        .collect();
    assert_eq!(
        names,
        vec!["Fine"],
        "layers that cannot be represented are skipped; empty ones stay out"
    );
}

#[test]
fn frame_rates_are_exact_rationals() {
    let rate = |t: &str| frame_rate(t).map(|r| (r.numerator(), r.denominator()));
    assert_eq!(rate("24"), Ok((24, 1)));
    assert_eq!(rate("29.97"), Ok((30_000, 1001)));
    assert_eq!(rate("23.976"), Ok((24_000, 1001)));
    assert_eq!(rate("59.94"), Ok((60_000, 1001)));
    assert_eq!(rate("12.5"), Ok((25, 2)));
    for bad in ["0", "-3", "abc", "inf", "NaN", "5000"] {
        assert!(rate(bad).is_err(), "{bad}");
    }
}

fn write_xfl_dir(dir: &std::path::Path) {
    std::fs::create_dir_all(dir.join("LIBRARY")).expect("mkdir");
    std::fs::write(dir.join("DOMDocument.xml"), MAIN).expect("write");
    std::fs::write(dir.join("LIBRARY/Dot.xml"), DOT).expect("write");
    std::fs::write(dir.join("project.xfl"), "PROXY-CS5").expect("write");
}

fn shape_of(doc: &Document) -> (usize, Vec<usize>) {
    let layers = doc.scenes()[0].timeline().layers();
    (
        doc.symbols().len(),
        layers.iter().map(|l| l.spans().len()).collect(),
    )
}

#[test]
fn directories_zip_files_and_xfl_stubs_import_the_same_document() {
    let dir = tempfile::tempdir().expect("dir");
    let root = dir.path().join("proj");
    write_xfl_dir(&root);

    let from_dir = import_path(&root).expect("dir");
    let from_stub = import_path(&root.join("project.xfl")).expect("stub");

    let fla = dir.path().join("proj.fla");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&fla).expect("create"));
    let options = zip::write::SimpleFileOptions::default();
    for (name, body) in [("DOMDocument.xml", MAIN), ("LIBRARY/Dot.xml", DOT)] {
        zip.start_file(name, options).expect("entry");
        zip.write_all(body.as_bytes()).expect("body");
    }
    zip.finish().expect("finish");
    let from_zip = import_path(&fla).expect("zip");

    assert_eq!(shape_of(&from_dir.document), shape_of(&from_stub.document));
    assert_eq!(shape_of(&from_dir.document), shape_of(&from_zip.document));
    assert_eq!(from_dir.diagnostics.len(), from_zip.diagnostics.len());
    assert_ne!(
        from_dir.document.id(),
        from_zip.document.id(),
        "every import is a new document"
    );
}

#[test]
fn non_xfl_files_are_recognised_and_explained() {
    let dir = tempfile::tempdir().expect("dir");
    let legacy = dir.path().join("old.fla");
    std::fs::write(
        &legacy,
        [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1, 0, 0],
    )
    .expect("write");
    let e = import_path(&legacy).expect_err("legacy");
    assert!(e.to_string().contains("legacy binary FLA"), "{e}");

    let text = dir.path().join("a.txt");
    std::fs::write(&text, "hello").expect("write");
    assert!(matches!(import_path(&text), Err(ImportError::NotXfl(_))));
    assert!(matches!(
        import_path(&dir.path().join("missing")),
        Err(ImportError::Io(_))
    ));

    let empty_zip = dir.path().join("e.fla");
    zip::ZipWriter::new(std::fs::File::create(&empty_zip).expect("create"))
        .finish()
        .expect("finish");
    assert!(
        matches!(import_path(&empty_zip), Err(ImportError::NotXfl(_))),
        "a zip without DOMDocument.xml"
    );
}

#[test]
fn sources_refuse_names_that_escape_the_document() {
    let dir = tempfile::tempdir().expect("dir");
    let root = dir.path().join("proj");
    write_xfl_dir(&root);
    std::fs::write(dir.path().join("secret.txt"), "secret").expect("write");
    let source = DirSource(root.clone());
    for name in [
        "../secret.txt",
        "/etc/passwd",
        "LIBRARY/../../secret.txt",
        "a\\b",
    ] {
        assert!(source.read(name).is_err(), "{name}");
    }
    assert_eq!(source.read("nope.xml").expect("read"), None);
    assert!(source.read("DOMDocument.xml").expect("read").is_some());

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(dir.path().join("secret.txt"), root.join("LIBRARY/link.xml"))
            .expect("symlink");
        assert!(
            source.read("LIBRARY/link.xml").is_err(),
            "symlinks out of the document are refused"
        );
    }

    let fla = dir.path().join("p.fla");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&fla).expect("create"));
    zip.start_file("DOMDocument.xml", zip::write::SimpleFileOptions::default())
        .expect("entry");
    zip.write_all(b"<DOMDocument/>").expect("body");
    zip.finish().expect("finish");
    let zipped = ZipSource::open(&fla).expect("open");
    assert!(zipped.read("../x").is_err());
    assert_eq!(zipped.read("missing.xml").expect("read"), None);
}
