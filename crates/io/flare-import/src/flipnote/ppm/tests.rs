#![allow(clippy::expect_used)]
//! PPM header, frame and sound tests. The files are built by `synth`, laid out as the format is,
//! and every failure path has a test of its own.

use super::adpcm;
use super::synth::{SynthFrame, encode, wrap};
use super::{FrameDecoder, HEIGHT, Ppm, WIDTH};
use crate::flipnote::error::FlipnoteError;

const PIXELS: usize = WIDTH * HEIGHT;

fn blank() -> Vec<u8> {
    vec![0; PIXELS]
}

/// Sets the pixels of a picture from `(x, y)` pairs.
fn dots(points: &[(usize, usize)]) -> Vec<u8> {
    let mut out = blank();
    for &(x, y) in points {
        out[y * WIDTH + x] = 1;
    }
    out
}

fn key(layers: [Vec<u8>; 2]) -> SynthFrame {
    SynthFrame {
        keyframe: true,
        inverted: false,
        pens: [1, 2],
        shift: None,
        layers,
    }
}

/// Decodes every frame of `bytes`, returning the layers of each.
fn decode_all(bytes: &[u8]) -> Result<Vec<[Vec<u8>; 2]>, FlipnoteError> {
    let ppm = Ppm::parse(bytes)?;
    let mut dec = FrameDecoder::new();
    let mut out = Vec::new();
    for i in 0..ppm.frame_count() {
        ppm.decode_frame(&mut dec, i)?;
        out.push(dec.layers().clone());
    }
    Ok(out)
}

#[test]
fn keyframes_round_trip_through_the_layers() {
    let a = [dots(&[(0, 0), (7, 3), (255, 191)]), dots(&[(100, 50)])];
    let b = [dots(&[(1, 1), (2, 2)]), blank()];
    let prev = [blank(), blank()];
    let frames = vec![
        encode(&key(a.clone()), &prev, None),
        encode(&key(b.clone()), &prev, None),
    ];
    let file = wrap(&frames, &[0, 0], 2, &[], 0);
    let decoded = decode_all(&file).expect("decodes");
    assert_eq!(decoded, vec![a, b]);
}

#[test]
fn delta_frames_xor_the_previous_frame() {
    let a = [dots(&[(3, 3), (4, 4)]), dots(&[(9, 9)])];
    let b = [dots(&[(4, 4), (5, 5)]), dots(&[])];
    let prev = a.clone();
    let delta = SynthFrame {
        keyframe: false,
        inverted: false,
        pens: [1, 2],
        shift: None,
        layers: b.clone(),
    };
    let file = wrap(
        &[
            encode(&key(a.clone()), &[blank(), blank()], None),
            encode(&delta, &prev, None),
        ],
        &[0, 0],
        2,
        &[],
        0,
    );
    let decoded = decode_all(&file).expect("decodes");
    assert_eq!(decoded[1], b);
}

#[test]
fn a_zero_delta_payload_leaves_the_previous_picture() {
    // A delta with no bits set changes nothing, so the second frame is the first one.
    let a = [dots(&[(10, 10)]), dots(&[(20, 20)])];
    let zero = [blank(), blank()];
    let delta = SynthFrame {
        keyframe: false,
        inverted: false,
        pens: [1, 2],
        shift: None,
        layers: zero.clone(),
    };
    let file = wrap(
        &[
            encode(&key(a.clone()), &[blank(), blank()], None),
            encode(&delta, &a, Some(zero)),
        ],
        &[0, 0],
        2,
        &[],
        0,
    );
    let decoded = decode_all(&file).expect("decodes");
    assert_eq!(decoded[1], a);
}

#[test]
fn a_translated_delta_moves_the_previous_picture() {
    // With payload zero, the picture is the previous one moved by (dx, dy): a pixel at (5, 5)
    // lands at (6, 7) for a shift of (1, 2).
    let a = [dots(&[(5, 5)]), blank()];
    let file = wrap(
        &[
            encode(&key(a.clone()), &[blank(), blank()], None),
            encode(
                &SynthFrame {
                    keyframe: false,
                    inverted: false,
                    pens: [1, 2],
                    shift: Some((1, 2)),
                    layers: [blank(), blank()],
                },
                &a,
                Some([blank(), blank()]),
            ),
        ],
        &[0, 0],
        2,
        &[],
        0,
    );
    let decoded = decode_all(&file).expect("decodes");
    assert_eq!(decoded[1][0], dots(&[(6, 7)]));
}

#[test]
fn compressed_line_types_decode() {
    // Frame 0: layer 1 line 0 is type 1, with chunks 0 and 1 present: pixels 0, 2 and 15.
    let mut frame = vec![0x80_u8];
    let mut types = [0_u8; HEIGHT];
    types[0] = 1;
    frame.push(types[0]); // first group: four line types, line 0 in the low bits
    frame.extend(std::iter::repeat_n(0, HEIGHT / 4 - 1)); // blank groups
    frame.extend(std::iter::repeat_n(0, HEIGHT / 4)); // layer 2: all blank
    frame.extend_from_slice(&0xC000_0000_u32.to_be_bytes());
    frame.push(0b0000_0101);
    frame.push(0b1000_0000);
    let file = wrap(&[frame], &[0], 2, &[], 0);
    let decoded = decode_all(&file).expect("decodes");
    let row = &decoded[0][0][..WIDTH];
    let set: Vec<usize> = row
        .iter()
        .enumerate()
        .filter(|(_, v)| **v == 1)
        .map(|(x, _)| x)
        .collect();
    assert_eq!(set, vec![0, 2, 15]);
    assert!(decoded[0][1].iter().all(|&v| v == 0));
}

