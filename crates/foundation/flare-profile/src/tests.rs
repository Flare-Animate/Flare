#![allow(clippy::expect_used)]

//! Tests for the profile reader. Each refusal has a test, so a check cannot be dropped silently.

use std::collections::BTreeSet;

use crate::builtin::builtin;
use crate::load::{MAX_PROFILE_BYTES, MAX_ROOMS, MAX_TOOLBARS};
use crate::model::{DockNode, PanelId, Profile};
use crate::{ProfileError, parse};

fn panel_name(panel: PanelId) -> String {
    serde_json::to_string(&panel).expect("a panel name")
}

fn stack_of(panels: &[PanelId]) -> String {
    let names: Vec<String> = panels.iter().map(|p| panel_name(*p)).collect();
    format!(
        r#"{{"kind":"stack","panels":[{}],"active":0}}"#,
        names.join(",")
    )
}

fn split(axis: &str, fraction: &str, first: &str, second: &str) -> String {
    format!(
        r#"{{"kind":"split","axis":"{axis}","first_fraction":{fraction},"first":{first},"second":{second}}}"#
    )
}

/// A chain of splits, one stack per panel, so the deepest stack sits `panels.len()` levels down.
fn chain(panels: &[PanelId]) -> String {
    let (last, head) = panels.split_last().expect("at least one panel");
    let mut layout = stack_of(&[*last]);
    for panel in head.iter().rev() {
        layout = split("horizontal", "0.5", &stack_of(&[*panel]), &layout);
    }
    layout
}

fn room_json(id: &str, layout: &str) -> String {
    format!(r#"{{"id":"{id}","title":"Room {id}","layout":{layout}}}"#)
}

fn profile_with_rooms(rooms: &str) -> String {
    format!(
        r#"{{"schema":1,"id":"test","title":"Test","shortcut_set":"flare","theme":"dark","toolbars":[],"rooms":[{rooms}],"default_room":0}}"#
    )
}

fn profile_json(layout: &str) -> String {
    profile_with_rooms(&room_json("main", layout))
}

fn all_in_one() -> String {
    stack_of(&PanelId::ALL)
}

fn panels_in(node: &DockNode, out: &mut BTreeSet<PanelId>) {
    match node {
        DockNode::Split { first, second, .. } => {
            panels_in(first, out);
            panels_in(second, out);
        }
        DockNode::Stack { panels, .. } => out.extend(panels.iter().copied()),
    }
}

fn panels_of(profile: &Profile) -> BTreeSet<PanelId> {
    let mut out = BTreeSet::new();
    for room in &profile.rooms {
        panels_in(&room.layout, &mut out);
    }
    out
}

#[test]
fn one_room_with_every_panel_is_valid() {
    parse(&profile_json(&all_in_one())).expect("a complete profile loads");
}

#[test]
fn the_four_built_in_profiles_load_under_their_own_ids() {
    let profiles = builtin().expect("built-in profiles load");
    let ids: Vec<&str> = profiles.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "flare-default",
            "opentoonz-classic",
            "tahoma-style",
            "animate-style"
        ]
    );
}

#[test]
fn every_built_in_profile_exposes_every_panel() {
    for profile in builtin().expect("built-in profiles load") {
        let placed = panels_of(&profile);
        let missing: Vec<PanelId> = PanelId::ALL
            .iter()
            .copied()
            .filter(|p| !placed.contains(p))
            .collect();
        assert!(missing.is_empty(), "{} hides {missing:?}", profile.id);
    }
}

#[test]
fn the_panel_list_has_each_panel_once_and_marks_the_unbuilt_ones() {
    let unique: BTreeSet<PanelId> = PanelId::ALL.iter().copied().collect();
    assert_eq!(unique.len(), PanelId::ALL.len());
    assert_eq!(PanelId::ALL.iter().filter(|p| p.is_built()).count(), 11);
    assert!(PanelId::Timeline.is_built());
    assert!(!PanelId::Xsheet.is_built());
    assert!(!PanelId::ScriptConsole.is_built());
}

#[test]
fn a_profile_that_leaves_out_panels_is_refused() {
    let json = profile_json(&stack_of(&[PanelId::Tools, PanelId::Timeline]));
    match parse(&json) {
        Err(ProfileError::MissingPanels(missing)) => {
            assert_eq!(missing.len(), PanelId::ALL.len() - 2);
        }
        other => panic!("expected MissingPanels, got {other:?}"),
    }
}

#[test]
fn a_panel_twice_in_one_room_is_refused() {
    let layout = split(
        "horizontal",
        "0.5",
        &all_in_one(),
        &stack_of(&[PanelId::Tools]),
    );
    assert!(matches!(
        parse(&profile_json(&layout)),
        Err(ProfileError::Duplicate { .. })
    ));
}

#[test]
fn a_panel_in_two_rooms_is_allowed() {
    let rooms = format!(
        "{},{}",
        room_json("a", &all_in_one()),
        room_json("b", &all_in_one())
    );
    parse(&profile_with_rooms(&rooms)).expect("each room may show a panel");
}

