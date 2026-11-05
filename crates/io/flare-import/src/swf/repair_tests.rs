#![allow(clippy::expect_used)]

use super::{MAX_SPRITE_NESTING, RepairError, repair};

const MAX: usize = 1 << 20;

/// A short-form tag: code and length in one u16, then the body.
fn short(code: u16, body: &[u8]) -> Vec<u8> {
    let mut out = ((code << 6) | body.len() as u16).to_le_bytes().to_vec();
    out.extend_from_slice(body);
    out
}

/// A long-form tag: the length follows the header as a u32.
fn long(code: u16, body: &[u8]) -> Vec<u8> {
    let mut out = ((code << 6) | 0x3f).to_le_bytes().to_vec();
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(body);
    out
}

fn end() -> Vec<u8> {
    vec![0, 0]
}

fn show_frame() -> Vec<u8> {
    short(1, &[])
}

/// A DefineSprite of `id` with `frames` frames and the given body tags.
fn sprite(id: u16, frames: u16, tags: &[u8]) -> Vec<u8> {
    let mut body = id.to_le_bytes().to_vec();
    body.extend_from_slice(&frames.to_le_bytes());
    body.extend_from_slice(tags);
    long(39, &body)
}

fn cat(parts: &[Vec<u8>]) -> Vec<u8> {
    parts.concat()
}

#[test]
fn a_complete_stream_is_returned_unchanged() {
    let data = cat(&[
        sprite(1, 1, &[show_frame(), end()].concat()),
        show_frame(),
        end(),
    ]);
    let out = repair(&data, MAX).expect("repairs");
    assert_eq!(out.data, data);
    assert_eq!(out.ends_added, 0);
}

#[test]
fn a_movie_without_its_end_record_gets_one() {
    let data = cat(&[show_frame(), show_frame()]);
    let out = repair(&data, MAX).expect("repairs");
    assert_eq!(out.data, cat(&[show_frame(), show_frame(), end()]));
    assert_eq!(out.ends_added, 1);
}

#[test]
fn a_sprite_without_its_end_record_gets_one_and_a_new_length() {
    let data = cat(&[sprite(7, 2, &show_frame()), end()]);
    let out = repair(&data, MAX).expect("repairs");
    let expected = cat(&[sprite(7, 2, &cat(&[show_frame(), end()])), end()]);
    assert_eq!(out.data, expected);
    assert_eq!(out.ends_added, 1);
}

#[test]
fn bytes_after_an_end_record_are_not_copied() {
    let data = cat(&[show_frame(), end(), show_frame()]);
    let out = repair(&data, MAX).expect("repairs");
    assert_eq!(out.data, cat(&[show_frame(), end()]));
    assert_eq!(out.ends_added, 0);
}

#[test]
fn a_single_stray_byte_at_the_end_is_dropped_and_the_end_added() {
    let data = cat(&[show_frame(), vec![0xAB]]);
    let out = repair(&data, MAX).expect("repairs");
    assert_eq!(out.data, cat(&[show_frame(), end()]));
    assert_eq!(out.ends_added, 1);
}

#[test]
fn long_form_tags_are_copied_as_they_are() {
    let data = cat(&[long(12, &[1, 2, 3, 4, 5]), end()]);
    let out = repair(&data, MAX).expect("repairs");
    assert_eq!(out.data, data);
}

#[test]
fn a_tag_running_past_its_block_is_an_error_and_not_guessed_at() {
    let mut data = short(12, &[1, 2, 3]);
    data.truncate(data.len() - 1);
    assert_eq!(repair(&data, MAX), Err(RepairError::TagOverrun(0)));
}

#[test]
fn a_sprite_whose_tags_run_past_the_sprite_is_an_error() {
    // The sprite's body is its id, frame count and a tag header that claims 10 bytes the sprite
    // does not have, though the movie around it does.
    let mut body = vec![1, 0, 1, 0];
    body.extend_from_slice(&((12_u16 << 6) | 10).to_le_bytes());
    let data = long(39, &body);
    assert_eq!(repair(&data, MAX), Err(RepairError::TagOverrun(10)));
}

#[test]
fn nesting_at_the_limit_is_read_and_one_level_more_is_refused() {
    let mut inner = end();
    for level in 0..MAX_SPRITE_NESTING {
        inner = sprite(level as u16 + 1, 1, &cat(&[inner, end()]));
    }
    let at_limit = cat(&[inner, end()]);
    assert!(repair(&at_limit, MAX).is_ok());

    let mut deeper = end();
    for level in 0..=MAX_SPRITE_NESTING {
        deeper = sprite(level as u16 + 1, 1, &cat(&[deeper, end()]));
    }
    let past = cat(&[deeper, end()]);
    assert_eq!(repair(&past, MAX), Err(RepairError::TooDeep));
}

#[test]
fn output_past_the_bound_is_refused() {
    let data = cat(&[show_frame(), show_frame()]);
    assert_eq!(repair(&data, 3), Err(RepairError::TooLarge(3)));
}

#[test]
fn an_empty_stream_gets_only_its_end() {
    let out = repair(&[], MAX).expect("repairs");
    assert_eq!(out.data, end());
    assert_eq!(out.ends_added, 1);
}
