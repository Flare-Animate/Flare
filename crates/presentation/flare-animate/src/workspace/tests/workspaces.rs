use super::*;
use flare_dock::{Preset, WorkspaceId};

#[test]
fn the_window_menu_switches_workspace_and_each_keeps_its_own_changes() {
    let mut ws = workspace();
    rect(&mut ws);
    let document = ws.document().clone();
    assert_eq!(
        ws.active_workspace(),
        &WorkspaceId::Preset(Preset::Essentials)
    );
    ws.perform(MenuAction::ShowColor);
    ws.panels.layout.dock(Pane::Color);

    ws.perform(MenuAction::WorkspaceSmallScreen);
    assert_eq!(
        ws.active_workspace(),
        &WorkspaceId::Preset(Preset::SmallScreen)
    );
    assert!(
        ws.panels.layout.strip.is_empty(),
        "Small Screen docks everything"
    );
    assert_eq!(ws.panels.flyout, None);

    ws.perform(MenuAction::WorkspaceEssentials);
    assert!(!ws.panels.layout.in_strip(Pane::Color), "as it was left");
    ws.perform(MenuAction::ResetLayout);
    assert!(ws.panels.layout.in_strip(Pane::Color), "as built in");
    assert_eq!(ws.document(), &document, "the document is never touched");
    for action in [
        MenuAction::WorkspaceEssentials,
        MenuAction::WorkspaceAnimator,
        MenuAction::WorkspaceDesigner,
        MenuAction::WorkspaceClassic,
        MenuAction::WorkspaceSmallScreen,
    ] {
        assert!(ws.is_enabled(action));
        ws.perform(action);
        flare_dock::check(&ws.panels.layout).expect("every panel once");
    }
}

#[test]
fn a_saved_workspace_is_kept_between_sessions_and_a_bad_name_is_refused_with_a_reason() {
    let mut ws = workspace();
    ws.perform(MenuAction::WorkspaceClassic);
    ws.panels.layout.dock(Pane::Layers);
    assert!(ws.save_workspace("Inking"));
    assert_eq!(ws.active_workspace(), &WorkspaceId::Custom("Inking".into()));
    assert!(ws.status().is_some_and(|s| s.contains("Inking")));
    assert!(!ws.save_workspace("   "));
    assert!(
        ws.status().is_some_and(|s| s.contains("name")),
        "{:?}",
        ws.status()
    );

    let json = ws.layout_json().expect("json");
    let mut next = workspace();
    next.restore_layout(&json).expect("restores");
    assert_eq!(
        next.active_workspace(),
        &WorkspaceId::Custom("Inking".into())
    );
    assert!(!next.panels.layout.in_strip(Pane::Layers));
    // Classic was left as it was when Inking was saved over it.
    next.perform(MenuAction::WorkspaceClassic);
    assert!(!next.panels.layout.in_strip(Pane::Layers));

    next.delete_workspace("Inking");
    assert_eq!(
        next.panels.workspaces.custom_names().count(),
        0,
        "forgotten"
    );
    next.delete_workspace("Inking");
    assert!(
        next.status().is_some_and(|s| s.contains("Inking")),
        "says why"
    );
}

#[test]
fn the_switcher_lists_the_workspaces_in_the_language_and_marks_the_one_in_use() {
    let mut ws = workspace();
    ws.set_system_locale("de-DE");
    assert!(ws.save_workspace("Mein Bereich"));
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    // The list as the popup draws it.
    let mut drawn = Vec::new();
    for step in 0..3 {
        let mut out = ctx.run_ui(input(f64::from(step) * 0.1, vec![]), |ui| {
            ws.workspace_list(ui);
        });
        out.textures_delta.clear();
        drawn = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) => Some(t.galley.text().to_owned()),
                _ => None,
            })
            .collect::<Vec<_>>();
    }
    for wanted in [
        "Neuer Arbeitsbereich",
        "Standard",
        "Grundlagen",
        "Klassisch",
        "Kleiner Bildschirm",
        "Eigene Arbeitsbereiche",
        "Mein Bereich",
    ] {
        assert!(drawn.iter().any(|t| t == wanted), "{wanted}: {drawn:?}");
    }
    assert_eq!(ws.workspace_name(ws.active_workspace()), "Mein Bereich");
}
