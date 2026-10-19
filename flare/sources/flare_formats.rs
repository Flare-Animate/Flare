//! Flare format sniffing (SWF/CFBF/XFL). SWF header layout per the SWF spec, as in ruffle (MIT/Apache-2.0).
//!
//! Crate root only: every module lives beside the C++ subsystem it ports
//! (`common/<subsystem>/rust/`), so the tree keeps matching OpenToonz/Tahoma.
use std::slice;

#[path = "common/flash/cfbf.rs"]
pub mod cfbf;
#[path = "common/flash/fcp.rs"]
pub mod fcp;
#[path = "common/flash/lso.rs"]
pub mod lso;
#[path = "sound/tsioutils.rs"]
pub mod tsioutils;
#[path = "tnzbase/permissionsmanager.rs"]
pub mod permissionsmanager;
#[path = "common/moho/moho.rs"]
pub mod moho;
#[path = "common/flash/swf.rs"]
pub mod swf;
#[path = "common/flash/FDT.rs"]
pub mod fdt;
#[path = "common/flash/xfl.rs"]
pub mod xfl;

/// Run `f`, returning `default` if it panics. A Rust panic unwinding out of an
/// `extern "C"` function aborts the whole process (SIGABRT / "C++ EH exception"
/// on the C++ side), so every FFI entry point that parses untrusted files goes
/// through this (issues #110/#117).
fn guard<T>(default: T, f: impl FnOnce() -> T) -> T {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or(default)
}

/// Parse FlipaClip .fcp; JSON (free with flare_fcp_free) or NULL.
#[no_mangle]
pub unsafe extern "C" fn flare_fcp_parse(data: *const u8, len: usize) -> *mut std::os::raw::c_char {
    guard(std::ptr::null_mut(), || match fcp::parse(bytes(data, len))
        .ok()
        .and_then(|s| std::ffi::CString::new(s).ok())
    {
        Some(c) => c.into_raw(),
        None => std::ptr::null_mut(),
    })
}

#[no_mangle]
pub unsafe extern "C" fn flare_fcp_free(p: *mut std::os::raw::c_char) {
    flare_moho_free(p)
}

/// 1 if `data` is a Flash Local Shared Object (`.sol`), 0 otherwise.
#[no_mangle]
pub unsafe extern "C" fn flare_lso_is_lso(data: *const u8, len: usize) -> i32 {
    guard(0, || i32::from(lso::is_lso(bytes(data, len))))
}

/// Parse a `.sol` file to normalised JSON (`{name, formatVersion, body}`).
/// Returns a Rust-owned NUL-terminated string (free with `flare_lso_free`) or
/// NULL if the bytes are not a decodable AMF0 LSO.
#[no_mangle]
pub unsafe extern "C" fn flare_lso_parse(data: *const u8, len: usize) -> *mut std::os::raw::c_char {
    guard(std::ptr::null_mut(), || {
        match lso::parse(bytes(data, len))
            .ok()
            .and_then(|l| std::ffi::CString::new(l.to_json().to_string()).ok())
        {
            Some(c) => c.into_raw(),
            None => std::ptr::null_mut(),
        }
    })
}

/// Free a string returned by [`flare_lso_parse`].
#[no_mangle]
pub unsafe extern "C" fn flare_lso_free(p: *mut std::os::raw::c_char) {
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
    guard(-1, || framecount::frame_count(std::path::Path::new(&p)).map_or(-1, |n| n.min(i32::MAX as u32) as i32))
}

