//! Reading a movie's script blocks from untrusted bytes, and the movies that are refused.
#![allow(clippy::expect_used)]

use flare_as3::fixtures::{HOST, movie};
use flare_as3::{Movie, Replacement, ScriptError, StringQuery, patch_strings, strings};
use swf::{Compression, Header, Rectangle, Tag, Twips};

fn header() -> Header {
    Header {
        compression: Compression::None,
        version: 10,
        stage_size: Rectangle {
            x_min: Twips::ZERO,
            x_max: Twips::from_pixels(100.0),
            y_min: Twips::ZERO,
            y_max: Twips::from_pixels(100.0),
        },
        frame_rate: swf::Fixed8::from_f32(24.0),
        num_frames: 1,
    }
}

fn written(tags: &[Tag<'_>]) -> Vec<u8> {
    let mut out = Vec::new();
    swf::write_swf(&header(), tags, &mut out).expect("write");
    out
}

/// The fixture's first block, as ABC bytes.
fn main_abc() -> Vec<u8> {
    let bytes = movie(Compression::None).expect("fixture");
    let buf = swf::decompress_swf(&bytes[..]).expect("decompress");
    let parsed = swf::parse_swf(&buf).expect("parse");
    parsed
        .tags
        .iter()
        .find_map(|t| match t {
            Tag::DoAbc2(abc) => Some(abc.data.to_vec()),
            _ => None,
        })
        .expect("a block")
}

#[test]
fn what_is_not_a_movie_or_has_no_scripts_is_refused_by_name() {
    assert!(matches!(
        Movie::read(b"PK\x03\x04 a zip"),
        Err(ScriptError::NotSwf(_))
    ));
    let mut huge = b"FWS\x0a".to_vec();
    huge.extend_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        Movie::read(&huge),
        Err(ScriptError::TooLarge { .. })
    ));
    assert!(matches!(
        Movie::read(&written(&[Tag::ShowFrame])),
        Err(ScriptError::NoScripts)
    ));
}

#[test]
fn a_tag_running_past_the_end_is_refused() {
    let mut bytes = written(&[Tag::ShowFrame]);
    // Replace the End tag with a long tag that claims a million bytes.
    bytes.truncate(bytes.len() - 2);
    bytes.extend_from_slice(&((82u16 << 6) | 0x3f).to_le_bytes());
    bytes.extend_from_slice(&1_000_000u32.to_le_bytes());
    let len = bytes.len() as u32;
    bytes[4..8].copy_from_slice(&len.to_le_bytes());
    assert!(matches!(
        Movie::read(&bytes),
        Err(ScriptError::Truncated(_))
    ));
}

#[test]
fn a_first_form_doabc_block_is_read_and_patched_too() {
    let abc = main_abc();
    let bytes = written(&[Tag::DoAbc(&abc), Tag::ShowFrame]);
    let movie = Movie::read(&bytes).expect("read");
    assert_eq!(movie.blocks()[0].name, "");
    let patched = patch_strings(
        &movie,
        &[Replacement {
            from: HOST.into(),
            to: "new.example".into(),
        }],
        false,
    )
    .expect("patch");
    let again = Movie::read(&patched.bytes).expect("read again");
    let found = strings(
        &again,
        &StringQuery {
            find: Some("new.example".into()),
            ..StringQuery::default()
        },
    );
    assert_eq!(found.matched, 1);
}

#[test]
fn a_block_that_cannot_be_read_is_reported_and_not_patched() {
    let abc = main_abc();
    // Cut the block just after its strings: they can still be found, the rest cannot be read.
    let last = abc
        .windows(5)
        .position(|w| w == b"trace")
        .expect("the last string")
        + 5;
    let cut = &abc[..last];
    let bytes = written(&[Tag::DoAbc(cut), Tag::ShowFrame]);
    let movie = Movie::read(&bytes).expect("the movie itself reads");
    let listed = strings(&movie, &StringQuery::default());
    assert_eq!(listed.unreadable.len(), 1);
    assert!(listed.strings.is_empty());
    let patch = [Replacement {
        from: HOST.into(),
        to: "x".into(),
    }];
    assert!(matches!(
        patch_strings(&movie, &patch, false),
        Err(ScriptError::Unreadable { block: 0, .. })
    ));
}