#[test]
fn inverted_line_type_starts_from_all_set() {
    // Layer 2 line 0 is type 2: every pixel starts set, and the one chunk clears pixel 0 only.
    let mut frame = vec![0x80_u8];
    frame.extend(std::iter::repeat_n(0, HEIGHT / 4)); // layer 1: blank
    let mut types = [0_u8; HEIGHT];
    types[0] = 2;
    frame.push(types[0]);
    frame.extend(std::iter::repeat_n(0, HEIGHT / 4 - 1));
    frame.extend_from_slice(&0x8000_0000_u32.to_be_bytes());
    frame.push(0b1111_1110);
    let file = wrap(&[frame], &[0], 2, &[], 0);
    let decoded = decode_all(&file).expect("decodes");
    let row = &decoded[0][1][..WIDTH];
    assert_eq!(row[0], 0);
    assert!(row[1..].iter().all(|&v| v == 1));
}

#[test]
fn palette_and_keyframe_come_from_the_header_byte() {
    // Header 0x80: keyframe, not inverted (bit 0 clear would make it inverted), pens 2 and 3.
    let frame = encode(
        &SynthFrame {
            keyframe: true,
            inverted: true,
            pens: [2, 3],
            shift: None,
            layers: [blank(), blank()],
        },
        &[blank(), blank()],
        None,
    );
    assert_eq!(frame[0] & 1, 0, "inverted clears bit 0");
    let file = wrap(&[frame], &[0], 2, &[], 0);
    let ppm = Ppm::parse(&file).expect("parses");
    let mut dec = FrameDecoder::new();
    let info = ppm.decode_frame(&mut dec, 0).expect("decodes");
    assert!(info.keyframe);
    assert_eq!(info.palette, [1, 2, 3]);
}

#[test]
fn speed_gives_the_frame_rate_and_the_meta_reads_names_and_flags() {
    let frame = encode(&key([blank(), blank()]), &[blank(), blank()], None);
    // Speed byte 0 is in-app speed 8: 30 frames a second.
    let mut file = wrap(&[frame], &[0], 0, &[], 0x0010);
    assert_eq!(Ppm::parse(&file).expect("parses").frame_rate(), (30, 1));
    // Patch the meta: lock 1, current name "Ab", current FSID bytes 1..8, layer 1 hidden.
    file[0x10..0x12].copy_from_slice(&1_u16.to_le_bytes());
    file[0x40..0x44].copy_from_slice(&[0x41, 0, 0x62, 0]);
    file[0x5E..0x66].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
    file[0x6A6..0x6A8].copy_from_slice(&0x10_u16.to_le_bytes());
    let ppm = Ppm::parse(&file).expect("parses");
    let meta = ppm.meta().expect("meta present");
    assert!(meta.locked);
    assert_eq!(meta.current.name, "Ab");
    assert_eq!(meta.current.fsid, "0807060504030201");
    assert_eq!(meta.layer_visible, [false, true]);
    assert_eq!(meta.created_unix, 946_684_800);
}

#[test]
fn effect_flags_and_bgm_track_are_read_from_their_places() {
    let frame = encode(&key([blank(), blank()]), &[blank(), blank()], None);
    let bgm = [0_u8, 0, 0, 0, 0x11, 0x22];
    let file = wrap(&[frame.clone(), frame], &[0b101, 0b010], 2, &bgm, 0);
    let ppm = Ppm::parse(&file).expect("parses");
    assert!(ppm.effect_on(0, 0).expect("flag"));
    assert!(!ppm.effect_on(0, 1).expect("flag"));
    assert!(ppm.effect_on(1, 1).expect("flag"));
    let section = ppm.sound().bgm;
    assert_eq!(ppm.section_bytes(section).expect("track"), &bgm[..]);
}

#[test]
fn thumbnail_is_1536_bytes() {
    let frame = encode(&key([blank(), blank()]), &[blank(), blank()], None);
    let file = wrap(&[frame], &[0], 2, &[], 0);
    assert_eq!(
        Ppm::parse(&file)
            .expect("parses")
            .thumbnail_bytes()
            .expect("thumb")
            .len(),
        1536
    );
}

#[test]
fn bad_magic_is_refused_by_name() {
    let mut file = wrap(
        &[encode(&key([blank(), blank()]), &[blank(), blank()], None)],
        &[0],
        2,
        &[],
        0,
    );
    file[0] = b'X';
    assert_eq!(Ppm::parse(&file).err(), Some(FlipnoteError::NotPpm));
}

