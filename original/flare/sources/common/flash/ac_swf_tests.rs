//! Tests for the SWF writer / reader (character ids, bitmap pass-through) and
//! SWF fonts ↔ TrueType.

use crate::swf::{self, RawTag};
use crate::swf_font::*;
use crate::{swf_read, swf_write};
use ac_core::bitmap::{BitmapAsset, BitmapCodec};
use ac_core::geom::{Matrix, Path};
use ac_core::model::*;

fn jpeg(w: u32, h: u32) -> Vec<u8> {
    let mut img = image::RgbImage::new(w, h);
    for (x, y, p) in img.enumerate_pixels_mut() {
        *p = image::Rgb([(x * 20) as u8, (y * 30) as u8, 90]);
    }
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90).encode_image(&img).unwrap();
    out
}

fn tag_of(sw: &swf::Swf, code: u16) -> Vec<&RawTag> {
    sw.tags.iter().filter(|t| t.code == code).collect()
}

fn char_id(t: &RawTag) -> u16 {
    u16::from_le_bytes([t.body[0], t.body[1]])
}

fn rect_shape(fill: Color) -> Shape {
    Shape::new(Path::rect(10.0, 20.0, 30.0, 40.0), Some(fill), None)
}

/// A document with bitmaps and symbols that remember SWF ids.
fn doc_with_ids() -> Document {
    let mut d = Document { width: 200.0, height: 100.0, fps: 12.0, ..Default::default() };
    let mut a = BitmapAsset::from_rgba(1, "a", 2, 2, &[255; 16]).unwrap();
    a.swf_char_id = Some(7);
    let mut b = BitmapAsset::from_rgba(2, "b", 2, 2, &[10; 16]).unwrap();
    b.swf_char_id = Some(300);
    d.bitmaps.push(a);
    d.bitmaps.push(b);
    // a symbol with a lone shape, and a clip with two layers' worth of content
    let mut tl = Timeline::new("Box");
    tl.layers[0].keyframes.insert(0, Keyframe::with(vec![Element::shape("", rect_shape(Color::rgb(200, 0, 0)))]));
    let box_id = d.add_symbol("Box", SymbolKind::Graphic, tl);
    d.swf_char_ids.insert(box_id, 42);
    let mut tl = Timeline::new("Clip");
    tl.layers[0].keyframes.insert(0, Keyframe::with(vec![Element::instance("b1", box_id, Matrix::translate(5.0, 6.0)), Element::bitmap("", 2, Matrix::IDENTITY)]));
    let clip = d.add_symbol("Clip", SymbolKind::MovieClip, tl);
    d.swf_char_ids.insert(clip, 100);
    d.scene.layers[0].keyframes.insert(0, Keyframe::with(vec![Element::instance("", clip, Matrix::translate(30.0, 40.0))]));
    d
}

#[test]
fn export_reuses_remembered_character_ids() {
    let d = doc_with_ids();
    let e = swf_write::export(&d).unwrap();
    assert_eq!(e.bitmap_ids[&1], 7);
    assert_eq!(e.bitmap_ids[&2], 300);
    assert_eq!(e.symbol_ids[&1], 42);
    assert_eq!(e.symbol_ids[&2], 100);
    let sw = swf::parse(&e.bytes).unwrap();
    assert_eq!(&e.bytes[..3], b"CWS");
    let lossless: Vec<u16> = tag_of(&sw, swf::DEFINE_BITS_LOSSLESS2).iter().map(|t| char_id(t)).collect();
    assert_eq!(lossless, vec![7, 300]);
    // the lone-shape symbol is a DefineShape3 with its own id
    let shapes: Vec<u16> = tag_of(&sw, swf::DEFINE_SHAPE3).iter().map(|t| char_id(t)).collect();
    assert!(shapes.contains(&42));
    assert_eq!(char_id(tag_of(&sw, swf::DEFINE_SPRITE)[0]), 100);
}

#[test]
fn fresh_ids_are_above_the_maximum_and_unique() {
    let d = doc_with_ids();
    let e = swf_write::export(&d).unwrap();
    let sw = swf::parse(&e.bytes).unwrap();
    let defs: Vec<u16> = sw.tags.iter().filter(|t| matches!(t.code, swf::DEFINE_SHAPE3 | swf::DEFINE_SPRITE | swf::DEFINE_BITS_LOSSLESS2)).map(char_id).collect();
    let mut sorted = defs.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), defs.len(), "ids collide: {defs:?}");
    assert!(!defs.contains(&0));
    // the bitmap shape the clip needed is new: above 300
    assert!(defs.iter().any(|&i| i > 300));
}

