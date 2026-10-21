//! Port of stdfx/iwa_simplexnoise.cpp: `SimplexNoise` 2D / 3D / 4D noise and
//! `getCellIds` (Stefan Gustavson's public domain simplex noise, with the Toonz
//! range changes: 3D scaled by 16 with range 0.6, 4D range 0.66).
//!
//! Only `+ - * /`, comparisons and `sqrt` (IEEE exact) are involved, so results are
//! bit-identical to the C++ built with `-ffp-contract=off` (recorded goldens below).
//! Rust never fuses multiply-add; a C++ build that contracts may differ by 1 ULP.
//!
//! Deviations (only where the C++ was undefined behaviour): `(int)x` of NaN /
//! out-of-range input saturates (NaN -> 0) and the `i + j + k` sums wrap.
//!
//! `iwa_simplexnoise.h` keeps the `SimplexNoise` class (public static signatures),
//! `Grad` and `CellIds` as inline forwarders to the `flare_simplex_*` exports.

const PERM: [i16; 512] = [
    151, 160, 137, 91, 90, 15, 131, 13, 201, 95, 96, 53, 194, 233, 7, 225,
    140, 36, 103, 30, 69, 142, 8, 99, 37, 240, 21, 10, 23, 190, 6, 148,
    247, 120, 234, 75, 0, 26, 197, 62, 94, 252, 219, 203, 117, 35, 11, 32,
    57, 177, 33, 88, 237, 149, 56, 87, 174, 20, 125, 136, 171, 168, 68, 175,
    74, 165, 71, 134, 139, 48, 27, 166, 77, 146, 158, 231, 83, 111, 229, 122,
    60, 211, 133, 230, 220, 105, 92, 41, 55, 46, 245, 40, 244, 102, 143, 54,
    65, 25, 63, 161, 1, 216, 80, 73, 209, 76, 132, 187, 208, 89, 18, 169,
    200, 196, 135, 130, 116, 188, 159, 86, 164, 100, 109, 198, 173, 186, 3, 64,
    52, 217, 226, 250, 124, 123, 5, 202, 38, 147, 118, 126, 255, 82, 85, 212,
    207, 206, 59, 227, 47, 16, 58, 17, 182, 189, 28, 42, 223, 183, 170, 213,
    119, 248, 152, 2, 44, 154, 163, 70, 221, 153, 101, 155, 167, 43, 172, 9,
    129, 22, 39, 253, 19, 98, 108, 110, 79, 113, 224, 232, 178, 185, 112, 104,
    218, 246, 97, 228, 251, 34, 242, 193, 238, 210, 144, 12, 191, 179, 162, 241,
    81, 51, 145, 235, 249, 14, 239, 107, 49, 192, 214, 31, 181, 199, 106, 157,
    184, 84, 204, 176, 115, 121, 50, 45, 127, 4, 150, 254, 138, 236, 205, 93,
    222, 114, 67, 29, 24, 72, 243, 141, 128, 195, 78, 66, 215, 61, 156, 180,
    151, 160, 137, 91, 90, 15, 131, 13, 201, 95, 96, 53, 194, 233, 7, 225,
    140, 36, 103, 30, 69, 142, 8, 99, 37, 240, 21, 10, 23, 190, 6, 148,
    247, 120, 234, 75, 0, 26, 197, 62, 94, 252, 219, 203, 117, 35, 11, 32,
    57, 177, 33, 88, 237, 149, 56, 87, 174, 20, 125, 136, 171, 168, 68, 175,
    74, 165, 71, 134, 139, 48, 27, 166, 77, 146, 158, 231, 83, 111, 229, 122,
    60, 211, 133, 230, 220, 105, 92, 41, 55, 46, 245, 40, 244, 102, 143, 54,
    65, 25, 63, 161, 1, 216, 80, 73, 209, 76, 132, 187, 208, 89, 18, 169,
    200, 196, 135, 130, 116, 188, 159, 86, 164, 100, 109, 198, 173, 186, 3, 64,
    52, 217, 226, 250, 124, 123, 5, 202, 38, 147, 118, 126, 255, 82, 85, 212,
    207, 206, 59, 227, 47, 16, 58, 17, 182, 189, 28, 42, 223, 183, 170, 213,
    119, 248, 152, 2, 44, 154, 163, 70, 221, 153, 101, 155, 167, 43, 172, 9,
    129, 22, 39, 253, 19, 98, 108, 110, 79, 113, 224, 232, 178, 185, 112, 104,
    218, 246, 97, 228, 251, 34, 242, 193, 238, 210, 144, 12, 191, 179, 162, 241,
    81, 51, 145, 235, 249, 14, 239, 107, 49, 192, 214, 31, 181, 199, 106, 157,
    184, 84, 204, 176, 115, 121, 50, 45, 127, 4, 150, 254, 138, 236, 205, 93,
    222, 114, 67, 29, 24, 72, 243, 141, 128, 195, 78, 66, 215, 61, 156, 180,
];
const PERM_MOD12: [i16; 512] = [
    7, 4, 5, 7, 6, 3, 11, 1, 9, 11, 0, 5, 2, 5, 7, 9,
    8, 0, 7, 6, 9, 10, 8, 3, 1, 0, 9, 10, 11, 10, 6, 4,
    7, 0, 6, 3, 0, 2, 5, 2, 10, 0, 3, 11, 9, 11, 11, 8,
    9, 9, 9, 4, 9, 5, 8, 3, 6, 8, 5, 4, 3, 0, 8, 7,
    2, 9, 11, 2, 7, 0, 3, 10, 5, 2, 2, 3, 11, 3, 1, 2,
    0, 7, 1, 2, 4, 9, 8, 5, 7, 10, 5, 4, 4, 6, 11, 6,
    5, 1, 3, 5, 1, 0, 8, 1, 5, 4, 0, 7, 4, 5, 6, 1,
    8, 4, 3, 10, 8, 8, 3, 2, 8, 4, 1, 6, 5, 6, 3, 4,
    4, 1, 10, 10, 4, 3, 5, 10, 2, 3, 10, 6, 3, 10, 1, 8,
    3, 2, 11, 11, 11, 4, 10, 5, 2, 9, 4, 6, 7, 3, 2, 9,
    11, 8, 8, 2, 8, 10, 7, 10, 5, 9, 5, 11, 11, 7, 4, 9,
    9, 10, 3, 1, 7, 2, 0, 2, 7, 5, 8, 4, 10, 5, 4, 8,
    2, 6, 1, 0, 11, 10, 2, 1, 10, 6, 0, 0, 11, 11, 6, 1,
    9, 3, 1, 7, 9, 2, 11, 11, 1, 0, 10, 7, 1, 7, 10, 1,
    4, 0, 0, 8, 7, 1, 2, 9, 7, 4, 6, 2, 6, 8, 1, 9,
    6, 6, 7, 5, 0, 0, 3, 9, 8, 3, 6, 6, 11, 1, 0, 0,
    7, 4, 5, 7, 6, 3, 11, 1, 9, 11, 0, 5, 2, 5, 7, 9,
    8, 0, 7, 6, 9, 10, 8, 3, 1, 0, 9, 10, 11, 10, 6, 4,
    7, 0, 6, 3, 0, 2, 5, 2, 10, 0, 3, 11, 9, 11, 11, 8,
    9, 9, 9, 4, 9, 5, 8, 3, 6, 8, 5, 4, 3, 0, 8, 7,
    2, 9, 11, 2, 7, 0, 3, 10, 5, 2, 2, 3, 11, 3, 1, 2,
    0, 7, 1, 2, 4, 9, 8, 5, 7, 10, 5, 4, 4, 6, 11, 6,
    5, 1, 3, 5, 1, 0, 8, 1, 5, 4, 0, 7, 4, 5, 6, 1,
    8, 4, 3, 10, 8, 8, 3, 2, 8, 4, 1, 6, 5, 6, 3, 4,
    4, 1, 10, 10, 4, 3, 5, 10, 2, 3, 10, 6, 3, 10, 1, 8,
    3, 2, 11, 11, 11, 4, 10, 5, 2, 9, 4, 6, 7, 3, 2, 9,
    11, 8, 8, 2, 8, 10, 7, 10, 5, 9, 5, 11, 11, 7, 4, 9,
    9, 10, 3, 1, 7, 2, 0, 2, 7, 5, 8, 4, 10, 5, 4, 8,
    2, 6, 1, 0, 11, 10, 2, 1, 10, 6, 0, 0, 11, 11, 6, 1,
    9, 3, 1, 7, 9, 2, 11, 11, 1, 0, 10, 7, 1, 7, 10, 1,
    4, 0, 0, 8, 7, 1, 2, 9, 7, 4, 6, 2, 6, 8, 1, 9,
    6, 6, 7, 5, 0, 0, 3, 9, 8, 3, 6, 6, 11, 1, 0, 0,
];

