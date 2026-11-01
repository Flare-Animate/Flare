//! How a selected Graphic plays: the Looping section of Properties.

use super::*;
use flare_document::{PlayMode, Playback, SymbolKind};

/// A rectangle turned into a symbol of `kind`, with the instance selected.
fn selected_instance(kind: SymbolKind) -> Workspace {
    let mut ws = workspace();
    ws.new_symbol_kind = kind;
    draw_and_select(&mut ws);
    ws.convert_selection_to_symbol();
    assert!(ws.selected.is_some(), "the new instance is selected");
    ws
}

fn drawn(ws: &mut Workspace) -> Vec<String> {
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    let mut all = Vec::new();
    for step in 0..4 {
        let mut out = ctx.run_ui(input(f64::from(step) * 0.1, vec![]), |ui| ws.show(ui));
        out.textures_delta.clear();
        all = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) => Some(t.galley.text().to_owned()),
                _ => None,
            })
            .collect();
    }
    all
}

#[test]
fn a_selected_graphic_shows_its_setting_and_changing_it_is_one_undo_step() {
    let mut ws = selected_instance(SymbolKind::Graphic);
    let (now, frames) = ws.selected_playback().expect("a graphic has one");
    assert_eq!(now, Playback::default());
    assert!(frames >= 1);
    let want = Playback {
        mode: PlayMode::PlayOnce,
        first_frame: 0,
    };
    ws.set_selection_playback(want);
    assert_eq!(ws.selected_playback().map(|(p, _)| p), Some(want));
    ws.undo();
    assert_eq!(
        ws.selected_playback().map(|(p, _)| p),
        Some(Playback::default())
    );
    ws.redo();
    assert_eq!(
        ws.selected_playback().map(|(p, _)| p),
        Some(want),
        "and redone"
    );
}

#[test]
fn a_movie_clip_a_button_artwork_or_nothing_has_no_looping_section() {
    for kind in [SymbolKind::MovieClip, SymbolKind::Button] {
        let mut ws = selected_instance(kind);
        assert!(ws.selected_playback().is_none(), "{kind:?}");
        ws.set_selection_playback(Playback {
            mode: PlayMode::SingleFrame,
            first_frame: 0,
        });
        assert!(
            ws.status().is_some_and(|s| s.contains("Graphic")),
            "{kind:?} says why"
        );
        let text = drawn(&mut ws);
        assert!(!text.iter().any(|t| t == "Looping"), "{kind:?}: {text:?}");
    }
    let mut ws = workspace();
    draw_and_select(&mut ws); // artwork
    assert!(ws.selected_playback().is_none());
    assert!(!drawn(&mut ws).iter().any(|t| t == "Looping"));
    let mut empty = workspace();
    assert!(empty.selected_playback().is_none());
    assert!(!drawn(&mut empty).iter().any(|t| t == "Looping"));
}

#[test]
fn the_object_tab_draws_the_looping_section_for_a_graphic_with_its_current_choice() {
    let mut ws = selected_instance(SymbolKind::Graphic);
    let text = drawn(&mut ws);
    for wanted in ["Looping", "Options", "First", "Loop"] {
        assert!(text.iter().any(|t| t == wanted), "{wanted}: {text:?}");
    }
    ws.set_selection_playback(Playback {
        mode: PlayMode::SingleFrame,
        first_frame: 0,
    });
    let text = drawn(&mut ws);
    assert!(text.iter().any(|t| t == "Single Frame"), "{text:?}");
}

#[test]
fn several_selected_things_have_no_single_setting() {
    let mut ws = selected_instance(SymbolKind::Graphic);
    let mut other = ws.selected.expect("selected");
    other.start += 1;
    ws.also_selected.push(other);
    assert!(ws.selected_playback().is_none());
}
