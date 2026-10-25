//! Port of `common/tiio/tiio_jpg_exif.cpp`: the EXIF resolution reader used by the JPEG
//! reader (`JpgExifReader::process_EXIF` / `ProcessExifDir`, built without `ReadAllTags`,
//! i.e. only `XResolution`, `YResolution` and `ResolutionUnit` of the top-level IFD).
//!
//! The C++ walked an attacker-controlled blob with raw pointers and no end check on the
//! value reads; every access here is a bounds-checked slice read. The class stays in
//! `tiio_jpg_exif.h`; its two members are inline forwarders over the entry points below, and
//! the diagnostics the original sent to `std::cout` come back as a text log the header
//! prints, byte for byte (two of the original messages have no trailing newline; kept).
//!
//! Contract of [`flare_jpg_exif_process`]: `data` points at the `Exif\0\0` of the APP1
//! payload and `length` bytes are readable from there (the original was handed a pointer two
//! bytes *before* the payload and `data_length`, so its `ExifLength = length - 8` bounds the
//! TIFF area to the payload).
//!
//! Deviations, all where the C++ read out of bounds or overflowed (undefined behaviour):
//! * a payload shorter than the 14 header bytes, a first-IFD offset check that the original
//!   evaluated with an unsigned underflow (`length - 16` for `length < 16`), a directory or
//!   a value that does not fit the buffer: the tag/directory is skipped instead of read;
//! * `OffsetVal + ByteCount` is evaluated in 32 bits like the original, so a wrapped sum
//!   still takes the "accepted" branch, but the following read is bounds-checked and skips
//!   the tag when it falls outside the buffer;
//! * `ResolutionUnit` starts at 0 instead of indeterminate;
//! * `(int)double` follows x86 (`INT_MIN` for NaN / out of range) instead of being UB;
//! * the text log is truncated at the capacity the caller passes (diagnostics only).

/// Mirror of the three observable `ImageInfo` fields plus the byte order flag.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExifState {
    pub x_resolution: f32,
    pub y_resolution: f32,
    pub resolution_unit: i32,
    pub motorola_order: i32,
}

const BYTES_PER_FORMAT: [i32; 13] = [0, 1, 1, 2, 4, 8, 1, 1, 2, 4, 8, 4, 8];
const NUM_FORMATS: i32 = 12; // FMT_DOUBLE
const TAG_X_RESOLUTION: i32 = 0x011A;
const TAG_Y_RESOLUTION: i32 = 0x011B;
const TAG_RESOLUTION_UNIT: i32 = 0x0128;

const FMT_BYTE: i32 = 1;
const FMT_USHORT: i32 = 3;
const FMT_ULONG: i32 = 4;
const FMT_URATIONAL: i32 = 5;
const FMT_SBYTE: i32 = 6;
const FMT_SSHORT: i32 = 8;
const FMT_SLONG: i32 = 9;
const FMT_SRATIONAL: i32 = 10;
const FMT_SINGLE: i32 = 11;
const FMT_DOUBLE: i32 = 12;

/// Bounded text sink (the `std::cout` replacement).
struct Log {
    buf: Vec<u8>,
    cap: usize,
}

impl Log {
    fn put(&mut self, s: &str) {
        let room = self.cap.saturating_sub(self.buf.len());
        self.buf.extend_from_slice(&s.as_bytes()[..s.len().min(room)]);
    }
}

struct Reader<'a> {
    b: &'a [u8],
    motorola: bool,
    st: ExifState,
    log: Log,
}