/// Gradient vectors (x, y, z, w); `Grad(x, y, z)` has w = 0.
const GRAD3: [[i32; 3]; 12] = [
    [1, 1, 0],
    [-1, 1, 0],
    [1, -1, 0],
    [-1, -1, 0],
    [1, 0, 1],
    [-1, 0, 1],
    [1, 0, -1],
    [-1, 0, -1],
    [0, 1, 1],
    [0, -1, 1],
    [0, 1, -1],
    [0, -1, -1],
];
const GRAD4: [[i32; 4]; 32] = [
    [0, 1, 1, 1],
    [0, 1, 1, -1],
    [0, 1, -1, 1],
    [0, 1, -1, -1],
    [0, -1, 1, 1],
    [0, -1, 1, -1],
    [0, -1, -1, 1],
    [0, -1, -1, -1],
    [1, 0, 1, 1],
    [1, 0, 1, -1],
    [1, 0, -1, 1],
    [1, 0, -1, -1],
    [-1, 0, 1, 1],
    [-1, 0, 1, -1],
    [-1, 0, -1, 1],
    [-1, 0, -1, -1],
    [1, 1, 0, 1],
    [1, 1, 0, -1],
    [1, -1, 0, 1],
    [1, -1, 0, -1],
    [-1, 1, 0, 1],
    [-1, 1, 0, -1],
    [-1, -1, 0, 1],
    [-1, -1, 0, -1],
    [1, 1, 1, 0],
    [1, 1, -1, 0],
    [1, -1, 1, 0],
    [1, -1, -1, 0],
    [-1, 1, 1, 0],
    [-1, 1, -1, 0],
    [-1, -1, 1, 0],
    [-1, -1, -1, 0],
];

