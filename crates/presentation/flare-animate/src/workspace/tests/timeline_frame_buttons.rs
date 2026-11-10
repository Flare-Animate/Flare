//! The frame buttons on the timeline toolbar.

use super::*;
use crate::workspace::panels::timeline_frame_buttons::FRAME_BUTTONS;

/// Presses the timeline toolbar's button for `action` with the pointer, as a person does.
fn press_frame_button(ws: &mut Workspace, action: MenuAction) {
    let ctx = &egui::Context::default();
    run(ws, ctx, 0.0);
    run(ws, ctx, 0.1);
    let icon = action.icon().expect("an icon");
    let id = egui::Id::unique(("timeline-frame-button", icon));
    let rect: Rect = ctx
        .data(|data| data.get_temp(id))
        .expect("the button was drawn");
    let at = rect.center();
    let click = |pressed| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    pass(ws, ctx, input(0.2, vec![Event::PointerMoved(at)]));
    pass(ws, ctx, input(0.3, vec![click(true)]));
    pass(ws, ctx, input(0.4, vec![click(false)]));
    run(ws, ctx, 0.5);
}

#[test]
fn every_frame_button_has_its_own_icon_and_says_what_it_does_with_its_keys() {
    let ws = workspace();
    let mut icons = std::collections::HashSet::new();
    for action in FRAME_BUTTONS {
        let icon = action.icon().expect("an icon");
        assert!(flare_ui::icons::exists(icon), "{action:?}: {icon}");
        assert!(icons.insert(icon), "{action:?} shares {icon}");
    }
    assert_eq!(
        ws.frame_button_hint(MenuAction::InsertKeyframe),
        "Insert Keyframe (F6)"
    );
    assert_eq!(
        ws.frame_button_hint(MenuAction::InsertBlankKeyframe),
        "Insert Blank Keyframe (F7)"
    );
    assert_eq!(
        ws.frame_button_hint(MenuAction::InsertFrame),
        "Insert Frame (F5)"
    );
    // The keys are written the way the system writes them, so only the key itself is checked.
    let remove = ws.frame_button_hint(MenuAction::RemoveFrame);
    assert!(remove.ends_with("F5)") && remove.contains(" ("), "{remove}");
}

#[test]
fn pressing_the_keyframe_buttons_splits_the_exposure_at_the_playhead() {
    // Two keyframes: 0..12 and 12..24.
    for action in [MenuAction::InsertKeyframe, MenuAction::InsertBlankKeyframe] {
        let (mut ws, layer) = two_exposures();
        ws.select_layer(layer);
        ws.set_frame(5);
        press_frame_button(&mut ws, action);
        assert_eq!(
            layer_spans(&ws, layer),
            vec![(0, 5), (5, 7), (12, 12)],
            "{action:?}"
        );
        ws.undo();
        assert_eq!(
            layer_spans(&ws, layer),
            vec![(0, 12), (12, 12)],
            "{action:?} is one undo step"
        );
    }
}

#[test]
fn pressing_insert_frame_and_remove_frames_lengthens_and_shortens_the_exposure() {
    let (mut ws, layer) = two_exposures();
    ws.select_layer(layer);
    ws.set_frame(5);
    press_frame_button(&mut ws, MenuAction::InsertFrame);
    assert_eq!(layer_spans(&ws, layer), vec![(0, 13), (13, 12)]);
    press_frame_button(&mut ws, MenuAction::RemoveFrame);
    assert_eq!(layer_spans(&ws, layer), vec![(0, 12), (12, 12)]);
}

#[test]
fn the_frame_buttons_do_nothing_on_a_locked_layer() {
    let (mut ws, layer) = two_exposures();
    ws.select_layer(layer);
    ws.set_frame(5);
    ws.toggle_layer_locked(layer);
    for action in FRAME_BUTTONS {
        assert!(!ws.is_enabled(action), "{action:?}");
        press_frame_button(&mut ws, action);
    }
    assert_eq!(layer_spans(&ws, layer), vec![(0, 12), (12, 12)]);
}
