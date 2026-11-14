//! A still picture read to look at: the kinds read, the size it is kept at, and what is refused.
#![allow(clippy::expect_used)]

use flare_import::{MAX_STILL_BYTES, STILL_EXTENSIONS, read_still};

fn png(width: u32, height: u32, pixel: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("header");
        let mut data = Vec::new();
        for y in 0..height {
            for x in 0..width {
                data.extend(pixel(x, y));
            }
        }
        writer.write_image_data(&data).expect("data");
    }
    out
}

#[test]
fn a_small_picture_is_kept_as_it_is() {
    let bytes = png(3, 2, |x, y| [x as u8 * 10, y as u8 * 20, 7, 255]);
    let still = read_still("person.PNG", &bytes, 100).expect("reads");
    assert_eq!((still.width, still.height, still.kept), (3, 2, [3, 2]));
    assert_eq!(&still.rgba[..8], &[0, 0, 7, 255, 10, 0, 7, 255]);
}

#[test]
fn a_large_picture_is_kept_smaller_with_its_shape_and_its_own_size_told() {
    // Left half red, right half blue, and a see-through column that must not darken its
    // neighbours when they are averaged together.
    let bytes = png(400, 200, |x, _| match x {
        0..=199 => [255, 0, 0, 255],
        200..=201 => [0, 0, 0, 0],
        _ => [0, 0, 255, 255],
    });
    let still = read_still("p.png", &bytes, 100).expect("reads");
    assert_eq!((still.width, still.height), (400, 200));
    assert_eq!(still.kept, [100, 50]);
    assert_eq!(still.rgba.len(), 100 * 50 * 4);
    let at = |x: usize, y: usize| {
        let i = (y * 100 + x) * 4;
        [
            still.rgba[i],
            still.rgba[i + 1],
            still.rgba[i + 2],
            still.rgba[i + 3],
        ]
    };
    assert_eq!(at(10, 10), [255, 0, 0, 255]);
    assert_eq!(at(90, 40), [0, 0, 255, 255]);
    // The kept pixel over the see-through column and two blue ones is blue, half as opaque.
    assert_eq!(at(50, 10), [0, 0, 255, 127]);
}

#[test]
fn a_wrong_or_unknown_picture_is_refused() {
    let bytes = png(4, 4, |_, _| [1, 2, 3, 255]);
    assert!(
        read_still("p.txt", &bytes, 100).is_err(),
        "not a picture kind"
    );
    assert!(!STILL_EXTENSIONS.contains(&"txt"));
    assert!(read_still("p", &bytes, 100).is_err(), "no ending");
    assert!(read_still("p.jpg", &bytes, 100).is_err(), "not a JPEG");
    assert!(read_still("p.png", &bytes[..20], 100).is_err(), "cut short");
    let huge = vec![0u8; MAX_STILL_BYTES + 1];
    let error = read_still("p.png", &huge, 100).expect_err("too large");
    assert!(error.to_string().contains("too large"), "{error}");
}
