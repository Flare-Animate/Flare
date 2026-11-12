//! Turning the view of the stage: the Rotation tool, its dial, the Alt (Option) tap that shows
//! it, and the tools still working on a turned stage.

use super::*;

/// A pass where the modifiers are `mods` from its start, then `events` happen.
fn with_mods(time: f64, mods: Modifiers, events: Vec<Event>) -> RawInput {
    let mut all = vec![Event::ModifiersChanged(mods)];
    all.extend(events);
    input(time, all)
}

fn alt_held(time: f64, alt: bool, events: Vec<Event>) -> RawInput {
    let mods = if alt { Modifiers::ALT } else { Modifiers::NONE };
    with_mods(time, mods, events)
}

fn press(at: Pos2, pressed: bool, modifiers: Modifiers) -> Event {
    Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers,
    }
}

/// Where a stage point shows on screen, the turn included.
fn screen_of(ws: &Workspace, at: Pos2) -> Pos2 {
    ws.stage_view
        .to_screen(ws.stage_panel, ws.stage_extent(), at)
}

/// A click at `at` on screen, over a few passes as a person's would be.
fn click(ws: &mut Workspace, ctx: &egui::Context, time: &mut f64, at: Pos2) {
    for event in [
        Event::PointerMoved(at),
        press(at, true, Modifiers::NONE),
        press(at, false, Modifiers::NONE),
    ] {
        *time += 0.02;
        pass(ws, ctx, input(*time, vec![event]));
    }
}

fn laid_out() -> (Workspace, egui::Context) {
    let mut ws = workspace();
    // Well away from the stage's center, so a turn moves it on screen.
    ws.commit_rectangle(Pos2::new(700.0, 400.0), Pos2::new(800.0, 500.0));
    ws.set_tool(Tool::Select);
    let ctx = egui::Context::default();
    for time in [0.0, 0.01, 0.02] {
        run(&mut ws, &ctx, time);
    }
    (ws, ctx)
}

#[test]
fn tapping_alt_alone_shows_the_dial_with_the_rotation_tool_and_again_hides_it() {
    let (mut ws, ctx) = laid_out();
    ws.set_tool(Tool::Brush);
    pass(&mut ws, &ctx, alt_held(1.0, true, vec![]));
    assert!(!ws.rotation_dial_open(), "nothing while it is held");
    pass(&mut ws, &ctx, alt_held(1.1, false, vec![]));
    assert!(ws.rotation_dial_open());
    assert_eq!(ws.tool(), Tool::RotateView);
    // The dial is drawn: its title is on screen.
    let mut out = ctx.run_ui(input(1.2, vec![]), |ui| ws.show(ui));
    out.textures_delta.clear();
    let title = ws.tr("view-turn-title");
    assert!(
        out.shapes.iter().any(|s| matches!(&s.shape,
            egui::epaint::Shape::Text(t) if t.galley.job.text.contains(&title))),
        "the dial shows"
    );
    // A second tap puts the dial away and the brush back in hand.
    pass(&mut ws, &ctx, alt_held(2.0, true, vec![]));
    pass(&mut ws, &ctx, alt_held(2.1, false, vec![]));
    assert!(!ws.rotation_dial_open());
    assert_eq!(ws.tool(), Tool::Brush);
}

