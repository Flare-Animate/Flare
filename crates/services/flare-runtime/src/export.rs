//! Exporting one frame of the document as a PNG. The work is slow for big stages, so the app
//! runs it on a thread of its own.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use flare_document::Document;
use flare_render::{Backdrop, StillFormat};

/// Renders `frame` of the first scene at the stage's size over `backdrop`, encoded as `format`.
/// A format with no alpha channel refuses a transparent backdrop before anything is drawn. Errors
/// are worded for the person who asked for the export.
pub fn still_bytes(
    document: &Document,
    frame: u32,
    format: StillFormat,
    backdrop: Backdrop,
) -> Result<Vec<u8>, String> {
    if backdrop.is_transparent() {
        match format {
            StillFormat::Jpeg { .. } => return Err(flare_render::no_transparency("JPEG pictures")),
            StillFormat::Hdr => {
                return Err(flare_render::no_transparency("Radiance HDR pictures"));
            }
            _ => {}
        }
    }
    let scene = document
        .scenes()
        .first()
        .ok_or("The document has no scene to export.")?;
    let drawn = flare_eval::evaluate_frame(document, scene.id(), frame)
        .map_err(|error| format!("Cannot draw frame {}: {error}", frame + 1))?;
    let stage = document.stage();
    let image = flare_render::render(
        &drawn,
        stage.width(),
        stage.height(),
        backdrop.over(stage.background()),
    )
    .map_err(|error| format!("Cannot render the frame: {error}"))?;
    let mut bytes = Vec::new();
    image
        .write_still(&mut bytes, format)
        .map_err(|error| format!("Cannot write the image: {error}"))?;
    Ok(bytes)
}

/// Renders `frame` of the first scene as an SVG picture at the stage's size: the shapes as paths,
/// the pictures embedded, and the notes for what SVG cannot carry (see `flare_export::svg`). A
/// transparent backdrop leaves the stage rectangle out.
pub fn svg_export(
    document: &Document,
    frame: u32,
    backdrop: Backdrop,
) -> Result<flare_export::svg::Svg, String> {
    let scene = document
        .scenes()
        .first()
        .ok_or("The document has no scene to export.")?;
    let drawn = flare_eval::evaluate_frame(document, scene.id(), frame)
        .map_err(|error| format!("Cannot draw frame {}: {error}", frame + 1))?;
    let stage = document.stage();
    flare_export::svg::frame_svg(
        &drawn,
        stage.width(),
        stage.height(),
        backdrop.over(stage.background()),
    )
    .map_err(|error| format!("Cannot write the SVG: {error}"))
}

/// Renders `frame` of the first scene over `backdrop` as a flattened PSD: one picture layer of the
/// stage's size, with its alpha (see `flare_export::psd`).
pub fn psd_export(document: &Document, frame: u32, backdrop: Backdrop) -> Result<Vec<u8>, String> {
    let scene = document
        .scenes()
        .first()
        .ok_or("The document has no scene to export.")?;
    let drawn = flare_eval::evaluate_frame(document, scene.id(), frame)
        .map_err(|error| format!("Cannot draw frame {}: {error}", frame + 1))?;
    let stage = document.stage();
    let image = flare_render::render(
        &drawn,
        stage.width(),
        stage.height(),
        backdrop.over(stage.background()),
    )
    .map_err(|error| format!("Cannot render the frame: {error}"))?;
    flare_export::psd::flattened_psd(stage.width(), stage.height(), image.rgba())
        .map_err(|error| format!("Cannot write the PSD: {error}"))
}

/// Renders `frame` over the stage colour as PNG bytes (see [`still_bytes`]).
pub fn png_bytes(document: &Document, frame: u32) -> Result<Vec<u8>, String> {
    still_bytes(document, frame, StillFormat::Png, Backdrop::Stage)
}

/// Encodes straight-alpha RGBA pixels as PNG bytes. `rgba` must hold `width * height` pixels.
pub fn rgba_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|error| format!("Cannot write the PNG: {error}"))?;
    writer
        .write_image_data(rgba)
        .map_err(|error| format!("Cannot write the PNG: {error}"))?;
    writer
        .finish()
        .map_err(|error| format!("Cannot write the PNG: {error}"))?;
    Ok(bytes)
}

/// Writes `frame` to `path` as a PNG over `backdrop` (see [`still_bytes`]).
pub fn export_png(
    document: &Document,
    frame: u32,
    path: &Path,
    backdrop: Backdrop,
) -> Result<(), String> {
    export_still(document, frame, path, StillFormat::Png, backdrop)
}