#[inline]
fn perm(i: i32) -> i32 {
    i32::from(PERM[i as usize])
}
#[inline]
fn perm12(i: i32) -> usize {
    PERM_MOD12[i as usize] as usize
}

/// This method is a *lot* faster than using (int)Math.floor(x)
#[inline]
fn fastfloor(x: f64) -> i32 {
    let xi = x as i32;
    if x < f64::from(xi) {
        xi.wrapping_sub(1)
    } else {
        xi
    }
}

#[inline]
fn dot2(g: [i32; 3], x: f64, y: f64) -> f64 {
    f64::from(g[0]) * x + f64::from(g[1]) * y
}
#[inline]
fn dot3(g: [i32; 3], x: f64, y: f64, z: f64) -> f64 {
    f64::from(g[0]) * x + f64::from(g[1]) * y + f64::from(g[2]) * z
}
#[inline]
fn dot4(g: [i32; 4], x: f64, y: f64, z: f64, w: f64) -> f64 {
    f64::from(g[0]) * x + f64::from(g[1]) * y + f64::from(g[2]) * z + f64::from(g[3]) * w
}

/// 2D simplex noise
pub fn noise2(xin: f64, yin: f64) -> f64 {
    let f2: f64 = 0.5 * (3.0f64.sqrt() - 1.0);
    let g2: f64 = (3.0 - 3.0f64.sqrt()) / 6.0;

    let s = (xin + yin) * f2;
    let i = fastfloor(xin + s);
    let j = fastfloor(yin + s);
    let t = f64::from(i.wrapping_add(j)) * g2;
    let x_0 = f64::from(i) - t;
    let y_0 = f64::from(j) - t;
    let x0 = xin - x_0;
    let y0 = yin - y_0;

    let (i1, j1) = if x0 > y0 { (1, 0) } else { (0, 1) };

    let x1 = x0 - f64::from(i1) + g2;
    let y1 = y0 - f64::from(j1) + g2;
    let x2 = x0 - 1.0 + 2.0 * g2;
    let y2 = y0 - 1.0 + 2.0 * g2;

    let ii = i & 255;
    let jj = j & 255;
    let gi0 = perm12(ii + perm(jj));
    let gi1 = perm12(ii + i1 + perm(jj + j1));
    let gi2 = perm12(ii + 1 + perm(jj + 1));

    let n0;
    let n1;
    let n2;
    let mut t0 = 0.5 - x0 * x0 - y0 * y0;
    if t0 < 0.0 {
        n0 = 0.0;
    } else {
        t0 *= t0;
        n0 = t0 * t0 * dot2(GRAD3[gi0], x0, y0);
    }
    let mut t1 = 0.5 - x1 * x1 - y1 * y1;
    if t1 < 0.0 {
        n1 = 0.0;
    } else {
        t1 *= t1;
        n1 = t1 * t1 * dot2(GRAD3[gi1], x1, y1);
    }
    let mut t2 = 0.5 - x2 * x2 - y2 * y2;
    if t2 < 0.0 {
        n2 = 0.0;
    } else {
        t2 *= t2;
        n2 = t2 * t2 * dot2(GRAD3[gi2], x2, y2);
    }
    70.0 * (n0 + n1 + n2)
}

