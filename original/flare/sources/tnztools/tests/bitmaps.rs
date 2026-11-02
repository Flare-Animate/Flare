//! Bitmaps: imported, placed and used as fills, they save and reopen, draw
//! where they're placed, break apart, hit by their box and publish to SWF.

use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use minnamate_core::doc::*;
use minnamate_core::hit::{self, ElementRef};
use minnamate_core::tiny_skia::Pixmap;
use minnamate_core::{arrange, bitmap, draw, render::render, swf_export, xfl, xfl_write};

/// An 8 × 8 red JPEG.
const RED_JPEG: &str = "FFD8FFE000104A46494600010101006000600000FFDB0043000402030303020403030304040404050906050505050B080806090D0B0D0D0D0B0C0C0E1014110E0F130F0C0C1218121315161717170E11191B19161A14161716FFDB0043010404040505050A06060A160F0C0F1616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616FFC00011080008000803012200021101031101FFC4001F0000010501010101010100000000000000000102030405060708090A0BFFC400B5100002010303020403050504040000017D01020300041105122131410613516107227114328191A1082342B1C11552D1F02433627282090A161718191A25262728292A3435363738393A434445464748494A535455565758595A636465666768696A737475767778797A838485868788898A92939495969798999AA2A3A4A5A6A7A8A9AAB2B3B4B5B6B7B8B9BAC2C3C4C5C6C7C8C9CAD2D3D4D5D6D7D8D9DAE1E2E3E4E5E6E7E8E9EAF1F2F3F4F5F6F7F8F9FAFFC4001F0100030101010101010101010000000000000102030405060708090A0BFFC400B51100020102040403040705040400010277000102031104052131061241510761711322328108144291A1B1C109233352F0156272D10A162434E125F11718191A262728292A35363738393A434445464748494A535455565758595A636465666768696A737475767778797A82838485868788898A92939495969798999AA2A3A4A5A6A7A8A9AAB2B3B4B5B6B7B8B9BAC2C3C4C5C6C7C8C9CAD2D3D4D5D6D7D8D9DAE2E3E4E5E6E7E8E9EAF2F3F4F5F6F7F8F9FAFFDA000C03010002110311003F00F17A28A2BF293FBF8FFFD9";

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

/// A 2 × 2 PNG: red, green / blue, white.
fn checker_png() -> Vec<u8> {
    let mut pm = Pixmap::new(2, 2).unwrap();
    let colors = [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255], [255, 255, 255, 255]];
    for (p, c) in pm.data_mut().chunks_exact_mut(4).zip(colors) {
        p.copy_from_slice(&c);
    }
    pm.encode_png().unwrap()
}

/// An empty folder for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn placed(name: &str, matrix: Matrix) -> Element {
    Element::Bitmap(BitmapInstance { name: name.into(), matrix, extra: Extra::default() })
}

fn scale(k: f32, tx: f32, ty: f32) -> Matrix {
    Matrix { a: k, d: k, tx, ty, ..Matrix::IDENTITY }
}

/// On a grey stage: the checker imported (unsmoothed) and placed 10× at (10, 10), a 50 × 20
/// rectangle at (40, 10) tiled with it 5×, and the red JPEG at (10, 50).
fn document(dir: &Path) -> Document {
    fs::write(dir.join("checker.png"), checker_png()).unwrap();
    fs::write(dir.join("photo.jpg"), unhex(RED_JPEG)).unwrap();
    let mut doc = Document::new(100.0, 80.0, 24.0);
    doc.background = [128.0 / 255.0, 128.0 / 255.0, 128.0 / 255.0, 1.0];
    assert_eq!(bitmap::import(&mut doc, &dir.join("checker.png")).unwrap(), "checker.png");
    assert_eq!(bitmap::import(&mut doc, &dir.join("photo.jpg")).unwrap(), "photo.jpg");
    assert_eq!(bitmap::import(&mut doc, &dir.join("checker.png")).unwrap(), "checker copy.png", "a name in use gets a copy's");
    assert!(bitmap::import(&mut doc, &dir.join("nowhere.png")).is_err());
    // One in a folder of the library.
    doc.bitmap_mut("checker copy.png").unwrap().name = "Copies/checker.png".into();
    doc.bitmap_mut("checker.png").unwrap().smoothing = false;
    let fill = BitmapFill { name: "checker.png".into(), matrix: scale(5.0, 40.0, 10.0), clipped: false, extra: Extra::default() };
    let tiled = draw::rectangle([(40.0, 10.0), (90.0, 30.0)], 0.0, None, Some(Paint::Bitmap(fill)));
    let elements = vec![placed("checker.png", scale(10.0, 10.0, 10.0)), Element::Shape(tiled), placed("photo.jpg", scale(1.0, 10.0, 50.0))];
    doc.scenes[0].layers[0].frames[0].elements = elements;
    doc
}

