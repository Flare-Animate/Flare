//! Port of the numeric half of `common/tgeometry/tgeometry.cpp`: `TAffine::inv` and
//! `rotation`, the `TRotation` / `TScale` about-a-centre constructors, all of `TAffine3`
//! and `TAffine4` (product, inverse, 2D extraction, translation / scale / rotation
//! builders) and the whole `TAngleRangeSet` bit-set algebra.
//!
//! Partial port (`tgeometry.cpp` stays): the `TConsts` constant objects, `TAffine`'s hot
//! inline-sized members (`operator*`, `==`, `isIdentity`, `place`, ...) and the `TRectD`
//! transform remain C++; they are a handful of flops each and called per point.
//!
//! The matrices cross the C ABI as row-major `double` arrays (6 for `TAffine`: a11 a12 a13
//! a21 a22 a23; 9 for `TAffine3`; 16 for `TAffine4`), the same order as the C++ members.
//! Every expression keeps the original operand order, so results are bit-identical
//! (libm `sin` / `cos` / `sqrt` are the platform's, as before).
//!
//! Faithfully kept quirks of the C++ (found while porting, listed for the maintainers):
//! * `TAffine3::inv` computes its determinant as `r.a11*a11 + r.a12*a21 + r.a12*a31`
//!   (`r.a12` twice; a correct cofactor expansion would use `r.a13*a31`), so the "inverse"
//!   is wrong for any matrix with `a31 != 0` unless `r.a12 == r.a13`;
//! * `TAffine4::inv` reads `r.a21`, `r.a31`, `r.a41` (still the identity's 0) when forming
//!   the determinant, and leaves `det` unscaled when `|det| <= epsilon`;
//! * `TAffine3::inv` yields an all-zero matrix when `|det| <= epsilon`.
//! `TAngleRangeSet::invert(x)` / `set(x)` on the set itself are no-ops or undefined in the
//! C++ (iterating a vector while mutating it); here the set works on a snapshot.

use std::f64::consts::FRAC_PI_4;

const EPSILON: f64 = 1e-8; // TConsts::epsilon
const M_PI_180: f64 = FRAC_PI_4 / 45.0;

/// x86 `cvttsd2si` (what the C++ `(int)double` compiles to): `INT_MIN` for NaN / range.
fn to_int(v: f64) -> i32 {
    if v.is_nan() || v >= 2147483648.0 || v <= -2147483649.0 {
        i32::MIN
    } else {
        v as i32
    }
}

type Aff = [f64; 6];

/// `TAffine::operator*(const TAffine &)`
fn aff_mul(a: &Aff, b: &Aff) -> Aff {
    let (a11, a12, a13, a21, a22, a23) = (a[0], a[1], a[2], a[3], a[4], a[5]);
    [
        a11 * b[0] + a12 * b[3],
        a11 * b[1] + a12 * b[4],
        a11 * b[2] + a12 * b[5] + a13,
        a21 * b[0] + a22 * b[3],
        a21 * b[1] + a22 * b[4],
        a21 * b[2] + a22 * b[5] + a23,
    ]
}

/// `TAffine::inv`
pub fn affine_inv(a: &Aff) -> Aff {
    let (a11, a12, a13, a21, a22, a23) = (a[0], a[1], a[2], a[3], a[4], a[5]);
    let big = f64::MAX / (1 << 16) as f64;
    if a12 == 0.0 && a21 == 0.0 {
        let inv_a11 = if a11 == 0.0 { big } else { 1.0 / a11 };
        let inv_a22 = if a22 == 0.0 { big } else { 1.0 / a22 };
        [inv_a11, 0.0, -a13 * inv_a11, 0.0, inv_a22, -a23 * inv_a22]
    } else if a11 == 0.0 && a22 == 0.0 {
        let inv_a21 = if a21 == 0.0 { big } else { 1.0 / a21 };
        let inv_a12 = if a12 == 0.0 { big } else { 1.0 / a12 };
        [0.0, inv_a21, -a23 * inv_a21, inv_a12, 0.0, -a13 * inv_a12]
    } else {
        let d = 1. / (a11 * a22 - a12 * a21);
        [a22 * d, -a12 * d, (a12 * a23 - a22 * a13) * d, -a21 * d, a11 * d, (a21 * a13 - a11 * a23) * d]
    }
}

/// `TAffine::rotation(angle)` (radians)
pub fn affine_rotation(angle: f64) -> Aff {
    let s = angle.sin();
    let c = angle.cos();
    [c, -s, 0.0, s, c, 0.0]
}

/// `TRotation(degrees)`
pub fn rotation_degrees(degrees: f64) -> Aff {
    let (sn, cs);
    let idegrees = to_int(degrees);
    if idegrees as f64 == degrees && idegrees % 90 == 0 {
        match (idegrees / 90) & 3 {
            0 => {
                sn = 0.0;
                cs = 1.0;
            }
            1 => {
                sn = 1.0;
                cs = 0.0;
            }
            2 => {
                sn = 0.0;
                cs = -1.0;
            }
            _ => {
                sn = -1.0;
                cs = 0.0;
            }
        }
    } else {
        let rad = degrees * M_PI_180;
        let mut s = rad.sin();
        let mut c = rad.cos();
        if s == 1.0 || s == -1.0 {
            c = 0.0;
        }
        if c == 1.0 || c == -1.0 {
            s = 0.0;
        }
        sn = s;
        cs = c;
    }
    // a11 = cs; a12 = -sn; a21 = -a12; a22 = a11; a13 = a23 = 0 (default TAffine)
    let a12 = -sn;
    [cs, a12, 0.0, -a12, cs, 0.0]
}

