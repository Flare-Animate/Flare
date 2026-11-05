//! Unit tests of the picture decoder's internal rules: the budget, the crop, and the colour
//! un-premultiplication, which the integration tests only reach through whole projects.
#![allow(clippy::expect_used)]

use super::frame::{self, Drawn};

/// A 2x1 canvas picture with the given pixels, in the picture layout (24-byte header, zstd).
fn picture(rgba: &[u8]) -> Vec<u8> {
    let payload = zstd::stream::encode_all(rgba, 0).expect("encodes");
    let length = 24 + payload.len();
    let mut out = vec![0x89, b'F', b'C', b'I', b'\r', b'\n', 0x1a, b'\n'];
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&24_u16.to_le_bytes());
    out.extend_from_slice(&(length as u32).to_be_bytes()[1..]);
    out.push(0);
    out.extend_from_slice(&2_u16.to_be_bytes());
    out.extend_from_slice(&1_u16.to_be_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&payload);
    out
}

fn decode(data: &[u8], budget: &mut u64) -> Result<Option<Drawn>, String> {
    frame::decode("test", data, (2, 1), budget).map_err(|e| e.to_string())
}

#[test]
fn a_picture_that_draws_nothing_is_none() {
    let mut budget = 1 << 20;
    let drawn = decode(&picture(&[0; 8]), &mut budget).expect("decodes");
    assert!(drawn.is_none());
}

#[test]
fn the_decoded_bytes_are_charged_against_the_budget() {
    // Two pixels take 8 bytes; a budget of 7 refuses the picture before it is inflated.
    let mut budget = 7;
    let err = decode(&picture(&[9; 8]), &mut budget).expect_err("refused");
    assert!(err.contains("pictures decoded together"), "{err}");
    let mut budget = 8;
    assert!(
        decode(&picture(&[9; 8]), &mut budget)
            .expect("decodes")
            .is_some()
    );
    assert_eq!(budget, 0, "the bytes are spent");
}

#[test]
fn only_the_drawn_part_is_kept_and_it_is_straight() {
    // Pixel 1 is premultiplied 64 at alpha 128; pixel 0 is transparent.
    let mut budget = 1 << 20;
    let drawn = decode(&picture(&[0, 0, 0, 0, 64, 0, 0, 128]), &mut budget)
        .expect("decodes")
        .expect("draws");
    assert_eq!((drawn.x, drawn.y, drawn.width, drawn.height), (1, 0, 1, 1));
    assert_eq!(drawn.rgba, [128, 0, 0, 128]);
}

#[test]
fn full_and_empty_alpha_keep_their_colour() {
    let mut budget = 1 << 20;
    let drawn = decode(&picture(&[10, 20, 30, 255, 40, 50, 60, 0]), &mut budget)
        .expect("decodes")
        .expect("draws");
    assert_eq!(drawn.width, 1);
    assert_eq!(drawn.rgba, [10, 20, 30, 255]);
}
