//! Exporting one frame of the document as a PNG. The work is pure and slow for big stages, so
//! the app runs it on its own thread; nothing here touches the UI.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use flare_document::Document;

/// Renders `frame` of the first scene at the stage's size over its background and writes it to
/// `path` as a PNG. Errors are worded for the person who asked for the export.
pub fn export_png(document: &Document, frame: u32, path: &Path) -> Result<(), String> {
    let scene = document
        .scenes()
        .first()
        .ok_or("The document has no scene to export.")?;
    let drawn = flare_eval::evaluate_frame(document, scene.id(), frame)
        .map_err(|error| format!("Cannot draw frame {}: {error}", frame + 1))?;
    let stage = document.stage();
    let image = flare_render::render(&drawn, stage.width(), stage.height(), stage.background())
        .map_err(|error| format!("Cannot render the frame: {error}"))?;
    let file =
        File::create(path).map_err(|error| format!("Cannot create {}: {error}", path.display()))?;
    image
        .write_png(BufWriter::new(file))
        .map_err(|error| format!("Cannot write the PNG: {error}"))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use flare_core::{Rgba8, ShapeId};
    use flare_document::{Content, FrameSpan, Layer, Scene, Shape, Timeline};
    use std::num::NonZeroU32;

    fn document_with_a_red_square() -> Document {
        let shape = Shape::rect(
            ShapeId::new(),
            0.0,
            0.0,
            10.0,
            10.0,
            Rgba8::opaque(255, 0, 0),
        )
        .expect("rect");
        let span = FrameSpan::new(
            0,
            NonZeroU32::new(5).expect("nonzero"),
            Content::Shape(shape),
        );
        let layer = Layer::new(flare_core::LayerId::new(), "L", vec![span]).expect("layer");
        let scene = Scene::new(flare_core::SceneId::new(), "S", Timeline::new(vec![layer]));
        Document::new(
            flare_core::DocumentId::new(),
            flare_core::FrameRate::new(24, 1).expect("rate"),
            vec![scene],
            vec![],
        )
        .expect("document")
    }

    #[test]
    fn a_frame_is_written_as_a_png_the_size_of_the_stage() {
        let document = document_with_a_red_square();
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("frame.png");
        export_png(&document, 0, &path).expect("export");
        let bytes = std::fs::read(&path).expect("read");
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        let reader = decoder.read_info().expect("png");
        let info = reader.info();
        assert_eq!(
            (info.width, info.height),
            (document.stage().width(), document.stage().height())
        );
    }

    #[test]
    fn problems_come_back_as_readable_messages() {
        let document = document_with_a_red_square();
        let missing = Path::new("/definitely/not/a/directory/frame.png");
        let error = export_png(&document, 0, missing).expect_err("no such folder");
        assert!(error.starts_with("Cannot create"), "{error}");
    }
}