fn translation(x: f64, y: f64) -> Aff {
    [1.0, 0.0, x, 0.0, 1.0, y]
}

/// `T(center) * m * T(-center)`, left to right as written in the C++.
fn about_center(cx: f64, cy: f64, m: &Aff) -> Aff {
    aff_mul(&aff_mul(&translation(cx, cy), m), &translation(-cx, -cy))
}

type M3 = [f64; 9];
type M4 = [f64; 16];

fn mul3_vec(a: &M3, b: [f64; 3]) -> [f64; 3] {
    [
        b[0] * a[0] + b[1] * a[3] + b[2] * a[6],
        b[0] * a[1] + b[1] * a[4] + b[2] * a[7],
        b[0] * a[2] + b[1] * a[5] + b[2] * a[8],
    ]
}

fn mul3(a: &M3, b: &M3) -> M3 {
    let mut r = [0.0; 9];
    for i in 0..3 {
        let v = mul3_vec(a, [b[3 * i], b[3 * i + 1], b[3 * i + 2]]);
        r[3 * i..3 * i + 3].copy_from_slice(&v);
    }
    r
}

fn identity3() -> M3 {
    [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]
}

/// `TAffine3::inv` (determinant quirk kept, see the module docs)
pub fn inv3(m: &M3) -> M3 {
    let [a11, a12, a13, a21, a22, a23, a31, a32, a33] = *m;
    let mut r = identity3();
    r[0] = a22 * a33 - a32 * a23;
    r[1] = a32 * a13 - a12 * a33;
    r[2] = a12 * a23 - a22 * a13;
    let mut det = r[0] * a11 + r[1] * a21 + r[1] * a31;
    det = if det.abs() > EPSILON { 1.0 / det } else { 0.0 };
    r[0] *= det;
    r[1] *= det;
    r[2] *= det;
    r[3] = (a31 * a23 - a21 * a33) * det;
    r[4] = (a11 * a33 - a31 * a13) * det;
    r[5] = (a21 * a13 - a11 * a23) * det;
    r[6] = (a21 * a32 - a31 * a22) * det;
    r[7] = (a31 * a12 - a11 * a32) * det;
    r[8] = (a11 * a22 - a21 * a12) * det;
    r
}

fn identity4() -> M4 {
    let mut r = [0.0; 16];
    for i in 0..4 {
        r[5 * i] = 1.0;
    }
    r
}

fn mul4_vec(a: &M4, b: [f64; 4]) -> [f64; 4] {
    [
        b[0] * a[0] + b[1] * a[4] + b[2] * a[8] + b[3] * a[12],
        b[0] * a[1] + b[1] * a[5] + b[2] * a[9] + b[3] * a[13],
        b[0] * a[2] + b[1] * a[6] + b[2] * a[10] + b[3] * a[14],
        b[0] * a[3] + b[1] * a[7] + b[2] * a[11] + b[3] * a[15],
    ]
}

fn mul4(a: &M4, b: &M4) -> M4 {
    let mut r = [0.0; 16];
    for i in 0..4 {
        let v = mul4_vec(a, [b[4 * i], b[4 * i + 1], b[4 * i + 2], b[4 * i + 3]]);
        r[4 * i..4 * i + 4].copy_from_slice(&v);
    }
    r
}

/// `TAffine4::inv`
pub fn inv4(m: &M4) -> M4 {
    let [a11, a12, a13, a14, a21, a22, a23, a24, a31, a32, a33, a34, a41, a42, a43, a44] = *m;
    let mut r = identity4();
    r[0] = a22 * (a33 * a44 - a34 * a43) + a23 * (a34 * a42 - a32 * a44) + a24 * (a32 * a43 - a33 * a42);
    r[1] = a21 * (a34 * a43 - a33 * a44) + a23 * (a31 * a44 - a34 * a41) + a24 * (a33 * a41 - a31 * a43);
    r[2] = a21 * (a32 * a44 - a34 * a42) + a22 * (a34 * a41 - a31 * a44) + a24 * (a31 * a42 - a32 * a41);
    r[3] = a21 * (a33 * a42 - a32 * a43) + a22 * (a31 * a43 - a33 * a41) + a23 * (a32 * a41 - a31 * a42);

    // r.a21, r.a31, r.a41 are still the identity's zeros here (as in the C++)
    let mut det = a11 * r[0] + a12 * r[4] + a13 * r[8] + a14 * r[12];
    if det.abs() > EPSILON {
        det = 1.0 / det;
    }
    r[0] *= det;
    r[1] *= det;
    r[2] *= det;
    r[3] *= det;

    r[4] = det * (a12 * (a34 * a43 - a33 * a44) + a13 * (a32 * a44 - a34 * a42) + a14 * (a33 * a42 - a32 * a43));
    r[5] = det * (a11 * (a33 * a44 - a34 * a43) + a13 * (a34 * a41 - a31 * a44) + a14 * (a31 * a43 - a33 * a41));
    r[6] = det * (a11 * (a34 * a42 - a32 * a44) + a12 * (a31 * a44 - a34 * a41) + a14 * (a32 * a41 - a31 * a42));
    r[7] = det * (a11 * (a32 * a43 - a33 * a42) + a12 * (a33 * a41 - a31 * a43) + a13 * (a31 * a42 - a32 * a41));
    r[8] = det * (a12 * (a23 * a44 - a24 * a43) + a13 * (a24 * a42 - a22 * a44) + a14 * (a22 * a43 - a23 * a42));
    r[9] = det * (a11 * (a24 * a43 - a23 * a44) + a13 * (a21 * a44 - a24 * a41) + a14 * (a23 * a41 - a21 * a43));
    r[10] = det * (a11 * (a22 * a44 - a24 * a42) + a12 * (a24 * a41 - a21 * a44) + a14 * (a21 * a42 - a22 * a41));
    r[11] = det * (a11 * (a23 * a42 - a22 * a43) + a12 * (a21 * a43 - a23 * a41) + a13 * (a22 * a41 - a21 * a42));
    r[12] = det * (a12 * (a24 * a33 - a23 * a34) + a13 * (a22 * a34 - a24 * a32) + a14 * (a23 * a32 - a22 * a33));
    r[13] = det * (a11 * (a23 * a34 - a24 * a33) + a13 * (a24 * a31 - a21 * a34) + a14 * (a21 * a33 - a23 * a31));
    r[14] = det * (a11 * (a24 * a32 - a22 * a34) + a12 * (a21 * a34 - a24 * a31) + a14 * (a22 * a31 - a21 * a32));
    r[15] = det * (a11 * (a22 * a33 - a23 * a32) + a12 * (a23 * a31 - a21 * a33) + a13 * (a21 * a32 - a22 * a31));
    r
}

