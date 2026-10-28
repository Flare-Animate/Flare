//! Input sniffing for flare-cli: files are dispatched by magic bytes, never by
//! extension alone (a ".xfl" can be a zip or a marker file; a ".swf" can be
//! anything). Also the `info` report for SWF files.

use ac_core::{Document};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Kind {
    Swf,
    Zip,
    Xml,
    Other,
}

pub fn sniff(head: &[u8]) -> Kind {
    if head.len() >= 3 && matches!(&head[..3], b"FWS" | b"CWS" | b"ZWS") {
        Kind::Swf
    } else if head.starts_with(b"PK") {
        Kind::Zip
    } else if head.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(head).iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'<') {
        Kind::Xml
    } else {
        Kind::Other
    }
}

fn load_swf(src: &str, bytes: &[u8]) -> Result<Document, String> {
    ac_xfl::swf::validate(bytes).map_err(|e| format!("{src}: {e}"))?;
    let im = ac_xfl::swf_read::import(bytes).map_err(|e| format!("{src}: {e}"))?;
    for w in &im.warnings {
        eprintln!("note: {w}");
    }
    Ok(im.doc)
}

/// Load any supported file by content. `acx` zips go to the package reader.
pub fn load_file(src: &str) -> Result<Document, String> {
    let p = Path::new(src);
    let bytes = std::fs::read(p).map_err(|e| format!("{src}: {e}"))?;
    let low = src.to_ascii_lowercase();
    match sniff(&bytes) {
        Kind::Swf => load_swf(src, &bytes),
        Kind::Zip if low.ends_with(".acx") => {
            let l = ac_package::open(p)?;
            for w in &l.warnings {
                eprintln!("note: {w}");
            }
            Ok(l.doc)
        }
        k if low.ends_with(".xfl") || low.ends_with(".fla") || k == Kind::Zip => {
            let im = ac_xfl::open(p)?;
            for w in &im.warnings {
                eprintln!("note: {w}");
            }
            Ok(im.doc)
        }
        _ => match std::str::from_utf8(&bytes) {
            Ok(s) => Document::from_json(s).map_err(|e| format!("{src}: {e}")),
            Err(_) => Err(format!("{src}: unrecognised binary file (not SWF, ZIP/.fla/.xfl or Flare JSON)")),
        },
    }
}

fn tag_name(code: u16) -> String {
    let n = match code {
        0 => "End",
        1 => "ShowFrame",
        2 => "DefineShape",
        4 => "PlaceObject",
        5 => "RemoveObject",
        6 => "DefineBits",
        8 => "JPEGTables",
        9 => "SetBackgroundColor",
        10 => "DefineFont",
        11 => "DefineText",
        12 => "DoAction",
        14 => "DefineSound",
        20 => "DefineBitsLossless",
        21 => "DefineBitsJPEG2",
        22 => "DefineShape2",
        26 => "PlaceObject2",
        28 => "RemoveObject2",
        32 => "DefineShape3",
        35 => "DefineBitsJPEG3",
        36 => "DefineBitsLossless2",
        39 => "DefineSprite",
        43 => "FrameLabel",
        48 => "DefineFont2",
        69 => "FileAttributes",
        70 => "PlaceObject3",
        72 => "DoABC (old)",
        75 => "DefineFont3",
        76 => "SymbolClass",
        82 => "DoABC",
        83 => "DefineShape4",
        87 => "DefineBinaryData",
        90 => "DefineBitsJPEG4",
        _ => return format!("tag{code}"),
    };
    n.to_string()
}

/// Human-readable report for a SWF file's bytes.
pub fn swf_info(name: &str, bytes: &[u8]) -> Result<String, String> {
    let chk = ac_xfl::swf::validate(bytes).map_err(|e| format!("{name}: {e}"))?;
    let sw = ac_xfl::swf::parse(bytes).map_err(|e| format!("{name}: {e}"))?;
    let mut hist: BTreeMap<u16, usize> = BTreeMap::new();
    for t in &sw.tags {
        *hist.entry(t.code).or_default() += 1;
    }
    let n = |codes: &[u16]| codes.iter().map(|c| hist.get(c).copied().unwrap_or(0)).sum::<usize>();
    let chars = n(&[2, 6, 10, 11, 14, 20, 21, 22, 32, 33, 34, 35, 36, 37, 39, 46, 48, 60, 62, 75, 83, 84, 87, 90]);
    let bitmaps = n(&[6, 20, 21, 35, 36, 90]);
    let sounds = n(&[14]);
    let abc = n(&[72, 82]);
    let comp = match chk.signature.as_str() {
        "FWS" => "FWS (uncompressed)",
        "CWS" => "CWS (zlib)",
        _ => "ZWS (LZMA)",
    };
    let mut s = String::new();
    s += &format!("SWF version {}, {}\n", chk.version, comp);
    s += &format!("File length {} bytes (header consistent)\n", chk.actual_len);
    s += &format!(
        "Stage {}x{} px (twips {}..{} x {}..{})\n",
        (sw.stage[1] - sw.stage[0]) as f32 / 20.0,
        (sw.stage[3] - sw.stage[2]) as f32 / 20.0,
        sw.stage[0], sw.stage[1], sw.stage[2], sw.stage[3]
    );
    s += &format!("Frame rate {} fps, {} frames\n", sw.frame_rate, sw.frame_count);
    s += &format!("Characters {}, bitmaps {}, sounds {}\n", chars, bitmaps, sounds);
    s += &format!("ActionScript 3 (ABC): {}\n", if abc > 0 { "yes" } else { "no" });
    s += "Tags:\n";
    for (c, k) in &hist {
        s += &format!("  {:<22} {}\n", tag_name(*c), k);
    }
    Ok(s)
}