/// Which tetrahedron of the cell: offsets (i1,j1,k1) and (i2,j2,k2). Shared by `noise3` and `cell_ids`
/// (the C++ pastes the same chain twice).
#[inline]
fn simplex3(x0: f64, y0: f64, z0: f64) -> ([i32; 3], [i32; 3]) {
    if x0 >= y0 {
        if y0 >= z0 {
            ([1, 0, 0], [1, 1, 0]) // X Y Z order
        } else if x0 >= z0 {
            ([1, 0, 0], [1, 0, 1]) // X Z Y order
        } else {
            ([0, 0, 1], [1, 0, 1]) // Z X Y order
        }
    } else if y0 < z0 {
        ([0, 0, 1], [0, 1, 1]) // Z Y X order
    } else if x0 < z0 {
        ([0, 1, 0], [0, 1, 1]) // Y Z X order
    } else {
        ([0, 1, 0], [1, 1, 0]) // Y X Z order
    }
}

/// 3D simplex noise
pub fn noise3(xin: f64, yin: f64, zin: f64) -> f64 {
    let f3: f64 = 1.0 / 3.0;
    let g3: f64 = 1.0 / 6.0;

    let s = (xin + yin + zin) * f3;
    let i = fastfloor(xin + s);
    let j = fastfloor(yin + s);
    let k = fastfloor(zin + s);
    let t = f64::from(i.wrapping_add(j).wrapping_add(k)) * g3;
    let x_0 = f64::from(i) - t;
    let y_0 = f64::from(j) - t;
    let z_0 = f64::from(k) - t;
    let x0 = xin - x_0;
    let y0 = yin - y_0;
    let z0 = zin - z_0;

    let ([i1, j1, k1], [i2, j2, k2]) = simplex3(x0, y0, z0);

    let x1 = x0 - f64::from(i1) + g3;
    let y1 = y0 - f64::from(j1) + g3;
    let z1 = z0 - f64::from(k1) + g3;
    let x2 = x0 - f64::from(i2) + 2.0 * g3;
    let y2 = y0 - f64::from(j2) + 2.0 * g3;
    let z2 = z0 - f64::from(k2) + 2.0 * g3;
    let x3 = x0 - 1.0 + 3.0 * g3;
    let y3 = y0 - 1.0 + 3.0 * g3;
    let z3 = z0 - 1.0 + 3.0 * g3;

    let ii = i & 255;
    let jj = j & 255;
    let kk = k & 255;
    let gi0 = perm12(ii + perm(jj + perm(kk)));
    let gi1 = perm12(ii + i1 + perm(jj + j1 + perm(kk + k1)));
    let gi2 = perm12(ii + i2 + perm(jj + j2 + perm(kk + k2)));
    let gi3 = perm12(ii + 1 + perm(jj + 1 + perm(kk + 1)));

    let range = 0.6;
    let n0;
    let n1;
    let n2;
    let n3;
    let mut t0 = range - x0 * x0 - y0 * y0 - z0 * z0;
    if t0 < 0.0 {
        n0 = 0.0;
    } else {
        t0 *= t0;
        n0 = t0 * t0 * dot3(GRAD3[gi0], x0, y0, z0);
    }
    let mut t1 = range - x1 * x1 - y1 * y1 - z1 * z1;
    if t1 < 0.0 {
        n1 = 0.0;
    } else {
        t1 *= t1;
        n1 = t1 * t1 * dot3(GRAD3[gi1], x1, y1, z1);
    }
    let mut t2 = range - x2 * x2 - y2 * y2 - z2 * z2;
    if t2 < 0.0 {
        n2 = 0.0;
    } else {
        t2 *= t2;
        n2 = t2 * t2 * dot3(GRAD3[gi2], x2, y2, z2);
    }
    let mut t3 = range - x3 * x3 - y3 * y3 - z3 * z3;
    if t3 < 0.0 {
        n3 = 0.0;
    } else {
        t3 *= t3;
        n3 = t3 * t3 * dot3(GRAD3[gi3], x3, y3, z3);
    }
    /* changed by Toonz: range [-0.5, 0.5] */
    16.0 * (n0 + n1 + n2 + n3)
}