/// CFBF (legacy FLA) streams as "name\tsize\n" lines; NULL if not CFBF.
/// Free with flare_moho_free. Used by the legacy-FLA import log.
#[no_mangle]
pub unsafe extern "C" fn flare_cfbf_streams(
    data: *const u8,
    len: usize,
) -> *mut std::os::raw::c_char {
    guard(std::ptr::null_mut(), || {
        let Some(v) = framecount::cfbf_streams(bytes(data, len)) else {
            return std::ptr::null_mut();
        };
        let s: String = v
            .iter()
            .map(|(n, z)| format!("{}\t{}\n", n.replace('\0', ""), z))
            .collect();
        std::ffi::CString::new(s).map_or(std::ptr::null_mut(), |c| c.into_raw())
    })
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
#[path = "common/trop/rust/invert.rs"]
pub mod trop_invert;
#[path = "common/trop/rust/checkboard.rs"]
pub mod trop_checkboard;
#[path = "common/trop/rust/bbox.rs"]
pub mod trop_bbox;
#[path = "common/tvectorimage/cornerdetector.rs"]
pub mod cornerdetector;
#[path = "tnzext/Types.rs"]
pub mod ext_types;
#[path = "flarefarm/tfarmclient/casmfileinfo.rs"]
pub mod casmfileinfo;
#[path = "common/psdlib/psdutils.rs"]
pub mod psdutils;
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
#[path = "stdfx/hsvutil.rs"]
pub mod hsvutil;
#[path = "stdfx/igs_perlin_noise.rs"]
pub mod igs_perlin_noise;
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
#[path = "flare/metnum.rs"]
pub mod metnum;
#[path = "flare/predict3d.rs"]
pub mod predict3d;
#[path = "flarelib/sandor_fxs/CIL.rs"]
pub mod cil;
#[path = "flarelib/sandor_fxs/CallCircle.rs"]
pub mod callcircle;
#[path = "flarelib/sandor_fxs/BlurMatrix.rs"]
pub mod blurmatrix;
#[path = "flarelib/sandor_fxs/PatternPosition.rs"]
pub mod patternposition;
#[path = "flarelib/sandor_fxs/SDirection.rs"]
pub mod sdirection;
#[path = "flarelib/sandor_fxs/PatternMapParam.rs"]
pub mod patternmapparam;
#[path = "stdfx/igs_hls_noise.rs"]
pub mod igs_hls_noise;
#[path = "stdfx/naru_maxflow.rs"]
pub mod naru_maxflow;
#[path = "stdfx/igs_color_blend.rs"]
pub mod igs_color_blend;
#[path = "stdfx/igs_motion_wind_table.rs"]
pub mod igs_motion_wind_table;
#[path = "stdfx/igs_attenuation_distribution.rs"]
pub mod igs_attenuation_distribution;
#[path = "stdfx/igs_motion_wind_pixel.rs"]
pub mod igs_motion_wind_pixel;
#[path = "flarelib/skeletonlut.rs"]
pub mod skeletonlut;
#[path = "flarelib/iknode.rs"]
pub mod iknode;

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
    guard(-1, || swf::tags(bytes(data, len)).map_or(-1, |t| t.len() as i32))
}

/// Write up to `max` tag codes into `codes`; returns total count or -1.
#[no_mangle]
pub unsafe extern "C" fn flare_swf_tag_codes(
    data: *const u8,
    len: usize,
    codes: *mut u16,
    max: usize,
) -> i32 {
    guard(-1, || match swf::tags(bytes(data, len)) {
        Some(t) => {
            if !codes.is_null() {
                for (i, g) in t.iter().take(max).enumerate() {
                    *codes.add(i) = g.code;
                }
            }
            t.len() as i32
        }
        None => -1,
    })
}

/// Layer count of first XFL timeline; writes total frame count to `frames`. -1 on error.
#[no_mangle]
pub unsafe extern "C" fn flare_xfl_layer_count(
    data: *const u8,
    len: usize,
    frames: *mut u32,
) -> i32 {
    guard(-1, || match xfl::layers(bytes(data, len)) {
        Some(l) => {
            if !frames.is_null() {
                *frames = l.iter().map(|x| x.frames.len() as u32).fold(0u32, u32::saturating_add);
            }
            l.len() as i32
        }
        None => -1,
    })
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(i32)]
pub enum Format {
    Unknown = 0,
    Swf = 1,
    Cfbf = 2,
    XflZip = 3,
    XflXml = 4,
    Sol = 5,
    // Project containers and scenes the rewrite is expected to open. Values are frozen:
    // the C ABI returns them, so append, never renumber.
    Xdts = 6,    // OpenToonz scene description (XML)
    Xstage = 7,  // Toon Boom Harmony stage (XML)
    Tlv = 8,     // OpenToonz / Tahoma project level
    Pli = 9,     // OpenToonz / Tahoma palette
    Tzp = 10,    // zipped OpenToonz project (.tnz/.tzp)
    Tnz = 11,    // OpenToonz project directory (.tnz/.tzu)
    Ppm = 12,    // Flipnote packed scene (inside .kwz)
    Kwz = 13,    // zipped Flipnote studio
    Wick = 14,   // Wick Editor project (JSON)
    Fc = 15,     // FlippaClip project (zip)
    MohoZip = 16, // zipped Moho project
    // Moho binary and its gzip form are both CFBF-free plain data; `Anime` is the
    // `.anime`/`.anme` spelling OpenToonz/Tahoma use for a gzipped `.moho`.
    Moho = 17,
    Anime = 18,
    Tahoma = 19, // Tahoma2D `.tahoma` (a `.tnz`-shaped folder or zip)
    // Palette/template spellings shared by more than one host (`.plt` OpenToonz,
    // `.tpl` OpenToonz *and* Toon Boom Harmony). Content cannot tell them apart, so they
    // are only reachable through the extension-aware entry point.
    Plt = 20,
    Tpl = 21,
}

