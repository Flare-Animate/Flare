//! Flare format sniffing (SWF/CFBF/XFL). SWF header layout per the SWF spec, as in ruffle (MIT/Apache-2.0).
//!
//! Crate root only: every module lives beside the C++ subsystem it ports
//! (`common/<subsystem>/rust/`), so the tree keeps matching OpenToonz/Tahoma.
use std::slice;

#[path = "common/flash/cfbf.rs"]
pub mod cfbf;
#[path = "common/flash/fcp.rs"]
pub mod fcp;
#[path = "sound/tsioutils.rs"]
pub mod tsioutils;
#[path = "tnzbase/permissionsmanager.rs"]
pub mod permissionsmanager;
#[path = "common/moho/moho.rs"]
pub mod moho;
#[path = "common/flash/swf.rs"]
pub mod swf;
#[path = "common/flash/xfl.rs"]
pub mod xfl;

/// Parse FlipaClip .fcp; JSON (free with flare_fcp_free) or NULL.
#[no_mangle]
pub unsafe extern "C" fn flare_fcp_parse(data: *const u8, len: usize) -> *mut std::os::raw::c_char {
    match fcp::parse(bytes(data, len))
        .ok()
        .and_then(|s| std::ffi::CString::new(s).ok())
    {
        Some(c) => c.into_raw(),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn flare_fcp_free(p: *mut std::os::raw::c_char) {
    flare_moho_free(p)
}
#[path = "common/flash/framecount.rs"]
pub mod framecount;
#[path = "common/flash/xflrender.rs"]
pub mod xflrender;

/// Frame count of a SWF / FLA / XFL file or folder; -1 if unknown.
#[no_mangle]
pub unsafe extern "C" fn flare_flash_frame_count(path: *const std::os::raw::c_char) -> i32 {
    if path.is_null() {
        return -1;
    }
    let p = std::ffi::CStr::from_ptr(path)
        .to_string_lossy()
        .into_owned();
    framecount::frame_count(std::path::Path::new(&p)).map_or(-1, |n| n as i32)
}

/// CFBF (legacy FLA) streams as "name\tsize\n" lines; NULL if not CFBF.
/// Free with flare_moho_free. Used by the legacy-FLA import log.
#[no_mangle]
pub unsafe extern "C" fn flare_cfbf_streams(
    data: *const u8,
    len: usize,
) -> *mut std::os::raw::c_char {
    let Some(v) = framecount::cfbf_streams(bytes(data, len)) else {
        return std::ptr::null_mut();
    };
    let s: String = v
        .iter()
        .map(|(n, z)| format!("{}\t{}\n", n.replace('\0', ""), z))
        .collect();
    std::ffi::CString::new(s).map_or(std::ptr::null_mut(), |c| c.into_raw())
}
#[path = "common/flash/symlib.rs"]
pub mod symlib;
#[path = "common/tapptools/tenv.rs"]
pub mod tenv;
#[path = "common/tsystem/tfilepath.rs"]
pub mod tfilepath;
#[path = "common/tcore/tmathutil.rs"]
pub mod tmathutil;
#[path = "common/tparam/tcubicbezier.rs"]
pub mod tcubicbezier;
#[path = "common/tcore/trandom.rs"]
pub mod trandom;
#[path = "common/tcolor/tcolorvalue.rs"]
pub mod tcolorvalue;
#[path = "common/trop/runsmap.rs"]
pub mod runsmap;
#[path = "stdfx/igs_color_rgb_hls.rs"]
pub mod igs_color_rgb_hls;
#[path = "stdfx/igs_color_rgb_hsv.rs"]
pub mod igs_color_rgb_hsv;
#[path = "stdfx/igs_math_random.rs"]
pub mod igs_math_random;
#[path = "stdfx/igs_gauss_distribution.rs"]
pub mod igs_gauss_distribution;
#[path = "stdfx/igs_negate.rs"]
pub mod igs_negate;
#[path = "stdfx/igs_density.rs"]
pub mod igs_density;
#[path = "stdfx/igs_warp_hori.rs"]
pub mod igs_warp_hori;
#[path = "stdfx/igs_warp_vert.rs"]
pub mod igs_warp_vert;
#[path = "stdfx/igs_hls_adjust.rs"]
pub mod igs_hls_adjust;
#[path = "stdfx/igs_hsv_adjust.rs"]
pub mod igs_hsv_adjust;
#[path = "stdfx/igs_hls_add.rs"]
pub mod igs_hls_add;
#[path = "stdfx/igs_hsv_add.rs"]
pub mod igs_hsv_add;
#[path = "stdfx/igs_level_auto.rs"]
pub mod igs_level_auto;
#[path = "stdfx/igs_level_auto_in_camera.rs"]
pub mod igs_level_auto_in_camera;
#[path = "stdfx/igs_levels.rs"]
pub mod igs_levels;
#[path = "stdfx/igs_median_filter.rs"]
pub mod igs_median_filter;
#[path = "stdfx/igs_median_filter_smooth.rs"]
pub mod igs_median_filter_smooth;
#[path = "stdfx/igs_hsv_noise.rs"]
pub mod igs_hsv_noise;
#[path = "stdfx/igs_hls_noise.rs"]
pub mod igs_hls_noise;

/// TFilePath query (see header). Returns full byte length; copies up to `cap` into `out`.
#[no_mangle]
pub unsafe extern "C" fn flare_tfilepath_query(
    path: *const std::os::raw::c_char,
    kind: i32,
    out: *mut u8,
    cap: usize,
) -> usize {
    if path.is_null() {
        return 0;
    }
    let p = std::ffi::CStr::from_ptr(path).to_string_lossy();
    let s = match kind {
        0 => tfilepath::get_type(&p),
        1 => tfilepath::get_name(&p),
        2 => tfilepath::get_level_name(&p),
        3 => tfilepath::get_parent_dir(&p),
        4 => tfilepath::get_dots(&p),
        5 => tfilepath::get_frame(&p).to_string(),
        _ => return 0,
    };
    if !out.is_null() {
        std::ptr::copy_nonoverlapping(s.as_ptr(), out, s.len().min(cap));
    }
    s.len()
}

/// Symbol library: add TSV `line` to TSV `lib`, write normalized TSV into `out` (up to `cap`). Returns full length.
#[no_mangle]
pub unsafe extern "C" fn flare_symlib_add(
    lib: *const u8,
    lib_len: usize,
    line: *const u8,
    line_len: usize,
    out: *mut u8,
    cap: usize,
) -> usize {
    let s = symlib::add(
        &String::from_utf8_lossy(bytes(lib, lib_len)),
        &String::from_utf8_lossy(bytes(line, line_len)),
    );
    if !out.is_null() {
        std::ptr::copy_nonoverlapping(s.as_ptr(), out, s.len().min(cap));
    }
    s.len()
}

/// Count of SWF tags (incl. End); -1 if not uncompressed SWF.
#[no_mangle]
pub unsafe extern "C" fn flare_swf_tag_count(data: *const u8, len: usize) -> i32 {
    swf::tags(bytes(data, len)).map_or(-1, |t| t.len() as i32)
}

/// Write up to `max` tag codes into `codes`; returns total count or -1.
#[no_mangle]
pub unsafe extern "C" fn flare_swf_tag_codes(
    data: *const u8,
    len: usize,
    codes: *mut u16,
    max: usize,
) -> i32 {
    match swf::tags(bytes(data, len)) {
        Some(t) => {
            if !codes.is_null() {
                for (i, g) in t.iter().take(max).enumerate() {
                    *codes.add(i) = g.code;
                }
            }
            t.len() as i32
        }
        None => -1,
    }
}

/// Layer count of first XFL timeline; writes total frame count to `frames`. -1 on error.
#[no_mangle]
pub unsafe extern "C" fn flare_xfl_layer_count(
    data: *const u8,
    len: usize,
    frames: *mut u32,
) -> i32 {
    match xfl::layers(bytes(data, len)) {
        Some(l) => {
            if !frames.is_null() {
                *frames = l.iter().map(|x| x.frames.len() as u32).sum();
            }
            l.len() as i32
        }
        None => -1,
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(i32)]
pub enum Format {
    Unknown = 0,
    Swf = 1,
    Cfbf = 2,
    XflZip = 3,
    XflXml = 4,
}

const CFBF_SIG: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

pub fn detect(d: &[u8]) -> Format {
    if swf_header(d).is_some() {
        return Format::Swf;
    }
    if d.starts_with(&CFBF_SIG) {
        return Format::Cfbf;
    }
    if d.starts_with(b"PK\x03\x04") {
        return if d.windows(15).any(|w| w == b"DOMDocument.xml") {
            Format::XflZip
        } else {
            Format::Unknown
        };
    }
    let head = &d[..d.len().min(512)];
    if head.windows(12).any(|w| w == b"<DOMDocument") {
        return Format::XflXml;
    }
    Format::Unknown
}

pub fn swf_header(d: &[u8]) -> Option<(u8, u8, u32)> {
    if d.len() < 8 || &d[1..3] != b"WS" || !matches!(d[0], b'F' | b'C' | b'Z') {
        return None;
    }
    Some((d[0], d[3], u32::from_le_bytes([d[4], d[5], d[6], d[7]])))
}

unsafe fn bytes<'a>(p: *const u8, n: usize) -> &'a [u8] {
    if p.is_null() {
        &[]
    } else {
        slice::from_raw_parts(p, n)
    }
}

#[no_mangle]
pub unsafe extern "C" fn flare_detect_format(data: *const u8, len: usize) -> i32 {
    detect(bytes(data, len)) as i32
}

#[no_mangle]
pub unsafe extern "C" fn flare_swf_header(
    data: *const u8,
    len: usize,
    c: *mut u8,
    v: *mut u8,
    l: *mut u32,
) -> i32 {
    match swf_header(bytes(data, len)) {
        Some((a, b, n)) => {
            if !c.is_null() {
                *c = a;
            }
            if !v.is_null() {
                *v = b;
            }
            if !l.is_null() {
                *l = n;
            }
            0
        }
        None => -1,
    }
}

/// Uncompressed SWF: (frame_rate_8.8, frame_count) after the RECT.
pub fn swf_frames(d: &[u8]) -> Option<(u16, u16)> {
    if swf_header(d)?.0 != b'F' || d.len() < 9 {
        return None;
    }
    let nbits = (d[8] >> 3) as usize;
    let off = 8 + (5 + 4 * nbits + 7) / 8;
    if d.len() < off + 4 {
        return None;
    }
    Some((
        u16::from_le_bytes([d[off], d[off + 1]]),
        u16::from_le_bytes([d[off + 2], d[off + 3]]),
    ))
}

/// CFBF header: (major_version, sector_size).
pub fn cfbf_header(d: &[u8]) -> Option<(u16, u32)> {
    if d.len() < 0x20 || !d.starts_with(&CFBF_SIG) {
        return None;
    }
    let shift = u16::from_le_bytes([d[0x1E], d[0x1F]]);
    if shift > 16 {
        return None;
    }
    Some((u16::from_le_bytes([d[0x1A], d[0x1B]]), 1u32 << shift))
}

fn attr(tag: &str, name: &str) -> Option<f64> {
    let k = format!(" {}=\"", name);
    let i = tag.find(&k)? + k.len();
    tag[i..].split('"').next()?.parse().ok()
}

/// DOMDocument stage info: (width, height, frameRate) with XFL defaults 550x400@24.
pub fn xfl_dom_info(d: &[u8]) -> Option<(f64, f64, f64)> {
    let s = std::str::from_utf8(d).ok()?;
    let i = s.find("<DOMDocument")?;
    let tag = &s[i..i + s[i..].find('>')?];
    Some((
        attr(tag, "width").unwrap_or(550.0),
        attr(tag, "height").unwrap_or(400.0),
        attr(tag, "frameRate").unwrap_or(24.0),
    ))
}

#[no_mangle]
pub unsafe extern "C" fn flare_xfl_dom_info(
    data: *const u8,
    len: usize,
    w: *mut f64,
    h: *mut f64,
    fps: *mut f64,
) -> i32 {
    match xfl_dom_info(bytes(data, len)) {
        Some((a, b, c)) => {
            if !w.is_null() {
                *w = a;
            }
            if !h.is_null() {
                *h = b;
            }
            if !fps.is_null() {
                *fps = c;
            }
            0
        }
        None => -1,
    }
}

/// Parse a Moho project; returns malloc'd-by-Rust NUL-terminated JSON (free with flare_moho_free), or NULL on error.
#[no_mangle]
pub unsafe extern "C" fn flare_moho_parse(
    data: *const u8,
    len: usize,
) -> *mut std::os::raw::c_char {
    match moho::parse(bytes(data, len))
        .ok()
        .and_then(|s| std::ffi::CString::new(s).ok())
    {
        Some(c) => c.into_raw(),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn flare_moho_free(p: *mut std::os::raw::c_char) {
    if !p.is_null() {
        drop(std::ffi::CString::from_raw(p));
    }
}

/// Render XFL dir (DOMDocument.xml + LIBRARY) to per-frame SVGs + frames.tsv in out_dir.
/// Returns log text (first line "ok <n>" or "error <msg>"); free with flare_moho_free.
#[no_mangle]
pub unsafe extern "C" fn flare_xfl_export_svg(
    xfl_dir: *const std::os::raw::c_char,
    out_dir: *const std::os::raw::c_char,
) -> *mut std::os::raw::c_char {
    use std::ffi::{CStr, CString};
    if xfl_dir.is_null() || out_dir.is_null() {
        return std::ptr::null_mut();
    }
    let d = CStr::from_ptr(xfl_dir).to_string_lossy().into_owned();
    let o = CStr::from_ptr(out_dir).to_string_lossy().into_owned();
    let s = match xflrender::export(std::path::Path::new(&d), std::path::Path::new(&o)) {
        Ok((n, log)) => format!("ok {n}\n{log}"),
        Err(e) => format!("error {e}\n"),
    };
    CString::new(s.replace('\0', " ")).map_or(std::ptr::null_mut(), |c| c.into_raw())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn swf() {
        let d = [b'F', b'W', b'S', 10, 0x20, 0, 0, 0];
        assert_eq!(detect(&d), Format::Swf);
        assert_eq!(swf_header(&d), Some((b'F', 10, 32)));
        assert_eq!(detect(b"CWS\x09\0\0\0\0"), Format::Swf);
    }
    #[test]
    fn cfbf() {
        assert_eq!(detect(&CFBF_SIG), Format::Cfbf);
    }
    #[test]
    fn xfl() {
        assert_eq!(detect(b"PK\x03\x04....DOMDocument.xml"), Format::XflZip);
        assert_eq!(
            detect(b"<?xml version=\"1.0\"?><DOMDocument>"),
            Format::XflXml
        );
        assert_eq!(detect(b"PK\x03\x04nothing"), Format::Unknown);
    }
    #[test]
    fn parse() {
        let d = [b'F', b'W', b'S', 10, 0, 0, 0, 0, 0x08, 0, 0x00, 0x18, 5, 0];
        assert_eq!(swf_frames(&d), Some((0x1800, 5)));
        let mut c = [0u8; 0x20];
        c[..8].copy_from_slice(&CFBF_SIG);
        c[0x1A] = 3;
        c[0x1E] = 9;
        assert_eq!(cfbf_header(&c), Some((3, 512)));
        assert_eq!(
            xfl_dom_info(b"<DOMDocument xmlns=\"x\" width=\"1280\" frameRate=\"30\">"),
            Some((1280.0, 400.0, 30.0))
        );
    }
    #[test]
    fn ffi_null() {
        unsafe {
            assert_eq!(flare_detect_format(std::ptr::null(), 0), 0);
        }
    }
    #[test]
    fn ffi_symlib_add() {
        let line = b"mc\tf\tSym1\tlvl";
        let n = unsafe {
            flare_symlib_add(
                line.as_ptr(),
                0,
                line.as_ptr(),
                line.len(),
                std::ptr::null_mut(),
                0,
            )
        };
        assert!(n > 0);
        let mut buf = vec![0u8; n];
        assert_eq!(
            unsafe {
                flare_symlib_add(
                    line.as_ptr(),
                    0,
                    line.as_ptr(),
                    line.len(),
                    buf.as_mut_ptr(),
                    n,
                )
            },
            n
        );
        assert!(String::from_utf8_lossy(&buf).contains("Sym1"));
    }
}

/// TEnv path-set split (see header). Entries joined by newline; returns full byte length, copies up to `cap`.
#[no_mangle]
pub unsafe extern "C" fn flare_tenv_path_set(
    value: *const std::os::raw::c_char,
    out: *mut u8,
    cap: usize,
) -> usize {
    if value.is_null() {
        return 0;
    }
    let v = std::ffi::CStr::from_ptr(value).to_string_lossy();
    let s = tenv::path_set(&v).join(
        "
",
    );
    let n = s.len().min(cap);
    if !out.is_null() && n > 0 {
        std::ptr::copy_nonoverlapping(s.as_ptr(), out, n);
    }
    s.len()
}
