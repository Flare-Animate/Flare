//! Library bitmaps and the bitmap instances that show them in an XFL document.

#![allow(clippy::expect_used)]

use std::collections::HashMap;

use flare_core::Rgba8;
use flare_document::{Content, Document};
use flare_import::{FileSource, Severity, import_xfl};

struct Files(HashMap<String, Vec<u8>>);

impl FileSource for Files {
    fn read(&self, name: &str) -> std::io::Result<Option<Vec<u8>>> {
        Ok(self.0.get(name).cloned())
    }
}

/// A `width` by `height` PNG of one opaque color.
fn png(width: u32, height: u32, color: [u8; 3]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("header");
    let data: Vec<u8> = (0..width * height).flat_map(|_| color).collect();
    writer.write_image_data(&data).expect("data");
    writer.finish().expect("finish");
    out
}

/// A 40 by 30 stage with one frame of one layer holding a bitmap instance at (20, 10).
fn document_with(media: &str, element: &str, files: Vec<(&str, Vec<u8>)>) -> Files {
    let xml = format!(
        r##"<DOMDocument frameRate="12" width="40" height="30" backgroundColor="#FFFFFF">
<media>{media}</media>
<timelines><DOMTimeline name="Scene 1"><layers><DOMLayer name="L"><frames><DOMFrame index="0"><elements>
{element}
</elements></DOMFrame></frames></DOMLayer></layers></DOMTimeline></timelines></DOMDocument>"##
    );
    let mut map = HashMap::from([("DOMDocument.xml".to_owned(), xml.into_bytes())]);
    for (name, bytes) in files {
        map.insert(format!("LIBRARY/{name}"), bytes);
    }
    Files(map)
}

const PIC: &str = r#"<DOMBitmapItem name="pic.png" href="pic.png"/>"#;
const PLACED: &str = r#"<DOMBitmapInstance libraryItemName="pic.png"><matrix><Matrix a="1" b="0" c="0" d="1" tx="20" ty="10"/></matrix></DOMBitmapInstance>"#;

fn pixel(document: &Document, x: u32, y: u32) -> Rgba8 {
    let scene = document.scenes()[0].id();
    let evaluated = flare_eval::evaluate_frame(document, scene, 0).expect("eval");
    let stage = document.stage();
    flare_render::render(
        &evaluated,
        stage.width(),
        stage.height(),
        stage.background(),
    )
    .expect("render")
    .pixel(x, y)
    .expect("pixel")
}

#[test]
fn a_png_library_bitmap_is_shown_where_its_instance_puts_it() {
    let files = document_with(PIC, PLACED, vec![("pic.png", png(4, 4, [0, 200, 0]))]);
    let imported = import_xfl(&files).expect("import");
    let document = &imported.document;
    assert_eq!(document.bitmaps().len(), 1);
    let content = document.scenes()[0].timeline().layers()[0].spans()[0]
        .content()
        .clone();
    let Content::Bitmap(instance) = content else {
        panic!("not a bitmap instance: {content:?}")
    };
    assert_eq!(instance.transform().as_affine().translation.x, 20.0);
    // Inside the 4 by 4 image at (20, 10), and white outside it.
    assert_eq!(pixel(document, 21, 11), Rgba8::new(0, 200, 0, 255));
    assert_eq!(pixel(document, 5, 5), Rgba8::WHITE);
}

#[test]
fn a_jpeg_library_bitmap_is_kept_as_source_data_and_reported() {
    let jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10];
    let files = document_with(
        r#"<DOMBitmapItem name="pic.jpg" href="pic.jpg"/>"#,
        r#"<DOMBitmapInstance libraryItemName="pic.jpg"/>"#,
        vec![("pic.jpg", jpeg)],
    );
    let imported = import_xfl(&files).expect("import");
    assert!(imported.document.bitmaps().is_empty());
    let messages: String = imported
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Unsupported)
        .map(|d| d.message.clone())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(messages.contains("only PNG bitmaps"), "{messages}");
    assert!(messages.contains("not imported"), "{messages}");
}

#[test]
fn a_corrupt_png_is_reported_and_the_rest_of_the_document_still_imports() {
    let mut broken = png(4, 4, [1, 2, 3]);
    broken.truncate(40);
    let files = document_with(PIC, PLACED, vec![("pic.png", broken)]);
    let imported = import_xfl(&files).expect("the document still imports");
    assert!(imported.document.bitmaps().is_empty());
    assert!(
        imported
            .diagnostics
            .iter()
            .any(|d| d.message.contains("could not be read")),
        "{:?}",
        imported.diagnostics
    );
}

#[test]
fn a_missing_image_file_is_reported() {
    let files = document_with(PIC, PLACED, vec![]);
    let imported = import_xfl(&files).expect("import");
    assert!(
        imported
            .diagnostics
            .iter()
            .any(|d| d.message == "image file not found"),
        "{:?}",
        imported.diagnostics
    );
}