#[test]
fn a_file_over_the_size_limit_is_refused_before_it_is_read() {
    let big = " ".repeat(MAX_PROFILE_BYTES + 1);
    assert!(matches!(parse(&big), Err(ProfileError::TooLarge { .. })));
}

#[test]
fn text_that_is_not_a_profile_is_refused() {
    assert!(matches!(parse("{"), Err(ProfileError::Parse(_))));
    assert!(matches!(parse("[]"), Err(ProfileError::Parse(_))));
}

#[test]
fn an_unknown_field_is_refused() {
    let json = profile_json(&all_in_one()).replacen(
        "\"default_room\":0",
        "\"default_room\":0,\"bogus\":1",
        1,
    );
    assert!(matches!(parse(&json), Err(ProfileError::Parse(_))));
}

#[test]
fn a_split_outside_five_to_ninety_five_percent_is_refused() {
    for fraction in ["0.0", "0.99", "-0.5"] {
        let layout = split(
            "vertical",
            fraction,
            &all_in_one(),
            &stack_of(&[PanelId::Tools]),
        );
        assert!(
            matches!(
                parse(&profile_json(&layout)),
                Err(ProfileError::Fraction(_))
            ),
            "fraction {fraction} was accepted"
        );
    }
}

#[test]
fn a_layout_nested_too_deep_is_refused() {
    // 17 splits put the deepest stack 18 levels down, past the limit of 16.
    let layout = chain(&PanelId::ALL[..18]);
    assert!(matches!(
        parse(&profile_json(&layout)),
        Err(ProfileError::TooDeep)
    ));
}

#[test]
fn a_layout_exactly_at_the_depth_limit_is_accepted() {
    // 15 splits put the deepest stack 16 levels down; the second room covers the rest.
    let rooms = format!(
        "{},{}",
        room_json("deep", &chain(&PanelId::ALL[..16])),
        room_json("rest", &all_in_one())
    );
    parse(&profile_with_rooms(&rooms)).expect("a layout at the limit loads");
}

#[test]
fn an_empty_stack_is_refused() {
    assert!(matches!(
        parse(&profile_json(&stack_of(&[]))),
        Err(ProfileError::EmptyStack)
    ));
}

#[test]
fn the_active_tab_must_exist() {
    let layout = r#"{"kind":"stack","panels":["tools"],"active":3}"#;
    assert!(matches!(
        parse(&profile_json(layout)),
        Err(ProfileError::ActiveTab { active: 3, len: 1 })
    ));
}

#[test]
fn names_outside_the_known_lists_are_refused() {
    let base = profile_json(&all_in_one());
    let theme = base.replace("\"theme\":\"dark\"", "\"theme\":\"neon\"");
    assert!(matches!(
        parse(&theme),
        Err(ProfileError::Unknown { what: "theme", .. })
    ));
    let shortcuts = base.replace("\"shortcut_set\":\"flare\"", "\"shortcut_set\":\"x\"");
    assert!(matches!(
        parse(&shortcuts),
        Err(ProfileError::Unknown {
            what: "shortcut set",
            ..
        })
    ));
}

#[test]
fn ids_must_be_plain_names() {
    let base = profile_json(&all_in_one());
    assert!(matches!(
        parse(&base.replace("\"id\":\"test\"", "\"id\":\"Te st\"")),
        Err(ProfileError::BadText { .. })
    ));
    assert!(matches!(
        parse(&base.replace("\"id\":\"test\"", "\"id\":\"\"")),
        Err(ProfileError::BadText { .. })
    ));
}

#[test]
fn the_schema_and_the_default_room_are_checked() {
    let base = profile_json(&all_in_one());
    assert!(matches!(
        parse(&base.replace("\"schema\":1", "\"schema\":2")),
        Err(ProfileError::Schema(2))
    ));
    assert!(matches!(
        parse(&base.replace("\"default_room\":0", "\"default_room\":4")),
        Err(ProfileError::DefaultRoom(4))
    ));
}

#[test]
fn room_and_toolbar_counts_are_bounded() {
    assert!(matches!(
        parse(&profile_with_rooms("")),
        Err(ProfileError::NoRooms)
    ));

    let rooms: Vec<String> = (0..=MAX_ROOMS)
        .map(|i| room_json(&format!("r{i}"), &all_in_one()))
        .collect();
    assert!(matches!(
        parse(&profile_with_rooms(&rooms.join(","))),
        Err(ProfileError::TooMany { what: "rooms", .. })
    ));

    let toolbars: Vec<String> = (0..=MAX_TOOLBARS)
        .map(|i| format!(r#"{{"id":"t{i}","visible":true}}"#))
        .collect();
    let json = profile_json(&all_in_one()).replace(
        "\"toolbars\":[]",
        &format!("\"toolbars\":[{}]", toolbars.join(",")),
    );
    assert!(matches!(
        parse(&json),
        Err(ProfileError::TooMany {
            what: "toolbars",
            ..
        })
    ));
}

#[test]
fn a_built_in_profile_survives_a_round_trip() {
    let profile = builtin().expect("built-in profiles load").remove(0);
    let text = serde_json::to_string(&profile).expect("serialize");
    assert_eq!(parse(&text).expect("reload"), profile);
}
