use super::*;
use flare_menubar::MenuEntry;

#[test]
fn the_file_menu_names_publishing_as_animate_does() {
    let ws = workspace();
    let labels: Vec<String> = flare_menubar::all_entries()
        .iter()
        .filter_map(|e| match e {
            MenuEntry::Action(a) => Some(ws.label(*a)),
            _ => None,
        })
        .collect();
    for wanted in [
        "Publish Settings...",
        "Publish",
        "Export Image...",
        "Export Animated GIF...",
    ] {
        assert!(
            labels.iter().any(|l| l == wanted),
            "{wanted} is in {labels:?}"
        );
    }
    assert!(
        !labels
            .iter()
            .any(|l| l.contains("SWF") || l.contains("HTML")),
        "no format has a command of its own: {labels:?}"
    );
}

#[test]
fn publish_opens_its_popup_and_settings_start_with_both_formats_and_no_gif() {
    let mut ws = workspace();
    let settings = ws.publish_settings();
    assert!(settings.swf && settings.html5 && settings.compress && settings.any());
    assert!(settings.loop_timeline && settings.center_stage && !settings.responsive);
    assert!(!settings.gif);
    assert!(ws.is_enabled(MenuAction::Publish));
    ws.perform(MenuAction::Publish);
    assert!(ws.windows.publish, "the popup opens");
    assert_eq!(ws.take_dialog(), None, "nothing is asked of the shell yet");
}

#[test]
fn the_choices_are_kept_with_the_other_preferences_and_old_files_still_load() {
    let mut ws = workspace();
    ws.publish = PublishSettings {
        swf: false,
        compress: false,
        html5: true,
        loop_timeline: false,
        center_stage: false,
        responsive: true,
        gif: true,
        mp4: true,
    };
    let saved = ws.preferences_json().expect("json");
    let mut fresh = workspace();
    fresh.restore_preferences(&saved).expect("restored");
    assert_eq!(fresh.publish_settings(), ws.publish_settings());
    // A file from before the HTML5 page has only the compress choice: the rest are defaults.
    let mut old = workspace();
    old.restore_preferences(r#"{"publish_compress": false}"#)
        .expect("old file");
    let kept = old.publish_settings();
    assert!(!kept.compress && kept.swf && kept.html5);
}

#[test]
fn the_settings_window_and_the_document_tab_draw_and_say_when_nothing_is_chosen() {
    let mut ws = workspace();
    ws.publish.swf = false;
    ws.publish.html5 = false;
    assert!(!ws.publish_settings().any());
    ws.perform(MenuAction::PublishSettings);
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
    assert!(ws.windows.publish_settings, "the window stays up");
}

#[test]
fn export_flash_document_is_in_the_export_menu_and_asks_the_shell_for_a_place() {
    let mut ws = workspace();
    assert!(ws.is_enabled(MenuAction::ExportFla));
    assert_eq!(
        ws.label(MenuAction::ExportFla),
        "Export Flash Document (.fla)..."
    );
    let export = MENU_LAYOUT
        .iter()
        .find(|(title, _)| *title == "File")
        .and_then(|(_, entries)| {
            entries.iter().find_map(|e| match e {
                MenuEntry::Submenu("Export", inner) => Some(*inner),
                _ => None,
            })
        })
        .expect("File > Export");
    assert!(
        export
            .iter()
            .any(|e| matches!(e, MenuEntry::Action(MenuAction::ExportFla))),
        "it is in File > Export"
    );
    ws.perform(MenuAction::ExportFla);
    assert_eq!(ws.take_dialog(), Some(DialogRequest::ExportFla));
}

#[test]
fn export_svg_is_in_the_export_menu_and_asks_the_shell_for_a_place() {
    let mut ws = workspace();
    assert!(ws.is_enabled(MenuAction::ExportSvg));
    assert_eq!(ws.label(MenuAction::ExportSvg), "Export SVG...");
    let export = MENU_LAYOUT
        .iter()
        .find(|(title, _)| *title == "File")
        .and_then(|(_, entries)| {
            entries.iter().find_map(|e| match e {
                MenuEntry::Submenu("Export", inner) => Some(*inner),
                _ => None,
            })
        })
        .expect("File > Export");
    assert!(
        export
            .iter()
            .any(|e| matches!(e, MenuEntry::Action(MenuAction::ExportSvg))),
        "it is in File > Export"
    );
    ws.perform(MenuAction::ExportSvg);
    assert_eq!(ws.take_dialog(), Some(DialogRequest::ExportSvg));
}

#[test]
fn the_palette_opens_and_performs_the_chosen_command() {
    let mut ws = workspace();
    assert!(!ws.windows.palette, "the palette starts shut");
    ws.toggle_palette();
    assert!(ws.windows.palette);
    // Ctrl+K through the shortcuts the way the window reads them.
    let ctx = egui::Context::default();
    ctx.input_mut(|i| {
        i.events.push(egui::Event::Key {
            key: egui::Key::K,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        })
    });
    ws.handle_shortcuts(&ctx);
    assert!(!ws.windows.palette, "Ctrl+K closed it again");
}
