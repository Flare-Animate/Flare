//! Legacy-FLA CFBF/OLE2 reader, port of CfbfReader (SWFAssets.cpp). [MS-CFB].
//! Streams reassembled whole; bitmap carving by JPEG/PNG signature per stream.

const FREE: u32 = 0xFFFF_FFFE;

fn u32at(b: &[u8], o: usize) -> u32 {
    b.get(o..o + 4).map_or(0xFFFF_FFFF, |s| u32::from_le_bytes(s.try_into().unwrap()))
}

pub struct Stream {
    pub name: String,
    pub data: Vec<u8>,
}

fn chain(fat: &[u32], start: u32) -> Vec<u32> {
    let (mut out, mut s) = (vec![], start);
    while s < FREE && (s as usize) < fat.len() && out.len() <= fat.len() {
        out.push(s);
        s = fat[s as usize];
    }
    out
}

/// All type-2 streams (name from UTF-16 dir entry, bytes). None if not CFBF.
pub fn parse(d: &[u8]) -> Option<Vec<Stream>> {
    if d.len() < 512 || d[..8] != [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1] {
        return None;
    }
    let sh = u16::from_le_bytes([d[30], d[31]]);
    let msh = u16::from_le_bytes([d[32], d[33]]);
    if !(7..=20).contains(&sh) || !(1..=12).contains(&msh) {
        return None;
    }
    let (ss, ms) = (1usize << sh, 1usize << msh);
    let sector = |i: u32| -> &[u8] {
        let o = 512 + i as usize * ss;
        d.get(o..o + ss).unwrap_or(&[])
    };
    let mut difat: Vec<u32> = (0..109).map(|i| u32at(d, 76 + i * 4)).collect();
    let (mut nx, mut g) = (u32at(d, 68), 0);
    while nx < FREE && g < 1_000_000 {
        let s = sector(nx);
        if s.len() < ss {
            break;
        }
        let n = ss / 4;
        difat.extend((0..n - 1).map(|i| u32at(s, i * 4)));
        nx = u32at(s, (n - 1) * 4);
        g += 1;
    }
    let mut fat = vec![];
    for f in difat.into_iter().filter(|&f| f < FREE) {
        let s = sector(f);
        if s.len() == ss {
            fat.extend((0..ss / 4).map(|i| u32at(s, i * 4)));
        }
    }
    if fat.is_empty() {
        return None;
    }
    let read = |st: u32| -> Vec<u8> { chain(&fat, st).iter().flat_map(|&s| sector(s).iter().copied()).collect() };
    let dir = read(u32at(d, 48));
    if dir.len() < 128 {
        return None;
    }
    let mut mfat = vec![];
    for s in chain(&fat, u32at(d, 60)) {
        let sc = sector(s);
        mfat.extend((0..sc.len() / 4).map(|i| u32at(sc, i * 4)));
    }
    let mini = read(u32at(&dir, 116));
    let cutoff = u32at(d, 56) as u64;
    let mut out = vec![];
    for e in dir.chunks_exact(128) {
        if e[66] != 2 {
            continue;
        }
        let start = u32at(e, 116);
        let size = u32at(e, 120) as u64 | (u32at(e, 124) as u64) << 32;
        let mut data: Vec<u8> = if size < cutoff {
            chain(&mfat, start)
                .iter()
                .flat_map(|&s| mini.get(s as usize * ms..).map_or(&[][..], |m| &m[..ms.min(m.len())]).iter().copied())
                .collect()
        } else {
            read(start)
        };
        data.truncate(size.min(data.len() as u64) as usize);
        if data.is_empty() {
            continue;
        }
        let nl = (u16::from_le_bytes([e[64], e[65]]) as usize).saturating_sub(2).min(64);
        let u: Vec<u16> = (0..nl / 2).map(|i| u16::from_le_bytes([e[i * 2], e[i * 2 + 1]])).collect();
        out.push(Stream { name: String::from_utf16_lossy(&u), data });
    }
    Some(out)
}

