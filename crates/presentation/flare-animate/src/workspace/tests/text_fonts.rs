//! Fonts for text: a Google Fonts family the machine lacks, which the shell is asked to download
//! while the text waits for it, the list of families, and the editor field that takes the typing.

use super::*;

/// A small Google font (Squada One, SIL Open Font License, licence text beside it).
const SQUADA: &[u8] =
    include_bytes!("../../../../../foundation/flare-text/data/test/SquadaOne-Regular.ttf");

fn squada_workspace() -> Workspace {
    let mut ws = text_workspace();
    ws.text.family = "Squada One".into();
    ws
}

#[test]
fn text_in_a_google_font_waits_for_the_shell_to_download_it() {
    let mut ws = squada_workspace();
    type_text(&mut ws, Pos2::new(10.0, 10.0), "Flare");
    assert_eq!(drawing_count(&ws), 0, "set before its font arrived");
    assert_eq!(ws.take_font_downloads(), ["Squada One"]);
    assert!(ws.take_font_downloads().is_empty(), "asked twice");
    assert!(
        ws.log.iter().any(|l| l.contains("Squada One")),
        "{:?}",
        ws.log
    );

    ws.font_downloaded("Squada One".into(), Ok(vec![SQUADA.to_vec()]));
    let placed = drawn_shape(&ws, 1);
    assert_eq!(placed.text().map(|t| t.family()), Some("Squada One"));
    assert!(
        ws.text.families.iter().any(|f| f == "Squada One"),
        "the downloaded family is listed with the others"
    );
    assert!(!ws.text.google.contains(&"Squada One"));
}

#[test]
fn a_family_that_cannot_be_downloaded_is_reported_and_the_text_falls_back() {
    let mut ws = squada_workspace();
    type_text(&mut ws, Pos2::new(10.0, 10.0), "Flare");
    assert_eq!(ws.take_font_downloads(), ["Squada One"]);
    ws.font_downloaded("Squada One".into(), Err("offline".into()));
    assert!(
        ws.log
            .iter()
            .any(|l| l.contains("Squada One") && l.contains("offline")),
        "{:?}",
        ws.log
    );
    let placed = drawn_shape(&ws, 1);
    assert_eq!(placed.text().map(|t| t.family()), Some("Squada One"));
    // Not asked for again: the next text in it is set at once.
    type_text(&mut ws, Pos2::new(10.0, 80.0), "Again");
    assert!(ws.take_font_downloads().is_empty());
}

#[test]
fn the_family_list_offers_google_fonts_that_are_not_here() {
    let mut ws = workspace();
    ws.ask_font_families();
    assert!(ws.text.google.contains(&"Roboto") || ws.text.families.iter().any(|f| f == "Roboto"));
    assert!(
        !ws.text.google.contains(&"Source Sans 3"),
        "the bundled font is here"
    );
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    let mut ws = squada_workspace();
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
}

#[test]
fn past_the_limit_of_downloads_text_falls_back_instead_of_waiting() {
    let mut ws = squada_workspace();
    let busy: Vec<String> = (0..16).map(|n| format!("busy {n}")).collect();
    ws.fonts_wanted(busy);
    assert_eq!(ws.take_font_downloads().len(), 16);
    type_text(&mut ws, Pos2::new(10.0, 10.0), "Flare");
    assert!(ws.take_font_downloads().is_empty());
    assert_eq!(drawn_shape(&ws, 1).text().map(|t| t.text()), Some("Flare"));
    assert!(
        ws.log.iter().any(|l| l.contains("too many")),
        "{:?}",
        ws.log
    );
}

/// One pass with `events`; returns each accesskit node's role, label, value and bounds.
fn frame(
    ws: &mut Workspace,
    ctx: &egui::Context,
    time: f64,
    events: Vec<Event>,
) -> Vec<(egui::accesskit::Role, String, String, Rect)> {
    let mut out = ctx.run_ui(input(time, events), |ui| ws.show(ui));
    out.textures_delta.clear();
    let Some(update) = out.platform_output.accesskit_update else {
        return Vec::new();
    };
    update
        .nodes
        .iter()
        .filter_map(|(_, n)| {
            let b = n.bounds()?;
            let rect = Rect::from_min_max(
                Pos2::new(b.x0 as f32, b.y0 as f32),
                Pos2::new(b.x1 as f32, b.y1 as f32),
            );
            let text = |s: Option<&str>| s.unwrap_or_default().to_owned();
            Some((n.role(), text(n.label()), text(n.value()), rect))
        })
        .collect()
}