/// `info` for a file path if it is a SWF (by magic). None when it is not.
pub fn info_if_swf(src: &str) -> Result<Option<String>, String> {
    let Ok(bytes) = std::fs::read(src) else { return Ok(None) };
    if sniff(&bytes) != Kind::Swf {
        return Ok(None);
    }
    swf_info(src, &bytes).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ac_xfl::swf::{self, RawTag};

    fn tiny(compress: bool) -> Vec<u8> {
        let tags = [RawTag { code: swf::SET_BACKGROUND, body: vec![1, 2, 3] }, RawTag { code: swf::SHOW_FRAME, body: vec![] }, RawTag { code: 82, body: vec![0; 4] }];
        swf::write(9, [0, 4000, 0, 3000], 12.0, 1, &tags, compress)
    }

    fn tmp(name: &str, data: &[u8]) -> String {
        let p = std::env::temp_dir().join(format!("flare_cli_{}_{}", std::process::id(), name));
        std::fs::write(&p, data).unwrap();
        p.to_string_lossy().into_owned()
    }

    #[test]
    fn sniff_kinds() {
        assert_eq!(sniff(b"FWS\x09"), Kind::Swf);
        assert_eq!(sniff(b"CWS"), Kind::Swf);
        assert_eq!(sniff(b"ZWS"), Kind::Swf);
        assert_eq!(sniff(b"PK\x03\x04"), Kind::Zip);
        assert_eq!(sniff(b"  \n<xml"), Kind::Xml);
        assert_eq!(sniff(b"{}"), Kind::Other);
    }

    #[test]
    fn info_fws_and_cws() {
        for c in [false, true] {
            let s = swf_info("t.swf", &tiny(c)).unwrap();
            assert!(s.contains("SWF version 9"), "{s}");
            assert!(s.contains(if c { "CWS" } else { "FWS" }));
            assert!(s.contains("Stage 200x150"), "{s}");
            assert!(s.contains("12 fps, 1 frames"), "{s}");
            assert!(s.contains("ActionScript 3 (ABC): yes"));
            assert!(s.contains("ShowFrame"));
        }
    }

    #[test]
    fn zws_refused_clearly() {
        let mut b = tiny(false);
        b[..3].copy_from_slice(b"ZWS");
        let e = swf_info("z.swf", &b).unwrap_err();
        assert!(e.contains("malformed SWF") && !e.contains("UTF-8"), "{e}");
    }

    #[test]
    fn malformed_lengths_rejected() {
        let mut zero = tiny(false);
        zero[4..8].copy_from_slice(&0u32.to_le_bytes());
        assert!(swf_info("a.swf", &zero).unwrap_err().contains("FileLength=0"));
        let mut long = tiny(false);
        let n = long.len() as u32 + 50;
        long[4..8].copy_from_slice(&n.to_le_bytes());
        assert!(swf_info("a.swf", &long).unwrap_err().contains("FileLength"));
    }

    #[test]
    fn missing_end_and_overrun_rejected() {
        let mut b = tiny(false);
        b.truncate(b.len() - 2);
        let n = b.len() as u32;
        b[4..8].copy_from_slice(&n.to_le_bytes());
        assert!(swf_info("a.swf", &b).unwrap_err().contains("End tag"));
        let mut o = tiny(false);
        o.truncate(o.len() - 2);
        o.extend_from_slice(&(((1u16) << 6) | 0x3F).to_le_bytes());
        o.extend_from_slice(&500u32.to_le_bytes());
        let n = o.len() as u32;
        o[4..8].copy_from_slice(&n.to_le_bytes());
        assert!(swf_info("a.swf", &o).unwrap_err().contains("past end"));
    }

    #[test]
    fn swf_loads_regardless_of_extension() {
        let p = tmp("x.dat", &tiny(true));
        let d = load_file(&p).unwrap();
        assert_eq!((d.width, d.height), (200.0, 150.0));
        assert!(info_if_swf(&p).unwrap().is_some());
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn binary_garbage_is_not_utf8_error() {
        let p = tmp("g.bin", &[0xff, 0xfe, 0x00, 0x80]);
        let e = load_file(&p).unwrap_err();
        assert!(e.contains("unrecognised") && !e.contains("UTF-8"), "{e}");
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn zipped_xfl_and_fla_load_and_errors_name_file() {
        let doc = ac_core::demo::demo_document();
        let (pkg, _) = ac_xfl::write_package(&doc);
        let z = ac_xfl::zip(&pkg).unwrap();
        for n in ["a.fla", "b.xfl", "c.bin"] {
            let p = tmp(n, &z);
            let d = load_file(&p).unwrap_or_else(|e| panic!("{n}: {e}"));
            assert_eq!(d.width, doc.width);
            let _ = std::fs::remove_file(p);
        }
        // zip without DOMDocument.xml: error names the file
        let mut empty = ac_xfl::Package::new();
        empty.insert("junk.txt".into(), b"x".to_vec());
        let p = tmp("d.fla", &ac_xfl::zip(&empty).unwrap());
        let e = load_file(&p).unwrap_err();
        assert!(e.contains("d.fla"), "{e}");
        let _ = std::fs::remove_file(p);
        // marker .xfl in a folder without DOMDocument.xml names the marker file
        let dir = std::env::temp_dir().join(format!("flare_cli_dir_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("m.xfl");
        std::fs::write(&p, b"PROXY-CS5").unwrap();
        let e = load_file(p.to_str().unwrap()).unwrap_err();
        assert!(e.contains("m.xfl") && e.contains("DOMDocument.xml"), "{e}");
        let _ = std::fs::remove_dir_all(dir);
    }
}
