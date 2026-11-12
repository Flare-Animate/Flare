//! The app icon on the window, for the task bar and the window switcher on Linux and Windows, and
//! in the Dock on macOS when the program is not in an app bundle (see `flare_platform::app_icon`).

use std::io::Cursor;

/// The largest icon decoded, a side in pixels: the app's own is 256.
const MAX_SIDE: u32 = 1024;

/// A PNG as 8-bit RGBA rows and its size, or `None` for one that cannot be read, has no
/// pixels, or is larger than an icon is.
pub(super) fn rgba(png: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let mut decoder = png::Decoder::new(Cursor::new(png));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let (width, height) = (reader.info().width, reader.info().height);
    if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
        return None;
    }
    let mut buffer = vec![0; reader.output_buffer_size()?];
    let frame = reader.next_frame(&mut buffer).ok()?;
    buffer.truncate(frame.buffer_size());
    let pixels = (width * height) as usize;
    let rgba = match frame.color_type {
        png::ColorType::Rgba => buffer,
        png::ColorType::Rgb => buffer
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|&[r, g, b]| [r, g, b, 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => buffer
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|&[g, a]| [g, g, g, a])
            .collect(),
        png::ColorType::Grayscale => buffer.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return None,
    };
    (rgba.len() == pixels * 4).then_some((rgba, width, height))
}

/// The app icon as winit takes it.
pub(super) fn app_icon() -> Option<winit::window::Icon> {
    let (pixels, width, height) = rgba(flare_ui::icons::APP_ICON_PNG)?;
    winit::window::Icon::from_rgba(pixels, width, height).ok()
}
