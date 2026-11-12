use super::*;

fn click_at(ws: &mut Workspace, ctx: &egui::Context, at: Pos2, time: f64) {
    let click = |pressed| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    pass(ws, ctx, input(time, vec![Event::PointerMoved(at)]));
    pass(ws, ctx, input(time + 0.05, vec![click(true)]));
    pass(ws, ctx, input(time + 0.1, vec![click(false)]));
}

fn icon_of(ws: &Workspace, pane: Pane) -> Rect {
    ws.panels
        .icons
        .iter()
        .find(|(p, _)| *p == pane)
        .map(|(_, r)| *r)
        .expect("the panel has an icon in the rail")
}

#[test]
fn every_panel_that_can_be_kept_in_the_strip_has_an_icon_of_its_own() {
    let mut icons = std::collections::HashSet::new();
    for pane in Pane::ALL {
        match pane.rail_icon() {
            Some(icon) => {
                assert!(flare_ui::icons::exists(icon), "{pane:?}: {icon}");
                assert!(icons.insert(icon), "{pane:?} shares {icon}");
            }
            None => assert!(
                matches!(
                    pane,
                    Pane::Tools | Pane::Stage | Pane::Timeline | Pane::Output
                ),
                "{pane:?} has no icon"
            ),
        }
    }
}

#[test]
fn the_rail_shows_the_strip_in_its_order_and_nothing_docked() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    let shown: Vec<Pane> = ws.panels.icons.iter().map(|(p, _)| *p).collect();
    assert_eq!(shown, flare_dock::DEFAULT_STRIP.to_vec());
    assert!(
        ws.panels
            .icons
            .windows(2)
            .all(|w| w[0].1.top() < w[1].1.top())
    );
}

#[test]
fn showing_a_strip_panel_opens_it_beside_the_strip_and_properties_stays_in_view() {
    let mut ws = workspace();
    assert!(ws.pane_is_front(Pane::Properties));
    assert!(!ws.pane_is_front(Pane::Color));
    ws.perform(MenuAction::ShowColor);
    assert!(ws.pane_is_front(Pane::Color));
    assert!(ws.pane_is_front(Pane::Properties), "not covered");
    ws.perform(MenuAction::ShowInfo);
    assert!(ws.pane_is_front(Pane::Info) && !ws.pane_is_front(Pane::Color));
    // A docked panel is brought forward in its stack as before.
    ws.perform(MenuAction::ShowLibrary);
    assert!(ws.pane_is_front(Pane::Library) && !ws.pane_is_front(Pane::Properties));
}

#[test]
fn clicking_an_icon_opens_its_panel_and_a_second_click_or_a_press_on_the_stage_puts_it_away() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
    let color = icon_of(&ws, Pane::Color).center();
    click_at(&mut ws, &ctx, color, 0.2);
    run(&mut ws, &ctx, 0.4);
    assert_eq!(ws.panels.flyout, Some(Pane::Color));
    assert!(ws.pane_is_front(Pane::Properties));
    click_at(&mut ws, &ctx, color, 0.5);
    assert_eq!(ws.panels.flyout, None, "the same icon again puts it away");

    let align = icon_of(&ws, Pane::Align).center();
    click_at(&mut ws, &ctx, align, 0.7);
    run(&mut ws, &ctx, 0.9);
    assert_eq!(ws.panels.flyout, Some(Pane::Align));
    // A press inside the panel keeps it.
    let inside = Pos2::new(
        1280.0 - crate::workspace::rail::RAIL_WIDTH - 40.0,
        icon_of(&ws, Pane::Align).center().y + 30.0,
    );
    click_at(&mut ws, &ctx, inside, 1.0);
    assert_eq!(ws.panels.flyout, Some(Pane::Align), "kept");
    // A press on the stage puts it away.
    click_at(&mut ws, &ctx, Pos2::new(300.0, 300.0), 1.2);
    assert_eq!(ws.panels.flyout, None);
}

#[test]
fn escape_puts_the_open_panel_away() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    ws.perform(MenuAction::ShowHistory);
    run(&mut ws, &ctx, 0.0);
    assert_eq!(ws.panels.flyout, Some(Pane::History));
    pass(
        &mut ws,
        &ctx,
        input(0.1, vec![key(Key::Escape, Modifiers::NONE)]),
    );
    assert_eq!(ws.panels.flyout, None);
}

#[test]
fn docking_from_the_flyout_and_collapsing_a_tab_move_the_panel_and_keep_every_panel_once() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    ws.perform(MenuAction::ShowColor);
    ws.panels.change = Some(crate::workspace::rail::PanelChange::Dock(Pane::Color));
    ws.apply_panel_change();
    assert!(!ws.panels.layout.in_strip(Pane::Color));
    assert_eq!(
        ws.panels.flyout, None,
        "it is docked now, not open beside the strip"
    );
    assert!(ws.pane_is_front(Pane::Color), "in front of its stack");
    run(&mut ws, &ctx, 0.0);
    assert!(ws.panels.icons.iter().all(|(p, _)| *p != Pane::Color));

    ws.panels.change = Some(crate::workspace::rail::PanelChange::Collapse(
        Pane::Properties,
    ));
    ws.apply_panel_change();
    assert!(ws.panels.layout.in_strip(Pane::Properties));
    run(&mut ws, &ctx, 0.1);
    assert!(ws.panels.icons.iter().any(|(p, _)| *p == Pane::Properties));
    flare_dock::check(&ws.panels.layout).expect("every panel once");

    // The arrangement is kept between sessions with the strip.
    let json = ws.layout_json().expect("json");
    let mut again = workspace();
    again.restore_layout(&json).expect("restores");
    assert_eq!(again.panels.layout.strip, ws.panels.layout.strip);
    again.reset_layout();
    assert_eq!(
        again.panels.layout.strip,
        flare_dock::DEFAULT_STRIP.to_vec()
    );
}

#[test]
fn home_has_no_rail_to_show() {
    let mut ws = workspace();
    ws.set_home(true);
    assert!(!ws.document_controls_shown());
}
