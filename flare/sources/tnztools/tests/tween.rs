//! The `fixtures/tween` document: a graphic symbol on a classic tween (x 100 -> 400,
//! alpha 1 -> 0.5) over a stroked rectangle, plus a two-frame movie clip.

use std::path::Path;

use minnamate_core::{doc::Document, render::render, swf_export, tiny_skia::Pixmap, xfl};
use swf::Tag;

const WHITE: [u8; 3] = [255, 255, 255];
const BLUE: [u8; 3] = [0xCC, 0xE5, 0xFF];
const RED: [u8; 3] = [255, 0, 0];

fn doc() -> Document {
    xfl::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tween")).unwrap()
}

fn frame(n: u32) -> Pixmap {
    let (pixmap, skipped) = render(&doc(), n, 1.0).unwrap();
    assert!(skipped.is_empty(), "{skipped:?}");
    pixmap
}

fn rgb(pixmap: &Pixmap, x: u32, y: u32) -> [u8; 3] {
    let c = pixmap.pixel(x, y).unwrap().demultiply();
    [c.red(), c.green(), c.blue()]
}

fn assert_near(got: [u8; 3], want: [f32; 3]) {
    assert!(got.iter().zip(want).all(|(&g, w)| (g as f32 - w).abs() <= 2.0), "{got:?} vs {want:?}");
}

#[test]
fn first_keyframe() {
    let f = frame(0);
    assert_eq!(rgb(&f, 10, 10), WHITE, "stage");
    assert_eq!(rgb(&f, 50, 200), [0, 0, 0], "background stroke");
    assert_eq!(rgb(&f, 400, 200), BLUE, "background fill");
    assert_eq!(rgb(&f, 100, 200), RED, "ball");
    assert_eq!(rgb(&f, 77, 200), RED, "inside the curved side");
    assert_eq!(rgb(&f, 72, 200), BLUE, "outside the curved side");
    assert_eq!(rgb(&f, 520, 30), [0, 255, 0], "movie clip on its first frame");
}

#[test]
fn classic_tween() {
    // Halfway: x = 250, alpha 0.75.
    let f = frame(5);
    assert_eq!(rgb(&f, 100, 200), BLUE);
    assert_near(rgb(&f, 250, 200), [255.0 * 0.75 + 204.0 * 0.25, 229.0 * 0.25, 255.0 * 0.25]);
    // Last keyframe: x = 400 at alpha 0.5.
    assert_near(rgb(&frame(10), 400, 200), [(255.0 + 204.0) / 2.0, 229.0 / 2.0, 255.0 / 2.0]);
}

#[test]
fn swf_export() {
    let (bytes, skipped) = swf_export::export(&doc()).unwrap();
    assert!(skipped.is_empty(), "{skipped:?}");
    let buf = swf::decompress_swf(&bytes[..]).unwrap();
    let movie = swf::parse_swf(&buf).unwrap();
    assert_eq!(movie.header.num_frames(), 11);

    // Depths bottom up: background 1, ball 2, clip 3. The ball moves every frame.
    let (mut frame, mut ball) = (0, Vec::new());
    for tag in &movie.tags {
        match tag {
            Tag::PlaceObject(p) if p.depth == 2 => {
                ball.push((frame, p.matrix.unwrap().tx.get(), p.color_transform.unwrap().a_multiply.to_f32()));
            }
            Tag::ShowFrame => frame += 1,
            _ => {}
        }
    }
    assert_eq!(ball.len(), 11);
    assert_eq!(ball[0], (0, 100 * 20, 1.0));
    assert_eq!(ball[5], (5, 250 * 20, 0.75));
    assert_eq!(ball[10], (10, 400 * 20, 0.5));

    let sprite = movie.tags.iter().find_map(|t| if let Tag::DefineSprite(s) = t { Some(s) } else { None }).unwrap();
    assert_eq!(sprite.num_frames, 2);
    assert!(
        matches!(sprite.tags[..], [Tag::PlaceObject(_), Tag::ShowFrame, Tag::RemoveObject(_), Tag::ShowFrame]),
        "{:?}",
        sprite.tags
    );
}