/// 4D simplex noise
pub fn noise4(x: f64, y: f64, z: f64, w: f64) -> f64 {
    let f4: f64 = (5.0f64.sqrt() - 1.0) / 4.0;
    let g4: f64 = (5.0 - 5.0f64.sqrt()) / 20.0;

    let s = (x + y + z + w) * f4;
    let i = fastfloor(x + s);
    let j = fastfloor(y + s);
    let k = fastfloor(z + s);
    let l = fastfloor(w + s);
    let t = f64::from(i.wrapping_add(j).wrapping_add(k).wrapping_add(l)) * g4;
    let x_0 = f64::from(i) - t;
    let y_0 = f64::from(j) - t;
    let z_0 = f64::from(k) - t;
    let w_0 = f64::from(l) - t;
    let x0 = x - x_0;
    let y0 = y - y_0;
    let z0 = z - z_0;
    let w0 = w - w_0;

    let mut rankx = 0;
    let mut ranky = 0;
    let mut rankz = 0;
    let mut rankw = 0;
    if x0 > y0 {
        rankx += 1;
    } else {
        ranky += 1;
    }
    if x0 > z0 {
        rankx += 1;
    } else {
        rankz += 1;
    }
    if x0 > w0 {
        rankx += 1;
    } else {
        rankw += 1;
    }
    if y0 > z0 {
        ranky += 1;
    } else {
        rankz += 1;
    }
    if y0 > w0 {
        ranky += 1;
    } else {
        rankw += 1;
    }
    if z0 > w0 {
        rankz += 1;
    } else {
        rankw += 1;
    }
    let b = |c: bool| -> i32 { i32::from(c) };
    let (i1, j1, k1, l1) = (b(rankx >= 3), b(ranky >= 3), b(rankz >= 3), b(rankw >= 3));
    let (i2, j2, k2, l2) = (b(rankx >= 2), b(ranky >= 2), b(rankz >= 2), b(rankw >= 2));
    let (i3, j3, k3, l3) = (b(rankx >= 1), b(ranky >= 1), b(rankz >= 1), b(rankw >= 1));

    let x1 = x0 - f64::from(i1) + g4;
    let y1 = y0 - f64::from(j1) + g4;
    let z1 = z0 - f64::from(k1) + g4;
    let w1 = w0 - f64::from(l1) + g4;
    let x2 = x0 - f64::from(i2) + 2.0 * g4;
    let y2 = y0 - f64::from(j2) + 2.0 * g4;
    let z2 = z0 - f64::from(k2) + 2.0 * g4;
    let w2 = w0 - f64::from(l2) + 2.0 * g4;
    let x3 = x0 - f64::from(i3) + 3.0 * g4;
    let y3 = y0 - f64::from(j3) + 3.0 * g4;
    let z3 = z0 - f64::from(k3) + 3.0 * g4;
    let w3 = w0 - f64::from(l3) + 3.0 * g4;
    let x4 = x0 - 1.0 + 4.0 * g4;
    let y4 = y0 - 1.0 + 4.0 * g4;
    let z4 = z0 - 1.0 + 4.0 * g4;
    let w4 = w0 - 1.0 + 4.0 * g4;

    let ii = i & 255;
    let jj = j & 255;
    let kk = k & 255;
    let ll = l & 255;
    let gi0 = (perm(ii + perm(jj + perm(kk + perm(ll)))) % 32) as usize;
    let gi1 = (perm(ii + i1 + perm(jj + j1 + perm(kk + k1 + perm(ll + l1)))) % 32) as usize;
    let gi2 = (perm(ii + i2 + perm(jj + j2 + perm(kk + k2 + perm(ll + l2)))) % 32) as usize;
    let gi3 = (perm(ii + i3 + perm(jj + j3 + perm(kk + k3 + perm(ll + l3)))) % 32) as usize;
    let gi4 = (perm(ii + 1 + perm(jj + 1 + perm(kk + 1 + perm(ll + 1)))) % 32) as usize;

    /* parameter adjusted by Toonz */
    let range = 0.66;

    let n0;
    let n1;
    let n2;
    let n3;
    let n4;
    let mut t0 = range - x0 * x0 - y0 * y0 - z0 * z0 - w0 * w0;
    if t0 < 0.0 {
        n0 = 0.0;
    } else {
        t0 *= t0;
        n0 = t0 * t0 * dot4(GRAD4[gi0], x0, y0, z0, w0);
    }
    let mut t1 = range - x1 * x1 - y1 * y1 - z1 * z1 - w1 * w1;
    if t1 < 0.0 {
        n1 = 0.0;
    } else {
        t1 *= t1;
        n1 = t1 * t1 * dot4(GRAD4[gi1], x1, y1, z1, w1);
    }
    let mut t2 = range - x2 * x2 - y2 * y2 - z2 * z2 - w2 * w2;
    if t2 < 0.0 {
        n2 = 0.0;
    } else {
        t2 *= t2;
        n2 = t2 * t2 * dot4(GRAD4[gi2], x2, y2, z2, w2);
    }
    let mut t3 = range - x3 * x3 - y3 * y3 - z3 * z3 - w3 * w3;
    if t3 < 0.0 {
        n3 = 0.0;
    } else {
        t3 *= t3;
        n3 = t3 * t3 * dot4(GRAD4[gi3], x3, y3, z3, w3);
    }
    let mut t4 = range - x4 * x4 - y4 * y4 - z4 * z4 - w4 * w4;
    if t4 < 0.0 {
        n4 = 0.0;
    } else {
        t4 *= t4;
        n4 = t4 * t4 * dot4(GRAD4[gi4], x4, y4, z4, w4);
    }
    27.0 * (n0 + n1 + n2 + n3 + n4)
}

