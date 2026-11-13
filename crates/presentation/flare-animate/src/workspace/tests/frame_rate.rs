//! Changing the frame rate of a project after it is made, from the timeline's toolbar or the
//! document's properties.

use super::layer_names::{click, settle};
use super::*;
use egui::PointerButton;
use flare_core::FrameRate;

fn rate(ws: &Workspace) -> FrameRate {
    ws.document().frame_rate()
}

#[test]
fn a_new_rate_is_one_undo_step_and_one_out_of_range_is_refused_with_a_message() {
    let mut ws = workspace();
    let before = rate(&ws);
    ws.set_frame_rate(12.5);
    assert_eq!(rate(&ws), FrameRate::new(25, 2).expect("rate"));
    ws.undo();
    assert_eq!(rate(&ws), before);
    for bad in [0.0, 500.0, f64::NAN] {
        ws.set_frame_rate(bad);
        assert_eq!(rate(&ws), before, "{bad}");
        assert!(ws.status().is_some(), "{bad} says why");
    }
}

#[test]
fn the_rate_in_the_timelines_toolbar_is_typed_into_and_applied_on_enter() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    let mut time = 0.0;
    settle(&mut ws, &ctx, &mut time);
    time += 0.1;
    let field = number_beside_fps(&mut ws, &ctx, time, 24.0).expect("the rate is drawn");
    click(&mut ws, &ctx, &mut time, field, PointerButton::Primary);
    time += 0.05;
    pass(
        &mut ws,
        &ctx,
        input(time, vec![Event::Text("12".to_owned())]),
    );
    time += 0.05;
    pass(
        &mut ws,
        &ctx,
        input(time, vec![key(Key::Enter, Modifiers::NONE)]),
    );
    settle(&mut ws, &ctx, &mut time);
    assert_eq!(rate(&ws), FrameRate::new(12, 1).expect("rate"));
    assert!(number_beside_fps(&mut ws, &ctx, time + 0.1, 12.0).is_some());
}

/// Where the rate field shows `number`: the field draws its number and its " fps" apart, so the
/// number is the one on the line of an " fps", just left of it, however many decimals it shows.
fn number_beside_fps(
    ws: &mut Workspace,
    ctx: &egui::Context,
    time: f64,
    number: f64,
) -> Option<Pos2> {
    let mut out = ctx.run_ui(input(time, vec![]), |ui| ws.show(ui));
    out.textures_delta.clear();
    let texts: Vec<(String, egui::Rect)> = out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) => {
                Some((t.galley.text().to_owned(), t.visual_bounding_rect()))
            }
            _ => None,
        })
        .collect();
    let suffixes = texts.iter().filter(|(t, _)| t == " fps");
    suffixes.into_iter().find_map(|(_, fps)| {
        texts.iter().find_map(|(t, r)| {
            let beside = (r.center().y - fps.center().y).abs() < 3.0
                && r.max.x <= fps.min.x + 2.0
                && fps.min.x - r.max.x < 12.0;
            (t.parse::<f64>() == Ok(number) && beside).then(|| r.center())
        })
    })
}
