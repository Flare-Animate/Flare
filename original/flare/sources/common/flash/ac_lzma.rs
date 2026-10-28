//! LZMA1 decoder for ZWS (LZMA-compressed) SWF files.
//!
//! Written from the public LZMA specification (Igor Pavlov, LZMA SDK, public
//! domain; `lzma-specification.txt`). A ZWS file stores `props[5]` followed by
//! a raw LZMA stream without the usual 8-byte uncompressed-size field, so the
//! size comes from the SWF header (`FileLength - 8`).

const TOP: u32 = 1 << 24;

struct Rc<'a> {
    d: &'a [u8],
    p: usize,
    range: u32,
    code: u32,
}

impl<'a> Rc<'a> {
    fn new(d: &'a [u8]) -> Result<Self, String> {
        if d.len() < 5 || d[0] != 0 {
            return Err("LZMA: bad range coder header".into());
        }
        let code = u32::from_be_bytes([d[1], d[2], d[3], d[4]]);
        Ok(Rc { d, p: 5, range: 0xFFFF_FFFF, code })
    }
    fn norm(&mut self) -> Result<(), String> {
        if self.range < TOP {
            let b = *self.d.get(self.p).ok_or("LZMA: truncated stream")?;
            self.p += 1;
            self.range <<= 8;
            self.code = (self.code << 8) | b as u32;
        }
        Ok(())
    }
    fn bit(&mut self, prob: &mut u16) -> Result<u32, String> {
        self.norm()?;
        let bound = (self.range >> 11) * (*prob as u32);
        if self.code < bound {
            self.range = bound;
            *prob += (2048 - *prob) >> 5;
            Ok(0)
        } else {
            self.range -= bound;
            self.code -= bound;
            *prob -= *prob >> 5;
            Ok(1)
        }
    }
    fn direct(&mut self, n: u32) -> Result<u32, String> {
        let mut r = 0u32;
        for _ in 0..n {
            self.norm()?;
            self.range >>= 1;
            let b = if self.code >= self.range {
                self.code -= self.range;
                1
            } else {
                0
            };
            r = (r << 1) | b;
        }
        Ok(r)
    }
    fn tree(&mut self, probs: &mut [u16], nbits: u32) -> Result<u32, String> {
        let mut m = 1usize;
        for _ in 0..nbits {
            m = (m << 1) | self.bit(&mut probs[m])? as usize;
        }
        Ok(m as u32 - (1 << nbits))
    }
    /// Reverse bit tree; `probs[base + m]`, m starting at 1.
    fn rtree(&mut self, probs: &mut [u16], base: isize, nbits: u32) -> Result<u32, String> {
        let (mut m, mut sym) = (1usize, 0u32);
        for i in 0..nbits {
            let b = self.bit(&mut probs[(base + m as isize) as usize])?;
            m = (m << 1) | b as usize;
            sym |= b << i;
        }
        Ok(sym)
    }
}

struct Len {
    choice: u16,
    choice2: u16,
    low: [[u16; 8]; 16],
    mid: [[u16; 8]; 16],
    high: [u16; 256],
}

impl Len {
    fn new() -> Self {
        Len { choice: 1024, choice2: 1024, low: [[1024; 8]; 16], mid: [[1024; 8]; 16], high: [1024; 256] }
    }
    fn decode(&mut self, rc: &mut Rc, ps: usize) -> Result<usize, String> {
        if rc.bit(&mut self.choice)? == 0 {
            return Ok(rc.tree(&mut self.low[ps], 3)? as usize);
        }
        if rc.bit(&mut self.choice2)? == 0 {
            return Ok(8 + rc.tree(&mut self.mid[ps], 3)? as usize);
        }
        Ok(16 + rc.tree(&mut self.high, 8)? as usize)
    }
}

