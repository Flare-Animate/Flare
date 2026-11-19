//! The string section of an ABC block's constant pool, rewritten in place.
//!
//! Patching reads the block's bytes only as far as the end of the string section and copies the
//! rest as it is. Re-encoding the whole block through a writer would also work, but any encoding
//! the writer does differently (a longer number form, a field it normalises) would change bytes
//! nobody asked to change; here they cannot.

/// One entry of the string section: where its text starts and ends in the block.
pub(crate) struct Section {
    /// The first byte of the string count.
    start: usize,
    /// The first byte after the last string.
    end: usize,
    /// How many strings it holds (without the implied empty one).
    pub count: usize,
}

/// Reads a variable-length 32-bit number (the ABC `u30`/`s32` form): seven bits a byte, at most
/// five bytes. Returns the value and the byte after it.
fn read_varint(bytes: &[u8], at: usize) -> Option<(u32, usize)> {
    let mut value = 0u32;
    for i in 0..5 {
        let byte = *bytes.get(at + i)?;
        value |= u32::from(byte & 0x7f) << (7 * i);
        if byte & 0x80 == 0 {
            return Some((value, at + i + 1));
        }
    }
    Some((value, at + 5))
}

fn write_varint(out: &mut Vec<u8>, mut value: u32) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

/// Skips a pool section of `count - 1` entries (a count of 0 means none, as in the ABC format).
fn skip_entries(
    bytes: &[u8],
    at: usize,
    mut skip: impl FnMut(&[u8], usize) -> Option<usize>,
) -> Option<usize> {
    let (count, mut at) = read_varint(bytes, at)?;
    for _ in 1..count {
        at = skip(bytes, at)?;
    }
    Some(at)
}

/// Finds the string section of `abc`: after the version, the ints, the unsigned ints and the
/// doubles. `None` when the block ends before it does.
pub(crate) fn string_section(abc: &[u8]) -> Option<Section> {
    let at = 4; // minor and major version
    let at = skip_entries(abc, at, |b, at| read_varint(b, at).map(|(_, next)| next))?;
    let at = skip_entries(abc, at, |b, at| read_varint(b, at).map(|(_, next)| next))?;
    let start = skip_entries(abc, at, |b, at| (at + 8 <= b.len()).then_some(at + 8))?;
    let end = skip_entries(abc, start, |b, at| {
        let (len, text) = read_varint(b, at)?;
        let end = text.checked_add(len as usize)?;
        (end <= b.len()).then_some(end)
    })?;
    let count = read_varint(abc, start)?.0.saturating_sub(1) as usize;
    Some(Section { start, end, count })
}

/// The strings of a section found by `string_section`, as bytes.
pub(crate) fn section_strings<'a>(abc: &'a [u8], section: &Section) -> Vec<&'a [u8]> {
    let mut out = Vec::new();
    let Some((_, mut at)) = read_varint(abc, section.start) else {
        return out;
    };
    for _ in 0..section.count {
        let Some((len, text)) = read_varint(abc, at) else {
            break;
        };
        let Some(end) = text.checked_add(len as usize) else {
            break;
        };
        let Some(string) = abc.get(text..end) else {
            break;
        };
        out.push(string);
        at = end;
    }
    out
}

/// `abc` with its strings replaced by `strings` (pool order, without the implied empty string
/// at index 0). Everything outside the string section is copied unchanged.
pub(crate) fn with_strings(abc: &[u8], section: &Section, strings: &[Vec<u8>]) -> Vec<u8> {
    let text: usize = strings.iter().map(|s| s.len() + 5).sum();
    let mut out = Vec::with_capacity(abc.len() + text);
    out.extend_from_slice(&abc[..section.start]);
    let count = if strings.is_empty() {
        0
    } else {
        strings.len() + 1
    };
    write_varint(&mut out, count as u32);
    for string in strings {
        write_varint(&mut out, string.len() as u32);
        out.extend_from_slice(string);
    }
    out.extend_from_slice(&abc[section.end..]);
    out
}

#[cfg(test)]
#[path = "pool_tests.rs"]
mod tests;
