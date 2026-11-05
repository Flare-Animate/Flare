//! KWZ tests: the line tables, the tile types on hand-worked blocks, the sections and records,
//! the refusals, and the import. Expected pixels are worked out by hand in each test.

#![allow(clippy::expect_used)]

use super::lines::LineTables;
use super::synth::{Frame, Options, blank_blocks, kwz, layer, line_code};
use super::tiles::{LAYER_WIDTH, decode_layer};
use super::{Kwz, KwzState};
use crate::flipnote::error::FlipnoteError;

const W: usize = LAYER_WIDTH;

fn at(out: &[u8], x: usize, y: usize) -> u8 {
    out[y * W + x]
}

fn blank_layer() -> Vec<u8> {
    layer(&blank_blocks())
}

#[test]
fn line_table_golden_entries() {
    let t = LineTables::new();
    // Code 729 is 3^6: digit b is 1, the rest 0, so the pixels are [b, a, ...] = [1, 0, 0, ...].
    assert_eq!(t.full[729], [1, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(t.full_shift[729], [0, 0, 0, 0, 0, 0, 0, 1]);
    assert_eq!(t.full[0], [0; 8]);
    assert_eq!(t.full[6560], [2; 8]);
    // Common line 1 is code 0x0CD0 = 3280, all digits 1: a line of ones.
    assert_eq!(t.common[1], [1; 8]);
    assert_eq!(t.common_shift[1], [1; 8]);
}

#[test]
fn every_line_code_is_its_own_inverse() {
    let t = LineTables::new();
    for (code, line) in t.full.iter().enumerate() {
        assert_eq!(line_code(*line), code as u32);
    }
}

#[test]
fn blank_blocks_decode_to_zero() {
    let mut out = vec![9_u8; W * 240];
    let used = decode_layer(&blank_layer(), &mut out, &LineTables::new()).expect("decodes");
    assert!(out.iter().all(|&v| v == 0));
    assert!(used > 0);
}

#[test]
fn a_full_line_fills_all_eight_rows() {
    let pixels = [1, 2, 0, 0, 1, 1, 2, 0];
    let mut blocks = blank_blocks();
    blocks[0] = vec![(1, 3), (line_code(pixels), 13)];
    let mut out = vec![0_u8; W * 240];
    decode_layer(&layer(&blocks), &mut out, &LineTables::new()).expect("decodes");
    for y in 0..8 {
        for (x, &want) in pixels.iter().enumerate() {
            assert_eq!(at(&out, x, y), want, "row {y} column {x}");
        }
    }
    // The next block is blank.
    assert_eq!(at(&out, 8, 0), 0);
}

#[test]
fn the_most_common_tile_mixes_common_and_full_lines_by_flag() {
    // Block 1 (x = 8): flags 0b1 means row 0 is common line 1 (all ones); rows 1 to 7 are the
    // full line of all twos, code 6560.
    let mut blocks = blank_blocks();
    let mut codes = vec![(4, 3), (1, 8)];
    codes.push((1, 5));
    for _ in 1..8 {
        codes.push((6560, 13));
    }
    blocks[1] = codes;
    let mut out = vec![0_u8; W * 240];
    decode_layer(&layer(&blocks), &mut out, &LineTables::new()).expect("decodes");
    for x in 8..16 {
        assert_eq!(at(&out, x, 0), 1);
        for y in 1..8 {
            assert_eq!(at(&out, x, y), 2);
        }
    }
    assert_eq!(at(&out, 16, 0), 0);
}

#[test]
fn pattern_tile_with_common_lines_follows_its_row_order() {
    // Block 2 (x = 16): pattern bits 0 plus the common flag makes pattern 1, the rows a, a, b,
    // a, a, b, a, a. a is common line 1 (ones), b is common line 0 (zeros).
    let mut blocks = blank_blocks();
    blocks[2] = vec![(7, 3), (0, 2), (1, 1), (1, 5), (0, 5)];
    let mut out = vec![0_u8; W * 240];
    decode_layer(&layer(&blocks), &mut out, &LineTables::new()).expect("decodes");
    let ones_rows = [0, 1, 3, 4, 6, 7];
    for y in 0..8 {
        let want = u8::from(ones_rows.contains(&y));
        assert_eq!(at(&out, 16, y), want, "row {y}");
    }
}

#[test]
fn alternating_rows_take_the_shifted_line() {
    // Block 3 (x = 24): full code 729 on even rows, its shifted form on odd rows.
    let mut blocks = blank_blocks();
    blocks[3] = vec![(3, 3), (729, 13)];
    let mut out = vec![0_u8; W * 240];
    decode_layer(&layer(&blocks), &mut out, &LineTables::new()).expect("decodes");
    for y in 0..8 {
        if y % 2 == 0 {
            assert_eq!(at(&out, 24, y), 1, "row {y} starts with the pixel");
            assert_eq!(at(&out, 31, y), 0);
        } else {
            assert_eq!(at(&out, 24, y), 0);
            assert_eq!(at(&out, 31, y), 1, "row {y} ends with the pixel");
        }
    }
}

#[test]
fn a_skip_keeps_the_pixels_from_the_layer_before() {
    let mut out = vec![0_u8; W * 240];
    let mut first = blank_blocks();
    first[0] = vec![(1, 3), (line_code([1; 8]), 13)];
    decode_layer(&layer(&first), &mut out, &LineTables::new()).expect("first");
    // The second layer's block 0 is a skip code with a count of zero: it writes nothing.
    let mut second = blank_blocks();
    second[0] = vec![(5, 3), (0, 5)];
    decode_layer(&layer(&second), &mut out, &LineTables::new()).expect("second");
    assert_eq!(at(&out, 0, 0), 1);
    assert_eq!(at(&out, 7, 7), 1);
}

#[test]
fn an_out_of_table_code_is_refused() {
    let mut blocks = blank_blocks();
    blocks[0] = vec![(1, 3), (7000, 13)];
    let mut out = vec![0_u8; W * 240];
    let err = decode_layer(&layer(&blocks), &mut out, &LineTables::new()).expect_err("refused");
    assert_eq!(err, FlipnoteError::BadLineCode(7000));
}

#[test]
fn a_layer_too_short_for_its_blocks_is_refused() {
    let full = blank_layer();
    let mut out = vec![0_u8; W * 240];
    let err = decode_layer(&full[..10], &mut out, &LineTables::new()).expect_err("refused");
    assert!(matches!(err, FlipnoteError::Truncated { .. }), "{err:?}");
}

fn frames_of_blank(n: usize) -> Vec<Frame> {
    (0..n)
        .map(|_| Frame {
            flags: 0,
            layers: [blank_layer(), blank_layer(), blank_layer()],
        })
        .collect()
}

#[test]
fn a_file_parses_its_header_and_frames_and_decodes_them() {
    let options = Options {
        speed: 2,
        layer_flags: 0b001,
        ..Options::default()
    };
    let bytes = kwz(&frames_of_blank(2), &options);
    let file = Kwz::parse(&bytes).expect("parses");
    assert_eq!(file.frame_count(), 2);
    assert_eq!(file.frame_rate(), (1, 1));
    assert_eq!(file.meta().layer_visible, [false, true, false]);
    let mut state = KwzState::new();
    for i in 0..2 {
        let info = file.decode_frame(&mut state, i).expect("decodes");
        assert!(info.consumed.iter().all(|&c| c > 0));
    }
    assert!(state.layers().iter().all(|l| l.iter().all(|&v| v == 0)));
}

#[test]
fn a_comment_has_no_sound_section() {
    let options = Options {
        sound: None,
        ..Options::default()
    };
    let bytes = kwz(&frames_of_blank(1), &options);
    let file = Kwz::parse(&bytes).expect("parses");
    assert!(file.sound().is_none());
}

#[test]
fn a_zero_frame_count_is_refused() {
    let options = Options {
        frame_count: Some(0),
        ..Options::default()
    };
    let bytes = kwz(&frames_of_blank(1), &options);
    assert_eq!(Kwz::parse(&bytes).err(), Some(FlipnoteError::NoFrames));
}

#[test]
fn a_bad_speed_is_refused() {
    let options = Options {
        speed: 11,
        ..Options::default()
    };
    let bytes = kwz(&frames_of_blank(1), &options);
    assert_eq!(Kwz::parse(&bytes).err(), Some(FlipnoteError::BadSpeed(11)));
}

#[test]
fn a_missing_layer_section_is_refused() {
    let mut bytes = kwz(&frames_of_blank(1), &Options::default());
    let at = bytes.windows(3).position(|w| w == b"KMC").expect("present");
    bytes[at..at + 3].copy_from_slice(b"ZZZ");
    assert_eq!(
        Kwz::parse(&bytes).err(),
        Some(FlipnoteError::MissingSection("KMC"))
    );
}

#[test]
fn a_frame_count_beyond_the_frame_table_is_refused() {
    let options = Options {
        frame_count: Some(5),
        ..Options::default()
    };
    let bytes = kwz(&frames_of_blank(2), &options);
    assert!(matches!(
        Kwz::parse(&bytes).err(),
        Some(FlipnoteError::FrameTableShort {
            entries: 2,
            frames: 5
        })
    ));
}

#[test]
fn a_file_shorter_than_its_signature_has_no_sections() {
    // The sections end where the 256-byte signature starts, so a short file has none.
    let bytes = kwz(&frames_of_blank(1), &Options::default());
    assert_eq!(
        Kwz::parse(&bytes[..100]).err(),
        Some(FlipnoteError::MissingSection("KFH"))
    );
}

#[test]
fn a_section_longer_than_the_file_is_refused() {
    let mut bytes = kwz(&frames_of_blank(1), &Options::default());
    let at = bytes
        .windows(4)
        .position(|w| w == b"KMC\x14")
        .expect("present");
    bytes[at + 4..at + 8].copy_from_slice(&0x7FFF_FFFF_u32.to_le_bytes());
    assert!(matches!(
        Kwz::parse(&bytes).err(),
        Some(FlipnoteError::SectionOutside { .. })
    ));
}

#[test]
fn a_layer_size_past_the_layer_section_is_refused() {
    let mut bytes = kwz(&frames_of_blank(1), &Options::default());
    let at = bytes
        .windows(4)
        .position(|w| w == b"KMI\x14")
        .expect("present");
    bytes[at + 8 + 4..at + 8 + 6].copy_from_slice(&0xFFFF_u16.to_le_bytes());
    assert!(matches!(
        Kwz::parse(&bytes).err(),
        Some(FlipnoteError::SectionOutside { .. })
    ));
}

#[test]
fn a_kwz_imports_as_four_layers_and_reports_its_sound() {
    let options = Options {
        sound: Some((3, [100, 0, 0, 0, 0])),
        ..Options::default()
    };
    let bytes = kwz(&frames_of_blank(2), &options);
    let imported = crate::import_bytes(bytes).expect("imports");
    let scene = &imported.document.scenes()[0];
    assert_eq!(scene.timeline().frame_count(), 2);
    let names: Vec<&str> = scene.timeline().layers().iter().map(|l| l.name()).collect();
    assert_eq!(names, vec!["Layer 1", "Layer 2", "Layer 3", "Paper"]);
    assert!(
        imported
            .diagnostics
            .iter()
            .any(|d| d.message.contains("sound is not imported")
                && d.message.contains("BGM speed 3"))
    );
}

#[test]
fn a_comment_imports_with_a_note_and_no_sound() {
    let options = Options {
        sound: None,
        ..Options::default()
    };
    let bytes = kwz(&frames_of_blank(1), &options);
    let imported = crate::import_bytes(bytes).expect("imports");
    assert!(
        imported
            .diagnostics
            .iter()
            .any(|d| d.message.contains("a comment"))
    );
}