impl Reader<'_> {
    fn bytes(&self, p: i64, n: usize) -> Option<&[u8]> {
        if p < 0 {
            return None;
        }
        let p = p as usize;
        self.b.get(p..p.checked_add(n)?)
    }

    /// `Get16u`
    fn get16u(&self, p: i64) -> Option<i32> {
        let s = self.bytes(p, 2)?;
        Some(if self.motorola { (s[0] as i32) << 8 | s[1] as i32 } else { (s[1] as i32) << 8 | s[0] as i32 })
    }

    /// `Get32s`
    fn get32s(&self, p: i64) -> Option<i32> {
        let s = self.bytes(p, 4)?;
        let a = [s[0], s[1], s[2], s[3]];
        Some(if self.motorola { i32::from_be_bytes(a) } else { i32::from_le_bytes(a) })
    }

    /// `ConvertAnyFormat`; `None` when the value does not fit the buffer.
    fn convert_any_format(&mut self, p: i64, format: i32) -> Option<f64> {
        Some(match format {
            FMT_SBYTE => self.bytes(p, 1)?[0] as i8 as f64,
            FMT_BYTE => self.bytes(p, 1)?[0] as f64,
            FMT_USHORT => self.get16u(p)? as f64,
            FMT_ULONG => self.get32s(p)? as u32 as f64,
            FMT_URATIONAL | FMT_SRATIONAL => {
                let num = self.get32s(p)?;
                let den = self.get32s(p + 4)?;
                if den == 0 {
                    0.0
                } else if format == FMT_SRATIONAL {
                    num as f64 / den as f64
                } else {
                    num as u32 as f64 / den as u32 as f64
                }
            }
            FMT_SSHORT => self.get16u(p)? as i16 as f64,
            FMT_SLONG => self.get32s(p)? as f64,
            // raw machine-order reads, as `*(float *)ValuePtr` / `*(double *)ValuePtr`
            FMT_SINGLE => {
                let s = self.bytes(p, 4)?;
                f32::from_ne_bytes([s[0], s[1], s[2], s[3]]) as f64
            }
            FMT_DOUBLE => {
                let s = self.bytes(p, 8)?;
                f64::from_ne_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]])
            }
            _ => {
                self.log.put(&format!("Illegal format code {format} in Exif header\n"));
                0.0
            }
        })
    }

    /// `ProcessExifDir`. Positions are indices into `self.b`; `base` is the TIFF origin
    /// (`OffsetBase`) and `exif_len` the original's `ExifLength`.
    fn process_dir(&mut self, dir_start: i64, base: i64, exif_len: u32, nesting: i32) {
        if nesting > 4 {
            self.log.put("Maximum Exif directory nesting exceeded (corrupt Exif header)\n");
            return;
        }
        let Some(n) = self.get16u(dir_start) else { return };
        let dir_end = dir_start + 2 + 12 * n as i64;
        let limit = base + exif_len as i64;
        if dir_end + 4 > limit && dir_end + 2 != limit && dir_end != limit {
            self.log.put(&format!("Illegally sized Exif subdirectory ({n}entries)\n"));
            return;
        }
        for de in 0..n as i64 {
            let entry = dir_start + 2 + 12 * de;
            let (Some(tag), Some(format), Some(comps)) =
                (self.get16u(entry), self.get16u(entry + 2), self.get32s(entry + 4))
            else {
                return;
            };
            if (format - 1) >= NUM_FORMATS {
                self.log.put(&format!("Illegal number format {format} for tag {tag} in Exif\n"));
                continue;
            }
            if comps as u32 > 0x10000 {
                self.log.put(&format!("Too many components {comps} for tag {tag} in Exif"));
                continue;
            }
            let byte_count = comps.wrapping_mul(BYTES_PER_FORMAT[format as usize]);
            let value_ptr = if byte_count > 4 {
                let Some(off) = self.get32s(entry + 8) else { return };
                let off = off as u32;
                if off.wrapping_add(byte_count as u32) > exif_len {
                    self.log.put(&format!("Illegal value pointer for tag {tag} in Exif"));
                    continue;
                }
                base + off as i64
            } else {
                entry + 8
            };
            match tag {
                TAG_X_RESOLUTION if nesting == 0 => {
                    if let Some(v) = self.convert_any_format(value_ptr, format) {
                        self.st.x_resolution = v as f32;
                        if self.st.y_resolution == 0.0 {
                            self.st.y_resolution = self.st.x_resolution;
                        }
                    }
                }
                TAG_Y_RESOLUTION if nesting == 0 => {
                    if let Some(v) = self.convert_any_format(value_ptr, format) {
                        self.st.y_resolution = v as f32;
                        if self.st.x_resolution == 0.0 {
                            self.st.x_resolution = self.st.y_resolution;
                        }
                    }
                }
                TAG_RESOLUTION_UNIT if nesting == 0 => {
                    if let Some(v) = self.convert_any_format(value_ptr, format) {
                        self.st.resolution_unit = cvttsd2si(v);
                    }
                }
                _ => {}
            }
        }
    }

    /// `process_EXIF`; `self.b` starts at `Exif\0\0`. Returns 0 when a directory was walked.
    fn process_exif(&mut self, length: u32) -> i32 {
        if self.b.len() < 6 || &self.b[..6] != b"Exif\0\0" {
            self.log.put("Incorrect Exif header\n");
            return 1;
        }
        match self.b.get(6..8) {
            Some(b"II") => self.motorola = false,
            Some(b"MM") => self.motorola = true,
            _ => {
                self.log.put("Invalid Exif alignment marker.\n");
                return 2;
            }
        }
        self.st.motorola_order = self.motorola as i32;
        if self.get16u(8) != Some(0x2a) {
            self.log.put("Invalid Exif start (1)\n");
            return 3;
        }
        let Some(first) = self.get32s(10).map(|v| v as u32) else {
            self.log.put("Invalid Exif start (1)\n");
            return 3;
        };
        if first < 8 || first > 16 {
            if first < 16 || length < 16 || first > length - 16 {
                self.log.put("invalid offset for first Exif IFD value\n");
                return 4;
            }
            self.log.put("Suspicious offset of first Exif IFD value\n");
        }
        // Section index = b index + 2: Section+8 is b[6]
        self.process_dir(6 + first as i64, 6, length.wrapping_sub(8), 0);
        0
    }
}