#[test]
fn duplicate_and_missing_ids_never_collide() {
    let mut d = Document::default();
    for (i, id) in [Some(5u16), Some(5), None, Some(0)].into_iter().enumerate() {
        let mut a = BitmapAsset::from_rgba(i as u32 + 1, "x", 1, 1, &[1, 2, 3, 255]).unwrap();
        a.swf_char_id = id;
        d.bitmaps.push(a);
    }
    let e = swf_write::export(&d).unwrap();
    let mut ids: Vec<u16> = e.bitmap_ids.values().copied().collect();
    assert_eq!(e.bitmap_ids[&1], 5);
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), 4);
    assert!(ids.iter().all(|&i| i != 0));
    assert!(e.bitmap_ids[&2] > 5 && e.bitmap_ids[&3] > 5 && e.bitmap_ids[&4] > 5);
}

#[test]
fn import_export_import_keeps_ids() {
    let d = doc_with_ids();
    let first = swf_write::export(&d).unwrap();
    let imp = swf_read::import(&first.bytes).unwrap();
    let doc2 = imp.doc;
    assert_eq!(doc2.width, 200.0);
    assert_eq!(doc2.height, 100.0);
    assert_eq!(doc2.fps, 12.0);
    let mut bm: Vec<u16> = doc2.bitmaps.iter().filter_map(|b| b.swf_char_id).collect();
    bm.sort_unstable();
    assert_eq!(bm, vec![7, 300]);
    let mut sy: Vec<u16> = doc2.swf_char_ids.values().copied().collect();
    sy.sort_unstable();
    assert!(sy.contains(&42) && sy.contains(&100), "{sy:?}");
    // and a second export uses the same set of ids for those items
    let second = swf_write::export(&doc2).unwrap();
    let ids = |b: &[u8]| {
        let s = swf::parse(b).unwrap();
        let mut v: Vec<(u16, u16)> = s.tags.iter().filter(|t| matches!(t.code, swf::DEFINE_SHAPE3 | swf::DEFINE_SPRITE | swf::DEFINE_BITS_LOSSLESS2)).map(|t| (t.code, char_id(t))).collect();
        v.sort_unstable();
        v
    };
    assert_eq!(ids(&first.bytes), ids(&second.bytes));
}

#[test]
fn imported_sprite_keeps_children_and_placement() {
    let d = doc_with_ids();
    let imp = swf_read::import(&swf_write::export(&d).unwrap().bytes).unwrap();
    let doc = imp.doc;
    let clip_sym = doc.library.iter().find(|s| doc.swf_char_ids.get(&s.id) == Some(&100)).unwrap();
    assert_eq!(clip_sym.kind, SymbolKind::MovieClip);
    // two children at two depths -> two layers
    assert_eq!(clip_sym.timeline.layers.len(), 2);
    let main = &doc.scene.layers[0].keyframes[&0].elements[0];
    assert!((main.matrix.tx - 30.0).abs() < 0.01 && (main.matrix.ty - 40.0).abs() < 0.01);
    match &main.kind {
        ElementKind::Instance(i) => assert_eq!(doc.swf_char_ids[&i.symbol], 100),
        _ => panic!("not an instance"),
    }
    // the box shape came back as a path with its fill
    let box_sym = doc.library.iter().find(|s| doc.swf_char_ids.get(&s.id) == Some(&42)).unwrap();
    let el = &box_sym.timeline.layers[0].keyframes[&0].elements[0];
    match &el.kind {
        ElementKind::Shape(s) => {
            assert_eq!(s.fill, Some(Color::rgb(200, 0, 0)));
            let pts: Vec<_> = s.path.segs.iter().filter_map(|g| if let ac_core::geom::Seg::M(p) | ac_core::geom::Seg::L(p) = g { Some(*p) } else { None }).collect();
            let (x0, x1) = (pts.iter().map(|p| p.x).fold(f32::MAX, f32::min), pts.iter().map(|p| p.x).fold(f32::MIN, f32::max));
            let (y0, y1) = (pts.iter().map(|p| p.y).fold(f32::MAX, f32::min), pts.iter().map(|p| p.y).fold(f32::MIN, f32::max));
            assert_eq!((x0, x1, y0, y1), (10.0, 40.0, 20.0, 60.0));
        }
        _ => panic!("not a shape"),
    }
}

