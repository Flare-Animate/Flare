use super::*;
use crate::workspace::rail::RAIL;

#[test]
fn every_panel_in_the_rail_has_an_icon_that_exists_and_a_place_in_the_layout() {
    let ws = workspace();
    for pane in RAIL {
        let icon = pane.rail_icon().expect("a rail panel has an icon");
        assert!(flare_ui::icons::exists(icon), "{pane:?}: {icon}");
        assert!(
            ws.layout.tiles.find_pane(&pane).is_some(),
            "{pane:?} is in the layout"
        );
    }
    let icons: std::collections::HashSet<_> = RAIL.iter().filter_map(|p| p.rail_icon()).collect();
    assert_eq!(icons.len(), RAIL.len(), "each panel has its own icon");
    for pane in [Pane::Tools, Pane::Stage, Pane::Timeline, Pane::Output] {
        assert_eq!(pane.rail_icon(), None, "{pane:?} is not in the rail");
    }
}

#[test]
fn bringing_a_panel_forward_makes_it_the_front_one_and_the_old_front_one_not() {
    let mut ws = workspace();
    assert!(
        ws.pane_is_front(Pane::Properties),
        "the first tab shows at first"
    );
    assert!(!ws.pane_is_front(Pane::Color));
    ws.perform(MenuAction::ShowColor);
    assert!(ws.pane_is_front(Pane::Color));
    assert!(!ws.pane_is_front(Pane::Properties));
    ws.perform(MenuAction::ShowInfo);
    assert!(ws.pane_is_front(Pane::Info) && !ws.pane_is_front(Pane::Color));
}

#[test]
fn clicking_a_rail_icon_brings_that_panel_forward() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
    // Press the third icon: the rail is at the right edge of the 1280 x 800 window.
    let x = 1280.0 - crate::workspace::rail::RAIL_WIDTH / 2.0;
    let y = 4.0 + 38.0 + 4.0 + 28.0 * 2.0 + 4.0 * 2.0 + 14.0 + 3.0;
    let at = Pos2::new(x, y);
    let click = |pressed| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    pass(&mut ws, &ctx, input(0.2, vec![Event::PointerMoved(at)]));
    pass(&mut ws, &ctx, input(0.3, vec![click(true)]));
    pass(&mut ws, &ctx, input(0.4, vec![click(false)]));
    run(&mut ws, &ctx, 0.5);
    assert!(
        ws.pane_is_front(Pane::Color) && !ws.pane_is_front(Pane::Properties),
        "the third icon is Color, and it came forward"
    );
}

#[test]
fn home_has_no_rail_to_show() {
    let mut ws = workspace();
    ws.set_home(true);
    assert!(!ws.document_controls_shown());
}
