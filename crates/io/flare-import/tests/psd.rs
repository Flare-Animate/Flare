//! Photoshop import: layers come in at their offsets, a hidden layer is reported and left out, a
//! file with no layers comes in flattened, and a damaged file is refused with its name.
#![allow(clippy::expect_used)]

use std::path::Path;

use flare_import::import_foreign_bytes;

/// One layer of a test file: its name, its place, its size and its straight-alpha pixels.
struct Layer {
    name: &'static str,
    left: i32,
    top: i32,
    width: u32,
    height: u32,
    hidden: bool,
    rgba: Vec<u8>,
}

fn be32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_be_bytes());
}

/// The RGBA pixels of one color, opaque.
fn solid(width: u32, height: u32, color: [u8; 3]) -> Vec<u8> {
    (0..width * height)
        .flat_map(|_| [color[0], color[1], color[2], 255])
        .collect()
}

/// A PSD with the given layers in its layer section, and a flattened composite of the first
/// layer's size; the layers are how the importer sees them.
fn layered_psd(canvas: (u32, u32), layers: &[Layer]) -> Vec<u8> {
    let mut info = Vec::new();
    info.extend_from_slice(&(layers.len() as i16).to_be_bytes());
    for layer in layers {
        let (top, left) = (layer.top, layer.left);
        let bottom = top + layer.height as i32;
        let right = left + layer.width as i32;
        for v in [top, left, bottom, right] {
            info.extend_from_slice(&v.to_be_bytes());
        }
        info.extend_from_slice(&4_u16.to_be_bytes()); // four channels: red, green, blue, alpha
        let plane = layer.width * layer.height;
        for id in [0_i16, 1, 2, -1] {
            info.extend_from_slice(&id.to_be_bytes());
            be32(&mut info, 2 + plane); // the compression word and the plane
        }
        info.extend_from_slice(b"8BIM");
        info.extend_from_slice(b"norm");
        info.extend_from_slice(&[255, 0, if layer.hidden { 2 } else { 0 }, 0]); // opacity, clipping, flags, filler
        // Extra data: no mask, no blending ranges, then the name as a Pascal string padded to 4.
        let mut name = vec![layer.name.len() as u8];
        name.extend_from_slice(layer.name.as_bytes());
        while name.len() % 4 != 0 {
            name.push(0);
        }
        let mut extra = Vec::new();
        be32(&mut extra, 0);
        be32(&mut extra, 0);
        extra.extend(name);
        be32(&mut info, extra.len() as u32);
        info.extend(extra);
    }
    for layer in layers {
        for channel in 0..4 {
            info.extend_from_slice(&0_u16.to_be_bytes()); // raw
            info.extend(layer.rgba.as_chunks::<4>().0.iter().map(|px| px[channel]));
        }
    }
    if info.len() % 2 == 1 {
        info.push(0);
    }
    let mut section = Vec::new();
    be32(&mut section, info.len() as u32);
    section.extend(info);
    be32(&mut section, 0); // no global mask
    let mut out = header(canvas, 4);
    be32(&mut out, 0); // colour mode data
    be32(&mut out, 0); // image resources
    be32(&mut out, section.len() as u32);
    out.extend(section);
    out.extend_from_slice(&0_u16.to_be_bytes()); // composite: raw
    let composite = solid(canvas.0, canvas.1, [0, 0, 0]);
    for channel in 0..4 {
        out.extend(composite.as_chunks::<4>().0.iter().map(|px| px[channel]));
    }
    out
}

fn header(canvas: (u32, u32), channels: u16) -> Vec<u8> {
    let mut out = b"8BPS".to_vec();
    out.extend_from_slice(&1_u16.to_be_bytes());
    out.extend_from_slice(&[0; 6]);
    out.extend_from_slice(&channels.to_be_bytes());
    be32(&mut out, canvas.1);
    be32(&mut out, canvas.0);
    out.extend_from_slice(&8_u16.to_be_bytes());
    out.extend_from_slice(&3_u16.to_be_bytes());
    out
}

#[test]
fn each_visible_layer_comes_in_at_its_offset_and_a_hidden_one_is_reported() {
    let red = Layer {
        name: "Red",
        left: 2,
        top: 1,
        width: 3,
        height: 2,
        hidden: false,
        rgba: solid(3, 2, [255, 0, 0]),
    };
    let blue = Layer {
        name: "Blue",
        left: 0,
        top: 0,
        width: 2,
        height: 2,
        hidden: false,
        rgba: solid(2, 2, [0, 0, 255]),
    };
    let ghost = Layer {
        name: "Ghost",
        left: 0,
        top: 0,
        width: 1,
        height: 1,
        hidden: true,
        rgba: solid(1, 1, [9, 9, 9]),
    };
    let bytes = layered_psd((8, 6), &[blue, red, ghost]);
    let imported = import_foreign_bytes(Path::new("a.psd"), bytes).expect("import");
    let doc = &imported.document;
    assert_eq!((doc.stage().width(), doc.stage().height()), (8, 6));
    assert_eq!(
        doc.scenes()[0].timeline().layers().len(),
        2,
        "the hidden layer is left out"
    );
    assert!(
        imported
            .diagnostics
            .iter()
            .any(|d| d.message.contains("hidden layer \"Ghost\"")),
        "{:?}",
        imported.diagnostics
    );
    let drawn = flare_eval::evaluate_frame(doc, doc.scenes()[0].id(), 0).expect("frame");
    assert_eq!(drawn.bitmaps.len(), 2);
    let mut offsets: Vec<(f64, f64)> = drawn
        .bitmaps
        .iter()
        .map(|b| {
            let [_, _, _, _, e, f] = b.transform.to_cols_array();
            (e, f)
        })
        .collect();
    offsets.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    assert_eq!(offsets, vec![(0.0, 0.0), (2.0, 1.0)]);
}

#[test]
fn a_file_with_no_layers_comes_in_as_its_flattened_picture() {
    let mut out = header((3, 2), 4);
    be32(&mut out, 0);
    be32(&mut out, 0);
    be32(&mut out, 0);
    out.extend_from_slice(&0_u16.to_be_bytes());
    let pixels = solid(3, 2, [10, 20, 30]);
    for channel in 0..4 {
        out.extend(pixels.as_chunks::<4>().0.iter().map(|px| px[channel]));
    }
    let imported = import_foreign_bytes(Path::new("flat.psd"), out).expect("import");
    let doc = &imported.document;
    assert_eq!((doc.stage().width(), doc.stage().height()), (3, 2));
    assert_eq!(doc.bitmaps()[0].pixels(), pixels.as_slice());
    assert_eq!(doc.scenes()[0].timeline().frame_count(), 1);
}

#[test]
fn a_damaged_psd_is_refused_with_its_name() {
    let error = import_foreign_bytes(Path::new("bad.psd"), b"8BPS but not really".to_vec())
        .expect_err("refused");
    assert!(error.to_string().contains("bad.psd"), "{error}");
    let mut huge = header((4, 4), 4);
    // A canvas far past the limit, claimed in the header and never backed by data.
    huge.truncate(14);
    be32(&mut huge, 40_000);
    be32(&mut huge, 40_000);
    assert!(import_foreign_bytes(Path::new("huge.psd"), huge).is_err());
}
