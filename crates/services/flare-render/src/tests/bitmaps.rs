use super::*;
use flare_core::BitmapId;
use flare_document::{Bitmap, BitmapInstance};

fn solid(rgba: [u8; 4], side: u32) -> Bitmap {
    let pixels = rgba.repeat((side * side) as usize);
    Bitmap::new(BitmapId::new(), "b", side, side, pixels).expect("bitmap")
}

fn placed(bitmap: &Bitmap, x: f64, y: f64, opacity: f32) -> Content {
    let at = Transform2D::new(DAffine2::from_translation(DVec2::new(x, y))).expect("finite");
    Content::Bitmap(BitmapInstance::new(InstanceId::new(), bitmap.id(), at).with_opacity(opacity))
}

fn layer(content: Content) -> Layer {
    Layer::new(LayerId::new(), "L", vec![span(content)]).expect("layer")
}

#[test]
fn a_bitmap_is_drawn_where_it_is_placed() {
    let red = solid([255, 0, 0, 255], 10);
    let (doc, id) = scene_with_layers(vec![layer(placed(&red, 5.0, 5.0, 1.0))]);
    let doc = doc.with_bitmaps(vec![red]).expect("bitmaps");
    let image = draw(&doc, id, Rgba8::WHITE);
    assert_eq!(image.pixel(10, 10), Some(RED));
    assert_eq!(image.pixel(2, 2), Some(Rgba8::WHITE));
    assert_eq!(image.pixel(17, 17), Some(Rgba8::WHITE));
}

#[test]
fn opacity_mixes_with_what_is_below_and_clear_pixels_show_it() {
    let half = solid([255, 0, 0, 255], 10);
    let clear = solid([0, 0, 255, 0], 10);
    let (doc, id) = scene_with_layers(vec![
        layer(placed(&half, 0.0, 0.0, 0.5)),
        layer(placed(&clear, 10.0, 10.0, 1.0)),
    ]);
    let doc = doc.with_bitmaps(vec![half, clear]).expect("bitmaps");
    let image = draw(&doc, id, Rgba8::WHITE);
    let mixed = image.pixel(5, 5).expect("pixel");
    assert_eq!(mixed.r, 255);
    assert!(
        (120..=135).contains(&mixed.g),
        "half red over white: {mixed:?}"
    );
    assert_eq!(image.pixel(15, 15), Some(Rgba8::WHITE));
}

#[test]
fn a_bitmap_in_a_higher_layer_covers_a_shape_in_a_lower_one() {
    let green = solid([0, 255, 0, 255], 10);
    let (doc, id) = scene_with_layers(vec![
        layer(placed(&green, 0.0, 0.0, 1.0)),
        layer(rect(0.0, 0.0, 20.0, 20.0, RED)),
    ]);
    let doc = doc.with_bitmaps(vec![green]).expect("bitmaps");
    let image = draw(&doc, id, Rgba8::WHITE);
    assert_eq!(image.pixel(5, 5), Some(Rgba8::opaque(0, 255, 0)));
    assert_eq!(image.pixel(15, 15), Some(RED));
}