#[test]
fn multi_frame_timeline_moves_and_removes() {
    let mut d = Document::default();
    let mut tl = Timeline::new("S");
    tl.layers[0].keyframes.insert(0, Keyframe::with(vec![Element::shape("", rect_shape(Color::rgb(0, 0, 255)))]));
    let s = d.add_symbol("S", SymbolKind::Graphic, tl);
    d.swf_char_ids.insert(s, 9);
    let l = &mut d.scene.layers[0];
    l.keyframes.insert(0, Keyframe::with(vec![Element::instance("", s, Matrix::IDENTITY)]));
    l.keyframes.insert(2, Keyframe::with(vec![Element::instance("", s, Matrix::translate(50.0, 0.0))]));
    l.keyframes.insert(4, Keyframe::blank());
    l.end = 5;
    let e = swf_write::export(&d).unwrap();
    let sw = swf::parse(&e.bytes).unwrap();
    assert_eq!(sw.frame_count, 5);
    assert_eq!(tag_of(&sw, swf::SHOW_FRAME).len(), 5);
    // move (no character) at frame 2, remove at frame 4
    let places = tag_of(&sw, swf::PLACE_OBJECT2);
    assert_eq!(places.len(), 2);
    assert_eq!(places[0].body[0] & 0x02, 0x02);
    assert_eq!(places[1].body[0] & 0x03, 0x01);
    assert_eq!(tag_of(&sw, swf::REMOVE_OBJECT2).len(), 1);
    // and the reader rebuilds the keyframes
    let doc = swf_read::import(&e.bytes).unwrap().doc;
    let kfs: Vec<u32> = doc.scene.layers[0].keyframes.keys().copied().collect();
    assert_eq!(kfs, vec![0, 2, 4]);
    assert_eq!(doc.scene.layers[0].end, 5);
}

#[test]
fn lossless_pixels_round_trip_through_swf() {
    let mut d = Document::default();
    let px: Vec<u8> = (0..16).flat_map(|i| [i * 10, 255 - i * 5, i * 3, 255]).collect();
    d.bitmaps.push(BitmapAsset::from_rgba(1, "p", 4, 4, &px).unwrap());
    let bytes = swf_write::export(&d).unwrap().bytes;
    let doc = swf_read::import(&bytes).unwrap().doc;
    let back = image::load_from_memory(&doc.bitmaps[0].png).unwrap().to_rgba8();
    assert_eq!(back.as_raw(), &px);
}

fn swf_with(tags: Vec<RawTag>) -> Vec<u8> {
    swf::write(11, [0, 400, 0, 400], 24.0, 1, &tags, false)
}

#[test]
fn jpeg_png_and_lossless_bitmaps_are_written_back_byte_identical() {
    let j = jpeg(8, 6);
    let png = BitmapAsset::from_rgba(0, "t", 3, 3, &[7u8; 36]).unwrap().png;
    // premultiplied ARGB, half alpha
    let argb: Vec<u8> = (0..4).flat_map(|_| [128u8, 60, 30, 10]).collect();
    let mut ll2 = vec![5u8];
    ll2.extend_from_slice(&2u16.to_le_bytes());
    ll2.extend_from_slice(&2u16.to_le_bytes());
    ll2.extend_from_slice(&swf::zlib(&argb));
    let mk = |code: u16, id: u16, data: &[u8]| {
        let mut b = id.to_le_bytes().to_vec();
        b.extend_from_slice(data);
        RawTag { code, body: b }
    };
    let src = swf_with(vec![mk(swf::DEFINE_BITS_JPEG2, 11, &j), mk(swf::DEFINE_BITS_JPEG2, 12, &png), mk(swf::DEFINE_BITS_LOSSLESS2, 13, &ll2), swf_tag_show()]);
    let imp = swf_read::import(&src).unwrap();
    let codecs: Vec<_> = imp.doc.bitmaps.iter().map(|b| b.original.as_ref().unwrap().codec).collect();
    assert_eq!(codecs, vec![BitmapCodec::Jpeg, BitmapCodec::Png, BitmapCodec::SwfLossless2]);
    assert_eq!(imp.doc.bitmaps.iter().map(|b| b.swf_char_id.unwrap()).collect::<Vec<_>>(), vec![11, 12, 13]);
    let out = swf::parse(&swf_write::export(&imp.doc).unwrap().bytes).unwrap();
    let find = |id: u16| out.tags.iter().find(|t| t.body.len() > 2 && char_id(t) == id && matches!(t.code, swf::DEFINE_BITS_JPEG2 | swf::DEFINE_BITS_LOSSLESS2)).unwrap();
    assert_eq!(&find(11).body[2..], &j[..]);
    assert_eq!(&find(12).body[2..], &png[..]);
    assert_eq!(&find(13).body[2..], &ll2[..]);
    assert_eq!(find(11).code, swf::DEFINE_BITS_JPEG2);
    assert_eq!(find(13).code, swf::DEFINE_BITS_LOSSLESS2);
}

