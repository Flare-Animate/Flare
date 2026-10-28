//! Distance transform for CM32 rasters, ported from `tdistancetransform.cpp`.
//!
//! Implements `TRop::expandPaint`: propagates paint values into empty (tone 0,
//! paint 0) regions using the O(rows*cols) separable parabolic DT.

#[inline] fn sq(v: i64) -> i64 { v * v }
#[inline] fn takeover_dist(a: u32, b: u32, d: u32) -> u32 {
    if b < a { d } else {
        ((d + (b - a) / d + 1) / 2).max(d)
    }
}

fn tone(v: u32) -> i32 { (v & 0xFF) as i32 }
fn paint(v: u32) -> i32 { ((v >> 8) & 0xFFF) as i32 }
fn ink(v: u32) -> i32 { (v >> 20) as i32 }
fn is_inside(v: u32) -> bool { tone(v) != 0 || paint(v) != 0 }
fn set_paint(v: u32, p: i32) -> u32 { (v & !0xFFF00) | ((p as u32) << 8) }

pub fn expand_paint(ras: *mut u8, wrap: usize, lx: i32, ly: i32) {
    let n = (lx as usize) * (ly as usize);
    // dt raster (squared distances), u32
    let mut dt = vec![0u32; n];
    // init
    let ui_max: u32 = u32::MAX - 2;
    for y in 0..ly as usize {
        for x in 0..lx as usize {
            let off = y * lx as usize + x;
            let v = unsafe { u32::from_le_bytes([*ras.add(y*wrap + x*4), *ras.add(y*wrap + x*4+1),
                                                 *ras.add(y*wrap + x*4+2), *ras.add(y*wrap + x*4+3)]) };
            if !is_inside(v) { dt[off] = ui_max; }
        }
    }

    // expand along x (forward then backward), then y.
    // We implement the 1D expand generically.
    // expand(lineLength, linesCount, buf, incrPix, incrLine, dtBuf, dtIncrPix, dtIncrLine, outFunc)
    // outFunc: CopyPaint -> out.setPaint(in.getPaint())
    // We'll do the four passes.

    // helper: run expand on a 1D set of lines.
    // Lines run along the "lineLength" axis; there are "linesCount" of them.
    // buf pointer base = ras pixels (u32 view), incrPix = step in elements along line,
    // incrLine = step in elements between lines.
    // dtBuf = dt slice base.

    // We need to operate on u32 elements. Let's convert ras to a u32 vec for ease.
    let mut pix = vec![0u32; n];
    for i in 0..n {
        let off = i * 4; // not right for strided, but ras is contiguous? wrap may != lx*4
        // ras is strided; handle via wrap
        let _ = off;
    }
    // Actually ras is strided by wrap (bytes). Let's build pix vec with stride.
    for y in 0..ly as usize {
        for x in 0..lx as usize {
            pix[y * lx as usize + x] = unsafe {
                u32::from_le_bytes([
                    *ras.add(y * wrap + x * 4),
                    *ras.add(y * wrap + x * 4 + 1),
                    *ras.add(y * wrap + x * 4 + 2),
                    *ras.add(y * wrap + x * 4 + 3),
                ])
            };
        }
    }

    // expand function for a given orientation.
    // line_length: number of elements along the line
    // lines_count: number of parallel lines
    // get_pix(idx) -> pix element index (into pix vec, row-major lx*ly)
    // We'll handle two orientations: horizontal (lines are rows) and vertical (lines are cols).

    // Horizontal pass: lines = rows (ly lines), each length lx.
    // incrPix=1, incrLine=lx, dt same.
    expand_1d(&mut pix, &mut dt, lx as usize, ly as usize, 1, lx as usize);
    // reverse horizontal
    expand_1d_rev(&mut pix, &mut dt, lx as usize, ly as usize, 1, lx as usize);
    // vertical pass: lines = cols (lx lines), each length ly.
    // We need to operate on transposed. Easier: do in-place with strided access.
    expand_1d_strided(&mut pix, &mut dt, ly as usize, lx as usize, lx as usize, 1);
    expand_1d_strided_rev(&mut pix, &mut dt, ly as usize, lx as usize, lx as usize, 1);

    // write back
    for y in 0..ly as usize {
        for x in 0..lx as usize {
            let v = pix[y * lx as usize + x];
            let bytes = v.to_le_bytes();
            unsafe {
                let off = y * wrap + x * 4;
                for i in 0..4 { *ras.add(off + i) = bytes[i]; }
            }
        }
    }
}