fn pixel(pm: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let p = pm.pixel(x, y).unwrap();
    [p.red(), p.green(), p.blue(), p.alpha()]
}

fn near(a: [u8; 4], b: [u8; 4], within: u8) -> bool {
    a.iter().zip(b).all(|(&x, y)| x.abs_diff(y) <= within)
}

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const WHITE: [u8; 4] = [255; 4];

#[test]
fn bitmaps_draw_where_they_are_placed() {
    let dir = scratch("bitmaps_draw");
    let mut doc = document(&dir);
    let (pm, skipped) = render(&doc, 0, 1.0).unwrap();
    assert!(skipped.is_empty(), "{skipped:?}");
    // The checker, ten times over, unsmoothed: crisp blocks.
    assert_eq!([pixel(&pm, 11, 11), pixel(&pm, 28, 11), pixel(&pm, 11, 28), pixel(&pm, 28, 28)], [RED, GREEN, BLUE, WHITE]);
    assert_eq!(pixel(&pm, 5, 5), [128, 128, 128, 255], "the stage around it");
    // Tiled every 10 px from the rectangle's corner.
    assert_eq!([pixel(&pm, 42, 12), pixel(&pm, 47, 12), pixel(&pm, 52, 12), pixel(&pm, 57, 17)], [RED, GREEN, RED, WHITE]);
    assert_eq!(pixel(&pm, 95, 20), [128, 128, 128, 255], "only inside the shape");
    assert!(near(pixel(&pm, 14, 54), RED, 8), "the JPEG: {:?}", pixel(&pm, 14, 54));
    // Clipped, its edge pixels go on past it.
    let Element::Shape(s) = &mut doc.scenes[0].layers[0].frames[0].elements[1] else { panic!() };
    let Paint::Bitmap(f) = &mut s.fills[0] else { panic!() };
    f.clipped = true;
    let (pm, _) = render(&doc, 0, 1.0).unwrap();
    assert_eq!([pixel(&pm, 52, 12), pixel(&pm, 85, 25)], [GREEN, WHITE]);

    // Through a symbol's colour effect: half see-through.
    let mut symbol = Timeline::new("Faded");
    symbol.layers[0].frames[0].elements = vec![placed("checker.png", scale(10.0, 0.0, 0.0))];
    doc.library.insert("Faded".into(), Symbol { kind: SymbolKind::Graphic, timeline: symbol, extra: Extra::default() });
    let instance = Instance {
        symbol: "Faded".into(),
        kind: SymbolKind::Graphic,
        matrix: scale(1.0, 60.0, 50.0),
        pivot: (0.0, 0.0),
        color: ColorTransform { mult: [1.0, 1.0, 1.0, 0.5], add: [0.0; 4] },
        looping: Loop::Loop,
        first_frame: 0,
        extra: Extra::default(),
    };
    doc.scenes[0].layers[0].frames[0].elements.push(Element::Instance(instance));
    let (pm, _) = render(&doc, 0, 1.0).unwrap();
    assert!(near(pixel(&pm, 62, 52), [191, 64, 64, 255], 2), "{:?}", pixel(&pm, 62, 52));

    // A bitmap the library doesn't have shows nothing, and says so.
    doc.scenes[0].layers[0].frames[0].elements.push(placed("gone.png", Matrix::IDENTITY));
    assert!(render(&doc, 0, 1.0).unwrap().1.contains("missing bitmap \"gone.png\""));
}

