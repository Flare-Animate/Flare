//! File > Import > Import Brush Presets: the menu item asks for a pen-profile file, the read
//! keeps its presets on the brush list, and what does not map is said. The `.abr` fixture is
//! built by hand, byte for byte, the way `flare-import`'s own tests build theirs.

use super::*;

/// A `.abr` header: version, count.
fn abr_header(version: u16, count: u16) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&version.to_be_bytes());
    out.extend_from_slice(&count.to_be_bytes());
    out
}

/// A sampled brush record: the six-byte head, its name and settings, and a minimal tip.
fn sampled_record(name: Option<&str>, spacing: i16) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&0i32.to_be_bytes());
    body.extend_from_slice(&spacing.to_be_bytes());
    if let Some(name) = name {
        body.extend_from_slice(&(name.chars().count() as u32).to_be_bytes());
        for unit in name.encode_utf16() {
            body.extend_from_slice(&unit.to_be_bytes());
        }
    }
    body.push(1);
    for value in [0i16, 0, 10, 10] {
        body.extend_from_slice(&value.to_be_bytes());
    }
    for value in [0i32, 0, 10, 10] {
        body.extend_from_slice(&value.to_be_bytes());
    }
    body.extend_from_slice(&8i16.to_be_bytes());
    body.push(0);
    body.push(255);
    let mut record = Vec::new();
    record.extend_from_slice(&2i16.to_be_bytes());
    record.extend_from_slice(&(body.len() as i32).to_be_bytes());
    record.extend_from_slice(&body);
    record
}

/// A one-brush Photoshop `.abr`, as a file the picker could hand over.
fn one_brush_abr() -> Vec<u8> {
    let mut bytes = abr_header(2, 1);
    bytes.extend(sampled_record(Some("Round"), 25));
    bytes
}

#[test]
fn import_brushes_is_on_the_file_menu_and_asks_for_a_profile() {
    let mut ws = workspace();
    assert!(ws.is_enabled(MenuAction::ImportBrushProfiles));
    ws.perform(MenuAction::ImportBrushProfiles);
    assert_eq!(ws.take_dialog(), Some(DialogRequest::ImportBrushProfiles));
}

#[test]
fn a_profile_is_kept_on_the_brush_list_and_chosen() {
    let mut ws = workspace();
    let before = ws.brush_shelf.presets().len();
    ws.import_brush_profiles("brushes.abr", &one_brush_abr());
    assert_eq!(ws.brush_shelf.presets().len(), before + 1);
    assert_eq!(ws.brush_shelf.presets().last().expect("kept").name, "Round");
    assert_eq!(
        ws.brush_shelf.selected(),
        Some(ws.brush_shelf.presets().len() - 1),
        "the first preset that landed is in hand"
    );
    assert!(
        ws.log.iter().any(|l| l.contains("1 brush preset")),
        "{:?}",
        ws.log
    );
}

#[test]
fn a_file_that_is_not_a_profile_changes_nothing_and_says_why() {
    let mut ws = workspace();
    let before = ws.brush_shelf.presets().to_vec();
    ws.import_brush_profiles("brushes.abr", b"not a photoshop brush file");
    assert_eq!(ws.brush_shelf.presets(), before, "nothing changed");
    assert!(
        ws.log.iter().any(|l| l.contains("Cannot import")),
        "{:?}",
        ws.log
    );
}

#[test]
fn a_file_with_the_wrong_ending_is_refused_by_its_name() {
    let mut ws = workspace();
    ws.import_brush_profiles("brushes.gbr", &one_brush_abr());
    assert!(
        ws.log
            .iter()
            .any(|l| l.contains("GIMP") && l.contains("nothing to import")),
        "{:?}",
        ws.log
    );
    assert!(ws.brush_shelf.presets().is_empty(), "nothing landed");
}
