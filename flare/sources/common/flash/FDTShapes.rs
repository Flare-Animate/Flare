//! Port of `common/flash/FDTShapes.cpp`: the SWF shape-record writers.
//!
//! The shape classes (`FCXForm`, `FDTDefineShape*`, `FDTDefineMorphShape`, the fill/line style
//! arrays and records, the gradients, `FShape`, `FShapeRec*`, `FShapeWStyle`) stay in
//! `FDTShapes.h`, now with inline member definitions: they form an owning pointer graph
//! (`FFillStyleArray` -> `FAFillStyle*`, `FShapeWStyle` -> `FShapeRec*`, ...) dispatched through
//! virtual `WriteToSWFStream` / `IncludeNFillBitInfo`, and the `FColor` / `FMatrix` / `FRect`
//! sub-objects are written by their own (already ported) `flare_f*_write` exports. What moved
//! here is everything that is arithmetic or bit/byte layout, behind `flare_fshape_*` /
//! `flare_fcxform_*` / `flare_fdt_shape_*`, each taking the destination `FSWFStream` handle
//! ([`crate::fswfstream`]), null-checked and under `catch_unwind`:
//!
//! * colour transforms: `MinBits` of both flavours and the two record writers (they differ in
//!   when the bit buffer is flushed: `FCXForm` flushes after, `FCXFormWAlpha` before);
//! * style arrays: the `0xff` escape count; fill style heads (solid / bitmap / gradient type
//!   byte and bitmap id), gradient and gradient-record heads, (morph) line style heads;
//! * shape records: `FShape` / `FShapeWStyle` bit-count header, `FShapeRecChange` (move bits,
//!   head, new-style tail), `FShapeRecEdgeStraight` (line classification, bit width, record),
//!   `FShapeRecEdgeCurved`, `FShapeRecEnd`;
//! * tag assembly: the character id head, the morph shape join (`DWORD offset` + styles +
//!   first shape) and the `AppendTag` of `DefineShape/2/3` and `DefineMorphShape`.
//!
//! Quirks of the C++ are kept on purpose: a vertical or horizontal line with `dx == dy == 0` is
//! written as a vertical line; `nBits - 2` is written in 4 bits and wraps; `WriteBits(int, n)`
//! of negative values keeps the low `n` bits; the style-array count of `0xff` or more is written
//! as `0xff` plus a word (so 255 styles take the long form).
//!
//! Deviation: an `FShapeRecEdgeStraight` whose `nBits` would not fit the 4-bit field trips
//! `assert(nBits < 16 + 2)` in debug builds only (kept in the header); Rust just writes the low
//! 4 bits like release C++.

use crate::fswfstream::{max_num, min_bits, Stream};
use std::os::raw::c_void;

const STAG_DEFINE_SHAPE: u16 = 2;
const STAG_DEFINE_SHAPE2: u16 = 22;
const STAG_DEFINE_SHAPE3: u16 = 32;
const STAG_DEFINE_MORPH_SHAPE: u16 = 46;

const FILL_SOLID: u32 = 0x00;
const FILL_LINEAR_GRADIENT: u32 = 0x10;
const FILL_RADIAL_GRADIENT: u32 = 0x12;
const FILL_TILED_BITS: u32 = 0x40;
const FILL_CLIPPED_BITS: u32 = 0x41;

type H = *mut Stream;

unsafe fn on(h: *mut c_void, f: impl FnOnce(&mut Stream)) -> i32 {
    if h.is_null() {
        return 1;
    }
    crate::guard(2, || {
        f(&mut *(h as H));
        0
    })
}

// ---- colour transforms ----------------------------------------------------------------------

/// `FCXForm::MinBits`.
pub fn cxform_min_bits(rm: i32, gm: i32, bm: i32, ra: i32, ga: i32, ba: i32) -> u32 {
    // two step process: MaxNum takes only 4 arguments
    let max_value = max_num(rm, gm, bm, ra);
    let max_value = max_num(ga, ba, max_value as i32, 0);
    min_bits(max_value, 1).wrapping_add(1)
}

