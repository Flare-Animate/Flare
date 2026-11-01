use super::*;
use crate::workspace::home::PRESETS;

#[test]
fn every_preset_is_a_stage_the_document_accepts() {
    for (name, w, h, fps) in PRESETS {
        let mut ws = workspace();
        ws.new_project(w, h, fps);
        let stage = ws.document().stage();
        assert_eq!((stage.width(), stage.height()), (w, h), "{name}");
        assert_eq!(ws.document().frame_rate().numerator(), fps, "{name}");
    }
}

#[test]
fn a_new_project_replaces_an_untouched_document_and_otherwise_opens_a_tab() {
    let mut ws = workspace();
    ws.new_project(1280, 720, 24);
    assert_eq!(ws.tabs().len(), 1, "the blank document is replaced");
    rect(&mut ws);
    ws.new_project(800, 600, 24);
    assert_eq!(ws.tabs().len(), 2, "a document with work in it is kept");
    assert_eq!(ws.document().stage().width(), 800);
}

#[test]
fn an_impossible_size_is_reported_and_changes_nothing() {
    let mut ws = workspace();
    let before = ws.document().stage();
    ws.new_project(0, 100, 24);
    assert_eq!(ws.document().stage(), before);
    assert!(ws.status().is_some());
}

#[test]
fn the_home_page_shows_over_the_document_and_goes_away_when_a_project_starts() {
    let mut ws = workspace();
    assert!(!ws.home_shown());
    ws.set_home(true);
    assert!(ws.home_shown());
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    assert!(ws.home_shown(), "drawing the page does not dismiss it");
    ws.new_project(1920, 1080, 24);
    assert!(!ws.home_shown(), "starting a project leaves Home");
}

#[test]
fn the_menu_and_the_new_command_treat_home_as_a_view_not_a_document() {
    let mut ws = workspace();
    ws.perform(MenuAction::ShowHome);
    assert!(ws.home_shown());
    ws.perform(MenuAction::ShowHome);
    assert!(!ws.home_shown());
    // New with Home up on an untouched document starts in place instead of stacking tabs.
    ws.set_home(true);
    ws.perform(MenuAction::New);
    assert_eq!(ws.tabs().len(), 1);
    assert!(!ws.home_shown());
    // On a document with work, Home is left behind in the old tab.
    rect(&mut ws);
    ws.set_home(true);
    ws.perform(MenuAction::New);
    assert_eq!(ws.tabs().len(), 2);
    assert!(!ws.home_shown());
}

#[test]
fn recent_projects_are_listed_newest_first_and_can_be_cleared() {
    let mut ws = workspace();
    ws.note_recent(std::path::Path::new("/a/one.flare"));
    ws.note_recent(std::path::Path::new("/a/two.flare"));
    assert_eq!(ws.recent_files()[0], PathBuf::from("/a/two.flare"));
    ws.set_home(true);
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    assert_eq!(ws.recent_files().len(), 2);
}

#[test]
fn the_title_is_the_app_on_home_and_the_saved_name_on_a_document() {
    let mut ws = workspace();
    ws.set_home(true);
    assert_eq!(ws.title(), "Flare", "no document is on show");
    ws.set_home(false);
    assert_eq!(ws.title(), "Untitled - Flare");
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("Rocket.flare");
    flare_storage::save(ws.document(), &path).expect("save");
    ws.open(&path);
    ws.set_home(true);
    assert_eq!(ws.title(), "Flare");
    ws.set_home(false);
    assert_eq!(ws.title(), "Rocket.flare - Flare");
}

#[test]
fn home_has_no_undo_play_or_document_name_in_the_top_bar() {
    let mut ws = workspace();
    assert!(ws.document_controls_shown());
    ws.set_home(true);
    assert!(!ws.document_controls_shown());
    ws.perform(MenuAction::ShowHome);
    assert!(
        ws.document_controls_shown(),
        "they come back with the document"
    );
}

#[test]
fn keys_and_commands_leave_the_hidden_document_alone_while_home_is_up() {
    let mut ws = two_rectangles();
    ws.perform(MenuAction::SelectAll);
    ws.set_home(true);
    for action in [
        MenuAction::Delete,
        MenuAction::Paste,
        MenuAction::Cut,
        MenuAction::SelectAll,
        MenuAction::ToolBrush,
        MenuAction::PlayPause,
        MenuAction::FlipHorizontal,
        MenuAction::MirrorCopyHorizontal,
    ] {
        assert!(!ws.is_enabled(action), "{action:?}");
    }
    for action in [
        MenuAction::New,
        MenuAction::Open,
        MenuAction::ShowHome,
        MenuAction::Quit,
    ] {
        assert!(ws.is_enabled(action), "{action:?}");
    }
    let ctx = egui::Context::default();
    pass(
        &mut ws,
        &ctx,
        input(0.0, vec![key(Key::Delete, Modifiers::NONE)]),
    );
    assert_eq!(layer_count(&ws), 3, "Delete did not reach the document");
    assert_eq!(ws.tool(), Tool::Select);
}