#[test]
fn alt_with_a_click_a_key_or_a_long_hold_is_not_a_tap() {
    let (mut ws, ctx) = laid_out();
    ws.set_tool(Tool::Zoom);
    let at = screen_of(&ws, Pos2::new(300.0, 300.0));
    // Alt-click with the Zoom tool zooms out, and stays that.
    pass(
        &mut ws,
        &ctx,
        alt_held(1.0, true, vec![Event::PointerMoved(at)]),
    );
    pass(
        &mut ws,
        &ctx,
        alt_held(1.05, true, vec![press(at, true, Modifiers::ALT)]),
    );
    pass(
        &mut ws,
        &ctx,
        alt_held(1.1, true, vec![press(at, false, Modifiers::ALT)]),
    );
    pass(&mut ws, &ctx, alt_held(1.15, false, vec![]));
    assert!(!ws.rotation_dial_open());
    assert_eq!(ws.tool(), Tool::Zoom);
    // Alt with a key is that chord.
    pass(
        &mut ws,
        &ctx,
        alt_held(2.0, true, vec![key(Key::C, Modifiers::ALT)]),
    );
    pass(&mut ws, &ctx, alt_held(2.1, false, vec![]));
    assert!(!ws.rotation_dial_open());
    // Held a long time and let go is no tap either.
    pass(&mut ws, &ctx, alt_held(3.0, true, vec![]));
    pass(&mut ws, &ctx, alt_held(4.0, false, vec![]));
    assert!(!ws.rotation_dial_open());
}

#[test]
fn escape_closes_the_dial_and_another_tool_does_too() {
    let (mut ws, ctx) = laid_out();
    ws.toggle_rotation_dial();
    pass(
        &mut ws,
        &ctx,
        input(1.0, vec![key(Key::Escape, Modifiers::NONE)]),
    );
    assert!(!ws.rotation_dial_open());
    assert_eq!(ws.tool(), Tool::Select, "back to the tool before");
    ws.toggle_rotation_dial();
    ws.set_tool(Tool::Pen);
    assert!(!ws.rotation_dial_open());
}

#[test]
fn dragging_with_the_rotation_tool_turns_the_view_and_shift_snaps() {
    let (mut ws, ctx) = laid_out();
    ws.set_tool(Tool::RotateView);
    let center = ws.stage_panel.center();
    let from = center + Vec2::new(150.0, 0.0);
    // A quarter turn clockwise about the panel's center: from its right to below it.
    let mut time = 1.0;
    pass(&mut ws, &ctx, input(time, vec![Event::PointerMoved(from)]));
    time += 0.02;
    pass(
        &mut ws,
        &ctx,
        input(time, vec![press(from, true, Modifiers::NONE)]),
    );
    for step in 1..=12 {
        time += 0.02;
        let angle = std::f32::consts::FRAC_PI_2 * step as f32 / 12.0;
        let at = center + egui::emath::Rot2::from_angle(angle) * Vec2::new(150.0, 0.0);
        pass(&mut ws, &ctx, input(time, vec![Event::PointerMoved(at)]));
    }
    let to = center + Vec2::new(0.0, 150.0);
    time += 0.02;
    pass(
        &mut ws,
        &ctx,
        input(time, vec![press(to, false, Modifiers::NONE)]),
    );
    assert!(
        (ws.stage_rotation_degrees() - 90.0).abs() <= 1.0,
        "{}",
        ws.stage_rotation_degrees()
    );
    assert_eq!(drawing_count(&ws), 1, "the document did not change");
    // With Shift a drag lands on a step of 15 degrees.
    ws.reset_stage_rotation();
    let near = center + egui::emath::Rot2::from_angle(0.4) * Vec2::new(150.0, 0.0);
    let shifted = |time, events| with_mods(time, Modifiers::SHIFT, events);
    time += 0.1;
    pass(
        &mut ws,
        &ctx,
        shifted(time, vec![Event::PointerMoved(from)]),
    );
    time += 0.02;
    pass(
        &mut ws,
        &ctx,
        shifted(time, vec![press(from, true, Modifiers::SHIFT)]),
    );
    for step in 1..=6 {
        time += 0.02;
        let at =
            center + egui::emath::Rot2::from_angle(0.4 * step as f32 / 6.0) * Vec2::new(150.0, 0.0);
        pass(&mut ws, &ctx, shifted(time, vec![Event::PointerMoved(at)]));
    }
    time += 0.02;
    pass(
        &mut ws,
        &ctx,
        shifted(time, vec![press(near, false, Modifiers::SHIFT)]),
    );
    // 0.4 radians is about 23 degrees: the nearest step is 30.
    assert_eq!(ws.stage_rotation_degrees(), 30.0);
    pass(
        &mut ws,
        &ctx,
        with_mods(time + 0.1, Modifiers::NONE, vec![]),
    );
}