/// Carve embedded images: (offset, len, is_png). Window = up to next signature.
pub fn carve(b: &[u8]) -> Vec<(usize, usize, bool)> {
    let mut out = vec![];
    for (sig, png) in [(&b"\xFF\xD8\xFF"[..], false), (&b"\x89PNG\r\n\x1a\n"[..], true)] {
        let pos: Vec<usize> = b.windows(sig.len()).enumerate().filter(|(_, w)| *w == sig).map(|(i, _)| i).take(4096).collect();
        for (k, &s) in pos.iter().enumerate() {
            out.push((s, pos.get(k + 1).copied().unwrap_or(b.len()) - s, png));
        }
    }
    out
}

/// Opaque C handle: parsed legacy-FLA streams.
pub struct FlareCfbf(Vec<Stream>);

/// Open legacy FLA; NULL if not CFBF. Free with flare_cfbf_free.
#[no_mangle]
pub unsafe extern "C" fn flare_cfbf_open(data: *const u8, len: usize) -> *mut FlareCfbf {
    if data.is_null() {
        return std::ptr::null_mut();
    }
    parse(std::slice::from_raw_parts(data, len)).map_or(std::ptr::null_mut(), |s| Box::into_raw(Box::new(FlareCfbf(s))))
}
#[no_mangle]
pub unsafe extern "C" fn flare_cfbf_free(h: *mut FlareCfbf) {
    if !h.is_null() {
        drop(Box::from_raw(h));
    }
}
#[no_mangle]
pub unsafe extern "C" fn flare_cfbf_count(h: *const FlareCfbf) -> usize {
    h.as_ref().map_or(0, |h| h.0.len())
}
/// Stream `i` bytes: sets *len; pointer valid until free (NULL if bad index).
#[no_mangle]
pub unsafe extern "C" fn flare_cfbf_stream(h: *const FlareCfbf, i: usize, len: *mut usize) -> *const u8 {
    match h.as_ref().and_then(|h| h.0.get(i)) {
        Some(s) => {
            if !len.is_null() {
                *len = s.data.len();
            }
            s.data.as_ptr()
        }
        None => std::ptr::null(),
    }
}
/// Image candidates in stream `i` as (offset,len,is_png) u64 triples into `out` (cap triples). Returns total.
#[no_mangle]
pub unsafe extern "C" fn flare_cfbf_bitmaps(h: *const FlareCfbf, i: usize, out: *mut u64, cap: usize) -> usize {
    let Some(s) = h.as_ref().and_then(|h| h.0.get(i)) else { return 0 };
    let v = carve(&s.data);
    if !out.is_null() {
        for (k, (o, l, p)) in v.iter().take(cap).enumerate() {
            *out.add(k * 3) = *o as u64;
            *out.add(k * 3 + 1) = *l as u64;
            *out.add(k * 3 + 2) = *p as u64;
        }
    }
    v.len()
}

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn rejects_junk() {
        assert!(parse(b"nope").is_none());
    }
    #[test]
    fn tiny_cs3_fixture_lists_streams_and_carves_the_png() {
        // fixture from tools/flash/tests/make_flash_fixtures.py
        let d = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/common/flash/tests/fixtures/swf/cs3_tiny.fla")).unwrap();
        let s = parse(&d).expect("valid CFBF");
        let names: Vec<&str> = s.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, ["Contents", "Media 1", "Symbol 1"]);
        let m = s.iter().find(|x| x.name == "Media 1").unwrap();
        assert!(carve(&m.data).iter().any(|&(_, _, png)| png));
        // truncated and bit-flipped copies never panic
        for n in [600usize, 1024, 5000] {
            let _ = parse(&d[..n]);
        }
        let mut b = d.clone();
        for i in (512..b.len()).step_by(97) {
            b[i] ^= 0xFF;
        }
        let _ = parse(&b);
    }
    #[test]
    fn samples() {
        let base = "C:/Users/charl/Downloads/Flare Samples";
        for p in ["assets_2-2.fla", "online/fla/icons.fla", "online/fla/branding.fla"] {
            let Ok(d) = std::fs::read(format!("{base}/{p}")) else { continue };
            match parse(&d) {
                Some(s) => {
                    let n: usize = s.iter().map(|x| carve(&x.data).len()).sum();
                    eprintln!("{p}: {} streams, {n} image candidates", s.len());
                    assert!(!s.is_empty());
                }
                None => eprintln!("{p}: not CFBF (zip/xfl)"),
            }
        }
    }
}