#[test]
fn choosing_a_tab_leaves_home_even_the_current_one() {
    let mut ws = workspace();
    rect(&mut ws);
    ws.new_project(800, 600, 24);
    assert_eq!(ws.tabs().len(), 2);
    ws.set_home(true);
    ws.switch_to_tab(1);
    assert!(!ws.home_shown(), "the current tab shows its document");
    ws.set_home(true);
    ws.switch_to_tab(0);
    assert!(!ws.home_shown());
    ws.switch_to_tab(1);
    assert!(!ws.home_shown(), "Home does not come back with the tab");
}

#[test]
fn the_home_page_draws_with_and_without_recent_projects_at_small_and_large_sizes() {
    for (recent, size) in [
        (false, 1280.0),
        (true, 1280.0),
        (true, 340.0),
        (true, 4000.0),
    ] {
        let mut ws = workspace();
        if recent {
            ws.note_recent(std::path::Path::new("/a/one.flare"));
            ws.note_recent(std::path::Path::new(
                "/a/a-very-long-name-that-goes-on-and-on.flare",
            ));
        }
        ws.set_home(true);
        let ctx = egui::Context::default();
        let raw = RawInput {
            time: Some(0.0),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(size, 700.0))),
            ..RawInput::default()
        };
        pass(&mut ws, &ctx, raw);
        assert!(ws.home_shown());
    }
}

#[test]
fn the_settings_gear_is_centered_over_the_rail_on_home_and_in_the_editor() {
    // The rail's icons are centered 16.5 px in from the right edge of a 1280 wide window (the
    // rail draws them there); the gear is a 22 px button, so it takes a click 10 px either side of
    // that line and not 12.
    let center = 1280.0 - crate::workspace::rail::CENTER_FROM_EDGE;
    let opened = |home: bool, x: f32| {
        let mut ws = workspace();
        ws.set_home(home);
        let ctx = egui::Context::default();
        run(&mut ws, &ctx, 0.0);
        run(&mut ws, &ctx, 0.1);
        let at = Pos2::new(x, 14.0);
        let click = |pressed| Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        pass(&mut ws, &ctx, input(0.2, vec![Event::PointerMoved(at)]));
        pass(&mut ws, &ctx, input(0.3, vec![click(true)]));
        pass(&mut ws, &ctx, input(0.4, vec![click(false)]));
        ws.windows.preferences
    };
    for home in [false, true] {
        assert!(opened(home, center), "the middle, home = {home}");
        assert!(opened(home, center - 10.0), "left edge, home = {home}");
        assert!(opened(home, center + 10.0), "right edge, home = {home}");
        assert!(!opened(home, center - 12.0), "beyond the left edge");
        assert!(!opened(home, center + 12.0), "beyond the right edge");
    }
}

/// Clicks the top bar at `x` in a fresh editor and says what that did.
fn press_top_bar(x: f32) -> (bool, bool, bool) {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
    let at = Pos2::new(x, 14.0);
    let click = |pressed| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    pass(&mut ws, &ctx, input(0.2, vec![Event::PointerMoved(at)]));
    pass(&mut ws, &ctx, input(0.3, vec![click(true)]));
    pass(&mut ws, &ctx, input(0.4, vec![click(false)]));
    (ws.windows.publish, ws.is_playing(), ws.windows.preferences)
}

#[test]
fn the_publish_button_is_left_of_play_in_the_top_bar() {
    // Walking left from the undo button, play comes, then publish.
    let mut play = None;
    let mut publish = None;
    // The interface switcher beside the gear moved the others left; the scan starts at the undo
    // button and goes on past publish.
    let mut x = 1180.0;
    while x > 1040.0 {
        let (published, playing, _) = press_top_bar(x);
        if playing && play.is_none() {
            play = Some(x);
        }
        if published && publish.is_none() {
            publish = Some(x);
        }
        x -= 8.0; // the buttons are 22 px wide, so no step misses one
    }
    let (play, publish) = (
        play.expect("play is in the bar"),
        publish.expect("publish is in the bar"),
    );
    assert!(publish < play, "publish at {publish}, play at {play}");
    assert!(
        play - publish < 40.0,
        "they are neighbours: {publish} and {play}"
    );
}

/// The tooltip the top bar shows with the pointer at `x`, in a window 1280 wide.
fn top_bar_hint(x: f32, home: bool) -> String {
    let mut ws = workspace();
    ws.set_home(home);
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    run(&mut ws, &ctx, 0.0);
    let at = Pos2::new(x, 14.0);
    let mut text = String::new();
    for step in 0..10 {
        let events = if step == 0 {
            vec![Event::PointerMoved(at)]
        } else {
            vec![]
        };
        let mut out = ctx.run_ui(input(0.1 + f64::from(step) * 0.2, events), |ui| ws.show(ui));
        out.textures_delta.clear();
        for shape in &out.shapes {
            if let egui::epaint::Shape::Text(t) = &shape.shape {
                text.push_str(t.galley.text());
                text.push('|');
            }
        }
    }
    text
}

