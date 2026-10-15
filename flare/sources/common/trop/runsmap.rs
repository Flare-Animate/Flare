//! Port of common/trop/runsmap.cpp: run-length header coding used by RunsMap.
//!
//! Only the first/last pixel of a run carry a header.  The last 2 bits of a header
//! cover runs up to 3 pixels directly (length-1 in bits 6..7); longer runs store
//! `length-1` in the following byte (up to 254), and runs of 255+ store 255 there
//! and the full 32-bit `length-1` in the next 4 bytes.  Headers are written at
//! both ends (forward / reversed traversal) -- including the original's quirk that
//! the 32-bit value is only written at the forward end.

/// RunsMap::setRunLength(TPixelGR8 *pix, TUINT32 length)
///
/// # Safety
/// `pix` must point to a run of at least `length` bytes; `length == 0` or a
/// null `pix` is a no-op.
pub unsafe fn set_run_length(pix: *mut u8, length: u32) {
    if length == 0 || pix.is_null() {
        return;
    }
    let orig_pix = pix;
    let mut pix = orig_pix;

    let mut pix_rev = pix.add(length as usize - 1);

    let length = length.wrapping_sub(1);
    if length < 3 {
        let v = (length << 6) as u8;
        *pix = v;
        *pix_rev = v;
    } else {
        *pix = 3 << 6;
        *pix_rev = 3 << 6;
        pix = pix.add(1);
        pix_rev = pix_rev.wrapping_sub(1);
        if length < 255 {
            *pix = length as u8;
            *pix_rev = length as u8;
        } else {
            *pix = 255;
            *pix_rev = 255;
            pix = pix.add(1);
            let _ = pix_rev.wrapping_sub(4); // pixRev -= 4 (unused afterwards in the original)
            (pix as *mut u32).write_unaligned(length);
        }
    }

    debug_assert_eq!(run_length(orig_pix, false), length.wrapping_add(1));
}

/// RunsMap::runLength(const TPixelGR8 *pix, bool reversed)
///
/// # Safety
/// `pix` must point at a run header written by [`set_run_length`].
pub unsafe fn run_length(pix: *const u8, reversed: bool) -> u32 {
    let mut pix = pix;
    let mut length: u32 = (*pix >> 6) as u32;
    if length >= 3 {
        pix = if reversed { pix.wrapping_sub(1) } else { pix.add(1) };
        length = *pix as u32;
        if length >= 255 {
            pix = if reversed { pix.wrapping_sub(4) } else { pix.add(1) };
            length = (pix as *const u32).read_unaligned();
        }
    }

    length.wrapping_add(1)
}

/// # Safety
/// See [`set_run_length`].
#[no_mangle]
pub unsafe extern "C" fn flare_runsmap_set_run_length(run: *mut u8, length: u32) {
    set_run_length(run, length)
}

/// # Safety
/// See [`run_length`].
#[no_mangle]
pub unsafe extern "C" fn flare_runsmap_run_length(run: *const u8, reversed: bool) -> u32 {
    run_length(run, reversed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_runs_use_two_bits() {
        for len in 1u32..=3 {
            let mut buf = vec![0u8; len as usize];
            unsafe {
                set_run_length(buf.as_mut_ptr(), len);
                assert_eq!(buf[0], ((len - 1) << 6) as u8);
                assert_eq!(buf[len as usize - 1], ((len - 1) << 6) as u8);
                assert_eq!(run_length(buf.as_ptr(), false), len);
                assert_eq!(run_length(buf.as_ptr().add(len as usize - 1), true), len);
            }
        }
    }

    #[test]
    fn medium_runs_store_length_in_next_byte() {
        for len in [4u32, 5, 100, 255] {
            let mut buf = vec![0u8; len as usize];
            unsafe {
                set_run_length(buf.as_mut_ptr(), len);
                assert_eq!(buf[0], 3 << 6);
                assert_eq!(buf[1], (len - 1) as u8);
                assert_eq!(buf[len as usize - 1], 3 << 6);
                assert_eq!(buf[len as usize - 2], (len - 1) as u8);
                assert_eq!(run_length(buf.as_ptr(), false), len);
                assert_eq!(run_length(buf.as_ptr().add(len as usize - 1), true), len);
            }
        }
    }

    #[test]
    fn long_runs_store_u32_after_marker() {
        let len = 1000u32;
        let mut buf = vec![0u8; len as usize];
        unsafe {
            set_run_length(buf.as_mut_ptr(), len);
            assert_eq!(buf[0], 3 << 6);
            assert_eq!(buf[1], 255);
            assert_eq!(u32::from_le_bytes([buf[2], buf[3], buf[4], buf[5]]), 999);
            assert_eq!(buf[len as usize - 1], 3 << 6);
            assert_eq!(buf[len as usize - 2], 255);
            assert_eq!(run_length(buf.as_ptr(), false), len);
        }
    }

    #[test]
    fn zero_length_and_null_are_no_ops() {
        let mut buf = [7u8; 4];
        unsafe {
            flare_runsmap_set_run_length(buf.as_mut_ptr(), 0);
            flare_runsmap_set_run_length(std::ptr::null_mut(), 5);
        }
        assert_eq!(buf, [7; 4]);
    }

    #[test]
    fn ffi() {
        let mut buf = [0u8; 7];
        unsafe {
            flare_runsmap_set_run_length(buf.as_mut_ptr(), 7);
            assert_eq!(flare_runsmap_run_length(buf.as_ptr(), false), 7);
        }
    }
}
