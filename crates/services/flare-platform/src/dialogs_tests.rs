use crate::dialogs::startup_text;

#[test]
fn the_startup_box_says_what_failed_and_what_to_try() {
    let text = startup_text("initialize the GPU surface: no adapter");
    assert!(text.contains("initialize the GPU surface: no adapter"));
    assert!(text.contains("graphics drivers"));
}

#[test]
fn a_very_long_system_message_is_cut_so_the_box_stays_readable() {
    let text = startup_text(&"x".repeat(100_000));
    assert!(text.len() < 2_000, "{} bytes", text.len());
    assert!(
        text.contains("graphics drivers"),
        "the advice survives the cut"
    );
}

#[test]
fn the_startup_box_takes_its_words_from_the_caller() {
    use crate::dialogs::startup_text_in;
    let text = startup_text_in(
        "Flare no se pudo iniciar.",
        "Actualiza los controladores.",
        "sin GPU",
    );
    assert_eq!(
        text,
        "Flare no se pudo iniciar.\n\nsin GPU\n\nActualiza los controladores."
    );
    assert!(startup_text_in("a", "b", &"x".repeat(100_000)).len() < 2_000);
}
