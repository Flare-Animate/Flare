//! Texture atlas packing (max-rects): no overlap, inside bounds, pixels intact.

use ac_render::formats::{decode_atlas_image, export_atlas, max_rects, pack_atlas, AtlasInput, AtlasOpts};

fn solid(name: &str, w: u32, h: u32, c: [u8; 4]) -> AtlasInput {
    AtlasInput { name: name.into(), w, h, rgba: (0..w * h).flat_map(|_| c).collect() }
}

/// Deterministic pseudo-random sizes.
fn sizes(n: usize, seed: u32, max: u32) -> Vec<(u32, u32)> {
    let mut s = seed;
    let mut next = || {
        s = s.wrapping_mul(1664525).wrapping_add(1013904223);
        (s >> 16) % max + 1
    };
    (0..n).map(|_| (next(), next())).collect()
}

fn check_layout(sz: &[(u32, u32)], p: &[ac_render::formats::Placement], bw: u32, bh: u32) {
    let rects: Vec<(u32, u32, u32, u32)> = sz.iter().zip(p).map(|(&(w, h), p)| if p.rotated { (p.x, p.y, h, w) } else { (p.x, p.y, w, h) }).collect();
    for (i, a) in rects.iter().enumerate() {
        assert!(a.0 + a.2 <= bw && a.1 + a.3 <= bh, "rect {i} {a:?} outside {bw}x{bh}");
        for (j, b) in rects.iter().enumerate().skip(i + 1) {
            let overlap = a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3;
            assert!(!overlap, "rects {i} {a:?} and {j} {b:?} overlap");
        }
    }
}

#[test]
fn max_rects_never_overlaps_and_stays_in_bounds() {
    for seed in 1..40 {
        for rot in [false, true] {
            let sz = sizes(25, seed, 60);
            if let Some(p) = max_rects(&sz, 256, 256, rot) {
                check_layout(&sz, &p, 256, 256);
                if !rot {
                    assert!(p.iter().all(|p| !p.rotated));
                }
            }
        }
    }
}

#[test]
fn exact_fit_and_overflow() {
    // four 50x50 squares fill a 100x100 bin exactly
    let sz = vec![(50, 50); 4];
    let p = max_rects(&sz, 100, 100, false).expect("fits exactly");
    check_layout(&sz, &p, 100, 100);
    // a fifth cannot
    assert!(max_rects(&vec![(50, 50); 5], 100, 100, false).is_none());
    // too wide without rotation, fine with it
    assert!(max_rects(&[(150, 20)], 100, 200, false).is_none());
    let p = max_rects(&[(150, 20)], 200, 100, true).unwrap();
    assert!(!p[0].rotated);
    let p = max_rects(&[(20, 150)], 100, 200, true).unwrap();
    assert!(!p[0].rotated);
    let p = max_rects(&[(150, 20)], 30, 200, true).unwrap();
    assert!(p[0].rotated);
}

#[test]
fn pack_atlas_places_every_image_with_its_pixels() {
    let inputs: Vec<AtlasInput> = (0..12).map(|i| solid(&format!("s{i}"), 10 + i * 3, 8 + (i * 7) % 20, [i as u8 * 20, 255 - i as u8 * 10, 7, 255])).collect();
    let o = AtlasOpts { max_size: 256, padding: 2, allow_rotate: false, power_of_two: false };
    let a = pack_atlas(&inputs, &o).unwrap();
    assert!(a.width <= 256 && a.height <= 256);
    assert_eq!(a.rgba.len(), (a.width * a.height * 4) as usize);
    assert_eq!(a.entries.len(), inputs.len());
    for (i, e) in a.entries.iter().enumerate() {
        assert_eq!(e.name, inputs[i].name, "input order kept");
        assert!(e.x + e.w <= a.width && e.y + e.h <= a.height);
        // every pixel of the cell is the image's colour
        let want = &inputs[i].rgba[..4];
        for y in 0..e.h {
            for x in 0..e.w {
                let d = (((e.y + y) * a.width + e.x + x) * 4) as usize;
                assert_eq!(&a.rgba[d..d + 4], want);
            }
        }
    }
    // padding: cells are at least `padding` apart
    for (i, p) in a.entries.iter().enumerate() {
        for q in a.entries.iter().skip(i + 1) {
            let gap_x = q.x as i64 - (p.x + p.w) as i64;
            let gap_x2 = p.x as i64 - (q.x + q.w) as i64;
            let gap_y = q.y as i64 - (p.y + p.h) as i64;
            let gap_y2 = p.y as i64 - (q.y + q.h) as i64;
            assert!(gap_x >= 2 || gap_x2 >= 2 || gap_y >= 2 || gap_y2 >= 2, "{} and {} closer than the padding", p.name, q.name);
        }
    }
}

