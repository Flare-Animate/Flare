//! Port of flare/metnum.cpp (P. Foggia, 1991): normal-equation least squares via
//! Cholesky. Pure f64 math on flat buffers; operation order is kept identical to
//! the C++ so results are bit-identical.
//!
//! `AllocMatrix`/`FreeMatrix`/`AllocTriangMatrix` stay inline C++ in `metnum.h`
//! (they only wrap `new[]`); the four solvers below are the C ABI. Matrices cross
//! the boundary as `double**` row-pointer arrays, exactly as before; they are
//! validated for null and copied to flat storage, so no panic or OOB read can
//! cross the FFI boundary.

const DBL_EPSILON: f64 = 2.2204460492503131e-16;

/// Lower-triangular storage: row i starts at i*(i+1)/2.
#[inline]
fn tri(i: usize, j: usize) -> usize {
    i * (i + 1) / 2 + j
}

/// In-place Cholesky of a lower-triangular packed matrix. 0 ok, -1 n<1, k>0 bad row k.
pub fn cholesky(n: i32, a: &mut [f64]) -> i32 {
    if n < 1 {
        return -1;
    }
    let n = n as usize;
    for j in 0..n {
        let mut acc = a[tri(j, j)];
        for k in 0..j {
            acc -= a[tri(j, k)] * a[tri(j, k)];
        }
        if acc <= 0.0 {
            return j as i32 + 1;
        }
        let radq = acc.sqrt();
        a[tri(j, j)] = radq;
        for i in j + 1..n {
            let mut acc = a[tri(i, j)];
            for k in 0..j {
                acc -= a[tri(i, k)] * a[tri(j, k)];
            }
            a[tri(i, j)] = acc / radq;
        }
    }
    0
}

/// Forward substitution; `x` may alias `b` (same slice semantics: pass the same vec).
pub fn chol_forw(n: i32, a: &[f64], x: &mut [f64]) -> i32 {
    if n < 1 {
        return -1;
    }
    let nn = n as usize;
    for i in 0..nn {
        if a[tri(i, i)].abs() <= n as f64 * DBL_EPSILON {
            return i as i32 + 1;
        }
        // x[i] = b[i] already (b == x in place)
        for j in 0..i {
            x[i] -= a[tri(i, j)] * x[j];
        }
        x[i] /= a[tri(i, i)];
    }
    0
}

pub fn chol_back(n: i32, a: &[f64], x: &mut [f64]) -> i32 {
    if n < 1 {
        return -1;
    }
    let nn = n as usize;
    for i in (0..nn).rev() {
        if a[tri(i, i)].abs() <= n as f64 * DBL_EPSILON {
            return i as i32 + 1;
        }
        for j in i + 1..nn {
            x[i] -= a[tri(j, i)] * x[j];
        }
        x[i] /= a[tri(i, i)];
    }
    0
}

/// Least-squares fit. `x` is m rows of n values (row-major flat), `y` has n values,
/// returns c (m values) or the C++ error code (-1 bad size, -3 solve failure).
pub fn approx(n: i32, m: i32, x: &[f64], y: &[f64]) -> Result<Vec<f64>, i32> {
    if n < 1 || m < 1 {
        return Err(-1);
    }
    let (nu, mu) = (n as usize, m as usize);
    if x.len() < nu.checked_mul(mu).ok_or(-2)? || y.len() < nu {
        return Err(-1);
    }
    let mut a = vec![0.0f64; mu * (mu + 1) / 2];
    for i in 0..mu {
        for j in 0..=i {
            let mut s = 0.0;
            for k in 0..nu {
                s += x[i * nu + k] * x[j * nu + k];
            }
            a[tri(i, j)] = s;
        }
    }
    let mut c = vec![0.0f64; mu];
    for i in 0..mu {
        let mut s = 0.0;
        for k in 0..nu {
            s += x[i * nu + k] * y[k];
        }
        c[i] = s;
    }
    if cholesky(m, &mut a) != 0 || chol_forw(m, &a, &mut c) != 0 || chol_back(m, &a, &mut c) != 0 {
        return Err(-3);
    }
    Ok(c)
}

// ---------------------------------------------------------------- C ABI

unsafe fn rows(a: *mut *mut f64, n: usize, len: impl Fn(usize) -> usize) -> Option<Vec<&'static [f64]>> {
    if a.is_null() {
        return None;
    }
    let mut v = Vec::with_capacity(n);
    for i in 0..n {
        let p = *a.add(i);
        if p.is_null() {
            return None;
        }
        v.push(std::slice::from_raw_parts(p as *const f64, len(i)));
    }
    Some(v)
}

fn guard<T>(d: T, f: impl FnOnce() -> T) -> T {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or(d)
}

/// MetNum::Approx. Returns 0, -1 (bad args/null), -2 (alloc), -3 (solve).
#[no_mangle]
pub unsafe extern "C" fn flare_metnum_approx(n: i32, m: i32, x: *mut *mut f64, y: *const f64, c: *mut f64) -> i32 {
    guard(-2, || {
        if n < 1 || m < 1 {
            return -1;
        }
        if x.is_null() || y.is_null() || c.is_null() {
            return -1;
        }
        let (nu, mu) = (n as usize, m as usize);
        let Some(rs) = rows(x, mu, |_| nu) else { return -1 };
        let mut flat = Vec::with_capacity(nu * mu);
        for r in &rs {
            flat.extend_from_slice(r);
        }
        let ys = std::slice::from_raw_parts(y, nu);
        match approx(n, m, &flat, ys) {
            Ok(cv) => {
                std::ptr::copy_nonoverlapping(cv.as_ptr(), c, mu);
                0
            }
            Err(e) => e,
        }
    })
}

