//! SWF tag walker (uncompressed body). RECORDHEADER layout per SWF spec, as in ruffle's swf crate (MIT/Apache-2.0).

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Tag { pub code: u16, pub offset: usize, pub len: usize }

/// Walk tags of an uncompressed ('F') SWF. Stops at End (code 0) or truncation.
pub fn tags(d: &[u8]) -> Option<Vec<Tag>> {
    if super::swf_header(d)?.0 != b'F' || d.len() < 9 { return None; }
    let nbits = (d[8] >> 3) as usize;
    let mut p = 8 + (5 + 4 * nbits + 7) / 8 + 4;
    let mut out = Vec::new();
    while p + 2 <= d.len() {
        let h = u16::from_le_bytes([d[p], d[p + 1]]);
        p += 2;
        let (code, mut len) = (h >> 6, (h & 0x3F) as usize);
        if len == 0x3F {
            if p + 4 > d.len() { break; }
            len = u32::from_le_bytes([d[p], d[p + 1], d[p + 2], d[p + 3]]) as usize;
            p += 4;
        }
        if p + len > d.len() { break; }
        out.push(Tag { code, offset: p, len });
        p += len;
        if code == 0 { break; }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn walk() {
        // header + RECT(nbits=1) + rate/count, SetBackgroundColor(9,len3), long ShowFrame(1), End
        let mut d = vec![b'F', b'W', b'S', 10, 0, 0, 0, 0, 0x08, 0, 0, 0x18, 1, 0];
        d.extend_from_slice(&((9u16 << 6) | 3).to_le_bytes()); d.extend_from_slice(&[1, 2, 3]);
        d.extend_from_slice(&((1u16 << 6) | 0x3F).to_le_bytes()); d.extend_from_slice(&0u32.to_le_bytes());
        d.extend_from_slice(&[0, 0]);
        let t = tags(&d).unwrap();
        assert_eq!(t.iter().map(|t| t.code).collect::<Vec<_>>(), vec![9, 1, 0]);
        assert_eq!(t[0], Tag { code: 9, offset: 16, len: 3 });
        assert!(tags(b"CWS\x0a\0\0\0\0\0").is_none());
        assert_eq!(tags(&d[..17]).unwrap().len(), 0);
    }
}