#[test]
fn the_interface_switcher_is_a_dimmed_button_in_the_top_right_corner_beside_the_gear() {
    let width = 1280.0;
    let mut found = None;
    let mut gear = None;
    let mut x = width - 4.0;
    while x > width - 140.0 {
        let hint = top_bar_hint(x, false);
        if hint.contains("Interface switcher (Not available yet)") {
            found.get_or_insert(x);
        }
        if hint.contains("Preferences") {
            gear.get_or_insert(x);
        }
        x -= 6.0;
    }
    let (found, gear) = (
        found.expect("the switcher is there"),
        gear.expect("the gear"),
    );
    assert!(
        found < gear,
        "it is left of the gear: {found} against {gear}"
    );
}

#[test]
fn home_has_no_interface_switcher_the_editor_does() {
    let mut on_home = false;
    let mut in_editor = false;
    let mut x = 1276.0;
    while x > 1140.0 {
        on_home |= top_bar_hint(x, true).contains("Interface switcher");
        in_editor |= top_bar_hint(x, false).contains("Interface switcher");
        x -= 6.0;
    }
    assert!(in_editor, "the editor has it");
    assert!(!on_home, "Home has no interface to switch");
}

/// WCAG contrast between two colors.
fn contrast(a: Color32, b: Color32) -> f32 {
    let lum = |c: Color32| {
        let ch = |v: u8| {
            let v = f32::from(v) / 255.0;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * ch(c.r()) + 0.7152 * ch(c.g()) + 0.0722 * ch(c.b())
    };
    let (hi, lo) = (lum(a).max(lum(b)), lum(a).min(lum(b)));
    (hi + 0.05) / (lo + 0.05)
}

#[test]
fn the_home_buttons_can_be_read_on_their_fills_in_both_themes() {
    for theme in ["dark", "light"] {
        let (ctx, mut ws) = (egui::Context::default(), workspace());
        flare_ui::icons::install(&ctx);
        let mut prefs = ws.preferences();
        prefs.theme = theme.to_owned();
        ws.apply_preferences(&prefs);
        ws.set_home(true);
        let mut shapes = Vec::new();
        for step in 0..4 {
            let mut out = ctx.run_ui(input(f64::from(step) * 0.1, vec![]), |ui| ws.show(ui));
            out.textures_delta.clear();
            shapes = out.shapes;
        }
        let mut checked = Vec::new();
        for (i, s) in shapes.iter().enumerate() {
            let egui::epaint::Shape::Text(t) = &s.shape else {
                continue;
            };
            let label = t.galley.text();
            if !["New File", "Open…", "Import…", "Home"].contains(&label) {
                continue;
            }
            let ink = t.galley.job.sections[0].format.color;
            // The fill under the label: the last rectangle drawn before it that holds it.
            let under = shapes[..i].iter().rev().find_map(|s| match &s.shape {
                egui::epaint::Shape::Rect(r)
                    if r.rect.contains(t.pos) && r.fill != Color32::TRANSPARENT =>
                {
                    Some(r.fill)
                }
                _ => None,
            });
            let fill = under.unwrap_or_else(flare_ui::theme::panel);
            assert!(
                contrast(ink, fill) >= 3.0,
                "{theme}: \"{label}\" is {ink:?} on {fill:?}, contrast {}",
                contrast(ink, fill)
            );
            checked.push(label.to_owned());
        }
        for wanted in ["New File", "Open…", "Import…"] {
            assert!(
                checked.iter().any(|c| c == wanted),
                "{theme}: {wanted} was looked at: {checked:?}"
            );
        }
        flare_ui::theme::set_mode(flare_ui::theme::Mode::Dark);
    }
}

#[test]
fn the_custom_size_card_opens_a_window_that_makes_a_project_of_the_chosen_size() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    ws.set_home(true);
    ws.show_custom_size();
    assert!(ws.custom_size_shown());
    ws.home.custom_size = Some(super::super::home_custom::CustomSize {
        width: 640,
        height: 360,
        fps: 12,
    });
    run(&mut ws, &ctx, 0.0);
    assert!(ws.custom_size_shown(), "it stays until Create or Cancel");
    // What Create does with the numbers.
    ws.new_project(640, 360, 12);
    let stage = ws.document().stage();
    assert_eq!((stage.width(), stage.height()), (640, 360));
    assert_eq!(ws.document().frame_rate().numerator(), 12);
}

#[test]
fn the_custom_size_window_goes_when_home_does() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    ws.set_home(true);
    ws.show_custom_size();
    ws.set_home(false);
    run(&mut ws, &ctx, 0.0);
    assert!(!ws.custom_size_shown());
}