#[test]
fn a_truncated_file_is_refused() {
    let file = wrap(
        &[encode(
            &key([dots(&[(1, 1)]), blank()]),
            &[blank(), blank()],
            None,
        )],
        &[0],
        2,
        &[],
        0,
    );
    assert!(Ppm::parse(&file[..0x200]).is_err());
    assert!(Ppm::parse(&file[..8]).is_err());
}

#[test]
fn a_frame_that_runs_past_the_file_is_refused_when_decoded() {
    // Point frame 0 at the last three bytes: its line types need far more than that.
    let mut file = wrap(
        &[encode(
            &key([dots(&[(1, 1)]), blank()]),
            &[blank(), blank()],
            None,
        )],
        &[0],
        2,
        &[],
        0,
    );
    let at = (file.len() - 3) as u32;
    let base = 0x6A8 + 4;
    file[0x6A8..0x6AC].copy_from_slice(&(at - base as u32).to_le_bytes());
    let ppm = Ppm::parse(&file).expect("the table and sound header are intact");
    let mut dec = FrameDecoder::new();
    assert!(matches!(
        ppm.decode_frame(&mut dec, 0),
        Err(FlipnoteError::Truncated { .. })
    ));
}

#[test]
fn zero_frames_are_refused() {
    let mut file = wrap(
        &[encode(&key([blank(), blank()]), &[blank(), blank()], None)],
        &[0],
        2,
        &[],
        0,
    );
    file[0x6A0..0x6A2].copy_from_slice(&0_u16.to_le_bytes());
    assert_eq!(Ppm::parse(&file).err(), Some(FlipnoteError::NoFrames));
}

#[test]
fn an_oversized_frame_count_is_refused() {
    let mut file = wrap(
        &[encode(&key([blank(), blank()]), &[blank(), blank()], None)],
        &[0],
        2,
        &[],
        0,
    );
    file[12..14].copy_from_slice(&0xFFFF_u16.to_le_bytes());
    assert_eq!(
        Ppm::parse(&file).err(),
        Some(FlipnoteError::TooManyFrames(65_536, super::MAX_FRAMES))
    );
}

#[test]
fn a_frame_pointer_outside_the_file_is_refused() {
    let mut file = wrap(
        &[encode(&key([blank(), blank()]), &[blank(), blank()], None)],
        &[0],
        2,
        &[],
        0,
    );
    file[0x6A8..0x6AC].copy_from_slice(&0x00FF_FFFF_u32.to_le_bytes());
    assert!(matches!(
        Ppm::parse(&file).err(),
        Some(FlipnoteError::FrameOutside { frame: 0, .. })
    ));
}

#[test]
fn a_short_frame_table_is_refused() {
    let mut file = wrap(
        &[
            encode(&key([blank(), blank()]), &[blank(), blank()], None),
            encode(&key([blank(), blank()]), &[blank(), blank()], None),
        ],
        &[0, 0],
        2,
        &[],
        0,
    );
    file[0x6A0..0x6A2].copy_from_slice(&4_u16.to_le_bytes());
    assert_eq!(
        Ppm::parse(&file).err(),
        Some(FlipnoteError::FrameTableShort {
            entries: 1,
            frames: 2
        })
    );
}

#[test]
fn a_bad_speed_byte_is_refused() {
    let file = wrap(
        &[encode(&key([blank(), blank()]), &[blank(), blank()], None)],
        &[0],
        9,
        &[],
        0,
    );
    assert_eq!(Ppm::parse(&file).err(), Some(FlipnoteError::BadSpeed(9)));
}

#[test]
fn a_track_outside_the_file_is_refused() {
    let mut file = wrap(
        &[encode(&key([blank(), blank()]), &[blank(), blank()], None)],
        &[0],
        2,
        &[1, 2],
        0,
    );
    let sound_at = file.len() - 2 - 32;
    file[sound_at..sound_at + 4].copy_from_slice(&0xFFFF_u32.to_le_bytes());
    assert!(matches!(
        Ppm::parse(&file).err(),
        Some(FlipnoteError::SectionOutside { .. })
    ));
}

#[test]
fn adpcm_decodes_known_nibbles() {
    // Predictor 0, step index 0. Nibble 7: diff 0+1+3+7 = 11; the next step is 16; nibble 0 adds 2.
    assert_eq!(adpcm::decode(&[0, 0, 0, 0, 0x07]), vec![11, 13]);
    // Nibble 15 (sign and 7) subtracts 11, then nibble 0 adds 2.
    assert_eq!(adpcm::decode(&[0, 0, 0, 0, 0x0F]), vec![-11, -9]);
    // A block header alone is silence, and an empty track has no samples.
    assert_eq!(adpcm::decode(&[0, 0, 0, 0]), Vec::<i16>::new());
    assert_eq!(adpcm::decode(&[]), Vec::<i16>::new());
}

#[test]
fn adpcm_stays_in_range_over_a_long_run() {
    let mut track = vec![0xFF, 0x7F, 88, 0];
    track.extend(std::iter::repeat_n(0x77, 4000));
    let out = adpcm::decode(&track);
    assert_eq!(out.len(), 8000);
    assert!(
        out.iter()
            .all(|&s| (-32768..=32767).contains(&i32::from(s)))
    );
}