/// `FCXFormWAlpha::MinBits`.
#[allow(clippy::too_many_arguments)]
pub fn cxform_alpha_min_bits(rm: i32, gm: i32, bm: i32, am: i32, ra: i32, ga: i32, ba: i32, aa: i32) -> u32 {
    let max_mult = max_num(rm, gm, bm, am);
    let max_add = max_num(ra, ga, ba, aa);
    let max_value = max_num(max_mult as i32, max_add as i32, 0, 0);
    min_bits(max_value, 1).wrapping_add(1)
}

#[no_mangle]
pub extern "C" fn flare_fcxform_min_bits(rm: i32, gm: i32, bm: i32, ra: i32, ga: i32, ba: i32) -> u32 {
    crate::guard(0, || cxform_min_bits(rm, gm, bm, ra, ga, ba))
}

#[no_mangle]
pub extern "C" fn flare_fcxform_alpha_min_bits(rm: i32, gm: i32, bm: i32, am: i32, ra: i32, ga: i32, ba: i32, aa: i32) -> u32 {
    crate::guard(0, || cxform_alpha_min_bits(rm, gm, bm, am, ra, ga, ba, aa))
}

/// `FCXForm::WriteToSWFStream`.
///
/// # Safety
/// `out` NULL or a live `FSWFStream` handle.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_fcxform_write(
    out: *mut c_void,
    has_add: u32,
    has_mult: u32,
    n_bits: u32,
    rm: i32,
    gm: i32,
    bm: i32,
    ra: i32,
    ga: i32,
    ba: i32,
) -> i32 {
    on(out, |s| {
        s.write_bits(has_mult, 1);
        s.write_bits(has_add, 1);
        s.write_bits(n_bits, 4);
        if has_mult != 0 {
            s.write_bits(rm as u32, n_bits);
            s.write_bits(gm as u32, n_bits);
            s.write_bits(bm as u32, n_bits);
        }
        if has_add != 0 {
            s.write_bits(ra as u32, n_bits);
            s.write_bits(ga as u32, n_bits);
            s.write_bits(ba as u32, n_bits);
        }
        s.flush_bits();
    })
}

/// `FCXFormWAlpha::WriteToSWFStream` (flushes *before* the record, not after).
///
/// # Safety
/// As [`flare_fcxform_write`].
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_fcxform_alpha_write(
    out: *mut c_void,
    has_add: u32,
    has_mult: u32,
    n_bits: u32,
    rm: i32,
    gm: i32,
    bm: i32,
    am: i32,
    ra: i32,
    ga: i32,
    ba: i32,
    aa: i32,
) -> i32 {
    on(out, |s| {
        s.flush_bits();
        s.write_bits(has_mult, 1);
        s.write_bits(has_add, 1);
        s.write_bits(n_bits, 4);
        if has_mult != 0 {
            s.write_bits(rm as u32, n_bits);
            s.write_bits(gm as u32, n_bits);
            s.write_bits(bm as u32, n_bits);
            s.write_bits(am as u32, n_bits);
        }
        if has_add != 0 {
            s.write_bits(ra as u32, n_bits);
            s.write_bits(ga as u32, n_bits);
            s.write_bits(ba as u32, n_bits);
            s.write_bits(aa as u32, n_bits);
        }
    })
}

// ---- style arrays, fill / line style records ------------------------------------------------

/// `FFillStyleArray::WriteToSWFStream` / `FLineStyleArray::WriteToSWFStream`: the count head.
///
/// # Safety
/// As [`flare_fcxform_write`].
#[no_mangle]
pub unsafe extern "C" fn flare_fstyle_array_count(out: *mut c_void, size: u32) -> i32 {
    on(out, |s| {
        if size >= 0xff {
            s.write_byte(0xff);
            s.write_word(size);
        } else {
            s.write_byte(size);
        }
    })
}

