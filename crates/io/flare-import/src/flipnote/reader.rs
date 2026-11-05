//! A cursor over the file bytes whose every read is checked against the end of the file.

use super::error::FlipnoteError;

pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn at(bytes: &'a [u8], pos: usize) -> Result<Self, FlipnoteError> {
        if pos > bytes.len() {
            return Err(FlipnoteError::Truncated {
                what: "a section",
                at: pos,
                need: 0,
                len: bytes.len(),
            });
        }
        Ok(Self { bytes, pos })
    }

    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    pub(crate) fn take(&mut self, n: usize, what: &'static str) -> Result<&'a [u8], FlipnoteError> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.bytes.len());
        let Some(end) = end else {
            return Err(FlipnoteError::Truncated {
                what,
                at: self.pos,
                need: n,
                len: self.bytes.len(),
            });
        };
        let slice = self.bytes.get(self.pos..end).unwrap_or_default();
        self.pos = end;
        Ok(slice)
    }

    pub(crate) fn skip_to(&mut self, pos: usize, what: &'static str) -> Result<(), FlipnoteError> {
        if pos < self.pos {
            return Err(FlipnoteError::Truncated {
                what,
                at: pos,
                need: 0,
                len: self.bytes.len(),
            });
        }
        self.take(pos - self.pos, what).map(|_| ())
    }

    pub(crate) fn u8(&mut self, what: &'static str) -> Result<u8, FlipnoteError> {
        Ok(self.take(1, what)?.first().copied().unwrap_or(0))
    }

    pub(crate) fn i8(&mut self, what: &'static str) -> Result<i8, FlipnoteError> {
        self.u8(what).map(|b| i8::from_le_bytes([b]))
    }

    pub(crate) fn u16(&mut self, what: &'static str) -> Result<u16, FlipnoteError> {
        Ok(u16::from_le_bytes(self.array(what)?))
    }

    pub(crate) fn i16(&mut self, what: &'static str) -> Result<i16, FlipnoteError> {
        Ok(i16::from_le_bytes(self.array(what)?))
    }

    pub(crate) fn u32(&mut self, what: &'static str) -> Result<u32, FlipnoteError> {
        Ok(u32::from_le_bytes(self.array(what)?))
    }

    pub(crate) fn i32(&mut self, what: &'static str) -> Result<i32, FlipnoteError> {
        Ok(i32::from_le_bytes(self.array(what)?))
    }

    /// Big-endian, as the line headers of a frame are stored.
    pub(crate) fn u32_be(&mut self, what: &'static str) -> Result<u32, FlipnoteError> {
        Ok(u32::from_be_bytes(self.array(what)?))
    }

    fn array<const N: usize>(&mut self, what: &'static str) -> Result<[u8; N], FlipnoteError> {
        let mut out = [0_u8; N];
        for (dst, src) in out.iter_mut().zip(self.take(N, what)?) {
            *dst = *src;
        }
        Ok(out)
    }
}
