use super::*;



#[test]
fn the_top_bar_has_no_settings_gear_and_preferences_stay_in_the_menus() {
    // The bar is Animate's: menus, then Play, Publish and the workspace switcher by the window
    // buttons. Preferences are in the Edit menu, on Home too, so the corner the gear sat in does
    // nothing.
    for home in [false, true] {
        for x in [1250.0, 1263.0, 1274.0] {
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
            assert!(!ws.windows.preferences, "home = {home}, x = {x}");
        }
        let mut ws = workspace();
        ws.set_home(home);
        assert!(ws.is_enabled(MenuAction::Preferences), "home = {home}");
    }
}

/// Clicks the top bar at `x` in a fresh editor and says what that did.
fn press_top_bar(x: f32, native: bool) -> (bool, bool, bool) {
    let mut ws = workspace();
    ws.set_native_menu(native);
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
fn the_play_button_is_left_of_publish_in_the_top_bar() {
    // Walking left from the window buttons, the workspace switcher comes, then publish, then play.
    let mut play = None;
    let mut publish = None;
    // In these tests the window has no buttons of its own (the system draws them), so the three
    // top-right buttons take the corner of the 1280 wide window; the scan goes on past play.
    let mut x = 1276.0;
    while x > 1120.0 {
        let (published, playing, _) = press_top_bar(x, false);
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
    assert!(play < publish, "publish at {publish}, play at {play}");
    assert!(
        publish - play < 40.0,
        "they are neighbours: {play} and {publish}"
    );
}

/// The tooltip the top bar shows with the pointer at `x`, in a window 1280 wide.
fn top_bar_hint(x: f32, home: bool, native: bool) -> String {
    let mut ws = workspace();
    ws.set_native_menu(native);
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
fn the_interface_switcher_is_a_dimmed_button_in_the_top_right_corner() {
    let width = 1280.0;
    let mut found = None;
    let mut x = width - 2.0;
    while x > width - 140.0 {
        let hint = top_bar_hint(x, false, false);
        assert!(!hint.contains("Preferences"), "there is no gear at {x}");
        if hint.contains("Interface switcher (Not available yet)") {
            found.get_or_insert(x);
        }
        x -= 6.0;
    }
    let found = found.expect("the switcher is there");
    assert!(
        found > width - 40.0,
        "it is the nearest to the corner, where the window buttons are: {found}"
    );
}

#[test]
fn home_has_no_interface_switcher_the_editor_does() {
    let mut on_home = false;
    let mut in_editor = false;
    let mut x = 1276.0;
    while x > 1140.0 {
        on_home |= top_bar_hint(x, true, false).contains("Interface switcher");
        in_editor |= top_bar_hint(x, false, false).contains("Interface switcher");
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
            if !["New File", "Open…", "Home"].contains(&label) {
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
        for wanted in ["New File", "Open…"] {
            assert!(
                checked.iter().any(|c| c == wanted),
                "{theme}: {wanted} was looked at: {checked:?}"
            );
        }
        flare_ui::theme::set_mode(flare_ui::theme::Mode::Dark);
    }
}

#[test]
fn the_new_file_button_and_the_custom_size_card_open_the_new_document_window() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    ws.set_home(true);
    assert!(!ws.new_document_shown());
    ws.show_new_document();
    assert!(ws.new_document_shown());
    // The first preset, the sizes Flare has always started with.
    let first = super::super::home_custom::NewDocument::preset(0);
    assert_eq!(ws.home.new_document, Some(first));
    assert_eq!((first.width, first.height, first.fps), (1920, 1080, 24));
    assert_eq!(first.matching_preset(), Some(0));
    // The Custom Size card starts on a size that is none of them.
    ws.home.new_document = None;
    ws.show_custom_size();
    let custom = super::super::home_custom::NewDocument::custom();
    assert_eq!(ws.home.new_document, Some(custom));
    assert_eq!(custom.matching_preset(), None);
    ws.home.new_document = Some(super::super::home_custom::NewDocument {
        width: 640,
        height: 360,
        fps: 12,
        ..custom
    });
    run(&mut ws, &ctx, 0.0);
    assert!(ws.new_document_shown(), "it stays until Create or Cancel");
}

#[test]
fn create_makes_a_project_of_the_chosen_size_rate_and_background() {
    let mut ws = workspace();
    ws.new_project_with(640, 360, 12, flare_core::Rgba8::new(10, 20, 30, 255));
    let stage = ws.document().stage();
    assert_eq!((stage.width(), stage.height()), (640, 360));
    assert_eq!(stage.background(), flare_core::Rgba8::new(10, 20, 30, 255));
    assert_eq!(ws.document().frame_rate().numerator(), 12);
    // The old call is a white stage.
    ws.new_project(800, 600, 24);
    assert_eq!(ws.document().stage().background(), flare_core::Rgba8::WHITE);
}

#[test]
fn choosing_a_preset_fills_the_numbers_and_editing_them_leaves_the_preset() {
    use super::super::home_custom::NewDocument;
    let social = NewDocument::preset(1);
    assert_eq!((social.width, social.height, social.fps), (1080, 1080, 30));
    assert_eq!(social.matching_preset(), Some(1));
    let edited = NewDocument {
        width: 1081,
        ..social
    };
    assert_eq!(
        edited.matching_preset(),
        None,
        "no longer the preset's numbers"
    );
    // A background color is not part of a preset.
    let colored = NewDocument {
        background: flare_core::Rgba8::new(1, 2, 3, 255),
        ..social
    };
    assert_eq!(colored.matching_preset(), Some(1));
    // A preset number out of range is the first, never a panic.
    assert_eq!(NewDocument::preset(999), NewDocument::preset(0));
}

#[test]
fn the_custom_size_window_goes_when_home_does() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    ws.set_home(true);
    ws.show_custom_size();
    ws.set_home(false);
    run(&mut ws, &ctx, 0.0);
    assert!(!ws.new_document_shown());
}

/// Presses the top bar's house in a window 1280 wide.
fn press_the_house(ws: &mut Workspace, ctx: &egui::Context, time: f64) {
    let at = Pos2::new(18.0, 14.0);
    let click = |pressed| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    pass(ws, ctx, input(time, vec![Event::PointerMoved(at)]));
    pass(ws, ctx, input(time + 0.1, vec![click(true)]));
    pass(ws, ctx, input(time + 0.2, vec![click(false)]));
}

#[test]
fn the_house_in_the_top_bar_goes_to_home_and_stays_there() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
    assert!(!ws.home_shown());
    press_the_house(&mut ws, &ctx, 0.2);
    assert!(ws.home_shown(), "from a document it goes to Home");
    let tabs = ws.tabs().len();
    press_the_house(&mut ws, &ctx, 1.0);
    assert!(ws.home_shown(), "on Home it does nothing");
    assert_eq!(ws.tabs().len(), tabs, "and makes no document");
    // A tab is the way back to a document.
    ws.switch_to_tab(0);
    assert!(!ws.home_shown());
}

// ---- the bar where the menus are the system's (macOS) ------------------------------------------

#[test]
fn with_a_native_menu_the_top_bar_keeps_its_gear_name_undo_redo_play_and_publish() {
    // The settings gear is centered over the rail's icons, 16.5 px in from the right edge of a
    // 1280 wide window; it is a 22 px button, so a click 10 px either side opens Preferences.
    let center = 1280.0 - crate::workspace::rail::CENTER_FROM_EDGE;
    let opened = |home: bool, x: f32| {
        let mut ws = workspace();
        ws.set_native_menu(true);
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

#[test]
fn with_a_native_menu_publish_is_left_of_play_and_the_bar_has_undo_redo_and_the_switcher() {
    // Walking left from the undo button, play comes, then publish.
    let (mut play, mut publish) = (None, None);
    let mut x = 1180.0;
    while x > 1040.0 {
        let (published, playing, _) = press_top_bar(x, true);
        if playing && play.is_none() {
            play = Some(x);
        }
        if published && publish.is_none() {
            publish = Some(x);
        }
        x -= 8.0;
    }
    let (play, publish) = (
        play.expect("play is in the bar"),
        publish.expect("publish is in the bar"),
    );
    assert!(publish < play, "publish at {publish}, play at {play}");
    let mut hints = String::new();
    let mut x = 1276.0;
    while x > 1060.0 {
        hints.push_str(&top_bar_hint(x, false, true));
        x -= 6.0;
    }
    for wanted in ["Undo", "Redo", "Preferences", "Interface switcher"] {
        assert!(hints.contains(wanted), "{wanted} is in the bar: {hints:?}");
    }
}