const CFBF_SIG: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

/// First entry name of a zip, from the local file header at offset 0 (name length is
/// bytes 26..28, name starts at 30). `""` when the buffer is not a zip or is truncated.
fn zip_first_entry(d: &[u8]) -> &[u8] {
    if !d.starts_with(b"PK\x03\x04") || d.len() < 30 {
        return &[];
    }
    let n = u16::from_le_bytes([d[26], d[27]]) as usize;
    let end = (30 + n).min(d.len()).max(30);
    &d[30..end]
}

fn is_gzip(d: &[u8]) -> bool {
    d.starts_with(&[0x1F, 0x8B])
}

/// Text sniffing: the first 512 bytes contain `needle`.
fn head_contains(d: &[u8], needle: &[u8]) -> bool {
    let head = &d[..d.len().min(512)];
    head.windows(needle.len()).any(|w| w == needle)
}

/// Format implied by a file extension, lower-cased and without the dot. This is the only
/// way to separate spellings that share a container (`.tnz` folder vs `.tnz` zip, `.pli`
/// vs `.plt`, `.tpl` as OpenToonz template vs Harmony template), and the only way to name
/// the folder-shaped formats at all -- a directory has no bytes to sniff.
pub fn detect_ext(ext: &str) -> Format {
    match ext.to_ascii_lowercase().as_str() {
        "swf" => Format::Swf,
        "fla" => Format::Cfbf,
        "xfl" => Format::XflZip,
        "sol" => Format::Sol,
        "xdts" => Format::Xdts,
        "xstage" => Format::Xstage,
        "tlv" => Format::Tlv,
        "pli" => Format::Pli,
        "plt" => Format::Plt,
        "tpl" => Format::Tpl,
        "tzp" => Format::Tzp,
        "tnz" | "tzu" => Format::Tnz,
        "tahoma" => Format::Tahoma,
        "ppm" => Format::Ppm,
        "kwz" => Format::Kwz,
        "wick" => Format::Wick,
        "fc" => Format::Fc,
        "moho" => Format::Moho,
        "anime" | "anme" => Format::Anime,
        _ => Format::Unknown,
    }
}

pub fn detect(d: &[u8]) -> Format {
    if swf_header(d).is_some() {
        return Format::Swf;
    }
    if d.starts_with(&CFBF_SIG) {
        return Format::Cfbf;
    }
    if lso::is_lso(d) {
        return Format::Sol;
    }
    if d.starts_with(b"PK\x03\x04") {
        let first = zip_first_entry(d);
        if head_contains(&first, b"DOMDocument.xml") || head_contains(d, b"DOMDocument.xml") {
            return Format::XflZip;
        }
        if head_contains(&first, b".ppm") || head_contains(&first, b".pps") {
            return Format::Kwz;
        }
        if head_contains(&first, b".moho") || head_contains(&first, b".moh") {
            return Format::MohoZip;
        }
        // A zip with no recognisable first entry is still a project archive far more
        // often than not: `.tzp` (OpenToonz), `.fc` (FlippaClip), `.kwz`. Callers that
        // know the extension get the precise answer from `detect_ext`.
        return Format::Tzp;
    }
    if is_gzip(d) {
        // `.anime` / `.anme` is a gzipped Moho project.
        return Format::Anime;
    }
    let head = &d[..d.len().min(512)];
    if head.windows(12).any(|w| w == b"<DOMDocument") {
        return Format::XflXml;
    }
    if head_contains(d, b"<xstage") {
        return Format::Xstage;
    }
    if head_contains(d, b"<sceneroot") || head_contains(d, b"<scene") && head_contains(d, b"cell") {
        return Format::Xdts;
    }
    if head_contains(d, b"\"wickEditor\"") || head_contains(d, b"\"wickeditor\"") {
        return Format::Wick;
    }
    Format::Unknown
}

