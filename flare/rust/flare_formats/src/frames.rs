//! Frame count for fla (zip) / xfl / swf files, and CFBF (legacy .fla) stream listing.
use crate::{detect, moho::zip_entry, swf_frames, xfl, Format};
use std::path::Path;

fn xfl_frames(dom: &[u8]) -> i32 {
    xfl::layers(dom).map_or(-1, |ls| {
        ls.iter().map(|l| l.frames.iter().map(|f| f.index + f.duration.max(1)).max().unwrap_or(0)).max().unwrap_or(0) as i32
    })
}

/// Frames of the main timeline; -1 unknown.
pub fn file_frames(p: &Path) -> i32 {
    if p.is_dir() || p.extension().map_or(false, |e| e.eq_ignore_ascii_case("xfl")) {
        let dir = if p.is_dir() { p } else { p.parent().unwrap_or(p) };
        return std::fs::read(dir.join("DOMDocument.xml")).map_or(-1, |d| xfl_frames(&d));
    }
    let Ok(d) = std::fs::read(p) else { return -1 };
    match detect(&d) {
        Format::Swf => match d[0] {
            b'F' => swf_frames(&d).map_or(-1, |f| f.1 as i32),
            b'C' => miniz_oxide::inflate::decompress_to_vec_zlib(&d[8..]).ok().and_then(|b| {
                let mut v = d[..8].to_vec();
                v[0] = b'F';
                v.extend(b);
                swf_frames(&v)
            }).map_or(-1, |f| f.1 as i32),
            _ => -1,
        },
        Format::XflZip => zip_entry(&d, &[b"DOMDocument.xml"]).map_or(-1, |x| xfl_frames(&x)),
        Format::XflXml => xfl_frames(&d),
        _ => -1,
    }
}

fn u32at(d: &[u8], o: usize) -> Option<u32> { Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?)) }

/// CFBF directory: (name, size) of each stream. Header DIFAT only (files < ~7MB at 512B sectors).
pub fn cfbf_streams(d: &[u8]) -> Option<Vec<(String, u64)>> {
    let ss = crate::cfbf_header(d)?.1 as usize;
    if ss < 128 { return None; }
    let sec = |n: u32| ss + n as usize * ss;
    let mut fat = Vec::new();
    for i in 0..109 {
        let s = u32at(d, 0x4C + i * 4)?;
        if s >= 0xFFFF_FFFA { break; }
        for k in 0..ss / 4 { fat.push(u32at(d, sec(s) + k * 4).unwrap_or(0xFFFF_FFFE)); }
    }
    let (mut s, mut out, mut guard) = (u32at(d, 0x30)?, Vec::new(), 0);
    while s < 0xFFFF_FFFA && guard < 100_000 {
        for e in 0..ss / 128 {
            let b = sec(s) + e * 128;
            let Some(ent) = d.get(b..b + 128) else { break };
            let nl = (u16::from_le_bytes([ent[64], ent[65]]) as usize).min(64);
            if ent[66] != 2 || nl < 2 { continue; } // 2 = stream
            let name: Vec<u16> = ent[..nl - 2].chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
            out.push((String::from_utf16_lossy(&name), u32at(ent, 120)? as u64));
        }
        s = *fat.get(s as usize)?;
        guard += 1;
    }
    Some(out)
}

#[cfg(test)]
mod t {
    #[test]
    fn samples() {
        let Ok(rd) = std::fs::read_dir(r"C:\Users\charl\Downloads\Flare Samples\online\fla") else { return };
        for e in rd.flatten() {
            let p = e.path();
            let n = super::file_frames(&p);
            let c = super::cfbf_streams(&std::fs::read(&p).unwrap()).map(|v| v.len());
            println!("{} frames={} cfbf_streams={:?}", p.display(), n, c);
            assert!(n > 0 || c.map_or(false, |k| k > 0), "{}", p.display());
        }
    }
}