/// Fill style kinds for [`flare_ffill_style_head`].
pub const FILL_KIND_SOLID: i32 = 0;
pub const FILL_KIND_BITMAP: i32 = 1;
pub const FILL_KIND_GRADIENT: i32 = 2;

/// The type byte (and bitmap id) of `FFillStyleSolid/Bitmap/Gradient` and the `FMorphFillStyle*`
/// twins: `kind` 0 solid, 1 bitmap (`flag` = tiled, writes `bitmap_id`), 2 gradient (`flag` =
/// linear). Any other kind is refused (status 1).
///
/// # Safety
/// As [`flare_fcxform_write`].
#[no_mangle]
pub unsafe extern "C" fn flare_ffill_style_head(out: *mut c_void, kind: i32, flag: i32, bitmap_id: u16) -> i32 {
    if !(0..=2).contains(&kind) {
        return 1;
    }
    on(out, |s| match kind {
        FILL_KIND_SOLID => s.write_byte(FILL_SOLID),
        FILL_KIND_BITMAP => {
            s.write_byte(if flag != 0 { FILL_TILED_BITS } else { FILL_CLIPPED_BITS });
            s.write_word(bitmap_id as u32);
        }
        _ => s.write_byte(if flag != 0 { FILL_LINEAR_GRADIENT } else { FILL_RADIAL_GRADIENT }),
    })
}

/// `FGradient::WriteToSWFStream` head: the record count byte.
///
/// # Safety
/// As [`flare_fcxform_write`].
#[no_mangle]
pub unsafe extern "C" fn flare_fgradient_head(out: *mut c_void, count: u32) -> i32 {
    on(out, |s| s.write_byte(count))
}

/// The ratio byte of `FGradRecord` / each half of `FMorphGradRecord`.
///
/// # Safety
/// As [`flare_fcxform_write`].
#[no_mangle]
pub unsafe extern "C" fn flare_fgrad_ratio(out: *mut c_void, ratio: u32) -> i32 {
    on(out, |s| s.write_byte(ratio))
}

/// `FLineStyle::WriteToSWFStream` head: the width word.
///
/// # Safety
/// As [`flare_fcxform_write`].
#[no_mangle]
pub unsafe extern "C" fn flare_fline_style_head(out: *mut c_void, width: u32) -> i32 {
    on(out, |s| s.write_word(width))
}

/// `FMorphLineStyle::WriteToSWFStream` head: the two width words.
///
/// # Safety
/// As [`flare_fcxform_write`].
#[no_mangle]
pub unsafe extern "C" fn flare_fmorph_line_style_head(out: *mut c_void, width1: u32, width2: u32) -> i32 {
    on(out, |s| {
        s.write_word(width1);
        s.write_word(width2);
    })
}

// ---- shape records ---------------------------------------------------------------------------

/// `FShape::WriteToSWFStream` head and `FShapeWStyle::WriteToSWFStream` bit counts.
///
/// # Safety
/// As [`flare_fcxform_write`].
#[no_mangle]
pub unsafe extern "C" fn flare_fshape_head(out: *mut c_void, n_fill_bits: u32, n_line_bits: u32) -> i32 {
    on(out, |s| {
        s.write_bits(n_fill_bits, 4);
        s.write_bits(n_line_bits, 4);
    })
}

/// `FShapeRecChange::MinBits`: width of the move delta fields.
#[no_mangle]
pub extern "C" fn flare_fshaperec_change_min_bits(dx: i32, dy: i32) -> u32 {
    crate::guard(0, || min_bits(max_num(dx, dy, 0, 0), 1))
}