/// `TAffine4::rotation(x, y, z, angle)`: rotation about an axis (identity for a null axis).
pub fn rotation4(mut x: f64, mut y: f64, mut z: f64, angle: f64) -> M4 {
    let mut r = identity4();
    let mut k = x * x + y * y + z * z;
    if k > EPSILON * EPSILON {
        k = 1.0 / k.sqrt();
        let s = angle.sin();
        let c = angle.cos();
        let ic = 1.0 - c;
        x *= k;
        y *= k;
        z *= k;
        r[0] = ic * x * x + c;
        r[1] = ic * x * y + s * z;
        r[2] = ic * z * x - s * y;
        r[4] = ic * x * y - s * z;
        r[5] = ic * y * y + c;
        r[6] = ic * y * z + s * x;
        r[8] = ic * z * x + s * y;
        r[9] = ic * y * z - s * x;
        r[10] = ic * z * z + c;
    }
    r
}

// ---------------------------------------------------------------------------------------
// C ABI: matrices

unsafe fn arr<'a, const N: usize>(p: *const f64) -> Option<&'a [f64; N]> {
    if p.is_null() {
        None
    } else {
        Some(&*(p as *const [f64; N]))
    }
}

unsafe fn put<const N: usize>(p: *mut f64, v: [f64; N]) -> i32 {
    if p.is_null() {
        return 1;
    }
    std::ptr::copy_nonoverlapping(v.as_ptr(), p, N);
    0
}

macro_rules! guarded {
    ($body:expr) => {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| $body)).unwrap_or(-1)
    };
}

/// `TAffine::inv`. `a` and `out` are 6 doubles; `out` may alias `a`. Returns 0 or 1 (null).
///
/// # Safety
/// Pointers valid for the stated element counts.
#[no_mangle]
pub unsafe extern "C" fn flare_taffine_inv(a: *const f64, out: *mut f64) -> i32 {
    guarded!(match arr::<6>(a) {
        Some(a) => put(out, affine_inv(a)),
        None => 1,
    })
}

/// `TAffine::rotation(angle)` (radians) into 6 doubles.
///
/// # Safety
/// `out` valid for 6 doubles.
#[no_mangle]
pub unsafe extern "C" fn flare_taffine_rotation(angle: f64, out: *mut f64) -> i32 {
    guarded!(put(out, affine_rotation(angle)))
}

/// `TRotation(degrees)` (`center == NULL`) or `TRotation(center, degrees)` (`center` 2 doubles).
///
/// # Safety
/// `out` valid for 6 doubles, `center` null or valid for 2.
#[no_mangle]
pub unsafe extern "C" fn flare_trotation(center: *const f64, degrees: f64, out: *mut f64) -> i32 {
    guarded!({
        let r = rotation_degrees(degrees);
        match arr::<2>(center) {
            Some(c) => put(out, about_center(c[0], c[1], &r)),
            None => put(out, r),
        }
    })
}

/// `TScale(center, sx, sy)` (pass `sy = sx` for the uniform `TScale(center, s)`).
///
/// # Safety
/// `center` valid for 2 doubles, `out` for 6.
#[no_mangle]
pub unsafe extern "C" fn flare_tscale_about(center: *const f64, sx: f64, sy: f64, out: *mut f64) -> i32 {
    guarded!(match arr::<2>(center) {
        Some(c) => put(out, about_center(c[0], c[1], &[sx, 0.0, 0.0, 0.0, sy, 0.0])),
        None => 1,
    })
}

