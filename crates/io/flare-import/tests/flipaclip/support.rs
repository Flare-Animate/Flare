//! Builds FlipaClip projects in memory, in the layout the sample files show: a zip of entries
//! without extensions, each picture a 24-byte header and a zstd stream of RGBA rows.
#![allow(dead_code, clippy::expect_used)]

use std::io::{Cursor, Write};

use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

pub const WIDTH: u32 = 8;
pub const HEIGHT: u32 = 6;

/// A picture of `width` x `height` pixels, with `rgba` as its pixel rows.
pub fn picture(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let payload = zstd::stream::encode_all(rgba, 0).expect("encodes");
    let length = 24 + payload.len();
    let mut out = vec![0x89, b'F', b'C', b'I', b'\r', b'\n', 0x1a, b'\n'];
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&24_u16.to_le_bytes());
    out.extend_from_slice(&(length as u32).to_be_bytes()[1..]);
    out.push(0);
    out.extend_from_slice(&(width as u16).to_be_bytes());
    out.extend_from_slice(&(height as u16).to_be_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&payload);
    out
}

/// A canvas-sized picture with every pixel `px` (straight RGBA).
pub fn solid(px: [u8; 4]) -> Vec<u8> {
    let rgba: Vec<u8> = px.repeat((WIDTH * HEIGHT) as usize);
    picture(WIDTH, HEIGHT, &rgba)
}

/// A canvas-sized picture that draws only the pixels for which `draw(x, y)` is true.
pub fn drawn(px: [u8; 4], draw: impl Fn(u32, u32) -> bool) -> Vec<u8> {
    let mut rgba = vec![0_u8; (WIDTH * HEIGHT * 4) as usize];
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            if draw(x, y) {
                let at = ((y * WIDTH + x) * 4) as usize;
                rgba[at..at + 4].copy_from_slice(&px);
            }
        }
    }
    picture(WIDTH, HEIGHT, &rgba)
}

/// A `meta` document for a canvas of `WIDTH` x `HEIGHT` with `frames` frames and the named layers.
pub fn meta(layers: &[&str], frames: u32) -> Vec<u8> {
    let list: Vec<serde_json::Value> = layers
        .iter()
        .enumerate()
        .map(|(i, name)| serde_json::json!({"id": i + 1, "name": name, "blendMode": 3}))
        .collect();
    serde_json::json!({
        "atState": "",
        "canvasWidth": WIDTH,
        "canvasHeight": HEIGHT,
        "format": 1,
        "fps": 12,
        "frameCount": frames,
        "layersState": serde_json::to_string(&list).expect("json"),
        "name": "Test",
        "version": 4,
    })
    .to_string()
    .into_bytes()
}

/// A zip of `files`, each stored under its name.
pub fn zip_of(files: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, data) in files {
        zip.start_file(*name, options).expect("starts");
        zip.write_all(data).expect("writes");
    }
    zip.finish().expect("finishes").into_inner()
}

/// A project: `meta`, then `bg` if given, then the named pictures.
pub fn project(
    meta: Vec<u8>,
    background: Option<Vec<u8>>,
    pictures: &[(&str, Vec<u8>)],
) -> Vec<u8> {
    let mut files: Vec<(&str, Vec<u8>)> = vec![("meta", meta)];
    if let Some(bg) = background {
        files.push(("bg", bg));
    }
    files.extend(pictures.iter().map(|(n, d)| (*n, d.clone())));
    zip_of(&files)
}