/// `FShapeRecChange::WriteToSWFStream` up to (not including) the new-styles block.
///
/// # Safety
/// As [`flare_fcxform_write`].
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_fshaperec_change_head(
    out: *mut c_void,
    state_new_styles: u32,
    state_line_style: u32,
    state_fill_style1: u32,
    state_fill_style0: u32,
    state_move_to: u32,
    n_move_bits: u32,
    move_dx: i32,
    move_dy: i32,
    fill0_style: u32,
    fill1_style: u32,
    line_style: u32,
    n_fill_bits: u32,
    n_line_bits: u32,
) -> i32 {
    on(out, |s| {
        // non-edge record flag
        s.write_bits(0, 1);
        s.write_bits(state_new_styles, 1);
        s.write_bits(state_line_style, 1);
        s.write_bits(state_fill_style1, 1);
        s.write_bits(state_fill_style0, 1);
        s.write_bits(state_move_to, 1);
        if state_move_to != 0 {
            s.write_bits(n_move_bits, 5);
            s.write_bits(move_dx as u32, n_move_bits);
            s.write_bits(move_dy as u32, n_move_bits);
        }
        if state_fill_style0 != 0 {
            s.write_bits(fill0_style, n_fill_bits);
        }
        if state_fill_style1 != 0 {
            s.write_bits(fill1_style, n_fill_bits);
        }
        if state_line_style != 0 {
            s.write_bits(line_style, n_line_bits);
        }
    })
}

/// The tail of `FShapeRecChange::WriteToSWFStream` once the new style arrays are written: the
/// index widths follow from their sizes, are written, and returned through `n_fill_bits` /
/// `n_line_bits` (the record keeps them as its current widths).
///
/// # Safety
/// As [`flare_fcxform_write`]; the two out pointers must be non-NULL.
#[no_mangle]
pub unsafe extern "C" fn flare_fshaperec_change_new_styles(
    out: *mut c_void,
    fill_count: u32,
    line_count: u32,
    n_fill_bits: *mut u32,
    n_line_bits: *mut u32,
) -> i32 {
    if n_fill_bits.is_null() || n_line_bits.is_null() {
        return 1;
    }
    let (nf, nl) = (min_bits(fill_count, 0), min_bits(line_count, 0));
    let st = on(out, |s| {
        s.write_bits(nf, 4);
        s.write_bits(nl, 4);
    });
    if st == 0 {
        *n_fill_bits = nf;
        *n_line_bits = nl;
    }
    st
}

/// The fields `FShapeRecEdgeStraight` keeps (layout shared with `FDTShapes.h`).
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct StraightEdge {
    pub general_line_flag: u32,
    pub general_delta_x: i32,
    pub general_delta_y: i32,
    pub vertical_line_flag: u32,
    pub horizontal_delta_x: i32,
    pub vertical_delta_y: i32,
    pub n_bits: u32,
}

impl StraightEdge {
    /// The constructor: classify the line, then derive `nBits`.
    pub fn new(dx: i32, dy: i32) -> StraightEdge {
        let mut e = StraightEdge::default();
        if dx != 0 && dy != 0 {
            // a general line
            e.general_line_flag = 1;
            e.general_delta_x = dx;
            e.general_delta_y = dy;
        } else if dx == 0 {
            // not general: vertical (also the degenerate dx == dy == 0)
            e.vertical_line_flag = 1;
            e.vertical_delta_y = dy;
        } else {
            e.vertical_line_flag = 0;
            e.horizontal_delta_x = dx;
        }
        e.n_bits = e.min_bits();
        e
    }

    /// `FShapeRecEdgeStraight::MinBits`.
    pub fn min_bits(&self) -> u32 {
        min_bits(max_num(self.general_delta_x, self.general_delta_y, self.horizontal_delta_x, self.vertical_delta_y), 1)
    }

    fn write(&self, s: &mut Stream) {
        s.write_bits(1, 1); // edge record
        s.write_bits(1, 1); // straight edge
        s.write_bits(self.n_bits.wrapping_sub(2), 4);
        s.write_bits(self.general_line_flag, 1);
        if self.general_line_flag != 0 {
            s.write_bits(self.general_delta_x as u32, self.n_bits);
            s.write_bits(self.general_delta_y as u32, self.n_bits);
        } else {
            s.write_bits(self.vertical_line_flag, 1);
            if self.vertical_line_flag == 0 {
                s.write_bits(self.horizontal_delta_x as u32, self.n_bits);
            } else {
                s.write_bits(self.vertical_delta_y as u32, self.n_bits);
            }
        }
    }
}