/// `TAffine3` operations. `op`: 0 `a*b` (matrices), 1 `inv(a)`, 2 `get2d` (6 out),
/// 3 translation2d(a[0], a[1]), 4 scale2d(a[0], a[1]), 5 rotation2d(a[0]), 6 `a * vec(b)`
/// (3 out). `b` is read for 0 and 6 only. `out` has 9 doubles (6 for op 2, 3 for op 6).
///
/// # Safety
/// Pointers valid for the element counts of the chosen op.
#[no_mangle]
pub unsafe extern "C" fn flare_taffine3_op(op: i32, a: *const f64, b: *const f64, out: *mut f64) -> i32 {
    guarded!({
        match op {
            0 => match (arr::<9>(a), arr::<9>(b)) {
                (Some(a), Some(b)) => put(out, mul3(a, b)),
                _ => 1,
            },
            1 => match arr::<9>(a) {
                Some(a) => put(out, inv3(a)),
                None => 1,
            },
            2 => match arr::<9>(a) {
                Some(m) => put(out, [m[0], m[3], m[6], m[1], m[4], m[7]]),
                None => 1,
            },
            3 | 4 => match arr::<2>(a) {
                Some(v) => {
                    let mut r = identity3();
                    if op == 3 {
                        r[6] = v[0];
                        r[7] = v[1];
                    } else {
                        r[0] = v[0];
                        r[4] = v[1];
                    }
                    put(out, r)
                }
                None => 1,
            },
            5 => match arr::<1>(a) {
                Some(v) => {
                    let mut r = identity3();
                    let s = v[0].sin();
                    let c = v[0].cos();
                    r[0] = c;
                    r[1] = s;
                    r[3] = -s;
                    r[4] = c;
                    put(out, r)
                }
                None => 1,
            },
            6 => match (arr::<9>(a), arr::<3>(b)) {
                (Some(a), Some(b)) => put(out, mul3_vec(a, *b)),
                _ => 1,
            },
            _ => 2,
        }
    })
}

/// `TAffine4` operations. `op`: 0 `a*b`, 1 `inv(a)`, 2 `get2d(z = b[0])` (6 out),
/// 3 `get2dPersp(z = b[0])` (9 out), 4 translation(a[0..3]), 5 scale(a[0..3]),
/// 6 rotation(a[0..4] = x y z angle), 7 rotationX(a[0]), 8 rotationY(a[0]),
/// 9 rotationZ(a[0]), 10 `a * vec4(b)` (4 out).
///
/// # Safety
/// Pointers valid for the element counts of the chosen op.
#[no_mangle]
pub unsafe extern "C" fn flare_taffine4_op(op: i32, a: *const f64, b: *const f64, out: *mut f64) -> i32 {
    guarded!({
        match op {
            0 => match (arr::<16>(a), arr::<16>(b)) {
                (Some(a), Some(b)) => put(out, mul4(a, b)),
                _ => 1,
            },
            1 => match arr::<16>(a) {
                Some(a) => put(out, inv4(a)),
                None => 1,
            },
            2 => match (arr::<16>(a), arr::<1>(b)) {
                (Some(m), Some(z)) => put(out, [m[0], m[4], z[0] * m[8] + m[12], m[1], m[5], z[0] * m[9] + m[13]]),
                _ => 1,
            },
            3 => match (arr::<16>(a), arr::<1>(b)) {
                (Some(m), Some(z)) => {
                    let z = z[0];
                    put(out, [m[0], m[1], m[3], m[4], m[5], m[7], m[8] * z + m[12], m[9] * z + m[13], m[11] * z + m[15]])
                }
                _ => 1,
            },
            4 => match arr::<3>(a) {
                Some(v) => {
                    let mut r = identity4();
                    r[12] = v[0];
                    r[13] = v[1];
                    r[14] = v[2];
                    put(out, r)
                }
                None => 1,
            },
            5 => match arr::<3>(a) {
                Some(v) => {
                    let mut r = identity4();
                    r[0] = v[0];
                    r[5] = v[1];
                    r[10] = v[2];
                    put(out, r)
                }
                None => 1,
            },
            6 => match arr::<4>(a) {
                Some(v) => put(out, rotation4(v[0], v[1], v[2], v[3])),
                None => 1,
            },
            7 | 8 | 9 => match arr::<1>(a) {
                Some(v) => {
                    let mut r = identity4();
                    let s = v[0].sin();
                    let c = v[0].cos();
                    match op {
                        7 => {
                            r[5] = c;
                            r[6] = s;
                            r[9] = -s;
                            r[10] = c;
                        }
                        8 => {
                            r[0] = c;
                            r[2] = -s;
                            r[8] = s;
                            r[10] = c;
                        }
                        _ => {
                            r[0] = c;
                            r[1] = s;
                            r[4] = -s;
                            r[5] = c;
                        }
                    }
                    put(out, r)
                }
                None => 1,
            },
            10 => match (arr::<16>(a), arr::<4>(b)) {
                (Some(a), Some(b)) => put(out, mul4_vec(a, *b)),
                _ => 1,
            },
            _ => 2,
        }
    })
}

// ---------------------------------------------------------------------------------------
// TAngleRangeSet

/// The set of angle ranges: a sorted list of boundaries plus the `flip` flag that says whether
/// the stretch before the first boundary is inside.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AngleSet {
    pub flip: bool,
    pub a: Vec<u32>,
}

impl AngleSet {
    fn is_empty(&self) -> bool {
        !self.flip && self.a.is_empty()
    }
    fn is_full(&self) -> bool {
        self.flip && self.a.is_empty()
    }
    fn clear(&mut self) {
        self.flip = false;
        self.a.clear();
    }
    fn fill(&mut self) {
        self.flip = true;
        self.a.clear();
    }
    fn invert_all(&mut self) {
        self.flip = !self.flip;
    }

