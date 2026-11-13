//! Cut, Select All and Reverse Frames, and the menu on a right click of a frame.

use super::layer_names::{find, press};
use super::*;
use egui::PointerButton;

#[test]
fn cut_frames_takes_them_out_remembers_them_for_paste_and_is_one_undo_step() {
    // Two keyframes: 0..12 and 12..24.
    let (mut ws, layer) = two_exposures();
    ws.select_layer(layer);
    ws.select_frames(layer, 2, 5);
    ws.perform(MenuAction::CutFrames);
    assert_eq!(layer_spans(&ws, layer), vec![(0, 8), (8, 12)]);
    assert!(ws.frame_range.is_none(), "what was selected is gone");
    assert!(
        ws.is_enabled(MenuAction::PasteFrames),
        "and is on the clipboard"
    );
    // The four frames go back, over the start of the second keyframe.
    ws.set_frame(8);
    ws.perform(MenuAction::PasteFrames);
    assert_eq!(layer_spans(&ws, layer), vec![(0, 8), (8, 4), (12, 8)]);
    ws.undo();
    ws.undo();
    assert_eq!(
        layer_spans(&ws, layer),
        vec![(0, 12), (12, 12)],
        "one step each"
    );
}

#[test]
fn cutting_nothing_or_from_a_locked_layer_says_so_and_changes_nothing() {
    let (mut ws, layer) = two_exposures();
    ws.select_layer(layer);
    ws.select_frames(layer, 40, 45);
    ws.perform(MenuAction::CutFrames);
    assert!(ws.status().is_some_and(|m| m.contains("nothing to cut")));
    assert!(
        !ws.is_enabled(MenuAction::PasteFrames),
        "nothing was lifted"
    );
    ws.select_frames(layer, 0, 3);
    ws.toggle_layer_locked(layer);
    assert!(
        !ws.is_enabled(MenuAction::CutFrames),
        "a locked layer cannot be cut from"
    );
    ws.toggle_layer_locked(layer);
    assert_eq!(layer_spans(&ws, layer), vec![(0, 12), (12, 12)]);
}

#[test]
fn select_all_frames_selects_the_active_layer_from_the_first_frame_to_its_last() {
    let (mut ws, layer) = two_exposures();
    ws.select_layer(layer);
    assert!(ws.is_enabled(MenuAction::SelectAllFrames));
    ws.perform(MenuAction::SelectAllFrames);
    let range = ws.frame_range.expect("selected");
    assert_eq!((range.layer, range.from, range.to), (layer, 0, 23));
}

#[test]
fn reverse_frames_needs_selected_whole_keyframes_and_reverses_them_in_one_step() {
    // Two keyframes: 0..12 and 12..24.
    let (mut ws, layer) = two_exposures();
    ws.select_layer(layer);
    let contents = |ws: &Workspace| {
        ws.layer(layer)
            .expect("layer")
            .spans()
            .iter()
            .map(|s| format!("{:?}", s.content()))
            .collect::<Vec<_>>()
    };
    let before = contents(&ws);
    assert!(
        !ws.is_enabled(MenuAction::ReverseFrames),
        "nothing selected"
    );
    ws.reverse_frames();
    assert!(ws.status().is_some_and(|m| m.contains("Select the frames")));

    ws.select_frames(layer, 2, 5); // cuts the first keyframe
    ws.perform(MenuAction::ReverseFrames);
    assert!(
        ws.status().is_some_and(|m| m.contains("whole keyframes")),
        "{:?}",
        ws.status()
    );
    assert_eq!(contents(&ws), before);

    ws.select_frames(layer, 0, 23);
    ws.perform(MenuAction::ReverseFrames);
    assert_eq!(
        contents(&ws),
        before.iter().rev().cloned().collect::<Vec<_>>()
    );
    assert_eq!(layer_spans(&ws, layer), vec![(0, 12), (12, 12)]);
    ws.undo();
    assert_eq!(contents(&ws), before, "one undo step");
}

#[test]
fn a_right_click_on_a_frame_opens_the_menu_for_frames_and_choosing_an_item_acts_there() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    rect(&mut ws);
    let mut time = 0.0;
    let mut name = None;
    for _ in 0..4 {
        time += 0.1;
        name = find(&mut ws, &ctx, time, vec![], "Layer 1").or(name);
    }
    let name = name.expect("the name of the layer the rectangle is on is drawn");
    // On the name's row, over the frames, a few frames in.
    let l = ws.timeline_layout.expect("drawn");
    let at = egui::pos2(l.frame_x(4.5), name.y);
    for events in [
        vec![Event::PointerMoved(at)],
        vec![press(at, PointerButton::Secondary, true)],
        vec![press(at, PointerButton::Secondary, false)],
    ] {
        time += 0.05;
        pass(&mut ws, &ctx, input(time, events));
    }
    let frame = ws.frame;
    assert!(frame > 0, "the right click picked the frame it was on");
    let mut insert = None;
    for _ in 0..4 {
        time += 0.1;
        insert = find(&mut ws, &ctx, time, vec![], "Insert Keyframe").or(insert);
    }
    let insert = insert.expect("the menu for frames is open");
    for wanted in [
        "Reverse Frames",
        "Cut Frames",
        "Select All Frames",
        "Remove Frames",
    ] {
        time += 0.1;
        assert!(
            find(&mut ws, &ctx, time, vec![], wanted).is_some(),
            "{wanted}"
        );
    }
    let spans_before = layer_spans(&ws, ws.current_layer.expect("layer")).len();
    for events in [
        vec![Event::PointerMoved(insert)],
        vec![press(insert, PointerButton::Primary, true)],
        vec![press(insert, PointerButton::Primary, false)],
        vec![],
    ] {
        time += 0.05;
        pass(&mut ws, &ctx, input(time, events));
    }
    let spans_after = layer_spans(&ws, ws.current_layer.expect("layer")).len();
    assert_eq!(
        spans_after,
        spans_before + 1,
        "a keyframe was inserted at the frame clicked"
    );
}