/// Content sniffing, refined by the file extension when the bytes are ambiguous or the
/// format has no content magic of its own (folders, `.plt`/`.tpl`).
pub fn detect_with_ext(d: &[u8], ext: &str) -> Format {
    let by_ext = detect_ext(ext);
    if by_ext == Format::Unknown {
        return detect(d);
    }
    // Zip-based containers: the extension decides which archive this is.
    if matches!(by_ext, Format::XflZip | Format::Tzp | Format::Kwz | Format::Fc | Format::MohoZip) {
        if d.starts_with(b"PK\x03\x04") || d.is_empty() {
            return by_ext;
        }
    }
    by_ext
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
    guard(0, || detect(bytes(data, len)) as i32)
}

/// `flare_detect_format` with a file extension (no dot, any case) as a tie-breaker, for
/// the formats whose bytes cannot identify them: project folders (`.tnz`, `.tzu`,
/// `.tahoma`), the two palettes (`.pli`, `.plt`) and the two templates (`.tpl`). `ext`
/// may be NULL.
#[no_mangle]
pub unsafe extern "C" fn flare_detect_format_ext(
    data: *const u8,
    len: usize,
    ext: *const std::os::raw::c_char,
) -> i32 {
    guard(0, || {
        let ext = if ext.is_null() { "" } else { std::ffi::CStr::from_ptr(ext).to_str().unwrap_or("") };
        detect_with_ext(bytes(data, len), ext) as i32
    })
}

#[no_mangle]
pub unsafe extern "C" fn flare_swf_header(
    data: *const u8,
    len: usize,
    c: *mut u8,
    v: *mut u8,
    l: *mut u32,
) -> i32 {
    guard(-1, || match swf_header(bytes(data, len)) {
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
    })
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
    guard(-1, || match xfl_dom_info(bytes(data, len)) {
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
    })
}

