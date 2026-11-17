use super::*;

fn texts() -> ColorTexts {
    let channel = |short: &str, name: &str| ChannelText {
        short: short.to_owned(),
        name: name.to_owned(),
    };
    ColorTexts {
        hue: channel("H", "Hue"),
        saturation: channel("S", "Saturation"),
        brightness: channel("B", "Brightness"),
        alpha: channel("A", "Alpha"),
        red: channel("R", "Red"),
        green: channel("G", "Green"),
        blue: channel("B", "Blue"),
    }
}

/// One pass of a 280-point-wide panel holding the picker for `color`: every piece of text it
/// drew, with where.
fn picker_pass(
    ctx: &egui::Context,
    time: f64,
    events: Vec<egui::Event>,
    color: &mut Color32,
) -> Vec<(String, Rect)> {
    let input = egui::RawInput {
        time: Some(time),
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(280.0, 400.0))),
        events,
        ..egui::RawInput::default()
    };
    let mut output = ctx.run_ui(input, |ui| {
        color_picker(ui, color, &texts());
    });
    output.textures_delta.clear();
    output
        .shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            Shape::Text(text) => Some((
                text.galley.text().to_owned(),
                text.galley.rect.translate(text.pos.to_vec2()),
            )),
            _ => None,
        })
        .collect()
}

fn press(at: Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn every_track_shows_its_number() {
    let ctx = egui::Context::default();
    let mut color = Color32::from_rgba_unmultiplied(255, 0, 0, 128);
    picker_pass(&ctx, 0.0, vec![], &mut color);
    let shown: Vec<String> = picker_pass(&ctx, 0.1, vec![], &mut color)
        .into_iter()
        .map(|(text, _)| text)
        .collect();
    // Each track's number is drawn as its digits and then its unit.
    let after = |unit: &str| -> Vec<&str> {
        shown
            .windows(2)
            .filter(|w| w[1] == unit)
            .map(|w| w[0].as_str())
            .collect()
    };
    assert_eq!(after("\u{b0}"), ["0"], "the hue: {shown:?}");
    assert_eq!(
        after("%"),
        ["100", "100", "50"],
        "saturation, brightness and alpha: {shown:?}"
    );
    assert!(shown.iter().any(|t| t == "255"), "the red: {shown:?}");
    assert_eq!(
        color,
        Color32::from_rgba_unmultiplied(255, 0, 0, 128),
        "only shown"
    );
}

#[test]
fn a_press_at_the_end_of_the_hue_track_turns_the_hue_and_its_number_follows() {
    let ctx = egui::Context::default();
    let mut color = Color32::from_rgb(255, 0, 0);
    picker_pass(&ctx, 0.0, vec![], &mut color);
    let drawn = picker_pass(&ctx, 0.1, vec![], &mut color);
    let (_, number) = drawn
        .iter()
        .find(|(text, _)| text == "\u{b0}")
        .expect("the hue's unit");
    // Half way along the track: the track runs from the letter to the number.
    let (_, letter) = drawn
        .iter()
        .find(|(text, rect)| text == "H" && (rect.center().y - number.center().y).abs() < 8.0)
        .expect("the hue's letter");
    let at = Pos2::new(
        (letter.right() + 4.0 + number.left() - 12.0) / 2.0,
        number.center().y,
    );
    picker_pass(&ctx, 0.2, vec![egui::Event::PointerMoved(at)], &mut color);
    picker_pass(&ctx, 0.3, vec![press(at, true)], &mut color);
    picker_pass(&ctx, 0.4, vec![press(at, false)], &mut color);
    let hue = Hsb::from_color(color).hue * 360.0;
    assert!((hue - 180.0).abs() < 25.0, "about cyan: {hue} {color:?}");
    let shown = picker_pass(&ctx, 0.5, vec![], &mut color);
    let degrees = format!("{}", hue.round());
    let on_number = shown
        .windows(2)
        .any(|w| w[0].0 == degrees && w[1].0 == "\u{b0}");
    assert!(on_number, "{degrees} on the number: {shown:?}");
}

#[test]
fn hsb_reads_as_other_programs_count_it() {
    let red = Hsb::from_color(Color32::from_rgb(255, 0, 0));
    assert_eq!((red.hue, red.saturation, red.brightness), (0.0, 1.0, 1.0));
    let grey = Hsb::from_color(Color32::from_rgb(128, 128, 128));
    assert_eq!(grey.saturation, 0.0);
    assert!((grey.brightness - 128.0 / 255.0).abs() < 1e-6, "50% bright");
    let blue = Hsb::from_color(Color32::from_rgb(0, 0, 255));
    assert!((blue.hue * 360.0 - 240.0).abs() < 1e-3);
    for color in [
        Color32::from_rgb(12, 200, 99),
        Color32::from_rgba_unmultiplied(250, 10, 128, 64),
        Color32::from_rgb(0, 0, 0),
        Color32::from_rgb(255, 255, 255),
        Color32::from_rgb(40, 40, 41),
    ] {
        assert_eq!(Hsb::from_color(color).to_color(), color, "round trip");
    }
}

#[test]
fn parts_out_of_range_are_brought_into_it() {
    let wild = Hsb {
        hue: f32::NAN,
        saturation: 7.0,
        brightness: -1.0,
        alpha: f32::INFINITY,
    };
    assert_eq!(wild.to_color(), Color32::from_rgba_unmultiplied(0, 0, 0, 0));
    let turned = Hsb {
        hue: 1.5,
        saturation: 1.0,
        brightness: 1.0,
        alpha: 1.0,
    };
    assert_eq!(
        turned.to_color(),
        Color32::from_rgb(0, 255, 255),
        "half a turn on"
    );
}

#[test]
fn a_hue_survives_a_trip_through_grey() {
    let ctx = egui::Context::default();
    let mut color = Color32::from_rgb(0, 0, 255);
    picker_pass(&ctx, 0.0, vec![], &mut color);
    // The picker remembers the blue hue it made the grey from.
    let id = egui::Id::unique("remember");
    ctx.data_mut(|d| {
        d.insert_temp(
            id,
            (
                Color32::from_rgb(9, 9, 9),
                Hsb {
                    hue: 2.0 / 3.0,
                    saturation: 0.0,
                    brightness: 9.0 / 255.0,
                    alpha: 1.0,
                },
            ),
        );
    });
    let mut hsb = None;
    let input = egui::RawInput::default();
    let mut output = ctx.run_ui(input, |ui| {
        hsb = Some(remembered(ui, id, Color32::from_rgb(9, 9, 9)));
        let other = remembered(ui, id, Color32::from_rgb(9, 9, 10));
        assert_eq!(other.hue, 2.0 / 3.0, "another color has a hue of its own");
    });
    output.textures_delta.clear();
    assert_eq!(hsb.map(|h| h.hue), Some(2.0 / 3.0));
}