fn click_at(at: Pos2) -> Vec<Event> {
    let button = |pressed| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    vec![Event::PointerMoved(at), button(true), button(false)]
}

#[test]
fn the_family_list_can_be_filtered_by_typing_and_a_click_on_the_filter_keeps_it_open() {
    use egui::accesskit::Role;
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    ctx.enable_accesskit();
    let mut ws = text_workspace();
    let mut nodes = Vec::new();
    for step in 0..3 {
        nodes = frame(&mut ws, &ctx, f64::from(step) * 0.1, vec![]);
    }
    let combo = nodes
        .iter()
        .find(|(role, label, value, _)| {
            *role == Role::ComboBox && (label.contains("Hack") || value.contains("Hack"))
        })
        .map(|n| n.3)
        .unwrap_or_else(|| panic!("no family list: {nodes:?}"));
    let mut time = 0.3;
    let mut step = |ws: &mut Workspace, events| {
        time += 0.1;
        frame(ws, &ctx, time, events)
    };
    step(&mut ws, click_at(combo.center()));
    let nodes = step(&mut ws, vec![]);
    let filter = nodes
        .iter()
        .find(|(role, _, _, rect)| *role == Role::TextInput && rect.min.y > combo.min.y)
        .map(|n| n.3)
        .unwrap_or_else(|| panic!("the list did not open with its filter: {nodes:?}"));
    // The bug: a click in the filter box closed the list.
    step(&mut ws, click_at(filter.center()));
    step(&mut ws, vec![Event::Text("squada".into())]);
    let nodes = step(&mut ws, vec![]);
    assert_eq!(ws.text.family_filter, "squada");
    let squada = nodes
        .iter()
        .find(|(_, label, _, _)| label == "Squada One")
        .map(|n| n.3)
        .unwrap_or_else(|| panic!("the list closed or did not filter: {nodes:?}"));
    assert!(
        !nodes.iter().any(|(_, label, _, _)| label == "Roboto"),
        "the list was not filtered"
    );
    step(&mut ws, click_at(squada.center()));
    let nodes = step(&mut ws, vec![]);
    assert_eq!(ws.text.family, "Squada One");
    assert!(
        !nodes.iter().any(|(_, label, _, _)| label == "Abel"),
        "the list stayed open after a choice"
    );
}

#[test]
fn the_text_being_typed_is_drawn_again_in_a_family_when_it_arrives() {
    let mut ws = squada_workspace();
    ws.text.edit = Some(PendingText {
        at: Pos2::new(10.0, 10.0),
        buffer: "Flare".into(),
        focused: true,
        editing: None,
    });
    ws.update_text_preview(5);
    let fallback = ws.text.preview.as_ref().and_then(|p| p.shape.clone());
    assert!(fallback.is_some(), "a preview in the fallback font at once");
    assert_eq!(ws.take_font_downloads(), ["Squada One"]);
    ws.font_downloaded("Squada One".into(), Ok(vec![SQUADA.to_vec()]));
    assert!(
        ws.log.iter().any(|l| l.contains("Downloaded Squada One")),
        "{:?}",
        ws.log
    );
    ws.update_text_preview(5);
    let arrived = ws.text.preview.as_ref().and_then(|p| p.shape.clone());
    assert!(arrived.is_some());
    assert_ne!(
        arrived.map(|s| s.path().to_vec()),
        fallback.map(|s| s.path().to_vec()),
        "still drawn in the fallback font"
    );
}