/// x86 `cvttsd2si`: truncate toward zero, `INT_MIN` for NaN and out of range.
fn cvttsd2si(v: f64) -> i32 {
    if v.is_nan() || v >= 2147483648.0 || v <= -2147483649.0 {
        i32::MIN
    } else {
        v as i32
    }
}

unsafe fn finish(r: Reader, st: *mut ExifState, log: *mut u8, log_cap: usize, log_len: *mut usize) {
    *st = r.st;
    if !log_len.is_null() {
        *log_len = r.log.buf.len();
    }
    if !log.is_null() && log_cap > 0 {
        std::ptr::copy_nonoverlapping(r.log.buf.as_ptr(), log, r.log.buf.len().min(log_cap));
    }
}

/// `JpgExifReader::process_EXIF`. Returns 0 (a directory was walked), 1 (not `Exif\0\0`),
/// 2 (bad byte order mark), 3 (bad TIFF magic), 4 (bad first IFD offset) or -1 (bad
/// argument). `*st` is read (initial values) and updated; the diagnostics land in `log`
/// (at most `log_cap` bytes, length in `*log_len`).
///
/// # Safety
/// `data` valid for `length` bytes, `st` for one `ExifState`, `log` for `log_cap` bytes.
#[no_mangle]
pub unsafe extern "C" fn flare_jpg_exif_process(
    data: *const u8,
    length: u32,
    st: *mut ExifState,
    log: *mut u8,
    log_cap: usize,
    log_len: *mut usize,
) -> i32 {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if st.is_null() || (data.is_null() && length != 0) {
            return -1;
        }
        let b: &[u8] = if length == 0 { &[] } else { std::slice::from_raw_parts(data, length as usize) };
        let mut r = Reader { b, motorola: (*st).motorola_order != 0, st: *st, log: Log { buf: Vec::new(), cap: log_cap } };
        let rc = r.process_exif(length);
        finish(r, st, log, log_cap, log_len);
        rc
    }))
    .unwrap_or(-1)
}