/// `getCellIds`: the simplex cell a point belongs to, as (i, j, k, i1, j1, k1, i2, j2, k2).
pub fn cell_ids(xin: f64, yin: f64, zin: f64) -> [i32; 9] {
    let f3: f64 = 1.0 / 3.0;
    let s = (xin + yin + zin) * f3;
    let i = fastfloor(xin + s);
    let j = fastfloor(yin + s);
    let k = fastfloor(zin + s);
    let g3: f64 = 1.0 / 6.0;
    let t = f64::from(i.wrapping_add(j).wrapping_add(k)) * g3;
    let x_0 = f64::from(i) - t;
    let y_0 = f64::from(j) - t;
    let z_0 = f64::from(k) - t;
    let x0 = xin - x_0;
    let y0 = yin - y_0;
    let z0 = zin - z_0;
    let ([i1, j1, k1], [i2, j2, k2]) = simplex3(x0, y0, z0);
    [i, j, k, i1, j1, k1, i2, j2, k2]
}

// ---- C ABI ----

#[no_mangle]
pub extern "C" fn flare_simplex_noise2(x: f64, y: f64) -> f64 {
    crate::guard(0.0, || noise2(x, y))
}
#[no_mangle]
pub extern "C" fn flare_simplex_noise3(x: f64, y: f64, z: f64) -> f64 {
    crate::guard(0.0, || noise3(x, y, z))
}
#[no_mangle]
pub extern "C" fn flare_simplex_noise4(x: f64, y: f64, z: f64, w: f64) -> f64 {
    crate::guard(0.0, || noise4(x, y, z, w))
}