/// Writes `frame` to `path` in `format` over `backdrop` (see [`still_bytes`]). The file is
/// created only after the frame is rendered and encoded, so a refused format leaves no empty file
/// behind.
pub fn export_still(
    document: &Document,
    frame: u32,
    path: &Path,
    format: StillFormat,
    backdrop: Backdrop,
) -> Result<(), String> {
    let bytes = still_bytes(document, frame, format, backdrop)?;
    let file =
        File::create(path).map_err(|error| format!("Cannot create {}: {error}", path.display()))?;
    BufWriter::new(file)
        .write_all(&bytes)
        .map_err(|error| format!("Cannot write the image: {error}"))
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
    fn a_jpeg_is_written_and_a_refused_quality_leaves_no_file() {
        let document = document_with_a_red_square();
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("frame.jpg");
        export_still(
            &document,
            0,
            &path,
            StillFormat::Jpeg { quality: 90 },
            Backdrop::Stage,
        )
        .expect("export");
        let bytes = std::fs::read(&path).expect("read");
        assert_eq!(
            &bytes[..3],
            &[0xFF, 0xD8, 0xFF],
            "JPEG start-of-image marker"
        );

        let refused = dir.path().join("refused.jpg");
        let error = export_still(
            &document,
            0,
            &refused,
            StillFormat::Jpeg { quality: 0 },
            Backdrop::Stage,
        )
        .expect_err("quality 0 is outside 1 to 100");
        assert!(error.contains("JPEG quality 0"), "{error}");
        assert!(!refused.exists(), "a refused export leaves no file behind");
    }

    #[test]
    fn a_frame_is_written_as_a_png_the_size_of_the_stage() {
        let document = document_with_a_red_square();
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("frame.png");
        export_png(&document, 0, &path, Backdrop::Stage).expect("export");
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

    /// The pixel at (`x`, `y`) of a PNG, as RGBA.
    fn png_pixel(bytes: &[u8], x: u32, y: u32) -> [u8; 4] {
        let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
            .read_info()
            .expect("png");
        let mut buffer = vec![0; reader.output_buffer_size().expect("size")];
        let info = reader.next_frame(&mut buffer).expect("frame");
        assert_eq!(info.color_type, png::ColorType::Rgba);
        let i = ((y * info.width + x) * 4) as usize;
        [buffer[i], buffer[i + 1], buffer[i + 2], buffer[i + 3]]
    }

    #[test]
    fn a_transparent_backdrop_leaves_the_empty_stage_clear_and_the_drawing_whole() {
        let document = document_with_a_red_square();
        let stage = still_bytes(&document, 0, StillFormat::Png, Backdrop::Stage).expect("png");
        let clear =
            still_bytes(&document, 0, StillFormat::Png, Backdrop::Transparent).expect("png");
        assert_eq!(
            png_pixel(&stage, 100, 100)[3],
            255,
            "the stage colour fills the rest"
        );
        assert_eq!(
            png_pixel(&clear, 100, 100),
            [0, 0, 0, 0],
            "nothing drawn: see-through"
        );
        assert_eq!(
            png_pixel(&clear, 5, 5),
            [255, 0, 0, 255],
            "the square is still drawn"
        );

        let svg = svg_export(&document, 0, Backdrop::Transparent).expect("svg");
        let with_stage = svg_export(&document, 0, Backdrop::Stage).expect("svg");
        assert!(
            svg.text.len() < with_stage.text.len(),
            "the stage rectangle is left out"
        );
        let psd = psd_export(&document, 0, Backdrop::Transparent).expect("psd");
        assert_eq!(&psd[..4], b"8BPS");
    }

    #[test]
    fn a_format_without_alpha_refuses_a_transparent_backdrop_and_writes_nothing() {
        let document = document_with_a_red_square();
        let dir = tempfile::tempdir().expect("dir");
        for (format, name) in [
            (StillFormat::Jpeg { quality: 90 }, "frame.jpg"),
            (StillFormat::Hdr, "frame.hdr"),
        ] {
            let path = dir.path().join(name);
            let error = export_still(&document, 0, &path, format, Backdrop::Transparent)
                .expect_err("no alpha channel");
            assert!(
                error.contains("cannot have a transparent background"),
                "{error}"
            );
            assert!(!path.exists(), "{name}: nothing is written");
        }
    }

    #[test]
    fn problems_come_back_as_readable_messages() {
        let document = document_with_a_red_square();
        let missing = Path::new("/definitely/not/a/directory/frame.png");
        let error = export_png(&document, 0, missing, Backdrop::Stage).expect_err("no such folder");
        assert!(error.starts_with("Cannot create"), "{error}");
    }
}