fn expand_1d(pix: &mut [u32], dt: &mut [u32], line_len: usize, lines: usize,
             incr_pix: usize, incr_line: usize) {
    // incr_pix/incr_line are in element units (indices into the flat arrays).
    // For horizontal: incr_pix=1, incr_line=line_len.
    // Process each line.
    for l in 0..lines {
        let base = l * incr_line;
        // dt original copy
        let mut odt = vec![0u32; line_len];
        for i in 0..line_len {
            odt[i] = dt[base + i * incr_pix];
        }
        // expand
        let mut dt_ptr = 0usize; // index along line (0..line_len)
        let mut odt_ref = 0usize;
        // iterate
        while dt_ptr < line_len {
            // build range
            let mut d = 1u32;
            let mut d_new = 0usize;
            let mut d_max = u32::MAX;
            let mut dt_i = odt_ref + 1;
            while d <= d_max && dt_i < line_len {
                let new_d_max = takeover_dist(odt[odt_ref], odt[dt_i], d);
                if new_d_max <= d_max {
                    d_new = d as usize;
                    d_max = new_d_max;
                }
                d += 1;
                dt_i += 1;
            }
            let dt_end_line = (odt_ref + d.min(d_max) as usize).min(line_len);
            // convert to flat index
            let dt_end = dt_end_line;
            let odt_new_ref = odt_ref + d_new;

            // process range
            let ref_pix_idx = base + odt_ref * incr_pix; // pix element (flat)
            // pix and dt are flat arrays indexed by element position.
            // pix flat index for (line, pos): base + pos*incr_pix
            let mut d_val = (0usize) as u32; // (pix - ref)/incr_pix; incr_pix=1 so d = pos - odt_ref
            // Actually d = (pix_index - ref_index)/incr_pix
            let mut dt_i2 = dt_ptr;
            let mut pix_i = dt_ptr;
            while dt_i2 < dt_end {
                let dnum = (pix_i as i32 - odt_ref as i32) as u32; // incr_pix=1
                let dd = dnum;
                let new_dt = odt[odt_ref] + (dd as u64 * dd as u64) as u32;
                dt[base + dt_i2 * incr_pix] = new_dt;
                // outFunc: CopyPaint: out.setPaint(in.getPaint())
                let in_val = pix[base + odt_ref * incr_pix];
                let out_val = pix[base + dt_i2 * incr_pix];
                pix[base + dt_i2 * incr_pix] = set_paint(out_val, paint(in_val));
                dt_i2 += 1;
                pix_i += 1;
            }
            dt_ptr = dt_end;
            odt_ref = odt_new_ref.min(line_len.saturating_sub(1));
            if odt_ref >= line_len { break; }
        }
    }
}

fn expand_1d_rev(pix: &mut [u32], dt: &mut [u32], line_len: usize, lines: usize,
                 incr_pix: usize, incr_line: usize) {
    // reverse: process from right to left. Implement by reversing each line,
    // running expand, reversing back. Simpler: just reverse indices.
    // We'll reverse each line in place in a temp, run expand_1d, then reverse back.
    // Actually the C++ calls expand with incrPix=-1. Let's just reverse.
    for l in 0..lines {
        let base = l * incr_line;
        // reverse pix and dt segments
        for i in 0..line_len / 2 {
            pix.swap(base + i * incr_pix, base + (line_len - 1 - i) * incr_pix);
            dt.swap(base + i * incr_pix, base + (line_len - 1 - i) * incr_pix);
        }
    }
    expand_1d(pix, dt, line_len, lines, incr_pix, incr_line);
    for l in 0..lines {
        let base = l * incr_line;
        for i in 0..line_len / 2 {
            pix.swap(base + i * incr_pix, base + (line_len - 1 - i) * incr_pix);
            dt.swap(base + i * incr_pix, base + (line_len - 1 - i) * incr_pix);
        }
    }
}