fn swf_tag_show() -> RawTag {
    RawTag { code: swf::SHOW_FRAME, body: vec![] }
}

#[test]
fn edited_bitmap_is_reencoded_not_passed_through() {
    let j = jpeg(8, 6);
    let mut b = vec![];
    b.extend_from_slice(&3u16.to_le_bytes());
    b.extend_from_slice(&j);
    let src = swf_with(vec![RawTag { code: swf::DEFINE_BITS_JPEG2, body: b }, swf_tag_show()]);
    let mut doc = swf_read::import(&src).unwrap().doc;
    assert!(doc.bitmaps[0].pristine_original().is_some());
    // an edit: new pixels, same asset
    let (w, h) = (doc.bitmaps[0].width, doc.bitmaps[0].height);
    let edited = BitmapAsset::from_rgba(1, "e", w, h, &vec![9u8; (w * h * 4) as usize]).unwrap();
    doc.bitmaps[0].png = edited.png;
    assert!(doc.bitmaps[0].pristine_original().is_none());
    let out = swf::parse(&swf_write::export(&doc).unwrap().bytes).unwrap();
    assert!(out.tags.iter().all(|t| t.code != swf::DEFINE_BITS_JPEG2));
    let t = out.tags.iter().find(|t| t.code == swf::DEFINE_BITS_LOSSLESS2).unwrap();
    assert_eq!(char_id(t), 3, "the edited bitmap keeps its id");
}

#[test]
fn jpeg3_alpha_and_8bit_lossless_import() {
    let j = jpeg(4, 4);
    let alpha = swf::zlib(&[255u8; 16]);
    let mut body = 21u16.to_le_bytes().to_vec();
    body.extend_from_slice(&(j.len() as u32).to_le_bytes());
    body.extend_from_slice(&j);
    body.extend_from_slice(&alpha);
    // 8-bit colour-mapped lossless v1 (2 colours, 2x2, rows padded to 4)
    let mut raw = vec![255, 0, 0, 0, 0, 255];
    raw.extend_from_slice(&[0, 1, 0, 0, 1, 0, 0, 0]);
    let mut l = 22u16.to_le_bytes().to_vec();
    l.push(3);
    l.extend_from_slice(&2u16.to_le_bytes());
    l.extend_from_slice(&2u16.to_le_bytes());
    l.push(1);
    l.extend_from_slice(&swf::zlib(&raw));
    let src = swf_with(vec![RawTag { code: swf::DEFINE_BITS_JPEG3, body }, RawTag { code: swf::DEFINE_BITS_LOSSLESS, body: l }, swf_tag_show()]);
    let doc = swf_read::import(&src).unwrap().doc;
    assert_eq!(doc.bitmaps.len(), 2);
    assert_eq!(doc.bitmaps[0].original.as_ref().unwrap().codec, BitmapCodec::SwfJpeg3);
    let img = image::load_from_memory(&doc.bitmaps[1].png).unwrap().to_rgba8();
    assert_eq!(img.get_pixel(0, 0).0, [255, 0, 0, 255]);
    assert_eq!(img.get_pixel(1, 0).0, [0, 0, 255, 255]);
    assert_eq!(img.get_pixel(0, 1).0, [0, 0, 255, 255]);
}

