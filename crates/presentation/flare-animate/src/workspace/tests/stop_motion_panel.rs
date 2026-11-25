//! The Stop Motion panel: a session is started, takes pictures, is held, laid on a layer and ended,
//! each edit one undo step; what the library refuses is said and changes nothing.

use super::*;

fn png(shade: u8) -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, 2, 2);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("header");
    writer.write_image_data(&[shade; 16]).expect("data");
    writer.finish().expect("finish");
    out
}

fn frames(ws: &Workspace) -> usize {
    ws.document().stop_motion().map_or(0, |s| s.frames().len())
}

#[test]
fn a_session_takes_pictures_holds_them_and_lays_them_on_a_layer_each_in_one_undo_step() {
    let mut ws = workspace();
    ws.stop_motion_start(12, 2);
    ws.add_stop_motion_pictures(decode_stop_motion_pictures(vec![
        ("a.png".into(), png(1)),
        ("b.png".into(), png(200)),
    ]));
    assert_eq!(frames(&ws), 2);
    ws.stop_motion_set_hold(1, 5);
    assert_eq!(ws.document().stop_motion().expect("session").duration(), 7);
    ws.undo();
    assert_eq!(ws.document().stop_motion().expect("session").duration(), 4);
    let layers = layer_count(&ws);
    ws.stop_motion_lay();
    assert_eq!(layer_count(&ws), layers + 1);
    ws.undo();
    assert_eq!(layer_count(&ws), layers);
    ws.stop_motion_remove(0);
    assert_eq!(frames(&ws), 1);
    ws.stop_motion_end();
    assert!(ws.document().stop_motion().is_none());
    ws.undo();
    assert!(
        ws.document().stop_motion().is_some(),
        "one undo brings it back"
    );
}

#[test]
fn a_bad_file_or_value_is_said_and_the_good_pictures_still_go_in() {
    let mut ws = workspace();
    ws.stop_motion_set_hold(0, 1);
    assert!(
        ws.status()
            .is_some_and(|s| s.contains("no stop-motion session"))
    );
    ws.stop_motion_start(0, 1);
    assert!(ws.document().stop_motion().is_none(), "a rate of 0");
    ws.stop_motion_start(12, 1);
    ws.add_stop_motion_pictures(decode_stop_motion_pictures(vec![
        ("bad.png".into(), b"no".to_vec()),
        ("good.png".into(), png(9)),
    ]));
    assert_eq!(frames(&ws), 1);
    assert!(ws.status().is_some_and(|s| s.contains("bad.png")));
    let revision = ws.active().revision();
    ws.stop_motion_set_hold(9, 2);
    assert_eq!(ws.active().revision(), revision, "no such frame");
}

#[test]
fn the_panel_draws_empty_and_with_a_session_and_the_menu_opens_it() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    ws.perform(MenuAction::ShowStopMotion);
    run(&mut ws, &ctx, 0.0);
    ws.stop_motion_start(12, 1);
    ws.add_stop_motion_pictures(decode_stop_motion_pictures(vec![("a.png".into(), png(3))]));
    ws.perform(MenuAction::ShowStopMotion);
    run(&mut ws, &ctx, 1.0);
    run(&mut ws, &ctx, 2.0);
}