    /// `find`: index of the boundary at or before `a` (wrapping to the last).
    fn find(&self, a: u32) -> usize {
        let mut i0 = 0usize;
        let mut i1 = self.a.len() - 1;
        if a < self.a[0] {
            return i1;
        }
        if self.a[i1] <= a {
            return i1;
        }
        loop {
            let i = (i1 + i0) / 2;
            if i == i0 {
                break;
            }
            if self.a[i] <= a {
                i0 = i;
            } else {
                i1 = i;
            }
        }
        i0
    }

    fn insert(&mut self, a: u32) {
        if self.a.is_empty() {
            self.a.push(a);
        } else {
            let i = self.find(a);
            if self.a[i] == a {
                self.a.remove(i);
            } else if a < self.a[0] {
                self.a.insert(0, a);
            } else {
                self.a.insert(i + 1, a);
            }
        }
    }

    fn do_add(&mut self, a0: u32, a1: u32) {
        if self.a.is_empty() {
            if !self.flip {
                self.set2(a0, a1);
            }
            return;
        }
        let mut i0 = self.find(a0);
        let i1 = self.find(a1);
        if i0 == i1 {
            let visible = (i0 % 2 != 0) == self.flip;
            if self.a[i0] != a0 && self.a[i0].wrapping_sub(a0) <= a1.wrapping_sub(a0) {
                if visible {
                    self.fill();
                } else {
                    self.set2(a0, a1);
                }
            } else if !visible {
                if a1 < a0 {
                    self.flip = true;
                }
                self.insert(a0);
                self.insert(a1);
            }
            return;
        }
        let visible0 = (i0 % 2 != 0) == self.flip;
        let visible1 = (i1 % 2 != 0) == self.flip;
        // remove range (i0, i1]
        i0 = (i0 + 1) % self.a.len();
        if i1 < i0 {
            self.a.drain(i0..);
            self.a.drain(..i1 + 1);
        } else {
            self.a.drain(i0..i1 + 1);
        }
        if !visible0 {
            self.insert(a0);
        }
        if !visible1 {
            self.insert(a1);
        }
        if self.a.is_empty() || a1 < a0 {
            self.flip = true;
        }
    }

    fn contains(&self, a: u32) -> bool {
        if self.is_empty() {
            return false;
        }
        if self.is_full() {
            return true;
        }
        (self.find(a) % 2 != 0) == self.flip
    }

    fn check(&self) -> bool {
        if self.a.len() % 2 != 0 {
            return false;
        }
        for i in 1..self.a.len() {
            if self.a[i - 1] >= self.a[i] {
                return false;
            }
        }
        true
    }

    fn set2(&mut self, a0: u32, a1: u32) {
        self.a.clear();
        if a0 < a1 {
            self.flip = false;
            self.a.push(a0);
            self.a.push(a1);
        } else if a0 > a1 {
            self.flip = true;
            self.a.push(a1);
            self.a.push(a0);
        } else {
            self.flip = true;
        }
    }

    fn set_x(&mut self, x: &AngleSet, flip: bool) {
        self.flip = x.flip != flip;
        self.a = x.a.clone();
    }

    fn invert2(&mut self, a0: u32, a1: u32) {
        if a0 == a1 {
            return;
        }
        if self.is_empty() {
            self.set2(a0, a1);
            return;
        }
        if self.is_full() {
            self.set2(a1, a0);
            return;
        }
        if a1 < a0 {
            self.flip = !self.flip;
        }
        self.insert(a0);
        self.insert(a1);
    }

    fn invert_x(&mut self, x: &AngleSet) {
        if x.is_empty() {
            return;
        }
        if x.is_full() {
            self.invert_all();
            return;
        }
        if self.is_empty() {
            self.set_x(x, false);
            return;
        }
        if self.is_full() {
            self.set_x(x, true);
            return;
        }
        self.flip = self.flip != x.flip;
        for &v in &x.a {
            self.insert(v);
        }
    }

    fn add2(&mut self, a0: u32, a1: u32) {
        if !self.is_full() && a0 != a1 {
            if self.is_empty() {
                self.set2(a0, a1);
            } else {
                self.do_add(a0, a1);
            }
        }
    }

    /// The `(a0, a1)` pairs `TAngleRangeSet::Iterator(x, flip)` visits before it laps.
    fn ranges(x: &AngleSet, flip: bool) -> Vec<(u32, u32)> {
        let f = x.flip != flip;
        let l = &x.a;
        if l.is_empty() {
            return vec![];
        }
        let n = l.len();
        if f {
            // prebegin = last, begin = 0, end = n - 2
            let mut v = vec![(l[n - 1], l[0])];
            let mut cur = 0usize;
            while cur != n - 2 {
                cur += 2;
                v.push((l[cur - 1], l[cur]));
            }
            v
        } else {
            // prebegin = 0, begin = 1, end = n - 1
            let mut v = vec![(l[0], l[1])];
            let mut cur = 1usize;
            while cur != n - 1 {
                cur += 2;
                v.push((l[cur - 1], l[cur]));
            }
            v
        }
    }

    fn add_x(&mut self, x: &AngleSet, same: bool) {
        if same || self.is_full() || x.is_empty() {
            return;
        }
        if self.is_empty() {
            self.set_x(x, false);
            return;
        }
        if x.is_full() {
            self.fill();
            return;
        }
        for (a0, a1) in Self::ranges(x, false) {
            if self.is_full() {
                break;
            }
            self.do_add(a0, a1);
        }
    }

