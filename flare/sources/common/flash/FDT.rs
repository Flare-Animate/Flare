//! Port of `common/flash/FDT.cpp`.
//!
//! `FDT` is a low-level abstract base class for Flash definition tags; `FDT.cpp` contained
//! nothing but an empty implementation, because every derived class lives in its own
//! `FDTxxx.cpp`. There is consequently nothing to port from the file itself. The declaration
//! of the abstract class remains in `FDT.h` until the SWF reader (`common/flash/swf.rs`) needs
//! it.
//!
//! What lives here instead is the one helper every `FDTxxx` class shares once its body has
//! been assembled in a scratch `FSWFStream`: [`flare_fdt_finish_tag`] ("flush the body if
//! asked, add the terminating zero byte if asked, then `AppendTag`"). `FDTFonts`, `FDTText` and
//! `FDTButtons` (whose classes keep calling virtual C++ sub-objects such as `FShape`,
//! `FActionCondition` or `FCXForm`) use it to wrap bodies built partly in C++.

use crate::fswfstream::Stream;
use std::os::raw::c_void;

/// Wrap the body stream `body` (built by the caller) as tag `tag_id` of `out`.
///
/// With `flush_bits != 0` the partial byte of `body` is written out first
/// (`FlushBits()`), with `trailing_zero_byte != 0` a zero byte follows (`WriteByte(0)`); then
/// `out.AppendTag(tag_id, body.Size(), &body)`. Returns 0, 1 for a NULL handle or
/// `out == body`, 2 if a panic was caught.
///
/// # Safety
/// `out` and `body` must be NULL or distinct live `FSWFStream` handles.
#[no_mangle]
pub unsafe extern "C" fn flare_fdt_finish_tag(out: *mut c_void, tag_id: u16, body: *mut c_void, flush_bits: i32, trailing_zero_byte: i32) -> i32 {
    if out.is_null() || body.is_null() || out == body {
        return 1;
    }
    crate::guard(2, || {
        let b = &mut *(body as *mut Stream);
        if flush_bits != 0 {
            b.flush_bits();
        }
        if trailing_zero_byte != 0 {
            b.write_byte(0);
        }
        let n = b.size();
        (*(out as *mut Stream)).append_tag(tag_id, n, Some(b));
        0
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finish_tag_flush_and_zero_byte() {
        let mut out = Stream::new();
        let mut body = Stream::new();
        body.write_bits(0b101, 3);
        let (po, pb) = (&mut out as *mut Stream as *mut c_void, &mut body as *mut Stream as *mut c_void);
        unsafe {
            assert_eq!(flare_fdt_finish_tag(po, 11, pb, 1, 1), 0);
            assert_eq!(flare_fdt_finish_tag(po, 11, po, 0, 0), 1);
            assert_eq!(flare_fdt_finish_tag(std::ptr::null_mut(), 11, pb, 0, 0), 1);
            assert_eq!(flare_fdt_finish_tag(po, 11, std::ptr::null_mut(), 0, 0), 1);
        }
        let mut v = vec![0u8; out.full_size() as usize];
        assert!(out.write_to_memory(&mut v));
        // tag 11, length 2: the flushed 101_00000 byte, then the zero byte
        assert_eq!(v, vec![(11u16 << 6 | 2) as u8, (11u16 << 6 >> 8) as u8, 0b1010_0000, 0]);
    }
}