#[test]
fn bitmaps_save_and_reopen() {
    let dir = scratch("bitmaps_save");
    let doc = document(&dir);
    for path in [dir.join("saved"), dir.join("saved.fla")] {
        xfl_write::save(&doc, &path).unwrap();
        let back = xfl::load(&path).unwrap();
        assert_eq!(back, doc, "{}", path.display());
        let (a, b) = (render(&doc, 0, 1.0).unwrap().0, render(&back, 0, 1.0).unwrap().0);
        assert!(a.data() == b.data(), "{}", path.display());
    }
    // The image files as imported, under LIBRARY/, and Animate's copies in bin/.
    let saved = dir.join("saved");
    assert_eq!(fs::read(saved.join("LIBRARY/checker.png")).unwrap(), checker_png());
    assert_eq!(fs::read(saved.join("LIBRARY/photo.jpg")).unwrap(), unhex(RED_JPEG));
    assert!(saved.join("LIBRARY/Copies/checker.png").is_file());
    assert_eq!(fs::read(saved.join("bin/M 2 0.dat")).unwrap(), unhex(RED_JPEG), "a JPEG's copy is the JPEG");
    let dat = BitmapData::new(fs::read(saved.join("bin/M 1 0.dat")).unwrap());
    assert_eq!(dat.pixels().map(|p| p.data().to_vec()), doc.bitmap("checker.png").unwrap().data.pixels().map(|p| p.data().to_vec()));
    let xml = fs::read_to_string(saved.join("DOMDocument.xml")).unwrap();
    for attr in [r#"name="checker.png""#, r#"compressionType="lossless""#, r#"frameRight="40""#, r#"href="checker.png""#, r#"bitmapDataHRef="M 1 0.dat""#] {
        assert!(xml.contains(attr), "{attr} missing from\n{xml}");
    }
    assert!(xml.contains(r#"<DOMBitmapItem name="photo.jpg" allowSmoothing="true" compressionType="photo""#), "{xml}");

    // An Animate document holding its bitmaps in bin/ only reads them from there.
    let only_bin = dir.join("only_bin");
    xfl_write::save(&doc, &only_bin).unwrap();
    fs::remove_dir_all(only_bin.join("LIBRARY")).unwrap();
    let back = xfl::load(&only_bin).unwrap();
    assert_eq!(back.bitmap("checker.png").unwrap().data.pixels().unwrap().data(), doc.bitmap("checker.png").unwrap().data.pixels().unwrap().data());
    assert_eq!(back.bitmap("photo.jpg").unwrap().data, doc.bitmap("photo.jpg").unwrap().data);
}

#[test]
fn each_bitmap_keeps_files_of_its_own() {
    let dir = scratch("bitmaps_own_files");
    let solid = |c: [u8; 4]| {
        let mut pm = Pixmap::new(2, 2).unwrap();
        pm.data_mut().chunks_exact_mut(4).for_each(|p| p.copy_from_slice(&c));
        pm.encode_png().unwrap()
    };
    let mut doc = Document::new(10.0, 10.0, 24.0);
    // Names apart only by case, which Windows' and macOS' file names aren't.
    doc.media.push(Media::Bitmap(BitmapItem::new("Sky.png", solid(RED))));
    doc.media.push(Media::Bitmap(BitmapItem::new("sky.png", solid(BLUE))));
    // One minnamate can't read keeps its files' names, which the next one's would be.
    let mut kept = BitmapItem::new("kept.png", b"not an image".to_vec());
    kept.extra.set("href", Some("new.png"));
    kept.extra.set("bitmapDataHRef", Some("M 4 0.dat"));
    doc.media.push(Media::Bitmap(kept));
    doc.media.push(Media::Bitmap(BitmapItem::new("new.png", solid(GREEN))));
    for path in [dir.join("saved"), dir.join("saved.fla")] {
        xfl_write::save(&doc, &path).unwrap();
        let back = xfl::load(&path).unwrap();
        let color = |name: &str| back.bitmap(name).and_then(|b| b.data.pixels()).map(|p| pixel(p, 0, 0));
        assert_eq!([color("Sky.png"), color("sky.png"), color("new.png"), color("kept.png")], [Some(RED), Some(BLUE), Some(GREEN), None], "{}", path.display());
        let kept = back.bitmap("kept.png").unwrap();
        assert_eq!((kept.extra.get("href"), kept.extra.get("bitmapDataHRef")), (Some("new.png"), Some("M 4 0.dat")));
    }
}

#[test]
fn break_apart_and_hit_by_the_box() {
    let dir = scratch("bitmaps_break");
    let mut doc = document(&dir);
    let tl = &doc.scenes[0];
    let checker = ElementRef { layer: 0, keyframe: 0, element: 0 };
    assert_eq!(hit::bounds(&doc, tl, 0, &[checker]), Some(hit::Rect { min: (10.0, 10.0), max: (30.0, 30.0) }));
    assert_eq!(hit::hit_test(&doc, tl, 0, (20.0, 20.0), 0.0), Some(checker));
    assert_eq!(hit::hit_test(&doc, tl, 0, (31.0, 20.0), 0.0), None);

    // Turned and stretched, then broken apart: a shape filled with the bitmap, drawn the same.
    let turned = Matrix { a: 6.0, b: 4.0, c: -3.0, d: 8.0, tx: 30.0, ty: 5.0 };
    doc.scenes[0].layers[0].frames[0].elements = vec![placed("checker.png", turned)];
    let before = render(&doc, 0, 1.0).unwrap().0;
    let el = doc.scenes[0].layers[0].frames[0].elements[0].clone();
    let parts = arrange::broken_apart(&doc, &el, 0).unwrap();
    let [Element::Shape(s)] = &parts[..] else { panic!("{parts:?}") };
    assert!(matches!(&s.fills[..], [Paint::Bitmap(f)] if f.name == "checker.png" && !f.clipped));
    doc.scenes[0].layers[0].frames[0].elements = parts.clone();
    let after = render(&doc, 0, 1.0).unwrap().0;
    // The same inside; along the outline the tiled fill's edge pixels blend with the far side's.
    for (x, y) in [(0.5, 0.5), (1.5, 0.5), (0.5, 1.5), (1.5, 1.5)].map(|p| turned.apply(p)) {
        assert_eq!(pixel(&before, x as u32, y as u32), pixel(&after, x as u32, y as u32), "at ({x}, {y})");
    }
    let off = before.pixels().iter().zip(after.pixels()).filter(|(a, b)| a != b).count();
    assert!(off < 70, "{off} pixels differ, more than the outline's");
    assert_eq!(hit::bounds(&doc, &doc.scenes[0], 0, &[checker]).map(|b| (b.min.0.round(), b.max.1.round())), Some((24.0, 29.0)));
}

#[test]
fn bitmaps_publish_to_swf() {
    let dir = scratch("bitmaps_swf");
    let doc = document(&dir);
    let (bytes, skipped) = swf_export::export(&doc).unwrap();
    assert!(skipped.is_empty(), "{skipped:?}");
    let buf = swf::decompress_swf(&bytes[..]).unwrap();
    let movie = swf::parse_swf(&buf).unwrap();
    let mut lossless = None;
    let mut jpeg = None;
    let mut fills = Vec::new();
    for tag in &movie.tags {
        match tag {
            swf::Tag::DefineBitsLossless(b) => lossless = Some(b.clone()),
            swf::Tag::DefineBitsJpeg2 { id, jpeg_data } => jpeg = Some((*id, jpeg_data.to_vec())),
            swf::Tag::DefineShape(s) => fills.extend(s.styles.fill_styles.clone()),
            _ => {}
        }
    }
    // The checker, lossless: premultiplied ARGB, zlib'd.
    let b = lossless.expect("a DefineBitsLossless2");
    assert_eq!((b.version, b.width, b.height, b.format), (2, 2, 2, swf::BitmapFormat::Rgb32));
    let mut argb = Vec::new();
    flate2_read(&b.data, &mut argb);
    assert_eq!(&argb[..8], &[255, 255, 0, 0, 255, 0, 255, 0]);
    // The photo goes as its JPEG.
    let (jpeg_id, data) = jpeg.expect("a DefineBitsJPEG2");
    assert_eq!(data, unhex(RED_JPEG));
    // Placed bitmaps are clipped fills of their own; the tiled fill repeats at 5 px a pixel (twips: 100).
    let bitmap_fills: Vec<_> = fills.iter().filter_map(|f| match f {
        swf::FillStyle::Bitmap { id, matrix, is_smoothed, is_repeating } => Some((*id, matrix.a.to_f32(), *is_smoothed, *is_repeating)),
        _ => None,
    }).collect();
    assert!(bitmap_fills.contains(&(b.id, 20.0, false, false)), "{bitmap_fills:?}");
    assert!(bitmap_fills.contains(&(b.id, 100.0, false, true)), "{bitmap_fills:?}");
    assert!(bitmap_fills.contains(&(jpeg_id, 20.0, true, false)), "{bitmap_fills:?}");
}

/// zlib-inflates `data` into `out`, through the zip crate's flate2 as the swf crate reads.
fn flate2_read(data: &[u8], out: &mut Vec<u8>) {
    let mut z = flate2::read::ZlibDecoder::new(data);
    z.read_to_end(out).unwrap();
}

#[test]
fn display_list_names_the_bitmap() {
    let dir = scratch("bitmaps_display");
    let doc = document(&dir);
    let list = minnamate_core::display::display_list(&doc, &doc.scenes[0], 0, &mut BTreeSet::new());
    assert!(matches!(list[0].item, minnamate_core::display::Item::Bitmap(b) if b.name == "checker.png"));
    assert_eq!(list[0].matrix, scale(10.0, 10.0, 10.0));
}