#[test]
fn bitmap_fill_and_stroke_survive_export() {
    let mut d = Document::default();
    d.bitmaps.push(BitmapAsset::from_rgba(1, "t", 2, 2, &[200; 16]).unwrap());
    let mut s = Shape::new(Path::rect(0.0, 0.0, 10.0, 10.0), None, Some(Stroke { color: Color::rgb(1, 2, 3), width: 2.0 }));
    s.bitmap_fill = Some(BitmapFill { bitmap: 1, matrix: Matrix::IDENTITY });
    d.scene.layers[0].keyframes.insert(0, Keyframe::with(vec![Element::shape("", s)]));
    let e = swf_write::export(&d).unwrap();
    let doc = swf_read::import(&e.bytes).unwrap().doc;
    let sym = &doc.library[0];
    let shapes: Vec<&Shape> = sym.timeline.layers[0].keyframes[&0].elements.iter().filter_map(|e| if let ElementKind::Shape(s) = &e.kind { Some(s) } else { None }).collect();
    assert!(shapes.iter().any(|s| s.bitmap_fill.is_some()), "bitmap fill lost");
    assert!(shapes.iter().any(|s| s.stroke.map(|st| st.width == 2.0 && st.color == Color::rgb(1, 2, 3)).unwrap_or(false)), "stroke lost");
}

#[test]
fn curves_and_cubics_export_as_quadratics_and_return() {
    let mut d = Document::default();
    let p = Path { segs: vec![ac_core::geom::Seg::M(ac_core::geom::pt(0.0, 0.0)), ac_core::geom::Seg::C(ac_core::geom::pt(0.0, 30.0), ac_core::geom::pt(40.0, 30.0), ac_core::geom::pt(40.0, 0.0)), ac_core::geom::Seg::Z] };
    d.scene.layers[0].keyframes.insert(0, Keyframe::with(vec![Element::shape("", Shape::new(p, Some(Color::rgb(0, 0, 0)), None))]));
    let doc = swf_read::import(&swf_write::export(&d).unwrap().bytes).unwrap().doc;
    let ElementKind::Shape(s) = &doc.library[0].timeline.layers[0].keyframes[&0].elements[0].kind else { panic!() };
    let quads = s.path.segs.iter().filter(|g| matches!(g, ac_core::geom::Seg::Q(..))).count();
    assert_eq!(quads, 4);
    // the apex of a cubic with these controls is at y = 22.5
    let top = s.path.segs.iter().filter_map(|g| if let ac_core::geom::Seg::Q(_, p) = g { Some(p.y) } else { None }).fold(0.0f32, f32::max);
    assert!(top > 20.0 && top < 25.0, "{top}");
}

#[test]
fn mm_swf_unsupported_things_warn() {
    let mut d = Document::default();
    d.scene.layers[0].keyframes.insert(0, Keyframe::with(vec![Element::text("hi", 12.0, Color::rgb(0, 0, 0), ac_core::geom::pt(0.0, 0.0))]));
    let e = swf_write::export(&d).unwrap();
    assert!(e.warnings.iter().any(|w| w.contains("text")));
}

// ------------------------------------------------------------------ fonts

fn synthetic_font() -> SwfFont {
    let on = |x, y| OPt { x, y, on: true };
    let off = |x, y| OPt { x, y, on: false };
    // 'A': an outer clockwise (y up) triangle and a counter-clockwise hole
    let a = FontGlyph {
        code: 'A' as u16,
        advance: 600,
        contours: vec![vec![on(0, 0), on(300, 700), on(600, 0)], vec![on(200, 100), on(400, 100), on(300, 400)]],
    };
    // 'B': a box with a curved top
    let b = FontGlyph { code: 'B' as u16, advance: 520, contours: vec![vec![on(50, 0), on(50, 500), off(250, 800), on(450, 500), on(450, 0)]] };
    let space = FontGlyph { code: ' ' as u16, advance: 250, contours: vec![] };
    let c = FontGlyph { code: 0x00E9, advance: 480, contours: vec![vec![on(100, 0), on(100, 400), on(380, 400), on(380, 0)]] };
    SwfFont { name: "Synth".into(), bold: true, italic: false, ascent: 800, descent: 200, leading: 40, glyphs: vec![a, b, space, c] }
}

