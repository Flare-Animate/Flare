//! The LSB-first bit reader of a KWZ layer: bits are taken from little-endian 16-bit words. A
//! read past the layer's end yields zero bits and is counted, so the caller can refuse a layer
//! that needs more bits than it holds.

pub(crate) struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    value: u32,
    index: i32,
    overrun_words: usize,
}

impl<'a> Bits<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            value: 0,
            index: 16,
            overrun_words: 0,
        }
    }

    fn next_word(&mut self) -> u32 {
        let word = match self.data.get(self.pos..self.pos + 2) {
            Some(&[a, b]) => u32::from(u16::from_le_bytes([a, b])),
            _ => {
                self.overrun_words += 1;
                0
            }
        };
        self.pos += 2;
        word
    }

    /// Reads `n` bits (at most 16).
    pub(crate) fn read(&mut self, n: u32) -> u32 {
        let n = n.min(16);
        if self.index + n as i32 > 16 {
            let word = self.next_word();
            self.value |= word << (16 - self.index) as u32;
            self.index -= 16;
        }
        let result = self.value & ((1_u32 << n) - 1);
        self.value >>= n;
        self.index += n as i32;
        result
    }

    /// Whether the reads stayed within the layer, allowing the one word the reader fetches ahead.
    pub(crate) fn within(&self) -> bool {
        self.overrun_words <= 1
    }

    /// Bytes fetched so far.
    pub(crate) fn consumed(&self) -> usize {
        self.pos
    }
}