/// `JpgExifReader::ProcessExifDir`: `base` is `OffsetBase` with `exif_length` readable
/// bytes, `dir_offset` the directory position relative to it, `st->motorola_order` the byte
/// order. Returns 0, or -1 for a bad argument.
///
/// # Safety
/// As [`flare_jpg_exif_process`]; `base` valid for `exif_length` bytes.
#[no_mangle]
pub unsafe extern "C" fn flare_jpg_exif_process_dir(
    base: *const u8,
    exif_length: u32,
    dir_offset: i64,
    nesting: i32,
    st: *mut ExifState,
    log: *mut u8,
    log_cap: usize,
    log_len: *mut usize,
) -> i32 {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if st.is_null() || (base.is_null() && exif_length != 0) {
            return -1;
        }
        let b: &[u8] = if exif_length == 0 { &[] } else { std::slice::from_raw_parts(base, exif_length as usize) };
        let mut r = Reader { b, motorola: (*st).motorola_order != 0, st: *st, log: Log { buf: Vec::new(), cap: log_cap } };
        r.process_dir(dir_offset, 0, exif_length, nesting);
        finish(r, st, log, log_cap, log_len);
        0
    }))
    .unwrap_or(-1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build `Exif\0\0` + TIFF with one IFD of `entries` (tag, format, comps, 4-byte value
    /// field), followed by `tail`.
    fn exif(motorola: bool, entries: &[(u16, u16, u32, [u8; 4])], tail: &[u8]) -> Vec<u8> {
        let w16 = |v: u16| if motorola { v.to_be_bytes() } else { v.to_le_bytes() };
        let w32 = |v: u32| if motorola { v.to_be_bytes() } else { v.to_le_bytes() };
        let mut v = b"Exif\0\0".to_vec();
        v.extend_from_slice(if motorola { b"MM" } else { b"II" });
        v.extend_from_slice(&w16(0x2a));
        v.extend_from_slice(&w32(8));
        v.extend_from_slice(&w16(entries.len() as u16));
        for (t, f, c, val) in entries {
            v.extend_from_slice(&w16(*t));
            v.extend_from_slice(&w16(*f));
            v.extend_from_slice(&w32(*c));
            v.extend_from_slice(val);
        }
        v.extend_from_slice(&[0; 4]);
        v.extend_from_slice(tail);
        v
    }

    fn run(data: &[u8]) -> (i32, ExifState, String) {
        let mut st = ExifState { x_resolution: 0.0, y_resolution: 0.0, resolution_unit: 0, motorola_order: 0 };
        let mut log = vec![0u8; 4096];
        let mut n = 0usize;
        let rc = unsafe { flare_jpg_exif_process(data.as_ptr(), data.len() as u32, &mut st, log.as_mut_ptr(), log.len(), &mut n) };
        (rc, st, String::from_utf8(log[..n].to_vec()).unwrap())
    }

    #[test]
    fn rational_resolution_both_orders() {
        for mot in [false, true] {
            // entries: X (rational at offset), Y (rational), unit (short inline = 2)
            let mut tail = vec![];
            let w32 = |v: u32| if mot { v.to_be_bytes() } else { v.to_le_bytes() };
            let off0 = 8 + 2 + 36 + 4; // after IFD (TIFF-relative)
            tail.extend_from_slice(&w32(300));
            tail.extend_from_slice(&w32(1));
            tail.extend_from_slice(&w32(150));
            tail.extend_from_slice(&w32(2));
            let w16 = |v: u16| if mot { v.to_be_bytes() } else { v.to_le_bytes() };
            let unit = { let a = w16(2); [a[0], a[1], 0, 0] };
            tail.extend_from_slice(&[0; 2]); // the original bounds the TIFF area 2 bytes short of the payload
            let d = exif(mot, &[(0x011A, 5, 1, w32(off0 as u32)), (0x011B, 5, 1, w32(off0 as u32 + 8)), (0x0128, 3, 1, unit)], &tail);
            let (rc, st, log) = run(&d);
            assert_eq!(rc, 0);
            assert_eq!((st.x_resolution, st.y_resolution, st.resolution_unit), (300.0, 75.0, 2), "{mot}");
            assert_eq!(log, "");
        }
    }

    #[test]
    fn y_only_fills_x_and_bad_headers() {
        let d = exif(false, &[(0x011B, 3, 1, [72, 0, 0, 0])], &[]);
        let (_, st, _) = run(&d);
        assert_eq!((st.x_resolution, st.y_resolution), (72.0, 72.0));
        assert_eq!(run(b"Exif").0, 1);
        assert_eq!(run(b"Exif\0\0XX\x2a\0\x08\0\0\0").2, "Invalid Exif alignment marker.\n");
        assert_eq!(run(b"Exif\0\0II\x2b\0\x08\0\0\0").2, "Invalid Exif start (1)\n");
        assert_eq!(run(b"Exif\0\0II\x2a\0\x00\0\0\0").2, "invalid offset for first Exif IFD value\n");
        // too short for the unsigned underflow of the original
        assert_eq!(run(b"Exif\0\0II\x2a\0\x20\0\0\0").0, 4);
    }

    #[test]
    fn hostile_entries_do_not_read_out_of_bounds() {
        // huge directory
        let mut d = exif(false, &[], &[]);
        d[14] = 0xff;
        d[15] = 0xff;
        let (_, _, log) = run(&d);
        assert!(log.starts_with("Illegally sized Exif subdirectory (65535entries)"));
        // value pointer wraps in 32 bits: accepted by the check, skipped by the read
        let d = exif(false, &[(0x011A, 5, 1, 0xffff_fff8u32.to_le_bytes())], &[]);
        let (rc, st, _) = run(&d);
        assert_eq!((rc, st.x_resolution), (0, 0.0));
        // value pointer beyond the end
        let d = exif(false, &[(0x011A, 5, 1, 4000u32.to_le_bytes())], &[]);
        assert_eq!(run(&d).2, "Illegal value pointer for tag 282 in Exif");
        // every truncation of a valid blob is safe
        let d = exif(false, &[(0x011A, 5, 1, 30u32.to_le_bytes()), (0x0128, 3, 1, [2, 0, 0, 0])], &[0; 16]);
        for n in 0..d.len() {
            let _ = run(&d[..n]);
        }
    }

    #[test]
    fn cvttsd2si_matches_x86() {
        assert_eq!(cvttsd2si(2.9), 2);
        assert_eq!(cvttsd2si(-2.9), -2);
        assert_eq!(cvttsd2si(f64::NAN), i32::MIN);
        assert_eq!(cvttsd2si(1e12), i32::MIN);
    }

    fn unhex(s: &str) -> Vec<u8> {
        if s == "-" {
            return vec![];
        }
        (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
    }

    /// Replays `tests/native/gen_jpgexif_cases.py`: 706 payloads (random structured IFDs in
    /// both byte orders, bad headers, truncated blobs) through the original C++ reader, with
    /// the resulting fields and the exact `std::cout` text.
    #[test]
    fn matches_recorded_cpp() {
        let golden = include_str!("tiio_jpg_exif_cases.txt");
        let mut lines = golden.lines();
        let (mut cases, mut with_res, mut with_log) = (0, 0, 0);
        while let Some(l) = lines.next() {
            let payload = unhex(l.strip_prefix("in ").expect(l));
            let o: Vec<&str> = lines.next().unwrap().split(' ').collect();
            assert_eq!(o[0], "out");
            let (rc, st, log) = run(&payload);
            let _ = rc;
            let wx = f32::from_bits(u32::from_str_radix(o[1], 16).unwrap());
            let wy = f32::from_bits(u32::from_str_radix(o[2], 16).unwrap());
            let ctx = format!("{l} -> {o:?}");
            assert_eq!(st.x_resolution.to_bits(), wx.to_bits(), "{ctx}");
            assert_eq!(st.y_resolution.to_bits(), wy.to_bits(), "{ctx}");
            assert_eq!(st.resolution_unit, o[3].parse::<i32>().unwrap(), "{ctx}");
            assert_eq!((st.x_resolution != 0.0) as i32, o[4].parse::<i32>().unwrap(), "{ctx}");
            assert_eq!(log.as_bytes(), unhex(o[5]).as_slice(), "{ctx}");
            cases += 1;
            with_res += (st.x_resolution != 0.0) as i32;
            with_log += !log.is_empty() as i32;
        }
        assert_eq!(cases, 706);
        assert!(with_res > 300 && with_log > 300);
    }
}
