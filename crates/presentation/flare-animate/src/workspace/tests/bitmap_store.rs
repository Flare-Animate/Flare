#![allow(clippy::expect_used)]

//! The Doc tab's Bitmaps rows: how many bitmaps and the room they take, and storing them all as
//! photos (JPEG) or lossless as one undo step, off the window's thread.

use super::layer_names::{click, find, settle};
use super::*;
use egui::PointerButton;
use flare_core::BitmapId;
use flare_document::Bitmap;
use flare_tools::library::Compression;

/// A smooth 96x64 picture, as a frame of video is.
fn frame(level: u8) -> Bitmap {
    let rgba = (0..64u32)
        .flat_map(|y| (0..96u32).flat_map(move |x| [(x * 2) as u8, (y * 3) as u8, level, 255]))
        .collect();
    Bitmap::new(BitmapId::new(), "frame", 96, 64, rgba).expect("bitmap")
}

fn with_frames() -> Workspace {
    let mut ws = workspace();
    ws.active_mut()
        .apply(vec![
            Command::AddBitmap(frame(10)),
            Command::AddBitmap(frame(200)),
        ])
        .expect("bitmaps");
    ws
}

fn photos(ws: &Workspace) -> usize {
    ws.document()
        .bitmaps()
        .iter()
        .filter(|b| b.photo().is_some())
        .count()
}

#[test]
fn apply_in_the_doc_tab_stores_every_bitmap_as_a_photo_in_one_undo_step() {
    let mut ws = with_frames();
    ws.show_props_context(PropsContext::Document);
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    let mut time = 0.0;
    settle(&mut ws, &ctx, &mut time);
    time += 0.1;
    assert!(
        find(&mut ws, &ctx, time, vec![], "2 (0 as photos), 1 MB").is_some(),
        "the count and the room they take"
    );
    time += 0.1;
    let apply = find(&mut ws, &ctx, time, vec![], "Apply to all bitmaps").expect("the button");
    click(&mut ws, &ctx, &mut time, apply, PointerButton::Primary);
    assert_eq!(photos(&ws), 2);
    assert!(
        !ws.is_storing_bitmaps(),
        "with no worker threads it is done at once"
    );
    assert!(
        ws.status().is_some_and(|s| s.contains("Stored 2 bitmaps")),
        "{:?}",
        ws.status()
    );
    ws.undo();
    assert_eq!(photos(&ws), 0, "one undo step");
}

#[test]
fn asking_again_changes_nothing_and_lossless_takes_them_back() {
    let mut ws = with_frames();
    ws.store_bitmaps(Compression::Photo { quality: 80 });
    assert_eq!(photos(&ws), 2);
    let revision = ws.active().revision();
    ws.store_bitmaps(Compression::Photo { quality: 80 });
    assert_eq!(ws.active().revision(), revision);
    assert!(
        ws.status().is_some_and(|s| s.contains("already stored")),
        "{:?}",
        ws.status()
    );
    ws.store_bitmaps(Compression::Lossless);
    assert_eq!(photos(&ws), 0);
}

#[test]
fn a_result_for_a_document_since_changed_is_not_applied() {
    let mut ws = with_frames();
    let stale = ws.active().snapshot();
    let done = flare_tools::library::set_bitmap_compression(
        &stale,
        &[],
        Compression::Photo { quality: 80 },
    )
    .expect("compressed");
    rect(&mut ws);
    ws.bitmap_store.pending = Some(77);
    ws.bitmaps_stored(77, stale, Ok(done));
    assert_eq!(photos(&ws), 0);
    assert!(
        ws.status().is_some_and(|s| s.contains("document changed")),
        "{:?}",
        ws.status()
    );
    // A result nobody is waiting for any more is dropped without a word.
    ws.status = None;
    ws.bitmaps_stored(78, ws.active().snapshot(), Err("late".into()));
    assert!(ws.status().is_none());
}

#[test]
fn a_document_without_bitmaps_says_none_and_offers_nothing() {
    let mut ws = workspace();
    ws.show_props_context(PropsContext::Document);
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    let mut time = 0.0;
    settle(&mut ws, &ctx, &mut time);
    time += 0.1;
    assert!(find(&mut ws, &ctx, time, vec![], "Apply to all bitmaps").is_none());
    ws.store_bitmaps(Compression::Lossless);
    assert!(!ws.is_storing_bitmaps());
}