/// Reads the lower triangle of a `double**` (row i has >= i+1 entries).
unsafe fn read_tri(a: *mut *mut f64, n: usize) -> Option<Vec<f64>> {
    let rs = rows(a, n, |i| i + 1)?;
    let mut v = Vec::with_capacity(n * (n + 1) / 2);
    for r in rs {
        v.extend_from_slice(r);
    }
    Some(v)
}

#[no_mangle]
pub unsafe extern "C" fn flare_metnum_cholesky(n: i32, a: *mut *mut f64) -> i32 {
    guard(-1, || {
        if n < 1 || a.is_null() {
            return -1;
        }
        let Some(mut t) = read_tri(a, n as usize) else { return -1 };
        let r = cholesky(n, &mut t);
        // the C++ leaves partial results in A on failure; write back everything
        for i in 0..n as usize {
            let row = *a.add(i);
            std::ptr::copy_nonoverlapping(t.as_ptr().add(tri(i, 0)), row, i + 1);
        }
        r
    })
}

unsafe fn solve(n: i32, a: *mut *mut f64, b: *const f64, x: *mut f64, back: bool) -> i32 {
    guard(-1, || {
        if n < 1 || a.is_null() || b.is_null() || x.is_null() {
            return -1;
        }
        let nu = n as usize;
        let Some(t) = read_tri(a, nu) else { return -1 };
        // b may alias x; the C++ writes x[i] progressively, reading b[i] just before.
        // Emulate on a working copy of b, then publish x (identical unless the
        // diagonal check fails midway; mirror that by writing the computed prefix/suffix).
        let mut w: Vec<f64> = std::slice::from_raw_parts(b, nu).to_vec();
        let r = if back { chol_back(n, &t, &mut w) } else { chol_forw(n, &t, &mut w) };
        let _ = r;
        // Failure position: reproduce which x entries the C++ had written.
        let fail = r - 1; // row index that failed (r>0)
        for i in 0..nu {
            let written = if r <= 0 {
                true
            } else if back {
                (i as i32) > fail
            } else {
                (i as i32) < fail
            };
            if written {
                *x.add(i) = w[i];
            }
            // else: the C++ checks the diagonal before touching x[i]
        }
        r
    })
}

#[no_mangle]
pub unsafe extern "C" fn flare_metnum_chol_forw(n: i32, a: *mut *mut f64, b: *const f64, x: *mut f64) -> i32 {
    solve(n, a, b, x, false)
}

#[no_mangle]
pub unsafe extern "C" fn flare_metnum_chol_back(n: i32, a: *mut *mut f64, b: *const f64, x: *mut f64) -> i32 {
    solve(n, a, b, x, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cholesky_2x2() {
        let mut a = vec![4.0, 2.0, 3.0];
        assert_eq!(cholesky(2, &mut a), 0);
        assert!((a[0] - 2.0).abs() < 1e-15 && (a[1] - 1.0).abs() < 1e-15);
        assert!((a[2] - 2f64.sqrt()).abs() < 1e-15);
    }
    #[test]
    fn cholesky_not_pd() {
        let mut a = vec![1.0, 2.0, 1.0];
        assert_eq!(cholesky(2, &mut a), 2);
        assert_eq!(cholesky(0, &mut a), -1);
    }
    #[test]
    fn approx_line_fit() {
        // y = 2 + 3t on basis {1, t}
        let t: Vec<f64> = (0..5).map(|i| i as f64).collect();
        let mut x = vec![1.0; 5];
        x.extend(&t);
        let y: Vec<f64> = t.iter().map(|t| 2.0 + 3.0 * t).collect();
        let c = approx(5, 2, &x, &y).unwrap();
        assert!((c[0] - 2.0).abs() < 1e-12 && (c[1] - 3.0).abs() < 1e-12);
    }
    #[test]
    fn approx_errors() {
        assert_eq!(approx(0, 2, &[], &[]), Err(-1));
        // dependent basis -> singular
        let x = vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0];
        assert_eq!(approx(3, 2, &x, &[1.0, 2.0, 3.0]), Err(-3));
    }
    #[test]
    fn ffi_null_checks() {
        unsafe {
            assert_eq!(flare_metnum_approx(2, 2, std::ptr::null_mut(), std::ptr::null(), std::ptr::null_mut()), -1);
            assert_eq!(flare_metnum_cholesky(2, std::ptr::null_mut()), -1);
            assert_eq!(flare_metnum_chol_forw(2, std::ptr::null_mut(), std::ptr::null(), std::ptr::null_mut()), -1);
        }
    }
    #[test]
    fn ffi_roundtrip() {
        unsafe {
            let mut r0 = [4.0f64];
            let mut r1 = [2.0f64, 3.0];
            let mut rows = [r0.as_mut_ptr(), r1.as_mut_ptr()];
            assert_eq!(flare_metnum_cholesky(2, rows.as_mut_ptr()), 0);
            let b = [1.0, 2.0];
            let mut x = [0.0; 2];
            assert_eq!(flare_metnum_chol_forw(2, rows.as_mut_ptr(), b.as_ptr(), x.as_mut_ptr()), 0);
            assert_eq!(flare_metnum_chol_back(2, rows.as_mut_ptr(), x.as_ptr(), x.as_mut_ptr()), 0);
            // A = [[4,2],[2,3+1]]? verify A x = b with A = L L^T
            let l = [[2.0, 0.0], [1.0, 2f64.sqrt()]];
            for i in 0..2 {
                let ax: f64 = (0..2).map(|j| (0..2).map(|k| l[i][k] * l[j][k]).sum::<f64>() * x[j]).sum();
                assert!((ax - b[i]).abs() < 1e-12);
            }
        }
    }
}