#[test]
fn define_font3_round_trip() {
    let f = synthetic_font();
    let tag = define_font3(77, &f);
    assert_eq!(tag.code, swf::DEFINE_FONT3);
    assert_eq!(char_id(&tag), 77);
    let back = parse_define_font(tag.code, &tag.body).unwrap();
    assert_eq!(back.name, "Synth");
    assert!(back.bold && !back.italic);
    assert_eq!((back.ascent, back.descent, back.leading), (800, 200, 40));
    assert_eq!(back.glyphs.len(), 4);
    for (a, b) in f.glyphs.iter().zip(&back.glyphs) {
        assert_eq!(a.code, b.code);
        assert_eq!(a.advance, b.advance);
        assert_eq!(a.contours.len(), b.contours.len(), "contours of {:#x}", a.code);
        for (ca, cb) in a.contours.iter().zip(&b.contours) {
            assert_eq!(ca, cb);
        }
    }
}

#[test]
fn swf_font_to_ttf_is_a_valid_font() {
    let f = synthetic_font();
    let ttf = to_ttf(&f);
    let face = ttf_parser::Face::parse(&ttf, 0).expect("parses as TrueType");
    assert_eq!(face.units_per_em(), 1024);
    assert_eq!(face.number_of_glyphs(), 5); // .notdef + 4
    assert_eq!(face.ascender(), 800);
    assert_eq!(face.descender(), -200);
    assert!(face.is_bold());
    let ga = face.glyph_index('A').unwrap();
    assert_eq!(ga.0, 1);
    assert_eq!(face.glyph_index('B').unwrap().0, 2);
    assert_eq!(face.glyph_index(' ').unwrap().0, 3);
    assert_eq!(face.glyph_index('\u{e9}').unwrap().0, 4);
    assert!(face.glyph_index('Z').is_none());
    assert_eq!(face.glyph_hor_advance(ga), Some(600));
    let bb = face.glyph_bounding_box(ga).unwrap();
    assert_eq!((bb.x_min, bb.y_min, bb.x_max, bb.y_max), (0, 0, 600, 700));
    let bbb = face.glyph_bounding_box(face.glyph_index('B').unwrap()).unwrap();
    assert_eq!((bbb.x_min, bbb.x_max, bbb.y_min), (50, 450, 0));
    // the space has no outline
    assert!(face.glyph_bounding_box(face.glyph_index(' ').unwrap()).is_none());
    let family = face.names().into_iter().find(|n| n.name_id == ttf_parser::name_id::FAMILY).and_then(|n| n.to_string());
    assert_eq!(family.as_deref(), Some("Synth"));
}

#[test]
fn ttf_checksum_adjustment_is_consistent() {
    let ttf = to_ttf(&synthetic_font());
    let mut sum = 0u32;
    for c in ttf.chunks(4) {
        let mut b = [0u8; 4];
        b[..c.len()].copy_from_slice(c);
        sum = sum.wrapping_add(u32::from_be_bytes(b));
    }
    assert_eq!(sum, 0xB1B0_AFBA);
}

#[test]
fn ttf_to_define_font3_and_back_round_trip() {
    let f = synthetic_font();
    let ttf = to_ttf(&f);
    let g = from_ttf(&ttf).unwrap();
    assert_eq!(g.name, "Synth");
    let codes: Vec<u16> = g.glyphs.iter().map(|g| g.code).collect();
    assert_eq!(codes, vec![0x20, 'A' as u16, 'B' as u16, 0xE9]);
    let a = g.glyphs.iter().find(|g| g.code == 'A' as u16).unwrap();
    assert_eq!(a.advance, 600);
    assert_eq!(a.contours.len(), 2);
    // through a DefineFont3 tag and a second TrueType
    let tag = define_font3(5, &g);
    let h = parse_define_font(tag.code, &tag.body).unwrap();
    assert_eq!(h.glyphs.len(), g.glyphs.len());
    let b = h.glyphs.iter().find(|g| g.code == 'B' as u16).unwrap();
    assert_eq!(b.contours[0].iter().filter(|p| !p.on).count(), 1);
    let ttf2 = to_ttf(&h);
    let face = ttf_parser::Face::parse(&ttf2, 0).unwrap();
    let bb = face.glyph_bounding_box(face.glyph_index('A').unwrap()).unwrap();
    assert_eq!((bb.x_min, bb.x_max, bb.y_max), (0, 600, 700));
}

