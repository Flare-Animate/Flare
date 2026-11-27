use super::*;
use crate::workspace::types::ColorTarget;

#[test]
fn a_swatch_sets_the_color_being_set_and_the_persons_own_are_kept_with_the_preferences() {
    let mut ws = workspace();
    let blue = Rgba8::opaque(0, 51, 255);
    ws.choose_swatch(blue);
    assert_eq!(ws.paint.fill, Color32::from_rgb(0, 51, 255));
    ws.color_target = ColorTarget::Stroke;
    ws.choose_swatch(Rgba8::opaque(255, 0, 0));
    assert_eq!(ws.paint.stroke_color, Color32::from_rgb(255, 0, 0));
    assert_eq!(
        ws.paint.fill,
        Color32::from_rgb(0, 51, 255),
        "the fill is untouched"
    );

    assert!(ws.add_swatch(), "the stroke color is kept");
    assert!(!ws.add_swatch(), "once");
    ws.color_target = ColorTarget::Fill;
    assert!(ws.add_swatch());
    assert_eq!(
        ws.paint.swatches.colors(),
        &[Rgba8::opaque(255, 0, 0), blue]
    );
    let preferences = ws.preferences();
    assert_eq!(
        preferences.swatches,
        vec![[255, 0, 0, 255], [0, 51, 255, 255]]
    );
    let mut next = workspace();
    next.apply_preferences(&preferences);
    assert_eq!(next.paint.swatches, ws.paint.swatches);
    ws.remove_swatch(0);
    ws.remove_swatch(9);
    assert_eq!(ws.paint.swatches.colors(), &[blue]);
}

#[test]
fn a_full_list_says_so() {
    let mut ws = workspace();
    for i in 0..flare_core::swatches::MAX_CUSTOM_SWATCHES {
        let i = u8::try_from(i).expect("small");
        ws.paint.fill = Color32::from_rgb(i, 0, 0);
        assert!(ws.add_swatch());
    }
    ws.paint.fill = Color32::from_rgb(0, 0, 9);
    assert!(!ws.add_swatch());
    assert!(
        ws.status().is_some_and(|s| s.contains("120")),
        "{:?}",
        ws.status()
    );
}

#[test]
fn the_panel_is_in_the_strip_and_a_click_on_a_cell_picks_its_color() {
    let mut ws = workspace();
    assert!(ws.panels.layout.in_strip(Pane::Swatches));
    ws.perform(MenuAction::ShowSwatches);
    assert!(ws.pane_is_front(Pane::Swatches));
    let ctx = egui::Context::default();
    let mut panel = Rect::NOTHING;
    for step in 0..3 {
        let mut out = ctx.run_ui(input(f64::from(step) * 0.1, vec![]), |ui| {
            panel = ui.max_rect();
            ws.swatches_panel(ui);
        });
        out.textures_delta.clear();
    }
    // The first cell of the greys, at the top left under the label, is black.
    ws.paint.fill = Color32::WHITE;
    let at = panel.min + egui::vec2(4.0, 26.0);
    let click = |pressed| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    for (time, events) in [
        (0.5, vec![Event::PointerMoved(at)]),
        (0.6, vec![click(true)]),
        (0.7, vec![click(false)]),
    ] {
        let mut out = ctx.run_ui(input(time, events), |ui| ws.swatches_panel(ui));
        out.textures_delta.clear();
    }
    assert_eq!(ws.paint.fill, Color32::BLACK);
}

#[test]
fn the_panel_has_skin_tones_and_a_click_on_one_picks_it() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let tone = flare_core::swatches::skin_tones()[0];
    let hex = format!("#{:02X}{:02X}{:02X}", tone.r, tone.g, tone.b);
    let mut cell = None;
    for step in 0..3 {
        let mut out = ctx.run_ui(input(f64::from(step) * 0.1, vec![]), |ui| {
            ws.swatches_panel(ui);
        });
        out.textures_delta.clear();
        let nodes = out.platform_output.accesskit_update.map(|u| u.nodes);
        cell = nodes.into_iter().flatten().find_map(|(_, n)| {
            let b = n.bounds()?;
            (n.label() == Some(hex.as_str()))
                .then(|| egui::pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32))
        });
    }
    let at = cell.expect("the first skin tone is a button named by its hex");
    let click = |pressed| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    for (time, events) in [
        (0.5, vec![Event::PointerMoved(at)]),
        (0.6, vec![click(true)]),
        (0.7, vec![click(false)]),
    ] {
        let mut out = ctx.run_ui(input(time, events), |ui| ws.swatches_panel(ui));
        out.textures_delta.clear();
    }
    assert_eq!(ws.paint.fill, Color32::from_rgb(tone.r, tone.g, tone.b));
}

/// One pass of the Swatches panel in `area`; returns the middle of the cell named `hex`.
fn skin_cell(
    ws: &mut Workspace,
    ctx: &egui::Context,
    area: Rect,
    time: f64,
    events: Vec<Event>,
    hex: &str,
) -> Option<egui::Pos2> {
    let mut out = ctx.run_ui(input(time, events), |ui| {
        ui.scope_builder(egui::UiBuilder::new().max_rect(area), |ui| {
            ws.swatches_panel(ui)
        });
    });
    out.textures_delta.clear();
    let nodes = out.platform_output.accesskit_update.map(|u| u.nodes);
    nodes.into_iter().flatten().find_map(|(_, n)| {
        let b = n.bounds()?;
        (n.label() == Some(hex))
            .then(|| egui::pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32))
    })
}

#[test]
fn a_short_panel_scrolls_down_to_the_skin_tones() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let area = Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(240.0, 200.0));
    let tone = flare_core::swatches::skin_tones()[12];
    let hex = format!("#{:02X}{:02X}{:02X}", tone.r, tone.g, tone.b);
    let mut cell = None;
    for step in 0..3 {
        cell = skin_cell(&mut ws, &ctx, area, f64::from(step) * 0.1, vec![], &hex);
    }
    let before = cell.expect("the skin tones are laid out");
    assert!(!area.contains(before), "below a short panel at first");

    let pointer = area.center();
    let wheel = Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: Vec2::new(0.0, -2000.0),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
        modifiers: Modifiers::NONE,
    };
    let mut time = 0.5;
    for events in [vec![Event::PointerMoved(pointer)], vec![wheel]]
        .into_iter()
        .chain((0..30).map(|_| vec![]))
    {
        cell = skin_cell(&mut ws, &ctx, area, time, events, &hex);
        time += 0.05;
    }
    let after = cell.expect("still laid out");
    assert!(area.contains(after), "scrolled into view: {after:?}");

    let click = |pressed| Event::PointerButton {
        pos: after,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    for events in [
        vec![Event::PointerMoved(after)],
        vec![click(true)],
        vec![click(false)],
    ] {
        skin_cell(&mut ws, &ctx, area, time, events, &hex);
        time += 0.1;
    }
    assert_eq!(ws.paint.fill, Color32::from_rgb(tone.r, tone.g, tone.b));
}
