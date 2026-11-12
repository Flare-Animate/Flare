use super::window_icon::{app_icon, rgba};

#[test]
fn the_app_icon_decodes_to_square_rgba_with_its_flame_and_clear_corners() {
    let (pixels, width, height) = rgba(flare_ui::icons::APP_ICON_PNG).expect("the app icon");
    assert_eq!((width, height), (256, 256));
    assert_eq!(pixels.len(), 256 * 256 * 4);
    let at = |x: usize, y: usize| &pixels[(y * 256 + x) * 4..(y * 256 + x) * 4 + 4];
    assert!(at(0, 0)[3] < 16, "the rounded tile leaves the corner clear");
    let middle = at(128, 150);
    assert!(
        middle[3] > 240 && middle[0] > 200,
        "the flame is warm and solid: {middle:?}"
    );
    assert!(app_icon().is_some());
}

#[test]
fn a_picture_that_is_no_png_or_too_large_for_an_icon_is_refused() {
    assert!(rgba(b"not a png").is_none());
    assert!(rgba(&[]).is_none());
    let mut big = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut big, 2048, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("header");
        writer.write_image_data(&vec![0; 2048 * 4]).expect("pixels");
    }
    assert!(
        rgba(&big).is_none(),
        "2048 pixels wide is more than an icon"
    );
}

#[test]
fn a_grey_png_is_widened_to_rgba() {
    let mut grey = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut grey, 2, 1);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("header");
        writer.write_image_data(&[10, 200]).expect("pixels");
    }
    let (pixels, width, height) = rgba(&grey).expect("grey");
    assert_eq!((width, height), (2, 1));
    assert_eq!(pixels, vec![10, 10, 10, 255, 200, 200, 200, 255]);
}