#[test]
fn font_parse_rejects_other_tags() {
    assert!(parse_define_font(swf::DEFINE_SHAPE3, &[]).is_err());
}

// ---- Flash QA section 8 fixes (F3/F4/F6/F7): fixtures made by tools/flash/tests/make_flash_fixtures.py ----

fn fixture(name: &str) -> Vec<u8> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/swf").join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

#[test]
fn all_three_compressions_parse_to_the_same_header() {
    for n in ["tiny_fws.swf", "tiny_cws.swf", "tiny_zws.swf", "mix_zws.swf", "mix_cws.swf"] {
        let b = fixture(n);
        let c = swf::validate(&b).unwrap_or_else(|e| panic!("{n}: {e}"));
        assert_eq!(c.declared_len as usize, c.actual_len, "{n}");
        let s = swf::parse(&b).unwrap();
        assert_eq!(s.stage, [0, 11000, 0, 8000], "{n}"); // 550x400 px
        assert_eq!(s.frame_rate, 12.0, "{n}");
        assert_eq!(s.frame_count, 2, "{n}");
        assert!(s.tags.iter().any(|t| t.code == 82), "{n}: DoABC2 present");
    }
}

#[test]
fn lzma_and_zlib_bodies_are_identical_to_the_plain_one() {
    let plain = swf::body_of(&fixture("mix_cws.swf")).unwrap().1;
    assert!(plain.len() > 2000);
    assert_eq!(swf::body_of(&fixture("mix_zws.swf")).unwrap().1, plain);
}

#[test]
fn truncated_compressed_swf_is_rejected_like_truncated_fws() {
    for n in ["tiny_fws_trunc.swf", "mix_cws_trunc.swf", "mix_zws_trunc.swf"] {
        let e = swf::validate(&fixture(n)).unwrap_err();
        assert!(e.contains("malformed SWF") || e.contains("FileLength"), "{n}: {e}");
        assert!(swf::parse(&fixture(n)).is_err() || n.starts_with("tiny_fws"), "{n}");
    }
    // zlib stream complete but FileLength claims more: also rejected
    let e = swf::validate(&fixture("cws_filelength_lie.swf")).unwrap_err();
    assert!(e.contains("FileLength"), "{e}");
}

#[test]
fn trailing_junk_after_a_complete_zlib_stream_is_not_truncation() {
    // QA's "truncated CWS accepted" (F4) was this shape: nothing missing from the SWF.
    assert!(swf::validate(&fixture("cws_trailing_junk.swf")).is_ok());
}

#[test]
fn absurd_or_inverted_stage_rect_is_rejected_with_a_message() {
    let e = swf::validate(&fixture("bad_rect_huge.swf")).unwrap_err();
    assert!(e.contains("absurd stage size") && e.contains("16384"), "{e}");
    assert!(swf::parse(&fixture("bad_rect_huge.swf")).is_err());
    let e = swf::validate(&fixture("bad_rect_negative.swf")).unwrap_err();
    assert!(e.contains("negative stage size"), "{e}");
    assert!(swf::check_stage([i32::MIN, i32::MAX, 0, 0]).is_err()); // overflow in i32 math
    assert!(swf::check_stage([-8000, 8000, -100, 100]).is_ok()); // negative origin is legal
}

#[test]
fn empty_and_short_input_give_plain_messages() {
    let e = swf::validate(&fixture("empty.swf")).unwrap_err();
    assert!(e.contains("empty file"), "{e}");
    let e = swf::parse(&fixture("short.swf")).unwrap_err();
    assert!(e.contains("too short"), "{e}");
    assert!(swf::validate(b"XYZ\x09\0\0\0\0\0").unwrap_err().contains("bad signature"));
}

#[test]
fn lzma_rejects_garbage_without_panicking() {
    let mut b = fixture("mix_zws.swf");
    for i in (20..b.len()).step_by(7) {
        b[i] ^= 0x5A;
    }
    let _ = swf::validate(&b); // Ok or Err, never a panic
    assert!(super::lzma::decode(&[0xFF, 0, 0, 0, 0], &[0, 0, 0, 0, 0], 4).is_err());
    assert!(super::lzma::decode(&[0x5D, 0, 0, 1, 0], &[1, 0, 0, 0, 0], 4).is_err());
}