#[test]
fn a_click_on_a_turned_stage_picks_what_shows_under_the_pointer() {
    let (mut ws, ctx) = laid_out();
    ws.set_stage_rotation_degrees(90.0);
    run(&mut ws, &ctx, 1.0);
    let mut time = 1.0;
    // Where the rectangle would be if the stage were upright is empty now.
    let upright = {
        let view = flare_ui::view::StageView::default();
        view.to_screen(ws.stage_panel, ws.stage_extent(), Pos2::new(750.0, 450.0))
    };
    let shown = screen_of(&ws, Pos2::new(750.0, 450.0));
    assert!((upright - shown).length() > 80.0, "the turn moved it");
    click(&mut ws, &ctx, &mut time, upright);
    assert!(!ws.has_selection());
    assert!(ws.stage_panel.contains(shown), "it shows on the panel");
    time += 1.0;
    click(&mut ws, &ctx, &mut time, shown);
    assert!(
        ws.has_selection(),
        "the rectangle where it shows is selected"
    );
}

#[test]
fn the_rotation_tool_has_its_letter_and_a_double_click_sets_the_view_upright() {
    let (mut ws, ctx) = laid_out();
    pass(
        &mut ws,
        &ctx,
        input(1.0, vec![key(Key::D, Modifiers::NONE)]),
    );
    assert_eq!(ws.tool(), Tool::RotateView);
    ws.set_stage_rotation_degrees(40.0);
    let at = ws.stage_panel.center() + Vec2::new(80.0, 40.0);
    let mut time = 1.0;
    click(&mut ws, &ctx, &mut time, at);
    click(&mut ws, &ctx, &mut time, at);
    assert_eq!(ws.stage_rotation_degrees(), 0.0);
}

#[test]
fn a_turned_stage_is_painted_turned_and_the_rulers_are_not() {
    let (mut ws, ctx) = laid_out();
    ws.set_stage_rotation_degrees(30.0);
    let mut out = ctx.run_ui(input(1.0, vec![]), |ui| ws.show(ui));
    out.textures_delta.clear();
    let panel = ws.stage_panel;
    let rects: Vec<&egui::epaint::RectShape> = out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Rect(r) if panel.contains_rect(r.rect.shrink(1.0)) => Some(r),
            _ => None,
        })
        .collect();
    // The stage's own color is a rectangle turned by the view's angle.
    assert!(
        rects
            .iter()
            .any(|r| (r.angle - 30.0_f32.to_radians()).abs() < 1e-4),
        "the stage is turned"
    );
    // The rulers along the panel's edges stay square to the screen.
    assert!(
        rects
            .iter()
            .any(|r| r.angle == 0.0 && r.rect.min == panel.min && r.rect.height() <= 16.0),
        "the top ruler is upright"
    );
    // Its numbers are gone: they would not line up with a turned stage.
    let numbers = |out: &egui::FullOutput| {
        out.shapes
            .iter()
            .filter(|s| {
                matches!(&s.shape, egui::epaint::Shape::Text(t)
                if (panel.min.y..=panel.min.y + 16.0).contains(&t.pos.y)
                    && panel.x_range().contains(t.pos.x)
                    && t.galley.job.text.parse::<f32>().is_ok())
            })
            .count()
    };
    assert_eq!(numbers(&out), 0);
    ws.reset_stage_rotation();
    let mut upright = ctx.run_ui(input(1.1, vec![]), |ui| ws.show(ui));
    upright.textures_delta.clear();
    assert!(numbers(&upright) > 0, "upright, the ruler is numbered");
}
