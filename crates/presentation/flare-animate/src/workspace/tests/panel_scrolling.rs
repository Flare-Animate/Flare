use super::*;

/// The panels that are given a scroll area where they are hosted, rather than keeping their own.
const HOSTED: [Pane; 8] = [
    Pane::Color,
    Pane::Align,
    Pane::Transform,
    Pane::Info,
    Pane::FindReplace,
    Pane::Assets,
    Pane::Rigs,
    Pane::StopMotion,
];

/// One pass of `pane` in `area`; returns where each accesskit node is, by its id.
fn nodes(
    ws: &mut Workspace,
    ctx: &egui::Context,
    pane: Pane,
    area: Rect,
    time: f64,
    events: Vec<Event>,
) -> Vec<(egui::accesskit::NodeId, Rect)> {
    let mut out = ctx.run_ui(input(time, events), |ui| {
        ui.scope_builder(egui::UiBuilder::new().max_rect(area), |ui| {
            ws.pane_contents(ui, pane)
        });
    });
    out.textures_delta.clear();
    let update = out.platform_output.accesskit_update;
    update
        .into_iter()
        .flat_map(|u| u.nodes)
        .filter_map(|(id, n)| {
            let b = n.bounds()?;
            let rect = Rect::from_min_max(
                Pos2::new(b.x0 as f32, b.y0 as f32),
                Pos2::new(b.x1 as f32, b.y1 as f32),
            );
            Some((id, rect))
        })
        .collect()
}

/// Whether the content of `pane` runs past the bottom of a short `area`, and if it does, whether
/// a turn of the wheel over it brings the lowest widget up.
fn overflows_and_scrolls(pane: Pane, area: Rect) -> Option<bool> {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut before = Vec::new();
    for step in 0..3 {
        before = nodes(&mut ws, &ctx, pane, area, f64::from(step) * 0.1, vec![]);
    }
    let (lowest, at) = before
        .iter()
        .filter(|(_, r)| r.width() < area.width() * 2.0 && r.height() < area.height())
        .max_by(|a, b| a.1.bottom().total_cmp(&b.1.bottom()))
        .copied()?;
    if at.bottom() <= area.bottom() {
        return None;
    }
    let wheel = Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: Vec2::new(0.0, -2000.0),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
        modifiers: Modifiers::NONE,
    };
    let mut after = Vec::new();
    let mut time = 0.5;
    for events in [vec![Event::PointerMoved(area.center())], vec![wheel]]
        .into_iter()
        .chain((0..30).map(|_| vec![]))
    {
        after = nodes(&mut ws, &ctx, pane, area, time, events);
        time += 0.05;
    }
    let moved = after
        .iter()
        .find(|(id, _)| *id == lowest)
        .is_some_and(|(_, r)| r.bottom() < at.bottom() - 1.0);
    Some(moved)
}

#[test]
fn every_panel_without_a_scroll_area_of_its_own_scrolls_when_it_is_short() {
    let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(240.0, 90.0));
    let mut overflowed = Vec::new();
    for pane in HOSTED {
        if let Some(moved) = overflows_and_scrolls(pane, area) {
            assert!(moved, "{pane:?} runs past its bottom but does not scroll");
            overflowed.push(pane);
        }
    }
    // The Color panel is always taller than this, so the test has something to scroll.
    assert!(overflowed.contains(&Pane::Color), "{overflowed:?}");
}
