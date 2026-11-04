#![allow(clippy::expect_used)]

use super::*;
use flare_animate::Workspace;

fn words_in(tag: &str) -> MovieWords {
    let mut workspace = Workspace::untitled().expect("a new workspace");
    workspace.set_system_locale(tag);
    MovieWords::from_lookup(|key, args| workspace.interface_text_with(key, args))
}

#[test]
fn english_words_come_from_the_messages_too() {
    assert_eq!(words_in("en-US"), MovieWords::english());
}

#[test]
fn every_word_of_the_window_is_in_the_language_of_the_interface() {
    let english = MovieWords::english();
    for tag in [
        "de-DE", "es-ES", "fr-FR", "pt-BR", "ja-JP", "ko-KR", "zh-CN",
    ] {
        let words = words_in(tag);
        for (name, ours, theirs) in [
            ("title", &words.title, &english.title),
            ("restart", &words.restart, &english.restart),
            ("step", &words.step, &english.step),
            ("speed", &words.speed, &english.speed),
            ("loop_on", &words.loop_on, &english.loop_on),
            ("loop_off", &words.loop_off, &english.loop_off),
            ("sound_off", &words.sound_off, &english.sound_off),
            ("sound_on", &words.sound_on, &english.sound_on),
            ("close", &words.close, &english.close),
        ] {
            assert_ne!(ours, theirs, "{tag}: {name} is still English");
        }
    }
}

#[test]
fn the_clock_line_keeps_the_time_and_the_frame_in_any_language() {
    let spanish = words_in("es-ES");
    assert_eq!(spanish.clock_line("0:01.5", 7), "0:01.5 · fotograma 7");
    assert_eq!(
        MovieWords::english().clock_line("0:00.0", 0),
        "0:00.0 · frame 0"
    );
}