/// Decode `data` (the stream after the 5 property bytes) to exactly `out_size`
/// bytes. Fewer bytes (truncated input, early end marker) is an error.
pub fn decode(props: &[u8], data: &[u8], out_size: usize) -> Result<Vec<u8>, String> {
    if props.len() < 5 || props[0] >= 9 * 5 * 5 {
        return Err("LZMA: bad properties".into());
    }
    let mut d = props[0] as u32;
    let lc = d % 9;
    d /= 9;
    let lp = d % 5;
    let pb = d / 5;
    let mut rc = Rc::new(data)?;
    let mut lit = vec![1024u16; 0x300usize << (lc + lp)];
    let mut is_match = [1024u16; 12 << 4];
    let (mut is_rep, mut g0, mut g1, mut g2) = ([1024u16; 12], [1024u16; 12], [1024u16; 12], [1024u16; 12]);
    let mut rep0_long = [1024u16; 12 << 4];
    let mut slot = [[1024u16; 64]; 4];
    let mut special = [1024u16; 115];
    let mut align = [1024u16; 16];
    let (mut mlen, mut rlen) = (Len::new(), Len::new());
    let mut out: Vec<u8> = Vec::with_capacity(out_size.min(1 << 28));
    let (mut state, mut r0, mut r1, mut r2, mut r3) = (0usize, 0usize, 0usize, 0usize, 0usize);
    while out.len() < out_size {
        let ps = out.len() & ((1 << pb) - 1);
        if rc.bit(&mut is_match[(state << 4) | ps])? == 0 {
            let prev = out.last().copied().unwrap_or(0) as usize;
            let ls = ((out.len() & ((1 << lp) - 1)) << lc) + (prev >> (8 - lc));
            let probs = &mut lit[0x300 * ls..0x300 * (ls + 1)];
            let mut sym = 1usize;
            if state >= 7 {
                let mut mb = *out.get(out.len().wrapping_sub(r0 + 1)).ok_or("LZMA: bad distance")? as usize;
                while sym < 0x100 {
                    let mbit = (mb >> 7) & 1;
                    mb <<= 1;
                    let b = rc.bit(&mut probs[((1 + mbit) << 8) + sym])? as usize;
                    sym = (sym << 1) | b;
                    if mbit != b {
                        break;
                    }
                }
            }
            while sym < 0x100 {
                sym = (sym << 1) | rc.bit(&mut probs[sym])? as usize;
            }
            out.push(sym as u8);
            state = if state < 4 { 0 } else if state < 10 { state - 3 } else { state - 6 };
            continue;
        }
        let len;
        if rc.bit(&mut is_rep[state])? == 0 {
            r3 = r2;
            r2 = r1;
            r1 = r0;
            len = mlen.decode(&mut rc, ps)?;
            state = if state < 7 { 7 } else { 10 };
            let s = rc.tree(&mut slot[len.min(3)], 6)? as usize;
            let dist: u32 = if s < 4 {
                s as u32
            } else {
                let nd = ((s >> 1) - 1) as u32;
                let base = (2 | (s as u32 & 1)) << nd;
                if s < 14 {
                    base + rc.rtree(&mut special, base as isize - s as isize - 1, nd)?
                } else {
                    base + (rc.direct(nd - 4)? << 4) + rc.rtree(&mut align, 0, 4)?
                }
            };
            if dist == 0xFFFF_FFFF {
                break; // end marker
            }
            r0 = dist as usize;
        } else {
            if out.is_empty() {
                return Err("LZMA: repeat before any data".into());
            }
            if rc.bit(&mut g0[state])? == 0 {
                if rc.bit(&mut rep0_long[(state << 4) | ps])? == 0 {
                    state = if state < 7 { 9 } else { 11 };
                    let b = *out.get(out.len().wrapping_sub(r0 + 1)).ok_or("LZMA: bad distance")?;
                    out.push(b);
                    continue;
                }
            } else {
                let dist;
                if rc.bit(&mut g1[state])? == 0 {
                    dist = r1;
                } else {
                    if rc.bit(&mut g2[state])? == 0 {
                        dist = r2;
                    } else {
                        dist = r3;
                        r3 = r2;
                    }
                    r2 = r1;
                }
                r1 = r0;
                r0 = dist;
            }
            len = rlen.decode(&mut rc, ps)?;
            state = if state < 7 { 8 } else { 11 };
        }
        if r0 >= out.len() {
            return Err("LZMA: match distance beyond start of data".into());
        }
        for _ in 0..len + 2 {
            if out.len() >= out_size {
                break;
            }
            let b = out[out.len() - r0 - 1];
            out.push(b);
        }
    }
    if out.len() != out_size {
        return Err(format!("LZMA: stream ended after {} of {} bytes", out.len(), out_size));
    }
    Ok(out)
}
