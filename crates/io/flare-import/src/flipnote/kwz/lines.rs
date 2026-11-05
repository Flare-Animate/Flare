//! The KWZ line tables. A line is eight pixels of values 0 to 2, and every line is one of 3^8 =
//! 6561 codes; a code is read as eight base-3 digits. The "shift" table is the same line with
//! its pixels rotated one place, used for the alternating rows of some tile types. The common
//! table holds 32 lines that the encoder uses most, so they take five bits instead of thirteen.
//! Ported from flipnote.js (MIT) `KwzParser.ts`, see ENGINEERING.md.

pub(crate) const LINES: usize = 6561;
const COMMON_CODES: [u16; 32] = [
    0x0000, 0x0CD0, 0x19A0, 0x02D9, 0x088B, 0x0051, 0x00F3, 0x0009, 0x001B, 0x0001, 0x0003, 0x05B2,
    0x1116, 0x00A2, 0x01E6, 0x0012, 0x0036, 0x0002, 0x0006, 0x0B64, 0x08DC, 0x0144, 0x00FC, 0x0024,
    0x001C, 0x0004, 0x0334, 0x099C, 0x0668, 0x1338, 0x1004, 0x166C,
];

pub type Line = [u8; 8];

pub struct LineTables {
    pub(crate) full: Vec<Line>,
    pub(crate) full_shift: Vec<Line>,
    pub(crate) common: [Line; 32],
    pub(crate) common_shift: [Line; 32],
}

impl LineTables {
    pub fn new() -> Self {
        let mut full = Vec::with_capacity(LINES);
        let mut full_shift = Vec::with_capacity(LINES);
        for code in 0..LINES {
            // Digit 0 is the most significant: the encoder's loops nest a, b, ... h.
            let mut d = [0_u8; 8];
            let mut rest = code;
            for slot in d.iter_mut().rev() {
                *slot = (rest % 3) as u8;
                rest /= 3;
            }
            let [a, b, c, dd, e, f, g, h] = d;
            full.push([b, a, dd, c, f, e, h, g]);
            full_shift.push([a, dd, c, f, e, h, g, b]);
        }
        let mut common = [[0_u8; 8]; 32];
        let mut common_shift = [[0_u8; 8]; 32];
        for (i, &code) in COMMON_CODES.iter().enumerate() {
            let k = usize::from(code);
            if let (Some(c), Some(cs), Some(f), Some(fs)) = (
                common.get_mut(i),
                common_shift.get_mut(i),
                full.get(k),
                full_shift.get(k),
            ) {
                *c = *f;
                *cs = *fs;
            }
        }
        Self {
            full,
            full_shift,
            common,
            common_shift,
        }
    }
}

impl Default for LineTables {
    fn default() -> Self {
        Self::new()
    }
}