#[test]
fn a_family_list_that_arrives_after_the_list_opened_fills_it() {
    use egui::accesskit::Role;
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    ctx.enable_accesskit();
    let mut ws = text_workspace();
    // Opened before the font worker has listed the families, as on a first use.
    ws.text.families_asked = true;
    let mut nodes = Vec::new();
    for step in 0..3 {
        nodes = frame(&mut ws, &ctx, f64::from(step) * 0.1, vec![]);
    }
    let combo = nodes
        .iter()
        .find(|(role, _, value, _)| *role == Role::ComboBox && value.contains("Hack"))
        .map(|n| n.3)
        .expect("the family list");
    frame(&mut ws, &ctx, 0.5, click_at(combo.center()));
    frame(&mut ws, &ctx, 0.6, vec![]);
    ws.families_arrived((0..300).map(|n| format!("Font {n:03}")).collect());
    let mut nodes = Vec::new();
    for step in 0..3 {
        nodes = frame(&mut ws, &ctx, 0.7 + f64::from(step) * 0.1, vec![]);
    }
    let filter = nodes
        .iter()
        .find(|(role, _, _, rect)| *role == Role::TextInput && rect.min.y > combo.max.y)
        .map(|n| n.3)
        .expect("the filter box");
    let list = nodes
        .iter()
        .find(|(role, _, _, rect)| *role == Role::ScrollBar && rect.min.y > filter.max.y)
        .map(|n| n.3)
        .expect("the list scrolls");
    assert!(list.height() > 200.0, "the list is small: {list:?}");
    // The drop-down itself must not be smaller than the list it holds: one that kept the size
    // it opened at cut the list to two or three rows, behind a scroll bar of its own.
    let cut = nodes.iter().find(|(role, _, _, rect)| {
        *role == Role::ScrollBar && rect.min.y <= filter.min.y + 1.0 && rect.max.y < list.max.y
    });
    assert!(
        cut.is_none(),
        "the drop-down cuts the list off: {cut:?} {list:?}"
    );
    let shown = nodes
        .iter()
        .filter(|(_, label, _, rect)| label.starts_with("Font ") && rect.min.y < list.max.y)
        .count();
    assert!(shown >= 5, "{shown} rows show");
    // The list scrolls by the height it is told each row has; rows drawn at another height
    // leave the last families out of reach at its end.
    let mut tops: Vec<f32> = nodes
        .iter()
        .filter(|(_, label, _, _)| label.starts_with("Font "))
        .map(|n| n.3.min.y)
        .collect();
    tops.sort_by(f32::total_cmp);
    let mut pitch = 0.0;
    let mut out = ctx.run_ui(input(9.0, vec![]), |ui| {
        pitch = crate::workspace::panels::character::family_row_height(ui)
            + ui.spacing().item_spacing.y;
    });
    out.textures_delta.clear();
    for pair in tops.windows(2) {
        assert!(
            (pair[1] - pair[0] - pitch).abs() < 0.5,
            "rows {pair:?}, told {pitch}"
        );
    }
}

#[test]
fn names_a_system_keeps_for_itself_are_not_offered() {
    let mut ws = workspace();
    ws.ask_font_families();
    assert!(!ws.text.families.is_empty());
    assert!(
        ws.text.families.iter().all(|name| !name.starts_with('.')),
        "{:?}",
        ws.text
            .families
            .iter()
            .filter(|n| n.starts_with('.'))
            .collect::<Vec<_>>()
    );
}

#[test]
fn selected_letters_in_the_editor_field_stay_unseen() {
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    let mut ws = text_workspace();
    ws.text.edit = Some(PendingText {
        at: Pos2::new(10.0, 10.0),
        buffer: "hello world".into(),
        focused: false,
        editing: None,
    });
    let select_all = key(
        Key::A,
        Modifiers {
            command: true,
            ..Modifiers::NONE
        },
    );
    let mut shown = Vec::new();
    for (step, events) in [vec![], vec![select_all], vec![]].into_iter().enumerate() {
        let mut out = ctx.run_ui(input(step as f64 * 0.1, events), |ui| ws.show(ui));
        out.textures_delta.clear();
        shown = out.shapes;
    }
    let mut letters = 0;
    for clipped in &shown {
        let egui::epaint::Shape::Text(text) = &clipped.shape else {
            continue;
        };
        if text.galley.text() != "hello world" {
            continue;
        }
        for row in &text.galley.rows {
            for vertex in &row.visuals.mesh.vertices {
                letters += 1;
                assert_eq!(vertex.color.a(), 0, "the field's own letters show");
            }
        }
    }
    assert!(letters > 0, "the editor field was not drawn");
}