/// `FShapeRecEdgeStraight` constructor: fills `*edge`.
///
/// # Safety
/// `edge` NULL or writable for one [`StraightEdge`].
#[no_mangle]
pub unsafe extern "C" fn flare_fedge_straight_new(dx: i32, dy: i32, edge: *mut StraightEdge) -> i32 {
    if edge.is_null() {
        return 1;
    }
    crate::guard(2, || {
        *edge = StraightEdge::new(dx, dy);
        0
    })
}

/// `FShapeRecEdgeStraight::WriteToSWFStream`.
///
/// # Safety
/// As [`flare_fcxform_write`]; `edge` NULL or readable.
#[no_mangle]
pub unsafe extern "C" fn flare_fedge_straight_write(out: *mut c_void, edge: *const StraightEdge) -> i32 {
    if edge.is_null() {
        return 1;
    }
    let e = *edge;
    on(out, |s| e.write(s))
}

/// `FShapeRecEdgeCurved::MinBits`.
#[no_mangle]
pub extern "C" fn flare_fedge_curved_min_bits(control_dx: i32, control_dy: i32, anchor_dx: i32, anchor_dy: i32) -> u32 {
    crate::guard(0, || min_bits(max_num(control_dx, control_dy, anchor_dx, anchor_dy), 1))
}

/// `FShapeRecEdgeCurved::WriteToSWFStream`.
///
/// # Safety
/// As [`flare_fcxform_write`].
#[no_mangle]
pub unsafe extern "C" fn flare_fedge_curved_write(
    out: *mut c_void,
    n_bits: u32,
    control_dx: i32,
    control_dy: i32,
    anchor_dx: i32,
    anchor_dy: i32,
) -> i32 {
    on(out, |s| {
        s.write_bits(1, 1); // edge record
        s.write_bits(0, 1); // curved edge
        s.write_bits(n_bits.wrapping_sub(2), 4);
        s.write_bits(control_dx as u32, n_bits);
        s.write_bits(control_dy as u32, n_bits);
        s.write_bits(anchor_dx as u32, n_bits);
        s.write_bits(anchor_dy as u32, n_bits);
    })
}

/// `FShapeRecEnd::WriteToSWFStream`: six zero bits, then flush.
///
/// # Safety
/// As [`flare_fcxform_write`].
#[no_mangle]
pub unsafe extern "C" fn flare_fshaperec_end_write(out: *mut c_void) -> i32 {
    on(out, |s| {
        s.write_bits(0, 6);
        s.flush_bits();
    })
}

// ---- tags -------------------------------------------------------------------------------------

/// The head every `FDTDefineShape*` / `FDTDefineMorphShape` body starts with.
///
/// # Safety
/// As [`flare_fcxform_write`].
#[no_mangle]
pub unsafe extern "C" fn flare_fdt_shape_head(body: *mut c_void, character_id: u16) -> i32 {
    on(body, |s| s.write_word(character_id as u32))
}

/// Shape tag kinds for [`flare_fdt_shape_finish`].
pub const SHAPE_TAG_SHAPE: i32 = 0;
pub const SHAPE_TAG_SHAPE2: i32 = 1;
pub const SHAPE_TAG_SHAPE3: i32 = 2;
pub const SHAPE_TAG_MORPH: i32 = 3;

/// `AppendTag(stagDefineShape / 2 / 3 / DefineMorphShape, body.Size(), &body)`.
///
/// # Safety
/// `out` and `body` NULL or live, distinct `FSWFStream` handles.
#[no_mangle]
pub unsafe extern "C" fn flare_fdt_shape_finish(out: *mut c_void, kind: i32, body: *mut c_void) -> i32 {
    if out.is_null() || body.is_null() || out == body {
        return 1;
    }
    let id = match kind {
        SHAPE_TAG_SHAPE => STAG_DEFINE_SHAPE,
        SHAPE_TAG_SHAPE2 => STAG_DEFINE_SHAPE2,
        SHAPE_TAG_SHAPE3 => STAG_DEFINE_SHAPE3,
        SHAPE_TAG_MORPH => STAG_DEFINE_MORPH_SHAPE,
        _ => return 1,
    };
    crate::guard(2, || {
        let b = &mut *(body as H);
        let n = b.size();
        (*(out as H)).append_tag(id, n, Some(b));
        0
    })
}

