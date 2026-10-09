//! Flare format sniffing (SWF/CFBF/XFL). SWF header layout per the SWF spec, as in ruffle (MIT/Apache-2.0).
use std::slice;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(i32)]
pub enum Format { Unknown = 0, Swf = 1, Cfbf = 2, XflZip = 3, XflXml = 4 }

const CFBF_SIG: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

pub fn detect(d: &[u8]) -> Format {
    if swf_header(d).is_some() { return Format::Swf; }
    if d.starts_with(&CFBF_SIG) { return Format::Cfbf; }
    if d.starts_with(b"PK\x03\x04") {
        return if d.windows(15).any(|w| w == b"DOMDocument.xml") { Format::XflZip } else { Format::Unknown };
    }
    let head = &d[..d.len().min(512)];
    if head.windows(12).any(|w| w == b"<DOMDocument") { return Format::XflXml; }
    Format::Unknown
}

pub fn swf_header(d: &[u8]) -> Option<(u8, u8, u32)> {
    if d.len() < 8 || &d[1..3] != b"WS" || !matches!(d[0], b'F' | b'C' | b'Z') { return None; }
    Some((d[0], d[3], u32::from_le_bytes([d[4], d[5], d[6], d[7]])))
}

unsafe fn bytes<'a>(p: *const u8, n: usize) -> &'a [u8] {
    if p.is_null() { &[] } else { slice::from_raw_parts(p, n) }
}

#[no_mangle]
pub unsafe extern "C" fn flare_detect_format(data: *const u8, len: usize) -> i32 {
    detect(bytes(data, len)) as i32
}

#[no_mangle]
pub unsafe extern "C" fn flare_swf_header(data: *const u8, len: usize, c: *mut u8, v: *mut u8, l: *mut u32) -> i32 {
    match swf_header(bytes(data, len)) {
        Some((a, b, n)) => {
            if !c.is_null() { *c = a; }
            if !v.is_null() { *v = b; }
            if !l.is_null() { *l = n; }
            0
        }
        None => -1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn swf() {
        let d = [b'F', b'W', b'S', 10, 0x20, 0, 0, 0];
        assert_eq!(detect(&d), Format::Swf);
        assert_eq!(swf_header(&d), Some((b'F', 10, 32)));
        assert_eq!(detect(b"CWS\x09\0\0\0\0"), Format::Swf);
    }
    #[test] fn cfbf() { assert_eq!(detect(&CFBF_SIG), Format::Cfbf); }
    #[test] fn xfl() {
        assert_eq!(detect(b"PK\x03\x04....DOMDocument.xml"), Format::XflZip);
        assert_eq!(detect(b"<?xml version=\"1.0\"?><DOMDocument>"), Format::XflXml);
        assert_eq!(detect(b"PK\x03\x04nothing"), Format::Unknown);
    }
    #[test] fn ffi_null() { unsafe { assert_eq!(flare_detect_format(std::ptr::null(), 0), 0); } }
}