#[test]
fn rotated_cells_hold_the_turned_image() {
    let tall = AtlasInput { name: "t".into(), w: 1, h: 4, rgba: vec![9; 16] };
    let wide = AtlasInput { name: "w".into(), w: 4, h: 1, rgba: [1, 2, 3, 255].repeat(4) };
    let o = AtlasOpts { max_size: 4, padding: 0, allow_rotate: true, power_of_two: false };
    let a = pack_atlas(&[tall, wide], &o).unwrap();
    assert!(a.width <= 4 && a.height <= 4);
    let e = &a.entries[1];
    let px = |x: u32, y: u32| &a.rgba[((y * a.width + x) * 4) as usize..][..4];
    for y in 0..e.h {
        for x in 0..e.w {
            assert_eq!(px(e.x + x, e.y + y), &[1, 2, 3, 255]);
        }
    }
    // a rotated asymmetric image: clockwise turn puts source (0,0) at top-right
    let asym = AtlasInput { name: "a".into(), w: 3, h: 1, rgba: vec![10, 0, 0, 255, 20, 0, 0, 255, 30, 0, 0, 255] };
    let o = AtlasOpts { max_size: 3, padding: 0, allow_rotate: true, power_of_two: false };
    let blocker = AtlasInput { name: "b".into(), w: 2, h: 3, rgba: vec![0; 24] };
    let a = pack_atlas(&[blocker, asym], &o).unwrap();
    let e = &a.entries[1];
    assert!(e.rotated, "{e:?}");
    assert_eq!((e.w, e.h), (1, 3));
    let reds: Vec<u8> = (0..3).map(|y| a.rgba[(((e.y + y) * a.width + e.x) * 4) as usize]).collect();
    // turned clockwise, the source's left end ends up on top
    assert_eq!(reds, vec![10, 20, 30]);
}

#[test]
fn errors_are_reported() {
    let o = AtlasOpts { max_size: 16, ..Default::default() };
    assert!(pack_atlas(&[], &o).is_err());
    assert!(pack_atlas(&[solid("big", 32, 4, [0; 4])], &o).unwrap_err().contains("big"));
    let many: Vec<_> = (0..40).map(|i| solid(&format!("m{i}"), 10, 10, [1, 1, 1, 255])).collect();
    assert!(pack_atlas(&many, &o).unwrap_err().contains("do not fit"));
    let bad = AtlasInput { name: "bad".into(), w: 2, h: 2, rgba: vec![0; 3] };
    assert!(pack_atlas(&[bad], &AtlasOpts::default()).is_err());
}

#[test]
fn power_of_two_rounds_the_size_up() {
    let o = AtlasOpts { max_size: 256, padding: 0, allow_rotate: false, power_of_two: true };
    let a = pack_atlas(&[solid("a", 20, 30, [1, 2, 3, 255]), solid("b", 20, 30, [1, 2, 3, 255])], &o).unwrap();
    assert!(a.width.is_power_of_two() && a.height.is_power_of_two());
}

#[test]
fn json_and_png_files_are_written() {
    let dir = std::env::temp_dir().join(format!("flare_atlas_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mk = |n: &str, w: u32, h: u32, c: [u8; 4]| {
        let a = solid(n, w, h, c);
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&a.rgba).unwrap();
        std::fs::write(dir.join(format!("{n}.png")), out).unwrap();
    };
    mk("red", 16, 16, [255, 0, 0, 255]);
    mk("green", 8, 24, [0, 255, 0, 128]);
    std::fs::write(dir.join("notes.txt"), "ignored").unwrap();
    let (p, j, atlas) = export_atlas(&[dir.clone()], &AtlasOpts::default(), &dir.join("out")).unwrap();
    assert_eq!(atlas.entries.len(), 2);
    let img = decode_atlas_image("o", &std::fs::read(&p).unwrap()).unwrap();
    assert_eq!((img.w, img.h), (atlas.width, atlas.height));
    let json = std::fs::read_to_string(&j).unwrap();
    assert!(json.contains("\"red\"") && json.contains("\"green\"") && json.contains("\"image\":\"out.png\""), "{json}");
    assert!(json.contains(&format!("\"size\":{{\"w\":{},\"h\":{}}}", atlas.width, atlas.height)));
    let _ = std::fs::remove_dir_all(&dir);
}