/// Writes the nine cell ids to `out` (`i, j, k, i1, j1, k1, i2, j2, k2`). Returns 0, or -1 for a null `out`
/// (or a caught panic), leaving `out` untouched.
///
/// # Safety
/// `out` must be null or point to 9 writable `int`s.
#[no_mangle]
pub unsafe extern "C" fn flare_simplex_cell_ids(x: f64, y: f64, z: f64, out: *mut i32) -> i32 {
    if out.is_null() {
        return -1;
    }
    crate::guard(-1, || {
        let ids = cell_ids(x, y, z);
        std::ptr::copy_nonoverlapping(ids.as_ptr(), out, 9);
        0
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_are_consistent() {
        assert_eq!(PERM[..256], PERM[256..]);
        for i in 0..512 {
            assert_eq!(PERM_MOD12[i], PERM[i] % 12);
        }
    }

    #[test]
    fn origin_is_zero_in_all_dimensions() {
        assert_eq!(noise2(0.0, 0.0), 0.0);
        assert_eq!(noise3(0.0, 0.0, 0.0), 0.0);
        assert_eq!(noise4(0.0, 0.0, 0.0, 0.0), 0.0);
    }

    #[test]
    fn ranges_are_bounded() {
        for a in 0..40 {
            let (x, y, z, w) = (a as f64 * 0.173 - 3.0, a as f64 * 0.311 + 1.0, a as f64 * 0.057, -(a as f64) * 0.43);
            assert!(noise2(x, y).abs() <= 1.2);
            assert!(noise3(x, y, z).abs() <= 1.0);
            assert!(noise4(x, y, z, w).abs() <= 1.2);
        }
    }

    #[test]
    fn cell_ids_of_a_known_point() {
        // s = 1.6/3 -> floor(1.03), floor(0.73), floor(1.43)
        let c = cell_ids(0.5, 0.2, 0.9);
        assert_eq!(&c[..3], &[1, 0, 1]);
        // x0 < z0 branch etc. are covered by the recorded cases
    }

    #[test]
    fn ffi_cell_ids_null_and_ok() {
        unsafe {
            assert_eq!(flare_simplex_cell_ids(1.0, 2.0, 3.0, std::ptr::null_mut()), -1);
            let mut o = [7i32; 9];
            assert_eq!(flare_simplex_cell_ids(1.0, 2.0, 3.0, o.as_mut_ptr()), 0);
            assert_eq!(o, cell_ids(1.0, 2.0, 3.0));
        }
        assert_eq!(flare_simplex_noise3(0.3, 0.6, 0.9), noise3(0.3, 0.6, 0.9));
    }

    // Replays cases recorded from the ORIGINAL iwa_simplexnoise.cpp (tests/native/gen_noise_cases.py).
    // Line: `S2|S3|S4 <f64 bits hex args...> <f64 bits hex result>` or `C <3 f64 bits> <9 ints>`
    #[test]
    fn matches_recorded_cpp() {
        let mut n = 0;
        for line in GOLDEN.lines() {
            let t: Vec<&str> = line.split_whitespace().collect();
            let f = |s: &str| f64::from_bits(u64::from_str_radix(s, 16).unwrap());
            match t[0] {
                "S2" => assert_eq!(noise2(f(t[1]), f(t[2])).to_bits(), f(t[3]).to_bits(), "{line}"),
                "S3" => assert_eq!(noise3(f(t[1]), f(t[2]), f(t[3])).to_bits(), f(t[4]).to_bits(), "{line}"),
                "S4" => assert_eq!(noise4(f(t[1]), f(t[2]), f(t[3]), f(t[4])).to_bits(), f(t[5]).to_bits(), "{line}"),
                "C" => {
                    let want: Vec<i32> = t[4..13].iter().map(|s| s.parse().unwrap()).collect();
                    assert_eq!(cell_ids(f(t[1]), f(t[2]), f(t[3])).to_vec(), want, "{line}");
                }
                k => panic!("bad kind {k}"),
            }
            n += 1;
        }
        assert!(n > 1000, "{n}");
    }

    const GOLDEN: &str = include_str!("iwa_simplexnoise_cases.txt");
}