    fn subtract2(&mut self, a0: u32, a1: u32) {
        if !self.is_empty() && a0 != a1 {
            if self.is_full() {
                self.set2(a1, a0);
            } else {
                self.invert_all();
                self.do_add(a0, a1);
                self.invert_all();
            }
        }
    }

    fn subtract_x(&mut self, x: &AngleSet, same: bool) {
        if self.is_empty() || x.is_empty() {
            return;
        }
        if same || x.is_full() {
            self.clear();
            return;
        }
        if self.is_full() {
            self.set_x(x, false);
            self.invert_all();
            return;
        }
        self.invert_all();
        for (a0, a1) in Self::ranges(x, false) {
            if self.is_full() {
                break;
            }
            self.do_add(a0, a1);
        }
        self.invert_all();
    }

    fn intersect2(&mut self, a0: u32, a1: u32) {
        if !self.is_empty() {
            if a0 == a1 {
                self.clear();
            } else if self.is_full() {
                self.set2(a0, a1);
            } else {
                self.invert_all();
                self.do_add(a1, a0);
                self.invert_all();
            }
        }
    }

    fn intersect_x(&mut self, x: &AngleSet, same: bool) {
        if same || self.is_empty() || x.is_full() {
            return;
        }
        if x.is_empty() {
            self.clear();
            return;
        }
        if self.is_full() {
            self.set_x(x, false);
            return;
        }
        self.invert_all();
        for (a0, a1) in Self::ranges(x, true) {
            if self.is_full() {
                break;
            }
            self.do_add(a0, a1);
        }
        self.invert_all();
    }
}

/// Operation codes of [`flare_angle_set_op`].
pub const ANGLE_CONTAINS: i32 = 1; // returns 0 / 1
pub const ANGLE_CHECK: i32 = 2; // returns 0 / 1
pub const ANGLE_SET2: i32 = 3;
pub const ANGLE_SET_X: i32 = 4; // arg_flip
pub const ANGLE_INVERT2: i32 = 5;
pub const ANGLE_INVERT_X: i32 = 6;
pub const ANGLE_ADD2: i32 = 7;
pub const ANGLE_ADD_X: i32 = 8;
pub const ANGLE_SUBTRACT2: i32 = 9;
pub const ANGLE_SUBTRACT_X: i32 = 10;
pub const ANGLE_INTERSECT2: i32 = 11;
pub const ANGLE_INTERSECT_X: i32 = 12;

