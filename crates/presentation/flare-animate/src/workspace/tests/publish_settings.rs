use super::*;
use crate::MenuEntry;

#[test]
fn the_file_menu_names_publishing_as_animate_does() {
    let ws = workspace();
    let labels: Vec<String> = crate::all_entries()
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
fn publish_asks_the_shell_for_a_place_and_settings_start_with_both_formats() {
    let mut ws = workspace();
    let settings = ws.publish_settings();
    assert!(settings.swf && settings.html5 && settings.compress && settings.any());
    assert!(settings.loop_timeline && settings.center_stage && !settings.responsive);
    assert!(ws.is_enabled(MenuAction::Publish));
    ws.perform(MenuAction::Publish);
    assert_eq!(ws.take_dialog(), Some(DialogRequest::Publish));
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
