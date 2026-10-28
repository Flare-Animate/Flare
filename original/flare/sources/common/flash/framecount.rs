//! Frame count for Flash files (SWF, zip FLA, XFL dir / DOMDocument.xml) for the file browser.
use std::path::Path;

/// Cap on any single inflated stream (decompression-bomb guard): 512 MiB.
pub const MAX_INFLATE: usize = 512 * 1024 * 1024;

fn u16le(d: &[u8], i: usize) -> Option<usize> {
    Some(u16::from_le_bytes(d.get(i..i + 2)?.try_into().ok()?) as usize)
}
fn u32le(d: &[u8], i: usize) -> Option<usize> {
    Some(u32::from_le_bytes(d.get(i..i + 4)?.try_into().ok()?) as usize)
}

/// Read one member of a zip via its central directory (stored or deflate).
pub fn zip_member(d: &[u8], name: &str) -> Option<Vec<u8>> {
    let mut p = 0;
    while let Some(i) = d[p..].windows(4).position(|w| w == b"PK\x01\x02") {
        let c = p + i;
        p = c + 4;
        let (method, csize, nlen, xlen, klen, off) = (
            u16le(d, c + 10)?,
            u32le(d, c + 20)?,
            u16le(d, c + 28)?,
            u16le(d, c + 30)?,
            u16le(d, c + 32)?,
            u32le(d, c + 42)?,
        );
        let n = d.get(c.checked_add(46)?..c.checked_add(46)?.checked_add(nlen)?)?;
        let _ = (xlen, klen);
        if n != name.as_bytes() || d.get(off..off + 4)? != b"PK\x03\x04" {
            continue;
        }
        let start = off.checked_add(30)?.checked_add(u16le(d, off + 26)?)?.checked_add(u16le(d, off + 28)?)?;
        let raw = d.get(start..start.checked_add(csize)?)?;
        return match method {
            0 => Some(raw.to_vec()),
            8 => miniz_oxide::inflate::decompress_to_vec_with_limit(raw, MAX_INFLATE).ok(),
            _ => None,
        };
    }
    None
}

/// Last frame (max index+duration) of the first XFL timeline.
pub fn xfl_frames(dom: &[u8]) -> Option<u32> {
    crate::xfl::layers(dom)?
        .iter()
        .flat_map(|l| l.frames.iter().map(|f| f.index.saturating_add(f.duration)))
        .max()
}

pub fn swf_frame_count(d: &[u8]) -> Option<u32> {
    let (c, _, _) = crate::swf_header(d)?;
    let body;
    let full: &[u8] = match c {
        b'F' => d,
        b'C' => {
            body = [
                b"FWS".as_slice(),
                &d[3..8],
                &miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&d[8..], MAX_INFLATE).ok()?,
            ]
            .concat();
            &body
        }
        _ => return None, // ponytail: ZWS (LZMA) not decoded
    };
    crate::swf_frames(full).map(|(_, n)| n as u32)
}

fn u32at(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(o..o.checked_add(4)?)?.try_into().ok()?))
}

/// CFBF directory: (name, size) of each stream. Header DIFAT only (files under
/// ~7MB at 512-byte sectors). Used by the legacy-FLA import log so the user can
/// see what a binary .fla actually contains.
pub fn cfbf_streams(d: &[u8]) -> Option<Vec<(String, u64)>> {
    let ss = crate::cfbf_header(d)?.1 as usize;
    if ss < 128 {
        return None;
    }
    let sec = |n: u32| ss.saturating_add((n as usize).saturating_mul(ss));
    let mut fat = Vec::new();
    for i in 0..109 {
        let s = u32at(d, 0x4C + i * 4)?;
        if s >= 0xFFFF_FFFA {
            break;
        }
        for k in 0..ss / 4 {
            fat.push(u32at(d, sec(s).saturating_add(k * 4)).unwrap_or(0xFFFF_FFFE));
        }
    }
    let (mut s, mut out, mut guard) = (u32at(d, 0x30)?, Vec::new(), 0);
    while s < 0xFFFF_FFFA && guard < 100_000 {
        for e in 0..ss / 128 {
            let b = sec(s).saturating_add(e * 128);
            let Some(ent) = d.get(b..b.saturating_add(128)).filter(|e| e.len() == 128) else { break };
            let nl = (u16::from_le_bytes([ent[64], ent[65]]) as usize).min(64);
            if ent[66] != 2 || nl < 2 {
                continue;
            } // 2 = stream
            // chunks_exact: an odd (corrupt) name length used to index past the
            // end of the last 1-byte chunk and panic.
            let name: Vec<u16> = ent[..nl - 2]
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            out.push((String::from_utf16_lossy(&name), u32at(ent, 120)? as u64));
        }
        s = *fat.get(s as usize)?;
        guard += 1;
    }
    Some(out)
}

pub fn frame_count(path: &Path) -> Option<u32> {
    if path.is_dir() {
        return xfl_frames(&std::fs::read(path.join("DOMDocument.xml")).ok()?);
    }
    let d = std::fs::read(path).ok()?;
    match crate::detect(&d) {
        crate::Format::Swf => swf_frame_count(&d),
        crate::Format::XflZip => xfl_frames(&zip_member(&d, "DOMDocument.xml")?),
        crate::Format::XflXml => xfl_frames(&d),
        // .xfl marker file inside an XFL project folder
        _ => xfl_frames(&std::fs::read(path.parent()?.join("DOMDocument.xml")).ok()?),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn sample() {
        let p = std::path::Path::new("C:/Users/charl/Downloads/Flare Samples/online/fla/icons.fla");
        if p.exists() {
            assert!(super::frame_count(p).unwrap_or(0) > 0);
        }
        if let Ok(d) = std::env::var("FLARE_XFL_PROBE") {
            let r = crate::xflrender::export(
                std::path::Path::new(&d),
                &std::path::Path::new(&d).join("_svg"),
            );
            println!(
                "{:?}",
                r.map(|(n, l)| (n, l.lines().take(20).collect::<Vec<_>>().join("\n")))
            );
        }
        assert_eq!(
            super::xfl_frames(br#"<DOMLayer name="a"><DOMFrame index="3" duration="4"/>"#),
            Some(7)
        );
        assert!(super::cfbf_streams(b"not a compound document").is_none());
    }
}