/// Parse a Moho project; returns malloc'd-by-Rust NUL-terminated JSON (free with flare_moho_free), or NULL on error.
#[no_mangle]
pub unsafe extern "C" fn flare_moho_parse(
    data: *const u8,
    len: usize,
) -> *mut std::os::raw::c_char {
    guard(std::ptr::null_mut(), || match moho::parse(bytes(data, len))
        .ok()
        .and_then(|s| std::ffi::CString::new(s).ok())
    {
        Some(c) => c.into_raw(),
        None => std::ptr::null_mut(),
    })
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
    let s = guard("error import panicked (corrupt document?)\n".to_string(), || {
        match xflrender::export(std::path::Path::new(&d), std::path::Path::new(&o)) {
            Ok((n, log)) => format!("ok {n}\n{log}"),
            Err(e) => format!("error {e}\n"),
        }
    });
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
    fn sol() {
        let mut d = vec![0x00, 0xbf, 0, 0, 0, 0];
        d.extend_from_slice(&lso::HEADER_SIGNATURE);
        d.extend_from_slice(&[0, 2, b'S', b'O', 0, 0, 0, 0x00]);
        assert_eq!(detect(&d), Format::Sol);
        // An unnamed zip is a project archive, not "unknown".
        assert_eq!(detect(b"PK\x03\x04nothing"), Format::Tzp);
    }
    #[test]
    fn xfl() {
        assert_eq!(detect(b"PK\x03\x04....DOMDocument.xml"), Format::XflZip);
        assert_eq!(
            detect(b"<?xml version=\"1.0\"?><DOMDocument>"),
            Format::XflXml
        );
        assert_eq!(detect(b"PK\x03\x04nothing"), Format::Tzp);
    }
    /// Every project/container type the rewrite is expected to open. Content rules are
    /// used where the bytes are unambiguous; the rest go through the extension.
    #[test]
    fn project_file_types() {
        // --- content-certain ---
        assert_eq!(detect(b"\x1F\x8B\x08\x00rubbish"), Format::Anime);
        assert_eq!(
            detect(b"<?xml version=\"1.0\"?><xstage version=\"2\"/>"),
            Format::Xstage
        );
        assert_eq!(
            detect(b"<?xml version=\"1.0\"?><sceneroot><cell/></sceneroot>"),
            Format::Xdts
        );
        assert_eq!(
            detect(br#"{"wickEditor":"wickEditor.fileformat","frames":[]}"#),
            Format::Wick
        );
        // zip first entry decides: .ppm/.pps is Flipnote, .moho is Moho
        assert_eq!(detect(&zip_with_name(b"movie.ppm")), Format::Kwz);
        assert_eq!(detect(&zip_with_name(b"doc.moho")), Format::MohoZip);
        assert_eq!(detect(&zip_with_name(b"DOMDocument.xml")), Format::XflZip);

        // --- extension-only (no content magic: folders, shared spellings) ---
        assert_eq!(detect_ext("tlv"), Format::Tlv);
        assert_eq!(detect_ext("pli"), Format::Pli);
        assert_eq!(detect_ext("plt"), Format::Plt);
        assert_eq!(detect_ext("tpl"), Format::Tpl);
        assert_eq!(detect_ext("tnz"), Format::Tnz);
        assert_eq!(detect_ext("tzu"), Format::Tnz);
        assert_eq!(detect_ext("tzp"), Format::Tzp);
        assert_eq!(detect_ext("ppm"), Format::Ppm);
        assert_eq!(detect_ext("kwz"), Format::Kwz);
        assert_eq!(detect_ext("wick"), Format::Wick);
        assert_eq!(detect_ext("fc"), Format::Fc);
        assert_eq!(detect_ext("moho"), Format::Moho);
        assert_eq!(detect_ext("anime"), Format::Anime);
        assert_eq!(detect_ext("anme"), Format::Anime);
        assert_eq!(detect_ext("tahoma"), Format::Tahoma);
        assert_eq!(detect_ext("xstage"), Format::Xstage);
        assert_eq!(detect_ext("xdts"), Format::Xdts);
        // case-insensitive, and unknown extensions fall back to content
        assert_eq!(detect_ext("TLV"), Format::Tlv);
        assert_eq!(detect_ext("bin"), Format::Unknown);
        assert_eq!(detect_with_ext(b"random", "tnz"), Format::Tnz);
        // an unrecognised extension still gets content sniffing
        assert_eq!(
            detect_with_ext(b"FWS\x09\0\0\0\0", "bin"),
            Format::Swf
        );
        // ... and a zip extension beats the generic "it's a zip" answer
        assert_eq!(detect_with_ext(&zip_with_name(b"movie.ppm"), "kwz"), Format::Kwz);
        assert_eq!(detect_with_ext(&zip_with_name(b"project/root.tpl"), "tzp"), Format::Tzp);
    }
    /// A minimal zip whose first entry is `name`, enough for `zip_first_entry`.
    /// Layout: 4-byte signature, 22 bytes of header (through the 4-byte CRC at 14..18 and
    /// the two size fields at 18..26), then the name length at 26..28, extra length at
    /// 28..30, then the name.
    fn zip_with_name(name: &[u8]) -> Vec<u8> {
        let mut v = b"PK\x03\x04".to_vec();
        v.extend_from_slice(&[0u8; 22]);
        v.extend_from_slice(&(name.len() as u16).to_le_bytes());
        v.extend_from_slice(&[0, 0]); // extra length
        v.extend_from_slice(name);
        v
    }
    /// The `#[path]` registry must match the tree. A module that is registered but whose
    /// file moved compiles nowhere; a file that sits beside a deleted `.cpp` but is *not*
    /// registered still compiles and ships as dead code (see docs/PORTING.md), which is
    /// the failure mode nobody notices. Both are cheap to catch here.
    #[test]
    fn module_registry_matches_the_tree() {
        // Modules that deliberately keep both a `.rs` and a `.cpp`: the memory-safety
        // wrapper scripts, which the port policy exempts from deletion.
        const KEEPS_CPP: &[&str] = &[
                        "common/tapptools/tenv.rs",

                        "common/tsystem/tfilepath.rs",
        ];

        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let src = include_str!("flare_formats.rs");

        let mut registered = Vec::new();
        let mut orphan_paths = Vec::new();
        for (i, line) in src.lines().enumerate() {
            let Some(rest) = line.trim().strip_prefix("#[path = \"") else {
                continue;
            };
            let Some(quote) = rest.find('"') else {
                panic!("malformed #[path] on line {}", i + 1);
            };
            let path = &rest[..quote];

            // The attribute must actually declare a module on the next line.
            let next = src.lines().skip(i + 1).find(|l| !l.trim().is_empty());
            assert!(
                next.is_some_and(|l| l.contains("mod ")),
                "{path}: #[path] on line {} is not followed by a `mod` declaration",
                i + 1
            );

            assert!(root.join(path).is_file(), "{path}: registered but not on disk");
            let cpp = path.strip_suffix(".rs").map(|p| format!("{p}.cpp"));
            if let Some(cpp) = cpp {
                if root.join(&cpp).is_file() && !KEEPS_CPP.contains(&path) {
                    orphan_paths.push(path.to_string());
                }
            }
            registered.push(path.to_string());
        }

        assert!(registered.len() >= 40, "registry shrank to {}", registered.len());
        assert!(
            orphan_paths.is_empty(),
            "ported C++ still present next to the Rust module: {orphan_paths:?} \
             (delete the .cpp, or list it in KEEPS_CPP if it is an exempt memory-safety script)"
        );
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

unsafe fn opt_cstr(p: *const std::os::raw::c_char) -> Option<String> {
    if p.is_null() {
        None
    } else {
        Some(std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned())
    }
}

/// Resolve the FLAREROOT/"stuff" directory (see tapptools/tenv.rs). Never
/// aborts: a panic or bad input yields "0\n\n<trace>" (no path).
/// Output text: line 1 = "1" if portable else "0"; line 2 = path (empty when
/// not found); remaining lines = decision trace. Returns the full byte length,
/// copies up to `cap` bytes into `out`.
#[no_mangle]
pub unsafe extern "C" fn flare_tenv_resolve_stuff_dir(
    env_root: *const std::os::raw::c_char,
    ini_root: *const std::os::raw::c_char,
    arg_root: *const std::os::raw::c_char,
    cwd: *const std::os::raw::c_char,
    exe_dir: *const std::os::raw::c_char,
    appdir: *const std::os::raw::c_char,
    is_macos: i32,
    out: *mut u8,
    cap: usize,
) -> usize {
    let inp = tenv::StuffInputs {
        env_root: opt_cstr(env_root),
        ini_root: opt_cstr(ini_root),
        arg_root: opt_cstr(arg_root),
        cwd: opt_cstr(cwd).map(std::path::PathBuf::from).unwrap_or_default(),
        exe_dir: opt_cstr(exe_dir).map(std::path::PathBuf::from),
        appdir: opt_cstr(appdir),
        is_macos: is_macos != 0,
    };
    let text = std::panic::catch_unwind(|| {
        let r = tenv::resolve_stuff_dir(&inp, &tenv::fs_is_dir);
        format!(
            "{}\n{}\n{}",
            if r.portable { 1 } else { 0 },
            r.path.map(|p| p.to_string_lossy().into_owned()).unwrap_or_default(),
            r.trace.join("\n")
        )
    })
    .unwrap_or_else(|_| "0\n\nresolver panicked".to_string());
    let n = text.len().min(cap);
    if !out.is_null() && n > 0 {
        std::ptr::copy_nonoverlapping(text.as_ptr(), out, n);
    }
    text.len()
}

/// Robustness ("fuzz-ish") tests for issues #110/#117: truncated and
/// bit-flipped SWF / FLA(zip) / XFL / legacy-CFBF inputs, built in-test, must
/// never panic (a panic across the FFI boundary aborts the whole app).
#[cfg(test)]
mod robustness {
    use super::*;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    const DOM: &str = r#"<DOMDocument width="550" height="400" frameRate="24"><timelines><DOMTimeline name="S1"><layers><DOMLayer name="a"><frames><DOMFrame index="0" duration="4294967295"><elements/></DOMFrame><DOMFrame index="4294967295" duration="9"/></frames></DOMLayer></layers></DOMTimeline></timelines></DOMDocument>"#;

    fn swf_raw() -> Vec<u8> {
        // header, RECT (nbits=0), frame rate, frame count, End tag
        let mut body = vec![0x00, 0x00, 0x18, 0x01, 0x00, 0x00, 0x00];
        let mut v = b"FWS".to_vec();
        v.push(10);
        let total = (8 + body.len()) as u32;
        v.extend_from_slice(&total.to_le_bytes());
        v.append(&mut body);
        v
    }
    fn swf_cws() -> Vec<u8> {
        let raw = swf_raw();
        let mut v = b"CWS".to_vec();
        v.extend_from_slice(&raw[3..8]);
        v.extend(miniz_oxide::deflate::compress_to_vec_zlib(&raw[8..], 6));
        v
    }
    fn crc32(d: &[u8]) -> u32 {
        let mut c = !0u32;
        for &b in d {
            c ^= b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { (c >> 1) ^ 0xEDB8_8320 } else { c >> 1 };
            }
        }
        !c
    }
    /// Minimal valid stored-entry zip.
    fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let (mut out, mut cd) = (Vec::new(), Vec::new());
        for (name, data) in entries {
            let off = out.len() as u32;
            let crc = crc32(data);
            out.extend_from_slice(b"PK\x03\x04\x0a\0\0\0\0\0\0\0\0\0");
            out.extend_from_slice(&crc.to_le_bytes());
            out.extend_from_slice(&(data.len() as u32).to_le_bytes());
            out.extend_from_slice(&(data.len() as u32).to_le_bytes());
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(&[0, 0]);
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(data);
            cd.extend_from_slice(b"PK\x01\x02\x14\0\x0a\0\0\0\0\0\0\0\0\0");
            cd.extend_from_slice(&crc.to_le_bytes());
            cd.extend_from_slice(&(data.len() as u32).to_le_bytes());
            cd.extend_from_slice(&(data.len() as u32).to_le_bytes());
            cd.extend_from_slice(&(name.len() as u16).to_le_bytes());
            cd.extend_from_slice(&[0u8; 12]);
            cd.extend_from_slice(&off.to_le_bytes());
            cd.extend_from_slice(name.as_bytes());
        }
        let cd_off = out.len() as u32;
        let n = entries.len() as u16;
        out.extend_from_slice(&cd);
        out.extend_from_slice(b"PK\x05\x06\0\0\0\0");
        out.extend_from_slice(&n.to_le_bytes());
        out.extend_from_slice(&n.to_le_bytes());
        out.extend_from_slice(&(cd.len() as u32).to_le_bytes());
        out.extend_from_slice(&cd_off.to_le_bytes());
        out.extend_from_slice(&[0, 0]);
        out
    }
    /// Legacy binary FLA: CFBF with one stream whose directory name length is ODD
    /// (corrupt) - the case that used to index out of bounds.
    fn cfbf(name_len: u16) -> Vec<u8> {
        let mut d = vec![0u8; 512 * 3];
        d[..8].copy_from_slice(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]);
        d[0x1A] = 3;
        d[0x1E] = 9;
        d[0x30..0x34].copy_from_slice(&1u32.to_le_bytes()); // first dir sector
        d[0x4C..0x50].copy_from_slice(&0u32.to_le_bytes()); // DIFAT[0] = FAT sector 0
        for i in 1..109 {
            d[0x4C + i * 4..0x50 + i * 4].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        }
        // FAT sector (at 512): sector0 = FAT, sector1 = end of chain
        d[512..516].copy_from_slice(&0xFFFF_FFFDu32.to_le_bytes());
        d[516..520].copy_from_slice(&0xFFFF_FFFEu32.to_le_bytes());
        // directory sector (at 1024): one stream entry
        let e = 1024;
        for (i, ch) in "Contents".encode_utf16().enumerate() {
            d[e + i * 2..e + i * 2 + 2].copy_from_slice(&ch.to_le_bytes());
        }
        d[e + 64..e + 66].copy_from_slice(&name_len.to_le_bytes());
        d[e + 66] = 2;
        d[e + 120..e + 124].copy_from_slice(&1234u32.to_le_bytes());
        d
    }

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    fn battery(b: &[u8]) {
        let _ = detect(b);
        let _ = swf_header(b);
        let _ = swf_frames(b);
        let _ = cfbf_header(b);
        let _ = swf::tags(b);
        let _ = framecount::swf_frame_count(b);
        let _ = framecount::cfbf_streams(b);
        let _ = framecount::zip_member(b, "DOMDocument.xml");
        let _ = xfl::layers(b);
        let _ = xfl_dom_info(b);
        let _ = moho::parse(b);
        let _ = fcp::parse(b);
        unsafe {
            let p = flare_cfbf_streams(b.as_ptr(), b.len());
            flare_moho_free(p);
            let mut f = 0u32;
            let _ = flare_xfl_layer_count(b.as_ptr(), b.len(), &mut f);
            let _ = flare_swf_tag_count(b.as_ptr(), b.len());
        }
    }

    fn hammer(name: &str, good: &[u8]) {
        let mut bad = Vec::new();
        let mut check = |label: String, v: &[u8]| {
            if catch_unwind(AssertUnwindSafe(|| battery(v))).is_err() {
                bad.push(label);
            }
        };
        check("empty".into(), &[]);
        for n in 0..good.len() {
            check(format!("truncated to {n}"), &good[..n]);
        }
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        for round in 0..600 {
            let mut v = good.to_vec();
            for _ in 0..1 + round % 4 {
                let i = (rng.next() as usize) % v.len();
                v[i] = rng.next() as u8;
            }
            check(format!("mutation round {round}"), &v);
        }
        assert!(bad.is_empty(), "{name}: panics on {:?}", &bad[..bad.len().min(5)]);
    }

    #[test]
    fn swf_inputs_never_panic() {
        hammer("swf", &swf_raw());
        hammer("cws", &swf_cws());
    }
    #[test]
    fn fla_zip_and_xfl_inputs_never_panic() {
        hammer("fla zip", &zip(&[("DOMDocument.xml", DOM.as_bytes()), ("LIBRARY/x.xml", b"<a/>")]));
        hammer("xfl xml", DOM.as_bytes());
    }
    #[test]
    fn legacy_cfbf_never_panics_incl_odd_name_length() {
        for nl in [0u16, 1, 2, 3, 5, 17, 64, 65, 200, 0xFFFF] {
            hammer(&format!("cfbf nl={nl}"), &cfbf(nl));
        }
        // regression: odd length used to panic (index 1 of a 1-byte chunk)
        let r = framecount::cfbf_streams(&cfbf(3)).expect("directory parses");
        assert_eq!(r.len(), 1);
    }
    #[test]
    fn huge_frame_durations_do_not_overflow() {
        // u32::MAX + 9 used to overflow (panic in debug, wrap in release)
        assert_eq!(framecount::xfl_frames(DOM.as_bytes()), Some(u32::MAX));
    }
    #[test]
    fn zip_member_reads_our_fixture() {
        let z = zip(&[("DOMDocument.xml", DOM.as_bytes())]);
        assert_eq!(framecount::zip_member(&z, "DOMDocument.xml").unwrap(), DOM.as_bytes());
        assert_eq!(detect(&z), Format::XflZip);
    }
    #[test]
    fn xfl_export_survives_corrupt_documents() {
        let dir = std::env::temp_dir().join(format!("flare_robust_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut rng = Rng(42);
        for round in 0..60 {
            let mut v = DOM.as_bytes().to_vec();
            if round > 0 {
                let cut = (rng.next() as usize) % v.len();
                v.truncate(cut.max(1));
                if round % 3 == 0 {
                    let i = (rng.next() as usize) % v.len();
                    v[i] = rng.next() as u8;
                }
            }
            std::fs::write(dir.join("DOMDocument.xml"), &v).unwrap();
            let (d, o) = (dir.to_string_lossy().to_string(), dir.join("_svg").to_string_lossy().to_string());
            let (d, o) = (std::ffi::CString::new(d).unwrap(), std::ffi::CString::new(o).unwrap());
            // via the FFI entry point: must return text, never abort
            let p = unsafe { flare_xfl_export_svg(d.as_ptr(), o.as_ptr()) };
            assert!(!p.is_null(), "round {round}");
            unsafe { flare_moho_free(p) };
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn tenv_ffi_resolver_never_null_and_traces() {
        let cwd = std::ffi::CString::new("/nonexistent-cwd").unwrap();
        let mut buf = vec![0u8; 4096];
        let n = unsafe {
            flare_tenv_resolve_stuff_dir(
                std::ptr::null(), std::ptr::null(), std::ptr::null(), cwd.as_ptr(),
                std::ptr::null(), std::ptr::null(), 0, buf.as_mut_ptr(), buf.len(),
            )
        };
        let text = String::from_utf8_lossy(&buf[..n.min(buf.len())]).to_string();
        assert!(text.lines().count() > 2 && text.contains("cwd"), "{text}");
        assert!(text.contains("no stuff directory found"));
    }
}
