//! Port of flarelib/ikjacobian.cpp: the numeric kernels of the planar IK solver
//! (`VectorRn`, `MatrixRmn` incl. the Golub-Reinsch SVD, and the `Jacobian` solvers:
//! transpose, pseudo-inverse with task priority, damped least squares, DLS via SVD and
//! SDLS), after S. Buss's "Selectively Damped Least Squares" sample code.
//!
//! # Shape of the port
//!
//! The C++ classes stay in `flare/ikjacobian.h`: they own the buffers (`new double[]`),
//! and `Jacobian` walks the `IKSkeleton` / `IKNode` object graph. Every non-inline member
//! of the old .cpp became an inline forwarder that passes raw `(ptr, rows, cols)` triples
//! to the `flare_ikjac_*` entry points below. What runs here is a *literal* transcription
//! of the C++ pointer walks (same loop order, same operation order, no fused multiply-add:
//! Rust never contracts), so results are bit-identical to a C++ build that does not
//! contract either (`-ffp-contract=off`, which is the x86-64 default; clang on arm64 and
//! gcc with `-march` that has FMA may differ by 1 ULP per fused operation).
//!
//! # Safety model
//!
//! Every entry point returns 0 on success, 1 for a null pointer / negative or oversized
//! dimension, 3 for a shape the original only `assert`ed on, and 2 if a bounds check or a
//! panic fired. The pointer walks keep the C++ shape (several of them alias: the SVD
//! expands Householder vectors that live in the matrix being rewritten), so instead of
//! slices every dereference goes through [`rd`]/[`wr`], which check the address against the
//! buffers registered for the current call (caller buffers plus Rust-owned temporaries).
//! An out-of-range access, which the C++ would have performed as undefined behaviour,
//! becomes a caught panic and status 2. Nothing unwinds across the C boundary. (The check
//! is against the union of the call's buffers, so a stray access that lands inside a
//! *different* buffer of the same call is not caught; it never leaves memory the caller
//! handed over, and valid inputs never do it.) Lengths passed by the caller are trusted.
//!
//! # Deviations (only where the C++ was undefined behaviour or could not finish)
//!
//! * Temporaries that the C++ left uninitialised (`new double[]`) start at zero. This only
//!   matters in `CalcDeltaThetasPseudoinverse`, where `VD` keeps unset entries when a
//!   singular value is below the threshold; the reference harness zero-fills `new[]` too.
//! * `ConvertBidiagToDiagonal` was an unbounded `while (true)` (NaN input never converged);
//!   it is capped at `10_000 + 1_000 * n` sweeps and reports status 2.
//! * `CalcDeltaThetasSDLS` computes the "norms of the 3-vectors" over a 2-row Jacobian, so
//!   the original read past the end of `J`. Here that read is a bounds failure (status 2)
//!   after `dTheta` was zeroed; the method is never selected by `IKEngine` (it always uses
//!   DLS).
//! * `assert(...)` conditions are not re-checked except where they protect memory
//!   (`ComputeSVD` shape, `alpha > 0` in the transpose method is not checked).

use std::cell::RefCell;
use std::f64::consts::PI;

const DEG2RAD: f64 = PI / 180.0;
const DEFAULT_DAMPING_LAMBDA: f64 = 1.1;
const PSEUDO_INVERSE_THRESHOLD_FACTOR: f64 = 0.001;
const MAX_ANGLE_JTRANSPOSE: f64 = 30.0 * DEG2RAD;
const MAX_ANGLE_PSEUDOINVERSE: f64 = 5.0 * DEG2RAD;
const MAX_ANGLE_DLS: f64 = 5.0 * DEG2RAD;
const MAX_ANGLE_SDLS: f64 = 45.0 * DEG2RAD;
const BASE_MAX_TARGET_DIST: f64 = 3.4;

/// Largest element count accepted for one buffer (the C ABI hands us lengths, not slices).
const MAX_ELEMS: i64 = 1 << 36;

// ---------------------------------------------------------------------------------------
// Checked raw access
// ---------------------------------------------------------------------------------------

thread_local! {
    static REG: RefCell<Vec<(usize, usize)>> = const { RefCell::new(Vec::new()) };
}

fn reg_clear() {
    REG.with(|r| r.borrow_mut().clear());
}
fn reg_add(p: *const f64, n: usize) {
    REG.with(|r| r.borrow_mut().push((p as usize, n)));
}
fn reg_remove(p: *const f64) {
    REG.with(|r| {
        let mut r = r.borrow_mut();
        if let Some(i) = r.iter().rposition(|e| e.0 == p as usize) {
            r.remove(i);
        }
    });
}
fn inside(p: *const f64) -> bool {
    let a = p as usize;
    REG.with(|r| r.borrow().iter().any(|&(s, n)| a >= s && a - s < n * 8 && (a - s) % 8 == 0))
}

#[inline]
unsafe fn rd(p: *const f64) -> f64 {
    assert!(inside(p), "ikjacobian: read outside the registered buffers");
    *p
}
#[inline]
unsafe fn wr(p: *mut f64, v: f64) {
    assert!(inside(p), "ikjacobian: write outside the registered buffers");
    *p = v;
}
/// Pointer arithmetic that may leave the buffer (the C++ did, one past the end); only
/// dereferences are checked.
#[inline]
fn off(p: *mut f64, k: i64) -> *mut f64 {
    p.wrapping_offset(k as isize)
}

/// A Rust-owned, zero-initialised, registered scratch buffer.
struct Buf {
    v: Vec<f64>,
}
impl Buf {
    fn new(n: i64) -> Buf {
        assert!((0..=MAX_ELEMS).contains(&n));
        let mut v = vec![0.0f64; n as usize];
        if n > 0 {
            reg_add(v.as_mut_ptr(), n as usize);
        }
        Buf { v }
    }
    fn p(&mut self) -> *mut f64 {
        self.v.as_mut_ptr()
    }
}
impl Drop for Buf {
    fn drop(&mut self) {
        if !self.v.is_empty() {
            reg_remove(self.v.as_ptr());
        }
    }
}

type R = Result<(), i32>;
const E_ARG: i32 = 1;
const E_RANGE: i32 = 2;
const E_SHAPE: i32 = 3;

/// Run an entry point body with a fresh registry; map the outcome to a status code.
fn run(f: impl FnOnce() -> R) -> i32 {
    reg_clear();
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    reg_clear();
    match r {
        Ok(Ok(())) => 0,
        Ok(Err(c)) => c,
        Err(_) => E_RANGE,
    }
}

#[derive(Clone, Copy)]
struct Mx {
    x: *mut f64,
    r: i64,
    c: i64,
}
#[derive(Clone, Copy)]
struct Vn {
    x: *mut f64,
    n: i64,
}

unsafe fn mat(x: *mut f64, r: i64, c: i64) -> Result<Mx, i32> {
    if r < 0 || c < 0 {
        return Err(E_ARG);
    }
    let n = r.checked_mul(c).ok_or(E_ARG)?;
    if n > MAX_ELEMS {
        return Err(E_ARG);
    }
    if n > 0 {
        if x.is_null() {
            return Err(E_ARG);
        }
        reg_add(x, n as usize);
    }
    Ok(Mx { x, r, c })
}
unsafe fn vecr(x: *mut f64, n: i64) -> Result<Vn, i32> {
    if !(0..=MAX_ELEMS).contains(&n) {
        return Err(E_ARG);
    }
    if n > 0 {
        if x.is_null() {
            return Err(E_ARG);
        }
        reg_add(x, n as usize);
    }
    Ok(Vn { x, n })
}
/// Scratch matrix (zero-initialised).
fn tmp(r: i64, c: i64) -> (Buf, Mx) {
    let mut b = Buf::new(r * c);
    let x = b.p();
    (b, Mx { x, r, c })
}
fn tmpv(n: i64) -> (Buf, Vn) {
    let mut b = Buf::new(n);
    let x = b.p();
    (b, Vn { x, n })
}

#[inline]
fn square(x: f64) -> f64 {
    x * x
}
#[inline]
fn near_zero(x: f64, tol: f64) -> bool {
    x.abs() <= tol
}
/// `std::min(a, b)` (returns `a` unless `b < a`).
#[inline]
fn std_min(a: f64, b: f64) -> f64 {
    if b < a {
        b
    } else {
        a
    }
}
/// `std::max(a, b)` (returns `a` unless `a < b`).
#[inline]
fn std_max(a: f64, b: f64) -> f64 {
    if a < b {
        b
    } else {
        a
    }
}

unsafe fn dot_array(mut len: i64, mut a: *mut f64, sa: i64, mut b: *mut f64, sb: i64) -> f64 {
    let mut result = 0.0;
    while len > 0 {
        result += rd(a) * rd(b);
        a = off(a, sa);
        b = off(b, sb);
        len -= 1;
    }
    result
}
unsafe fn copy_array_scale(mut len: i64, mut from: *mut f64, fs: i64, mut to: *mut f64, ts: i64, scale: f64) {
    while len > 0 {
        wr(to, rd(from) * scale);
        from = off(from, fs);
        to = off(to, ts);
        len -= 1;
    }
}
unsafe fn add_array_scale(mut len: i64, mut from: *mut f64, fs: i64, mut to: *mut f64, ts: i64, scale: f64) {
    while len > 0 {
        wr(to, rd(to) + rd(from) * scale);
        from = off(from, fs);
        to = off(to, ts);
        len -= 1;
    }
}

// ---------------------------------------------------------------------------------------
// VectorRn
// ---------------------------------------------------------------------------------------

impl Vn {
    unsafe fn max_abs(self) -> f64 {
        let mut result = 0.0;
        let mut t = self.x;
        let mut i = self.n;
        while i > 0 {
            if rd(t) > result {
                result = rd(t);
            } else if -rd(t) > result {
                result = -rd(t);
            }
            t = off(t, 1);
            i -= 1;
        }
        result
    }
    unsafe fn norm_sq(self) -> f64 {
        let mut t = self.x;
        let mut res = 0.0;
        let mut i = self.n;
        while i > 0 {
            res += rd(t) * rd(t);
            t = off(t, 1);
            i -= 1;
        }
        res
    }
    unsafe fn set_zero(self) {
        let mut t = self.x;
        let mut i = self.n;
        while i > 0 {
            wr(t, 0.0);
            t = off(t, 1);
            i -= 1;
        }
    }
    unsafe fn scale(self, f: f64) {
        let mut t = self.x;
        let mut i = self.n;
        while i > 0 {
            wr(t, rd(t) * f);
            t = off(t, 1);
            i -= 1;
        }
    }
    unsafe fn load_scaled(self, d: *mut f64, f: f64) {
        let mut to = self.x;
        let mut from = d;
        let mut i = self.n;
        while i > 0 {
            wr(to, rd(from) * f);
            to = off(to, 1);
            from = off(from, 1);
            i -= 1;
        }
    }
    unsafe fn add_scaled(self, src: Vn, f: f64) {
        let mut to = self.x;
        let mut from = src.x;
        let mut i = self.n;
        while i > 0 {
            wr(to, rd(to) + rd(from) * f);
            to = off(to, 1);
            from = off(from, 1);
            i -= 1;
        }
    }
}

/// `Dot(u, v)` of the header.
unsafe fn dot(u: Vn, v: Vn) -> f64 {
    let mut res = 0.0;
    let mut p = u.x;
    let mut q = v.x;
    let mut i = u.n;
    while i > 0 {
        res += rd(p) * rd(q);
        p = off(p, 1);
        q = off(q, 1);
        i -= 1;
    }
    res
}

// ---------------------------------------------------------------------------------------
// MatrixRmn (column-major)
// ---------------------------------------------------------------------------------------

impl Mx {
    unsafe fn set_zero(self) {
        let mut t = self.x;
        let mut i = self.r * self.c;
        while i > 0 {
            wr(t, 0.0);
            t = off(t, 1);
            i -= 1;
        }
    }
    unsafe fn col_ptr(self, j: i64) -> *mut f64 {
        off(self.x, j * self.r)
    }
    unsafe fn get(self, i: i64, j: i64) -> f64 {
        rd(off(self.x, j * self.r + i))
    }
    unsafe fn set(self, i: i64, j: i64, v: f64) {
        wr(off(self.x, j * self.r + i), v)
    }
    unsafe fn set_diagonal(self, d: f64) {
        let mut diag_len = self.r.min(self.c);
        let mut p = self.x;
        while diag_len > 0 {
            wr(p, d);
            p = off(p, self.r + 1);
            diag_len -= 1;
        }
    }
    unsafe fn set_diagonal_vec(self, d: Vn) {
        let mut diag_len = self.r.min(self.c);
        let mut p = self.x;
        let mut from = d.x;
        while diag_len > 0 {
            wr(p, rd(from));
            from = off(from, 1);
            p = off(p, self.r + 1);
            diag_len -= 1;
        }
    }
    unsafe fn set_super_diagonal(self, d: f64) {
        let mut len = self.r.min(self.c - 1);
        let mut to = off(self.x, self.r);
        while len > 0 {
            wr(to, d);
            to = off(to, self.r + 1);
            len -= 1;
        }
    }
    unsafe fn set_super_diagonal_vec(self, d: Vn) {
        let mut len = (self.r - 1).min(self.c);
        let mut to = off(self.x, self.r);
        let mut from = d.x;
        while len > 0 {
            wr(to, rd(from));
            from = off(from, 1);
            to = off(to, self.r + 1);
            len -= 1;
        }
    }
    unsafe fn set_sub_diagonal(self, d: f64) {
        let mut len = self.r.min(self.c) - 1;
        let mut to = off(self.x, 1);
        while len > 0 {
            wr(to, d);
            to = off(to, self.r + 1);
            len -= 1;
        }
    }
    unsafe fn set_sub_diagonal_vec(self, d: Vn) {
        let mut len = self.r.min(self.c) - 1;
        let mut to = off(self.x, 1);
        let mut from = d.x;
        while len > 0 {
            wr(to, rd(from));
            from = off(from, 1);
            to = off(to, self.r + 1);
            len -= 1;
        }
    }
    unsafe fn set_column(self, i: i64, d: Vn) {
        let mut to = off(self.x, i * self.r);
        let mut from = d.x;
        let mut k = self.r;
        while k > 0 {
            wr(to, rd(from));
            to = off(to, 1);
            from = off(from, 1);
            k -= 1;
        }
    }
    unsafe fn set_row(self, i: i64, d: Vn) {
        let mut to = off(self.x, i);
        let mut from = d.x;
        let mut k = self.r;
        while k > 0 {
            wr(to, rd(from));
            from = off(from, 1);
            to = off(to, self.r);
            k -= 1;
        }
    }
    unsafe fn set_sequence(self, d: Vn, start_row: i64, start_col: i64, delta_row: i64, delta_col: i64) {
        let mut length = d.n;
        let mut to = off(self.x, start_row + self.r * start_col);
        let mut from = d.x;
        let stride = delta_row + self.r * delta_col;
        while length > 0 {
            wr(to, rd(from));
            from = off(from, 1);
            to = off(to, stride);
            length -= 1;
        }
    }
    unsafe fn load_as_submatrix(self, a: Mx) {
        let extra_col_step = self.r - a.r;
        let mut to = self.x;
        let mut from = a.x;
        let mut i = a.c;
        while i > 0 {
            let mut j = a.r;
            while j > 0 {
                wr(to, rd(from));
                to = off(to, 1);
                from = off(from, 1);
                j -= 1;
            }
            to = off(to, extra_col_step);
            i -= 1;
        }
    }
    unsafe fn load_as_submatrix_transpose(self, a: Mx) {
        let mut row_ptr = self.x;
        let mut from = a.x;
        let mut i = a.c;
        while i > 0 {
            let mut to = row_ptr;
            let mut j = a.r;
            while j > 0 {
                wr(to, rd(from));
                from = off(from, 1);
                to = off(to, self.r);
                j -= 1;
            }
            row_ptr = off(row_ptr, 1);
            i -= 1;
        }
    }
    unsafe fn frobenius_norm_sq(self) -> f64 {
        let mut p = self.x;
        let mut result = 0.0;
        let mut i = self.r * self.c;
        while i > 0 {
            result += square(rd(p));
            p = off(p, 1);
            i -= 1;
        }
        result
    }
    unsafe fn frobenius_norm(self) -> f64 {
        self.frobenius_norm_sq().sqrt()
    }
    /// `result = (this) * v`
    unsafe fn multiply_vec(self, v: Vn, result: Vn) {
        let mut out = result.x;
        let mut row_ptr = self.x;
        let mut j = self.r;
        while j > 0 {
            let mut inp = v.x;
            let mut m = row_ptr;
            row_ptr = off(row_ptr, 1);
            wr(out, 0.0);
            let mut i = self.c;
            while i > 0 {
                wr(out, rd(out) + rd(inp) * rd(m));
                inp = off(inp, 1);
                m = off(m, self.r);
                i -= 1;
            }
            out = off(out, 1);
            j -= 1;
        }
    }
    /// `result = (this)^T * v`
    unsafe fn multiply_transpose_vec(self, v: Vn, result: Vn) {
        let mut out = result.x;
        let mut col_ptr = self.x;
        let mut i = self.c;
        while i > 0 {
            let mut inp = v.x;
            wr(out, 0.0);
            let mut j = self.r;
            while j > 0 {
                wr(out, rd(out) + rd(inp) * rd(col_ptr));
                inp = off(inp, 1);
                col_ptr = off(col_ptr, 1);
                j -= 1;
            }
            out = off(out, 1);
            i -= 1;
        }
    }
    unsafe fn dot_product_column(self, v: Vn, col: i64) -> f64 {
        let mut pc = off(self.x, col * self.r);
        let mut pv = v.x;
        let mut ret = 0.0;
        let mut i = self.r;
        while i > 0 {
            ret += rd(pc) * rd(pv);
            pc = off(pc, 1);
            pv = off(pv, 1);
            i -= 1;
        }
        ret
    }
    unsafe fn add_to_diagonal(self, d: f64) {
        let mut diag_len = self.r.min(self.c);
        let mut p = self.x;
        while diag_len > 0 {
            wr(p, rd(p) + d);
            p = off(p, self.r + 1);
            diag_len -= 1;
        }
    }
    unsafe fn add_to_diagonal_vec(self, v: Vn) {
        let mut diag_len = self.r.min(self.c);
        let mut p = self.x;
        let mut dv = v.x;
        while diag_len > 0 {
            wr(p, rd(p) + rd(dv));
            dv = off(dv, 1);
            p = off(p, self.r + 1);
            diag_len -= 1;
        }
    }
    unsafe fn sub_assign(self, b: Mx) {
        let mut a = self.x;
        let mut bp = b.x;
        let mut i = self.r * self.c;
        while i > 0 {
            wr(a, rd(a) - rd(bp));
            a = off(a, 1);
            bp = off(bp, 1);
            i -= 1;
        }
    }
    unsafe fn set_identity(self) {
        self.set_zero();
        self.set_diagonal(1.0);
    }

    /// `MatrixRmn::MultiplyScalar` (including the original's habit of restarting at the
    /// first column of `A` for every output column).
    unsafe fn multiply_scalar(a: Mx, k: f64, dst: Mx) {
        let mut d = dst.x;
        let mut i = dst.c;
        while i > 0 {
            let mut ap = a.x;
            let mut j = dst.r;
            while j > 0 {
                wr(d, rd(ap) * k);
                d = off(d, 1);
                ap = off(ap, 1);
                j -= 1;
            }
            i -= 1;
        }
    }
    unsafe fn multiply(a: Mx, b: Mx, dst: Mx) {
        let length = a.c;
        let mut bp = b.x;
        let mut d = dst.x;
        let mut i = dst.c;
        while i > 0 {
            let mut ap = a.x;
            let mut j = dst.r;
            while j > 0 {
                wr(d, dot_array(length, ap, a.r, bp, 1));
                d = off(d, 1);
                ap = off(ap, 1);
                j -= 1;
            }
            bp = off(bp, b.r);
            i -= 1;
        }
    }
    unsafe fn transpose_multiply(a: Mx, b: Mx, dst: Mx) {
        let length = a.r;
        let mut bp = b.x;
        let mut d = dst.x;
        let mut i = dst.c;
        while i > 0 {
            let mut ap = a.x;
            let mut j = dst.r;
            while j > 0 {
                wr(d, dot_array(length, ap, 1, bp, 1));
                d = off(d, 1);
                ap = off(ap, a.r);
                j -= 1;
            }
            bp = off(bp, b.r);
            i -= 1;
        }
    }
    unsafe fn multiply_transpose(a: Mx, b: Mx, dst: Mx) {
        let length = a.c;
        let mut bp = b.x;
        let mut d = dst.x;
        let mut i = dst.c;
        while i > 0 {
            let mut ap = a.x;
            let mut j = dst.r;
            while j > 0 {
                wr(d, dot_array(length, ap, a.r, bp, b.r));
                d = off(d, 1);
                ap = off(ap, 1);
                j -= 1;
            }
            bp = off(bp, 1);
            i -= 1;
        }
    }

    /// Row echelon form by pivoting row operations; assumes no free variables.
    unsafe fn convert_to_ref_no_free(self) {
        let mut num_iters = self.r.min(self.c);
        let mut row_ptr1 = self.x;
        let diag_step = self.r + 1;
        let mut len_row_left = self.c;
        while num_iters > 1 {
            let mut row_ptr2 = row_ptr1;
            let mut max_abs = rd(row_ptr1).abs();
            let mut row_pivot = row_ptr1;
            let mut i = num_iters - 1;
            while i > 0 {
                row_pivot = off(row_pivot, 1);
                let new_max = rd(row_pivot);
                if new_max > max_abs {
                    max_abs = rd(row_pivot);
                    row_ptr2 = row_pivot;
                } else if -new_max > max_abs {
                    max_abs = -new_max;
                    row_ptr2 = row_pivot;
                }
                i -= 1;
            }
            if row_ptr1 != row_ptr2 {
                let mut to = row_ptr1;
                let mut k = len_row_left;
                while k > 0 {
                    let temp = rd(to);
                    wr(to, rd(row_ptr2));
                    wr(row_ptr2, temp);
                    to = off(to, self.r);
                    row_ptr2 = off(row_ptr2, self.r);
                    k -= 1;
                }
            }
            row_ptr2 = row_ptr1;
            let mut i = num_iters - 1;
            while i > 0 {
                row_ptr2 = off(row_ptr2, 1);
                let mut to = row_ptr2;
                let mut from = row_ptr1;
                let alpha = rd(to) / rd(from);
                wr(to, 0.0);
                let mut j = len_row_left - 1;
                while j > 0 {
                    to = off(to, self.r);
                    from = off(from, self.r);
                    wr(to, rd(to) - rd(from) * alpha);
                    j -= 1;
                }
                i -= 1;
            }
            row_ptr1 = off(row_ptr1, diag_step);
            len_row_left -= 1;
            num_iters -= 1;
        }
    }

    /// Solves `(this) * x = b` (square, invertible) with row operations.
    unsafe fn solve(self, b: Vn, xvec: Vn) {
        let (_aug_buf, aug) = tmp(self.r, self.c + 1);
        aug.load_as_submatrix(self);
        aug.set_column(self.r, b);
        aug.convert_to_ref_no_free();

        let x_last = off(xvec.x, self.r - 1);
        let mut end_row = off(aug.x, self.r * self.c - 1);
        let mut b_ptr = off(end_row, self.r);
        let mut i = self.r;
        while i > 0 {
            let mut accum = rd(b_ptr);
            b_ptr = off(b_ptr, -1);
            let mut row_ptr = end_row;
            let mut x_ptr = x_last;
            let mut j = self.r - i;
            while j > 0 {
                accum -= rd(row_ptr) * rd(x_ptr);
                x_ptr = off(x_ptr, -1);
                row_ptr = off(row_ptr, -self.c);
                j -= 1;
            }
            wr(x_ptr, accum / rd(row_ptr));
            end_row = off(end_row, -1);
            i -= 1;
        }
    }

    unsafe fn post_apply_givens(self, c: f64, s: f64, idx1: i64, idx2: i64) {
        let mut col_a = off(self.x, idx1 * self.r);
        let mut col_b = off(self.x, idx2 * self.r);
        let mut i = self.r;
        while i > 0 {
            let temp = rd(col_a);
            wr(col_a, rd(col_a) * c + rd(col_b) * s);
            wr(col_b, rd(col_b) * c - temp * s);
            col_a = off(col_a, 1);
            col_b = off(col_b, 1);
            i -= 1;
        }
    }
}

/// `MatrixRmn::CalcGivensValues`
fn calc_givens_values(a: f64, b: f64) -> (f64, f64) {
    let mut denom_inv = (a * a + b * b).sqrt();
    if denom_inv == 0.0 {
        (1.0, 0.0)
    } else {
        denom_inv = 1.0 / denom_inv;
        (a * denom_inv, -b * denom_inv)
    }
}

// ---------------------------------------------------------------------------------------
// SVD
// ---------------------------------------------------------------------------------------

unsafe fn svd_householder(
    base_pt: *mut f64,
    col_length: i64,
    num_cols: i64,
    col_stride: i64,
    row_stride: i64,
    ret_first_entry: *mut f64,
) {
    let mut c_ptr = base_pt;
    let mut norm = 0.0;
    let mut i = col_length;
    while i > 0 {
        norm += square(rd(c_ptr));
        c_ptr = off(c_ptr, col_stride);
        i -= 1;
    }
    norm = norm.sqrt();

    let image_val;
    if rd(base_pt) < 0.0 {
        image_val = norm;
        norm = 2.0 * norm * (norm - rd(base_pt));
    } else {
        image_val = -norm;
        norm = 2.0 * norm * (norm + rd(base_pt));
    }
    norm = norm.sqrt();

    if norm == 0.0 {
        c_ptr = base_pt;
        let mut i = col_length;
        while i > 0 {
            wr(c_ptr, 0.0);
            c_ptr = off(c_ptr, col_stride);
            i -= 1;
        }
        wr(ret_first_entry, 0.0);
        return;
    }

    wr(ret_first_entry, image_val);
    wr(base_pt, rd(base_pt) - image_val);
    norm = 1.0 / norm;
    c_ptr = base_pt;
    let mut i = col_length;
    while i > 0 {
        wr(c_ptr, rd(c_ptr) * norm);
        c_ptr = off(c_ptr, col_stride);
        i -= 1;
    }

    let mut r_ptr = base_pt;
    let mut j = num_cols - 1;
    while j > 0 {
        r_ptr = off(r_ptr, row_stride);
        let dot_p = dot_array(col_length, base_pt, col_stride, r_ptr, col_stride);
        add_array_scale(col_length, base_pt, col_stride, r_ptr, col_stride, -2.0 * dot_p);
        j -= 1;
    }
}

impl Mx {
    unsafe fn expand_householders(
        self,
        num_xforms: i64,
        num_zeros_skipped: i64,
        base_pt: *mut f64,
        col_stride: i64,
        row_stride: i64,
    ) {
        let mut num_to_transform = self.c - num_xforms + 1 - num_zeros_skipped;
        if num_xforms == 0 {
            self.set_identity();
            return;
        }
        let h_diag_stride = row_stride + col_stride;
        let mut h_base = off(base_pt, h_diag_stride * (num_xforms - 1));
        let mut h_diag_ptr = off(h_base, col_stride * (num_to_transform - 1));
        let mut diag_ptr = off(self.x, self.c * self.r - 1);
        let mut col_ptr = off(diag_ptr, -(num_to_transform - 1));
        let mut i = num_to_transform;
        while i > 0 {
            copy_array_scale(num_to_transform, h_base, col_stride, col_ptr, 1, -2.0 * rd(h_diag_ptr));
            wr(diag_ptr, rd(diag_ptr) + 1.0);
            diag_ptr = off(diag_ptr, -(self.r + 1));
            col_ptr = off(col_ptr, -self.r);
            h_diag_ptr = off(h_diag_ptr, -col_stride);
            i -= 1;
        }

        let mut col_last_ptr = off(self.x, self.r * self.c - num_to_transform - 1);
        let mut i = num_xforms - 1;
        while i > 0 {
            num_to_transform += 1;
            h_base = off(h_base, -h_diag_stride);
            col_ptr = col_last_ptr;
            let mut j = num_to_transform - 1;
            while j > 0 {
                let dot_prod_2n =
                    -2.0 * dot_array(num_to_transform - 1, off(h_base, col_stride), col_stride, off(col_ptr, 1), 1);
                wr(col_ptr, dot_prod_2n * rd(h_base));
                add_array_scale(num_to_transform - 1, off(h_base, col_stride), col_stride, off(col_ptr, 1), 1, dot_prod_2n);
                col_ptr = off(col_ptr, -self.r);
                j -= 1;
            }
            copy_array_scale(num_to_transform, h_base, col_stride, col_ptr, 1, -2.0 * rd(h_base));
            wr(col_ptr, rd(col_ptr) + 1.0);
            col_last_ptr = off(col_last_ptr, -1);
            i -= 1;
        }

        if num_zeros_skipped != 0 {
            let mut d = self.x;
            wr(d, 1.0);
            let mut d2 = d;
            let mut i = self.r - 1;
            while i > 0 {
                d = off(d, 1);
                wr(d, 0.0);
                d2 = off(d2, self.r);
                wr(d2, 0.0);
                i -= 1;
            }
        }
    }
}

unsafe fn calc_bidiagonal(u: Mx, v: Mx, w: Vn, super_diag: Vn) {
    let row_step = u.c;
    let diag_step = u.c + 1;
    let mut diag_ptr = u.x;
    let mut w_ptr = w.x;
    let mut sd_ptr = super_diag.x;
    let mut col_length_left = u.r;
    let mut row_length_left = v.c;
    loop {
        svd_householder(diag_ptr, col_length_left, row_length_left, 1, row_step, w_ptr);
        if row_length_left == 2 {
            wr(sd_ptr, rd(off(diag_ptr, row_step)));
            break;
        }
        svd_householder(off(diag_ptr, row_step), row_length_left - 1, col_length_left, row_step, 1, sd_ptr);
        row_length_left -= 1;
        col_length_left -= 1;
        diag_ptr = off(diag_ptr, diag_step);
        w_ptr = off(w_ptr, 1);
        sd_ptr = off(sd_ptr, 1);
    }

    let mut extra = 0;
    diag_ptr = off(diag_ptr, diag_step);
    w_ptr = off(w_ptr, 1);
    if col_length_left > 2 {
        extra = 1;
        col_length_left -= 1;
        svd_householder(diag_ptr, col_length_left, 1, 1, 0, w_ptr);
    } else {
        wr(w_ptr, rd(diag_ptr));
    }

    v.expand_householders(v.c - 2, 1, off(u.x, u.r), u.r, 1);
    u.expand_householders(v.c - 1 + extra, 0, u.x, 1, u.r);
}

/// `ApplyGivensCBTD` with four pointers (matrix in column order).
unsafe fn apply_givens_4(cosine: f64, sine: f64, a: *mut f64, b: *mut f64, c: *mut f64, d: *mut f64) {
    let mut temp = rd(a);
    wr(a, cosine * rd(a) - sine * rd(b));
    wr(b, sine * temp + cosine * rd(b));
    temp = rd(c);
    wr(c, cosine * rd(c) - sine * rd(d));
    wr(d, sine * temp + cosine * rd(d));
}
/// `ApplyGivensCBTD` with a by-value `d` (matrix in row order, `c` assumed zero).
#[allow(clippy::too_many_arguments)]
unsafe fn apply_givens_6(cosine: f64, sine: f64, a: *mut f64, b: *mut f64, c: *mut f64, d: f64, e: *mut f64, f: *mut f64) {
    wr(a, cosine * rd(a) - sine * d);
    let temp = rd(b);
    wr(b, cosine * rd(b) - sine * rd(e));
    wr(e, sine * temp + cosine * rd(e));
    wr(c, -sine * rd(f));
    wr(f, cosine * rd(f));
}

unsafe fn update_bidiag_indices(first: &mut i64, last: &mut i64, w: Vn, super_diag: Vn, eps: f64) -> bool {
    let mut last_idx = *last;
    let mut sd_ptr = off(super_diag.x, last_idx - 1);
    while near_zero(rd(sd_ptr), eps) {
        wr(sd_ptr, 0.0);
        sd_ptr = off(sd_ptr, -1);
        last_idx -= 1;
        if last_idx == 0 {
            return false;
        }
    }
    *last = last_idx;
    let mut first_idx = last_idx - 1;
    let mut w_ptr = off(w.x, first_idx);
    while first_idx > 0 {
        if near_zero(rd(w_ptr), eps) {
            wr(w_ptr, 0.0);
            break;
        }
        sd_ptr = off(sd_ptr, -1);
        if near_zero(rd(sd_ptr), eps) {
            wr(sd_ptr, 0.0);
            break;
        }
        w_ptr = off(w_ptr, -1);
        first_idx -= 1;
    }
    *first = first_idx;
    true
}

unsafe fn clear_row_with_diagonal_zero(first: i64, last: i64, u: Mx, mut w_ptr: *mut f64, mut sd_ptr: *mut f64) {
    let mut cur_sd = rd(sd_ptr);
    wr(sd_ptr, 0.0);
    let mut i = first + 1;
    loop {
        w_ptr = off(w_ptr, 1);
        let (c, s) = calc_givens_values(rd(w_ptr), cur_sd);
        u.post_apply_givens(c, -s, i, first);
        wr(w_ptr, c * rd(w_ptr) - s * cur_sd);
        if i == last {
            break;
        }
        sd_ptr = off(sd_ptr, 1);
        cur_sd = s * rd(sd_ptr);
        wr(sd_ptr, c * rd(sd_ptr));
        i += 1;
    }
}

unsafe fn clear_column_with_diagonal_zero(end_idx: i64, v: Mx, mut w_ptr: *mut f64, mut sd_ptr: *mut f64, eps: f64) {
    let mut cur_sd = rd(sd_ptr);
    wr(sd_ptr, 0.0);
    let mut i = end_idx - 1;
    loop {
        w_ptr = off(w_ptr, -1);
        let (c, s) = calc_givens_values(rd(w_ptr), cur_sd);
        v.post_apply_givens(c, -s, i, end_idx);
        wr(w_ptr, c * rd(w_ptr) - s * cur_sd);
        if i == 0 {
            break;
        }
        sd_ptr = off(sd_ptr, -1);
        cur_sd = s * rd(sd_ptr);
        if near_zero(cur_sd, eps) {
            break;
        }
        wr(sd_ptr, c * rd(sd_ptr));
        i -= 1;
    }
}

unsafe fn convert_bidiag_to_diagonal(u: Mx, v: Mx, w: Vn, super_diag: Vn) -> R {
    let mut last_bidiag_idx = v.r - 1;
    let mut first_bidiag_idx = 0i64;
    let eps = 1.0e-15 * std_max(w.max_abs(), super_diag.max_abs());
    let (mut ex_buf, _) = tmpv(1);
    let extra = ex_buf.p();
    let mut sweeps: i64 = 0;
    let cap = 10_000 + 1_000 * w.n;

    loop {
        let work_left = update_bidiag_indices(&mut first_bidiag_idx, &mut last_bidiag_idx, w, super_diag, eps);
        if !work_left {
            break;
        }
        sweeps += 1;
        if sweeps > cap {
            return Err(E_RANGE);
        }

        let mut w_ptr = off(w.x, first_bidiag_idx);
        let mut sd_ptr = off(super_diag.x, first_bidiag_idx);
        wr(extra, 0.0);
        if rd(w_ptr) == 0.0 {
            clear_row_with_diagonal_zero(first_bidiag_idx, last_bidiag_idx, u, w_ptr, sd_ptr);
            if first_bidiag_idx > 0 {
                sd_ptr = off(sd_ptr, -1);
                if near_zero(rd(sd_ptr), eps) {
                    wr(sd_ptr, 0.0);
                } else {
                    clear_column_with_diagonal_zero(first_bidiag_idx, v, w_ptr, sd_ptr, eps);
                }
            }
            continue;
        }

        let sdi = |k: i64| rd(off(super_diag.x, k));
        let wi = |k: i64| rd(off(w.x, k));
        let mut a = if first_bidiag_idx < last_bidiag_idx - 1 { square(sdi(last_bidiag_idx - 2)) } else { 0.0 };
        let mut b_sq = square(wi(last_bidiag_idx - 1));
        a += b_sq;
        let mut cc = square(sdi(last_bidiag_idx - 1));
        b_sq *= cc;
        cc += square(wi(last_bidiag_idx));
        let mut lambda = (square((a - cc) * 0.5) + b_sq).sqrt();
        if a > cc {
            lambda = -lambda;
        }
        lambda += (a + cc) * 0.5;
        let t11 = square(wi(first_bidiag_idx));
        let t12 = wi(first_bidiag_idx) * sdi(first_bidiag_idx);

        let (mut c, mut s) = calc_givens_values(t11 - lambda, t12);
        apply_givens_4(c, s, w_ptr, sd_ptr, extra, off(w_ptr, 1));
        v.post_apply_givens(c, -s, first_bidiag_idx, first_bidiag_idx + 1);
        let mut i = first_bidiag_idx;
        while i < last_bidiag_idx - 1 {
            (c, s) = calc_givens_values(rd(w_ptr), rd(extra));
            apply_givens_6(c, s, w_ptr, sd_ptr, extra, rd(extra), off(w_ptr, 1), off(sd_ptr, 1));
            u.post_apply_givens(c, -s, i, i + 1);
            (c, s) = calc_givens_values(rd(sd_ptr), rd(extra));
            apply_givens_6(c, s, sd_ptr, off(w_ptr, 1), extra, rd(extra), off(sd_ptr, 1), off(w_ptr, 2));
            v.post_apply_givens(c, -s, i + 1, i + 2);
            w_ptr = off(w_ptr, 1);
            sd_ptr = off(sd_ptr, 1);
            i += 1;
        }
        (c, s) = calc_givens_values(rd(w_ptr), rd(extra));
        apply_givens_4(c, s, w_ptr, extra, sd_ptr, off(w_ptr, 1));
        u.post_apply_givens(c, -s, i, i + 1);
    }
    Ok(())
}

impl Mx {
    /// `ComputeSVD`: `(this) = U * diag(w) * V^T`.
    unsafe fn compute_svd(self, u: Mx, w: Vn, v: Mx) -> R {
        // The asserts of the original; ComputeSVD also needs a non-empty super-diagonal.
        if !(u.r == self.r && v.c == self.c && u.r == u.c && v.r == v.c && w.n == self.r.min(self.c)) || w.n < 2 {
            return Err(E_SHAPE);
        }
        let (_sd_buf, super_diag) = tmpv(w.n - 1);
        let (left, right);
        if self.r >= self.c {
            u.load_as_submatrix(self);
            left = u;
            right = v;
        } else {
            v.load_as_submatrix_transpose(self);
            left = v;
            right = u;
        }
        calc_bidiagonal(left, right, w, super_diag);
        convert_bidiag_to_diagonal(left, right, w, super_diag)
    }

    /// `DebugCheckSVD` without the final `assert`; returns the verdict.
    unsafe fn debug_check_svd(self, u: Mx, w: Vn, v: Mx) -> bool {
        let (_b1, iv) = tmp(v.r, v.c);
        iv.set_identity();
        let (_b2, vtv) = tmp(v.r, v.c);
        Mx::transpose_multiply(v, v, vtv);
        iv.sub_assign(vtv);
        let mut error = iv.frobenius_norm();

        let (_b3, iu) = tmp(u.r, u.c);
        iu.set_identity();
        let (_b4, utu) = tmp(u.r, u.c);
        Mx::transpose_multiply(u, u, utu);
        iu.sub_assign(utu);
        error += iu.frobenius_norm();

        let (_b5, diag) = tmp(u.r, v.r);
        diag.set_zero();
        diag.set_diagonal_vec(w);
        let (_b6, b) = tmp(u.r, v.r);
        let (_b7, c) = tmp(u.r, v.r);
        Mx::multiply(u, diag, b);
        Mx::multiply_transpose(b, v, c);
        c.sub_assign(self);
        error += c.frobenius_norm();

        error.abs() <= 1.0e-13 * w.max_abs()
    }
}

// ---------------------------------------------------------------------------------------
// Jacobian solvers
// ---------------------------------------------------------------------------------------

/// `Jacobian::CalcDeltaThetasTranspose`
unsafe fn calc_transpose(j: Mx, ds: Vn, dt: Vn, dtheta: Vn) {
    j.multiply_transpose_vec(ds, dtheta);
    j.multiply_vec(dtheta, dt);
    let alpha = dot(ds, dt) / dt.norm_sq();
    let max_change = dtheta.max_abs();
    let beta = MAX_ANGLE_JTRANSPOSE / max_change;
    dtheta.scale(std_min(alpha, beta));
}

/// `Jacobian::CalcDeltaThetasDLS`
unsafe fn calc_dls(j: Mx, u: Mx, damp_sq_v: Vn, ds: Vn, dt: Vn, dtheta: Vn) {
    Mx::multiply_transpose(j, j, u);
    u.add_to_diagonal_vec(damp_sq_v);
    u.solve(ds, dt);
    j.multiply_transpose_vec(dt, dtheta);
    let max_change = 100.0 * dtheta.max_abs();
    if max_change > MAX_ANGLE_DLS {
        dtheta.scale(MAX_ANGLE_DLS / max_change);
    }
}

/// `Jacobian::CalcDeltaThetasDLSwithSVD`
unsafe fn calc_dls_with_svd(j: Mx, u: Mx, w: Vn, v: Mx, damping_lambda_sq: f64, ds: Vn, dtheta: Vn) -> R {
    j.compute_svd(u, w, v)?;
    let diag_length = w.n;
    let mut w_ptr = w.x;
    dtheta.set_zero();
    for i in 0..diag_length {
        let dot_prod_col = u.dot_product_column(ds, i);
        let mut alpha = rd(w_ptr);
        w_ptr = off(w_ptr, 1);
        alpha /= square(alpha) + damping_lambda_sq;
        add_array_scale(v.r, v.col_ptr(i), 1, dtheta.x, 1, dot_prod_col * alpha);
    }
    let max_change = dtheta.max_abs();
    if max_change > MAX_ANGLE_DLS {
        dtheta.scale(MAX_ANGLE_DLS / max_change);
    }
    Ok(())
}

/// `Jacobian::CalcdTClampedFromdS`
unsafe fn calc_dt_clamped(ds: Vn, dt: Vn, ds_clamp: Vn) {
    let len = ds.n;
    let mut j = 0i64;
    let mut i = 0i64;
    while i < len {
        let at = |p: Vn, k: i64| rd(off(p.x, k));
        let norm_sq = square(at(ds, i)) + square(at(ds, i + 1));
        if norm_sq > square(at(ds_clamp, j)) {
            let factor = at(ds_clamp, j) / norm_sq.sqrt();
            wr(off(dt.x, i), at(ds, i) * factor);
            wr(off(dt.x, i + 1), at(ds, i + 1) * factor);
        } else {
            wr(off(dt.x, i), at(ds, i));
            wr(off(dt.x, i + 1), at(ds, i + 1));
        }
        i += 2;
        j += 1;
    }
}

/// `Jacobian::CalcDeltaThetasSDLS` (see the module notes: it reads past `J`).
#[allow(clippy::too_many_arguments)]
unsafe fn calc_sdls(
    jm: Mx,
    u: Mx,
    w: Vn,
    v: Mx,
    num_end_effectors: i64,
    jnorms: Mx,
    ds: Vn,
    dt: Vn,
    ds_clamp: Vn,
    d_pre_theta: Vn,
    dtheta: Vn,
) -> R {
    jm.compute_svd(u, w, v)?;
    let n_rows = jm.r;
    let n_cols = jm.c;
    dtheta.set_zero();

    let mut jx = jm.x;
    let mut jnx = jnorms.x;
    let mut i = n_cols * num_end_effectors;
    while i > 0 {
        let mut accum_sq = square(rd(jx));
        jx = off(jx, 1);
        accum_sq += square(rd(jx));
        jx = off(jx, 1);
        accum_sq += square(rd(jx));
        jx = off(jx, 1);
        wr(jnx, accum_sq.sqrt());
        jnx = off(jnx, 1);
        i -= 1;
    }

    calc_dt_clamped(ds, dt, ds_clamp);

    for i in 0..n_rows {
        let mut wi_inv = rd(off(w.x, i));
        if near_zero(wi_inv, 1.0e-10) {
            continue;
        }
        wi_inv = 1.0 / wi_inv;

        let mut n = 0.0;
        let mut alpha = 0.0;
        let mut dtx = dt.x;
        let mut ux = u.col_ptr(i);
        let mut j = num_end_effectors;
        while j > 0 {
            alpha += rd(ux) * rd(dtx);
            dtx = off(dtx, 1);
            let mut tmp = square(rd(ux));
            ux = off(ux, 1);
            alpha += rd(ux) * rd(dtx);
            dtx = off(dtx, 1);
            tmp += square(rd(ux));
            ux = off(ux, 1);
            alpha += rd(ux) * rd(dtx);
            dtx = off(dtx, 1);
            tmp += square(rd(ux));
            ux = off(ux, 1);
            n += tmp.sqrt();
            j -= 1;
        }

        let mut m = 0.0;
        let mut vx = v.col_ptr(i);
        jnx = jnorms.x;
        let mut j = n_cols;
        while j > 0 {
            let mut accum = 0.0;
            let mut k = num_end_effectors;
            while k > 0 {
                accum += rd(jnx);
                jnx = off(jnx, 1);
                k -= 1;
            }
            m += rd(vx).abs() * accum;
            vx = off(vx, 1);
            j -= 1;
        }
        m *= wi_inv.abs();

        let mut gamma = MAX_ANGLE_SDLS;
        if n < m {
            gamma *= n / m;
        }

        let scale = alpha * wi_inv;
        d_pre_theta.load_scaled(v.col_ptr(i), scale);
        let max = d_pre_theta.max_abs();
        let rescale = gamma / (gamma + max);
        dtheta.add_scaled(d_pre_theta, rescale);
    }

    let max_change = dtheta.max_abs();
    if max_change > 100.0 * MAX_ANGLE_SDLS {
        dtheta.scale(MAX_ANGLE_SDLS / (MAX_ANGLE_SDLS + max_change));
    }
    Ok(())
}

/// Pseudo-inverse of `svd(a) = (U, w, V)` as `V * diag(1/w) * ...` filled into `vd`
/// (`factor` scales the singular value, `thr_mul` scales the threshold; unset entries stay).
unsafe fn fill_vd(vd: Mx, v: Mx, w: Vn, cols: i64, factor: f64, thr: f64) {
    let mut w_ptr = w.x;
    for j in 0..cols {
        let mut v_ptr = v.col_ptr(j);
        let alpha = factor * rd(w_ptr);
        w_ptr = off(w_ptr, 1);
        for i in 0..v.r {
            if alpha.abs() > thr {
                let entry = rd(v_ptr);
                v_ptr = off(v_ptr, 1);
                vd.set(i, j, entry * (1.0 / alpha));
            }
        }
    }
}

/// `Jacobian::CalcDeltaThetasPseudoinverse` (task-priority pseudo-inverse).
unsafe fn calc_pseudoinverse(j: Mx, num_effector: i64, ds: Vn, dtheta: Vn, diag_mat_identity: Vn) -> R {
    let cols = j.c;
    let (_b1, j1) = tmp(2, cols);
    for i in 0..2 {
        for k in 0..cols {
            j1.set(i, k, j.get(i, k));
        }
    }
    let (_b2, ds1) = tmpv(2);
    for i in 0..2 {
        wr(off(ds1.x, i), rd(off(ds.x, i)));
    }

    let (_b3, u) = tmp(j1.r, j1.r);
    let (_b4, w) = tmpv(j1.r.min(j1.c));
    let (_b5, v) = tmp(j1.c, j1.c);
    j1.compute_svd(u, w, v)?;

    let mut threshold = PSEUDO_INVERSE_THRESHOLD_FACTOR * w.max_abs();
    let diag_length = w.n;
    let mut w_ptr = w.x;
    dtheta.set_zero();
    for i in 0..diag_length {
        let dot_prod_col = u.dot_product_column(ds1, i);
        let mut alpha = rd(w_ptr);
        w_ptr = off(w_ptr, 1);
        if alpha.abs() > threshold {
            alpha = 1.0 / alpha;
            add_array_scale(v.r, v.col_ptr(i), 1, dtheta.x, 1, dot_prod_col * alpha);
        }
    }

    let (_b6, jcurrent_pinv) = tmp(v.r, j1.r);
    let (_b7, jproj_pre) = tmp(jcurrent_pinv.r, j1.c);
    let negate_and_add_identity = |m: Mx| {
        for jj in 0..m.c {
            for ii in 0..m.r {
                let temp = m.get(ii, jj);
                m.set(ii, jj, -1.0 * temp);
            }
        }
        m.add_to_diagonal_vec(diag_mat_identity);
    };
    if num_effector > 1 {
        let (_b8, vd) = tmp(v.r, j1.r);
        threshold = PSEUDO_INVERSE_THRESHOLD_FACTOR * w.max_abs();
        fill_vd(vd, v, w, vd.c, 1.0, threshold);
        Mx::multiply_transpose(vd, u, jcurrent_pinv);
        Mx::multiply(jcurrent_pinv, j1, jproj_pre);
        negate_and_add_identity(jproj_pre);
    }

    let mut i = 1;
    while i < num_effector {
        let (_c1, jcurrent) = tmp(2, cols);
        for jj in 0..cols {
            for k in 0..2 {
                jcurrent.set(k, jj, j.get(k + 2 * i, jj));
            }
        }
        let (_c2, ds_current) = tmpv(2);
        for k in 0..2 {
            wr(off(ds_current.x, k), rd(off(ds.x, k + 2 * i)));
        }

        let (_c3, jdst) = tmp(jcurrent.r, jproj_pre.c);
        Mx::multiply(jcurrent, jproj_pre, jdst);

        let (_c4, uu) = tmp(jdst.r, jdst.r);
        let (_c5, vv) = tmp(jdst.c, jdst.c);
        let (_c6, ww) = tmpv(jdst.r.min(jdst.c));
        jdst.compute_svd(uu, ww, vv)?;

        let (_c7, vvd) = tmp(vv.r, j1.r);
        vvd.set_zero();
        threshold = PSEUDO_INVERSE_THRESHOLD_FACTOR * ww.max_abs();
        fill_vd(vvd, vv, ww, vvd.c, 50.0, 100.0 * threshold);
        let (_c8, jdst_pinv) = tmp(vv.r, j1.r);
        Mx::multiply_transpose(vvd, uu, jdst_pinv);

        let (_c9, d_temp) = tmpv(j1.r);
        jcurrent.multiply_vec(dtheta, d_temp);
        let (_c10, d_temp2) = tmpv(ds_current.n);
        for k in 0..ds_current.n {
            wr(off(d_temp2.x, k), rd(off(ds_current.x, k)) - rd(off(d_temp.x, k)));
        }
        let (_c11, d_theta_current) = tmpv(jdst_pinv.r);
        jdst_pinv.multiply_vec(d_temp2, d_theta_current);
        for k in 0..dtheta.n {
            wr(off(dtheta.x, k), rd(off(dtheta.x, k)) + rd(off(d_theta_current.x, k)));
        }

        jcurrent.compute_svd(u, w, v)?;
        let (_c12, vd) = tmp(v.r, jcurrent.r);
        threshold = PSEUDO_INVERSE_THRESHOLD_FACTOR * w.max_abs();
        fill_vd(vd, v, w, vvd.c, 1.0, threshold);
        Mx::multiply_transpose(vd, u, jcurrent_pinv);
        Mx::multiply(jcurrent_pinv, jcurrent, jproj_pre);
        negate_and_add_identity(jproj_pre);
        i += 1;
    }

    let max_change = 10.0 * dtheta.max_abs();
    if max_change > MAX_ANGLE_PSEUDOINVERSE {
        dtheta.scale(MAX_ANGLE_PSEUDOINVERSE / max_change);
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------
// C ABI
// ---------------------------------------------------------------------------------------
//
// Every function: 0 ok; 1 null / negative / oversized argument; 2 bounds failure or panic;
// 3 shape the original asserted on. `long` on the C++ side is widened to `i64`.

/// # Safety
/// Every pointer must be null or valid for the number of `f64` its length argument says.
#[no_mangle]
pub unsafe extern "C" fn flare_ikjac_vec_max_abs(x: *mut f64, n: i64, out: *mut f64) -> i32 {
    if out.is_null() {
        return E_ARG;
    }
    run(|| {
        let v = vecr(x, n)?;
        *out = v.max_abs();
        Ok(())
    })
}

/// # Safety
/// See [`flare_ikjac_vec_max_abs`].
#[no_mangle]
pub unsafe extern "C" fn flare_ikjac_set_diagonal(x: *mut f64, r: i64, c: i64, d: f64) -> i32 {
    run(|| {
        mat(x, r, c)?.set_diagonal(d);
        Ok(())
    })
}
/// # Safety
/// See [`flare_ikjac_vec_max_abs`].
#[no_mangle]
pub unsafe extern "C" fn flare_ikjac_set_diagonal_vec(x: *mut f64, r: i64, c: i64, d: *mut f64, dn: i64) -> i32 {
    run(|| {
        let m = mat(x, r, c)?;
        let dv = vecr(d, dn)?;
        if dn != r.min(c) {
            return Err(E_SHAPE);
        }
        m.set_diagonal_vec(dv);
        Ok(())
    })
}
/// # Safety
/// See [`flare_ikjac_vec_max_abs`].
#[no_mangle]
pub unsafe extern "C" fn flare_ikjac_set_super_diagonal(x: *mut f64, r: i64, c: i64, d: f64) -> i32 {
    run(|| {
        mat(x, r, c)?.set_super_diagonal(d);
        Ok(())
    })
}
/// # Safety
/// See [`flare_ikjac_vec_max_abs`].
#[no_mangle]
pub unsafe extern "C" fn flare_ikjac_set_super_diagonal_vec(x: *mut f64, r: i64, c: i64, d: *mut f64, dn: i64) -> i32 {
    run(|| {
        let m = mat(x, r, c)?;
        let dv = vecr(d, dn)?;
        if dn != (r - 1).min(c) {
            return Err(E_SHAPE);
        }
        m.set_super_diagonal_vec(dv);
        Ok(())
    })
}
/// # Safety
/// See [`flare_ikjac_vec_max_abs`].
#[no_mangle]
pub unsafe extern "C" fn flare_ikjac_set_sub_diagonal(x: *mut f64, r: i64, c: i64, d: f64) -> i32 {
    run(|| {
        mat(x, r, c)?.set_sub_diagonal(d);
        Ok(())
    })
}
/// # Safety
/// See [`flare_ikjac_vec_max_abs`].
#[no_mangle]
pub unsafe extern "C" fn flare_ikjac_set_sub_diagonal_vec(x: *mut f64, r: i64, c: i64, d: *mut f64, dn: i64) -> i32 {
    run(|| {
        let m = mat(x, r, c)?;
        let dv = vecr(d, dn)?;
        if dn != r.min(c) - 1 {
            return Err(E_SHAPE);
        }
        m.set_sub_diagonal_vec(dv);
        Ok(())
    })
}
/// # Safety
/// See [`flare_ikjac_vec_max_abs`].
#[no_mangle]
pub unsafe extern "C" fn flare_ikjac_set_column(x: *mut f64, r: i64, c: i64, i: i64, d: *mut f64, dn: i64) -> i32 {
    run(|| {
        let m = mat(x, r, c)?;
        let dv = vecr(d, dn)?;
        if dn != r {
            return Err(E_SHAPE);
        }
        m.set_column(i, dv);
        Ok(())
    })
}
/// # Safety
/// See [`flare_ikjac_vec_max_abs`].
#[no_mangle]
pub unsafe extern "C" fn flare_ikjac_set_row(x: *mut f64, r: i64, c: i64, i: i64, d: *mut f64, dn: i64) -> i32 {
    run(|| {
        let m = mat(x, r, c)?;
        let dv = vecr(d, dn)?;
        if dn != c {
            return Err(E_SHAPE);
        }
        m.set_row(i, dv);
        Ok(())
    })
}
/// # Safety
/// See [`flare_ikjac_vec_max_abs`].
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_ikjac_set_sequence(
    x: *mut f64,
    r: i64,
    c: i64,
    d: *mut f64,
    dn: i64,
    start_row: i64,
    start_col: i64,
    delta_row: i64,
    delta_col: i64,
) -> i32 {
    run(|| {
        let m = mat(x, r, c)?;
        let dv = vecr(d, dn)?;
        m.set_sequence(dv, start_row, start_col, delta_row, delta_col);
        Ok(())
    })
}
/// # Safety
/// See [`flare_ikjac_vec_max_abs`].
#[no_mangle]
pub unsafe extern "C" fn flare_ikjac_load_as_submatrix(x: *mut f64, r: i64, c: i64, a: *mut f64, ar: i64, ac: i64) -> i32 {
    run(|| {
        let m = mat(x, r, c)?;
        m.load_as_submatrix(mat(a, ar, ac)?);
        Ok(())
    })
}
/// # Safety
/// See [`flare_ikjac_vec_max_abs`].
#[no_mangle]
pub unsafe extern "C" fn flare_ikjac_load_as_submatrix_transpose(
    x: *mut f64,
    r: i64,
    c: i64,
    a: *mut f64,
    ar: i64,
    ac: i64,
) -> i32 {
    run(|| {
        let m = mat(x, r, c)?;
        m.load_as_submatrix_transpose(mat(a, ar, ac)?);
        Ok(())
    })
}
/// # Safety
/// See [`flare_ikjac_vec_max_abs`].
#[no_mangle]
pub unsafe extern "C" fn flare_ikjac_frobenius_norm(x: *mut f64, r: i64, c: i64, out: *mut f64) -> i32 {
    if out.is_null() {
        return E_ARG;
    }
    run(|| {
        *out = mat(x, r, c)?.frobenius_norm();
        Ok(())
    })
}
/// # Safety
/// See [`flare_ikjac_vec_max_abs`].
#[no_mangle]
pub unsafe extern "C" fn flare_ikjac_multiply_vec(
    x: *mut f64,
    r: i64,
    c: i64,
    v: *mut f64,
    vn: i64,
    res: *mut f64,
    rn: i64,
) -> i32 {
    run(|| {
        let m = mat(x, r, c)?;
        m.multiply_vec(vecr(v, vn)?, vecr(res, rn)?);
        Ok(())
    })
}
/// # Safety
/// See [`flare_ikjac_vec_max_abs`].
#[no_mangle]
pub unsafe extern "C" fn flare_ikjac_multiply_transpose_vec(
    x: *mut f64,
    r: i64,
    c: i64,
    v: *mut f64,
    vn: i64,
    res: *mut f64,
    rn: i64,
) -> i32 {
    run(|| {
        let m = mat(x, r, c)?;
        m.multiply_transpose_vec(vecr(v, vn)?, vecr(res, rn)?);
        Ok(())
    })
}
/// # Safety
/// See [`flare_ikjac_vec_max_abs`].
#[no_mangle]
pub unsafe extern "C" fn flare_ikjac_dot_product_column(
    x: *mut f64,
    r: i64,
    c: i64,
    v: *mut f64,
    vn: i64,
    col: i64,
    out: *mut f64,
) -> i32 {
    if out.is_null() {
        return E_ARG;
    }
    run(|| {
        let m = mat(x, r, c)?;
        *out = m.dot_product_column(vecr(v, vn)?, col);
        Ok(())
    })
}
/// # Safety
/// See [`flare_ikjac_vec_max_abs`].
#[no_mangle]
pub unsafe extern "C" fn flare_ikjac_add_to_diagonal(x: *mut f64, r: i64, c: i64, d: f64) -> i32 {
    run(|| {
        mat(x, r, c)?.add_to_diagonal(d);
        Ok(())
    })
}
/// # Safety
/// See [`flare_ikjac_vec_max_abs`].
#[no_mangle]
pub unsafe extern "C" fn flare_ikjac_add_to_diagonal_vec(x: *mut f64, r: i64, c: i64, v: *mut f64, vn: i64) -> i32 {
    run(|| {
        let m = mat(x, r, c)?;
        m.add_to_diagonal_vec(vecr(v, vn)?);
        Ok(())
    })
}
/// # Safety
/// See [`flare_ikjac_vec_max_abs`].
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_ikjac_multiply_scalar(
    a: *mut f64,
    ar: i64,
    ac: i64,
    k: f64,
    dst: *mut f64,
    dr: i64,
    dc: i64,
) -> i32 {
    run(|| {
        Mx::multiply_scalar(mat(a, ar, ac)?, k, mat(dst, dr, dc)?);
        Ok(())
    })
}

macro_rules! binary_product {
    ($name:ident, $f:path) => {
        /// # Safety
        /// See [`flare_ikjac_vec_max_abs`].
        #[no_mangle]
        #[allow(clippy::too_many_arguments)]
        pub unsafe extern "C" fn $name(
            a: *mut f64,
            ar: i64,
            ac: i64,
            b: *mut f64,
            br: i64,
            bc: i64,
            dst: *mut f64,
            dr: i64,
            dc: i64,
        ) -> i32 {
            run(|| {
                $f(mat(a, ar, ac)?, mat(b, br, bc)?, mat(dst, dr, dc)?);
                Ok(())
            })
        }
    };
}
binary_product!(flare_ikjac_multiply, Mx::multiply);
binary_product!(flare_ikjac_transpose_multiply, Mx::transpose_multiply);
binary_product!(flare_ikjac_multiply_transpose, Mx::multiply_transpose);

/// # Safety
/// See [`flare_ikjac_vec_max_abs`].
#[no_mangle]
pub unsafe extern "C" fn flare_ikjac_solve(x: *mut f64, r: i64, c: i64, b: *mut f64, bn: i64, xv: *mut f64, xn: i64) -> i32 {
    run(|| {
        let m = mat(x, r, c)?;
        let bv = vecr(b, bn)?;
        let xx = vecr(xv, xn)?;
        if !(r == c && c == xn && r == bn) {
            return Err(E_SHAPE);
        }
        m.solve(bv, xx);
        Ok(())
    })
}
/// # Safety
/// See [`flare_ikjac_vec_max_abs`].
#[no_mangle]
pub unsafe extern "C" fn flare_ikjac_convert_to_ref_no_free(x: *mut f64, r: i64, c: i64) -> i32 {
    run(|| {
        mat(x, r, c)?.convert_to_ref_no_free();
        Ok(())
    })
}
/// # Safety
/// `c` and `s` must be null or valid for one write.
#[no_mangle]
pub unsafe extern "C" fn flare_ikjac_calc_givens_values(a: f64, b: f64, c: *mut f64, s: *mut f64) -> i32 {
    if c.is_null() || s.is_null() {
        return E_ARG;
    }
    run(|| {
        let (cc, ss) = calc_givens_values(a, b);
        *c = cc;
        *s = ss;
        Ok(())
    })
}
/// Givens rotation of columns `idx1` and `idx2` (the one-index C++ overload passes `idx, idx + 1`).
///
/// # Safety
/// See [`flare_ikjac_vec_max_abs`].
#[no_mangle]
pub unsafe extern "C" fn flare_ikjac_post_apply_givens(x: *mut f64, r: i64, c: i64, cs: f64, sn: f64, idx1: i64, idx2: i64) -> i32 {
    run(|| {
        mat(x, r, c)?.post_apply_givens(cs, sn, idx1, idx2);
        Ok(())
    })
}
/// # Safety
/// See [`flare_ikjac_vec_max_abs`].
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_ikjac_compute_svd(
    a: *mut f64,
    ar: i64,
    ac: i64,
    u: *mut f64,
    ur: i64,
    uc: i64,
    w: *mut f64,
    wn: i64,
    v: *mut f64,
    vr: i64,
    vc: i64,
) -> i32 {
    run(|| {
        let am = mat(a, ar, ac)?;
        let um = mat(u, ur, uc)?;
        let wv = vecr(w, wn)?;
        let vm = mat(v, vr, vc)?;
        am.compute_svd(um, wv, vm)
    })
}
/// `*ok` receives the verdict of `DebugCheckSVD` (1 = within tolerance).
///
/// # Safety
/// See [`flare_ikjac_vec_max_abs`]; `ok` must be null or valid for one write.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_ikjac_debug_check_svd(
    a: *mut f64,
    ar: i64,
    ac: i64,
    u: *mut f64,
    ur: i64,
    uc: i64,
    w: *mut f64,
    wn: i64,
    v: *mut f64,
    vr: i64,
    vc: i64,
    ok: *mut i32,
) -> i32 {
    if ok.is_null() {
        return E_ARG;
    }
    run(|| {
        let am = mat(a, ar, ac)?;
        let um = mat(u, ur, uc)?;
        let wv = vecr(w, wn)?;
        let vm = mat(v, vr, vc)?;
        // `Diag(U.rows, V.rows).SetDiagonalEntries(w)` and `IV.SetIdentity()` need these.
        if !(vr == vc && ur == uc && wn == ur.min(vr) && am.r == ur && am.c == vc) {
            return Err(E_SHAPE);
        }
        *ok = am.debug_check_svd(um, wv, vm) as i32;
        Ok(())
    })
}

// --- Jacobian -------------------------------------------------------------------------

/// `Jacobian::Reset`: default damping, `DampingLambdaSqV` and `diagMatIdentity` filled,
/// `dSclamp` set to `HUGE_VAL`.
///
/// # Safety
/// See [`flare_ikjac_vec_max_abs`]; `lambda`/`lambda_sq` valid for one write.
#[no_mangle]
pub unsafe extern "C" fn flare_ikjac_reset(
    lambda: *mut f64,
    lambda_sq: *mut f64,
    damp_sq_v: *mut f64,
    dn: i64,
    diag_id: *mut f64,
    in_: i64,
    ds_clamp: *mut f64,
    cn: i64,
) -> i32 {
    if lambda.is_null() || lambda_sq.is_null() {
        return E_ARG;
    }
    run(|| {
        let dv = vecr(damp_sq_v, dn)?;
        let iv = vecr(diag_id, in_)?;
        let cv = vecr(ds_clamp, cn)?;
        *lambda = DEFAULT_DAMPING_LAMBDA;
        *lambda_sq = square(DEFAULT_DAMPING_LAMBDA);
        for i in 0..dv.n {
            wr(off(dv.x, i), *lambda_sq);
        }
        for i in 0..iv.n {
            wr(off(iv.x, i), 1.0);
        }
        for i in 0..cv.n {
            wr(off(cv.x, i), f64::INFINITY);
        }
        Ok(())
    })
}

unsafe fn sl<'a, T>(p: *const T, n: usize) -> &'a [T] {
    if n == 0 {
        &[]
    } else {
        std::slice::from_raw_parts(p, n)
    }
}

/// `Jacobian::computeJacobian` over a pre-gathered skeleton.
///
/// Per effector node `k` (skeleton order): `eff_num[k]` (its effector index `i`),
/// `eff_s[2k..2k+2]` (its global position), `eff_nanc[k]` ancestors (nearest first, laid
/// out consecutively in `anc_*`): joint number, frozen flag, global position.
/// `target` holds `ntarget` points (2 doubles each).
///
/// # Safety
/// See [`flare_ikjac_vec_max_abs`]; the gather arrays must be valid for the counts given.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_ikjac_compute_jacobian(
    jend: *mut f64,
    nrow: i64,
    ncol: i64,
    ds: *mut f64,
    dsn: i64,
    effector_count: i64,
    n_eff: usize,
    eff_num: *const i32,
    eff_s: *const f64,
    eff_nanc: *const usize,
    n_anc: usize,
    anc_joint: *const i32,
    anc_frozen: *const u8,
    anc_s: *const f64,
    target: *const f64,
    ntarget: usize,
) -> i32 {
    if (n_eff > 0 && (eff_num.is_null() || eff_s.is_null() || eff_nanc.is_null()))
        || (n_anc > 0 && (anc_joint.is_null() || anc_frozen.is_null() || anc_s.is_null()))
        || (ntarget > 0 && target.is_null())
        || n_eff > (1 << 30)
        || n_anc > (1 << 30)
        || ntarget > (1 << 30)
    {
        return E_ARG;
    }
    run(|| {
        let j = mat(jend, nrow, ncol)?;
        let dsv = vecr(ds, dsn)?;
        let eff_num = sl(eff_num, n_eff);
        let eff_s = sl(eff_s, 2 * n_eff);
        let eff_nanc = sl(eff_nanc, n_eff);
        let anc_joint = sl(anc_joint, n_anc);
        let anc_frozen = sl(anc_frozen, n_anc);
        let anc_s = sl(anc_s, 2 * n_anc);
        let target = sl(target, 2 * ntarget);

        let mut a = 0usize;
        for k in 0..n_eff {
            let i = eff_num[k] as i64;
            if i < 0 || i as usize >= ntarget {
                return Err(E_RANGE);
            }
            let scaled = i < effector_count - 1;
            let (nsx, nsy) = (eff_s[2 * k], eff_s[2 * k + 1]);
            let mut tx = target[2 * i as usize] - nsx;
            let mut ty = target[2 * i as usize + 1] - nsy;
            if scaled {
                tx *= 100.0;
                ty *= 100.0;
            }
            if 2 * i + 1 >= dsv.n || 2 * i + 1 >= j.r {
                return Err(E_RANGE);
            }
            // dS.SetCouple(i, temp)
            wr(off(dsv.x, 2 * i), tx);
            wr(off(dsv.x, 2 * i + 1), ty);

            let na = eff_nanc[k];
            if a + na > n_anc {
                return Err(E_RANGE);
            }
            for m in a..a + na {
                let jn = anc_joint[m] as i64;
                if jn < 0 || jn >= j.c {
                    return Err(E_RANGE);
                }
                let dst = off(j.x, jn * j.r + 2 * i);
                if anc_frozen[m] != 0 {
                    wr(dst, 0.0);
                    wr(off(dst, 1), 0.0);
                } else {
                    let mut x = anc_s[2 * m] - nsx;
                    let y0 = anc_s[2 * m + 1] - nsy;
                    let t = x;
                    x = y0;
                    let mut y = -t;
                    if scaled {
                        x *= 100.0;
                        y *= 100.0;
                    }
                    wr(dst, x);
                    wr(off(dst, 1), y);
                }
            }
            a += na;
        }
        Ok(())
    })
}

/// # Safety
/// See [`flare_ikjac_vec_max_abs`].
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_ikjac_calc_transpose(
    jx: *mut f64,
    jr: i64,
    jc: i64,
    ds: *mut f64,
    dsn: i64,
    dt: *mut f64,
    dtn: i64,
    dth: *mut f64,
    dthn: i64,
) -> i32 {
    run(|| {
        let j = mat(jx, jr, jc)?;
        calc_transpose(j, vecr(ds, dsn)?, vecr(dt, dtn)?, vecr(dth, dthn)?);
        Ok(())
    })
}

/// # Safety
/// See [`flare_ikjac_vec_max_abs`].
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_ikjac_calc_pseudoinverse(
    jx: *mut f64,
    jr: i64,
    jc: i64,
    num_effector: i64,
    ds: *mut f64,
    dsn: i64,
    dth: *mut f64,
    dthn: i64,
    diag_id: *mut f64,
    din: i64,
) -> i32 {
    run(|| {
        let j = mat(jx, jr, jc)?;
        calc_pseudoinverse(j, num_effector, vecr(ds, dsn)?, vecr(dth, dthn)?, vecr(diag_id, din)?)
    })
}

/// # Safety
/// See [`flare_ikjac_vec_max_abs`].
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_ikjac_calc_dls(
    jx: *mut f64,
    jr: i64,
    jc: i64,
    ux: *mut f64,
    ur: i64,
    uc: i64,
    damp_v: *mut f64,
    dampn: i64,
    ds: *mut f64,
    dsn: i64,
    dt: *mut f64,
    dtn: i64,
    dth: *mut f64,
    dthn: i64,
) -> i32 {
    run(|| {
        let j = mat(jx, jr, jc)?;
        let u = mat(ux, ur, uc)?;
        let dv = vecr(damp_v, dampn)?;
        let dsv = vecr(ds, dsn)?;
        let dtv = vecr(dt, dtn)?;
        let dthv = vecr(dth, dthn)?;
        // Solve() asserts a square system of the size of dS / dT.
        if !(ur == uc && ur == dsn && ur == dtn) {
            return Err(E_SHAPE);
        }
        calc_dls(j, u, dv, dsv, dtv, dthv);
        Ok(())
    })
}

/// # Safety
/// See [`flare_ikjac_vec_max_abs`].
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_ikjac_calc_dls_with_svd(
    jx: *mut f64,
    jr: i64,
    jc: i64,
    ux: *mut f64,
    ur: i64,
    uc: i64,
    w: *mut f64,
    wn: i64,
    vx: *mut f64,
    vr: i64,
    vc: i64,
    damping_lambda_sq: f64,
    ds: *mut f64,
    dsn: i64,
    dth: *mut f64,
    dthn: i64,
) -> i32 {
    run(|| {
        let j = mat(jx, jr, jc)?;
        let u = mat(ux, ur, uc)?;
        let wv = vecr(w, wn)?;
        let v = mat(vx, vr, vc)?;
        calc_dls_with_svd(j, u, wv, v, damping_lambda_sq, vecr(ds, dsn)?, vecr(dth, dthn)?)
    })
}

/// # Safety
/// See [`flare_ikjac_vec_max_abs`].
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_ikjac_calc_sdls(
    jx: *mut f64,
    jr: i64,
    jc: i64,
    ux: *mut f64,
    ur: i64,
    uc: i64,
    w: *mut f64,
    wn: i64,
    vx: *mut f64,
    vr: i64,
    vc: i64,
    num_effector: i64,
    jn: *mut f64,
    jnr: i64,
    jnc: i64,
    ds: *mut f64,
    dsn: i64,
    dt: *mut f64,
    dtn: i64,
    ds_clamp: *mut f64,
    dcn: i64,
    dpre: *mut f64,
    dpn: i64,
    dth: *mut f64,
    dthn: i64,
) -> i32 {
    run(|| {
        let j = mat(jx, jr, jc)?;
        let u = mat(ux, ur, uc)?;
        let wv = vecr(w, wn)?;
        let v = mat(vx, vr, vc)?;
        let jnorms = mat(jn, jnr, jnc)?;
        calc_sdls(
            j,
            u,
            wv,
            v,
            num_effector,
            jnorms,
            vecr(ds, dsn)?,
            vecr(dt, dtn)?,
            vecr(ds_clamp, dcn)?,
            vecr(dpre, dpn)?,
            vecr(dth, dthn)?,
        )
    })
}

/// `Jacobian::CalcdTClampedFromdS`
///
/// # Safety
/// See [`flare_ikjac_vec_max_abs`].
#[no_mangle]
pub unsafe extern "C" fn flare_ikjac_calc_dt_clamped(
    ds: *mut f64,
    dsn: i64,
    dt: *mut f64,
    dtn: i64,
    ds_clamp: *mut f64,
    dcn: i64,
) -> i32 {
    run(|| {
        calc_dt_clamped(vecr(ds, dsn)?, vecr(dt, dtn)?, vecr(ds_clamp, dcn)?);
        Ok(())
    })
}

/// The arithmetic of one effector inside `Jacobian::UpdatedSClampValue` (`i` is the
/// effector number, `(sx, sy)` its global position, `(tx, ty)` its target). Note the
/// original indexes `dS` and `dSclamp` with `i`, not `2 * i`; kept.
///
/// # Safety
/// See [`flare_ikjac_vec_max_abs`].
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_ikjac_updated_sclamp_one(
    i: i64,
    sx: f64,
    sy: f64,
    tx: f64,
    ty: f64,
    ds: *mut f64,
    dsn: i64,
    ds_clamp: *mut f64,
    dcn: i64,
) -> i32 {
    run(|| {
        let dsv = vecr(ds, dsn)?;
        let cv = vecr(ds_clamp, dcn)?;
        let temp_x = tx - sx;
        let temp_y = ty - sy;
        let norm_si = (square(rd(off(dsv.x, i))) + square(rd(off(dsv.x, i + 1)))).sqrt();
        let norma = (temp_x * temp_x + temp_y * temp_y).sqrt();
        let changed_dist = norma - norm_si;
        if changed_dist > 0.0 {
            wr(off(cv.x, i), BASE_MAX_TARGET_DIST + changed_dist);
        } else {
            wr(off(cv.x, i), BASE_MAX_TARGET_DIST);
        }
        Ok(())
    })
}

/// `IKEngine::getJointAngle`'s arithmetic. `has_prev == 0` means joint 0 (`u = (1, 0)`);
/// otherwise `u = normalize(pos - prev)` as `p * (1 / norm(p))` (NaN result when
/// `pos == prev`, as in a C++ build without asserts). Returns 1 for a null `out`.
///
/// # Safety
/// `out` must be null or valid for one write.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_ikengine_joint_angle(
    px: f64,
    py: f64,
    has_prev: i32,
    prx: f64,
    pry: f64,
    nx: f64,
    ny: f64,
    out: *mut f64,
) -> i32 {
    if out.is_null() {
        return E_ARG;
    }
    crate::guard(E_RANGE, || {
        let (mut ux, mut uy) = (1.0, 0.0);
        if has_prev != 0 {
            let (dx, dy) = (px - prx, py - pry);
            let n = (dx * dx + dy * dy).sqrt();
            // n == 0 asserted in the C++; without asserts 1/0 = inf and the angle is NaN
            let inv = 1.0 / n;
            ux = dx * inv;
            uy = dy * inv;
        }
        let (vx, vy) = (-uy, ux);
        let (ddx, ddy) = (nx - px, ny - py);
        *out = (ddx * vx + ddy * vy).atan2(ddx * ux + ddy * uy);
        0
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr::{null, null_mut};

    // ---- golden replay --------------------------------------------------------------
    //
    // Recorded from the ORIGINAL ikjacobian.cpp (and ikengine.cpp for `ang`) rebuilt from
    // git by tests/native/gen_ikjacobian_cases.py (`--check` re-derives and diffs it).
    // Line: `name ints.. | inputs | outputs`, doubles as 16 hex digits of their bits.
    // BEGIN GOLDEN
    const GOLDEN: &str = r#"
maxabs 1 | bff5e0ea1958e651 | 3ff5e0ea1958e651
mulv 1 1 | c0c278d22cf2752b bff4115a863f3276 | 40c72b0f9a19e71c
mulvt 1 1 | c0c278d22cf2752b be4210cd79f6713d | 3f14db527d90cff7
dotcol 1 1 0 | c0c278d22cf2752b be4210cd79f6713d | 3f14db527d90cff7
frob 1 1 | c0c278d22cf2752b | 40c278d22cf2752b
setdiag 1 1 | 0000000000000000 | 3ff87b1eaf48d256 | 3ff87b1eaf48d256
setdiagv 1 1 | 3df175a8549f0b1e | be40f13847c19026 | be40f13847c19026
setsuper 1 1 | be18898afa9210e4 | 3ff87b1eaf48d256 | be18898afa9210e4
setsub 1 1 | 40d07979ab302418 | 3ff87b1eaf48d256 | 40d07979ab302418
adddiag 1 1 | c007417f2815a6e8 | 3ff87b1eaf48d256 | bff607dfa0e27b7a
adddiagv 1 1 | 8000000000000000 | be40f13847c19026 | be40f13847c19026
setcol 1 1 0 | 3fcf20117689b8f0 bffae85978e76934 | bffae85978e76934
setrow 1 1 0 | 3e043c9c053f9933 3fdc76670f226de8 | 3fdc76670f226de8
setseq 1 1 0 0 1 0 1 | 0000000000000000 40005d8b4199197a | 40005d8b4199197a
setseq 1 1 0 0 0 1 1 | be323f119920a979 8000000000000000 | 8000000000000000
setseq 1 1 0 0 1 1 1 | 8000000000000000 bff0044db10e3333 | bff0044db10e3333
load 1 1 1 1 | 4000a2ad96eff478 3ffddfe717b2bf8e | 3ffddfe717b2bf8e
loadt 1 1 1 1 | bffdb49481528f2a 0000000000000000 | 0000000000000000
maxabs 4 | 8000000000000000 c010000000000000 3fd7bf6c63e70d70 0000000000000000 | 4010000000000000
mulv 1 4 | 8000000000000000 c0066b12f32499c6 bfd733b6cac88cf0 c007d107472a2436 3ff1128bb93f8752 4000b08923e55f0b bffd8543d10855cf 40f60bcac1775669 | c11068907866186e
mulvt 1 4 | 8000000000000000 c0066b12f32499c6 bfd733b6cac88cf0 c007d107472a2436 40f7cedcf9aae57e | 0000000000000000 c110ade1b7cab6cf c0e14328830891f5 c111b83350e42478
dotcol 1 4 3 | 8000000000000000 c0066b12f32499c6 bfd733b6cac88cf0 c007d107472a2436 40f7cedcf9aae57e | c111b83350e42478
frob 1 4 | 8000000000000000 c0066b12f32499c6 bfd733b6cac88cf0 c007d107472a2436 | 40106b08b4b91f46
setdiag 1 4 | be359c7b147ab0e9 40d503a1a72a9fdb 3ffbd47d270bed21 be422c2e9d2a45cc | 3feac1b9736718ec | 3feac1b9736718ec 40d503a1a72a9fdb 3ffbd47d270bed21 be422c2e9d2a45cc
setdiagv 1 4 | c00522a1779b7578 bffa0999231c2560 bfd39ce09793dd18 be32b8103c7c54c5 | 3ff0000000000000 | 3ff0000000000000 bffa0999231c2560 bfd39ce09793dd18 be32b8103c7c54c5
setsuper 1 4 | 4006d1f533cf8761 0000000000000000 40036a1727a6bde6 be39ed1a0af9a355 | 3feac1b9736718ec | 4006d1f533cf8761 3feac1b9736718ec 40036a1727a6bde6 be39ed1a0af9a355
setsub 1 4 | bfd54c4b76900e38 40ee294bd787f0dc bff5c62f28c4212f 4010000000000000 | 3feac1b9736718ec | bfd54c4b76900e38 40ee294bd787f0dc bff5c62f28c4212f 4010000000000000
adddiag 1 4 | 3ff0000000000000 40077b24998438aa 4004ed8d9292f252 bff9e2803541dcc6 | 3feac1b9736718ec | 3ffd60dcb9b38c76 40077b24998438aa 4004ed8d9292f252 bff9e2803541dcc6
adddiagv 1 4 | 400562384b1ffb9b 3fff9a4dea73e755 40060f556bbad18e 3fe99fc2e773bf84 | 3ff0000000000000 | 400d62384b1ffb9b 3fff9a4dea73e755 40060f556bbad18e 3fe99fc2e773bf84
setcol 1 4 3 | 3fc419e3f0eb1e30 3e22d128b697c69b bff350a64673f9f0 3ff629bcded789ed c003e2a297da8596 | 3fc419e3f0eb1e30 3e22d128b697c69b bff350a64673f9f0 c003e2a297da8596
setrow 1 4 0 | 0000000000000000 40012070a3328c02 8000000000000000 3ff7212969d41653 c008000000000000 3fe07354c7dd6f76 3e437b85670ae8c6 bff3891fd64948d8 | c008000000000000 40012070a3328c02 8000000000000000 3ff7212969d41653
setseq 1 4 0 0 1 0 1 | 4008000000000000 40f46fca42aff64e 40aad2896cfdd6fd bff706ffb6795f11 40038e8592601f70 | 40038e8592601f70 40f46fca42aff64e 40aad2896cfdd6fd bff706ffb6795f11
setseq 1 4 0 0 0 1 4 | 4008000000000000 be43b30b5a9c3713 3fef4abdc3de68e2 0000000000000000 bfe4dbd8ba117b6e bff7f4b0c7a45bf3 3fe6685f891c7150 0000000000000000 | bfe4dbd8ba117b6e bff7f4b0c7a45bf3 3fe6685f891c7150 0000000000000000
setseq 1 4 0 0 1 1 1 | 3ff51c4b0aadc788 3ff406cb03d7e139 3ffa16b6fdbef368 c006570b594c1e44 bfe62d6e31d99f58 | bfe62d6e31d99f58 3ff406cb03d7e139 3ffa16b6fdbef368 c006570b594c1e44
load 1 4 1 4 | 0000000000000000 0000000000000000 bffaa5f3ba3a6ee2 bfd75d04d7b681a4 c0f1e50d0bf4d39d 3e064e9eb0a3eb98 3e3b27825e100f75 0000000000000000 | c0f1e50d0bf4d39d 3e064e9eb0a3eb98 3e3b27825e100f75 0000000000000000
loadt 1 4 4 1 | 0000000000000000 3ff4fa07b0a55487 bfe5e4e5920f6954 bffd831efeb9b3a1 3ffa45bfbd318d9d c001ab3f24080330 c0f5b98cd35e69de bfe87b83e049785e | 3ffa45bfbd318d9d c001ab3f24080330 c0f5b98cd35e69de bfe87b83e049785e
maxabs 1 | 3ff55423872506cf | 3ff55423872506cf
mulv 4 1 | bffd5985a2be1527 c003ccad3439c3a7 8000000000000000 40065f04d9a9afaa bff9a9ed07142d88 | 400789ca966bd0a8 400fc215d799fd66 0000000000000000 c011f1075058e9bd
mulvt 4 1 | bffd5985a2be1527 c003ccad3439c3a7 8000000000000000 40065f04d9a9afaa 3e0cd92966194f72 400160ea7af804ae 0000000000000000 40f13666a08a5d00 | 410810da530cf83e
dotcol 4 1 0 | bffd5985a2be1527 c003ccad3439c3a7 8000000000000000 40065f04d9a9afaa 3e0cd92966194f72 400160ea7af804ae 0000000000000000 40f13666a08a5d00 | 410810da530cf83e
frob 4 1 | bffd5985a2be1527 c003ccad3439c3a7 8000000000000000 40065f04d9a9afaa | 4010a46242b61318
setdiag 4 1 | 0000000000000000 bfed5dfcdffbccc4 4004ad2272a2e7af 0000000000000000 | 40067e2bddd2acf1 | 40067e2bddd2acf1 bfed5dfcdffbccc4 4004ad2272a2e7af 0000000000000000
setdiagv 4 1 | 8000000000000000 0000000000000000 3fffe36ef5ad40c5 0000000000000000 | 4001802d5c2da2b1 | 4001802d5c2da2b1 0000000000000000 3fffe36ef5ad40c5 0000000000000000
setsuper 4 1 | 3ffd1778f4a6d71f bff53a83b4743cf4 c0f2e161983529cc 4001dc71e25d7893 | 40067e2bddd2acf1 | 3ffd1778f4a6d71f bff53a83b4743cf4 c0f2e161983529cc 4001dc71e25d7893
setsub 4 1 | 3ffd87f362828f8f 3ff15f2de1f9cd6d 0000000000000000 c0dab7f851559da6 | 40067e2bddd2acf1 | 3ffd87f362828f8f 3ff15f2de1f9cd6d 0000000000000000 c0dab7f851559da6
adddiag 4 1 | 0000000000000000 3fecec0b39e82706 0000000000000000 3fe73394ba46b324 | 40067e2bddd2acf1 | 40067e2bddd2acf1 3fecec0b39e82706 0000000000000000 3fe73394ba46b324
adddiagv 4 1 | bffb2d9c08de840d 3ff5abeb59c9fa08 be007b5f938d0ff4 bff534f3e6165d6b | 4001802d5c2da2b1 | 3fdf4afabdf30554 3ff5abeb59c9fa08 be007b5f938d0ff4 bff534f3e6165d6b
setcol 4 1 0 | 3fc02a304bdcef68 4000031d93b13a5e 3ff35461c48b7f82 4000000000000000 0000000000000000 4005c2dd7519fa81 3fe8002a6a73ac66 bff899035a844563 | 0000000000000000 4005c2dd7519fa81 3fe8002a6a73ac66 bff899035a844563
setseq 4 1 0 0 1 0 4 | 3ff5268aa72526ec bfe0efb437e87b22 bfd7d17ba405e584 40ea56097f9b4ed2 3e3ac73aacb732b5 bff053663cd25d9c 3ff37357e8205d16 bffc6c2dced1f445 | 3e3ac73aacb732b5 bff053663cd25d9c 3ff37357e8205d16 bffc6c2dced1f445
setseq 4 1 0 0 0 1 1 | c0eaafeb26c7c2c2 c004611137c67d00 40f29a2656f17d65 be20235d3151cd72 c000605387cc2a40 | c000605387cc2a40 c004611137c67d00 40f29a2656f17d65 be20235d3151cd72
setseq 4 1 0 0 1 1 1 | 3fc5ea820ab33a70 8000000000000000 bffe435a27814cc4 3e3dbb41ffa94ad8 0000000000000000 | 0000000000000000 8000000000000000 bffe435a27814cc4 3e3dbb41ffa94ad8
load 4 1 4 1 | 400414045e7dac9f 3ffc8dd0e1ccf7de 3ffe7aa15783eb02 bff91eba64f1a680 c004b75d79ef8ac4 c0017d6ca7e2c9f8 bfd2687fec3eb384 bffb4928a3573bc1 | c004b75d79ef8ac4 c0017d6ca7e2c9f8 bfd2687fec3eb384 bffb4928a3573bc1
loadt 4 1 1 4 | 0000000000000000 3fef60603bff5c04 0000000000000000 3ff47ebd96cf692e bf82bd1017236580 c0065369d30185ac 0000000000000000 bfe7539ee8e83010 | bf82bd1017236580 c0065369d30185ac 0000000000000000 bfe7539ee8e83010
maxabs 2 | 8000000000000000 c00453e1141c4bb2 | 400453e1141c4bb2
mulv 2 2 | 3e41af6f05226966 c0028622a08f38b8 3ffbdc37c34e055c 3fdf316976c3c540 bfe1c28a3e9f735c 3ffd88850a79f6cf | 4009b66e49c679c0 40017a566ecab460
mulvt 2 2 | 3e41af6f05226966 c0028622a08f38b8 3ffbdc37c34e055c 3fdf316976c3c540 bfb5810168d0cd40 8000000000000000 | be07c4d9b4a89d3d bfc2b8d59319c37c
dotcol 2 2 1 | 3e41af6f05226966 c0028622a08f38b8 3ffbdc37c34e055c 3fdf316976c3c540 bfb5810168d0cd40 8000000000000000 | bfc2b8d59319c37c
frob 2 2 | 3e41af6f05226966 c0028622a08f38b8 3ffbdc37c34e055c 3fdf316976c3c540 | 400780c12d92b20f
setdiag 2 2 | c0f80a0b1c6b9d73 c0dc31bc3de0587c 0000000000000000 3ff51e74efef4cd1 | 3fc828af6af63c00 | 3fc828af6af63c00 c0dc31bc3de0587c 0000000000000000 3fc828af6af63c00
setdiagv 2 2 | 4001c91ad7b1d28d c008000000000000 c008000000000000 4007bd637de39a35 | 40d894cdcb9d376c 3e11a165394d38c3 | 40d894cdcb9d376c c008000000000000 c008000000000000 3e11a165394d38c3
setsuper 2 2 | 0000000000000000 3ff461de31af0d8a 3e4442bd6b690c94 c000000000000000 | 3fc828af6af63c00 | 0000000000000000 3ff461de31af0d8a 3fc828af6af63c00 c000000000000000
setsub 2 2 | 0000000000000000 4003786553d33106 4004465c87ec829a 40043ccb3ede2ede | 3fc828af6af63c00 | 0000000000000000 3fc828af6af63c00 4004465c87ec829a 40043ccb3ede2ede
adddiag 2 2 | 40018b71963c97e8 3fed920476f8535c c0054c49d435173d 0000000000000000 | 3fc828af6af63c00 | 40030dfc8cebfba8 3fed920476f8535c c0054c49d435173d 3fc828af6af63c00
adddiagv 2 2 | 40b67ee86a37aae5 bfbb975628dc9370 3e41bbc5305a089a 4010000000000000 | 40d894cdcb9d376c 3e11a165394d38c3 | 40de3487e62b2225 bfbb975628dc9370 3e41bbc5305a089a 401000000011a165
setsuperv 2 2 | c0064fe93c87e82e c0c93fb5e44de6e7 bfe666bedbfaa6d4 c0f5065116fbc31c | 40ec60122d82266a | c0064fe93c87e82e c0c93fb5e44de6e7 40ec60122d82266a c0f5065116fbc31c
setsubv 2 2 | c002a6b0d97b5430 3fcc47731cfee108 0000000000000000 3fedde4bfffdb2d4 | 0000000000000000 | c002a6b0d97b5430 0000000000000000 0000000000000000 3fedde4bfffdb2d4
setcol 2 2 0 | 3ff567caec5b6265 c0f57d4befedda85 40f1ee7ffd7300d2 3fda497ddaf32d74 3e24465fa7ebab1a bff735e901572981 | 3e24465fa7ebab1a bff735e901572981 40f1ee7ffd7300d2 3fda497ddaf32d74
setrow 2 2 1 | be3ed9358ce8d201 c0f73556e02829d3 40073b4b58dfc4f8 40e9e5276e7bf101 3ffa2c99602c8d4e 0000000000000000 | be3ed9358ce8d201 3ffa2c99602c8d4e 40073b4b58dfc4f8 0000000000000000
setseq 2 2 0 0 1 0 2 | 0000000000000000 c006f2c703584ffc bff083a4d7be6700 3fd7b9c790d58944 3fee1913f51eb7ec 3e205f416c0443d7 | 3fee1913f51eb7ec 3e205f416c0443d7 bff083a4d7be6700 3fd7b9c790d58944
setseq 2 2 0 0 0 1 2 | c005ba34261ea128 bfae30d25a60b8e0 c000000000000000 bff0823a578c9ffa c005e04d32756e02 bfc98008e92cf4f8 | c005e04d32756e02 bfae30d25a60b8e0 bfc98008e92cf4f8 bff0823a578c9ffa
setseq 2 2 0 0 1 1 2 | 40035ee099d5f3cc bffdd22241890bb4 c0f2a440f8225112 bfef210721098524 be4314ede3ede4d3 3ff61d4feaa5bed9 | be4314ede3ede4d3 bffdd22241890bb4 c0f2a440f8225112 3ff61d4feaa5bed9
load 2 2 2 1 | c0011888e7accb34 bff2f4f8fe805237 0000000000000000 0000000000000000 c00070a5c664be6a bff8093b4361ed62 | c00070a5c664be6a bff8093b4361ed62 0000000000000000 0000000000000000
loadt 2 2 2 1 | 3e3be394bb6c9753 400092529163a1c8 bfffa88467cae880 40040c5c8c8dcbc2 bffff0eb43c7bfd2 bffcd6c60eb30132 | bffff0eb43c7bfd2 400092529163a1c8 bffcd6c60eb30132 40040c5c8c8dcbc2
maxabs 3 | bff4176feebc7efe c0f7d911473d1c98 0000000000000000 | 40f7d911473d1c98
mulv 2 3 | 8000000000000000 c000000000000000 c0ca51854b8b4ba7 bffab694eba34aa2 bfe7737bb254e090 bfe2f0ebfedeebc2 bff0000000000000 bfec234850add616 c0f31988ea05a7a8 | 40f0e3d329897151 40e69cc1beb3d653
mulvt 2 3 | 8000000000000000 c000000000000000 c0ca51854b8b4ba7 bffab694eba34aa2 bfe7737bb254e090 bfe2f0ebfedeebc2 4007e7fc6aef21f4 3fec64a1445531e8 | bffc64a1445531e8 c0e3a99349f8db95 c005b89b3e2d8cab
dotcol 2 3 0 | 8000000000000000 c000000000000000 c0ca51854b8b4ba7 bffab694eba34aa2 bfe7737bb254e090 bfe2f0ebfedeebc2 4007e7fc6aef21f4 3fec64a1445531e8 | bffc64a1445531e8
frob 2 3 | 8000000000000000 c000000000000000 c0ca51854b8b4ba7 bffab694eba34aa2 bfe7737bb254e090 bfe2f0ebfedeebc2 | 40ca518554e038cd
setdiag 2 3 | bfef0a7f2379b70c 40e4d043d7915535 0000000000000000 3fe29dd4a804306e be278aa0921513a8 0000000000000000 | 3fb898f5fa06aae0 | 3fb898f5fa06aae0 40e4d043d7915535 0000000000000000 3fb898f5fa06aae0 be278aa0921513a8 0000000000000000
setdiagv 2 3 | c00770e01a17eb8a bfd891b9016e3984 bff639ff14c4d784 8000000000000000 400361d6bcb4b167 c005f8967dea7508 | 3e372f32652499d3 0000000000000000 | 3e372f32652499d3 bfd891b9016e3984 bff639ff14c4d784 0000000000000000 400361d6bcb4b167 c005f8967dea7508
setsuper 2 3 | 4000000000000000 0000000000000000 3fecd145e7dad7b0 3feb3a6dba72c7f6 4007b611f964604c c00263552f29608c | 3fb898f5fa06aae0 | 4000000000000000 0000000000000000 3fb898f5fa06aae0 3feb3a6dba72c7f6 4007b611f964604c 3fb898f5fa06aae0
setsub 2 3 | 3ff75f308c422a72 c0f180012ae859fc c00171279d552a6f 3e44ed685f924f13 0000000000000000 40041f9e1fcdcb9c | 3fb898f5fa06aae0 | 3ff75f308c422a72 3fb898f5fa06aae0 c00171279d552a6f 3e44ed685f924f13 0000000000000000 40041f9e1fcdcb9c
adddiag 2 3 | 4073cce5948bb7be 400533a859b12fad bfe19af58f697a18 c0f1caa5cf12208f c003731e4d36a4ab 4001d7c81ae82464 | 3fb898f5fa06aae0 | 4073ce6f23eb5829 400533a859b12fad bfe19af58f697a18 c0f1caa44582c0ef c003731e4d36a4ab 4001d7c81ae82464
adddiagv 2 3 | 40b7f86e485e80d6 0000000000000000 c000bf31ffb378ab 3ff17207c40a99ca 3ff6209690008526 8000000000000000 | 3e372f32652499d3 0000000000000000 | 40b7f86e485e9805 0000000000000000 c000bf31ffb378ab 3ff17207c40a99ca 3ff6209690008526 8000000000000000
setsuperv 2 3 | be3e3a0eb71acb38 0000000000000000 bfea9ffa17735dea bfe2334ecb1f4b00 0000000000000000 3fd8e93492f5cd94 | 0000000000000000 | be3e3a0eb71acb38 0000000000000000 0000000000000000 bfe2334ecb1f4b00 0000000000000000 3fd8e93492f5cd94
setsubv 2 3 | 3ffc7a7621159a6d c0f65ca5520d0474 0000000000000000 4004801b82e9475a bfe4580f0f3d37ac 4008000000000000 | 0000000000000000 | 3ffc7a7621159a6d 0000000000000000 0000000000000000 4004801b82e9475a bfe4580f0f3d37ac 4008000000000000
setcol 2 3 1 | bff0000000000000 3ffc3574ab3390be 3ff2e6d7dd8c861a 3fe5b194cd0a3984 8000000000000000 bfe3c626325021f2 c0c027ae038a1a21 3ff8ac80980bb1ec | bff0000000000000 3ffc3574ab3390be c0c027ae038a1a21 3ff8ac80980bb1ec 8000000000000000 bfe3c626325021f2
setrow 2 3 1 | c0eaf835614dd849 0000000000000000 3e1d243efb777ef2 bfe4702814f50fdc 3e200260ba34a22e 3fc8a8d8a8ce2050 3e4011f73720f9a8 4001edef3866f940 be455b0e7b807844 | c0eaf835614dd849 3e4011f73720f9a8 3e1d243efb777ef2 4001edef3866f940 3e200260ba34a22e 3fc8a8d8a8ce2050
setseq 2 3 0 0 1 0 2 | be333d24c3133173 8000000000000000 c00077f01da1108b 4002ba4a4c73051b 40055adae148936a 3ffd3bde70f09faf 3fff9aface293bc3 0000000000000000 | 3fff9aface293bc3 0000000000000000 c00077f01da1108b 4002ba4a4c73051b 40055adae148936a 3ffd3bde70f09faf
setseq 2 3 0 0 0 1 3 | c0ef35eb57f5b63b 0000000000000000 3fd4543724518ab4 0000000000000000 c006503a44640606 bfdcbea668488644 40036acaca5e2dd6 8000000000000000 3ff00309d92ed41f | 40036acaca5e2dd6 0000000000000000 8000000000000000 0000000000000000 3ff00309d92ed41f bfdcbea668488644
setseq 2 3 0 0 1 1 2 | 3fdc3ff653acd850 3fe3ac62c644eb26 0000000000000000 bfcc5f9d943038b0 bffe75259cf9b3b1 400063e3efd52fb8 400553b8891b4729 c070cba97af3a6a8 | 400553b8891b4729 3fe3ac62c644eb26 0000000000000000 c070cba97af3a6a8 bffe75259cf9b3b1 400063e3efd52fb8
load 2 3 1 3 | 3fe9b02e4fe20cd8 bfe47648c879c70e 3fe927fb1bc0dc92 8000000000000000 0000000000000000 c003d11ed081daa8 3fe8a5af103f2290 3e433eb8c11eafca c000ba30d2cca775 | 3fe8a5af103f2290 bfe47648c879c70e 3e433eb8c11eafca 8000000000000000 c000ba30d2cca775 c003d11ed081daa8
loadt 2 3 3 1 | c0e8181d24c2ce1d bffded5833abe9b8 be42cb2f8dc1483c bfeaa2aae6d35f4e 3ffaa02b451ebe19 bfd4c0e9e6c566dc bffd448f9d56b08b 40f5fbc36f1a64a6 bfc5b044caf73f18 | bffd448f9d56b08b bffded5833abe9b8 40f5fbc36f1a64a6 bfeaa2aae6d35f4e bfc5b044caf73f18 bfd4c0e9e6c566dc
maxabs 2 | c0ee94008922e2c9 3f97e8dc39fabb00 | 40ee94008922e2c9
mulv 3 2 | c0f5765bd5a83585 bfe1a22b674c9f6a c00212d81a20e6c2 c0e719748630802b c00412ae27bc70ca c006f1d2abba6be9 0000000000000000 bf9b5838eb375440 | 4093bd296800bca4 3fb1271a04b0eed3 3fb39b4b9415c763
mulvt 3 2 | c0f5765bd5a83585 bfe1a22b674c9f6a c00212d81a20e6c2 c0e719748630802b c00412ae27bc70ca c006f1d2abba6be9 3fe44f53dc2fc8ec bfe1d3b390b77d8c 3fe14260fd4f3938 | c0eb3e78d90b9a86 c0dd5261f3c1c7cc
dotcol 3 2 0 | c0f5765bd5a83585 bfe1a22b674c9f6a c00212d81a20e6c2 c0e719748630802b c00412ae27bc70ca c006f1d2abba6be9 3fe44f53dc2fc8ec bfe1d3b390b77d8c 3fe14260fd4f3938 | c0eb3e78d90b9a86
frob 3 2 | c0f5765bd5a83585 bfe1a22b674c9f6a c00212d81a20e6c2 c0e719748630802b c00412ae27bc70ca c006f1d2abba6be9 | 40f85f6886601619
setdiag 3 2 | 4003a11ca5a3a95a bff8834f89fef851 8000000000000000 4008000000000000 bff943f5323f7455 3e352dcfda587a02 | be4362e1b50dc0b6 | be4362e1b50dc0b6 bff8834f89fef851 8000000000000000 4008000000000000 be4362e1b50dc0b6 3e352dcfda587a02
setdiagv 3 2 | 4005e12f517f4344 400502e5916b6b5e c0034c748a7ed890 4004cabdfe47f3e6 c010000000000000 bf91794db0d640c0 | c008000000000000 3fec6b59f9301036 | c008000000000000 400502e5916b6b5e c0034c748a7ed890 4004cabdfe47f3e6 3fec6b59f9301036 bf91794db0d640c0
setsuper 3 2 | c008000000000000 bff718a097e7ad48 be3a999dc96dac71 bff5449c27cddb7e be42a95f82081e28 bfe3928a7690bc48 | be4362e1b50dc0b6 | c008000000000000 bff718a097e7ad48 be3a999dc96dac71 be4362e1b50dc0b6 be42a95f82081e28 bfe3928a7690bc48
setsub 3 2 | 400698cdc6b27c2e 400234e2ca37283c c0d5251c6c9b15ad 3ff01c9d1b2611bc 4003380d4bd31410 3ff253b3e7b82572 | be4362e1b50dc0b6 | 400698cdc6b27c2e be4362e1b50dc0b6 c0d5251c6c9b15ad 3ff01c9d1b2611bc 4003380d4bd31410 3ff253b3e7b82572
adddiag 3 2 | bfc9c6a2a1fc2bb0 3fee2bf687956498 0000000000000000 40f0cc8d6f1f8230 bfecf4efc50b887e c0c52ea8c82cedfe | be4362e1b50dc0b6 | bfc9c6a2b55f0d65 3fee2bf687956498 0000000000000000 40f0cc8d6f1f8230 bfecf4efc9e440eb c0c52ea8c82cedfe
adddiagv 3 2 | 3ff20b0d4e37414e 400454dce127ed64 bfd3aec314906438 3fed6d9c33e5e0ba 3e415883928f6722 0000000000000000 | c008000000000000 3fec6b59f9301036 | bffdf4f2b1c8beb2 400454dce127ed64 bfd3aec314906438 3fed6d9c33e5e0ba 3fec6b59fd86311b 0000000000000000
setsubv 3 2 | 4010000000000000 c000a3ccd8a083bc 3e40768770228359 bff5bae3f0568a4a bff0000000000000 0000000000000000 | 3feed4368c2201ea | 4010000000000000 3feed4368c2201ea 3e40768770228359 bff5bae3f0568a4a bff0000000000000 0000000000000000
setcol 3 2 1 | 4005f2720a904fe8 3ff9d446d2c2fc09 4006b85d20e3844a be20089048cf1958 3ffeed2184182b4d c000000000000000 bfed78292b44aa78 bfe3d90a6a65e2c2 3ff778ac8e455ce4 | 4005f2720a904fe8 3ff9d446d2c2fc09 4006b85d20e3844a bfed78292b44aa78 bfe3d90a6a65e2c2 3ff778ac8e455ce4
setseq 3 2 0 0 1 0 3 | be20be0bd1adc236 c000892240001e44 3fd1ce6c58c17e6c 40efd99a6369802f bff7ac7738420e00 3fc90dfc32de84b0 0000000000000000 40025486cfb24b18 bfeeff93f73009b0 | 0000000000000000 40025486cfb24b18 bfeeff93f73009b0 40efd99a6369802f bff7ac7738420e00 3fc90dfc32de84b0
setseq 3 2 0 0 0 1 2 | 4007b3eb4bfae122 bff7f291c28bf403 bfe5aed8e905cc02 be03bbbe728e8264 0000000000000000 3f95bd6049e08f40 bfcf90a3d31cc3e0 3fd123951ea3bfcc | bfcf90a3d31cc3e0 bff7f291c28bf403 bfe5aed8e905cc02 3fd123951ea3bfcc 0000000000000000 3f95bd6049e08f40
setseq 3 2 0 0 1 1 2 | 400039d976e3d4e4 c006f53bd6484bb1 3ff304c4241505b8 3ffecc94233c8595 400269a2bf941710 be445a98abbe4a66 4003bada6b78a9e2 bfff9528a2f012b0 | 4003bada6b78a9e2 c006f53bd6484bb1 3ff304c4241505b8 3ffecc94233c8595 bfff9528a2f012b0 be445a98abbe4a66
load 3 2 3 2 | bfe951ddb2ed9e3c bffe4a5ea86a3fc8 bfe3632faa6df852 40e7f7ba9ad487be c0050b71ad92e522 3fe841a5acc8d9ba 4003c493838fb986 bfe289026d73c9a8 be435f169f3235a5 0000000000000000 c008000000000000 40d4a7c19b852916 | 4003c493838fb986 bfe289026d73c9a8 be435f169f3235a5 0000000000000000 c008000000000000 40d4a7c19b852916
loadt 3 2 1 1 | 40f84677c36e5cff 3fd7c5e1199ced94 40064edbb7692da0 3fd5c6fd00f7df54 c004f991f8b3819b be0e0c85326f1cc4 0000000000000000 | 0000000000000000 3fd7c5e1199ced94 40064edbb7692da0 3fd5c6fd00f7df54 c004f991f8b3819b be0e0c85326f1cc4
maxabs 3 | bff2d70bec9beabe 4006e89ebf33af24 3fe7024aae4fbed0 | 4006e89ebf33af24
mulv 3 3 | 3ffe01de482b2095 3ff6947d1f856fc6 3fff09d74cbb7588 3e3037292cf1b2de be34a64721be984f bfd07d3734b76a9c bff70a1efcdc1a87 c0f7c62e30ec55ab 4003e30150b92f17 0000000000000000 0000000000000000 4006c89c6985f1c6 | c01067652cd46865 c110ed4a96db395d 401c5179c7e4b2b8
mulvt 3 3 | 3ffe01de482b2095 3ff6947d1f856fc6 3fff09d74cbb7588 3e3037292cf1b2de be34a64721be984f bfd07d3734b76a9c bff70a1efcdc1a87 c0f7c62e30ec55ab 4003e30150b92f17 0000000000000000 4003c013e50f711f c002b2f305cad5a2 | bff0cda137753fe4 3fe345498ec814f8 c10d58ed0fafeb68
dotcol 3 3 2 | 3ffe01de482b2095 3ff6947d1f856fc6 3fff09d74cbb7588 3e3037292cf1b2de be34a64721be984f bfd07d3734b76a9c bff70a1efcdc1a87 c0f7c62e30ec55ab 4003e30150b92f17 0000000000000000 4003c013e50f711f c002b2f305cad5a2 | c10d58ed0fafeb68
frob 3 3 | 3ffe01de482b2095 3ff6947d1f856fc6 3fff09d74cbb7588 3e3037292cf1b2de be34a64721be984f bfd07d3734b76a9c bff70a1efcdc1a87 c0f7c62e30ec55ab 4003e30150b92f17 | 40f7c62e314b0c1b
setdiag 3 3 | 0000000000000000 be2fc15ecb6a78ba bff0000000000000 bffaf3df462aff5c 4000000000000000 c0f0fffbb854eccc 3ffebf8b289749db bff45bd2a2c9fdce c0e8ae37503c4d1a | 40cc5d362ab23bef | 40cc5d362ab23bef be2fc15ecb6a78ba bff0000000000000 bffaf3df462aff5c 40cc5d362ab23bef c0f0fffbb854eccc 3ffebf8b289749db bff45bd2a2c9fdce 40cc5d362ab23bef
setdiagv 3 3 | 0000000000000000 c010000000000000 8000000000000000 3fdfafc0c4d36e54 c0ea3fc9938f3c79 c0de31fb8a49c23a 0000000000000000 3fed29bf855750cc bfee6d0d1e4a210e | c000000000000000 8000000000000000 c007fa74dd8b8086 | c000000000000000 c010000000000000 8000000000000000 3fdfafc0c4d36e54 8000000000000000 c0de31fb8a49c23a 0000000000000000 3fed29bf855750cc c007fa74dd8b8086
setsuper 3 3 | c010000000000000 c0d423575710dffb 8000000000000000 0000000000000000 bfd7ba97148d6eb0 bfe8d1d5414b3758 bfd7990829dcd664 8000000000000000 bfed9e92046381c8 | 40cc5d362ab23bef | c010000000000000 c0d423575710dffb 8000000000000000 40cc5d362ab23bef bfd7ba97148d6eb0 bfe8d1d5414b3758 bfd7990829dcd664 40cc5d362ab23bef bfed9e92046381c8
setsub 3 3 | 3ff85d455cb3e807 8000000000000000 40f6e615711dc525 c010000000000000 bff2b650daa95c69 bff182a36e491127 3ff0000000000000 0000000000000000 0000000000000000 | 40cc5d362ab23bef | 3ff85d455cb3e807 40cc5d362ab23bef 40f6e615711dc525 c010000000000000 bff2b650daa95c69 40cc5d362ab23bef 3ff0000000000000 0000000000000000 0000000000000000
adddiag 3 3 | 3e3a54c56812513c 40e5fde594471954 be3cc51ef3e12c30 40f56916e488e839 c0ea64c75faa7945 3e207a3ef35e52a7 4000000000000000 0000000000000000 3ff28a7677239319 | 40cc5d362ab23bef | 40cc5d362ab24919 40e5fde594471954 be3cc51ef3e12c30 40f56916e488e839 c0e34d79d4fdea49 3e207a3ef35e52a7 4000000000000000 0000000000000000 40cc5dca7e65f50c
adddiagv 3 3 | 40074115f8149dc8 bff8b0987ea738af bffeffc8a12ec51d bffc9b8ed4250a9a 0000000000000000 4005341664335f3a 3ff602f91341ede1 0000000000000000 c0d418b57beb5344 | c000000000000000 8000000000000000 c007fa74dd8b8086 | 3fed0457e0527720 bff8b0987ea738af bffeffc8a12ec51d bffc9b8ed4250a9a 0000000000000000 4005341664335f3a 3ff602f91341ede1 0000000000000000 c0d419754f923fa0
setsuperv 3 3 | bfce2fd8c9b65120 c0079a7d089a6da2 bff3f67b89348441 4005383e6809e424 3e440e0ed7f62304 0000000000000000 c0e438adf6798380 3fc4829307bda4a8 3fea8dcaf544cd6a | c007d67a711dda1f 3fe103bef1418406 | bfce2fd8c9b65120 c0079a7d089a6da2 bff3f67b89348441 c007d67a711dda1f 3e440e0ed7f62304 0000000000000000 c0e438adf6798380 3fe103bef1418406 3fea8dcaf544cd6a
setsubv 3 3 | 3ff189b96426b352 3fe28836f8a4e92c 0000000000000000 c000000000000000 3ff58315b83940e0 4004c797d578bd8e be42a6eae6af8a5e 0000000000000000 3fdebb2e51cdabf0 | 40056de3d6f1ef8e 8000000000000000 | 3ff189b96426b352 40056de3d6f1ef8e 0000000000000000 c000000000000000 3ff58315b83940e0 8000000000000000 be42a6eae6af8a5e 0000000000000000 3fdebb2e51cdabf0
setcol 3 3 0 | bff18318c675dd06 3ff20f53d882c9ba 400138555a7fd806 c002fe79ad603e4d be42890f8a8fbd07 3fefb42d349f7cde c0d93541c9cd5782 4002f7352e8a5b6c c0f00f5ccaf1a647 bff0000000000000 3ff0000000000000 bfd4a393d06d0a0c | bff0000000000000 3ff0000000000000 bfd4a393d06d0a0c c002fe79ad603e4d be42890f8a8fbd07 3fefb42d349f7cde c0d93541c9cd5782 4002f7352e8a5b6c c0f00f5ccaf1a647
setrow 3 3 0 | bfe3466e784e834e bfef966a58ecfe1a 3fffa91492e45eb9 3ffc8c0695f9d768 3fd42271d881e59c 3fed1148a906e08c 8000000000000000 3fd426e3451d0f40 c000db4bd970bbfb bfff9f80c7901d93 c00660d5ec220e2e 3fdd75602591dafc | bfff9f80c7901d93 bfef966a58ecfe1a 3fffa91492e45eb9 c00660d5ec220e2e 3fd42271d881e59c 3fed1148a906e08c 3fdd75602591dafc 3fd426e3451d0f40 c000db4bd970bbfb
setseq 3 3 0 0 1 0 3 | 3ff8b5258f2c9312 bffd9c7ea4784ee9 c003cbbd4a80960a c010000000000000 3e315ee3c394fc3e c000abdb4d090fa6 3ff2f55d87cc5a31 40eada15f4f873f7 bfe296d572c574b0 bff25fbf24f9220d c0d55339870a4ca5 be15dfe738be3113 | bff25fbf24f9220d c0d55339870a4ca5 be15dfe738be3113 c010000000000000 3e315ee3c394fc3e c000abdb4d090fa6 3ff2f55d87cc5a31 40eada15f4f873f7 bfe296d572c574b0
setseq 3 3 0 0 0 1 3 | 0000000000000000 4004dd19676401fa bfb03d193230c100 bfef66a012f7c3a8 4000bedde6ece00c 3ff7b84811ed051a 8000000000000000 0000000000000000 bff4d8bb11e4aa11 3fc9fb96e8058520 bffc80918ab3535e 0000000000000000 | 3fc9fb96e8058520 4004dd19676401fa bfb03d193230c100 bffc80918ab3535e 4000bedde6ece00c 3ff7b84811ed051a 0000000000000000 0000000000000000 bff4d8bb11e4aa11
setseq 3 3 0 0 1 1 3 | c0053802bec0e7c4 c0f16e4fd6d3f7ee 3ff29d4949435b97 c010000000000000 bffbfa8eaad7e60a 3e3ea9c9dea3381c c001624ae6ec85c8 3ff41904b086f3ee 3ffa8e33183cf7d7 0000000000000000 3fff29da89a93598 bff9d11a8088b9e9 | 0000000000000000 c0f16e4fd6d3f7ee 3ff29d4949435b97 c010000000000000 3fff29da89a93598 3e3ea9c9dea3381c c001624ae6ec85c8 3ff41904b086f3ee bff9d11a8088b9e9
load 3 3 1 2 | 0000000000000000 3fee15081a056f72 bfe86142cf574da8 3ff05ea620fffe32 c0f3ca0cf783ffd1 3fbe87aab4b44570 c0c0dc9c281d866f 0000000000000000 bff5bb0ca6ac9a32 40057f176f6a2082 4002300408c44b54 | 40057f176f6a2082 3fee15081a056f72 bfe86142cf574da8 4002300408c44b54 c0f3ca0cf783ffd1 3fbe87aab4b44570 c0c0dc9c281d866f 0000000000000000 bff5bb0ca6ac9a32
loadt 3 3 3 2 | 0000000000000000 bff4bf98136b9e58 3fe5ea5e92fba70e 0000000000000000 c004ff891f8f9676 bfd9af527b969510 bfea97a73092a3fa c010000000000000 0000000000000000 c008000000000000 3fda090ea2841cd0 8000000000000000 40001bf990f33a1c 0000000000000000 3fded2cbb7fbf764 | c008000000000000 40001bf990f33a1c 3fe5ea5e92fba70e 3fda090ea2841cd0 0000000000000000 bfd9af527b969510 8000000000000000 3fded2cbb7fbf764 0000000000000000
maxabs 5 | 8000000000000000 3fcf6c981fff6290 0000000000000000 3fc729a81d9c6438 be2f90e950eefaf5 | 3fcf6c981fff6290
mulv 4 5 | 4000c225c94a84f4 bfefc59b49f3a9aa 40014fa7e06297b9 bfdd9a23bae1256c 40068a3cb764d45f be2226dd5103a10a 0000000000000000 c000ca60d14529c4 c0e3d34c8da2fed1 0000000000000000 4005a09e7a15a1f4 40e79735854bbed0 3fefb58c8c3cbda2 0000000000000000 bff0000000000000 3feb342d49dc8928 c0f1d00751472bbd 3fc9be0154ae5250 c005fa30eb573824 be21f8e229b6ca2c 0000000000000000 bfb579cb755aa8f0 bfcb084d359ed528 bff0000000000000 0000000000000000 | 40c0bed0291796ad 3de85d2f36446f53 3fdb75bbe549092a c0c3ee0a287fa2d3
mulvt 4 5 | 4000c225c94a84f4 bfefc59b49f3a9aa 40014fa7e06297b9 bfdd9a23bae1256c 40068a3cb764d45f be2226dd5103a10a 0000000000000000 c000ca60d14529c4 c0e3d34c8da2fed1 0000000000000000 4005a09e7a15a1f4 40e79735854bbed0 3fefb58c8c3cbda2 0000000000000000 bff0000000000000 3feb342d49dc8928 c0f1d00751472bbd 3fc9be0154ae5250 c005fa30eb573824 be21f8e229b6ca2c 40eb5d3b079e5004 0000000000000000 40055f9a4ec4b106 bfe322fbcd80c8c6 | 40fca9a7001f510e 4103465899f464e7 c1e0f4195af58cb1 40eb1d2af1f0ea83 c1ee76d73c31a351
dotcol 4 5 2 | 4000c225c94a84f4 bfefc59b49f3a9aa 40014fa7e06297b9 bfdd9a23bae1256c 40068a3cb764d45f be2226dd5103a10a 0000000000000000 c000ca60d14529c4 c0e3d34c8da2fed1 0000000000000000 4005a09e7a15a1f4 40e79735854bbed0 3fefb58c8c3cbda2 0000000000000000 bff0000000000000 3feb342d49dc8928 c0f1d00751472bbd 3fc9be0154ae5250 c005fa30eb573824 be21f8e229b6ca2c 40eb5d3b079e5004 0000000000000000 40055f9a4ec4b106 bfe322fbcd80c8c6 | c1e0f4195af58cb1
frob 4 5 | 4000c225c94a84f4 bfefc59b49f3a9aa 40014fa7e06297b9 bfdd9a23bae1256c 40068a3cb764d45f be2226dd5103a10a 0000000000000000 c000ca60d14529c4 c0e3d34c8da2fed1 0000000000000000 4005a09e7a15a1f4 40e79735854bbed0 3fefb58c8c3cbda2 0000000000000000 bff0000000000000 3feb342d49dc8928 c0f1d00751472bbd 3fc9be0154ae5250 c005fa30eb573824 be21f8e229b6ca2c | 40f78d393f20195e
setdiag 4 5 | bfd1f99a277afae4 c0e6f129d6eff278 3fedf6805e56c48e bff7c1e676dad49b c0f81a2073b57b5c 0000000000000000 be440dfb7326eda0 3fe35c2b27019352 3fe786ec73abf78e 3fed114b61965e4e 4010000000000000 bfd551555efbd484 0000000000000000 0000000000000000 bfd4b85b47f03f24 3ffad8dfb3f88de9 0000000000000000 0000000000000000 0000000000000000 0000000000000000 | 3e44e5ed3627985b | 3e44e5ed3627985b c0e6f129d6eff278 3fedf6805e56c48e bff7c1e676dad49b c0f81a2073b57b5c 3e44e5ed3627985b be440dfb7326eda0 3fe35c2b27019352 3fe786ec73abf78e 3fed114b61965e4e 3e44e5ed3627985b bfd551555efbd484 0000000000000000 0000000000000000 bfd4b85b47f03f24 3e44e5ed3627985b 0000000000000000 0000000000000000 0000000000000000 0000000000000000
setdiagv 4 5 | 0000000000000000 3ff0000000000000 3ffa48237646b673 3fd749a6c8a78074 bff0f3a1d14264c6 0000000000000000 40003c5b571aab06 bfe2e0f2a6d11556 0000000000000000 bfee23c60e6d2008 4000000000000000 400169735ac0490b bfdacc7abca0ea98 4001214878c3139c 3ff392b1c9fbd2c7 40005e6bf0de6af0 400606ddb2d44c76 3ff31de9a9dd7408 0000000000000000 0000000000000000 | 3ffe669f3be79c7c 3e350ddd8ab2bbf8 4002fcc38085e30c 3ffc4236a82ab52e | 3ffe669f3be79c7c 3ff0000000000000 3ffa48237646b673 3fd749a6c8a78074 bff0f3a1d14264c6 3e350ddd8ab2bbf8 40003c5b571aab06 bfe2e0f2a6d11556 0000000000000000 bfee23c60e6d2008 4002fcc38085e30c 400169735ac0490b bfdacc7abca0ea98 4001214878c3139c 3ff392b1c9fbd2c7 3ffc4236a82ab52e 400606ddb2d44c76 3ff31de9a9dd7408 0000000000000000 0000000000000000
setsuper 4 5 | 3fe67842682d6d50 0000000000000000 bff0000000000000 40f5a919eb71b1bf 8000000000000000 40016e1d4f16b156 bfacd81fad776600 bffaacdc515439ef bfdaef56e09402dc 0000000000000000 8000000000000000 4003d9a9583c7064 3ffddde2d5b8f80d 0000000000000000 4000000000000000 3ffc449e840a8f6d c0a7b065d1d84468 8000000000000000 3fe11f168770f7e4 c010000000000000 | 3e44e5ed3627985b | 3fe67842682d6d50 0000000000000000 bff0000000000000 40f5a919eb71b1bf 3e44e5ed3627985b 40016e1d4f16b156 bfacd81fad776600 bffaacdc515439ef bfdaef56e09402dc 3e44e5ed3627985b 8000000000000000 4003d9a9583c7064 3ffddde2d5b8f80d 0000000000000000 3e44e5ed3627985b 3ffc449e840a8f6d c0a7b065d1d84468 8000000000000000 3fe11f168770f7e4 3e44e5ed3627985b
setsub 4 5 | 0000000000000000 bfd9a7bf5742a6e4 bfeb61e15cbc1bb8 400473f9b65dc4a2 3e2713c2250c554e 8000000000000000 3ff599008784b8e9 c0f804342f959897 bfd2faf3bc788d84 3fd59a3d157d7d24 0000000000000000 0000000000000000 3ff1ba93acf262aa 4010000000000000 c008000000000000 c00727220a81f582 be2d922a76ef6a77 0000000000000000 bff146aa10320f3d 3ff4581c8f091d19 | 3e44e5ed3627985b | 0000000000000000 3e44e5ed3627985b bfeb61e15cbc1bb8 400473f9b65dc4a2 3e2713c2250c554e 8000000000000000 3e44e5ed3627985b c0f804342f959897 bfd2faf3bc788d84 3fd59a3d157d7d24 0000000000000000 3e44e5ed3627985b 3ff1ba93acf262aa 4010000000000000 c008000000000000 c00727220a81f582 be2d922a76ef6a77 0000000000000000 bff146aa10320f3d 3ff4581c8f091d19
adddiag 4 5 | 3ffe5bf588ed16d3 400177d6dc649ebe 0000000000000000 3fea5425898c4412 0000000000000000 c008000000000000 bfe1a4032e40b220 3fefd971948e7ee4 0000000000000000 be15295c5997e128 4007c47600c98656 3ff33f36eed2ed6a bffa8e4ced859a06 400370a39d0cc31d c000000000000000 0000000000000000 c0f1fa447a10a0dd c000000000000000 c008000000000000 bfff2c2d041670a8 | 3e44e5ed3627985b | 3ffe5bf58b89d47a 400177d6dc649ebe 0000000000000000 3fea5425898c4412 0000000000000000 c007fffffeb1a12d bfe1a4032e40b220 3fefd971948e7ee4 0000000000000000 be15295c5997e128 4007c4760217e529 3ff33f36eed2ed6a bffa8e4ced859a06 400370a39d0cc31d c000000000000000 3e44e5ed3627985b c0f1fa447a10a0dd c000000000000000 c008000000000000 bfff2c2d041670a8
adddiagv 4 5 | 0000000000000000 0000000000000000 c0074c1d20793913 0000000000000000 4003838012b94606 3fdc0d8b77c61a80 bffbe9d6087b97fc 3fd415a19a2ce768 c005bef96b05212c bfebce2f090f0e6c bff88b621c96b7ef c000000000000000 bffbb81334bc171a c0f5dad9bd30630f 4008000000000000 0000000000000000 be44dce9c99fa1f8 c001b251d32606ec c000fe6c150aa722 4010000000000000 | 3ffe669f3be79c7c 3e350ddd8ab2bbf8 4002fcc38085e30c 3ffc4236a82ab52e | 3ffe669f3be79c7c 0000000000000000 c0074c1d20793913 0000000000000000 4003838012b94606 3fdc0d8b7d0991e3 bffbe9d6087b97fc 3fd415a19a2ce768 c005bef96b05212c bfebce2f090f0e6c 3feadc49c8ea1c52 c000000000000000 bffbb81334bc171a c0f5dad9bd30630f 4008000000000000 3ffc4236a82ab52e be44dce9c99fa1f8 c001b251d32606ec c000fe6c150aa722 4010000000000000
setsuperv 4 5 | 0000000000000000 0000000000000000 0000000000000000 3e2619c1471e8a3f 0000000000000000 3e22db21fccd461f 40035005f3072116 4000a3170606fe36 c0f74459405a1778 40050523cffcda62 3fdc525eac73a198 3fde48eb06035ed0 8000000000000000 3e36e47923601115 3ff2b512dc78d4b3 3fc189b122373498 3ff5262ae26c9daa c001e1e88188960a 3fed551868e7c28c bfd1af6e643abd08 | bfe78a69b1f597be c007d3d55b8d32f4 bff33ea619494d3a | 0000000000000000 0000000000000000 0000000000000000 3e2619c1471e8a3f bfe78a69b1f597be 3e22db21fccd461f 40035005f3072116 4000a3170606fe36 c0f74459405a1778 c007d3d55b8d32f4 3fdc525eac73a198 3fde48eb06035ed0 8000000000000000 3e36e47923601115 bff33ea619494d3a 3fc189b122373498 3ff5262ae26c9daa c001e1e88188960a 3fed551868e7c28c bfd1af6e643abd08
setsubv 4 5 | bfe217f68efc5202 bff9f07b667471df 0000000000000000 c000ab087ba537f4 be3a67b3e37f9b2b 0000000000000000 0000000000000000 3ff37419535259ab bffeb9ebb690fb95 be3840b4197f9fc6 c0f5efff8d7573b5 3ff951c53e1055b3 3ff1d2cc6e09e54f 4000000000000000 8000000000000000 3fe09dc6c40f4a70 0000000000000000 bff018930b3ac974 400522a18745906c 40f362ca7dc25d08 | 0000000000000000 40f15efc266f7de4 0000000000000000 | bfe217f68efc5202 0000000000000000 0000000000000000 c000ab087ba537f4 be3a67b3e37f9b2b 0000000000000000 40f15efc266f7de4 3ff37419535259ab bffeb9ebb690fb95 be3840b4197f9fc6 c0f5efff8d7573b5 0000000000000000 3ff1d2cc6e09e54f 4000000000000000 8000000000000000 3fe09dc6c40f4a70 0000000000000000 bff018930b3ac974 400522a18745906c 40f362ca7dc25d08
setcol 4 5 2 | c0e4db57f03c7003 8000000000000000 40f5917776d57fa6 4003137f178d214a c00195dbc926e0a0 c0d666ab169f984f c00566b5925f924c c004421c624faa06 3ff8b78054443bc3 0000000000000000 c010000000000000 3fff0396c1ecb606 c0ec16bb77f7b56d 3fc52606b15b19e0 3ffbb5848257e608 400635175210a7c0 3fdc0c567a6150a0 0000000000000000 bfe7161befaccbb2 3e27edd8777620ef bf942d50de0c3500 be32cf71853f7b8b bfec329dd6f4c906 3e4097898a895932 | c0e4db57f03c7003 8000000000000000 40f5917776d57fa6 4003137f178d214a c00195dbc926e0a0 c0d666ab169f984f c00566b5925f924c c004421c624faa06 bf942d50de0c3500 be32cf71853f7b8b bfec329dd6f4c906 3e4097898a895932 c0ec16bb77f7b56d 3fc52606b15b19e0 3ffbb5848257e608 400635175210a7c0 3fdc0c567a6150a0 0000000000000000 bfe7161befaccbb2 3e27edd8777620ef
setrow 4 5 3 | bfde7a16a71fb094 c0058d38deb684dd 40ed02beb886fb54 3fdd1eb47efaa338 bfeb837f86811d44 3ff0000000000000 bfb06468ef8b4e20 4008000000000000 bfd98ac3712e4b80 bfd26170d0c282c0 3fea749adb8bdda0 3fee5e881d2fe75a bff6c7f503609662 c0f306089e46ab15 bff6b78b5cdae60d c008000000000000 3ff1394854317be4 c0f753f846d761f8 40042abccb130ed2 0000000000000000 0000000000000000 3ff53d1134b99b5e c0ee14bafd23ec43 c0f7b8345d5bcf99 3ff722ea955ec7ec | bfde7a16a71fb094 c0058d38deb684dd 40ed02beb886fb54 0000000000000000 bfeb837f86811d44 3ff0000000000000 bfb06468ef8b4e20 3ff53d1134b99b5e bfd98ac3712e4b80 bfd26170d0c282c0 3fea749adb8bdda0 c0ee14bafd23ec43 bff6c7f503609662 c0f306089e46ab15 bff6b78b5cdae60d c0f7b8345d5bcf99 3ff1394854317be4 c0f753f846d761f8 40042abccb130ed2 0000000000000000
setseq 4 5 0 0 1 0 4 | bfe3818ab9d59f96 bff59a09eaf0805a bffcb419f215b4e3 bffc5a0b3589b64e c000000000000000 c003c120e2a7806a bfb74d52d07f7f40 4010000000000000 3ff6cef1bdfb49f2 bffcfa99420fd1aa bfe205a94431e856 8000000000000000 4000c2360255e5d8 bfbf962588bc0f40 400259d581d046e6 3ffdeffc81d2ba20 c0052be87672fde9 3fd146c00abf1560 0000000000000000 3faa896dced92a40 be41ab1968e43af8 c010000000000000 40f74c079f849d32 c0e3e3b204b17284 | be41ab1968e43af8 c010000000000000 40f74c079f849d32 c0e3e3b204b17284 c000000000000000 c003c120e2a7806a bfb74d52d07f7f40 4010000000000000 3ff6cef1bdfb49f2 bffcfa99420fd1aa bfe205a94431e856 8000000000000000 4000c2360255e5d8 bfbf962588bc0f40 400259d581d046e6 3ffdeffc81d2ba20 c0052be87672fde9 3fd146c00abf1560 0000000000000000 3faa896dced92a40
setseq 4 5 0 0 0 1 5 | c007f688f7219b3a 3feb3cb3c6fb0710 3fb6c387b4f747b0 3e3d136f17321670 3fc8ffcca4f2b5a0 0000000000000000 c0f070c97f862f96 bff1a840d086b364 3ff8e2081cf94ca6 bff8c0fea389de6a bfff8895eae7aa3f 3fee17e094aefcf4 bff8d6a180c4e78a 40051f9c9dcfe0d2 be3ef54742657a4b bffc116161aa72e6 c0ec3a1abbad8485 3fff5c06fe1a9236 bfdf0f40e80093cc 40038a5a93c8fff8 bfef04915f59e3a2 3e34ded0cc983b05 3e3577d9beee26b4 3feba0040a613044 bfd46c5d1049d394 | bfef04915f59e3a2 3feb3cb3c6fb0710 3fb6c387b4f747b0 3e3d136f17321670 3e34ded0cc983b05 0000000000000000 c0f070c97f862f96 bff1a840d086b364 3e3577d9beee26b4 bff8c0fea389de6a bfff8895eae7aa3f 3fee17e094aefcf4 3feba0040a613044 40051f9c9dcfe0d2 be3ef54742657a4b bffc116161aa72e6 bfd46c5d1049d394 3fff5c06fe1a9236 bfdf0f40e80093cc 40038a5a93c8fff8
setseq 4 5 0 0 1 1 4 | 0000000000000000 3ff0d0900012bf53 3e4226875b066728 4001fdc6ec6684c4 3fec0ec41b9321f2 bff608197d82d9fd c00536b6ec4f8ab6 c0d573ee84720405 bff7b7f792040bee 8000000000000000 3df54a30eda50e42 40043cc2b26b9092 c0038ab78347b9b2 40f63b9de55f2ef9 0000000000000000 3fe17bdb3f89cea8 3e27359fae4373a8 bff64c5b744f40ec 3ff0000000000000 0000000000000000 bfe4c36e6f332d74 3fd5b8fe36859e30 bfe02adbc0fba70a 3fd303be986ca340 | bfe4c36e6f332d74 3ff0d0900012bf53 3e4226875b066728 4001fdc6ec6684c4 3fec0ec41b9321f2 3fd5b8fe36859e30 c00536b6ec4f8ab6 c0d573ee84720405 bff7b7f792040bee 8000000000000000 bfe02adbc0fba70a 40043cc2b26b9092 c0038ab78347b9b2 40f63b9de55f2ef9 0000000000000000 3fd303be986ca340 3e27359fae4373a8 bff64c5b744f40ec 3ff0000000000000 0000000000000000
load 4 5 3 4 | be3d3ba190b25d96 be2ce4a8599ac36b c0ed1d4548746aca bfe8c89d32ad71d2 0000000000000000 4001cd971be3d172 bffae1612f7b5539 c0ea1a01adf3886f c0b4f2182ec13fc4 c0025681aa24205a 3e42dd51895b8e1f bfe1cd46f9dfcb92 be3229079b2fedcf 0000000000000000 3ff0000000000000 4000000000000000 3ff2759c7aa9e5b6 bff41e2b888484c2 c010000000000000 0000000000000000 3ff8fae1db167fea be334434a7f464da 0000000000000000 bfee3c381617471a bff9640534917068 40e37b11f3521624 3fc5099f9d5eab70 bfcb83961313c080 c0d1bc8d6f0ae85a 3e009af71483fe12 bfef637b296ff438 c008000000000000 | 3ff8fae1db167fea be334434a7f464da 0000000000000000 bfe8c89d32ad71d2 bfee3c381617471a bff9640534917068 40e37b11f3521624 c0ea1a01adf3886f 3fc5099f9d5eab70 bfcb83961313c080 c0d1bc8d6f0ae85a bfe1cd46f9dfcb92 3e009af71483fe12 bfef637b296ff438 c008000000000000 4000000000000000 3ff2759c7aa9e5b6 bff41e2b888484c2 c010000000000000 0000000000000000
loadt 4 5 2 2 | bfd61549e8522b70 40071482e2319816 3ff351ed97ef9470 0000000000000000 3fd5230c57435f64 400499d43776eef0 0000000000000000 c0f00098a9e6090c 4002a1607066ca68 bfbaa6cc5cb69020 c000b497899d7d0e c010000000000000 bff34e550a75a99e c0e58f24c108fcc0 8000000000000000 3feaf4c9b2c51e36 400121f6c6fd8830 bffd812bc71d3492 c006b457546fa330 bfb5e51138abca00 bfb5028890485460 4003c6e2bffedae8 c00354b037b11342 8000000000000000 | bfb5028890485460 c00354b037b11342 3ff351ed97ef9470 0000000000000000 4003c6e2bffedae8 8000000000000000 0000000000000000 c0f00098a9e6090c 4002a1607066ca68 bfbaa6cc5cb69020 c000b497899d7d0e c010000000000000 bff34e550a75a99e c0e58f24c108fcc0 8000000000000000 3feaf4c9b2c51e36 400121f6c6fd8830 bffd812bc71d3492 c006b457546fa330 bfb5e51138abca00
maxabs 3 | 3f97831ae717fc40 3ff09d44f7da2063 0000000000000000 | 3ff09d44f7da2063
mulv 5 3 | 4002279b0e037ca8 c008000000000000 3fc5239526639c90 bff9fd6cc2350e70 4010000000000000 c00532aa6c0bc00c 3fd4d5e3d64b4f0c 3ff33dda94983fc8 c00278d9621c650f c00789b40794d1bc c002173c7328dec5 be44a697c47adbc9 c0f81635c3d40bb0 3ff359f1f9abdd30 bff32ad7cea37ee1 40ce8ef0456b4c3b 3fe99e2baefac484 c0dd63d88dd15c35 | 40f9487bde7cede7 c0e6eb2bdaa6493b 41e61f49b393b5d7 c0ee2ef68183cd09 40f814a35d636fc4
mulvt 5 3 | 4002279b0e037ca8 c008000000000000 3fc5239526639c90 bff9fd6cc2350e70 4010000000000000 c00532aa6c0bc00c 3fd4d5e3d64b4f0c 3ff33dda94983fc8 c00278d9621c650f c00789b40794d1bc c002173c7328dec5 be44a697c47adbc9 c0f81635c3d40bb0 3ff359f1f9abdd30 bff32ad7cea37ee1 be0c6a406deb220d 3ffed708e41f438a 0000000000000000 bff891e433b3e2e9 c000000000000000 | c026937f8614abb8 40241d8fb9f7cad0 3fe13ca30249d206
dotcol 5 3 0 | 4002279b0e037ca8 c008000000000000 3fc5239526639c90 bff9fd6cc2350e70 4010000000000000 c00532aa6c0bc00c 3fd4d5e3d64b4f0c 3ff33dda94983fc8 c00278d9621c650f c00789b40794d1bc c002173c7328dec5 be44a697c47adbc9 c0f81635c3d40bb0 3ff359f1f9abdd30 bff32ad7cea37ee1 be0c6a406deb220d 3ffed708e41f438a 0000000000000000 bff891e433b3e2e9 c000000000000000 | c026937f8614abb8
frob 5 3 | 4002279b0e037ca8 c008000000000000 3fc5239526639c90 bff9fd6cc2350e70 4010000000000000 c00532aa6c0bc00c 3fd4d5e3d64b4f0c 3ff33dda94983fc8 c00278d9621c650f c00789b40794d1bc c002173c7328dec5 be44a697c47adbc9 c0f81635c3d40bb0 3ff359f1f9abdd30 bff32ad7cea37ee1 | 40f81635c524e709
setdiag 5 3 | bfba7a9f9025f1c0 0000000000000000 4000ea1b05ca59ce 4001583f8427b07f 3ffa69802d4adb48 0000000000000000 0000000000000000 bff0000000000000 c00571a9adc0da65 0000000000000000 0000000000000000 3ff6e65767ee92e9 40033cfafec2d1a0 c0035b7f1708c340 c005dd9aee884ee4 | be3382a591056c4e | be3382a591056c4e 0000000000000000 4000ea1b05ca59ce 4001583f8427b07f 3ffa69802d4adb48 0000000000000000 be3382a591056c4e bff0000000000000 c00571a9adc0da65 0000000000000000 0000000000000000 3ff6e65767ee92e9 be3382a591056c4e c0035b7f1708c340 c005dd9aee884ee4
setdiagv 5 3 | 400114cdb7626bf0 3fd4160605ff44a4 0000000000000000 c008000000000000 be06e3ae84b14014 3fd5f2bd44ef1c80 4004ede766972ae1 bff6e801d984781d bfe23a91ddb62958 0000000000000000 40011dd0f53e0afc be43132d73413780 3fdfafd6c90abcfc 4003ce3ed4d5a8e4 4001e49e9eddd372 | 3ff61d8715bb5d8c 3ff6f7ce16af2538 4010000000000000 | 3ff61d8715bb5d8c 3fd4160605ff44a4 0000000000000000 c008000000000000 be06e3ae84b14014 3fd5f2bd44ef1c80 3ff6f7ce16af2538 bff6e801d984781d bfe23a91ddb62958 0000000000000000 40011dd0f53e0afc be43132d73413780 4010000000000000 4003ce3ed4d5a8e4 4001e49e9eddd372
setsuper 5 3 | 3fd218e4a40ef818 40058fb90e00b04a c008000000000000 c0067cdf738fa6e3 c0e32dc9ac9a54c2 3ffdcec9e6d0c19d c000000000000000 40067117e3a69044 40d9f60c6c8ed676 40006039f6e8293c bffcfba6d20c34b8 0000000000000000 4001df3e1d289ed7 0000000000000000 4099ba2d80821b5b | be3382a591056c4e | 3fd218e4a40ef818 40058fb90e00b04a c008000000000000 c0067cdf738fa6e3 c0e32dc9ac9a54c2 be3382a591056c4e c000000000000000 40067117e3a69044 40d9f60c6c8ed676 40006039f6e8293c bffcfba6d20c34b8 be3382a591056c4e 4001df3e1d289ed7 0000000000000000 4099ba2d80821b5b
setsub 5 3 | 4008000000000000 40f73b39d9c07462 3ff205809706948a 8000000000000000 bfb9d7cd9684aeb0 3fd675c873b9e4fc 40077055e680219d bffa504dd42f5d5f be3411639e1c5c4f c0f70e554d550ce8 bfede157565128d6 0000000000000000 3fe867b2e19e09b0 3e38b21e42ff2de8 40e4ff81b65854d9 | be3382a591056c4e | 4008000000000000 be3382a591056c4e 3ff205809706948a 8000000000000000 bfb9d7cd9684aeb0 3fd675c873b9e4fc 40077055e680219d be3382a591056c4e be3411639e1c5c4f c0f70e554d550ce8 bfede157565128d6 0000000000000000 3fe867b2e19e09b0 3e38b21e42ff2de8 40e4ff81b65854d9
adddiag 5 3 | c0e0708c8eb89f8a 0000000000000000 bff2110e20455700 3e32f77793644242 8000000000000000 40060d4344126b5e 0000000000000000 40041e6e268b99f0 c006e03d4bec85b6 bfb6491f2406db90 40db6f0960ac55df bfe83bfaf1b298f8 3e2b1eed1106758b 3fdfea678513d52c 0000000000000000 | be3382a591056c4e | c0e0708c8eb8a1fa 0000000000000000 bff2110e20455700 3e32f77793644242 8000000000000000 40060d4344126b5e be3382a591056c4e 40041e6e268b99f0 c006e03d4bec85b6 bfb6491f2406db90 40db6f0960ac55df bfe83bfaf1b298f8 be17ccbc2208c622 3fdfea678513d52c 0000000000000000
adddiagv 5 3 | bfd9de51570de530 bfa1361e6e5017a0 c0e62d3b344e66c3 3ff4258e6648cb27 8000000000000000 40e154f669a36110 bffa89a8cf44e77e 3ff6f7ba380d9754 bff9a31c2ad925b5 3fdd924c5791e49c c0e648c1da2b3964 bfe0a9f0d838c92c bff5858af6e32fd0 4000000000000000 bfe9dfb086036600 | 3ff61d8715bb5d8c 3ff6f7ce16af2538 4010000000000000 | 3fef4be57fefc880 bfa1361e6e5017a0 c0e62d3b344e66c3 3ff4258e6648cb27 8000000000000000 40e154f669a36110 bfcc8ed5c4ae1230 3ff6f7ba380d9754 bff9a31c2ad925b5 3fdd924c5791e49c c0e648c1da2b3964 bfe0a9f0d838c92c 40053d3a848e6818 4000000000000000 bfe9dfb086036600
setsubv 5 3 | c0071b5e6b863130 bfca84e36d4db930 0000000000000000 be3900e2803eda16 c0babb4e9d7ce327 c0e096854744101d 0000000000000000 40e1e3a0c4f5e30c 3e3c8c6551a68012 0000000000000000 40eb14e2ec8fb0e9 3e37fda69aaa7c8b 0000000000000000 bfef7fabf059620c bffc81cb7ee6668b | c010000000000000 bff7aaeb32e22752 | c0071b5e6b863130 c010000000000000 0000000000000000 be3900e2803eda16 c0babb4e9d7ce327 c0e096854744101d 0000000000000000 bff7aaeb32e22752 3e3c8c6551a68012 0000000000000000 40eb14e2ec8fb0e9 3e37fda69aaa7c8b 0000000000000000 bfef7fabf059620c bffc81cb7ee6668b
setcol 5 3 0 | bffd1e3b7226eec5 bfe4695280e09b7e 0000000000000000 bfd66e5d5d3fe298 c0d1f4a3bfc0af9e bff939d70e162fbb 0000000000000000 3e41249ddf623a14 0000000000000000 0000000000000000 3e2b6a46343c165c 0000000000000000 bfe371317e4744d0 3e401c27d9475655 bfc1d32e04aa2450 3fab48b391dbc480 0000000000000000 40068b45b9d62c77 bfa0ba711c035420 c0f1a91912564b99 | 3fab48b391dbc480 0000000000000000 40068b45b9d62c77 bfa0ba711c035420 c0f1a91912564b99 bff939d70e162fbb 0000000000000000 3e41249ddf623a14 0000000000000000 0000000000000000 3e2b6a46343c165c 0000000000000000 bfe371317e4744d0 3e401c27d9475655 bfc1d32e04aa2450
setseq 5 3 0 0 1 0 5 | be3f9876f69ea51a 3fe158eae370f708 0000000000000000 c0045aa11411a339 8000000000000000 4005ccf56fa8c64e 0000000000000000 0000000000000000 be26edea9fb07005 c00375cdda07b41f 0000000000000000 0000000000000000 bff95e2b642efb44 bffc4770eacf4af0 4004919a5537d82c c0048dcce51ac182 40f70d69e13afe39 c000000000000000 8000000000000000 c00005701dd6adfc | c0048dcce51ac182 40f70d69e13afe39 c000000000000000 8000000000000000 c00005701dd6adfc 4005ccf56fa8c64e 0000000000000000 0000000000000000 be26edea9fb07005 c00375cdda07b41f 0000000000000000 0000000000000000 bff95e2b642efb44 bffc4770eacf4af0 4004919a5537d82c
setseq 5 3 0 0 0 1 3 | bff538d24ef7bd6c bfd2c04d5a439354 bff4ba64ab4ed5a8 3fc98ea7e8171ec8 400035c1dc32b5ba bffc5dbea1a878d7 0000000000000000 4000001195d1c650 3ff9334809141b7e 8000000000000000 4000000000000000 0000000000000000 3e2d22bdf7e37144 3ff0000000000000 c000cf19705582aa 0000000000000000 c0050f7ae15e9def 3e3a18cb38275a01 | 0000000000000000 bfd2c04d5a439354 bff4ba64ab4ed5a8 3fc98ea7e8171ec8 400035c1dc32b5ba c0050f7ae15e9def 0000000000000000 4000001195d1c650 3ff9334809141b7e 8000000000000000 3e3a18cb38275a01 0000000000000000 3e2d22bdf7e37144 3ff0000000000000 c000cf19705582aa
setseq 5 3 0 0 1 1 3 | bfbf362c7b26fc10 3ff3c74fff1ffdeb 8000000000000000 0000000000000000 3fd48ec945b6eb00 0000000000000000 3ff6db19bceaa2d4 0000000000000000 0000000000000000 3fe0a0b1a9ec7f1e 3fbbd52a34ef9120 bfd9b663e53ff758 c0dfe8e95a83ad33 3fcf2cf52a008770 4003ff0ab12fe9bd c00338469b0a60b9 bffc3d3932b455af bffc1b6ad5a8be3c | c00338469b0a60b9 3ff3c74fff1ffdeb 8000000000000000 0000000000000000 3fd48ec945b6eb00 0000000000000000 bffc3d3932b455af 0000000000000000 0000000000000000 3fe0a0b1a9ec7f1e 3fbbd52a34ef9120 bfd9b663e53ff758 bffc1b6ad5a8be3c 3fcf2cf52a008770 4003ff0ab12fe9bd
load 5 3 2 2 | 40071e87a1842d9a 0000000000000000 3ff4b301fdef163c 0000000000000000 bff64514b56cf23f 40e81d83bce8ba79 3ff854e3db789b20 3e423045fc12ea11 8000000000000000 bffcd3e97c6e986f bfd8290e0f759494 3ff8016700918a1e 3ff49abd382ecc72 bff8f68945bca028 3ff61524ad691008 c0032e86c1583b58 4010000000000000 0000000000000000 4010000000000000 | c0032e86c1583b58 4010000000000000 3ff4b301fdef163c 0000000000000000 bff64514b56cf23f 0000000000000000 4010000000000000 3e423045fc12ea11 8000000000000000 bffcd3e97c6e986f bfd8290e0f759494 3ff8016700918a1e 3ff49abd382ecc72 bff8f68945bca028 3ff61524ad691008
loadt 5 3 2 3 | c00770c75d4f9553 3fec5630aba55d7e bfdc0dd90f4884f0 8000000000000000 8000000000000000 400747133eda239c c0072cfa46c93694 0000000000000000 bffb7e60e5ff6617 c006d0e8bcb2456c 0000000000000000 be3532f9cf133c76 0000000000000000 40043e7e46e1217e 40054b87d6356a76 3e2105968b172b7b bfe6e39293e367a6 c008000000000000 4003385c2845357c 3fe4e35e29e8d2e8 4000000000000000 | 3e2105968b172b7b c008000000000000 3fe4e35e29e8d2e8 8000000000000000 8000000000000000 bfe6e39293e367a6 4003385c2845357c 4000000000000000 bffb7e60e5ff6617 c006d0e8bcb2456c 0000000000000000 be3532f9cf133c76 0000000000000000 40043e7e46e1217e 40054b87d6356a76
mul 1 1 1 | 3fae4929965cf400 3ff8d2e96c4a341f | 3fb77e7c3ce46eb6
tmul 1 1 1 | 8000000000000000 3fcdc50e4c1b9668 | 0000000000000000
mult 1 1 1 | c0033b18551bb6be bff1368434a2b85f | 4004b050335910dc
mscalar 1 1 1 | 3e3ccff04c6dc5d4 3fd172d45f8d7fec be402314ef9fce4a | 3e1f6bb750969698
mul 2 3 2 | 40029afb77eb6ea3 0000000000000000 3fe25ef366e0c0cc 3fec493b84174794 40061d94299dd58a be42922dba0d199e 3f98a367ff547f00 8000000000000000 3ff9f69bd9a6d85b 40f205bad07fe034 0000000000000000 c0045d2a27df1206 | 40122ad7d33d6d56 be4e22a3d67c2fb4 4104f4cc461e5423 3e57a2dbc0590e4b
tmul 2 3 2 | c0ef421085972cb9 400599c72da3315e bffcbfd5a09c69ec 3fe99259864cac8a bff6450e8cb465ef 3ff52c723151438c 400362bce493a5aa 3f9891c2b565f180 3ff499d62f37c378 4008000000000000 0000000000000000 0000000000000000 | c102efbea2c03b83 400cdaf8f8568458 c107718c6431618b 40032dc324b98168
mult 2 3 2 | c007b1b260c9f771 bff5b35479a11156 bfe13bec44a56ee6 0000000000000000 c000000000000000 0000000000000000 bf8fa4ac35197b80 c0074e8c6e64a33a 0000000000000000 be365b3f02435e2c bfddae77f831e018 bfe0d6b2cc8467a4 | 3fef255928581aa4 3f957565aeabbbc9 40235cb7ad2f7890 400f9c5221850557
mscalar 2 3 2 | 3fca1bb001699400 3fe765437da7d59c 8000000000000000 0000000000000000 c0052c703b12906d 0000000000000000 be40d825952c20ff 3ff2581610582f03 be400aa06aea0800 4005efa3488529a2 4010000000000000 | be1b7c631ced819b be38a1518113cd2f be1b7c631ced819b be38a1518113cd2f
mul 3 2 4 | 40043dc950c54f68 3e374207b8955702 3de2c3476812787e be35865cd21c5b4b 0000000000000000 bff28b4df4ad6768 c000855d3d973d3d 3fda2433f5c94ab4 c0f1612847dab235 400398f59599bd40 be4351a45cfb032c 40031d63b5c94ade 3ff95e5360953df2 40044f46a87654df | c014e680df64ec2b be4803e4221826e6 bfde4c545634af15 c105fc8f6fa974c3 bf3943631073ec3f c006b6b9b1fa4125 be62a619de6ca89a bc8c15165e3f7c30 c006277ca07cedb1 40100bef82f6c121 3e42702498597910 c0078a03276e503e
tmul 3 2 4 | 0000000000000000 be3148234816d06c 3fe58917265cae5e 0000000000000000 bfd5d39f99eb7fb4 bffd29f963564d2b c0036c1a0f418053 3fef77e3432d66e4 3e380a0487eb7bd5 c0019344457eaf56 3feefe76c76fbe16 3ff3dd30e00d0fd5 bff4335c6da3a3f2 3e325ddf8044b2c2 | be30fea12d0c0503 bffa244bed1f4e5a bfeedd14147506d9 3e42fbb5fe646f0d 3e302d8f409d384b 4010048386cd4c3f be3574931758b48b 3fe4dbc5cb747c02 c004bf014d6a01a4 bc73d68c6623b9be bfeb307e51956cf2 3fdb8e981b290930
mult 3 2 4 | 3fe7ba3967806232 4002daa110628033 3ffd26abb8f32455 0000000000000000 3ffc9c2c728426c7 0000000000000000 0000000000000000 c008000000000000 4003cffcb4d6f241 40ed0b517bda029f 0000000000000000 be3b97763305eef5 3e3f61f80effba32 bff9f939b560272e | 0000000000000000 0000000000000000 0000000000000000 c001cbab0da049a6 c01c47f1995919cd c015dd00cab65b40 3ffd619432c96785 401758b5909b0952 40120c6e51ea681d 40e5892874beae5a 41011cb5e7c1d9e4 40fa755b2559d68d
mscalar 3 2 4 | c0f600a891affe08 3fd23c79699d50bc bfdf94cec75068b4 4000dddbaf4bed35 bfe311da18b246a4 0000000000000000 40028203d7db15fa c008000000000000 c002ab6028266cb2 407c63cd2f1fb74a c004decda02557ba 0000000000000000 3ff0000000000000 8000000000000000 0000000000000000 bfeee364f224824e bfe47de1b95afaf8 0000000000000000 bffaab68bad3098e | c1097388467b6f6c 3fe5183852a83116 bff24404a32259a1 c1097388467b6f6c 3fe5183852a83116 bff24404a32259a1 c1097388467b6f6c 3fe5183852a83116 bff24404a32259a1 c1097388467b6f6c 3fe5183852a83116 bff24404a32259a1
mul 4 4 4 | c007d2e07ae105e4 bf89899bc0b86900 3fdbd8c24fae2700 be37ed1e255c8b9c bfe7a1ecfe1f822c bffa65e46b60bcf1 3ff3f77f9d40753d 3fedec21823c9c58 3e20e417593b3459 bff00cf77b733d29 bffb0f4dc1de130b 3ff762119f67071f 4004091950ee1893 c0d4c6a333f050c4 40cf2b8293e4821b 3fbd06131e41e9b0 bff68f8c7de79458 4007b141bfbb325c 40f609e9397d87f0 3ffe1f0f201a716e 4010000000000000 c010000000000000 bff88ca3cbd74634 bfaff12da52011c0 3e42fe488a3b0447 3fff2d811ee3d5a8 3ff939ecb9c0a95d c008000000000000 c0042a0d801e0b10 40002b4d0e830f76 bfddec5a7d07f5b4 c0063c7a82cda34b | 401ae85c8edf3d58 c0ffe34511a74e06 c0fdefd9d232ebf9 41001ac1fb33e7af c0223a6481f80afb 4094dd5ead6e32d1 c08f2252db0124f9 c017f577e599f3d5 c021e7a2125b1a34 40ef295b502f6ae8 c0e760a9720b2bf1 400e4a06451da2cc bfee5404ab65c678 40ecdf4dae535d15 c0e5a88bc4111b34 3fec871f4a54889a
tmul 4 4 4 | bffd7a2feb3b7077 bff9bf34d23d49ed c005582e0e68ad34 c006b7165a1132c2 40f2819dbd5d9f02 4002e30907938472 be38b960fcb60e6f 40015d229b48d92c 4004aec60454e830 40055bb46a1d129a be2013ed867dc2e4 3ff1c4629eecdd25 3fef9fe07e67ccc2 bff3464035539c47 bff8bd099766b072 0000000000000000 3fd97bc4dfe19084 c008000000000000 0000000000000000 0000000000000000 40074c779f297553 c001fcacfa76e48e bffa01dcee561e4a 0000000000000000 4005a87fd1342ba2 c005aa145171d58a bfd8e74338cc9318 0000000000000000 bfd925a39c4b39e4 3ff483c2858f6888 3fe29d128acbcc48 400410d27dfb9e9a | 401060388f5b0b08 40dd77e952690e71 c01beb6c303eb6c8 401007a3afa125fe 4004b70366c829c5 410af29a529c1938 3ff86d6a08c3495e 40203322b4fae1c6 3fda2051b79e5350 41090cd0b21c8fac bfcd86411d7766e7 401a28560d47ca23 c02406b5a331caa8 c0dd13f14c7aa5fd 4014c51f1362dfe8 c006a869b9589ea5
mult 4 4 4 | bff30c53564ac9d1 0000000000000000 c000a2813ad2d738 3e371f5a86fe1f86 3fefc7f9d593ee7e 8000000000000000 0000000000000000 3fe32cd53cda02e0 0000000000000000 4005404a80ec2194 40019ba9602afdc4 c006a616f6d12d9c bfb628f9bbf64e00 be21931c74dd76e4 3fe85fbb4d2f276e bffbca7fb114ba1a c000000000000000 8000000000000000 3ff0000000000000 c00340d2e60eb512 bff1e8eb7fb1628f c00264dd9a954108 c006b8ae4a217512 bff0000000000000 c0041a87e508fb44 0000000000000000 3ff7812e8338daab 3ff7a8d45109d5d1 0000000000000000 bff181c11e2ce3a1 4008000000000000 0000000000000000 | 3ff44f163c6e0070 c01ab39a18e161ad bff5f513f8484738 4019c6521ab16870 c00182af9210a901 3e233ad47145ed82 bfeaab6108db9a53 3fe0ba6c2604b4c4 c0111573e692342e 400f37ff385012d9 400b8338eed5ae77 c02624b694108732 3ffdf3a1c42af82f 400f6ca8271ad532 402084934483373f c013247968237df8
mscalar 4 4 4 | bfd1373019c48fa4 bff0000000000000 40e87a45f2e5643c 3fe311e431223952 bfd4887947f0192c 3e41e737b7afe1bc bfe5e30959d6876a bff6bf23f44780d4 0000000000000000 3df747350fdc2d68 0000000000000000 c0e3dba5d8eb0c40 c0e8905c47c63b62 3fdc6a73f94939e0 bff4d153e3c0c9e5 3fde75fb209a20f8 4006d54eb7331f6d 3fe9e460619a64b0 c00652f81fe4835a 3e3a7e736bbeea49 3ffc6f0513b097fb 4010000000000000 3e1d852d5c09bcc2 c006a910bef6adb2 bff7ea2b1a3adfd4 c0d04d3f9fcbe418 bf90d2d944b19ec0 0000000000000000 bffaeec1c099ec78 c0e885f4c3615d22 8000000000000000 0000000000000000 c006e0c494c4ee24 | bfe8916585ee03b8 c006d54eb7331f6d 4101773a2ec356ca 3ffb36d5c2ef2661 bfe8916585ee03b8 c006d54eb7331f6d 4101773a2ec356ca 3ffb36d5c2ef2661 bfe8916585ee03b8 c006d54eb7331f6d 4101773a2ec356ca 3ffb36d5c2ef2661 bfe8916585ee03b8 c006d54eb7331f6d 4101773a2ec356ca 3ffb36d5c2ef2661
mul 5 3 1 | 3ff25ae8c43e6287 c008000000000000 bfa868c142a86c00 3feb12b781c4a808 3fed7f47548349b2 c006a65efeceb300 3e3b3316ea8dce04 4006b7cee0fca7ae c008000000000000 bdd0c3a05e6dd59c c000000000000000 c0051ab85359385d c00609524a24e027 bfd2fc1d5542a9b0 c0a574552a9b4b7a bffbff892b8e1580 40c6e18a65e3c67c 40d3e3ff937cfdb4 | c0f20b2270a86000 c0ea3bcf18a9e459 c0d64d2cd3030c62 c0e41c948b328344 c18aabde5b9e1cf7
tmul 5 3 1 | be3a968203ecf183 40f0f326e45aabc2 bf94897bcbb95440 3ffda6d300972392 bff7a6d47494a885 bff6338435186c27 bfc548f5800cbc18 40ebe34426d807ad 3ff4435484f5b680 c006907cdf50513e 40f22442e2210f66 40077efd4329c218 3fe62fdc712aaa82 0000000000000000 0000000000000000 4001e0487882782b c010000000000000 bfda90afd6091a6c | c110f326dbd464a2 40254287903bf358 c10be34b54691d6a c1122460f84d5a7f 3ff8c9dcc4e98890
mult 5 3 1 | c0030d75d35f1177 bfebb556c4f41b88 c010000000000000 c00309df7c07277a 0000000000000000 0000000000000000 4000e8f54d0e0cda 40e8a42197470032 3ff665f345978d05 be3318d7ef6102ac c000690c962068cc 3ff3aab0d2512926 40070566d0b0762a 3ffbe67b46b0172c c0037a3996f88eea 0000000000000000 3ff4d07671ab5262 c00207707bb23cee | 40127dcf7c9c9ee2 bf94d3ccb4d17500 40f006b34a87906a c000dea20a42889c 4015f28f30ce037e
mscalar 5 3 1 | 3fc5893c7eedff70 c0db0061547b74e3 c002be5b6c62e153 8000000000000000 be3ee9cfa19bf8a2 40e7840a8d9d24db bfc1117895d50630 3ff40dc5c8c9919d 0000000000000000 40da188dff7617b2 40e8d0569824761d 0000000000000000 3e4242a0c5af79c6 0000000000000000 0000000000000000 3fecc74de2b44e9a bff55e9990ec62a9 c0061a53011bbb1d 3ffb4bf0ef376d35 bfee90464ae2fa04 3ff95bb7c7d717ad | 3fc35e36ca11bd16 c0d848813f666d2a c000db4d66368b5a 8000000000000000 be3bcd1eee3a91e3
mul 2 6 3 | bfc15b5626270de8 4006b9f84f560a57 8000000000000000 8000000000000000 bff5476d444da8e3 bfd7d2edc8fc24d4 0000000000000000 3fec419037533906 c09edbf06f926dc4 3ff58ab1f4a3e2e1 3fff0a9289684375 3fc63605ad0e4a00 400037d837122697 4010000000000000 3fff7d458d04db9e 3fdd6fab6a184d28 3ff740ec7b6bc5b1 8000000000000000 3ff32fd304ec69ce c007d85d0118c6e1 3fed766ec16f9f2a 0000000000000000 3fa04f4874894d40 be34aa30f2a1bdd8 4008000000000000 3fd0b5c04c6009a8 8000000000000000 3e1479ae11f49178 bffe9c1c0d142ff9 bff2781f79fc0bd7 | c0a672799d3f7754 401d8eaa05cdf92f c05013332ca35f5b 4008dab02c3aed97 40ad7f71c397c1da 4016fc494f931412
tmul 2 6 3 | c00092e3f72e9050 3ff2334befcfb33e c001a53fd7f151b6 0000000000000000 3ff11386f4be3f35 0000000000000000 c0f3167ff867030b 40ee5a8d1473f35a 3e30d3d2d3cd3d4b c0c2c0cb421c690b bff7a56c79a5a04a c004369bbf1cba8c 3e423a5265330072 3fe2e64aae71133e bfe42ff136ad8bf4 be0c258b28ba38b6 c000000000000000 3ffa633ba8ea51f5 3e3904bd4f415fb4 3e0f9ee5d2f0c5d1 0000000000000000 bff340b5add2a8a9 c00154ca1a7613e0 3fe59f670386c4ce bfcd28d9ce3dfd28 40002286d91eb5e1 bdf05a5ff8f56afc 0000000000000000 0000000000000000 0000000000000000 | bfb23c8ccd6db8a0 40e1ed3a404b61b2 c0027f3d9ccf4b0a 40c69188b7f15835 4006212416cea4de 41017a9ee9d757d8
mult 2 6 3 | 0000000000000000 0000000000000000 3fd316bb47f796fc 3ff5c968a41a045f bffb94343172814b c002b5cb31fe2f0e 3fdefbdb141caeac 0000000000000000 c0053cc8569a0efe c010000000000000 3fe3da2d1b134b2c bfff579dbda0bf43 3fe29d0090ae57b6 3febca126e97cab2 c0f75cada19b9f2b 4005fc3c05428e3e 0000000000000000 3ff0000000000000 0000000000000000 3ff21ff5b4226ddf 40055fe500b6a9ee 0000000000000000 c0f85b04976107e5 3ffbf30f31310052 40054f7e250c310e be18c497928ff72f 3e30806c555e8df8 bffdb707473e8756 3ff45eff63314e05 bff558ce5c63e77c | c01d9dc356518fe7 c00a337e2f63c9c0 c0e7952a2d303007 c01492d91e5fb22e c011281248e837c0 c002304ce0f371fe
mscalar 2 6 3 | 3fe3e80de78f6e08 400369bf1bdab4b2 c00776e866ab00dd bff869231db1448d 3e3efb752afacefe 40029d5b8fca35f0 bf747f33350a1800 c006a22c4d6d95a3 8000000000000000 4000000000000000 bfe1b063dd30d91a 3fee3e69d9d286fa bfff853e64dbe827 40044c7a2af38de8 c010000000000000 c00604149e90acbf 0000000000000000 3ffa8ad91f59975c 0000000000000000 | bff39bb0c276475c c0131f467f59c7c4 bff39bb0c276475c c0131f467f59c7c4 bff39bb0c276475c c0131f467f59c7c4
mul 3 1 5 | 40003933d7ddc31a 40f291d32ff6fa02 bff67fb053c25560 400407b5d2609bc0 bfe23ca2b64b0e3e 0000000000000000 8000000000000000 3fea19bb1333bc32 | 40144f5230eedc78 41073f3ace28d6b6 c00c2a73f22606b4 bff27dd5d146ad0a c0e52a6d45f5db68 3fe9a4ea00a40926 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 3ffa770b4fcff533 40ee4ad42689bec6 bff259d68d7def2c
tmul 3 1 5 | 3ff7325b39041b4b 40057e2b22b84cd4 3fe9c194b64f08c4 bff36c87df62e726 c007a680cfd6d81e 8000000000000000 0000000000000000 4010000000000000 | bffc29253834a82b c00a179d610480b2 bfef4495fd3c7093 c01124e42a491880 c01fc50809fb2432 c0030926c4168807 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 4017325b39041b4b 40257e2b22b84cd4 4009c194b64f08c4
mult 3 1 5 | 4000000000000000 0000000000000000 3ff6cf69be518c47 8000000000000000 bff72781b4d9f79b 8000000000000000 4004be7827b54e90 3e3cb0ecfb8cfe40 | 0000000000000000 0000000000000000 0000000000000000 c0072781b4d9f79b 0000000000000000 c000813d3eb6e942 0000000000000000 0000000000000000 0000000000000000 4014be7827b54e90 0000000000000000 400d92ce81cf173b 3e4cb0ecfb8cfe40 0000000000000000 3e44739a39195cf2
mscalar 3 1 5 | c0051450a353b4fb bfe8d797bb508a48 be0f0f9e4d2a947e be409a69cdab2663 0000000000000000 3ff700105fcb3e0f 3fe22ebb718dfe54 3ff0000000000000 40f1e6b15d0cf4f4 3fe4d7c6d9d7130a 3ff9ed5d4d3fdc53 be2449c3e53e18fb 0000000000000000 3fdcab79b1338e54 bff64a81343ee124 bff670857e7f97d9 3ffbf9c8a531da73 bff828bc49e405de bfef0f4b04a1d478 | 3e55dfbf8fbeaf5b 3e39c75715e3328b 3c601db1034a4334 3e55dfbf8fbeaf5b 3e39c75715e3328b 3c601db1034a4334 3e55dfbf8fbeaf5b 3e39c75715e3328b 3c601db1034a4334 3e55dfbf8fbeaf5b 3e39c75715e3328b 3c601db1034a4334 3e55dfbf8fbeaf5b 3e39c75715e3328b 3c601db1034a4334
solve 1 | 3ff0000000000000 be42fe99d6c1c737 bff09ca928c72cd8 | be42fe99d6c1c737
solve 1 | bfe7adf95010cede 3feae7e745ed0ae4 400019777d2fbdd4 | bff22e11fcdcec76
solve 1 | 4044000000000000 c002dbed4d95c34c c0ef1f0406cc0485 | bfae2caee289387a
solve 2 | 3ff3b379908ee73d 40cd23397cbd32e6 3ff755f1ecc24ea6 3ff0000000000000 8000000000000000 4005bb7253c5419c 3ffc0465441d754c 3ff70ca6a3cf9173 | 3f27de54d44b9f0b bf24269e4ecd4191
solve 2 | 3fe28c6e39183788 3fd6d188c4b19298 40033c59b266c668 3ff0000000000000 0000000000000000 0000000000000000 3fce74415ae85560 3fd910d99cd04ba4 | 0000000000000000 8000000000000000
solve 2 | 4044000000000000 bff818ccc8e85b44 c0ef7e5aac75ce1e 4042800000000000 3fd4f41249756260 3ff0000000000000 be19b81453de85dd 4000000000000000 | bfe59492374b119e bf3bbdec015e8208
solve 3 | bfe55ee4c80433e0 be335fbc3d4c3ead 3e33fb0a17721881 40b6180bb52792b0 bffa9e1cf087d8d0 0000000000000000 3e41a88cba5d7ad1 bff73f4d6469de7c bfd38cc52f94f6b8 c0ee537beaf2a437 3fe413fb7ee36812 4006d26de830a82c 3fd08406eef9d3c0 4001ec4ba992cf62 0000000000000000 | 4103647282151273 401f1b681744fde3 c022ac514942a382
solve 3 | 3ff0000000000000 3fc9ec8a5a82e660 3fca4cefa14b7b08 0000000000000000 c0000fb374e8acfa 400530047b66af46 c000000000000000 3ff0000000000000 c000a8dadf4d5bf4 40b88edb70498ce8 8000000000000000 0000000000000000 400567b35ad133a8 3ff335526da74ae7 bffc8ca00101d455 | c0d9bf42d59b84d2 c0c513bd75ba7503 c0cfe2f9b1ade80c
solve 3 | 4042f49071bb9cb3 3fe4924e10f41ab8 3fea944b6fb6a5a6 3e40613fa2142cb4 4045800000000000 0000000000000000 0000000000000000 c0cd4431779eb183 4043fffffffc8387 c0053d3ec80344e4 3ff30db789e90900 bff958232169bf57 3ff3f5c9cbe74f24 4000c4eb568896c3 400267555b376584 | bfb1ed85e9df8bdb c02a874ab96e935c bfa387e8061c6ed1
solve 4 | 3ff0000000000000 40f44851b5cc587f 8000000000000000 3fdb07ebdb9a93f0 0000000000000000 4000c398f2733a42 400125407cc68826 3ffb0866a8c7bb5c bff5c71b51d9e4d3 3e2dd34d8362e69a 3e1490b7574cc581 3ffe61a5fe2b118a bfff1dd8e29bc660 bfeffdb2e901a9e0 4010000000000000 4000000000000000 3fd9ad2eec93a3e0 0000000000000000 bff695786acf8606 3fddda18ac1ba6cc 4008000000000000 0000000000000000 c00041e517c0a154 4002627521e9f37e | bef0de667fb3d631 3fd7ed80dcfac527 3fdfb8abd5f7515a bfe1b3bfdad5c390
solve 4 | be359861682a4a11 3ffd2a7b08a62056 3ffa53045c58e91d 40d9884c2ee81816 bff18bdcf77aa757 bfe77aea45691d9c 3fe815a3a14d8878 bff0000000000000 c000000000000000 4000000000000000 3ff0000000000000 0000000000000000 3ff3263b9d02a8fd 3fe15aae1371a49e bfe6c617f09fcf30 c006984f101056b6 0000000000000000 4006395910414a25 c0f1f1415a2d7bc9 bfe3d395f9fde4c4 bfe3526c40ceab2e bffc7b6eef235d2f 3e2373b0ea1ba924 0000000000000000 | c06058b22d2d785a c12a44be013497e2 c0f11ad8b05755a8 c12ba47b478dbc6b
solve 4 | 4043fa0e39e32cbd bfead14b418381a0 c0009439092bc46a 3dc1d5190d6ede2f bff4f708cf909d87 4046000000000000 40022102d1d1ad28 3fec35c79ff58ae4 c008000000000000 40012e232b22331a 4042c533954877f9 3fd1794c5653b9f8 40dc4ea8205555e8 8000000000000000 be316ab67ab5705f 40450db2e8d8c0c0 0000000000000000 4004ff7b2d9e3ae0 c003b7eeeba4807c c0e2437e24548761 bff0000000000000 bff5dccb25d03a4b 3fe8d3f16eb47b1a c0f26b0ec1e7b10d | 413404bebbeef15a 40d5031fc26df429 40f15d47d00e9de8 c09c1fdb7eff5d12
solve 6 | c006355ecae3731e 3fddc6603a3fe16c be355ad04c64af27 c0e8f797cfd1e4f3 3fd8542bf9eed66c 4004eb2f6f8e4869 4003c1ac287d1728 bfec185938da8582 40f25428eddd9c46 bff67b001b76ed7c bffafd309fbc3fee 3fe200edc15ac4f4 bfcd1ef41632dba8 40f1761d5f1abed7 bffc5b3ccdd1737b 4010000000000000 be3ae90cbb0bd6b9 40f350eb155654a4 bff6e59208a65c1b 4000000000000000 c0010935a9d61f06 3ffc9a778caa8299 8000000000000000 0000000000000000 bff5adfe55e36c95 0000000000000000 be3fd78d5d766755 3f9af62d77ccea80 3ffb4869c1fb750c 3ff3d47f067e47b1 3ff585b6147e7cc9 4003a3da6dacd872 3fa6e5ab917b3f60 c010000000000000 3e32143959ce87f3 bfee99d386f35988 3e3120ecc8bc2939 3ff7b74afe9a63fe 3ff96b6a13933e9e 3ff65eabb0309290 8000000000000000 bff38d7576079042 40f4a046063131e0 3ff6b7042bc90d92 40065ff1db581a42 bfecb009d92a2942 c007f692162175d8 bfe196391c793dd0 | bf0a2ab76fc15f88 3f01e549593b7d78 bee3c7a47ee3538c 3fddefac487d87fa 3f07892200f9fee0 3fdfd71694470549
solve 6 | 40e44637d12ec545 0000000000000000 be417b07a3a976ce bff435adad2da636 be3c3206c3e33ad8 40f39411e2120a1e 8000000000000000 3ff0000000000000 0000000000000000 bff5e3b7504ffdd8 40ec8f5b78faac86 3e39641998d3c2c6 bfebcfd640f7fa6e 3fc3ef283070eda0 40f7e97a1bbc3a10 3ff10ec13a2e24b3 bfe10e415c858f7c bfdc0b8140404a38 0000000000000000 4006a2c391aba6c4 bff0000000000000 3ff0000000000000 0000000000000000 3fdd1b9eb33c7b3c c00608ed1a09d837 3fc900ed1934a5c0 be2a7c61f5ae1a80 0000000000000000 bfe2bbaabd420dfe 8000000000000000 3ff1fbca4eaca125 0000000000000000 c005bfaa98c52869 bffa82f55f8877da 0000000000000000 3ff0000000000000 3ff3bef2cd0d22e6 c000000000000000 3dea17ece96a929b be371d50bce52234 0000000000000000 8000000000000000 4000000000000000 4004349b0be04dee 3e28628ca716a8f4 c0062166f37640ba 3f925fb4cb165ec0 c0010f265637e0e1 | 3ee2a5611184dce4 bed425f34fc1fef1 bef30bfeebec5f7f bfe58f0407222b45 bfdeb76a3a852c6e bfda05cde1a68807
solve 6 | 40440000000cef1f 3fd4e966765ef54c 4010000000000000 0000000000000000 3fefec7a3b6b8094 c0df55b484aa4ab8 8000000000000000 404576bee89dfe34 8000000000000000 40036039c2a90e7c 0000000000000000 bfdd442627bfdbc4 bfe1d2ad185a077c 400697afb2009c3b 4043891fac1c04b1 3ff84f14b10afc1d 40074007cdb69e1e 3ff0000000000000 0000000000000000 3ff45b3aa1a2486c be44a9528c792947 404558b8110f2800 8000000000000000 3ffd207d14178636 40f2a8c1715a5c26 40017828d19f07b2 c010000000000000 be389fd82abd3250 40433967ba715323 4000000000000000 c000b3a58fda8039 3ff8fc6d8c745038 c0f773cbc526e8ee 3ffa1a4307ee6338 3fe61e67a4c17a32 4042808bcb9c3aac 0000000000000000 0000000000000000 3fb7bcfcd28d6d40 40009ea01f26ed41 c010000000000000 be405b0789636b23 40f2e42d9ed8a22e 4002b093d898b3c6 3e2623c5245c2139 bf6c32b135772000 40f7b247428d6b63 c0043407aa175221 | bf04b9230089d615 3fb67a1ca06461bf bff60454e6d4913e 3fb7b9381f9cd70f bee504d839c3924d bf425f22d83da0b5
solve 8 | 40eab22fc625415d bfe33bc5adb9c996 3ff0000000000000 bfe83cc18e523282 bfd553b10d8ceaac 3e369bd07b77a932 3fe114c7d6e5670a 3ff6629043cfe705 c0037318af06b97c 3ff0000000000000 3fde5024d5d83c4c c002e73da93001da 8000000000000000 bfde3e282b084d34 40013edc0f4caa00 3fae7b1d02e14fa0 bff0157c982cf3b7 be3081723eb62eb9 bfdda6883e075f84 40050267f38ebc2c c000000000000000 0000000000000000 400601faee053c36 c0e7ae0a550799b6 0000000000000000 be207c07451d2b9a c008000000000000 c0045964c447401b c0031a6be045dbfa 8000000000000000 8000000000000000 40073d7bbad9ad9c 3ff8658f106bb9bd 8000000000000000 3ffe3b1408e21e0b 3fe5a49e5bb9113c 4007f81e0e0232ef 40039b896eb7cf64 c0e1330c0ab9e771 3fe207f05bddb5ac 4007a19b30535392 3fe1e52c0c57ca0c bfcc9c390e7e7a28 3e34eb3552432a01 be2c3c647f41dba2 c0034866fe5e1b08 c0c03145fa3a5be0 bfdab01fa863841c c010000000000000 4004578e6eecc74c c00682f7f0cdbb0d bfc5b7d4f93c3988 0000000000000000 40dc1644faf9fd58 bffefcb12cfb5843 c001c22645d6c01d 4005f27012d261d8 3fff39e3c221d63f 3ff05774d6d978d9 8000000000000000 0000000000000000 be27a612f92447d3 c008000000000000 c010000000000000 4010000000000000 4010000000000000 3ffdbeb78d0824bb c000000000000000 bfeadacd1e01f822 c006c0816873aad2 3ffe1c83480355f5 4006cbe7833c82d2 3ffd4e85b6bcc5de bf8792d804d80d00 c000000000000000 be1896d7bd07a4c5 0000000000000000 4010000000000000 3f97559eb9e6dd40 bfacac3993844de0 | 3f23e12e602ca48b bffeeedf19bbcb42 bf2f5d3ddb6d0fd8 40097439ec2845a1 40020b6f7d5e92c1 c0232c2412dd0f1a bf5273d126bd3dd1 40172779506bb5b6
solve 8 | 40e719ce8e717016 bff91ec0dd25b98d 400174395823b2e2 8000000000000000 c000227a7644b2d0 bfe5fe4666ff6366 4004a49c0690aa52 8000000000000000 bfe3439b795d5bf0 c001c1a56a4ef588 c004d50379161a98 0000000000000000 0000000000000000 c003143d5e37f4e0 bfdd30256cfbf64c bff480ba8c79b15c 0000000000000000 400004b541e2a0a0 bfea13ea4756932c 3ffe035f7abfbaf0 3ff5c5fcdf71e884 3ff777d80ee06c70 8000000000000000 3e2e0c5dc5792419 40067102477b5050 c007723bffc4380b c0065fa6a9a4b284 3ff0000000000000 c010000000000000 0000000000000000 3e197f99df996ffe bfdf1320dedd7ac4 3ff1be8b36243bd8 0000000000000000 c0066e9e824bd8e4 bfea6c05e513fa98 3fe8f49cfe7acee6 3feec401ab31079e 40e2eeffeaa3e504 4006d1dcd786e9fa 3fffba2e3cf129b2 3e270456a3f002dd 3ffc66bde4bf175e 3ffa0d19c38fe4c6 3feaaf9861bdd9b6 4006b4c3da093f48 3e275e50c5f07b48 3ff1c87610542c88 3fd44dccb7f39d28 c000dfd8670a023f 3fe8c5d8c2ee3fb8 bfebac8ec41a97ba bffb1b55db90108e c0f48ea5c9c12f3b 3ff0000000000000 be1690e930f1709b bfe1c177d1fba5d2 3fb34dfb3050ccb0 400473fe55cbc529 3ff1d87800cba742 c005cc3e22ece5c2 3fd4e1c45a10cb24 bfeb4b56a8988234 40eabef08f43a9a3 8000000000000000 4007364db61ea002 bfe8561b76f443e2 c00529186ae1a5cc 3fffe130e6ea48d2 be2d9c493e512333 c005203b37fe71a6 bff4d821fdf2f682 bfddf5ce33eb6d64 3fcf891e49802ee0 bfed3d2c4f2e774e 0000000000000000 3ffac5ea8119dbc7 be131f8d69ae2f9e 40007677a16aea6e 0000000000000000 | 3f1bbd825d9936bc bfb616fbe83d5fb9 3fd3ae0bd581de37 bfe6de166970246d bf121ec754291aa1 bff8a3ae0e02a5a5 bf07204e87375b38 beadbc19796fd6a5
solve 8 | 4042800000000000 4003c4f41b93d5ca 3fe7aa2fd465a03a c000000000000000 8000000000000000 0000000000000000 8000000000000000 bff403ba2d2514f7 3f8d673d4778db00 4043fffffff23c53 0000000000000000 c000fbef635ed51b 3e43a39f6b80c7cc bfe8d86246dd1e04 be4216537c651a77 8000000000000000 bfd7327b83375090 bff5861746ddd634 4044000000000000 be38798d2d4c6886 bfee04e5f73a4fb2 c0ed4e6729774f2d 4006b53f07f5209e 0000000000000000 bffa5abf0332cdf5 bffce032b5900b82 c004409b245d9a46 4042d79d777efb0c 3febd00d9c07a66e be17d0402c7eda31 3ffae0f3628d09cc 3e432a32fd801c44 3fb816898afc6c30 40060690685cf5d6 3fe40dee5c24442e 3e3b665c329df55c 4044c7756b2a2b53 c004b01548ec3180 3faed55ede4b4e20 3fed6b1254bdbd80 c0068d54b3daefd6 bfc652785b6551a0 c0dfc12e6dfdeef4 3ffec89dcb4472a7 be40e6ceba9a5b36 4043cdd5411541ec 3fe8d71030f32478 bfe75318caad1b74 3f7638c03f74a600 c0e3a64ed6967aa8 bffc2bbafd373b39 3fe4594813bd6538 0000000000000000 3ff2fe38b4075b60 4042fc21d327977f 3fab77eb2f488020 40ea2c6d69b7c6a1 3fe0c1fe374b5a8e bffe102775a999e2 3fd8a225894987b0 be3c20a50c19cb2b bfd94e2c0e23f644 bfd060dbf0195548 c0edbfcd3f998c72 3ff0000000000000 3ff45213c18ddc48 bfe5a1387e07987a 3fe49cd5fc915f5e 4005a93a3d718f2f 3ff0000000000000 c0eef2c57c698d26 c010000000000000 3fdc0f8e8e778fc8 bfea3fbcfb0d5b5e 3e147e17d15812c6 0000000000000000 3e3ee0cc7353c2ab 8000000000000000 bff0000000000000 c00124df35da30fa | c09097c1eea490aa c11d3af7237f2a77 4018a6c1faf45f1a c0da6524c6be9534 4081abd39777cb99 4000fea182bf64fc c07dad6c570d29a1 3f9ead4393746e8b
ref 1 2 | 3e44c746056168af 40e7fee3d235b71e | 3e44c746056168af 40e7fee3d235b71e
ref 1 2 | 3fffe81c0cd78299 4007eeb2cb5eb968 | 3fffe81c0cd78299 4007eeb2cb5eb968
ref 2 3 | 400070a8411406dc bff6c5f2ebc7a20e 40025c640aced455 40a63ab2b9fcf121 c001b3d44efa9782 3fff4fe75f9aa471 | 400070a8411406dc 0000000000000000 40025c640aced455 40a63de09d917138 c001b3d44efa9782 3fdb2926da040bc4
ref 2 3 | 3fffb566822e2147 bff0000000000000 400280db75b139a8 bff17c159780f65d bffc5ded3596a6e6 3ff1752c4fe30b96 | 3fffb566822e2147 0000000000000000 400280db75b139a8 3fb304e165ac3ee0 bffc5ded3596a6e6 3fc926ba40bbbd30
ref 3 4 | be1eedf62b4362f7 c006bc97b3d49f4f c002e349690f5d06 40f7841b53fb561f c001d9d23abfc423 c008000000000000 0000000000000000 bf928edcb5617dc0 bfdf131ee5127284 bfe86247832f31a2 bff78dcaba0749cd bfb9c2b6812e34e0 | c006bc97b3d49f4f 0000000000000000 0000000000000000 c001d9d23abfc423 40f7841b53fb5680 0000000000000000 bf928edcb5617dc0 3da93ec4ce22cb70 bfde1c74a98b37f0 bff78dcaba0749cd bfe8624782af0776 3ff1f4ddc303e1ed
ref 3 4 | bfed1d8bb36bedd8 0000000000000000 3fff1becb05c0d2b bfd25cb76a85bc24 3defa34c77aa1b72 3fecc7391575ef56 0000000000000000 bffe1d6b0c4b36f8 bff0000000000000 3fd7ef23545691cc bff994eea5a0dff4 40003479489df707 | 3fff1becb05c0d2b 0000000000000000 0000000000000000 3fecc7391575ef56 3fc124bc75333258 0000000000000000 bff0000000000000 bfddf3012eaa7c41 bffe1d6b0c13f1b6 40003479489df707 3ff52665d129ca2b bff994eea63d007a
ref 4 5 | 3fe936ba814058de 3fe7e63441133f28 bfea82696064303e bfe3900cf70a5fac bffba5cbe8e302a7 40f491d4a1635204 4000000000000000 c006f4d0d55caf5d 3fd66c023019590c 3fdf7ce99e15f3d8 3ff0000000000000 c0e607381a8479bc 0000000000000000 3ff9a6b475763582 bff41fa06c5eeede bfe9921926ddfa60 40045fdc556bb779 3fd6a8da7afdb9dc 3fe463e7789425da bfd04f416125c57c | bfea82696064303e 0000000000000000 0000000000000000 0000000000000000 4000000000000000 40f491f17ac859e8 0000000000000000 0000000000000000 3ff0000000000000 3ff64beceb77718a c0e6074fb73cb01e 0000000000000000 bff41fa06c5eeede 3fde094857eeb280 3fc085a75874dd45 bff323d4ec65594a 3fe463e7789425da 3fedb65da103d326 bfe7334715ca624d 40093902815dfc84
ref 4 5 | 40045526c82c2f55 40dd1dbe74f63d72 40e8321529333b23 400103abd9569808 4004ce994add4d95 3fd7785546b179a0 0000000000000000 3ff6ae3ad39a5bfd 3e378e7bf52046e5 c00745a633184398 c00557c19076c9ba 3ff07f034c1c73a3 bfe8622870bd1964 8000000000000000 4003eab92e28e4a2 c007d9e052aea51c 4008000000000000 3fd21c3220a34434 3ff5fdbfc04f17db 3ff7a45bba7e8ff5 | 40e8321529333b23 0000000000000000 0000000000000000 0000000000000000 0000000000000000 4004ce994add4d95 0000000000000000 0000000000000000 c00557c19076c9ba 3f21ef9c6bd704f6 bff4dc7bac6240fa 0000000000000000 4003eab92e28e4a2 bfe863343b87826f bff63f7c63cbb45c c00d5360a65ca2d8 3ff5fdbfc04f17db 4007ffdb0a2a0886 bfeef1a3fd1875f2 bfed823cb02b80b1
ref 5 2 | c002602df16d4423 be44a21c13fbe39e 4000661a1617f8d9 bfeea80ae2d79fc8 3e22207f28bf5e7a 3ff11cb35d60dccc bff49cc2431ed98a bff6d11521f332a9 0000000000000000 c010000000000000 | c002602df16d4423 0000000000000000 4000661a1617f8d9 bfeea80ae2d79fc8 3e22207f28bf5e7a 3ff11cb35d60dccc bff49cc2445247c7 bff6d11521f332a9 0000000000000000 c010000000000000
ref 5 2 | 3ffac185847b37d6 c0040f47f482413d bff7d4e512cf09b2 40e83091619e8827 8000000000000000 3feec39eb8b21e20 40f738ffc34790b6 3fe0523d1353fa44 bfcb50532388a880 3ff0000000000000 | c0040f47f482413d 0000000000000000 bff7d4e512cf09b2 40e83091619e8827 8000000000000000 40f738ffc34790b6 40eef9a822e71528 3fe0523d1353fa44 bfcb50532388a880 3ff0000000000000
ref 4 3 | 3ff0000000000000 3f8b0150b22d5600 3fe860c6c7821fe4 0000000000000000 c00466972c2d6584 3fb93ef387295260 0000000000000000 3fde064df5c16094 3fee6a9ac4a91598 0000000000000000 40050800b24dc368 3fd6f179a6e2f6dc | 3ff0000000000000 0000000000000000 0000000000000000 0000000000000000 c00466972c2d6584 3fff1547c13dcdc4 0000000000000000 3fde064df5c16094 3fee6a9ac4a91598 3ffe7a08fa14ffbe bfc2337b9b87088e 3fd6f179a6e2f6dc
ref 4 3 | 3ffae1eceebf1466 0000000000000000 bfe59dd80f521f4e c002a3a9f3442a23 bfe9bdd7f86b88ca bff40d5ce49c11b6 bff7537c57ade441 c000000000000000 bfc614d03eabf2e8 3ff6a64c4915457a 3ff0000000000000 c0013c8aa55bdbae | 3ffae1eceebf1466 0000000000000000 0000000000000000 c002a3a9f3442a23 bfe9bdd7f86b88ca bffc803ae273e3c9 0000000000000000 c000000000000000 bfc614d03eabf2e8 3fedc7d00599e8fd 3fe858c205d24e6c c0013c8aa55bdbae
givens | 0000000000000000 0000000000000000 | 3ff0000000000000 0000000000000000
givens | 3ff7ef0b3ad5f716 0000000000000000 | 3ff0000000000000 8000000000000000
givens | 0000000000000000 4004ab2d143a8e99 | 0000000000000000 bff0000000000000
givens | be436a1b6699c9a9 bff7f9e6d384b815 | be39e964ba75944b 3ff0000000000000
givens | bff058fc83af253f 8000000000000000 | bff0000000000000 0000000000000000
givens | bff0000000000000 c003bcb7e27c467a | bfd80a859a5edbf7 3feda80e95cdf000
givens | c010000000000000 be307c89dbf40ae2 | bff0000000000000 3e107c89dbf40ae2
givens | 3e21684b216796fe c0f2da7dc9f8562a | 3d1d8ba9d5f8c680 3ff0000000000000
givens | 4007af076a88b505 bff6d8de4b5b8fda | 3fecd2880541b36e 3fdbcde76dd90bac
givens | 0000000000000000 3fe6d2499ab4ef3a | 0000000000000000 bff0000000000000
givens | c00259b447286bf6 be10354d726208ff | bff0000000000000 3dfc43ae2ee7007a
givens | c0059617c2d967b7 c0f7abd71e4caed4 | befd2e73f95ef401 3fefffffffcac714
givens | 400511c3701a24fa 0000000000000000 | 3ff0000000000000 8000000000000000
givens | 3ffa3cf274b84127 3e40a4e3605ab110 | 3fefffffffffffff be344c961f90d9aa
givens | 400321e4b995f054 40f77cbaf6b30e91 | 3efa111947382009 bfefffffffd5885b
givens | bff155f741efd387 0000000000000000 | bff0000000000000 8000000000000000
givens | c0e3d4e3d0e90879 0000000000000000 | bff0000000000000 8000000000000000
givens | 0000000000000000 0000000000000000 | 3ff0000000000000 0000000000000000
givens | c0046adeddca79a1 40d1c77820fbbfea | bf225fd0350dd013 bfeffffffab98b71
givens | 3fe1d253cea7720a bff6a26ce93ff006 | 3fd771bf30a91c14 3fedc692bf8d68a9
givens | bff12fae53c2f532 be2ec6089c74701a | bff0000000000000 3e2ca643d1fe894e
givens | bfebafb3c75a8216 bfeab931a4dc385c | bfe706361b062be6 3fe63936591ce7b2
givens | 8000000000000000 0000000000000000 | 3ff0000000000000 0000000000000000
givens | 40067721daa30d5e 40f2dfdd7ec7de3f | 3f030b4498b2b680 bfefffffffa554d4
pgivens 2 3 2 0 | be3899524902a930 3ffa667312a1053e 4005aff3ba6230a2 c0065790b21bff7c 3fd7150a67f48de8 40067e96ee65cbf0 bfff6bd1dda1bf85 3dfee3443964d66f | 3e47ce4b14b9cc6e c009ec332ef64235 4005aff3ba6230a2 c0065790b21bff7c bfe6aa27d0ca7608 c016166d0628bf1d
pgivens 1 2 0 1 | bff0000000000000 c00468e247a5b498 40d9cc5256b16016 c0e0cd5d8a5856d7 | 40edf789ae97ced9 c0f8daf0b3bd3a08
pgivens1 1 2 0 | 0000000000000000 c001fab74868c7d8 40d9cc5256b16016 c0e0cd5d8a5856d7 | 40f2e17cb1778c8f c0ecfd5788552df8
pgivens 3 3 1 0 | bfae59937598a3a0 c006089e898a29c3 3e1e36d11c0cdce8 be3ac908c3d6a6ce bff3c96bbc401e8d 8000000000000000 0000000000000000 3e27a98643259f82 4000000000000000 3fe3f2e28f040a10 3ff0000000000000 | bfa2eb8ba3af702e bfdebd2b8e98b1c8 3e12d5e0a039615b bfae599396fdf9f4 c00c33823048c276 3e1e36d11c0cdce8 0000000000000000 3e27a98643259f82 4000000000000000
pgivens1 3 3 1 | bff0000000000000 bff868494d823f72 40040ea9535c3972 bfcb2f0cfc9c2608 8000000000000000 400608d519cd268c bff9ad90c2389c2f bfdca12ab2cb7590 3fea9561cf71c35c 3fe3f2e28f040a10 3ff0000000000000 | bff0000000000000 bff868494d823f72 40040ea9535c3972 bffbcbd941f82bae bfdca12ab2cb7590 400461d5c79ca5ba bfe938256e1838e2 bfd1d8fedf425329 c001e4371aa1a42a
pgivens 3 4 0 2 | 4007633220cb7298 bfe5140970609282 4004a45c269cc488 c0f1edc0f37f9aa1 bfdf35cf640fa400 c0e2ee4d5e13cbf6 0000000000000000 3ff9975fd3d2b071 c05727dcda1a0b7e 3fffdcf296c06f86 c0c3e78ab9f3808a c0d4398fe9f1c122 40055145461b29d5 3ff7ba9ed8715b1a | 401f28fd3b12e9a3 3fe3bd74eae94e2c c0604faccec477f1 c0f1edc0f37f9aa1 bfdf35cf640fa400 c0e2ee4d5e13cbf6 c01157b0b2e8c60f 4014f4aa6c52b4a2 c06f54640e04aacb 3fffdcf296c06f86 c0c3e78ab9f3808a c0d4398fe9f1c122
pgivens1 3 4 0 | 0000000000000000 3fe482b9ce379f52 3fe5c14146f795e6 bfda1955f5bb0380 4005b5d2e4ca64a0 8000000000000000 bfe50a7562b5ce12 4001c1acc0188e14 4006eab43cb7b962 bff1e40c25ccda1c 4008000000000000 3ff2262404fef70c 40055145461b29d5 3ff7ba9ed8715b1a | bfe35a6a99e29ed3 4016ee3cd1181294 3ffcfc26385828c6 bff162e90aa7a910 40191f867a13f00a bff021c633c64a38 bfe50a7562b5ce12 4001c1acc0188e14 4006eab43cb7b962 bff1e40c25ccda1c 4008000000000000 3ff2262404fef70c
pgivens 2 2 0 1 | 0000000000000000 400031bc40a5104e be1b2272afd99bfe 0000000000000000 c005d89cdace55c1 bfedcdcedc12ba2a | 3e1945bc0672c12c c0161c8544a34c20 3e328648cb028848 3ffe2a73d03e0195
pgivens1 2 2 0 | 0000000000000000 bfffce415703169c 0000000000000000 0000000000000000 c005d89cdace55c1 bfedcdcedc12ba2a | 8000000000000000 4015b6a7011e15ff 0000000000000000 bffd9f7a23f176ae
pgivens 3 3 0 2 | 4008000000000000 c002847efbb70432 bff6a0e9a3b50813 8000000000000000 4000000000000000 c000000000000000 c005925640331bd3 c0d0415c8dce7cad 3e28b7aa92d165f9 400153d7f81c3040 bfff9a6a64aca60c | 4027a5cfbd0612fe 40e00d21d3ef773a c008818c6bde6859 8000000000000000 4000000000000000 c000000000000000 3fb5d276bd3b4880 c0e19b331eda33fc c0065913f65bf067
pgivens1 3 3 0 | 40dd915a10a9dbc0 0000000000000000 3ff99b96b427e7b1 3fb35e7d1dc79aa0 bfd79cf9e0ed197c 4010000000000000 c006317b7d3ce9f2 0000000000000000 3ff72fe6b5d51be6 400153d7f81c3040 bfff9a6a64aca60c | 40f002ae042b106d 3fe7520406b5d1f1 c011bca9f22b4254 40ed33824e8df85a bfe992868ac33283 4027a66af20b7f98 c006317b7d3ce9f2 0000000000000000 3ff72fe6b5d51be6
pgivens 4 2 1 0 | c008000000000000 0000000000000000 4001f8c3e2a1307e 3ff514f1703745b5 c0e9493ea1d772e4 c0c2c37c4f85c802 bfe2504dc764a182 3fede7932bf3bc9a 3fdb218e9058520c 8000000000000000 | bff4592aec423d89 0000000000000000 3fee797bc46000f5 3fe1dfc73bb22be4 c0d57051083a0165 c0afd13c26cbf963 bfcf0dec0ecf2d4d 3fd95ac0375cad4d
pgivens 3 2 1 0 | 3fff27a9160d6db8 c000000000000000 4002bb062c4845fc 3fe590f72fac1136 8000000000000000 0000000000000000 4000000000000000 bff53b80d90da24b | 401327a3a05229cb c010000000000000 4012bb062c4845fc bff3c6f3e0b40234 40053b80d90da24b c008db20972c024a
pgivens 1 2 0 1 | c0f1723b19f9dc95 40e8aaf8e07a5d19 c00062aa0f367878 3ffa059559f73186 | 410be56c399f1a4c 40c8e5028d5cc710
pgivens1 1 2 0 | 0000000000000000 c003d9f0ec478193 c00062aa0f367878 3ffa059559f73186 | c010248a74384fa6 4014545ace6e6ce6
pgivens 3 3 0 1 | be40f7dc25d0e61c bff5d095e17a8db5 3ff240a0c868851a c0f15d2f1176ffbf 0000000000000000 4005160bb5d874b1 3fe2c7ad031c8096 bffd105a1ce87bc7 400508662b32416b 4010000000000000 3fddc68288a975e8 | c0e0282a0fc72c24 c015d095e17a8db5 401728530aa01608 c1115d2f1176ff80 3fe44c5b70451db1 4024064ef509009b 3fe2c7ad031c8096 bffd105a1ce87bc7 400508662b32416b
pgivens1 3 3 0 | bfff5aa39cdc79ad c0b9f67f131e1a88 bfe413b00723252a bfe10b233c27b32e 3ffc4cbebe412a18 3fe4ab20c1e03c2c bfc83fc0863dd860 3feaf9e4441a7c14 c0f008d6f9bc9e76 4010000000000000 3fddc68288a975e8 | c0202c306162c435 c0d9f64a68e7c7fa c001ac466be79092 bff37ff3f7fe3db8 40a83698a33e0a19 400700ed298bdff2 bfc83fc0863dd860 3feaf9e4441a7c14 c0f008d6f9bc9e76
svd 2 2 rand | 4004fe62bf9d39d5 8000000000000000 c000000000000000 3fe6c60fd0dbd360 | 3fefb6be5e0c61a3 bfc11465a446e28d bfc11465a446e28d bfefb6be5e0c61a3 400aa086b9cdd9fe bfe1f49f7774d8c0 3fe901195c0c86aa bfe3f8385b70d5d9 3fe3f8385b70d5d9 3fe901195c0c86aa 1
svd 2 2 sparse | 0000000000000000 0000000000000000 0000000000000000 0000000000000000 | 3ff0000000000000 8000000000000000 8000000000000000 3ff0000000000000 0000000000000000 0000000000000000 3ff0000000000000 0000000000000000 0000000000000000 3ff0000000000000 1
svd 2 2 rank1 | 4000000000000000 4010000000000000 4008000000000000 4018000000000000 | 3fec9f25c5bfedda bfdc9f25c5bfedda bfdc9f25c5bfeddc bfec9f25c5bfedda 0000000000000000 40201fe03f61bad2 bfeaa027f059dce2 3fe1c01aa03be895 bfe1c01aa03be895 bfeaa027f059dce2 1
svd 2 2 zerocol | bff707fa55136fe4 400058427a56687e 0000000000000000 0000000000000000 | bfe26e32c28e1cfe 3fea28d346470a4c 3fea28d346470a4c 3fe26e32c28e1d03 4003fe76a2c96863 0000000000000000 3ff0000000000000 0000000000000000 0000000000000000 3ff0000000000000 1
svd 3 3 rand | 3fe0513e5234b836 0000000000000000 c005b49f1e3bafab 3e414e1ba1759def bfd3a16a2859bad4 bff446a1bb9cd770 3fe586d2040751b0 0000000000000000 3ffe9890635c0659 | 3f80ac10a94cd2bb bf9fb79ed5812ad8 bfeffbcc3fc30f21 bfefffa29443c4d5 bf749745ab9e19c2 bf805c6333e52da1 bf73911b5bb02937 3feffbf742f69f0f bf9fc1fc97b7de73 400c704d99201846 3feaff539236c4af 3fd25440ffc054e3 3fe872d300b77345 3fd6f96f524bf1a7 bfe127d44aafe1f7 bfe2849b0fbdf657 3f8c5172b3556fef bfea1805929added 3fd2427566100f1b bfeddd24a7dc9564 bfcbf130e88a45e2 1
svd 3 3 sparse | c000000000000000 c000000000000000 4000000000000000 0000000000000000 bff0000000000000 0000000000000000 4000000000000000 bff0000000000000 0000000000000000 | bfe5570767ada8c1 bfe079ed8bd1a923 3fe13cda1ed0cf6c 3fd39980205c42b6 3fde0ed220944415 3fea7f3fc9f5aabc bfe5bd19e6fff671 3fe6f2fab0a8ce07 bfc3e81654441a9b 400c980e8def5516 3fe00f1e5bf79779 c001d70bc307f41f 3feece4f7c3b207e 3fc2705ad3f047aa bfcd532ea4096831 3fca98495ca85dc4 bfedf285f02539b0 3fd23725fc90f957 3fc6322507ca7ebf 3fd4950867dec6f6 3fedc9a1ce6fede7 1
svd 3 3 rank1 | 4000000000000000 4010000000000000 4018000000000000 4008000000000000 4018000000000000 4022000000000000 4010000000000000 4020000000000000 4028000000000000 | 3feecbb531b10a51 bfbb063c74121f1b bfd006c40dc856ff 3fd11acee560242b 3fe11acee560242a 3fe9a83658103640 3fa9324e6e7993f7 bfead548a3dbd481 3fe15d2420455a97 3ced0b4f180f165d 40342641cf569573 3cc3a417478525b9 bfedb614bfce6a6a 3fcc85d67ab1add1 3fd303e451cbc936 3fd7c4dd663ebb85 3fe1d3a60caf0ca5 3fe7c4dd663ebb87 0000000000000000 bfe9999999999998 3fe3333333333334 1
svd 3 3 zerocol | bff930d78aee548c bfdc0a1e49137a18 40f19008bdf969fd 0000000000000000 0000000000000000 0000000000000000 0000000000000000 40ddd3bb51a5fe50 c010000000000000 | bef6f30e928e125a bf02febc29b1d851 3fefffffff84e12f 3e2f6ec346e861b4 bfefffffffa5cc07 bf02febc29110b44 3fefffffffdf152b 3e289f0f12ad2000 3ef6f30e94a30000 40f19008bea8ac54 c0ddd3bb5034d977 bc59df19604b93ac 3feffffffea5b5ae 3bd29bdf5bfbc350 bf129bdf5bfbc351 3f129bdf5bfbc350 bcaffffffea5b5ae 3feffffffea5b5b0 0000000000000000 3ff0000000000001 bcb0000000000000 1
svd 4 4 rand | c0034fb75417cd32 40066bed98880726 400285294509e212 0000000000000000 4008000000000000 40ed51953a70c90f 3fd6a4cfe81f80a4 c001a9ef2ec45f1a 40045fe941413300 0000000000000000 bfff2d891ebc5113 3fdf1b81cb8e9da0 c0021611c9aed312 3fe5f2319cb8c64a bfe262358c92eaca 3ffcd31e7ce9def1 | 3f0a3195c43d98a7 3feffffffef52a30 3ed8b8c512d6af2f bf0347702cc89f68 3fead79cffd5f572 bf058db9c6bdb9ba bfe14bf22fec989f bfb0a07c7732de7a bfdafec3a336b046 beb92f70707569cf bfe23c1c393b1f26 bfe6915d6a467246 3fd60731d99a28c6 bf0889fcb2525d27 3fe3ce9795c82dd5 bfe69768eeeb581e 40ed51953bf6a6e9 40136f50f09dcb5e 3fdb820d71363899 40048e233299f3eb 3f0878a128c8fa29 3fefffffff615472 3e1c01863aa87a24 3ee7f25e621278ca bfe593f47585db31 3f02995077382435 3fe4cc95f3a497a7 bfd66e531071ae86 bfe668db43350472 3f00fe15be920340 bfe6d478336d42b2 3f98d9455d647530 3fcdfcbb6a9e98a3 be9058deface05c1 bfd0c170f2b971cf bfedf5cbd517cb31 1
svd 4 4 sparse | bff0000000000000 0000000000000000 bff0000000000000 c000000000000000 4000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 4000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 3ff0000000000000 0000000000000000 | bfe60812dad4a09e 3c981443d5a90ed4 bfd73fdc82580dbe bfe416346b11c242 bfe71e444ae1fcfd 3cb41805226ff36a 3fdada59dcf76c3e 3fe19616d445feaa 3fb05084be292c41 3ca46566987d598c 3fea9e94f09f1a0f bfe1a46415ade323 bcad64d51e0db1c6 bff0000000000000 3caf7a910602bd28 bc60addf3fa85b10 4005b0b8cf4f9856 3ffb3c9884390f28 bfebbbb0cb90cd91 3ffffffffffffffd 3feb3b0110bf14aa bfe0406f808d8d80 3c8b76fdabcf7cc6 bfc1266d7e4ff9c1 bfddefe0fefff883 bfeb2948ce458991 3cb3339481565d90 3fcf8c92de3a13a7 bfce945d27151e13 bfc2d316be3dbb51 bcb95e1d31097e80 bfeeb706bcb3481e 0000000000000000 bcb90b410d07f020 bfeffffffffffffe 3cc90b410d07f01e 1
svd 4 4 rank1 | 4000000000000000 4010000000000000 4018000000000000 4020000000000000 4008000000000000 4018000000000000 4022000000000000 4028000000000000 4010000000000000 4020000000000000 4028000000000000 4030000000000000 4014000000000000 4024000000000000 402e000000000000 4034000000000000 | 3fd75e9746a0b099 bfec645d7e058a81 3fc5a5cf0bdec0fb 3fccdd140fd3abf9 bfc75e9746a0b0a0 bfd75e9746a0b099 bfe186f174f88472 bfe75e9746a0b099 bfe186f174f88472 bfc5a5cf0bdec0fb 3fe7e1d25b8c77a2 bfd5a5cf0bdec0fb bfe75e9746a0b099 bfccdd140fd3abf9 bfd5a5cf0bdec0fb 3fe19175f8162a04 0000000000000000 40441fe68f0af33c 0000000000000000 0000000000000000 3feecac173f0552f bfbd8f7208e6b833 bfc3b4f6b099d01f bfc8a2345cc04427 bfd16b28f55d72d4 bfda20bd700c2c42 bfe16b28f55d72d4 bfe5c5f332b4cf8a 0000000000000000 bfe21a1851ff630b 3fe8cf7143b4e4cb bfd1f964d6bbc406 0000000000000000 bfe6a09e667f3bce bfd1f964d6bbc406 3fe4c420f9caa57c 1
svd 4 4 zerocol | 4010000000000000 3e36c6a40be87906 bff7be478e1f439d c001a3d0eb6b7f84 8000000000000000 bff6c70c87a54c7e 3ff3de19ce917591 c005df94a70dfbc8 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 c010000000000000 40f5d43b1967e919 3ff74d387808a393 | 3e0983311bf22fbd 3f0774830189c0d7 bfefffffff643b96 bef114515158f080 bfe8425d38ad07d7 3fbdd35ac115daef bed5fa02eb1f5160 3fe4885c44fc9120 bfe35159c329cae3 bfdfd7f8e0f946e8 bee96788bab232ff bfe3eed053e9b031 3fcf93507f9ab650 bfeb819c6863187b bf07fba84567f800 3fdca4590061470e 40f5d43b19e81f44 4013605481aba81d 4005034d8d898e08 bc95fa0fe3b6c76c 3ef1674c5d4cdf13 beed200601b5bbc2 3bc7d27a48af3bbf bfefffffffdfd0cb bfed60dd03a61f6d bfd95ec2b39d8b8f 3c995ec2b3bbb74f bee468f49ddb1f1f bfd95ec2b35ee04e 3fed60dd039ec67f bcad60dd030c2a16 bef444b5d49736fa 0000000000000000 3cafffffffbb997e bfeffffffffffffe 3bc08a78c8634000 1
svd 5 5 rand | 3e290f4c4089c632 40f2974226160fb2 0000000000000000 bff23373c202b75e bffba0456f4d59e2 be2b041b0dc8774d c0dc4114b088abdd 8000000000000000 be44c251078a1727 bffd95045e16540b 3fe14737469e59f2 0000000000000000 bfe27045bd51e882 bfefe09025871766 bfc1abeee78daa28 c0d149b152351a39 bffc10c1441299a3 3fa72f1a50865060 3ff51d7a57e0592e c008000000000000 8000000000000000 bfe20eaa6b539190 3ffaebea47c330c4 0000000000000000 c00351b4d67ffce1 | 3ed499bc7750890a 3fefffffffe923ec bde56442c3f6e23b beeb60a87a4503a7 bee8a52ec767f3aa 3feffffff6ca7517 bed498a486e8a25f bec5771a68c3cbc4 bf138ad1e8528009 3f263655bb87d5a9 3f240bec0814b9ec bee8a7fc0ab4565e 3fd7bd847302513e bfb1155200d852ec bfeda3e1ff2566ad bf1b52a4ddc3c76d bedd0a8b755548c6 3fe1d3fee60582b6 bfe90616691d2d3b 3fd1e2948209483f 3ed5fc0e34dadc92 3ee732e9e6f9ca45 3fe7c670a95b7343 3fe3d49e896f5afd 3fd02f739a2cc766 40f3e334d03a16de 40d149b1573e5422 400cda400b17ee15 3fe32e7ec7914b89 3ff830cc9353ff67 3fede9e12c0e9c7e bfd6bb30c035d5a1 3ded2742f3fb26fc bef7b2c63b73710d bedd0db89ef9cbcb bef62931b34f5e4e 3ee0cb23c550f1d3 3efffc357a976d91 bfefffffff9cf55b be58ea4cdaaff1c2 3fcbae3ca8c5efcc 3fe23682b7009057 bf75ec000199eeeb be8a1779b382f655 3fe9626c8b1b5e22 bfc9aa8ca259bec3 bfe0e36484c0e97e 3fe662cc1c8566a5 3ef660899f28aa22 3fdc08dd384f4cba bfc955c1a500ed55 bfe0ab9905ddbc5e bfe6dd9f860858d3 bef6da77aa389e91 3fdb10ed202d241b 1
svd 5 5 sparse | bff0000000000000 0000000000000000 3ff0000000000000 bff0000000000000 0000000000000000 0000000000000000 0000000000000000 4000000000000000 3ff0000000000000 0000000000000000 c000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 3ff0000000000000 3ff0000000000000 0000000000000000 0000000000000000 4000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 | bfeeafad47562dba 3fa03dd7f2305842 3fd100c7d5720e23 bfb810854ec09989 0000000000000000 bfad0c2eaed7623a 3fee247047c2b206 bfce0aafc808ae68 3fcddce36f8f87e1 0000000000000000 bfc354b0217c9f67 bfd2376ccf028852 bfc75a95f6ceab7f 3fedb9b1400562fc 0000000000000000 3fcdda9ff88f35c0 3fc66e8285ed9588 3fed58c26cf9b953 3fd16444f5f19586 0000000000000000 0000000000000000 0000000000000000 8000000000000000 0000000000000000 3ff0000000000000 40087eb7e1af0e91 3febba6cd0c2c332 3ff47f0d1acf39ab 4003f9110db7d3ca 0000000000000000 3fdb8fe547d418ac 3fc2486bef56cc39 3fe40b495be10b8d 3fb8dd659849031d bfe40b495be10b8e bfde605a93193f59 bfd1702de4e3e8d6 3fc0c2e559245826 3fea1e5908cca2ab bfc0c2e55924582a bfe7fd568d0e0f48 3fdc2da295273195 3fce2e2d9a543a14 bfd755eeddfcb637 bfce2e2d9a543a10 3fc520dfd07b5dc1 3feafe04d2d3c70b bfc7ea64056b4698 3fdc006ab6f49436 3fc7ea64056b4698 0000000000000000 bc99f5b22bcede16 3fe6a09e667f3bcd bc963ca61d42d598 3fe6a09e667f3bcc 1
svd 5 5 rank1 | 4000000000000000 4010000000000000 4018000000000000 4020000000000000 4024000000000000 4008000000000000 4018000000000000 4022000000000000 4028000000000000 402e000000000000 4010000000000000 4020000000000000 4028000000000000 4030000000000000 4034000000000000 4014000000000000 4024000000000000 402e000000000000 4034000000000000 4039000000000000 4018000000000000 4028000000000000 4032000000000000 4038000000000000 403e000000000000 | bfef9a9650f62780 3f931cb7ba242ce8 3f90e0235d6135f0 3fa31cb7ba242cd8 3fc33d47ab3afd25 bfc1426fac0654e0 bfd1426fac0654da bfd9e3a782097f49 bfe1426fac0654db bfe5930b9707ea11 bfab08402b7d36c6 3fd6c850366ce12b bfd16b62ab60c900 3fe6c850366ce12b bfe137fe9c117a95 3fa35a7696c12261 3fe6968c8240c542 bfe1828851f3f257 bfd5269f69f4fac8 3fd360d624bcdff8 3fa80e5f3aed8e85 bfe19571bcdd0a7e bfe5c3c3b939f2a9 3fd35ec64ac1416e 3fd8164b3e7c3a13 3ce7b019c2f718b8 405196cc93bf113e bcdb27f9c0fc0191 b9922da4da38d449 b97c2a5b44744c40 3fef47e2f8509918 bfb17584a2688514 bfb7475b833606c2 bfbd193264038874 bfc17584a2688511 bfcafc19d8606167 bfd43d1362484912 bfdafc19d8606169 bfe0dd90273c3ce2 bfe43d136248490f 0000000000000000 bfcfdbca096b0c30 bfb911ab91e71ecc 3fea669be27432ca bfdfdbca096b0c77 0000000000000000 3fc7649578b54742 bfecb31aef59a8ca 3fc5a8fc2d657a8e 3fd7649578b546d6 0000000000000000 bfec9f25c5bfedd7 bce8000000000000 3ce2000000000000 3fdc9f25c5bfede1 1
svd 5 5 zerocol | bff0000000000000 bff48eae8d6b91f0 bff6ca6b9f280a7b c0070906d8c4ad55 bfe1c82539decc28 bff3e6c301466532 0000000000000000 bfed36ff815785d8 3e411cadc444e01d 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 3ff0a150e907f2ad 8000000000000000 0000000000000000 400749eb70cc7741 bfea14cb3b9bcdd6 bfed3d3106136db4 3fd5bacd46434be4 bff31989236c1bcf 4004d0e4bfe4ebed 8000000000000000 | bfc8044e7eaaea47 bfc9792431fa0761 bfb72fab8fb2ef43 bfeea22c75b361fc 3f98717b0114a44d 3fe45cb16827e322 3fc0109a37b4af55 3fe72329c546f514 bfcbbf683abf2c64 3fb90a71cb951402 3fdf143eaddcee1f bfe18f31308293ee bfce804118630522 3f9a2b97fa81faec bfe4617f14f165aa 3fde107a9d7d742c bfcb1f76938c5859 bfde09a2371e615b 3f8d33b4d5a4b4e8 3fe6f0dae3c4ff9e bfd494a820ab8f45 bfe8c24afecc9f8b 3fdc0a1d38b778f6 3fc8324158533b30 3fd0f7326b2530e0 40141df68f42e807 4005fdaf8b41f652 3ff764818a348e92 bfc8c9d9cc8a2533 3c731175677bc0a8 3fe5188ada5063be 3fb016b4da371ed1 3c876caedbe61f98 bfe318a62ecc1d13 bfdd0007c848a581 bfdd47e761df914f bfe0e5ea6c2ef28f 3cba7756c06659e7 bf926811156d3721 bfe6e2e19f141d5f 3fe25df564c1b87c bfd0eae973edf643 3cc18adff794d5bd 3fe8098e12b50f10 bfc86e9cdce66875 bfc4e3d19eac3030 3fe9befbb1759269 3cce83295cd4e222 3fd205ea1538cb32 bfdfca612993de42 0000000000000000 3ccdc75604cd1600 bfeffffffffffffd 3cc1e1b2f4a01d7b bcbfd78f2a7af349 1
svd 2 3 rand | 3fd068b346355770 3ff26b52aaa9dcd4 8000000000000000 be405be1d3b4a6ab 3fff90b9a9e5a639 40c44017b5d56112 | bfeffffff647e6ab 3f28f0bf9984e235 bf28f0bf9984e235 bfeffffff647e6ab 3fd0651c7e9e5720 c0c44017be140670 bfeffffffcb0bec0 bd98e2c4854bff96 3f1d1b8e8b7ca800 3f1d1b8e8b7c6000 bd69d9d498778b4c 3feffffffcb0bebf bd98e24dcc004e25 3ff0000000000001 3d69df1a57ac5698 1
svd 2 3 sparse | 0000000000000000 0000000000000000 bff0000000000000 0000000000000000 0000000000000000 4000000000000000 | 3ff0000000000000 0000000000000000 0000000000000000 3ff0000000000000 bff0000000000000 c000000000000000 3cb0000000000000 3feffffffffffffe 8000000000000000 3caffffffffffffe 3970000000000000 bfeffffffffffffe bfeffffffffffffd bca0000000000000 3cb0000000000000 1
svd 2 3 zerocol | 4010000000000000 3ff03ff6f53b3541 0000000000000000 0000000000000000 bffedc73dc2862e1 c004d6b91b1f3d1a | 3fec26eceec5e3f6 3fde6d80b942df7a bfde6d80b942df7a 3fec26eceec5e3f6 4013da12bb14eeda 3ffb4683c1c4cdcf 3fe9cd96261e0b12 bc875c74c7508a3e bfe2ed1c27c8f372 bfe2ed1c27c8f372 bcaf76689a71d37c bfe9cd96261e0b0b 3c9bcc4e661d2434 3feffffffffffffe 3cacd2eaa707931c 1
svd 2 5 rand | 3fc18a9446e45d68 bff094eb52f144e0 c0035008b7c75aa4 3e18ea20746c1d1f 4007dabb23626fb8 4005a50de04a31b2 be41c7d5f0a2082e c0068066744853d1 bfc2d31d34f02198 c0051b0a0d60ff48 | bfeb4c56e8d116cc 3fe0b292b691856e bfe0b292b691856e bfeb4c56e8d116cc 4008e1f794225aac 40154c559ff60eb6 bfcb10bdeeb906c0 3fe52ffb73ff2afe bfd74a703ada2e17 bfde330b8f596177 bfd9be9039bc774b 3fc388b4ccf0a988 3fce48244c0c65a8 bfe738d540b47970 3fdcd731280f8b8e 3fdbf94a5f2d7a5d bfed3f689ac3cee8 3fbf7952636f43d0 3fc3bf49fe485ba4 3fcf1be1e08c3371 3fd08569c4e3bd8c 3fcaac452c804f62 3fe0d55fbf18f389 3fd98c8d518a1af3 3fe43e202516eb61 bfd6334a859b7ee7 3fcd85df0e7e5a5e 3fdd97ce9b162ded 3fd96be77fa61534 bfd5b518907db364 3fe5b528fe9a1b4a 1
svd 2 5 sparse | c000000000000000 0000000000000000 c000000000000000 0000000000000000 bff0000000000000 0000000000000000 0000000000000000 3ff0000000000000 c000000000000000 0000000000000000 | 3ff0000000000000 0000000000000000 0000000000000000 3ff0000000000000 400cd82b446159f3 bff0000000000000 bfe1c01aa03be896 bfe1c01aa03be896 bfd1c01aa03be896 0000000000000000 bfe1c01aa03be896 bca1c01aa03be896 3ca9aab6801aa043 bc795525ff957ef5 bfeffffffffffffe bc895525ff957ef5 bfd1c01aa03be896 bfb95525ff957ef5 3fee6aada006a811 0000000000000000 bfb95525ff957ef5 3fe1c01aa03be895 bfe9aab6801aa041 3fb95525ff957ef4 3cb0000000000000 3fc95525ff957ef4 bfe1c01aa03be896 bfc95525ff957ef5 bfb95525ff957ef5 0000000000000000 3fe9aab6801aa043 1
svd 2 5 rank1 | 4000000000000000 4010000000000000 4008000000000000 4018000000000000 4010000000000000 4020000000000000 4014000000000000 4024000000000000 4018000000000000 4028000000000000 | bfec9f25c5bfedda 3fdc9f25c5bfedd8 bfdc9f25c5bfedd8 bfec9f25c5bfedda 3ccfffffffffffff c035369480174811 3fe6a09e667f3bce bfd0cd72875e5ef4 3fcf8488a6597d14 3fd3b2d567f7ee2c bfe0cd72875e5ef2 3fcafc19d8606170 3fd43d136248490f 3fdafc19d8606169 3fe0dd90273c3ce2 3fe43d136248490f bfdafc19d8606169 bfbc30b74c8bcd01 3feb4d36c8935dd5 bfc77dee151f2ad5 bfcc30b74c8bcd01 bfe0dd90273c3ce2 bfc19e728fd76020 bfc77dee151f2ad5 3fe8a8a59966429e bfd19e728fd76020 8000000000000000 bfec9f25c5bfedda 0000000000000000 0000000000000000 3fdc9f25c5bfedda 1
svd 2 5 zerocol | 0000000000000000 4000000000000000 3e2e7f8d4095dc2b 4000e4ae9e0575a8 0000000000000000 0000000000000000 3e4288a7086b0122 8000000000000000 bfc05b6f0b650480 3fcd4c723ac709a0 | bfeffff391a06bf7 bf6c34cafd2d86cf 3f6c34cafd2d86cf bfeffff391a06bf7 3fc04e804cc7c9c9 400756f851516a1c bfabad01a83cbff5 bfad3891fa078620 8000000000000000 be722f66ed003f5c 3fefe6a6b2607ce3 bfe5efcca7ce659b bfe72955349e3172 0000000000000000 3da665f214204e00 bfb41f9b9234628e 8000000000000000 8000000000000000 3ff0000000000000 0000000000000000 0000000000000000 be6b763a54a48124 3e661456bb504f86 0000000000000000 3fefffffffffffd8 3e72210d401c0ad2 3fe73bbcb71b9a11 bfe6013b06222311 0000000000000000 3e718f89727a2f8d be5483a200000000 1
svd 3 2 rand | 3fed00f997f4711e 4005b551cf6a1acc 40ef92d1822f9f68 3fd52a53fd3b2644 8000000000000000 3fd3c1b87e241c8c | 3eed65489b96441a 3f06006317a47bcc 3fefffffff797b56 bfefffffff890d14 3f0489ae5b310639 3eed6466ad2a4419 bf04899a25791528 bfefffffff1d886c 3f060075f519dded 40ef92d182b5e4b8 bfd52a41d7618b74 3feffffffffe6f0c 3ed4061ddc119850 bed4061ddc119850 3feffffffffe6f0c 1
svd 3 2 sparse | bff0000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 | bff0000000000000 0000000000000000 0000000000000000 0000000000000000 3ff0000000000000 8000000000000000 0000000000000000 8000000000000000 3ff0000000000000 3ff0000000000000 0000000000000000 3ff0000000000000 0000000000000000 0000000000000000 3ff0000000000000 1
svd 3 2 rank1 | 4000000000000000 4010000000000000 4018000000000000 4008000000000000 4018000000000000 4022000000000000 | bfee990cdad55ed3 3fa67a50100d599e 3fd28681e5e277be bfd11acee5602433 bfe11acee560242b bfe9a83658103640 3fbe990cdad55eba bfeb0228098409ae 3fe0bb0f7d39cd2c 3cedc4b1b016e2b9 c02afb41f4320030 3feaa027f059dce1 bfe1c01aa03be894 3fe1c01aa03be894 3feaa027f059dce1 1
svd 3 2 zerocol | 40041de8768e3ec4 4000000000000000 3ffd30f2b870341d 0000000000000000 0000000000000000 0000000000000000 | bfe5c73df0627028 bfe1524a7fd47e36 bfdf9a24ab8340dc bfe1524a7fd47e36 3fea6bc25a4c6f28 bfc45b822f75c084 bfdf9a24ab8340dc bfc45b822f75c084 3feb5b7b96160100 c00d8f08c50cdb16 0000000000000000 3ff0000000000000 0000000000000000 0000000000000000 3ff0000000000000 1
svd 5 2 rand | 8000000000000000 0000000000000000 3fe35df3bde9daf4 3ff995fe2a06ef92 c0043dbe4b24eb48 bfecbcca59cef6e8 3ffb8649ff5840c1 3ff3065c1614517b 3fef381320297ccc 400257a6a3bebb24 | 3fcaa6345cefe29f bfd9864414efa60c bfdda1664b770a70 bfe7133e8ca3b790 3fd0190435352482 3fc890bdf0d12197 bfd7875198340e5a bfc41feaef0fb41a 3fa8515a83221790 bfeca3e5dcd7e40d bfbb17e0467a659a bfe01958cd051f92 3fea13e5c6c80fb1 bfd109cfa3e2ff92 3f9b5029a7ff1c5a bfd9ebd29b24ee4a bfe4ae6980af99a7 bfd2a0f4bdad773b 3fe08c19c0e513fa 3fd075e736066aad 3feb9ccde6fab73d bfc7f1af23cb5785 3fbc2923999138fc 3fd7e72a2c17f705 3fd0ca9534c5f438 4004b01e8f0c76a1 400df612d3bba928 bfe99c8b0654e366 bfe32f4601b21a74 3fe32f4601b21a74 bfe99c8b0654e366 1
svd 5 2 sparse | 0000000000000000 0000000000000000 c000000000000000 0000000000000000 0000000000000000 0000000000000000 c000000000000000 0000000000000000 0000000000000000 bff0000000000000 | 3cb0000000000000 8000000000000000 3feffffffffffffe 8000000000000000 8000000000000000 8000000000000000 bfec9f25c5bfedda 0000000000000000 0000000000000000 bfdc9f25c5bfedd9 3feffffffffffffe 0000000000000000 3cb0000000000000 0000000000000000 0000000000000000 8000000000000000 0000000000000000 0000000000000000 3ff0000000000000 0000000000000000 8000000000000000 bfdc9f25c5bfedd9 0000000000000000 0000000000000000 3fec9f25c5bfedd9 c000000000000000 4001e3779b97f4a8 3ff0000000000000 0000000000000000 0000000000000000 3ff0000000000000 1
svd 5 2 rank1 | 4000000000000000 4010000000000000 4018000000000000 4020000000000000 4024000000000000 4008000000000000 4018000000000000 4022000000000000 4028000000000000 402e000000000000 | 3fef112f68d83256 3f9f7244f769f5f0 bfab9d7f79a3baf0 3faf7244f769f5b0 bfcc92b0d915499c 3fc1426fac0654e0 3fd1426fac0654db 3fd9e3a782097f49 3fe1426fac0654db 3fe5930b9707ea12 3f8268ffa66d2729 bfd97699bce4d3da 3febd5fba143500a bfc085119e3dfa19 bfd0b991694d22b6 bfc01bdfb19f826b bfdb8397ab7c69fc bfc6cf0b46b9823e 3fe9d1bda9d8eb58 bfd5da0c3dc81a0b 3fc38f8fa0d3f9be bfe8701ac59908c9 bfcb29292a1ebd49 bfc8d62b821d9bac 3fe1d6ac00f058f4 3ce63021484ae2ba c03abd4ed15b9642 bfeaa027f059dce1 3fe1c01aa03be895 bfe1c01aa03be895 bfeaa027f059dce1 1
svd 5 2 zerocol | c000000000000000 bffdeaee3baea77a 0000000000000000 3ffe71bfcf6e431a bdcb6cbbc4ce2c73 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 | bfe331eb9f059620 bfe1f2297a2d25f4 0000000000000000 3fe243083b8235a8 bdb07357eb2bc203 bfe1f2297a2d25f4 3fe9b58ed900024d 0000000000000000 3fc99b2943c79306 bd9710fe5d518c7b 0000000000000000 0000000000000000 3ff0000000000000 8000000000000000 0000000000000000 3fe243083b8235a8 3fc99b2943c79306 8000000000000000 3fe97c5cc60593d3 3d9778f024ceb111 bdb07357eb2bc203 bd9710fe5d518c7b 0000000000000000 3d9778f024ceb111 3ff0000000000000 400aac71c1775ce4 0000000000000000 3ff0000000000000 0000000000000000 0000000000000000 3ff0000000000000 1
svd 3 6 rand | 3ff35a0acf0663af c006557486807a7c bfe653642e913d5c c00653f675f301a4 3ff609f57562d759 3e356c79f2b38eae 0000000000000000 bfc29210ae93d620 bfeae45670c3b476 3feaf9c3354bd41e 8000000000000000 bfeb2d70c9221390 400231bc9cad9f7c 8000000000000000 3ff34dc33a9e53bb 3fd88dadf1de6bc4 0000000000000000 3fec48a15aff1c8c | 3fea6713e50b3ffd bfe2145d13367974 3f764412d69abf36 bfdad65b4e290665 bfe3ccf4e68aaae2 bfe541e21726e809 bfd83c487b5d6e04 bfe1775483bb01af 3fe7eb22b110efc9 4011fe90563f3c52 40053a2277233f9b 3ff5de536fd03236 3fe24a9490c69206 bfe5ea99111479e9 3f919e5bbc9f6707 3fc3a8af0cf1ead2 3fdaca0c642830ef 3fb24964a8612a32 3fe44e4756d01c89 3fbeaf6c8b9a5a70 3fcf42cd16d11b5c 3fb453caba596039 bfe52ac8cc696fd4 bfd20ad8fa5d884f 3fd97c78e6ca117a 3fcc91e4e77b756f bfd9b472478400bb bfe656236b7dcca5 3f9e6468a740d97b 3fd821e32726beed 3fc9aef2deeeebd8 3fd60035b2a7a931 bfe31ce3bee110f8 3fe571195d687082 3fa2221a04504162 3fc7cef86d8e7037 3fd1232aea360d18 3fe2dca8a90c80ae 3fdc646c71b6de08 3f9a0880d9843970 3fe325f6d8be8f32 bfc460df2acf4f77 bf80772c87e8aed1 3fa13fa81826110c 3fde5f07032f6a15 3fc75809890d4839 bfc50cb5cf91948f 3feb06b79c6eda68 1
svd 3 6 sparse | 0000000000000000 0000000000000000 c000000000000000 0000000000000000 0000000000000000 c000000000000000 0000000000000000 c000000000000000 0000000000000000 0000000000000000 0000000000000000 4000000000000000 0000000000000000 c000000000000000 4000000000000000 4000000000000000 0000000000000000 0000000000000000 | 3ff0000000000000 0000000000000000 0000000000000000 0000000000000000 bfd87de2a6aea959 3fed906bcf328d45 0000000000000000 3fed906bcf328d47 3fd87de2a6aea961 c000000000000000 4010cedaa7c992ae 40042602eb0e49a2 3cb0000000000000 8000000000000000 8000000000000000 8000000000000000 8000000000000000 bfeffffffffffffe bfdc249440ea7539 bfdc249440ea753f 3fc7507800c42471 3fdc249440ea753d 3fe3e66820a643ba 3c9d212a94dfe456 bfd372f1b076322a bfd372f1b076322e bfe77a1dffd3b8c6 3fd372f1b076322d bfdb814a4f313f5b 3c8ef317019a287b 3fc711a6b04c7393 3fe06f1456bdc791 3fc02061446ffa9a 3fea3b9653ece31a bfc02061446ffa9a bc80000000000000 3fb94f7d1d037c39 3fdba9349c963463 bfe42879958bf941 bfb94f7d1d037c39 3fe42879958bf940 bc70000000000000 bfea3b9653ece31b 3fe06f1456bdc78f 3fc02061446ffa96 bfc711a6b04c7393 bfc02061446ffa9a 3ca0000000000000 1
svd 3 6 rank1 | 4000000000000000 4010000000000000 4018000000000000 4008000000000000 4018000000000000 4022000000000000 4010000000000000 4020000000000000 4028000000000000 4014000000000000 4024000000000000 402e000000000000 4018000000000000 4028000000000000 4032000000000000 401c000000000000 402c000000000000 4035000000000000 | 3feed6023ba6dba1 bfc2f9d9fd52fd4b bfcc76c6fbfc7bf5 bfd11acee5602429 bfe11acee5602426 bfe9a8365810363b 0000000000000000 bfeaa027f059dcde 3fe1c01aa03be897 3ccad60a1d0fb38c 40460e86d71fb225 39a52a7fa9d2f8ea bfedbf83afee34cc 3fc35e2a53acb722 3fb57a4856784340 3f90e0f0165c6100 3fd35e2a53acb71e bfbe4bdc9c2b3dbc bfc5b6b0e361668e bfd04904aa890cf5 bfd5b6b0e361669b bfdb245d1c39c042 bfe04904aa890cf6 bfe2ffdac6f539c8 3fb44fade568b2c8 3fee7c506fcef5be bfbc5159dbce0379 bfbad848e0117238 bfcdca6ad78a94a9 bfb7e626e8984fc8 bfb5b6b0e3616696 bf92e535e2f45f93 bfd9c3abb991aa86 3febb2c6af669a5a bfcf4396def7c7f2 bfc2c7ff154f493a 3fc5b6b0e361669b 3fa2e535e2f45f7d bfe69d7813be4545 bfc2aac847aa32ed 3fe51782094133e9 bfbf092943ad9b56 bfd04904aa890cf3 bfac57d0d46e8f54 bfdce9df5f650fc8 bfc9146660f613be bfd45b18e838fbdc 3fe88c9309933518 1
svd 3 6 zerocol | bffc9a65e9e801c4 4003d6712e0f2eac 3e3c63857ce73264 3ff72d74564f1ca0 3fde38fdbe6bdc8c bffd9b3fee4494b8 0000000000000000 c0062a4686b108e8 c0009aa5e1c912bb 0000000000000000 0000000000000000 0000000000000000 400691a4ba74efce 3e3d099d52312001 bfc5a6cb9a3dfa48 c0045f4a7b497b9e 0000000000000000 0000000000000000 | 3fe9d4b798d2934f bfdfbfc2c0943cc1 bfd478802cac0114 bfe2ba8a0cf18e41 bfe7c66930517f09 bfd4c8793b12fb89 3fb398a30b6bd669 bfdcc1e2e9195ed5 3fec7b859ee552fb 40134e1a45c9bbd8 400d1848c9631fcf 4001efc676510e24 bfe1b96802adc49b 3fd43f8bddf132ce 3fdb0797cd4d83ca 0000000000000000 3fdeea5f6297ee97 bfdb4240d8adee12 bfcc0482797e0ffd bfc509fe86bfe100 3fe80aef3c56e7a5 0000000000000000 bfdc16cd2a93cbfa 3fda3a4693a3dce9 bfe1db1195bf68d2 bfe8f3e0087ca43e bfd13238dee84a18 0000000000000000 3f9ddd862c3c071a bfb641d82c0f7af9 0000000000000000 8000000000000000 8000000000000000 3ff0000000000000 0000000000000000 0000000000000000 3fdb1ca98bad0d2e bfda4d976f5de5d9 3fd3c82d44c9b377 0000000000000000 3fe6815b77f1706e 3fcfbcabbf5f4a4b bfd927c32daa6193 3fd3d2be4b52f77a bfd3229bd05dd46a 0000000000000000 3fd1f04da3652275 3fe867866daa78e6 1
svd 6 3 rand | bff1d00a501efdda 0000000000000000 4000000000000000 40f4d9e554255add 4005386dc32ed0da c000000000000000 400240405bd4ffea 3ff3ea9860cf0564 40062849566579c2 4002e287150c9480 be054a8b4cbd03ed 3ff562a286def3ec 40f02ed4f96aa3c2 0000000000000000 3ffabd99c49662d9 4002dff8cc06f4ca 3feab22a8f2da240 3e4048aeca778245 | 3ef62ae9c55ab3da 3dfbaf1d0eabb9fa 3ef88e8ce5b0d97b 3fefffffff53a034 3f004898d28a5f77 bef88df65bf735eb bef6126905d83d73 3fd803515d05f141 3feab6bb74b3971c bee5364ec3ba60cb bf001fb4cb2f09d2 3fd9c8fa3dcf32df bfefffffffaab719 be0635326df12000 befa703ebfab41d4 3ef62b2d3463ce87 beea63f6eba18273 be216013eb553000 bee3d657dfb4c5b5 bfed8b7949b45d11 3fd803350e36662f bedcc088ba549d32 bf0a2da732d6e42f 3fb51861036a529a beea64f47aa86725 bf01d4123c3a81bb 3f08b56102439387 bf0048ad464ea68c 3feffffffec35f7e 3ee5b3f97c8cbb2b 3ee54d2abdbfae29 bfb5165d80401145 bfd9c91497e9345e 3f00234a45b6ceec 3edc548258df5b22 3fed2b422ad2cec1 40f4d9e554cdff77 400a8a7537d43ebb 40f02ed4f9aef445 3fefffffff4640fd 3efcfbbe93adfd00 3f0716154908bf87 befcfb566d511b5e 3fefffffff7a1809 bf020ba370fd5b4b 3f071635f8f3aedc bf020b799f4895cc bfefffffff2958b1 1
svd 6 3 sparse | 3ff0000000000000 0000000000000000 0000000000000000 bff0000000000000 0000000000000000 bff0000000000000 0000000000000000 c000000000000000 0000000000000000 c000000000000000 0000000000000000 0000000000000000 0000000000000000 3ff0000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 | 3fb986a521a55ea2 bfe6018b04263e15 bc685b7652c32098 bfe6ca2d8e1b1df7 0000000000000000 bfb986a521a55e9c bfe2b5067f0e8d3e bfde8fe0618a2478 bc7ed19983093562 3fd308cff6985705 0000000000000000 3fe2b5067f0e8d3e bfd8a580a85af95a 3fe17fd22c3bf4c3 3cafbb13f21ccb99 bfe4592442467b6f 0000000000000000 3fd8a580a85af95d 3fc76cf5d0b09957 bc5e2494168860f5 3feee8dd4748bf15 0000000000000000 0000000000000000 3fc76cf5d0b09956 8000000000000000 8000000000000000 8000000000000000 0000000000000000 3ff0000000000000 0000000000000000 3fe5db3d742c2655 0000000000000000 bfd0907dc1930692 3c90000000000000 0000000000000000 3fe5db3d742c2655 4008306f3db83bec 3ff957aa5f9e4562 bfe2e619f23c842c 3fd34b9e8d36ec8d 3feda159ad0a851a bfcd1ca3ce97ed81 bfeda159ad0a851a 3fcd1ca3ce97ed7a bfd34b9e8d36ec8e 3fcd1ca3ce97ed7f bfd34b9e8d36ec8e bfeda159ad0a8519 1
svd 6 3 rank1 | 4000000000000000 4010000000000000 4018000000000000 4020000000000000 4024000000000000 4028000000000000 4008000000000000 4018000000000000 4022000000000000 4028000000000000 402e000000000000 4032000000000000 4010000000000000 4020000000000000 4028000000000000 4030000000000000 4034000000000000 4038000000000000 | bfef2556e51b9d24 3f50529591a73f40 3fb826cce7a8a6ef 3f60529591a73c00 bfb721a38e8e333a 3fc826cce7a8a6ef bfbad60a1d0fb3b8 bfcad60a1d0fb397 bfd4208795cbc6b0 bfdad60a1d0fb397 bfe0c5c65229d03e bfe4208795cbc6b0 bf9e22ea7ab5bb31 3fef43c4e2cc0a59 bfb41cbb31d17f32 bfb4d3bbafb4dfee bfb58abc2d9840b3 bfc41cbb31d17f2a bfb27d55ce98f7ad bf8c11be38a90fc7 bfd98166d14fe4a2 3febce28e1736181 bfc1a7ea6bf7c27c bfcfcd9d10da6302 bfc6a9aff5d455a2 bfa133cfd123e0f1 bfdb6a48dddeb212 bfc5df0c9baf487e 3fe9fe9e5a8b0ed9 bfd3b892a18acc61 3fb0b1689ced77cf 3f895785a67ac86f bfe79884c4c11732 bfc75f58b16b56fd bfc6949c843780b8 3fe3eaf590b06960 3cf199999999999b 4049af836bd28863 b9a52a7fa9d2f8ea 3fedb614bfce6a6c bfcc85d67ab1add4 bfd303e451cbc939 bfd7c4dd663ebb88 bfe1d3a60caf0ca6 bfe7c4dd663ebb89 0000000000000000 bfe9999999999999 3fe3333333333334 1
svd 6 3 zerocol | bff946edaaadfa17 400239f69ecaef5c c004e217e0a2cd54 c003987d68445e8e c00150d5050b62ae c0f13894f573de4b 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 c00242478494678f 8000000000000000 be44b8353d251171 40e7a7c35298365a bfeb4cbb4fdce264 bfe67b257acfcab8 | bef77be438edc99f 3f00ef21e16dd991 bf036709fa3e18b3 bf0bac6a7637831c bf00167cf6246aff bfeffffffe3790dc bf08b34c43a587b7 3e153dd33f12532c be1857576e964022 3feffffffe92b751 bef2775b6e4eeb83 bf0bac33a36640f9 bfe8c4568f2d74e0 3ee0e6a5bc0d4a40 bf07fdfe83a86098 befa8a97ea3feea1 3fe44362e28ce1fd bec18fd193e40000 3f013087f53eb405 3fefffffff2acfba bef01f1a120388e6 3e256d893fcb59a7 3efcac91857bd0ab 3f00eeff5dcad720 3fdf5d0255da5c54 bef7d0151be4a72a 3fe443bab90fc4b9 3f01a25f95686dca 3fe32b25581a78df bf0badea4da8c000 bfd9aa753b3a432a 3f0428a35cedc40a 3fe8c40eb05ff528 befcdc53e58c62f2 bfdf5de5579e8e09 bed35e9b75350000 40f13894f63c9211 40e7a7c352fc1218 3c7f214f87499046 3fefffffffd07fa9 bbbb918822ff4ade befb918822ff4adc 3efb918822ff4ade 3cafffffffd07fa9 3fefffffffd07fa7 0000000000000000 3feffffffffffffe 3cb0000000000000 1
svd 2 2 ident | 3ff0000000000000 0000000000000000 0000000000000000 3ff0000000000000 | bff0000000000000 8000000000000000 8000000000000000 3ff0000000000000 bff0000000000000 3ff0000000000000 3ff0000000000000 0000000000000000 0000000000000000 3ff0000000000000 1
svd 2 2 zero | 0000000000000000 0000000000000000 0000000000000000 0000000000000000 | 3ff0000000000000 8000000000000000 8000000000000000 3ff0000000000000 0000000000000000 0000000000000000 3ff0000000000000 0000000000000000 0000000000000000 3ff0000000000000 1
svd 3 3 ident | 3ff0000000000000 0000000000000000 0000000000000000 0000000000000000 3ff0000000000000 0000000000000000 0000000000000000 0000000000000000 3ff0000000000000 | bff0000000000000 8000000000000000 8000000000000000 8000000000000000 bff0000000000000 8000000000000000 8000000000000000 8000000000000000 3ff0000000000000 bff0000000000000 bff0000000000000 3ff0000000000000 3ff0000000000000 0000000000000000 0000000000000000 0000000000000000 3ff0000000000000 8000000000000000 0000000000000000 8000000000000000 3ff0000000000000 1
svd 3 3 zero | 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 | 3ff0000000000000 8000000000000000 8000000000000000 8000000000000000 3ff0000000000000 8000000000000000 8000000000000000 8000000000000000 3ff0000000000000 0000000000000000 0000000000000000 0000000000000000 3ff0000000000000 0000000000000000 0000000000000000 0000000000000000 3ff0000000000000 8000000000000000 0000000000000000 8000000000000000 3ff0000000000000 1
svd 5 5 ident | 3ff0000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 3ff0000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 3ff0000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 3ff0000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 3ff0000000000000 | bff0000000000000 8000000000000000 8000000000000000 8000000000000000 8000000000000000 8000000000000000 bff0000000000000 8000000000000000 8000000000000000 8000000000000000 8000000000000000 8000000000000000 bff0000000000000 8000000000000000 8000000000000000 8000000000000000 8000000000000000 8000000000000000 bff0000000000000 8000000000000000 8000000000000000 8000000000000000 8000000000000000 8000000000000000 3ff0000000000000 bff0000000000000 bff0000000000000 bff0000000000000 bff0000000000000 3ff0000000000000 3ff0000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 3ff0000000000000 8000000000000000 8000000000000000 8000000000000000 0000000000000000 8000000000000000 3ff0000000000000 8000000000000000 8000000000000000 0000000000000000 8000000000000000 8000000000000000 3ff0000000000000 8000000000000000 0000000000000000 8000000000000000 8000000000000000 8000000000000000 3ff0000000000000 1
svd 5 5 zero | 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 | 3ff0000000000000 8000000000000000 8000000000000000 8000000000000000 8000000000000000 8000000000000000 3ff0000000000000 8000000000000000 8000000000000000 8000000000000000 8000000000000000 8000000000000000 3ff0000000000000 8000000000000000 8000000000000000 8000000000000000 8000000000000000 8000000000000000 3ff0000000000000 8000000000000000 8000000000000000 8000000000000000 8000000000000000 8000000000000000 3ff0000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 3ff0000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 3ff0000000000000 8000000000000000 8000000000000000 8000000000000000 0000000000000000 8000000000000000 3ff0000000000000 8000000000000000 8000000000000000 0000000000000000 8000000000000000 8000000000000000 3ff0000000000000 8000000000000000 0000000000000000 8000000000000000 8000000000000000 8000000000000000 3ff0000000000000 1
svd 8 8 rand | bfff8beeafd039d6 0000000000000000 c0047029bc105c53 4006e5546edb7ea4 3ff2335fcbf590b1 bff5605337f2fa66 3ffd120dc1fe920a bfe56be64279ce1a c0047182f7527f7f 0000000000000000 0000000000000000 3ff4ea4c67b5ea67 c000000000000000 0000000000000000 400698438edca78c 4006116fbf2d7eea 3ff9eb76b1884382 c0018752eea39886 bff2fc94af0dbbfe 3ff7b8d43713416c c000367569c98d0c bff069d9e3e53aab c004dad270e81e9e 0000000000000000 3e21daaa0b0f697a c0c960ea520a0f0a bfeb4163e4464f14 3fd4e0cc2d230aa4 3fff74e11126adf3 bff31ed4081acf50 0000000000000000 c0f2cab40844e279 40f12799ce59a315 400724b6b9558c6c be330f5428421b38 bff3e77ad81c2186 0000000000000000 8000000000000000 bff0000000000000 4006be869db0e885 3ffe6834dcd06c9a c010000000000000 bff430d290be0c39 0000000000000000 c0e8851738313513 40061319b56957b8 c000000000000000 0000000000000000 bffee6f12bf3d037 bffe9063704de430 bfd49fc805b2c440 3fd58ec5b7bc9b08 3ff6e29f74f00203 3e38701830f93045 40021f974cf858de bfaa72756874bd60 3ff6c2b8587116e5 c000f908ecd3596d bfe96c227480b7a2 3fde27d4c33a2904 8000000000000000 3ff28a6464582628 400529f18973dcb6 40070319e3611cce | bf2a2add09790000 bfc54e86094cf01e bee6e1d19012abaa 3ed18aab3f65d7bf 3f019f9fcb316da6 bef00dc9c6a0acf1 3e11769fc26c3000 bfef8db555b17af8 3feffffff46f91d9 3e849731dca80000 be2b15b5143a8800 bef29094b4c00366 bf04b5468d47d5a6 be126533da6a6000 beeddb232e7ca9a1 bf2a8a8ff4dc2000 3f04b5e5352a5fe8 3f15c515c56edad7 3efa5998dd3d0ae5 be22e77a511dc680 3feffffffc374b31 bf0ccef8b287ab7b 3f04e1a76e708aa2 3ef509aa40683410 bf001b0cf19ffc51 3fd75ac7bc9e7f4c 3fd36b4c6c9caa15 bfd9788baa1ff52f bed672707ac19e85 3f692b087dd56ba4 bfe90ba5eac86461 bfaf8b15ca1f97c6 3ee3738e85421a86 bfce26115c06a6db bfddfa67738c16a9 3fe082cfc5a83df4 3f00ae1687a1a27d bfd86986aaf5cf08 bfe1de6f6a903c36 3fa45d6a95e09d15 3ef6bbdc87bc6c16 bfebabb97d30809b 3fc36438a2a740ec bfd21e9572cbf11d 3f176b7d10bcfc11 3fd27bd976f15080 bfcafc6faeb73880 3fc2af16a7148255 bee90cc408b31a6c 3fc3084b9a1f82df bfe9168bb80072fc bfd5bc1c523751ba 3f038ede236a3f3c 3fdf9b809316726a bfadb3bf4babb018 bf99b4358178f101 bed2e3c9e836ec7d bfbe16952e0ae506 bfcce2078fb47bc9 bfe3b937ce4b2443 bf0084858b00dddc bfe73bf91f4cbd08 3fc55737a4f15456 3f9454246578a710 40f30ec5193d6f56 40f12799ce026071 40e885173b269bcf 4018d57e5882f309 4011aa74b0460cff 400abfad72baadd8 3feebee4e346b5e6 bfe2e875d09cbff3 3ee1bf30f189cd49 bf024412eb038338 3ed393f9d707afde 3feffffff21ef702 bf2d11fb6be9c800 beeb78dc308eecb0 3ed3d885c91cc0f4 bf00ae2e37ea83b3 befd6c488c9cbcd0 bf03124741d49685 3ef82d2c594949ce 3f2d11fe8390d800 3feffffff0f41514 3f0cfad6f32e6792 befcd2c687d071a2 3ef5381e4e23ab69 3ef7c0b9bee244ba bf04e15c2713c1e7 bf052921efbfe246 beeb7318e346c476 3f0cfb59adb108f6 bfeffffffdea4c5c 3efddd0cf36b5854 be0bc0080bcb2106 bfe10109495f68d9 bfdde6b058c2097c 3fa7b44367fd974c befcf3ef6fd45b70 bf01a86879a51996 bee01aff3fe85441 bfdbd1f4c9e5dcd6 bfe1c44ae7f3e7de 3fdf098fbd1521fc bfc6fbcaec86c8c4 3feab10e8dd55ae3 bef39b4100ce3b68 bee9474edd694b0d bef39ce03b5ef680 bfbc8a4519730faf bfc3fe7e24826b2e bfe3c09e865dabba bfc58ebccc73c2b5 3fdda2da25c53d11 3eea898bfee73bd4 bf031fe986c2d5fe bef2187e24cc0a76 3fd392ad60df0e4c 3fe1035b5e59b9ac 3fd2d3e0c0f82200 bfe68cd0078c67bf bfd0f2f5fe9cd736 bee0fa0b67ddd919 beff0132c19010a1 3f038633e1a13700 bfd25f97a0338cbc 3fe073a9a0d26615 bfbf3efe43652873 3fde536e58c2c9a1 3fc0f4be2fd99543 3f009d50be0672fb bef2d49f76d848ba bf09ee1b0ea4ddb4 bfe94040555c2bdb 3fd63417b25a3b84 1
jtranspose 5 1 | -1 0 0 0 -1 bfd17f3ad00d95a0 0000000000000000 0 0 0 1 -1 bfd84ea50177f648 40074a1e6a58b31c 1 0 0 2 -1 bff1890485ef5f02 0000000000000000 2 0 0 3 -1 400215ac3e1066c0 c001a37ad3dfcae2 3 1 0 -1 0 c00013576e97829a 4008000000000000 | 0000000000000000 0000000000000000 | | c008000000000000 bffbc6e0292b9fcc bfb6bc32b4e99c80 bffa13059cd107a2 c008000000000000 bfed3b54ae7f4c64 c014d1bd69efe571 c0111481d653f4ad 400013576e97829a c008000000000000 bfb756bba6733379 3fe0c152382d7365 bfd763c9f47b3411 3fd0ba1b7e42cbb5 bfd54357ab95d260 c02a940745798f62 7ff0000000000000
jtranspose 8 1 | -1 0 0 0 -1 bfb7d83b7b39fbc0 bfd5defd91b2cffc 0 0 0 1 -1 0000000000000000 c00574067066a29a 1 0 0 2 -1 3fc740e9a5e452c0 4001dc18c8a9deb0 2 1 0 -1 0 c0051a628a68f97c bff0000000000000 0 0 0 3 -1 be41da036632b06b 0000000000000000 4 0 0 4 -1 bfcbc273cd0cea38 3fdc06fdb2e1183c 0 0 0 5 -1 bfffd3a7fa278b90 bff9bc79d9f59ec6 6 0 0 6 -1 8000000000000000 0000000000000000 | 0000000000000000 4000923fb25af0e6 | | 3fe5108137269802 c0045ba0ae8f299e bffae80ce0cd4534 c0051a628a68f97c 4009dc18c8a9deb0 c0068e7124c73ea8 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 40051a628a68f97c 4008923fb25af0e6 bfd03faa4c685132 bfe0c152382d7365 bf76c574cab8222a 0000000000000000 0000000000000000 0000000000000000 0000000000000000 4030a700d1f8014b 404875c23e42df37 7ff0000000000000
jtranspose 6 1 | -1 0 0 0 -1 c000ddb52f798ebb c005ff3c867fb826 0 0 0 1 -1 4008000000000000 bfe121c79157f098 1 0 0 2 -1 0000000000000000 c008000000000000 2 0 0 3 -1 bff7164fc1865345 4004ae3ed970c1dd 3 0 0 4 -1 bffa0e06d30d708c 3ff9617f10d3a975 3 1 0 -1 0 3fe3b5c9c8684ea8 bff3587501e9c1a8 | be409a32838640d4 bffec4ca5a93ebfc | | bff8a6040b15aea4 4005cb27a193a265 3fe58f22727b92b8 c003128d8de5ec56 bffca78afe163e58 3fe3b5c9c8684ea8 400e5a795a65a2b1 4000789a52dd3d4c 0000000000000000 0000000000000000 bfe3b5c9cc8edb49 bfe6d8aab15454a8 bfa9a5e240731280 3fb092430882e1db 3fa114a5c5db9a84 bfc881ae61734669 0000000000000000 c02a7606554647e1 c02a6c49e7f0d3ec 7ff0000000000000
jpseudo 7 1 | -1 0 0 0 -1 3dfa5bbcd0fac42c 0000000000000000 0 0 0 1 -1 0000000000000000 4001a5833355212c 1 0 0 2 -1 3ff5de9569b98eeb bfecc1ed845c2426 2 0 0 3 -1 3ff9553d2ce8a287 4000000000000000 3 0 0 4 -1 3ff40e7ec6fbc25e bfda28c777f6a128 1 0 0 5 -1 4000907c4fae837a 0000000000000000 5 1 0 -1 0 c001b3816f814642 c0003ac0fb09f822 | c008000000000000 3fdd300ba9848f80 | | 40003ac0fb09f822 c001b3816f8e7420 4010f022172f8ca7 c001b3816f814642 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 40003ac0fb09f822 c01121fedf97e4de bfe931fa41fae6f8 4003e0c2703a8a12 bf632ac43e25fb24 3f64c95e22663f1c 0000000000000000 0000000000000000 0000000000000000 bf81df46a2529d39 0000000000000000 0000000000000000 7ff0000000000000
jpseudo 5 1 | -1 0 0 0 -1 3e2d78c35a7c2cac be433ed5f2bba60d 0 0 0 1 -1 4008000000000000 40f1e0f223c141c3 1 0 0 2 -1 c007ae833dfa044c 3ff661301a2f5be9 2 0 0 3 -1 c0e568eef61cbcf9 3ff25d3bacdc6db5 3 1 0 -1 0 3fcb95d77e52e8f0 be44e67b31a57f74 | 3ffd6819acf4936b 3f2d8e4bc1b7e000 | | 3e0a7a53ee9d9670 3fcb95d776f4b819 40f1e0f223c14460 c00646a2881ad171 3ff661301ccc2b4f 400967e0b5df32db 3ff25d3baf793d1b 40e568f5db929c8e 3ff9f55ebd2a364d 3f2d8e9f5ba4a696 3d21296842b0538b 3ef73b12f14d7994 3dfd1c25c857ac9a 3e3c2182ec09e893 0000000000000000 0000000000000000 7ff0000000000000
jpseudo 7 1 | -1 0 0 0 -1 3ff819df00df999c c010000000000000 0 0 0 1 -1 40d3e7fa8fdf1e32 bfe61e49109f7176 1 0 0 2 -1 c000000000000000 bffa26349e046028 2 0 0 3 -1 3e36101a7a19af0e c00156f0e6523ae2 0 0 0 4 -1 be2f217502eb3b02 c0032d24b3e643bc 4 0 1 5 -1 40ec8037b83e10fa bff3a63421cb3ed1 5 1 0 -1 0 c00766ecb39b7718 4003cfba86e32c76 | 3fea05aa1bd6b44a 3fefac279cf00532 | | c019e7dd4371963b c011b9ee1a05a1f3 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 c0137e6f9d64b819 c00766ecb31ef144 0000000000000000 0000000000000000 400de8573a91242a bff7c9613f4e5653 3f78ceff5d6032fd bc45bd2fbf0858eb 0000000000000000 0000000000000000 bf81df46a2529d39 0000000000000000 0000000000000000 0000000000000000 7ff0000000000000
jpseudo 8 1 | -1 0 0 0 -1 3fb43efd228b39b0 4008000000000000 0 0 0 1 -1 3e2005ca3f4072e7 bffec985e7a3d402 1 0 1 2 -1 4005c5b07d6271f5 0000000000000000 2 0 1 3 -1 4005838dd3f3c6c6 3ffdbd491a4885d8 3 0 0 4 -1 8000000000000000 4007b4e7252bc328 4 0 0 5 -1 c004c516563c4214 be39f27a63ffd256 5 0 0 6 -1 400132b2b8af053e c0f2fe7777e16d7f 6 1 0 -1 0 4001a5017bc30f57 3e2037ef84bca984 | bfefbf77f7387c90 0000000000000000 | | 4007ffffffbf2042 4001030992aeb58a bffec985e825937e 4001a5017b82f82e 0000000000000000 0000000000000000 0000000000000000 0000000000000000 4007b4e724eae36a 4001a5017bc30f57 be410739132f138c 4013350be8ffa8b6 c0f2fe7777e16e01 3fac93b0c5028640 c00994df79912e7b be2037ef84bca984 be1b3a8131de15c1 3e1176e3374f90a7 0000000000000000 0000000000000000 be1ae54e1fe6ed3e 3ce065c514d9d9fb 3f058c9453d8a4ba 0000000000000000 0000000000000000 7ff0000000000000
jpseudo 5 2 | -1 0 0 0 -1 40bf2e072cd74eab 0000000000000000 0 0 1 1 -1 bffc9da20e28a52e 3e362208efb23903 1 0 0 2 -1 0000000000000000 0000000000000000 2 1 0 -1 1 bddf614ad4fd35cb 3fbf6bdb4280f2e0 1 1 0 -1 0 bfe8c5442a4d4458 c0d42767d1880caf | bfdef1b5f5a03e98 0000000000000000 0000000000000000 3ffd30d9a90bc32a | | 413f7d92376493d1 c1285c906bf23dd8 bfbf6bdb4280f2e0 c0bf2e072cd74f29 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 bfbf6bdb4280f2e0 bddf614ad4fd35cb 403d0ec8b46713c6 413f7d92376493d1 3ddf614ad4fd35cb 3ffb3a1bf4e3b3fc bf81df46a2529d38 b7b6ac0eb3ef3374 3f26e0b5086dc575 0000000000000000 0000000000000000 0000000000000000 0000000000000000 7ff0000000000000 7ff0000000000000
jpseudo 6 1 | -1 0 0 0 -1 3ff79a4e4d9b1808 4002de1f8e5de4e5 0 0 0 1 -1 c0012d3ce3f314b2 c007367e2f3d6ff1 1 0 0 2 -1 40016a020c88980e c0e22acc866df758 2 0 0 3 -1 3fe4384a16dcccd6 3e19a90ed4ac2df1 3 1 0 -1 0 be44a2e693ee389c 0000000000000000 0 0 0 4 -1 40070e8cc6a38d2c 40054ac612e87666 | 0000000000000000 0000000000000000 | | 4002de1f8e5de4e5 bff79a4e502f74da c007367e2f3d6ff1 40012d3ce2a8e649 c0e22acc866df758 c0016a020dd2c677 3e19a90ed4ac2df1 bfe4384a1c05867b 0000000000000000 0000000000000000 3e44a2e693ee389c 0000000000000000 3c72dfeb16d7dbe4 bc7738a95ae8a697 bd522cb29538c63c bb7362f1f97e2986 0000000000000000 0000000000000000 0000000000000000 7ff0000000000000
jpseudo 5 1 | -1 0 0 0 -1 400260e8165b3498 0000000000000000 0 0 0 1 -1 bfe685d9e2b3516c 0000000000000000 1 0 0 2 -1 bfe57ee9c5abf524 0000000000000000 1 0 0 3 -1 bff25ad1c8858791 4008000000000000 3 1 0 -1 0 bfdfbd8e31ee0524 0000000000000000 | 3ff2fc86c9d0a902 3ff5d190deb3cf71 | | 0000000000000000 c0065899dc98f53c 0000000000000000 3fca9c4b26f13b68 0000000000000000 0000000000000000 4008000000000000 3fe4d6dc78140c90 3ffaebea564c2a4b 3ff5d190deb3cf71 bf76a775e45423cc 3f3afa33c5576c70 0000000000000000 3f81df46a2529d39 0000000000000000 0000000000000000 7ff0000000000000
jpseudo 3 1 | -1 0 0 0 -1 bffc3363b59dc864 be2cb17a9ddb6e71 0 0 0 1 -1 4006634811eff8ee 0000000000000000 1 1 0 -1 0 bfe8238f5cab574e bfc358a8c2f23660 | bfef7b99c23e4210 bfff2b434e2eb006 | | 3fc358a8bbc5d7b9 3ff0219c07481cbd 3fc358a8c2f23660 c00c6c2be91acec2 bfcd6029964bab08 bffcc02e35d0693a bf81df46a2529d39 3f352861de2839b1 0000000000000000 0000000000000000 7ff0000000000000
jpseudo 6 2 | -1 0 0 0 -1 40001685e6800c0a 0000000000000000 0 0 0 1 -1 4003302fa6c581ff bdf4d6e03a56cfc6 1 0 1 2 -1 8000000000000000 3fc181f44efbabe8 2 1 0 -1 0 400107a4d28a4a48 400435597fb6c0e0 1 0 0 3 -1 3fe363ec98af0276 3fc7d603e3c2d070 4 1 0 -1 1 4007bb60f52278c6 3ff93a0d25fb3f75 | 0000000000000000 4010000000000000 40e6c9847a0c8ae6 3fd49ed0cc467104 | | c06f935bd78d8d5e 40278c050d00140e bff93a0d25fb3f75 3fee936c3a89b2f0 c06f935bd79dd53d c03afac75ce43870 bff93a0d26101655 3fe22cc53973db1c 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 bff63f4ca982e567 4002e265cef6b828 c06a9bf188f81410 40626ca4287272a2 40e6c9258c88b65c bff41258f2e9a334 3f2d09cacc55f356 bf2776d4b74b2469 0000000000000000 bf81df46a2529d39 0000000000000000 0000000000000000 0000000000000000 0000000000000000 7ff0000000000000 7ff0000000000000
jpseudo 7 1 | -1 0 0 0 -1 bfed4b0bd701cd70 bffb3123509d692d 0 0 0 1 -1 c00286a181c78e30 3fef0a51e1530eba 1 0 1 2 -1 bffb48fe1b47e8b2 3fe0e1286dc6b54a 2 0 0 3 -1 c0074fdf7d9c9548 3ff234853f3495fd 3 0 1 4 -1 0000000000000000 0000000000000000 3 0 0 5 -1 0000000000000000 bff8073f19668233 5 1 0 -1 0 c0078ae51bdb52a7 bff818622ba6750b | bffcd5570df517bd bff12e3b94a190cd | | bfc8c60927b7a110 c0003822261adf4b 4003cec58e27fe34 bfe4110e684f11dc 0000000000000000 0000000000000000 40052673b56d8584 bf9d82cf1f5eaf80 0000000000000000 0000000000000000 3f7123123ff2d800 c0078ae51bdb52a7 3ff2407329c18d91 3fdba89a5c1390f8 bf6e71542016c269 3f7dbf9fc91984b6 0000000000000000 3f81df46a2529d39 0000000000000000 bf72384d4dd492dc 0000000000000000 0000000000000000 7ff0000000000000
jdls 7 1 | -1 0 0 0 -1 3ff2e059457d81e7 40e442cb44516f73 0 0 0 1 -1 3fffe2481c3f5805 4001b14873ac39e2 1 0 0 2 -1 3ff0000000000000 bfd60c30850a855c 2 0 0 3 -1 be4532f0bfc4d607 3e3ca075a245da98 3 0 0 4 -1 3fe9613f1d29a290 bff9994c0f640363 4 0 0 5 -1 400043745afd7ca2 3fdc54b1b7df5ad0 5 1 0 -1 0 bfe69bba4eba52e6 0000000000000000 | c0041da42a29456d bfffb7164eda6ddc | | 40e442cb44516f73 bffe2e366cdaab5a 4001b14873ac39e2 c0059812a1ce40bc bfd60c30850a855c bffb4ddd275d2973 3e3ca075a245da98 bfe69bba496d96b6 bff9994c0f640363 bff7fe7cb5f1fabb 3fdc54b1b7df5ad0 c005ea62eeac115c bffced6b2cf56167 bfffb7164eda6ddc be88adb31f57b211 3f4c2cd29d3c5cc0 3f41d044afd1ff04 3f2d7fe45f61ad9e 3f3f4f2ccfe82897 3f4c987103b761f4 bed1745ff0060344 bfb76d9f1f7cc0f5 7ff0000000000000 41d9a8130413fa50 c0f31c2428dac198 c0f31c2428dac198 40393804f5471645
jdls 8 2 | -1 0 0 0 -1 0000000000000000 0000000000000000 0 0 0 1 -1 c005b9e64cff4320 40022d6cef8b42f9 1 0 0 2 -1 3ff9d8fc2abfb2f5 c0e7e76fe0403f2e 2 0 0 3 -1 bfe7e87da76facd6 3fd76230f9cb6294 1 0 0 4 -1 40f0d56890f44b93 3ffbb6ab07a85319 4 1 0 -1 1 4004445ae8ab08e6 be306e644c455a3d 0 0 0 5 -1 bff5951bb0fa97e7 be37bc6623c14d50 6 1 0 -1 0 be2394efe17981dc 3fd56fa6cfc74dc8 | 3fe9a8b27281c4a2 3fbd3651d4ef4980 bfe16aa60c6c929c 3ff2676385af469e | | c040bf3a5253b4c4 be8e98b6d04ddae8 3e306e644c455a3d 4004445ae8ab08e6 0000000000000000 0000000000000000 40022d6cf00eb61b 4014ff209ad52603 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 3ffbb6ab08af395e c0f0d540083e7a3d c040bf3a56f680b7 4060dc7da1c963d1 0000000000000000 0000000000000000 40540bcb6a6a2755 c036153cad79f0d2 c0089f046bc62d8d 3ff2676386b62ce3 bf4c987103b761f4 bf3c2af7b48c2dae 0000000000000000 0000000000000000 be7325322a0113ad bf10d346e5d67707 3fb106da30afe9af 3f8f55c40f40fa45 bfdee736329d3079 bee96f0462f08559 7ff0000000000000 7ff0000000000000 40a189cdafe27b18 c0b1a60337f0f1c4 be8132c5fe4b22f0 c0553694c5f66786 c0b1a60337f0f1c4 40d1c52736f23734 bccf6bd085768b91 bea360cdb2a42e16 be8132c5fe4b22f0 bccf6bd085768b91 4022bef4ff8e0b4a c0fd274ae17d0b16 c0553694c5f66786 bea360cdb2a42e16 c0fd274ae17d0b16 41f1b59a4c8b9d89
jdls 8 2 | -1 0 0 0 -1 bff046d9803e5cc3 c00233bc931ea368 0 0 0 1 -1 3fec21457141e938 40020409d53e2b5f 0 1 0 -1 1 40eb0e8b823e49a5 40f0961aa1e5b200 1 0 0 2 -1 3ff31df1930a84c4 3ffc5940db7a1e8d 3 0 0 3 -1 40004b7901f451f4 3fe0751656182e58 4 0 1 4 -1 3fecf2adf0c3755e 3fd28555be294978 5 0 0 5 -1 bfb34037786c8550 c0f4225a2471d15a 6 1 0 -1 0 40b399821e930221 40060f4d3dc46e36 | c002081b0d86b16c 3ff64bbe1c4ad8b5 bff17f21b44a673f 0000000000000000 | | c07f745fab3165c3 411ea1723b03396b c0f0963f095ed83d 40eb0eac0ff14a22 c04946654d4721c0 411e9e7bafe1ab9c 0000000000000000 0000000000000000 c058a45c0a1708ac 411e9dfd632e574d 0000000000000000 0000000000000000 c06c0a2bf6e17a1a 411e9cac9223519c 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 c15f75f1c8c33823 411e9ff9641c5f7e 0000000000000000 0000000000000000 c11ea360e50e57a2 c0610cbc2a6872e7 c0eb0eae8081b23a c0f0961aa1e5b200 3e900ec7a4e562b8 bf332988effe2bad bf33291cc20c177e bf3327fda9c26639 0000000000000000 3f4c987103b761f4 be452915e1c44b11 be65d362da232b92 c0e65c68e4486b5e c0eb6a735f299437 7ff0000000000000 7ff0000000000000 42ceee372e921e97 c28e1caba5ea4811 41804ddf4c5d78c6 c17a989cc37a85ca c28e1caba5ea4811 427250275517c5a2 c21fc114778ea2bb 4219e643b6cd2b73 41804ddf4c5d78c6 c21fc114778ea2bb 41f13200f1b918ca c1ec0cc036a4e3a8 c17a989cc37a85ca 4219e643b6cd2b73 c1ec0cc036a4e3a8 41e6e0c9153903f6
jdls 7 1 | -1 0 0 0 -1 3fe8aacc81348e38 400754fe7a4c1c26 0 0 0 1 -1 3ff52b0a09ae2770 3fee8014d5ca4abc 1 0 0 2 -1 bfff660431ce1084 c0e11933aaae67fc 0 0 1 3 -1 3ff0d5ba00a15ba7 4010000000000000 3 0 0 4 -1 c0d252f34048fdef bfe02fb772902178 1 0 0 5 -1 bffa5af853b38dac 3fcc01075ba67090 5 1 0 -1 0 bfe242afd22c444c 0000000000000000 | bffe694ec48fdbea c004fe7a346329cf | | 400754fe7a4c1c26 bff576be29b06942 3fee8014d5ca4abc bffe4c61f2c44996 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 3fcc01075ba67090 3ff139a06a9d6b86 bff547f6db79b9c4 c004fe7a346329cf bf40433a4c8692de 3f4c86c8910eda75 0000000000000000 0000000000000000 0000000000000000 bf4c987103b761f4 bfddfdc7ff6e1a07 bfe56e1c418ac046 7ff0000000000000 40255838f6d1cee7 c015ed63b5109e18 c015ed63b5109e18 401f049b70fd4905
jdls 7 1 | -1 0 0 0 -1 bfe88e289b434cfc 0000000000000000 0 0 0 1 -1 8000000000000000 4007f8cb67914e16 1 0 0 2 -1 c0019da0910ba76a c0e40bf6a20736b6 2 0 0 3 -1 40070737a4e240c6 400544a49c8a10da 3 0 0 4 -1 3e4176a799a3d163 40025335c56d57f0 4 0 0 5 -1 3ff6302cce2ea93e 3febcab76902b268 5 1 0 -1 0 c002ac93473cff08 8000000000000000 | 4008000000000000 c010000000000000 | | 0000000000000000 bff9121240d85792 4007f8cb67914e16 c002ac93473cff08 c0e40bf6a20736b6 bfc0ef2b631579e0 400544a49c8a10da c014d9e5760f9fe7 40025335c56d57f0 c002ac9348546982 3febcab76902b268 c00dc4a9ae5453a7 40155649a39e7f84 c010000000000000 3f3130e4ff2aa1bb 3f399c2482d021e5 be88755f4c48034a 3f4c987103b761f3 3f399c22de31cc5b 3f446985116cdd67 3e8f61bae04a1a0b bfb26c598e3ae19a 7ff0000000000000 41d91df18cdb10ad 40b51a313b329d6e 40b51a313b329d6e 404bccbd03f615d8
jdlssvd 5 1 | -1 0 0 0 -1 40d47d7674fba6ec bff9dd09e7b533ef 0 0 0 1 -1 3fecfc699435bab6 0000000000000000 1 0 0 2 -1 3e4410462fb6445d 0000000000000000 2 0 0 3 -1 8000000000000000 be255c2bb8ed978e 3 1 0 -1 0 3e1d4266b4efa6ac c000fd2df0b659b7 | 40f203c2ae704b12 8000000000000000 | | 3fe03aa3f36efefe c0d47d7674fba518 4000fd2df0b659b7 bfecfc69934ba780 4000fd2df0b659b7 be4067f959184f88 4000fd2df060e908 3e1d4266b4efa6ac 40f203c2ae704a9d 4000fd2df0b659b7 becf9c01aa28076b 3fb6570931eed838 3fb657184ae74487 3fb657184a76ecf2 0000000000000000 0000000000000000 7ff0000000000000
jdlssvd 3 1 | -1 0 0 0 -1 400208ca48ac41b2 bfe808256b812bfc 0 0 0 1 -1 3fdbe6e4410be364 bfcb9903487b48d8 1 1 0 -1 0 0000000000000000 4008000000000000 | be2a784654a5054e c005da8065bf30e6 | | c00e02095ae04aff c00208ca48ac41b2 c009b9903487b48e bfdbe6e4410be364 be2a784654a5054e c016ed4032df9873 3fb657184ae74487 bfb6125a065446f4 0000000000000000 0000000000000000 7ff0000000000000
jdlssvd 4 1 | -1 0 0 0 -1 4007f81e171f007e 4005ee6e91a45df2 0 0 0 1 -1 bff03549f5318eee bfeca508bb90ff62 1 0 0 2 -1 3fe6419672c88c44 c0018403bbc869fc 2 1 0 -1 0 3fe6de52e39225e0 3ffa7d100804c9fa | 40011c83bafc7144 40e78c6f44700e93 | | 3ff15fcd1b43f1ea c00240895e3a7706 c00467ca32e6a4d6 3ffba47366faa1de c00ec28bbfcacef9 3f93978e19333380 3ff6c9de042fcf98 40e78c3a4a4ffe89 bfb657184ae74487 3fa6453ad74d6ef2 bfa949c00cd17c46 0000000000000000 0000000000000000 7ff0000000000000
jdlssvd 8 2 | -1 0 1 0 -1 be2bbc1c0d6764d4 bffa48cd6b3006be 0 0 0 1 -1 c000e01df0c5396e c008000000000000 0 0 0 2 -1 c0e8ea8240c853a0 40006ceb01805e54 2 1 0 -1 0 c0008cf749548f08 bffd3bc603564689 0 0 0 3 -1 40f5138accade911 4001b9d4f48fea80 4 0 0 4 -1 bffcb9e852ea3f49 0000000000000000 5 0 0 5 -1 3fec9958883245ee 400433d51835eed2 1 1 0 -1 1 c004cb390dc11d92 bff1a92967c9c8e8 | bfec038c6f38f81a 40025fc077ade660 0000000000000000 4010000000000000 | | 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 bffe56d698363718 bfdf58d8e7df2120 40784070f279fd3f 415377020a17bc2d 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 405dd5bf0e43bd25 4079c627b6cd8f88 4004cb390dc11d92 40146a4a59f2723a bd5d20b110b08746 bfb657184ae74487 3ed417a0e2d7a151 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 7ff0000000000000 7ff0000000000000
jdlssvd 7 1 | -1 0 0 0 -1 bffb9e86021ec8b5 be365cb7c274d227 0 0 0 1 -1 c001c368df8c4d90 bff9987348839639 1 0 1 2 -1 4001aaa3635750be 0000000000000000 2 0 0 3 -1 c006cccf1b344c12 0000000000000000 3 0 1 4 -1 bff0826829ad3cd6 40f42ff98bbd1f00 0 0 0 5 -1 0000000000000000 3e437e7be3f86e36 5 1 0 -1 0 bfe070ea1adda5de 40066b06b162a5d8 | 8000000000000000 c005fc640a185d74 | | c0066b06b2158b96 3ff36610f4aff5c6 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 c0066b06b02abe1a bfe070ea1adda5de 3fe070ea1adda5de c01633b55dbd81a6 bfb657184ae74487 3c7d5085b437b6ee 0000000000000000 0000000000000000 0000000000000000 3fb31de6fd0591e4 0000000000000000 0000000000000000 7ff0000000000000
jclamp 8 1 | -1 0 1 0 -1 bff947afecda0c9a bff00b74d4a7d0b9 0 0 0 1 -1 3ffd97b07e7f8c25 4008000000000000 1 0 0 2 -1 3e2a025cef2c6c65 8000000000000000 2 0 0 3 -1 3e418eba7a94f214 c00059bb8e4ae65d 3 0 0 4 -1 bffead8667a5e374 bffefae255476346 4 0 0 5 -1 bffbec9869d3f4f6 0000000000000000 5 0 0 6 -1 4010000000000000 0000000000000000 6 1 0 -1 0 0000000000000000 4005e031ca0d2353 | 8000000000000000 3ffc793995ffb22b | 7ff0000000000000 | 0000000000000000 0000000000000000 3fd0fe71af96e568 bffd97b07e7f8c25 c005e031ca0d2353 be2a025cef2c6c65 c0131cf6ac2c04d8 be418eba7a94f214 c012aed17a586a7b 3ffead8667a5e374 c005e031ca0d2353 3ffbec9869d3f4f6 c005e031ca0d2353 c010000000000000 8000000000000000 bfee8e53fc3528f6 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 8000000000000000 bfee8e53fc3528f6 7ff0000000000000
jclamp 6 1 | -1 0 0 0 -1 0000000000000000 0000000000000000 0 0 0 1 -1 bfe914fe4a3f66bc 400522be6d7f3986 1 0 0 2 -1 40c969326b962f9c c0ac75d184bf4d95 2 0 0 3 -1 4006bb76b8ff8ec4 3e25dd80991a3dcf 3 0 0 4 -1 bfaab2c7e9d248c0 3ff2be14905a0d47 4 1 0 -1 0 c010000000000000 0000000000000000 | 4010000000000000 c0059dd76c4dfa45 | 0000000000000000 | 0000000000000000 c010000000000000 400522be6d7f3986 c009bac06d702651 c0ac75d184bf4d95 c0c96b326b962f9c 3e25dd80991a3dcf c01b5dbb5c7fc762 3ff2be14905a0d47 c00f9534e058b6dd 4020000000000000 c0059dd76c4dfa45 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 8000000000000000 0000000000000000
jupdclamp 5 1 | -1 0 0 0 -1 8000000000000000 4007dfa5776ca97e 0 0 0 1 -1 402147ae7d17ceb8 be1dd16a138e80f7 1 0 0 2 -1 3ffffbeaf32d1f1d 0000000000000000 0 0 0 3 -1 3fe22dcf1b1d3460 c00141a4d6735f53 3 1 0 -1 0 bff9067b16ab362b c000000000000000 | 3ffe5659afd87d1d 3fe8efb0cd166e88 | 3ff11b76076b3960 | 4013efd2bbb654bf bff9067b16ab362b 0000000000000000 0000000000000000 0000000000000000 0000000000000000 bfc41a4d6735f530 c0010eb1521ce82e 400bae6a6341d9a4 40063bec33459ba2 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 400b333333333333
jupdclamp 7 2 | -1 0 0 0 -1 3e443a8a9ad0195f 8000000000000000 0 0 0 1 -1 bfe1164a49db286e c010000000000000 1 1 0 -1 1 400313dac8fab8a0 bff0000000000000 0 0 0 2 -1 3ff0000000000000 40048bebffb5f68e 3 0 0 3 -1 c0f408681bb7985f 4000a94ce559d34c 4 0 0 4 -1 bfe28cf06766e6b2 8000000000000000 4 1 0 -1 0 bfee2256851c56ce 40f2b401c629f01b | 0000000000000000 0000000000000000 3e4354e16d3ef814 c000000000000000 | 4000200ce74853fa 7ff0000000000000 | c15d3942c5a1872a c0578ad39bf192e3 3ff0000000000000 400313dac7b70ff6 0000000000000000 0000000000000000 c008000000000000 4007596d5b7182bc c15d390290600811 c0684569cbff11e8 0000000000000000 0000000000000000 c15d390eb4913a71 415f4d0b207b4617 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 40578ad397fe23d1 c15d3942c5a1872a c00313dac7c56a89 bff0000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 400b333333333333 400b333333333333
jtranspose 4 1 | -1 0 0 0 -1 4000000000000000 4001f2654e009b90 0 0 0 1 -1 c001242e55eb5f94 bff46ccc9287a5c2 1 1 0 -1 0 bffe9ba283bb813d bfffcdef07a40299 1 0 0 2 -1 bfec2b169f99b61c bfa6b11515630920 | c0022ece4a5914ea 3fe7486fb796cb86 | | 4010ecae68e94e6e c00f4dd141ddc09e 3fe6c244ea38b9ae 3fcd65d140d9ef58 0000000000000000 0000000000000000 bfd707e843daa25c 4005b91371b7b42e bfd784e8abf135cc 3f86c7e375b7d75e 0000000000000000 c049919d31db8023 4047cf37f41b612a 7ff0000000000000
jtranspose 8 1 | -1 0 0 0 -1 4008000000000000 c005f67076ea750e 0 0 0 1 -1 c001f18c5bfd53ea bff0000000000000 1 0 0 2 -1 bff7e2676a74bf9f 40006518a4804eb6 2 0 0 3 -1 4000000000000000 0000000000000000 3 0 0 4 -1 3fcb6b172bea5ae8 c010000000000000 4 1 0 -1 0 be3a22c50981a2d8 3ff2a7f75a7a9529 4 0 0 5 -1 c0011cdb117e40d2 0000000000000000 6 0 0 6 -1 40c59353a3633c69 bfee50a435c0f228 | 4000a3e4de2d1282 0000000000000000 | | c00f4a6c2427bfa2 c008000000d11628 c00153fbad3d4a94 4001f18c5b2c3dc2 3fec4473dd0c1086 3ff7e26768d2934e bff2a7f75a7a9529 c000000000d11628 c014a9fdd69ea54a bfcb6b1738fbbd6d 0000000000000000 0000000000000000 0000000000000000 0000000000000000 4000a3e4defe28aa bff2a7f75a7a9529 bfbbf9ae6d25f206 bfc579ff8fc0af55 3f62b0543971abc9 bf62030d0790c678 bfcfa7ed4769ff68 0000000000000000 0000000000000000 4055fe954c8dadcb 3fe0ad8a86bdfbe0 7ff0000000000000
jtranspose 6 1 | -1 0 1 0 -1 40f6d775081afc14 c008000000000000 0 0 1 1 -1 4008000000000000 3fedcef271d948fc 1 1 0 -1 0 4001b8818305d240 bfd0fbc002675820 1 0 1 2 -1 3ffd425c83b480b2 3ff2f374631212fa 3 0 1 3 -1 3fe047265cda6db0 bffc5aacd0ba0063 4 0 0 4 -1 40f6405a6ff5883b 3ff4e402a20aac7e | bdc5ce6436415e9a 3fcdc10761eb0d68 | | 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 c001b88183072f26 3fdfdc43b35cded4 fff8000000000000 fff8000000000000 fff8000000000000 fff8000000000000 fff8000000000000 0000000000000000 0000000000000000 7ff0000000000000
jpseudo 4 1 | -1 0 0 0 -1 3e401667f4ba12a9 c0059ec40f2aa318 0 0 0 1 -1 be3f6b495322bcde bfe6099300056174 1 0 1 2 -1 3ff075f401db56cd 400324cbbcdc2c5e 2 1 0 -1 0 c00196d0e0db75f1 3fe287bfc007a968 | 3fe034f932d6628c c001c259c1f914cc | | c00a40b3ff2c8d72 c00196d0e1dcdc70 bff448a96006856e c00196d0dfe01ba6 0000000000000000 0000000000000000 4005a40f2d910e94 c0066449b1faff26 bf766e168e6864b3 3f81df46a2529d39 0000000000000000 0000000000000000 0000000000000000 7ff0000000000000
jpseudo 7 2 | -1 0 0 0 -1 3fcb9a61c1ea57c0 bffacaa4c2fafec8 0 0 0 1 -1 bff2f3cfa44ad6b5 bfffc6c089000639 1 0 1 2 -1 bffd5068dcd2f281 8000000000000000 2 0 0 3 -1 c000000000000000 0000000000000000 3 1 0 -1 1 4006264e138131f0 0000000000000000 1 0 1 4 -1 0000000000000000 bf78f1f641e20e00 5 1 0 -1 0 3fea912fe3d117be 3ff61d48dc12b9b4 | 0000000000000000 40c486f984a518ea 3ff568fc09d618a5 c008000000000000 | | c0731a98d2215c10 404eba8ca4372ad2 bffacaa4c2fafec8 40046ca7f7628c74 c0750d13ab7b52f8 40692f30ed582504 bfffc6c089000639 400fa035e5a69d4a 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 4013132709c098f8 0000000000000000 0000000000000000 0000000000000000 0000000000000000 c054c16d69fb5a8c 413008e8b8999c02 bff6e3a01d2c4b3b c008000000000000 bf81df46a2529d38 3f80383390650966 0000000000000000 bf03dc1600fac6e8 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 7ff0000000000000 7ff0000000000000
jpseudo 8 1 | -1 0 0 0 -1 3e41650cf06be1de be001bcb10649b4b 0 0 0 1 -1 c005d56dfe4c645a bff0b6df80afab74 1 0 0 2 -1 c010000000000000 bff68f66289081da 2 0 0 3 -1 bff630efb1769713 bfc8f7ba39859fe8 3 0 0 4 -1 bff4c09767a9d04e 4000f78840d93043 4 0 0 5 -1 8000000000000000 3ff99aa458d4a793 1 0 0 6 -1 c006caf7361a563b bff8a8cdff8c791a 6 1 0 -1 0 bf77e7bd5b18f000 3fdad58644775df4 | c0e664ec47278892 3e0aee7740fdbd86 | | bfdad58644f83c4d bf77e7bf87ba8e0d bff76c4111cd82f1 4005c97a1f9ed7e2 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 bfff5e2f90aa5097 4006bf03576cc9c3 c0e664ec17580ddc bfdad586439fea3a 3f81df46a2529d39 bf72358348fb4f53 0000000000000000 0000000000000000 0000000000000000 0000000000000000 3f7183b7ac350e5f 0000000000000000 0000000000000000 7ff0000000000000
jpseudo 5 2 | -1 0 0 0 -1 c0a34534c5bbfab5 bfe306a16861ef04 0 0 0 1 -1 4010000000000000 c004abcfa6c73ba5 1 0 0 2 -1 3ff46be26ac5c0b5 c008000000000000 2 1 0 -1 1 0000000000000000 bfe833313bace94e 2 1 0 -1 0 be41e5782f141434 c00783536bd745a0 | bfb044c55a4af530 3ff7dd2491e8da70 c004e65b04bb2640 bfe2ee3dd469d7d2 | | 406d4e9b4bba1b6c 410e1c2274f547e0 3fc4b23f4d2be928 40a34534c5bbfab5 4041c2f78fa43e61 c079000000dfb45e bffd3e06afb802a3 c010000000000000 c01859b4eff466c0 c05fe891ca53ce94 c001f333b114c5ac bff46be26ac5c0b5 c0196b7425280788 407bb0fb753f23b9 c004e65b04bb2640 3fc513cd9d0c45f0 3f529a8c55ccc9c4 3f7d1b583b52e659 3f81df46a2529d39 0000000000000000 0000000000000000 0000000000000000 0000000000000000 7ff0000000000000 7ff0000000000000
jpseudo 6 2 | -1 0 0 0 -1 8000000000000000 c0057d9caabc044c 0 0 0 1 -1 3fe77feb7b4fa38a be45076d77ad2457 1 0 0 2 -1 0000000000000000 bff1385a5769009b 2 1 0 -1 0 3fcc28802971d0c0 3ff0000000000000 0 0 0 3 -1 3ffc4f4b83b6bf94 bfd5f17ccdbddde8 4 1 0 -1 1 be418f4502911366 bfd9df8bffd07d48 | bfcc66ee85b73300 bfef30522445ab6a 3ff6a19aa412037e bfe921e861838a40 | | c0770a226562e35b 4035ffa42060eb16 c00241ab2ac1f4a3 be418f4502911366 c0590000041b7361 c049b80de07bf9fd 0000000000000000 0000000000000000 c069f406944a087a 4035ffa42060eb16 0000000000000000 0000000000000000 0000000000000000 0000000000000000 3faf70799094fb00 bffc4f4b85e8a834 c04618073c6c0577 c068aee0162b36f5 3ff6a19aa643ec1e bfd86444c3369738 bf4e84962c77357c 3f81df46a2529d39 bf605a8cd0f9802b 3ee77bedb5722284 0000000000000000 0000000000000000 0000000000000000 0000000000000000 7ff0000000000000 7ff0000000000000
jpseudo 4 1 | -1 0 0 0 -1 bff55ce09d987e92 bff01951e05a251f 0 0 1 1 -1 bfafbeda581f4bc0 bff01dbd1c16557e 1 1 0 -1 0 bfb3c3aafd96fdd0 c0bbcdc9574b4729 1 0 0 2 -1 3ffc8e5145b0d6a0 3ff883636f9a228c | bff0000000000000 0000000000000000 | | 40bbccc7c22d4187 3ff420a5edbf0eb5 0000000000000000 0000000000000000 0000000000000000 0000000000000000 bfed878aa04d2046 40bbcdc9574b4729 3f08b4889d58baeb 0000000000000000 0000000000000000 0000000000000000 0000000000000000 7ff0000000000000
jpseudo 8 1 | -1 0 0 0 -1 0000000000000000 3f7053dc00a7f900 0 0 1 1 -1 c006c58d66722d06 bff30386dd9b4da8 0 0 0 2 -1 bff0000000000000 3fe13bbb1b422bd2 1 0 0 3 -1 4001c349ecca86ec 3ff0000000000000 1 0 1 4 -1 bfda9ab02a5de668 c005bd7ce9910230 4 0 0 5 -1 4008000000000000 c000cb6fadcdc35c 2 0 0 6 -1 4000ad6b2664e563 c0ef0aaba96cbc00 5 1 0 -1 0 c005979c3221a27b 0000000000000000 | c0e6af23a9833559 be4504564db6518a | | 3f7053dc00a7f900 c005979c3221a27b 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 c000cb6fadcdc35c c016cbce1910d13e 0000000000000000 0000000000000000 c0e6aecd4b126cd2 be4504564db6518a bf81df46a2529d39 3c41db273e6ff232 0000000000000000 0000000000000000 0000000000000000 3f70eda69d60f599 0000000000000000 0000000000000000 0000000000000000 7ff0000000000000
jpseudo 4 2 | -1 0 0 0 -1 0000000000000000 3fb341c55e749220 0 0 0 1 -1 bfed1b36965cc35c be42203317c868e9 1 1 0 -1 0 bfed3898dd4f9bd6 be2eea336371bcb1 1 1 0 -1 1 3ff6c8a4c6b13a84 3e2eda27e09d94e1 | 4006efb30af5074c c000000000000000 0000000000000000 bff5cb7de3362fa0 | | 401e16c47bbd1c78 c056d4376ce631bf 3fb341c54f077e30 3ff6c8a4c6b13a84 bea03ed3c250b637 bfd6f4c76db91f50 be49d6bd0fefce21 4002ab2008efce19 4077a051bbc8fa24 c068ffffff3ec83f bff6c8a4c6b13a84 bff5cb7de42d00df 3f81df46a2529d39 bf1de2670c624d01 0000000000000000 0000000000000000 0000000000000000 0000000000000000 7ff0000000000000 7ff0000000000000
jpseudo 3 1 | -1 0 0 0 -1 0000000000000000 bfd77200918612dc 0 0 0 1 -1 c008000000000000 3e3fc6c07ce7957b 1 1 0 -1 0 3ff46c308fb90c6c 3ffbaaa8e4b256d3 | 0000000000000000 bfef921c65258efa | | c000c3948489edc5 3ff46c308fb90c6c bffbaaa8e2b5eacb 40111b0c23ee431b bff46c308fb90c6c c005b9db8ba28f28 3f81df46a2529d39 bf79c4654d349324 0000000000000000 0000000000000000 7ff0000000000000
jpseudo 7 2 | -1 0 0 0 -1 0000000000000000 bfc2781dbee3d990 0 0 1 1 -1 3fc68591f34ea458 3fad1aa4211c7280 1 0 1 2 -1 c008000000000000 0000000000000000 2 1 0 -1 0 c0f785d871f42dda 40066a9a9cc32817 2 0 0 3 -1 c007edd0f340f90e bfc76b54c1dfbb20 4 0 0 4 -1 c00312302cb070be 4008000000000000 0 1 0 -1 1 3e441f5bb2cc8e59 c004743294b9b7fc | 40033cf7bf77ab92 40057ee431a21596 4010000000000000 bfec6630028688d8 | | c0726a263e4a9772 c16260911906c3d2 40034cb0b8cb7a63 3e441f5bb2cc8e59 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 416260af2849defd c02704d0763ace99 400ffffffebe0a45 3ffab54d28302b8c 3f81df46a2529d39 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 7ff0000000000000 7ff0000000000000
jdls 6 1 | -1 0 0 0 -1 8000000000000000 bfff2c9d1bcdf847 0 0 0 1 -1 0000000000000000 0000000000000000 1 0 0 2 -1 bff97e9ccc1a3975 3e3d25fef5d4052b 2 0 0 3 -1 bffcd96ebb7f69ba bfea1a0f3bb8f5ae 1 0 0 4 -1 c007976420ef0cd0 3e26155dbbc34661 3 1 0 -1 0 0000000000000000 c0e09da4b0fa078c | 3fe17e6549d80fa0 bff90ea62acb0039 | | 40e09d6657bfcff0 0000000000000000 40e09da4b0fa078c 8000000000000000 40e09da4b0fa0b31 3ff97e9ccc1a3975 40e09d8a96eacbd3 3ffcd96ebb7f69ba 0000000000000000 0000000000000000 3fe17e6549d80fa0 40e09d7293adb1f6 bf4973c913c1de49 bf49742896c535b4 3f464f4e12528af9 3f4c987103b761f4 0000000000000000 bfca67e37ccb26f5 40c0259756d94642 7ff0000000000000 41f1412cab9d9c2b 40fc37a5e3fdb33c 40fc37a5e3fdb33c 401c0012cf7f78bb
jdls 4 1 | -1 0 0 0 -1 c000000000000000 4010000000000000 0 0 0 1 -1 bfe21a4fdc62eece bfe00e4b4691020e 1 0 1 2 -1 0000000000000000 bfe7077fc7e4364c 2 1 0 -1 0 40c996a4134b2788 40f3a87d2fb6f51f | 3fd9920adfd60ea0 c000774b86a8d5e8 | | c0f3a83d2fb6f51f 40c997a4134b2788 c0f3a88536dc9868 40c996ec7c8a9914 0000000000000000 0000000000000000 c0c99670ef3567dc c0f3a89e1e4e0271 bf4c987103b761f4 3f4c98103bbbc31a 0000000000000000 c0ad45ce097efd34 c0d67c614e01cdc6 7ff0000000000000 420826d2d5ad820b c1df70f5c2b9d8fe c1df70f5c2b9d8fe 41b4772c25660fb1
jdls 5 1 | -1 0 0 0 -1 c002c616cb662b65 3fd321a5d6fb4ac0 0 0 0 1 -1 bdf960f24eaa8ad9 bded30b23c906d4b 1 1 0 -1 0 4000ad5a3bda9152 be3b9366960e34cc 0 0 0 2 -1 3fd7f567e6ef3b94 3fcefae9cf1038d8 1 0 0 3 -1 be3d9240cb986ccc c00333ea4123a010 | c008000000000000 bfb5002c0abdd260 | | 3fd321a5dde02466 4011b9b883a05e5c 3e3aa9e10429b162 4000ad5a3be741cb 0000000000000000 0000000000000000 0000000000000000 0000000000000000 c01456ad1ded48a9 bfb5002bef2a6bca bf426269880f8c61 3f4c987103b761f5 0000000000000000 0000000000000000 c010867d6388eec4 3fcb637ff911fe8a 7ff0000000000000 3ff4ca2c00e08051 3ff531d1ed63a5c1 3ff531d1ed63a5c1 40393159148b5977
jdls 8 2 | -1 0 1 0 -1 4000000000000000 3fc1b8157c5d1868 0 0 1 1 -1 3e2a80b7789fbf9e 0000000000000000 1 1 0 -1 0 be3a86b39707a50b 4004133a92f09df4 0 0 0 2 -1 3fdbe8baf5d25d18 c000000000000000 3 0 0 3 -1 c0037777e8926631 40038afc81aaf916 4 0 0 4 -1 3ff9a94d3785f7ad 4000edb6b7ff62be 5 0 0 5 -1 bff1621c193663ff 3ffd19d307dc8d8d 6 1 0 -1 1 0000000000000000 0000000000000000 | c004bd559cc1c473 bfc4c867e6d37780 3ffceda198005020 4005005a1b586c23 | | 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 c000000000000000 bfdbe8baf5d25d18 0000000000000000 0000000000000000 40038afc81aaf916 40037777e8926631 0000000000000000 0000000000000000 4000edb6b7ff62be bff9a94d3785f7ad 0000000000000000 0000000000000000 3ffd19d307dc8d8d 3ff1621c193663ff c07033eae1d19797 c070b2ced5914ebc 3ffceda198005020 4005005a1b586c23 0000000000000000 0000000000000000 bf2ddba5b538ef0e 3f4c987103b761f5 bf3a886df17a2516 3f3bb7534b91f713 c06ac80fdf0a095e c06b99cc5aa83e6f 3fa08f48a66dea64 3fcc5341b702462d 7ff0000000000000 7ff0000000000000 3ff35c28f5c28f5d 0000000000000000 0000000000000000 0000000000000000 0000000000000000 3ff35c28f5c28f5d 0000000000000000 0000000000000000 0000000000000000 0000000000000000 4032f6a10726a8d1 40159840be654499 0000000000000000 0000000000000000 40159840be654499 402625dbaa660ba8
jdls 7 1 | -1 0 0 0 -1 4000000000000000 40e5ee8d6d967a16 0 0 1 1 -1 c004c0f6416134aa 0000000000000000 1 0 0 2 -1 c0066251f75314a3 8000000000000000 2 0 0 3 -1 4001a2bc222df902 0000000000000000 2 0 0 4 -1 0000000000000000 bfe6754c2c82084a 4 0 1 5 -1 0000000000000000 400253bbd8e9f86e 5 1 0 -1 0 0000000000000000 0000000000000000 | c008000000000000 4001838535d1967c | | 40e5ee8d6d967a16 c000000000000000 0000000000000000 0000000000000000 8000000000000000 40066251f75314a3 0000000000000000 0000000000000000 bfe6754c2c82084a 8000000000000000 0000000000000000 0000000000000000 c008000000000000 4001838535d1967c be7716d0f8b68057 0000000000000000 3f4c987103b761f5 0000000000000000 be44ed7310df4169 0000000000000000 3ee69cbed25955f6 3fcefff60f2debf8 7ff0000000000000 41de1017f4765226 c0f5ee8d6d967a16 c0f5ee8d6d967a16 402a13e3e9a75eb6
jdlssvd 4 1 | -1 0 0 0 -1 be3a1bc0ce732313 bff9285f2d7e3ac1 0 0 0 1 -1 3ff0e831b107200c bffb9fa6c2aff155 1 0 0 2 -1 0000000000000000 4010000000000000 2 1 0 -1 0 bfe4ea2998df9572 3fe8c9c93ec405d0 | 40eb204d801bccca c0f476d2f1e23346 | | c002c6a1e6701ed4 bfe4ea29959c1d58 c0040245b108fa1e bffb5d467d76eac5 4009cd8db04efe8c bfe4ea2998df9572 40eb20626a4565aa c0f476df56c6d2a8 3f4f20c82ed0b034 3fae24fceb95eb2d 3fb657184ae74487 0000000000000000 0000000000000000 7ff0000000000000
jdlssvd 6 1 | -1 0 1 0 -1 40e6c48cef581500 bff687374383316c 0 1 0 -1 0 3fdb10c10b839ed0 be4467b063eae6f5 0 0 0 1 -1 0000000000000000 3ff9b1d086c31541 2 0 0 2 -1 0000000000000000 3fe92ef4bad5409e 3 0 0 3 -1 be4225e3a94b28d7 3ffda993235162e9 4 0 0 4 -1 0000000000000000 bff3935a68a8dca7 | c0cc460b6287a176 bfdf8f2e026f9a38 | | 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 c0cc46418409b87d bfdf8f2df83bc206 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 7ff0000000000000
jdlssvd 5 1 | -1 0 0 0 -1 4005012ccfe6f967 bfdd8aa1a481ceac 0 0 0 1 -1 4008000000000000 3ff63542ebfeea96 1 0 0 2 -1 8000000000000000 8000000000000000 2 0 0 3 -1 be43f8433943eb62 c000550ee61dcbc5 2 1 0 -1 0 40f1f03fc8bd13bf bff0e7ffbdd3aae0 | be44deb611502113 c000000000000000 | | 3fe30aaea9666e6a 40f1f015c66373f1 40038ea154e94abb 40f1f00fc8bd13bf 3ff0e7ffbdd3aae0 40f1f03fc8bd13bf 0000000000000000 0000000000000000 c0f1f03fc8bd165b bfee30008458aa40 3fafe4d14a9cf176 bfb657184ae74487 3f999264d9d483be 0000000000000000 0000000000000000 0000000000000000 7ff0000000000000
jdlssvd 5 1 | -1 0 0 0 -1 be41989cfcf3fce8 bfea064da8ae8e7c 0 0 0 1 -1 c06d343e7ebcfd91 0000000000000000 1 0 0 2 -1 0000000000000000 be35e30e62fee8b6 0 0 0 3 -1 3fd0e915d0cfc968 c0b0f746ae1ba26d 2 1 0 -1 0 c0e05e4de5c93c5f 3feb89a995bb3708 | 40cafa4b7aeffe92 3ff542267bafc3a2 | | bffac7fb9f34e2c2 c0e05e4de5c937f9 bfeb89a995bb3708 c0e04119a74a7f61 bfeb89a9987798d4 c0e05e4de5c93c5f 0000000000000000 0000000000000000 40e71ce0c4853c04 3fddf546c348a078 bfb657184ae74487 3fa616dcfbbd17a8 3fa6bebcde6f12b5 0000000000000000 0000000000000000 0000000000000000 7ff0000000000000
jdlssvd 7 1 | -1 0 0 0 -1 c005e313ae09bfd3 bfc1bf963b190c50 0 0 1 1 -1 c00346cad4cb0b4a bfed0fe19d73653c 1 0 0 2 -1 bfde883cd34c8238 0000000000000000 0 0 1 3 -1 c001cac594da32b3 0000000000000000 3 0 1 4 -1 bff858d1f39e6211 3fd7bdef778a93cc 4 0 0 5 -1 0000000000000000 c0043264f4f4ef4a 5 1 0 -1 0 c001e5629feeca6a c003448ad726aae8 | 40f689e07196d766 40ddfae2afe02c2f | | 4002289173751a23 3fdfed8870d7ab48 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 bfbdbb43b9c88c40 c001e5629feeca6a 40f68a043c5c1744 40ddfb7cd436e564 3fb657184ae74487 bc448dd9e8f15ed2 0000000000000000 0000000000000000 0000000000000000 bf8db9c0e481c3c2 0000000000000000 0000000000000000 7ff0000000000000
jclamp 8 2 | -1 0 1 0 -1 3e0028358bd678b3 c0027e40fe70e980 0 1 0 -1 0 40c809d33f82c687 40042897ad0a5fc6 0 0 0 1 -1 3e4257cf03102681 0000000000000000 2 0 0 2 -1 3ff923a49994663f c008000000000000 3 0 0 3 -1 0000000000000000 4000000000000000 4 0 0 4 -1 3fd85452027fd0a0 bff335e57de9324d 5 0 0 5 -1 3ffa8224c6f5e9e6 4007a7298ff03aaa 6 1 0 -1 1 4008000000000000 bfc03f8157d72130 | c0f27b7f33e6aa8d 8000000000000000 c006c448543039f4 be2d60b466cdd596 | 3e26664f61048500 7ff0000000000000 | 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 3fc03f8157d72130 4007fffffeda8310 0000000000000000 0000000000000000 c006fc07ea828ded 3ff6dc5b666b99c1 0000000000000000 0000000000000000 400103f8157d7213 4008000000000000 0000000000000000 0000000000000000 bff12df552ee4e27 4004f575bfb005ec 0000000000000000 0000000000000000 4008ab21a56dacbd 3ff57ddb390a161a c160c97101bffaa1 c06f7f6cfe6035a5 c01762242a181cfa 3fc03f81507ef416 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 be26664f60dd16d6 bd3503bbe7082957 c01762242a181cfa 3fc03f81507ef416 3e26664f61048500 7ff0000000000000
jclamp 5 1 | -1 0 0 0 -1 3e3a60ec41342640 8000000000000000 0 0 0 1 -1 bff2054c8d378af2 c0e829e43da8492d 1 0 0 2 -1 3fe0cf61e5d32a8c 3ff72bbd425087cd 2 1 0 -1 0 bffbe3a62e2cc780 c002219f8654c694 2 0 0 3 -1 bff0000000000000 3ff9118187ea153f | c00739369f0f979c 40e5ffa84312e385 | 3ffc8dd556a83b13 | 4002219f8654c694 bffbe3a62fd2d644 c0e8299bb72a2fda bfe3bcb341ea791c 400db77e277d0a7a c00225ab908b2e63 0000000000000000 0000000000000000 bff28ec70ff267b8 40e5fff0c990fcd8 0000000000000000 0000000000000000 0000000000000000 0000000000000000 bf0816267e7a4948 3ffc8dd5567f9843 3ffc8dd556a83b13
jupdclamp 4 1 | -1 0 0 0 -1 0000000000000000 c008000000000000 0 0 0 1 -1 0000000000000000 40079c8bdacf7488 1 1 0 -1 0 3fed3b5cb4942cd2 3e38ae10bad2cdb8 0 0 0 2 -1 3e385c0b195033c7 3ff60dafb58a323f | bfad9086eb02b4e0 bff2b948dc3f9a80 | 3ff41d47cfdcad36 | c008000000c57086 3fed3b5cb4942cd2 40079c8bda0a0402 3fed3b5cb4942cd2 0000000000000000 0000000000000000 bfef146523445820 bff2b948ddca7b8c 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 400b333333333333
jupdclamp 4 2 | -1 0 0 0 -1 8000000000000000 c0de9d682ca788ee 0 0 0 1 -1 c004561e79f7ad5a 3ff3c66fb169501b 1 1 0 -1 0 3fd259370d968be8 0000000000000000 1 1 0 -1 1 3e3c09c669c81c92 3ffe127445dcfa77 | 3ffa4a8b09bfad2a 0000000000000000 c0077a17a4140947 0000000000000000 | 7ff0000000000000 40f4999a23aaca9e | c147eaf962e2e2fa 403cab66053b3a9a c0de9de07678a062 3e3c09c669c81c92 405ee60e85348d2a 4071adfe2f9d3318 bfe4980928e754b8 4004561e7ad7fb8d 4060f4cfdef657f6 0000000000000000 c0077a17a4f4577a bffe127445dcfa77 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 400b333333333333 400f9a2d518352ab
ang 5 0 | be40dbb9d5f57274 400156c820f2b02c be0e1c5be193229d bff9cb89450c69e5 4008000000000000 c0006a8c8e0641b9 c000d8e6429b7351 c005bcceafab9d87 8000000000000000 40004942a9f358e8 | bff921fb53c5609d
ang 5 1 | be40dbb9d5f57274 400156c820f2b02c be0e1c5be193229d bff9cb89450c69e5 4008000000000000 c0006a8c8e0641b9 c000d8e6429b7351 c005bcceafab9d87 8000000000000000 40004942a9f358e8 | 3ff6cdb58d145271
ang 5 2 | be40dbb9d5f57274 400156c820f2b02c be0e1c5be193229d bff9cb89450c69e5 4008000000000000 c0006a8c8e0641b9 c000d8e6429b7351 c005bcceafab9d87 8000000000000000 40004942a9f358e8 | c006ee8a08423b31
ang 5 3 | be40dbb9d5f57274 400156c820f2b02c be0e1c5be193229d bff9cb89450c69e5 4008000000000000 c0006a8c8e0641b9 c000d8e6429b7351 c005bcceafab9d87 8000000000000000 40004942a9f358e8 | c000f07931c560b9
ang 7 0 | 3ff9282f90d75be6 bff92355846a4a32 be4326c17b9ef96f 400403a477dbe244 3fff4de650815c42 0000000000000000 c0e6deefefa68970 bff8997438f5cb43 0000000000000000 c0f6c251188747aa be0cf6fa286bb51a c000000000000000 0000000000000000 be1d13e13800035b | 3fff0703341ea123
ang 7 1 | 3ff9282f90d75be6 bff92355846a4a32 be4326c17b9ef96f 400403a477dbe244 3fff4de650815c42 0000000000000000 c0e6deefefa68970 bff8997438f5cb43 0000000000000000 c0f6c251188747aa be0cf6fa286bb51a c000000000000000 0000000000000000 be1d13e13800035b | c006c53d340a67b2
ang 7 2 | 3ff9282f90d75be6 bff92355846a4a32 be4326c17b9ef96f 400403a477dbe244 3fff4de650815c42 0000000000000000 c0e6deefefa68970 bff8997438f5cb43 0000000000000000 c0f6c251188747aa be0cf6fa286bb51a c000000000000000 0000000000000000 be1d13e13800035b | c001e02e84e4b71f
ang 7 3 | 3ff9282f90d75be6 bff92355846a4a32 be4326c17b9ef96f 400403a477dbe244 3fff4de650815c42 0000000000000000 c0e6deefefa68970 bff8997438f5cb43 0000000000000000 c0f6c251188747aa be0cf6fa286bb51a c000000000000000 0000000000000000 be1d13e13800035b | 40004a81f80a41e9
ang 7 4 | 3ff9282f90d75be6 bff92355846a4a32 be4326c17b9ef96f 400403a477dbe244 3fff4de650815c42 0000000000000000 c0e6deefefa68970 bff8997438f5cb43 0000000000000000 c0f6c251188747aa be0cf6fa286bb51a c000000000000000 0000000000000000 be1d13e13800035b | 40056865d0f7a2f7
ang 7 5 | 3ff9282f90d75be6 bff92355846a4a32 be4326c17b9ef96f 400403a477dbe244 3fff4de650815c42 0000000000000000 c0e6deefefa68970 bff8997438f5cb43 0000000000000000 c0f6c251188747aa be0cf6fa286bb51a c000000000000000 0000000000000000 be1d13e13800035b | bdfcf722e2c42e5d
ang 5 0 | 3fb07a7630265010 bff7bdf9dcd84320 bff33515ea88a655 bffb30e6894b1472 4008000000000000 3e215ecacd1e7d6f 3fe958dbfb09cfce 3fcc87bc327bc748 3ffa9191c4487ce7 40001e36b8eeca44 | c007c846237ec9bb
ang 5 1 | 3fb07a7630265010 bff7bdf9dcd84320 bff33515ea88a655 bffb30e6894b1472 4008000000000000 3e215ecacd1e7d6f 3fe958dbfb09cfce 3fcc87bc327bc748 3ffa9191c4487ce7 40001e36b8eeca44 | c00768551428431a
ang 5 2 | 3fb07a7630265010 bff7bdf9dcd84320 bff33515ea88a655 bffb30e6894b1472 4008000000000000 3e215ecacd1e7d6f 3fe958dbfb09cfce 3fcc87bc327bc748 3ffa9191c4487ce7 40001e36b8eeca44 | 40054092a7a20683
ang 5 3 | 3fb07a7630265010 bff7bdf9dcd84320 bff33515ea88a655 bffb30e6894b1472 4008000000000000 3e215ecacd1e7d6f 3fe958dbfb09cfce 3fcc87bc327bc748 3ffa9191c4487ce7 40001e36b8eeca44 | bffebe67548fb1ef
ang 5 0 | 4004f5e296addc5a 4000000000000000 4004f5e296addc5a 4000000000000000 bfcd70b3b8f69df8 c0000030b2cb9016 be1d9d08c6c04184 40d93f298a0a4eac 3ffa0fdf134ae7ea c00108924d195bca | 0000000000000000
ang 5 2 | 4004f5e296addc5a 4000000000000000 4004f5e296addc5a 4000000000000000 bfcd70b3b8f69df8 c0000030b2cb9016 be1d9d08c6c04184 40d93f298a0a4eac 3ffa0fdf134ae7ea c00108924d195bca | c0042e275e181d31
ang 5 3 | 4004f5e296addc5a 4000000000000000 4004f5e296addc5a 4000000000000000 bfcd70b3b8f69df8 c0000030b2cb9016 be1d9d08c6c04184 40d93f298a0a4eac 3ffa0fdf134ae7ea c00108924d195bca | c00921d5a26e2ff0
ang 5 0 | bfeb10a8c69361b8 c002432bf4a9a318 40cfca899c20c5f3 4003b5e244016d6e 0000000000000000 3ff3556a2ee4f56c bfe92ba97161fc84 8000000000000000 bfc889e0a69188f8 4007397cf5ea2cca | 3f331c33d5d195c5
ang 5 1 | bfeb10a8c69361b8 c002432bf4a9a318 40cfca899c20c5f3 4003b5e244016d6e 0000000000000000 3ff3556a2ee4f56c bfe92ba97161fc84 8000000000000000 bfc889e0a69188f8 4007397cf5ea2cca | 4009218ae2e9bca9
ang 5 2 | bfeb10a8c69361b8 c002432bf4a9a318 40cfca899c20c5f3 4003b5e244016d6e 0000000000000000 3ff3556a2ee4f56c bfe92ba97161fc84 8000000000000000 bfc889e0a69188f8 4007397cf5ea2cca | 3fefcc2fb62220eb
ang 5 3 | bfeb10a8c69361b8 c002432bf4a9a318 40cfca899c20c5f3 4003b5e244016d6e 0000000000000000 3ff3556a2ee4f56c bfe92ba97161fc84 8000000000000000 bfc889e0a69188f8 4007397cf5ea2cca | c006221fbc37f396
ang 5 0 | 3ffcc1c1941bd0d9 bfba2b0aa3635890 3ff24cd406bd7a55 c010000000000000 bfe3e379ef474330 c007aa9eaea47699 bfd6bdf46d80c2bc 3e437659cf9586ec 4007b19c328bcdd8 be3713cc14c43529 | bffbca6f4c9ac8cd
ang 5 1 | 3ffcc1c1941bd0d9 bfba2b0aa3635890 3ff24cd406bd7a55 c010000000000000 bfe3e379ef474330 c007aa9eaea47699 bfd6bdf46d80c2bc 3e437659cf9586ec 4007b19c328bcdd8 be3713cc14c43529 | bfff01243e40fcbe
ang 5 2 | 3ffcc1c1941bd0d9 bfba2b0aa3635890 3ff24cd406bd7a55 c010000000000000 bfe3e379ef474330 c007aa9eaea47699 bfd6bdf46d80c2bc 3e437659cf9586ec 4007b19c328bcdd8 be3713cc14c43529 | bff209eac727d726
ang 5 3 | 3ffcc1c1941bd0d9 bfba2b0aa3635890 3ff24cd406bd7a55 c010000000000000 bfe3e379ef474330 c007aa9eaea47699 bfd6bdf46d80c2bc 3e437659cf9586ec 4007b19c328bcdd8 be3713cc14c43529 | bff7b26f00382948
ang 3 0 | 3fe8afadc1e216c0 40d418079d5ddd5c 0000000000000000 bfff06a1ad389f89 0000000000000000 bffaa4febf5be991 | bff92222a391bb75
ang 3 1 | 3fe8afadc1e216c0 40d418079d5ddd5c 0000000000000000 bfff06a1ad389f89 0000000000000000 bffaa4febf5be991 | c00921e7ac9d65ea
ang 4 0 | 3ff338898f692d11 0000000000000000 3ff338898f692d11 0000000000000000 3fc968d6097351e8 400243712fa87372 c000000000000000 40f81eeb5ae61276 | 0000000000000000
ang 4 2 | 3ff338898f692d11 0000000000000000 3ff338898f692d11 0000000000000000 3fc968d6097351e8 400243712fa87372 c000000000000000 40f81eeb5ae61276 | bfda7cb3af14efda
ang 4 0 | 3fd71b67dcfeb454 4008000000000000 0000000000000000 be2150f63be32a01 4007bcb7f7b6402e bfdcea4eae696614 3fc956424b4d57a0 0000000000000000 | bffb0c929f881ac7
ang 4 1 | 3fd71b67dcfeb454 4008000000000000 0000000000000000 be2150f63be32a01 4007bcb7f7b6402e bfdcea4eae696614 3fc956424b4d57a0 0000000000000000 | 3ff8a1a35e3a3267
ang 4 2 | 3fd71b67dcfeb454 4008000000000000 0000000000000000 be2150f63be32a01 4007bcb7f7b6402e bfdcea4eae696614 3fc956424b4d57a0 0000000000000000 | 40090c3b659aa96a
ang 6 0 | 3fc076b68c9f3e40 3fe166c1c2c2c54e 8000000000000000 bfe98af795511514 3fe03cc820ae3b32 bfae075ea9e82900 3ff295268fd3523d 0000000000000000 bff3bac72c708923 4000000000000000 4008000000000000 3fe237902d6e1d76 | bffaa95c9a094402
ang 6 1 | 3fc076b68c9f3e40 3fe166c1c2c2c54e 8000000000000000 bfe98af795511514 3fe03cc820ae3b32 bfae075ea9e82900 3ff295268fd3523d 0000000000000000 bff3bac72c708923 4000000000000000 4008000000000000 3fe237902d6e1d76 | 400516204e3042f1
ang 6 2 | 3fc076b68c9f3e40 3fe166c1c2c2c54e 8000000000000000 bfe98af795511514 3fe03cc820ae3b32 bfae075ea9e82900 3ff295268fd3523d 0000000000000000 bff3bac72c708923 4000000000000000 4008000000000000 3fe237902d6e1d76 | bfec2915dfdfd3c0
ang 6 3 | 3fc076b68c9f3e40 3fe166c1c2c2c54e 8000000000000000 bfe98af795511514 3fe03cc820ae3b32 bfae075ea9e82900 3ff295268fd3523d 0000000000000000 bff3bac72c708923 4000000000000000 4008000000000000 3fe237902d6e1d76 | 4002d9ada4beecfa
ang 6 4 | 3fc076b68c9f3e40 3fe166c1c2c2c54e 8000000000000000 bfe98af795511514 3fe03cc820ae3b32 bfae075ea9e82900 3ff295268fd3523d 0000000000000000 bff3bac72c708923 4000000000000000 4008000000000000 3fe237902d6e1d76 | c0062c5b38a557f6
ang 6 0 | c0e6ab7f22671c52 bffe90db8774171a 3ff0000000000000 c003103691397b60 bfe26a09b9a5de36 0000000000000000 c0ee581629429606 bff9f56b720da815 3e43a3e0af8840f0 bfcb3f36d322b9f8 c002ba1e4b03068e 4005e6827dc2f1b5 | bee5583c70ea90ce
ang 6 1 | c0e6ab7f22671c52 bffe90db8774171a 3ff0000000000000 c003103691397b60 bfe26a09b9a5de36 0000000000000000 c0ee581629429606 bff9f56b720da815 3e43a3e0af8840f0 bfcb3f36d322b9f8 c002ba1e4b03068e 4005e6827dc2f1b5 | 40013d62fc1b2d7a
ang 6 2 | c0e6ab7f22671c52 bffe90db8774171a 3ff0000000000000 c003103691397b60 bfe26a09b9a5de36 0000000000000000 c0ee581629429606 bff9f56b720da815 3e43a3e0af8840f0 bfcb3f36d322b9f8 c002ba1e4b03068e 4005e6827dc2f1b5 | 3fef92ad79246f16
ang 6 3 | c0e6ab7f22671c52 bffe90db8774171a 3ff0000000000000 c003103691397b60 bfe26a09b9a5de36 0000000000000000 c0ee581629429606 bff9f56b720da815 3e43a3e0af8840f0 bfcb3f36d322b9f8 c002ba1e4b03068e 4005e6827dc2f1b5 | 400921f9887e1da2
ang 6 4 | c0e6ab7f22671c52 bffe90db8774171a 3ff0000000000000 c003103691397b60 bfe26a09b9a5de36 0000000000000000 c0ee581629429606 bff9f56b720da815 3e43a3e0af8840f0 bfcb3f36d322b9f8 c002ba1e4b03068e 4005e6827dc2f1b5 | 4001ee8d362f01b9
ang 5 0 | 4008000000000000 0000000000000000 4008000000000000 0000000000000000 4002ab1a23a7ddaa 3fd1ff515b752934 3fe9769aa404654c 40e0ba704d091954 40f3a4d53b406f25 c003d377267fdfe0 | 0000000000000000
ang 5 2 | 4008000000000000 0000000000000000 4008000000000000 0000000000000000 4002ab1a23a7ddaa 3fd1ff515b752934 3fe9769aa404654c 40e0ba704d091954 40f3a4d53b406f25 c003d377267fdfe0 | bff2be56c1053aac
ang 5 3 | 4008000000000000 0000000000000000 4008000000000000 0000000000000000 4002ab1a23a7ddaa 3fd1ff515b752934 3fe9769aa404654c 40e0ba704d091954 40f3a4d53b406f25 c003d377267fdfe0 | bfff93158510acd8
"#;
    // END GOLDEN

    fn hexv(s: &str) -> Vec<f64> {
        s.split_whitespace().map(|t| f64::from_bits(u64::from_str_radix(t, 16).unwrap())).collect()
    }
    fn same(a: f64, b: f64) -> bool {
        (a.is_nan() && b.is_nan()) || a.to_bits() == b.to_bits()
    }
    fn assert_same(ctx: &str, got: &[f64], want: &[f64]) {
        assert_eq!(got.len(), want.len(), "{ctx}: length");
        for (i, (g, w)) in got.iter().zip(want).enumerate() {
            assert!(same(*g, *w), "{ctx}: element {i}: got {:016x} ({g}) want {:016x} ({w})", g.to_bits(), w.to_bits());
        }
    }
    fn ok(rc: i32) {
        assert_eq!(rc, 0, "status");
    }

    /// Mirror of the header's `Jacobian` members: sizes as in its constructor, gather as in
    /// `computeJacobian`, then the requested solver.
    struct Skel {
        parent: Vec<i64>,
        purpose: Vec<i64>,
        frozen: Vec<i64>,
        joint_num: Vec<i64>,
        eff_num: Vec<i64>,
        s: Vec<(f64, f64)>,
        target: Vec<f64>,
    }
    struct Jac {
        n_eff: i64,
        n_row: i64,
        n_col: i64,
        jend: Vec<f64>,
        u: Vec<f64>,
        w: Vec<f64>,
        v: Vec<f64>,
        ds: Vec<f64>,
        dt: Vec<f64>,
        dtheta: Vec<f64>,
        dpre: Vec<f64>,
        dsclamp: Vec<f64>,
        damp_sq_v: Vec<f64>,
        diag_id: Vec<f64>,
        jnorms: Vec<f64>,
        lambda: f64,
        lambda_sq: f64,
    }
    impl Jac {
        fn new(sk: &Skel) -> Jac {
            let n_eff = sk.purpose.iter().filter(|&&p| p == 1).count() as i64;
            let n_row = 2 * n_eff;
            let n_col = sk.parent.len() as i64 - n_eff;
            let z = |n: i64| vec![0.0f64; n as usize];
            let mut j = Jac {
                n_eff,
                n_row,
                n_col,
                jend: z(n_row * n_col),
                u: z(n_row * n_row),
                w: z(n_row.min(n_col)),
                v: z(n_col * n_col),
                ds: z(n_row),
                dt: z(n_row),
                dtheta: z(n_col),
                dpre: z(n_col),
                dsclamp: z(n_eff),
                damp_sq_v: z(n_row),
                diag_id: z(n_col),
                jnorms: z(n_eff * n_col),
                lambda: 0.0,
                lambda_sq: 0.0,
            };
            unsafe {
                ok(flare_ikjac_reset(
                    &mut j.lambda,
                    &mut j.lambda_sq,
                    j.damp_sq_v.as_mut_ptr(),
                    n_row,
                    j.diag_id.as_mut_ptr(),
                    n_col,
                    j.dsclamp.as_mut_ptr(),
                    n_eff,
                ));
            }
            j
        }
        fn compute(&mut self, sk: &Skel) {
            let (mut en, mut es, mut enanc) = (vec![], vec![], vec![]);
            let (mut aj, mut af, mut asx) = (vec![], vec![], vec![]);
            for i in 0..sk.parent.len() {
                if sk.purpose[i] != 1 {
                    continue;
                }
                en.push(sk.eff_num[i] as i32);
                es.push(sk.s[i].0);
                es.push(sk.s[i].1);
                let mut cnt = 0usize;
                let mut m = sk.parent[i];
                while m >= 0 {
                    let m_us = m as usize;
                    aj.push(sk.joint_num[m_us] as i32);
                    af.push(sk.frozen[m_us] as u8);
                    asx.push(sk.s[m_us].0);
                    asx.push(sk.s[m_us].1);
                    cnt += 1;
                    m = sk.parent[m_us];
                }
                enanc.push(cnt);
            }
            unsafe {
                ok(flare_ikjac_compute_jacobian(
                    self.jend.as_mut_ptr(),
                    self.n_row,
                    self.n_col,
                    self.ds.as_mut_ptr(),
                    self.n_row,
                    self.n_eff,
                    en.len(),
                    en.as_ptr(),
                    es.as_ptr(),
                    enanc.as_ptr(),
                    aj.len(),
                    aj.as_ptr(),
                    af.as_ptr(),
                    asx.as_ptr(),
                    sk.target.as_ptr(),
                    sk.target.len() / 2,
                ));
            }
        }
    }

    fn parse_skel(sec: &str, tgt: &str) -> Skel {
        let t: Vec<&str> = sec.split_whitespace().collect();
        let mut sk = Skel { parent: vec![], purpose: vec![], frozen: vec![], joint_num: vec![], eff_num: vec![], s: vec![], target: hexv(tgt) };
        for c in t.chunks(7) {
            sk.parent.push(c[0].parse().unwrap());
            sk.purpose.push(c[1].parse().unwrap());
            sk.frozen.push(c[2].parse().unwrap());
            sk.joint_num.push(c[3].parse().unwrap());
            sk.eff_num.push(c[4].parse().unwrap());
            let x = f64::from_bits(u64::from_str_radix(c[5], 16).unwrap());
            let y = f64::from_bits(u64::from_str_radix(c[6], 16).unwrap());
            sk.s.push((x, y));
        }
        sk
    }

    fn ulps(a: f64, b: f64) -> u64 {
        if a.is_nan() && b.is_nan() {
            return 0;
        }
        let k = |x: f64| {
            let b = x.to_bits() as i64;
            if b < 0 { i64::MIN - b } else { b }
        };
        (k(a) as i128 - k(b) as i128).unsigned_abs() as u64
    }

    #[test]
    fn replay_recorded_cpp_cases() {
        let mut n = 0;
        let mut per_op = std::collections::BTreeMap::<String, usize>::new();
        for line in GOLDEN.lines().filter(|l| !l.trim().is_empty()) {
            let secs: Vec<&str> = line.split('|').collect();
            let head: Vec<&str> = secs[0].split_whitespace().collect();
            let name = head[0];
            let iv: Vec<i64> = head[1..].iter().filter_map(|t| t.parse().ok()).collect();
            let ctx = format!("{name} {:?}", &head[1..]);
            let inp = |k: usize| hexv(secs[k]);
            unsafe {
                match name {
                    "maxabs" => {
                        let mut v = inp(1);
                        let mut out = 0.0;
                        ok(flare_ikjac_vec_max_abs(v.as_mut_ptr(), iv[0], &mut out));
                        assert_same(&ctx, &[out], &inp(2));
                    }
                    "mulv" | "mulvt" => {
                        let (r, c) = (iv[0], iv[1]);
                        let i = inp(1);
                        let (mut a, mut v) = (i[..(r * c) as usize].to_vec(), i[(r * c) as usize..].to_vec());
                        let rn = if name == "mulv" { r } else { c };
                        let mut res = vec![0.0; rn as usize];
                        if name == "mulv" {
                            ok(flare_ikjac_multiply_vec(a.as_mut_ptr(), r, c, v.as_mut_ptr(), c, res.as_mut_ptr(), rn));
                        } else {
                            ok(flare_ikjac_multiply_transpose_vec(a.as_mut_ptr(), r, c, v.as_mut_ptr(), r, res.as_mut_ptr(), rn));
                        }
                        assert_same(&ctx, &res, &inp(2));
                    }
                    "dotcol" => {
                        let (r, c, col) = (iv[0], iv[1], iv[2]);
                        let i = inp(1);
                        let (mut a, mut v) = (i[..(r * c) as usize].to_vec(), i[(r * c) as usize..].to_vec());
                        let mut out = 0.0;
                        ok(flare_ikjac_dot_product_column(a.as_mut_ptr(), r, c, v.as_mut_ptr(), r, col, &mut out));
                        assert_same(&ctx, &[out], &inp(2));
                    }
                    "frob" => {
                        let mut a = inp(1);
                        let mut out = 0.0;
                        ok(flare_ikjac_frobenius_norm(a.as_mut_ptr(), iv[0], iv[1], &mut out));
                        assert_same(&ctx, &[out], &inp(2));
                    }
                    "setdiag" | "setsuper" | "setsub" | "adddiag" | "setdiagv" | "setsuperv" | "setsubv" | "adddiagv" => {
                        let (r, c) = (iv[0], iv[1]);
                        let mut a = inp(1);
                        let mut d = inp(2);
                        let dn = d.len() as i64;
                        let rc = match name {
                            "setdiag" => flare_ikjac_set_diagonal(a.as_mut_ptr(), r, c, d[0]),
                            "setsuper" => flare_ikjac_set_super_diagonal(a.as_mut_ptr(), r, c, d[0]),
                            "setsub" => flare_ikjac_set_sub_diagonal(a.as_mut_ptr(), r, c, d[0]),
                            "adddiag" => flare_ikjac_add_to_diagonal(a.as_mut_ptr(), r, c, d[0]),
                            "setdiagv" => flare_ikjac_set_diagonal_vec(a.as_mut_ptr(), r, c, d.as_mut_ptr(), dn),
                            "setsuperv" => flare_ikjac_set_super_diagonal_vec(a.as_mut_ptr(), r, c, d.as_mut_ptr(), dn),
                            "setsubv" => flare_ikjac_set_sub_diagonal_vec(a.as_mut_ptr(), r, c, d.as_mut_ptr(), dn),
                            _ => flare_ikjac_add_to_diagonal_vec(a.as_mut_ptr(), r, c, d.as_mut_ptr(), dn),
                        };
                        ok(rc);
                        assert_same(&ctx, &a, &inp(3));
                    }
                    "setcol" | "setrow" => {
                        let (r, c, k) = (iv[0], iv[1], iv[2]);
                        let i = inp(1);
                        let mut a = i[..(r * c) as usize].to_vec();
                        let mut d = i[(r * c) as usize..].to_vec();
                        let dn = d.len() as i64;
                        // the recorded vector is `col` (len r) for setcol and `row` (len c) for setrow
                        if name == "setcol" {
                            ok(flare_ikjac_set_column(a.as_mut_ptr(), r, c, k, d.as_mut_ptr(), dn));
                        } else {
                            ok(flare_ikjac_set_row(a.as_mut_ptr(), r, c, k, d.as_mut_ptr(), dn));
                        }
                        assert_same(&ctx, &a, &inp(2));
                    }
                    "setseq" => {
                        let (r, c, sr, sc, dr, dc, len) = (iv[0], iv[1], iv[2], iv[3], iv[4], iv[5], iv[6]);
                        let i = inp(1);
                        let mut a = i[..(r * c) as usize].to_vec();
                        let mut d = i[(r * c) as usize..].to_vec();
                        assert_eq!(d.len() as i64, len);
                        ok(flare_ikjac_set_sequence(a.as_mut_ptr(), r, c, d.as_mut_ptr(), len, sr, sc, dr, dc));
                        assert_same(&ctx, &a, &inp(2));
                    }
                    "load" | "loadt" => {
                        let (r, c, ar, ac) = (iv[0], iv[1], iv[2], iv[3]);
                        let i = inp(1);
                        let mut m = i[..(r * c) as usize].to_vec();
                        let mut a = i[(r * c) as usize..].to_vec();
                        assert_eq!(a.len() as i64, ar * ac);
                        if name == "load" {
                            ok(flare_ikjac_load_as_submatrix(m.as_mut_ptr(), r, c, a.as_mut_ptr(), ar, ac));
                        } else {
                            ok(flare_ikjac_load_as_submatrix_transpose(m.as_mut_ptr(), r, c, a.as_mut_ptr(), ar, ac));
                        }
                        assert_same(&ctx, &m, &inp(2));
                    }
                    "mul" | "tmul" | "mult" => {
                        let (r, k, c) = (iv[0], iv[1], iv[2]);
                        let i = inp(1);
                        let (ar, ac, br, bc) = match name {
                            "mul" => (r, k, k, c),
                            "tmul" => (k, r, k, c),
                            _ => (r, k, c, k),
                        };
                        let na = (ar * ac) as usize;
                        let mut a = i[..na].to_vec();
                        let mut b = i[na..].to_vec();
                        assert_eq!(b.len() as i64, br * bc);
                        let mut d = vec![0.0; (r * c) as usize];
                        let f = match name {
                            "mul" => flare_ikjac_multiply,
                            "tmul" => flare_ikjac_transpose_multiply,
                            _ => flare_ikjac_multiply_transpose,
                        };
                        ok(f(a.as_mut_ptr(), ar, ac, b.as_mut_ptr(), br, bc, d.as_mut_ptr(), r, c));
                        assert_same(&ctx, &d, &inp(2));
                    }
                    "mscalar" => {
                        let (r, k, c) = (iv[0], iv[1], iv[2]);
                        let i = inp(1);
                        let na = (r * k) as usize;
                        let mut a = i[..na].to_vec();
                        let s = i[na];
                        let mut d = i[na + 1..].to_vec();
                        ok(flare_ikjac_multiply_scalar(a.as_mut_ptr(), r, k, s, d.as_mut_ptr(), r, c));
                        assert_same(&ctx, &d, &inp(2));
                    }
                    "solve" => {
                        let n = iv[0];
                        let i = inp(1);
                        let nn = (n * n) as usize;
                        let mut a = i[..nn].to_vec();
                        let mut b = i[nn..nn + n as usize].to_vec();
                        let mut x = i[nn + n as usize..].to_vec();
                        ok(flare_ikjac_solve(a.as_mut_ptr(), n, n, b.as_mut_ptr(), n, x.as_mut_ptr(), n));
                        assert_same(&ctx, &x, &inp(2));
                    }
                    "ref" => {
                        let mut a = inp(1);
                        ok(flare_ikjac_convert_to_ref_no_free(a.as_mut_ptr(), iv[0], iv[1]));
                        assert_same(&ctx, &a, &inp(2));
                    }
                    "givens" => {
                        let ab = inp(1);
                        let (mut c, mut s) = (0.0, 0.0);
                        ok(flare_ikjac_calc_givens_values(ab[0], ab[1], &mut c, &mut s));
                        assert_same(&ctx, &[c, s], &inp(2));
                    }
                    "pgivens" | "pgivens1" => {
                        let (r, c, i1) = (iv[0], iv[1], iv[2]);
                        let i2 = if name == "pgivens" { iv[3] } else { i1 + 1 };
                        let i = inp(1);
                        let nm = (r * c) as usize;
                        let mut a = i[..nm].to_vec();
                        ok(flare_ikjac_post_apply_givens(a.as_mut_ptr(), r, c, i[nm], i[nm + 1], i1, i2));
                        assert_same(&ctx, &a, &inp(2));
                    }
                    "svd" => {
                        let (r, c) = (iv[0], iv[1]);
                        let mut a = inp(1);
                        let mut u: Vec<f64> = (0..r * r).map(|k| 1.0 + 0.01 * k as f64).collect();
                        let mut v: Vec<f64> = (0..c * c).map(|k| 1.0 + 0.01 * k as f64).collect();
                        let mut w = vec![0.0; r.min(c) as usize];
                        ok(flare_ikjac_compute_svd(a.as_mut_ptr(), r, c, u.as_mut_ptr(), r, r, w.as_mut_ptr(), r.min(c), v.as_mut_ptr(), c, c));
                        let mut chk = 0i32;
                        ok(flare_ikjac_debug_check_svd(a.as_mut_ptr(), r, c, u.as_mut_ptr(), r, r, w.as_mut_ptr(), r.min(c), v.as_mut_ptr(), c, c, &mut chk));
                        let want_all = hexv(&secs[2][..secs[2].trim_end().rfind(' ').unwrap()]);
                        let want_ok: i32 = secs[2].trim().rsplit(' ').next().unwrap().parse().unwrap();
                        let mut got = u.clone();
                        got.extend(&w);
                        got.extend(&v);
                        assert_same(&ctx, &got, &want_all);
                        assert_eq!(chk, want_ok, "{ctx}: DebugCheckSVD verdict");
                    }
                    "ang" => {
                        let (n, idx) = (iv[0] as usize, iv[1] as usize);
                        let p = inp(1);
                        assert_eq!(p.len(), 2 * n);
                        let at = |k: usize| (p[2 * k], p[2 * k + 1]);
                        let (pos, nxt) = (at(idx), at(idx + 1));
                        let prev = if idx > 0 { at(idx - 1) } else { (0.0, 0.0) };
                        let mut out = 0.0;
                        ok(flare_ikengine_joint_angle(pos.0, pos.1, (idx > 0) as i32, prev.0, prev.1, nxt.0, nxt.1, &mut out));
                        let want = inp(2)[0];
                        // atan2 is the only libm call recorded; allow the platform's last bit
                        assert!(ulps(out, want) <= 4, "{ctx}: {out} vs {want}");
                    }
                    n if n.starts_with('j') => {
                        let sk = parse_skel(secs[1], secs[2]);
                        let mut j = Jac::new(&sk);
                        let clamp_in = inp(3);
                        if !clamp_in.is_empty() {
                            j.dsclamp.copy_from_slice(&clamp_in);
                        }
                        j.compute(&sk);
                        let (nr, nc, ne) = (j.n_row, j.n_col, j.n_eff);
                        match n {
                            "jtranspose" => ok(flare_ikjac_calc_transpose(j.jend.as_mut_ptr(), nr, nc, j.ds.as_mut_ptr(), nr, j.dt.as_mut_ptr(), nr, j.dtheta.as_mut_ptr(), nc)),
                            "jpseudo" => ok(flare_ikjac_calc_pseudoinverse(j.jend.as_mut_ptr(), nr, nc, ne, j.ds.as_mut_ptr(), nr, j.dtheta.as_mut_ptr(), nc, j.diag_id.as_mut_ptr(), nc)),
                            "jdls" => ok(flare_ikjac_calc_dls(j.jend.as_mut_ptr(), nr, nc, j.u.as_mut_ptr(), nr, nr, j.damp_sq_v.as_mut_ptr(), nr, j.ds.as_mut_ptr(), nr, j.dt.as_mut_ptr(), nr, j.dtheta.as_mut_ptr(), nc)),
                            "jdlssvd" => ok(flare_ikjac_calc_dls_with_svd(j.jend.as_mut_ptr(), nr, nc, j.u.as_mut_ptr(), nr, nr, j.w.as_mut_ptr(), nr.min(nc), j.v.as_mut_ptr(), nc, nc, j.lambda_sq, j.ds.as_mut_ptr(), nr, j.dtheta.as_mut_ptr(), nc)),
                            "jclamp" => ok(flare_ikjac_calc_dt_clamped(j.ds.as_mut_ptr(), nr, j.dt.as_mut_ptr(), nr, j.dsclamp.as_mut_ptr(), ne)),
                            "jupdclamp" => {
                                for i in 0..sk.parent.len() {
                                    if sk.purpose[i] == 1 {
                                        let e = sk.eff_num[i];
                                        ok(flare_ikjac_updated_sclamp_one(
                                            e,
                                            sk.s[i].0,
                                            sk.s[i].1,
                                            sk.target[2 * e as usize],
                                            sk.target[2 * e as usize + 1],
                                            j.ds.as_mut_ptr(),
                                            nr,
                                            j.dsclamp.as_mut_ptr(),
                                            ne,
                                        ));
                                    }
                                }
                            }
                            _ => panic!("unknown op {n}"),
                        }
                        let mut got = j.jend.clone();
                        for part in [&j.ds, &j.dtheta, &j.dt, &j.dsclamp] {
                            got.extend(part.iter());
                        }
                        if n == "jdls" {
                            got.extend(j.u.iter());
                        }
                        assert_same(&ctx, &got, &inp(4));
                    }
                    _ => panic!("unknown golden op {name}"),
                }
            }
            n += 1;
            *per_op.entry(name.to_string()).or_default() += 1;
        }
        assert!(n >= 300, "golden went missing: {n} cases");
        for op in ["svd", "jpseudo", "jdls", "jdlssvd", "jtranspose", "solve", "mul", "ang", "givens"] {
            assert!(per_op.get(op).copied().unwrap_or(0) > 0, "no {op} cases");
        }
    }

    // ---- behaviour the goldens cannot record -------------------------------------------

    #[test]
    fn rejects_bad_arguments() {
        let mut a = [1.0f64; 4];
        let mut out = 0.0;
        unsafe {
            assert_eq!(flare_ikjac_set_diagonal(null_mut(), 2, 2, 1.0), E_ARG);
            assert_eq!(flare_ikjac_set_diagonal(a.as_mut_ptr(), -1, 2, 1.0), E_ARG);
            assert_eq!(flare_ikjac_set_diagonal(a.as_mut_ptr(), i64::MAX, 2, 1.0), E_ARG);
            assert_eq!(flare_ikjac_vec_max_abs(a.as_mut_ptr(), 4, null_mut()), E_ARG);
            assert_eq!(flare_ikjac_vec_max_abs(null_mut(), 4, &mut out), E_ARG);
            // an empty vector may be null: max |x| of nothing is 0
            assert_eq!(flare_ikjac_vec_max_abs(null_mut(), 0, &mut out), 0);
            assert_eq!(out, 0.0);
            assert_eq!(flare_ikjac_calc_givens_values(1.0, 1.0, null_mut(), &mut out), E_ARG);
            assert_eq!(
                flare_ikjac_compute_jacobian(a.as_mut_ptr(), 2, 2, a.as_mut_ptr(), 2, 1, 1, null(), null(), null(), 0, null(), null(), null(), null(), 0),
                E_ARG
            );
            assert_eq!(flare_ikengine_joint_angle(0.0, 0.0, 0, 0.0, 0.0, 1.0, 0.0, null_mut()), E_ARG);
        }
        assert_eq!(a, [1.0; 4]);
    }

    #[test]
    fn compute_jacobian_checks_joint_and_effector_indices() {
        let call = |joint: i32, eff: i32| unsafe {
            let (mut j, mut ds) = ([0.0f64; 4], [0.0f64; 2]);
            let (en, es, enanc) = ([eff], [0.0f64, 0.0], [1usize]);
            let (aj, af, asx) = ([joint], [0u8], [1.0f64, 1.0]);
            let target = [1.0f64, 1.0];
            flare_ikjac_compute_jacobian(
                j.as_mut_ptr(), 2, 2, ds.as_mut_ptr(), 2, 1, 1, en.as_ptr(), es.as_ptr(), enanc.as_ptr(), 1,
                aj.as_ptr(), af.as_ptr(), asx.as_ptr(), target.as_ptr(), 1,
            )
        };
        assert_eq!(call(1, 0), 0);
        assert_eq!(call(2, 0), E_RANGE);
        assert_eq!(call(-1, 0), E_RANGE);
        assert_eq!(call(0, 1), E_RANGE); // effector 1 has no target
    }

    #[test]
    fn out_of_range_accesses_are_status_2_not_ub() {
        unsafe {
            // vector overload of SetSuperDiagonalEntries counts min(r-1, c) entries: one too
            // many for r > c (the C++ wrote past the buffer); same for SetRow's NumRows loop
            // (the source vector aliases the matrix so that the only registered memory is `a`:
            // a bounds check cannot tell two adjacent caller buffers apart)
            let mut a = [0.0f64; 6]; // 3 x 2
            let p = a.as_mut_ptr();
            assert_eq!(flare_ikjac_set_super_diagonal_vec(p, 3, 2, p, 2), E_RANGE);
            assert_eq!(flare_ikjac_set_row(p, 3, 2, 0, p, 2), E_RANGE);
            let mut d = [1.0f64; 2];
            // shape asserts of the original
            assert_eq!(flare_ikjac_set_diagonal_vec(a.as_mut_ptr(), 3, 2, d.as_mut_ptr(), 1), E_SHAPE);
        }
    }

    #[test]
    fn svd_reconstructs_and_is_orthogonal() {
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1u64 << 53) as f64 - 0.5
        };
        for &(r, c) in &[(2i64, 2i64), (3, 3), (2, 6), (6, 2), (5, 7), (7, 5), (9, 9)] {
            for _ in 0..20 {
                let a: Vec<f64> = (0..r * c).map(|_| rnd() * 8.0).collect();
                let mut am = a.clone();
                let mut u = vec![0.0; (r * r) as usize];
                let mut v = vec![0.0; (c * c) as usize];
                let mut w = vec![0.0; r.min(c) as usize];
                unsafe {
                    ok(flare_ikjac_compute_svd(am.as_mut_ptr(), r, c, u.as_mut_ptr(), r, r, w.as_mut_ptr(), r.min(c), v.as_mut_ptr(), c, c));
                    let mut chk = 0;
                    ok(flare_ikjac_debug_check_svd(am.as_mut_ptr(), r, c, u.as_mut_ptr(), r, r, w.as_mut_ptr(), r.min(c), v.as_mut_ptr(), c, c, &mut chk));
                    assert_eq!(chk, 1, "{r}x{c}");
                }
                assert_eq!(am, a, "ComputeSVD must not modify A");
                // A = U diag(w) V^T, column-major
                for i in 0..r {
                    for j in 0..c {
                        let mut s = 0.0;
                        for k in 0..r.min(c) {
                            s += u[(k * r + i) as usize] * w[k as usize] * v[(k * c + j) as usize];
                        }
                        assert!((s - a[(j * r + i) as usize]).abs() < 1e-12, "{r}x{c} ({i},{j})");
                    }
                }
            }
        }
    }

    #[test]
    fn nonterminating_svd_input_is_capped() {
        // The original spins forever on this rank-one 2x3 (found by the reference harness).
        let mut a = [2.0, 4.0, 3.0, 6.0, 4.0, 8.0];
        let mut u = [0.0; 4];
        let mut v = [0.0; 9];
        let mut w = [0.0; 2];
        let rc = unsafe { flare_ikjac_compute_svd(a.as_mut_ptr(), 2, 3, u.as_mut_ptr(), 2, 2, w.as_mut_ptr(), 2, v.as_mut_ptr(), 3, 3) };
        assert!(rc == 0 || rc == E_RANGE, "{rc}");
        // NaN never converges either: bounded, status 2
        let mut a = [f64::NAN; 4];
        let mut u = [0.0; 4];
        let mut v = [0.0; 4];
        let mut w = [0.0; 2];
        let rc = unsafe { flare_ikjac_compute_svd(a.as_mut_ptr(), 2, 2, u.as_mut_ptr(), 2, 2, w.as_mut_ptr(), 2, v.as_mut_ptr(), 2, 2) };
        assert!(rc == 0 || rc == E_RANGE, "{rc}");
        // a non-square U or a 1-element w are the original's assert()ed shapes
        let mut a = [1.0, 2.0];
        let mut u = [0.0; 4];
        let mut v = [0.0; 1];
        let mut w = [0.0; 1];
        let rc = unsafe { flare_ikjac_compute_svd(a.as_mut_ptr(), 2, 1, u.as_mut_ptr(), 2, 2, w.as_mut_ptr(), 1, v.as_mut_ptr(), 1, 1) };
        assert_eq!(rc, E_SHAPE);
    }

    #[test]
    fn sdls_reads_past_a_two_row_jacobian_and_fails_cleanly() {
        // the original sums 3-vectors over a 2-D Jacobian; here the read is a bounds failure
        let (nr, nc, ne) = (2i64, 2i64, 1i64);
        let mut j = [1.0, 0.5, -0.5, 1.0];
        let (mut u, mut w, mut v) = ([0.0; 4], [0.0; 2], [0.0; 4]);
        let (mut jn, mut ds, mut dt) = ([0.0; 2], [0.3, -0.2], [0.0; 2]);
        let (mut clamp, mut pre, mut th) = ([f64::INFINITY], [0.0; 2], [9.0; 2]);
        let rc = unsafe {
            flare_ikjac_calc_sdls(
                j.as_mut_ptr(), nr, nc, u.as_mut_ptr(), nr, nr, w.as_mut_ptr(), 2, v.as_mut_ptr(), nc, nc, ne,
                jn.as_mut_ptr(), ne, nc, ds.as_mut_ptr(), 2, dt.as_mut_ptr(), 2, clamp.as_mut_ptr(), 1, pre.as_mut_ptr(), 2,
                th.as_mut_ptr(), 2,
            )
        };
        assert_eq!(rc, E_RANGE);
        assert_eq!(th, [0.0, 0.0], "dTheta is zeroed before the failing read, as in the C++");
    }

    #[test]
    fn joint_angle_edge_cases() {
        let mut out = 0.0;
        unsafe {
            // root: u = (1, 0); next straight up -> +90 degrees
            ok(flare_ikengine_joint_angle(0.0, 0.0, 0, 0.0, 0.0, 0.0, 2.0, &mut out));
            assert!((out - std::f64::consts::FRAC_PI_2).abs() < 1e-15);
            // collinear continuation -> 0; bend back -> pi
            ok(flare_ikengine_joint_angle(1.0, 0.0, 1, 0.0, 0.0, 3.0, 0.0, &mut out));
            assert_eq!(out, 0.0);
            ok(flare_ikengine_joint_angle(1.0, 0.0, 1, 0.0, 0.0, 0.0, 0.0, &mut out));
            assert!((out.abs() - std::f64::consts::PI).abs() < 1e-15);
            // coincident joints: normalize() of zero -> NaN, as a C++ build without asserts
            ok(flare_ikengine_joint_angle(1.0, 1.0, 1, 1.0, 1.0, 2.0, 2.0, &mut out));
            assert!(out.is_nan());
        }
    }

    #[test]
    fn dls_pulls_a_two_link_arm_towards_its_target() {
        // root(0,0) -> joint(1,0) -> effector(2,0); target above the tip. One DLS step must
        // reduce the distance, and it never exceeds MaxAngleDLS (5 degrees) per joint.
        let sk = Skel {
            parent: vec![-1, 0, 1],
            purpose: vec![0, 0, 1],
            frozen: vec![0, 0, 0],
            joint_num: vec![0, 1, -1],
            eff_num: vec![-1, -1, 0],
            s: vec![(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)],
            target: vec![2.0, 0.5],
        };
        let mut j = Jac::new(&sk);
        j.compute(&sk);
        unsafe {
            ok(flare_ikjac_calc_dls(
                j.jend.as_mut_ptr(), j.n_row, j.n_col, j.u.as_mut_ptr(), j.n_row, j.n_row, j.damp_sq_v.as_mut_ptr(), j.n_row,
                j.ds.as_mut_ptr(), j.n_row, j.dt.as_mut_ptr(), j.n_row, j.dtheta.as_mut_ptr(), j.n_col,
            ));
        }
        assert_eq!(j.ds, vec![0.0, 0.5]);
        // the step limit is applied to 100 * max|dTheta| (the Jacobian is not rescaled)
        let m = 100.0 * j.dtheta.iter().fold(0.0f64, |a, t| a.max(t.abs()));
        assert!(j.dtheta.iter().all(|t| *t > 0.0), "{:?}", j.dtheta);
        assert!(m <= MAX_ANGLE_DLS * (1.0 + 1e-12) && m > 0.99 * MAX_ANGLE_DLS, "{m} vs {MAX_ANGLE_DLS}");
    }
}