/// The middle of `FDTDefineMorphShape::WriteToSWFStream`: `DWORD temp.Size()`, then `temp`
/// (styles + first shape) appended to `body`.
///
/// # Safety
/// As [`flare_fdt_shape_finish`].
#[no_mangle]
pub unsafe extern "C" fn flare_fdt_morph_join(body: *mut c_void, temp: *mut c_void) -> i32 {
    if body.is_null() || temp.is_null() || body == temp {
        return 1;
    }
    crate::guard(2, || {
        let t = &mut *(temp as H);
        let b = &mut *(body as H);
        b.write_dword(t.size());
        b.append(t);
        0
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &mut Stream) -> *mut c_void {
        s as *mut Stream as *mut c_void
    }

    fn bytes(s: &mut Stream) -> Vec<u8> {
        let mut v = vec![0u8; s.full_size() as usize];
        assert!(s.write_to_memory(&mut v));
        v
    }

    fn unhex(s: &str) -> Vec<u8> {
        if s == "-" {
            return vec![];
        }
        (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
    }

    fn prefixed(n: u32) -> Stream {
        let mut s = Stream::new();
        if n > 0 {
            s.write_bits((1 << n) - 1, n);
        }
        s
    }

    #[test]
    fn null_handles_are_refused() {
        unsafe {
            assert_eq!(flare_fcxform_write(std::ptr::null_mut(), 0, 0, 1, 0, 0, 0, 0, 0, 0), 1);
            assert_eq!(flare_fstyle_array_count(std::ptr::null_mut(), 3), 1);
            assert_eq!(flare_fshaperec_end_write(std::ptr::null_mut()), 1);
            assert_eq!(flare_fedge_straight_new(1, 1, std::ptr::null_mut()), 1);
            let mut s = Stream::new();
            assert_eq!(flare_fedge_straight_write(p(&mut s), std::ptr::null()), 1);
            assert_eq!(flare_fdt_shape_finish(p(&mut s), 0, std::ptr::null_mut()), 1);
            assert_eq!(flare_fdt_shape_finish(p(&mut s), 0, p(&mut s)), 1);
            assert_eq!(flare_fdt_shape_finish(p(&mut Stream::new()), 9, p(&mut s)), 1);
            assert_eq!(flare_fdt_morph_join(p(&mut s), p(&mut s)), 1);
            assert_eq!(flare_ffill_style_head(p(&mut s), 7, 0, 0), 1);
            assert_eq!(flare_fshaperec_change_new_styles(p(&mut s), 1, 1, std::ptr::null_mut(), std::ptr::null_mut()), 1);
        }
    }

    #[test]
    fn style_array_count_escape() {
        let mut s = Stream::new();
        unsafe {
            assert_eq!(flare_fstyle_array_count(p(&mut s), 254), 0);
            assert_eq!(flare_fstyle_array_count(p(&mut s), 255), 0);
            assert_eq!(flare_fstyle_array_count(p(&mut s), 256), 0);
        }
        assert_eq!(bytes(&mut s), vec![254, 0xff, 255, 0, 0xff, 0, 1]);
    }

    #[test]
    fn fill_style_heads() {
        let mut s = Stream::new();
        unsafe {
            flare_ffill_style_head(p(&mut s), FILL_KIND_SOLID, 1, 9);
            flare_ffill_style_head(p(&mut s), FILL_KIND_BITMAP, 1, 0x1234);
            flare_ffill_style_head(p(&mut s), FILL_KIND_BITMAP, 0, 7);
            flare_ffill_style_head(p(&mut s), FILL_KIND_GRADIENT, 1, 0);
            flare_ffill_style_head(p(&mut s), FILL_KIND_GRADIENT, 0, 0);
        }
        assert_eq!(bytes(&mut s), vec![0x00, 0x40, 0x34, 0x12, 0x41, 7, 0, 0x10, 0x12]);
    }

    #[test]
    fn straight_edge_classification() {
        let g = StraightEdge::new(5, -3);
        assert_eq!((g.general_line_flag, g.vertical_line_flag, g.n_bits), (1, 0, 4));
        let v = StraightEdge::new(0, 9);
        assert_eq!((v.general_line_flag, v.vertical_line_flag, v.vertical_delta_y), (0, 1, 9));
        let h = StraightEdge::new(-6, 0);
        assert_eq!((h.general_line_flag, h.vertical_line_flag, h.horizontal_delta_x), (0, 0, -6));
        // the degenerate zero-length line is written as a vertical line
        let z = StraightEdge::new(0, 0);
        assert_eq!((z.vertical_line_flag, z.vertical_delta_y), (1, 0));
        let mut s = Stream::new();
        z.write(&mut s);
        s.flush_bits();
        // 1 1 1111 0 1 | one-bit delta 0, flushed
        assert_eq!(z.n_bits, 1);
        assert_eq!(bytes(&mut s), vec![0xfd, 0x00]);
    }

    #[test]
    fn cxform_bits_and_flush_order() {
        assert_eq!(cxform_min_bits(0, 0, 0, 0, 0, 0), 2);
        assert_eq!(cxform_min_bits(255, -256, 0, 0, 0, 1), 11);
        assert_eq!(cxform_alpha_min_bits(1, 2, 3, 4, 5, 6, 7, 8), 6);
        // FCXForm flushes after the record, FCXFormWAlpha before it
        let mut a = prefixed(3);
        unsafe { flare_fcxform_write(p(&mut a), 1, 0, 3, 0, 0, 0, 1, 2, 3) };
        // 3 + 2 + 4 + 3 * 3 = 18 bits, flushed to 3 bytes
        assert_eq!(bytes(&mut a).len(), 3);
        let mut b = prefixed(3);
        unsafe { flare_fcxform_alpha_write(p(&mut b), 0, 1, 3, 1, 2, 3, 0, 0, 0, 0, 0) };
        assert_eq!(bytes(&mut b)[0], 0b1110_0000);
    }

    #[test]
    fn change_record_new_styles_widths() {
        let mut s = Stream::new();
        let (mut nf, mut nl) = (0u32, 0u32);
        unsafe { assert_eq!(flare_fshaperec_change_new_styles(p(&mut s), 5, 0, &mut nf, &mut nl), 0) };
        assert_eq!((nf, nl), (3, 0));
        assert_eq!(bytes(&mut s), vec![0x30]);
    }

    #[test]
    fn morph_join_prefixes_size() {
        let mut body = Stream::new();
        let mut temp = Stream::new();
        temp.write_byte(0xaa);
        temp.write_byte(0xbb);
        unsafe { assert_eq!(flare_fdt_morph_join(p(&mut body), p(&mut temp)), 0) };
        assert_eq!(bytes(&mut body), vec![2, 0, 0, 0, 0xaa, 0xbb]);
        let mut out = Stream::new();
        unsafe { assert_eq!(flare_fdt_shape_finish(p(&mut out), SHAPE_TAG_MORPH, p(&mut body)), 0) };
        assert_eq!(bytes(&mut out), vec![(46u16 << 6 | 6) as u8, (46u16 << 6 >> 8) as u8, 2, 0, 0, 0, 0xaa, 0xbb]);
    }

    // Replays the records recorded from the original C++ (swfshapes_cases.txt).
    #[test]
    fn replay_recorded_cpp_cases() {
        let mut counts = std::collections::BTreeMap::<String, usize>::new();
        for line in include_str!("swfshapes_cases.txt").lines() {
            let f: Vec<&str> = line.split('|').collect();
            let nums = |i: usize| -> Vec<i64> { f[i].split(',').map(|x| x.parse().unwrap()).collect() };
            match f[0] {
                "cx" => {
                    let a = nums(2);
                    let mut s = prefixed(f[1].parse().unwrap());
                    let nb = cxform_min_bits(a[2] as i32, a[3] as i32, a[4] as i32, a[5] as i32, a[6] as i32, a[7] as i32);
                    unsafe {
                        assert_eq!(
                            flare_fcxform_write(p(&mut s), a[0] as u32, a[1] as u32, nb, a[2] as i32, a[3] as i32, a[4] as i32, a[5] as i32, a[6] as i32, a[7] as i32),
                            0
                        )
                    };
                    assert_eq!(bytes(&mut s), unhex(f[3]), "{line}");
                }
                "cxa" => {
                    let a = nums(2);
                    let mut s = prefixed(f[1].parse().unwrap());
                    let v: Vec<i32> = a[2..].iter().map(|&x| x as i32).collect();
                    let nb = cxform_alpha_min_bits(v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7]);
                    unsafe { assert_eq!(flare_fcxform_alpha_write(p(&mut s), a[0] as u32, a[1] as u32, nb, v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7]), 0) };
                    s.flush_bits();
                    assert_eq!(bytes(&mut s), unhex(f[3]), "{line}");
                }
                "straight" => {
                    let a = nums(2);
                    let mut s = prefixed(f[1].parse().unwrap());
                    let e = StraightEdge::new(a[0] as i32, a[1] as i32);
                    unsafe { assert_eq!(flare_fedge_straight_write(p(&mut s), &e), 0) };
                    s.flush_bits();
                    assert_eq!(bytes(&mut s), unhex(f[3]), "{line}");
                }
                "curved" => {
                    let a = nums(2);
                    let mut s = prefixed(f[1].parse().unwrap());
                    let v: Vec<i32> = a.iter().map(|&x| x as i32).collect();
                    let nb = flare_fedge_curved_min_bits(v[0], v[1], v[2], v[3]);
                    unsafe { assert_eq!(flare_fedge_curved_write(p(&mut s), nb, v[0], v[1], v[2], v[3]), 0) };
                    s.flush_bits();
                    assert_eq!(bytes(&mut s), unhex(f[3]), "{line}");
                }
                "change" => {
                    // flags(new,line,fill1,fill0,move), move dx, dy, fill0, fill1, line, nfill, nline
                    let a = nums(2);
                    let mut s = prefixed(f[1].parse().unwrap());
                    let nm = flare_fshaperec_change_min_bits(a[5] as i32, a[6] as i32);
                    unsafe {
                        assert_eq!(
                            flare_fshaperec_change_head(
                                p(&mut s),
                                a[0] as u32,
                                a[1] as u32,
                                a[2] as u32,
                                a[3] as u32,
                                a[4] as u32,
                                nm,
                                a[5] as i32,
                                a[6] as i32,
                                a[7] as u32,
                                a[8] as u32,
                                a[9] as u32,
                                a[10] as u32,
                                a[11] as u32
                            ),
                            0
                        )
                    };
                    s.flush_bits();
                    assert_eq!(bytes(&mut s), unhex(f[3]), "{line}");
                }
                "shapeend" => {
                    let mut s = prefixed(f[1].parse().unwrap());
                    unsafe { assert_eq!(flare_fshaperec_end_write(p(&mut s)), 0) };
                    assert_eq!(bytes(&mut s), unhex(f[2]), "{line}");
                }
                "stylecount" => {
                    let mut s = Stream::new();
                    unsafe { assert_eq!(flare_fstyle_array_count(p(&mut s), f[1].parse().unwrap()), 0) };
                    assert_eq!(bytes(&mut s), unhex(f[2]), "{line}");
                }
                _ => continue,
            }
            *counts.entry(f[0].into()).or_default() += 1;
        }
        for k in ["cx", "cxa", "straight", "curved", "change", "shapeend", "stylecount"] {
            assert!(counts.get(k).copied().unwrap_or(0) > 0, "no {k} cases");
        }
    }
}