fn expand_1d_strided(pix: &mut [u32], dt: &mut [u32], line_len: usize, lines: usize,
                     stride: usize, _incr: usize) {
    // lines_count lines, each of length line_len, strided by `stride` between elements along a column.
    // Equivalent to transposing. We'll collect each column into a temp vec, run expand_1d, write back.
    for col in 0..lines {
        // gather column
        let mut col_pix = vec![0u32; line_len];
        let mut col_dt = vec![0u32; line_len];
        for i in 0..line_len {
            col_pix[i] = pix[i * stride + col];
            col_dt[i] = dt[i * stride + col];
        }
        // run expand_1d on a 1-line "image"
        // We need expand_1d to operate on a contiguous slice. Reuse by treating col as a 1-line image.
        // Inline: just call a contiguous version.
        expand_1d_contiguous(&mut col_pix, &mut col_dt, line_len);
        expand_1d_contiguous_rev(&mut col_pix, &mut col_dt, line_len);
        // write back
        for i in 0..line_len {
            pix[i * stride + col] = col_pix[i];
            dt[i * stride + col] = col_dt[i];
        }
    }
}

fn expand_1d_strided_rev(pix: &mut [u32], dt: &mut [u32], line_len: usize, lines: usize,
                        stride: usize, _incr: usize) {
    // Already handled reverse inside strided by calling both. This is a no-op duplicate guard.
    let _ = (pix, dt, line_len, lines, stride);
}

fn expand_1d_contiguous(pix: &mut [u32], dt: &mut [u32], n: usize) {
    if n == 0 { return; }
    let mut odt = vec![0u32; n];
    odt.copy_from_slice(dt);
    let mut dt_ptr = 0usize;
    let mut odt_ref = 0usize;
    while dt_ptr < n {
        let mut d = 1u32;
        let mut d_new = 0usize;
        let mut d_max = u32::MAX;
        let mut dt_i = odt_ref + 1;
        while d <= d_max && dt_i < n {
            let new_d_max = takeover_dist(odt[odt_ref], odt[dt_i], d);
            if new_d_max <= d_max {
                d_new = d as usize;
                d_max = new_d_max;
            }
            d += 1;
            dt_i += 1;
        }
        let dt_end = (odt_ref + (d.min(d_max) as usize)).min(n);
        let odt_new_ref = odt_ref + d_new;
        // process
        let mut di = dt_ptr;
        while di < dt_end {
            let dd = (di as i32 - odt_ref as i32) as u32;
            let new_dt = odt[odt_ref] + dd * dd;
            dt[di] = new_dt;
            let in_val = pix[odt_ref];
            let out_val = pix[di];
            pix[di] = set_paint(out_val, paint(in_val));
            di += 1;
        }
        dt_ptr = dt_end;
        if odt_new_ref >= n { break; }
        odt_ref = odt_new_ref;
    }
}

fn expand_1d_contiguous_rev(pix: &mut [u32], dt: &mut [u32], n: usize) {
    // reverse, expand, reverse back
    for i in 0..n / 2 {
        pix.swap(i, n - 1 - i);
        dt.swap(i, n - 1 - i);
    }
    expand_1d_contiguous(pix, dt, n);
    for i in 0..n / 2 {
        pix.swap(i, n - 1 - i);
        dt.swap(i, n - 1 - i);
    }
}

#[no_mangle]
pub extern "C" fn flare_expand_paint(ras: *mut u8, wrap: i32, lx: i32, ly: i32) {
    expand_paint(ras, wrap.max(0) as usize, lx, ly);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takeover_dist_basic() {
        assert_eq!(takeover_dist(0, 0, 1), 1);
    }

    #[test]
    fn is_inside_some_paint() {
        // tone 0, paint 0 -> not inside
        let v = 0u32; // tone 0, paint 0
        assert!(!is_inside(v));
        // tone 255 (max), paint 0 -> inside (tone != 0)
        let v = 255u32;
        assert!(is_inside(v));
    }

    #[test]
    fn expand_paint_runs() {
        // 4x4 all empty (tone 0, paint 0) except one pixel
        let mut buf = vec![0u8; 4 * 4 * 4];
        // set pixel (1,1) to have tone 255 (inside) with paint 5
        let v: u32 = (5u32 << 8) | 255u32;
        let bytes = v.to_le_bytes();
        let off = (1 * 4 + 1) * 4;
        for i in 0..4 { buf[off + i] = bytes[i]; }
        flare_expand_paint(buf.as_mut_ptr(), 16, 4, 4);
        // smoke: no panic
        let _ = buf;
    }
}