/// One `TAngleRangeSet` operation on the set `(*flip, angles[..*len])`. The two-angle forms
/// use `a0`, `a1`; the set forms use `(xa[..xlen], xflip)` as the operand (`same != 0` when the
/// operand is the receiver itself, which the C++ special-cases) and `arg_flip` for `set(x,
/// flip)`. `angles` needs room for `*len + xlen + 2` entries (`cap`; `contains` / `check` only read and need `*len`); the new content is written
/// back to `angles` / `len` / `flip`. Returns the query result for `contains` / `check`, 0 for
/// the mutators, -1 for a bad argument (null pointers, `cap` too small, unknown op).
///
/// # Safety
/// `angles` valid for `cap` entries, `xa` for `xlen`, `flip` / `len` for one value.
#[no_mangle]
pub unsafe extern "C" fn flare_angle_set_op(
    op: i32,
    a0: u32,
    a1: u32,
    flip: *mut i32,
    angles: *mut u32,
    len: *mut usize,
    cap: usize,
    xa: *const u32,
    xlen: usize,
    xflip: i32,
    arg_flip: i32,
    same: i32,
) -> i32 {
    guarded!({
        if flip.is_null() || len.is_null() || (angles.is_null() && *len != 0) || (xa.is_null() && xlen != 0) {
            return -1;
        }
        let n = *len;
        // contains / check only read; they need no spare room and write nothing back
        let query = op == ANGLE_CONTAINS || op == ANGLE_CHECK;
        if n > cap
            || xlen > (1 << 28)
            || (!query && n.checked_add(xlen).and_then(|v| v.checked_add(2)).map_or(true, |v| v > cap))
        {
            return -1;
        }
        let mut s = AngleSet { flip: *flip != 0, a: if n == 0 { vec![] } else { std::slice::from_raw_parts(angles, n).to_vec() } };
        let x = AngleSet {
            flip: xflip != 0,
            a: if xlen == 0 { vec![] } else { std::slice::from_raw_parts(xa, xlen).to_vec() },
        };
        let same = same != 0;
        // find() / insert() assume a non-empty list when they run; the C++ guards that by
        // construction (flip/empty tests first), and a caller handing a malformed set (odd
        // length) gets exactly the C++ behaviour or a refused call below.
        let mut ret = 0;
        match op {
            ANGLE_CONTAINS => ret = s.contains(a0) as i32,
            ANGLE_CHECK => ret = s.check() as i32,
            ANGLE_SET2 => s.set2(a0, a1),
            ANGLE_SET_X => {
                if !same {
                    s.set_x(&x, arg_flip != 0)
                }
            }
            ANGLE_INVERT2 => s.invert2(a0, a1),
            ANGLE_INVERT_X => s.invert_x(&x),
            ANGLE_ADD2 => s.add2(a0, a1),
            ANGLE_ADD_X => s.add_x(&x, same),
            ANGLE_SUBTRACT2 => s.subtract2(a0, a1),
            ANGLE_SUBTRACT_X => s.subtract_x(&x, same),
            ANGLE_INTERSECT2 => s.intersect2(a0, a1),
            ANGLE_INTERSECT_X => s.intersect_x(&x, same),
            _ => return -1,
        }
        if query {
            return ret;
        }
        if s.a.len() > cap {
            return -1;
        }
        *flip = s.flip as i32;
        *len = s.a.len();
        if !s.a.is_empty() {
            if angles.is_null() {
                return -1;
            }
            std::ptr::copy_nonoverlapping(s.a.as_ptr(), angles, s.a.len());
        }
        ret
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(flip: bool, a: &[u32]) -> AngleSet {
        AngleSet { flip, a: a.to_vec() }
    }

    #[test]
    fn affine_inverse_round_trips() {
        let a = [2.0, 1.0, 3.0, -1.0, 4.0, 5.0];
        let i = affine_inv(&a);
        let p = aff_mul(&a, &i);
        for (x, y) in p.iter().zip([1.0, 0.0, 0.0, 0.0, 1.0, 0.0]) {
            assert!((x - y).abs() < 1e-12, "{p:?}");
        }
        // degenerate diagonal / anti-diagonal branches
        assert_eq!(affine_inv(&[2.0, 0.0, 4.0, 0.0, 0.5, 1.0]), [0.5, 0.0, -2.0, 0.0, 2.0, -2.0]);
        assert_eq!(affine_inv(&[0.0, 2.0, 1.0, 4.0, 0.0, 8.0])[1], 0.25);
        assert_eq!(affine_inv(&[0.0; 6])[0], f64::MAX / 65536.0);
    }

    #[test]
    fn rotations() {
        assert_eq!(rotation_degrees(90.0), [0.0, -1.0, 0.0, 1.0, 0.0, 0.0]);
        assert_eq!(rotation_degrees(180.0)[0], -1.0);
        assert_eq!(rotation_degrees(-90.0)[1], 1.0);
        let r = rotation_degrees(30.0);
        assert!((r[0] - 0.866_025_403_784_438_6).abs() < 1e-15);
        let c = about_center(10.0, 0.0, &rotation_degrees(180.0));
        assert!((c[2] - 20.0).abs() < 1e-12 && c[5].abs() < 1e-12);
    }

    #[test]
    fn affine3_and_4_basics() {
        let t = {
            let mut o = [0.0; 9];
            unsafe { flare_taffine3_op(3, [4.0, 5.0].as_ptr(), std::ptr::null(), o.as_mut_ptr()) };
            o
        };
        // translation applied to the row (1, 1, 1): z row adds
        assert_eq!(mul3_vec(&t, [1.0, 1.0, 1.0]), [1.0 + 4.0, 1.0 + 5.0, 1.0]);
        // a pure translation has r.a12 == 0, so the quirky determinant is still right
        assert_eq!(mul3(&t, &inv3(&t)), identity3());
        // with r.a12 != 0 and a31 != 0 it is not (kept as in the C++)
        let q = [1.0, 2.0, 0.0, 0.0, 1.0, 0.0, 3.0, 0.0, 1.0];
        assert_ne!(mul3(&q, &inv3(&q)), identity3());
        assert_eq!(inv3(&identity3()), identity3());
        let m = rotation4(0.0, 0.0, 0.0, 1.0);
        assert_eq!(m, identity4());
        assert_eq!(inv4(&identity4()), identity4());
    }

    #[test]
    fn angle_set_basics() {
        let mut s = set(false, &[]);
        s.add2(10, 20);
        assert_eq!(s, set(false, &[10, 20]));
        s.add2(15, 30);
        assert_eq!(s, set(false, &[10, 30]));
        s.add2(40, 50);
        assert!(s.contains(45) && !s.contains(35) && s.check());
        s.subtract2(12, 45);
        assert!(!s.contains(20) && s.contains(46));
        let mut full = set(true, &[]);
        full.subtract2(5, 6);
        assert_eq!(full, set(true, &[5, 6])); // everything except [5,6): flip with the hole
        let x = set(false, &[10, 20]);
        let mut y = set(false, &[15, 30]);
        y.intersect_x(&x, false);
        assert_eq!(y, set(false, &[15, 20]));
    }

    fn hv(t: &[&str]) -> Vec<f64> {
        t.iter().map(|x| f64::from_bits(u64::from_str_radix(x, 16).unwrap())).collect()
    }

    fn same_bits(got: &[f64], want: &[f64], ctx: &str) {
        assert_eq!(got.len(), want.len(), "{ctx}");
        for (g, w) in got.iter().zip(want) {
            assert_eq!(g.to_bits(), w.to_bits(), "{ctx}: {got:?} vs {want:?}");
        }
    }

    fn parse_set(t: &mut std::slice::Iter<&str>) -> AngleSet {
        let flip = t.next().unwrap().parse::<i32>().unwrap() != 0;
        let n: usize = t.next().unwrap().parse().unwrap();
        AngleSet { flip, a: (0..n).map(|_| t.next().unwrap().parse().unwrap()).collect() }
    }

    /// Replays `tests/native/gen_tgeometry_cases.py`: the original `tgeometry.cpp` (g++,
    /// `-ffp-contract=off`) for every ported member, compared bit for bit / entry for entry.
    #[test]
    fn matches_recorded_cpp() {
        let golden = include_str!("tgeometry_cases.txt");
        let mut counts = std::collections::BTreeMap::<String, usize>::new();
        for line in golden.lines() {
            let (lhs, rhs) = line.split_once(" ->").expect(line);
            let l: Vec<&str> = lhs.split(' ').collect();
            let kind = if l[0] == "as" { "as".to_string() } else if l[0].starts_with('m') { format!("{} {}", l[0], l[1]) } else { l[0].to_string() };
            *counts.entry(kind).or_default() += 1;
            let mut out = [0.0f64; 16];
            match l[0] {
                "a-inv" => {
                    let a = hv(&l[1..]);
                    let r = affine_inv(&[a[0], a[1], a[2], a[3], a[4], a[5]]);
                    same_bits(&r, &hv(&rhs.trim().split(' ').collect::<Vec<_>>()), line);
                }
                "a-rot" => {
                    let a = hv(&l[1..]);
                    same_bits(&affine_rotation(a[0]), &hv(&rhs.trim().split(' ').collect::<Vec<_>>()), line);
                }
                "a-deg" | "a-degc" | "a-sc2" | "a-sc1" => {
                    let a = hv(&l[1..]);
                    let r = unsafe {
                        match l[0] {
                            "a-deg" => flare_trotation(std::ptr::null(), a[0], out.as_mut_ptr()),
                            "a-degc" => flare_trotation(a.as_ptr(), a[2], out.as_mut_ptr()),
                            "a-sc2" => flare_tscale_about(a.as_ptr(), a[2], a[3], out.as_mut_ptr()),
                            _ => flare_tscale_about(a.as_ptr(), a[2], a[2], out.as_mut_ptr()),
                        }
                    };
                    assert_eq!(r, 0);
                    same_bits(&out[..6], &hv(&rhs.trim().split(' ').collect::<Vec<_>>()), line);
                }
                "m3" | "m4" => {
                    let op3 = ["mul", "inv", "get2d", "tr", "sc", "rot", "vec"];
                    let op4 = ["mul", "inv", "get2d", "persp", "tr", "sc", "rot", "rx", "ry", "rz", "vec"];
                    let names: &[&str] = if l[0] == "m3" { &op3 } else { &op4 };
                    let op = names.iter().position(|n| *n == l[1]).unwrap() as i32;
                    let n = if l[0] == "m3" { 9 } else { 16 };
                    let args = hv(&l[2..]);
                    let want = hv(&rhs.trim().split(' ').collect::<Vec<_>>());
                    let (pa, pb): (&[f64], &[f64]) = match (l[0], l[1]) {
                        ("m3", "mul") | ("m4", "mul") => (&args[..n], &args[n..]),
                        ("m3", "inv") | ("m3", "get2d") | ("m4", "inv") => (&args[..n], &[]),
                        ("m3", "vec") => (&args[..9], &args[9..]),
                        ("m4", "vec") => (&args[..16], &args[16..]),
                        ("m4", "get2d") | ("m4", "persp") => (&args[..16], &args[16..]),
                        _ => (&args[..], &[]),
                    };
                    let pbp = if pb.is_empty() { std::ptr::null() } else { pb.as_ptr() };
                    let rc = unsafe {
                        if l[0] == "m3" {
                            // the C++ goldens call `get2d` as op 2 and `vec` as op 6, builders 3..5
                            flare_taffine3_op(op, pa.as_ptr(), pbp, out.as_mut_ptr())
                        } else {
                            // golden names rx/ry/rz/vec map to ops 7, 8, 9, 10
                            flare_taffine4_op(op, pa.as_ptr(), pbp, out.as_mut_ptr())
                        }
                    };
                    assert_eq!(rc, 0, "{line}");
                    same_bits(&out[..want.len()], &want, line);
                }
                "as" => {
                    let op: i32 = l[1].parse().unwrap();
                    let a0: u32 = l[2].parse().unwrap();
                    let a1: u32 = l[3].parse().unwrap();
                    let arg_flip: i32 = l[4].parse().unwrap();
                    let same: i32 = l[5].parse().unwrap();
                    let mut it = l[6..].iter(); // "|S" flip n a..  "|X" flip n a..
                    assert_eq!(*it.next().unwrap(), "|S");
                    let mut s = parse_set(&mut it);
                    assert_eq!(*it.next().unwrap(), "|X");
                    let x = parse_set(&mut it);
                    let w: Vec<&str> = rhs.trim().split(' ').collect();
                    let mut wi = w.iter();
                    let want = parse_set(&mut wi);
                    let want_ret: i32 = wi.next().unwrap().parse().unwrap();
                    let cap = s.a.len() + x.a.len() + 2;
                    s.a.resize(cap, 0);
                    let mut len = cap - x.a.len() - 2;
                    let mut flip = s.flip as i32;
                    let ret = unsafe {
                        flare_angle_set_op(
                            op, a0, a1, &mut flip, s.a.as_mut_ptr(), &mut len, cap, x.a.as_ptr(), x.a.len(),
                            x.flip as i32, arg_flip, same,
                        )
                    };
                    assert!(ret >= 0, "{line}");
                    s.a.truncate(len);
                    assert_eq!((flip != 0, s.a.clone(), ret), (want.flip, want.a.clone(), want_ret), "{line}");
                }
                other => panic!("unknown record {other}"),
            }
        }
        assert!(counts.len() >= 25 && counts["as"] > 2000, "{counts:?}");
    }
}
