//! Port of flarelib/iknode.cpp: the maths of `IKNode::computeS` and `rotatePoint`.
//!
//! `IKNode` keeps its layout and accessors in `flare/iknode.h` (it is a pointer-linked
//! node class, so the object graph stays in C++). What moved here is the arithmetic:
//!
//! ```text
//! s = r;                      // local position
//! for y in ancestors(node):   // parent, grandparent, ... root
//!     s = rotatePoint(s, y.theta);
//!     s += y.r;
//!     m_pos = s;              // only when there is at least one ancestor
//! ```
//!
//! The C++ walked two pointers (`y` and a trailing `w`) but after each step `w` is
//! the ancestor just rotated by, so every iteration adds that ancestor's `r`.
//! The header forwarder gathers `(theta, r)` of the ancestor chain nearest-first and
//! hands the arrays to [`flare_iknode_compute_s`].
//! `IKNode::setPurpose` is a plain setter and stays inline in the header.

/// `rotatePoint`: rotate `(x, y)` by `theta` radians. Operation order is the C++ one
/// (`tempx` is computed before `y` is overwritten, `y` uses the *old* `x`).
pub fn rotate_point(x: f64, y: f64, theta: f64) -> (f64, f64) {
    let costheta = theta.cos();
    let sintheta = theta.sin();
    let tempx = x * costheta - y * sintheta;
    let ty = y * costheta + x * sintheta;
    (tempx, ty)
}

/// `IKNode::computeS` over a pre-gathered ancestor chain (nearest ancestor first).
pub fn compute_s(r: (f64, f64), chain: &[(f64, (f64, f64))]) -> (f64, f64) {
    let mut s = r;
    for &(theta, ar) in chain {
        s = rotate_point(s.0, s.1, theta);
        s.0 += ar.0;
        s.1 += ar.1;
    }
    s
}

/// `rotatePoint(point, theta)` in place. Returns 0, or 1 for null pointers.
///
/// # Safety
/// `x` and `y` must be null or valid for a write of one `f64`.
#[no_mangle]
pub unsafe extern "C" fn flare_iknode_rotate_point(x: *mut f64, y: *mut f64, theta: f64) -> i32 {
    if x.is_null() || y.is_null() {
        return 1;
    }
    crate::guard(2, || {
        let (nx, ny) = rotate_point(*x, *y, theta);
        *x = nx;
        *y = ny;
        0
    })
}

/// Global position of a node. `thetas[i]` / `rs[2*i], rs[2*i+1]` describe the i-th
/// ancestor (nearest first); `n == 0` is the root. Writes `out_s[0..2]`.
/// Returns 0, 1 for null/oversized arguments, 2 if a panic was caught.
///
/// # Safety
/// `thetas` must be null or valid for `n` reads, `rs` for `2 * n` reads, `out_s` for
/// two writes.
#[no_mangle]
pub unsafe extern "C" fn flare_iknode_compute_s(
    rx: f64,
    ry: f64,
    n: usize,
    thetas: *const f64,
    rs: *const f64,
    out_s: *mut f64,
) -> i32 {
    if out_s.is_null() || (n > 0 && (thetas.is_null() || rs.is_null())) || n > usize::MAX / 16 {
        return 1;
    }
    crate::guard(2, || {
        let chain: Vec<(f64, (f64, f64))> = if n == 0 {
            Vec::new()
        } else {
            let t = std::slice::from_raw_parts(thetas, n);
            let r = std::slice::from_raw_parts(rs, 2 * n);
            (0..n).map(|i| (t[i], (r[2 * i], r[2 * i + 1]))).collect()
        };
        let (sx, sy) = compute_s((rx, ry), &chain);
        *out_s = sx;
        *out_s.add(1) = sy;
        0
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

    /// Direct transcription of the original C++ (two pointers `y`/`w`), kept here so
    /// the gathered-chain formulation is checked against the literal control flow.
    struct Node {
        parent: Option<usize>,
        r: (f64, f64),
        theta: f64,
    }
    fn cpp_compute_s(nodes: &[Node], idx: usize) -> ((f64, f64), Option<(f64, f64)>) {
        let mut s = nodes[idx].r;
        let mut y = nodes[idx].parent;
        let mut w = Some(idx);
        let mut m_pos = None;
        while let Some(yi) = y {
            s = rotate_point(s.0, s.1, nodes[yi].theta);
            y = nodes[yi].parent;
            w = nodes[w.unwrap()].parent;
            s.0 += nodes[w.unwrap()].r.0;
            s.1 += nodes[w.unwrap()].r.1;
            m_pos = Some(s);
        }
        (s, m_pos)
    }
    fn chain_of(nodes: &[Node], idx: usize) -> Vec<(f64, (f64, f64))> {
        let mut v = vec![];
        let mut y = nodes[idx].parent;
        while let Some(yi) = y {
            v.push((nodes[yi].theta, nodes[yi].r));
            y = nodes[yi].parent;
        }
        v
    }

    #[test]
    fn rotate_quarter_turn() {
        let (x, y) = rotate_point(1.0, 0.0, FRAC_PI_2);
        assert!((x - 0.0).abs() < 1e-15 && (y - 1.0).abs() < 1e-15);
        // theta = 0 is the identity, bit for bit
        assert_eq!(rotate_point(3.5, -2.25, 0.0), (3.5, -2.25));
    }

    #[test]
    fn rotate_uses_old_x_for_y() {
        // x' = 2cos - 1sin, y' = 1cos + 2sin at theta = pi/2 -> (-1, 2)
        let (x, y) = rotate_point(2.0, 1.0, FRAC_PI_2);
        assert!((x + 1.0).abs() < 1e-15 && (y - 2.0).abs() < 1e-15);
    }

    #[test]
    fn root_keeps_local_position() {
        let mut out = [9.0, 9.0];
        let rc = unsafe { flare_iknode_compute_s(0.0, 1.0, 0, std::ptr::null(), std::ptr::null(), out.as_mut_ptr()) };
        assert_eq!(rc, 0);
        assert_eq!(out, [0.0, 1.0]);
    }

    #[test]
    fn two_link_chain_by_hand() {
        // child r=(1,0), parent theta=pi/2 r=(0,1): rotate (1,0) -> (0,1), += (0,1) -> (0,2)
        let mut out = [0.0; 2];
        let thetas = [FRAC_PI_2];
        let rs = [0.0, 1.0];
        let rc = unsafe { flare_iknode_compute_s(1.0, 0.0, 1, thetas.as_ptr(), rs.as_ptr(), out.as_mut_ptr()) };
        assert_eq!(rc, 0);
        assert!(out[0].abs() < 1e-15 && (out[1] - 2.0).abs() < 1e-15, "{out:?}");
    }

    #[test]
    fn matches_literal_cpp_walk_on_random_trees() {
        let mut seed = 0x1234_5678_9abc_def0u64;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        for _ in 0..200 {
            let n = 1 + (rnd() * 9.0) as usize;
            let mut nodes: Vec<Node> = Vec::new();
            for i in 0..n {
                nodes.push(Node {
                    parent: if i == 0 { None } else { Some((rnd() * i as f64) as usize) },
                    r: (rnd() * 20.0 - 10.0, rnd() * 20.0 - 10.0),
                    theta: rnd() * 12.0 - 6.0,
                });
            }
            for idx in 0..n {
                let (want, m_pos) = cpp_compute_s(&nodes, idx);
                let chain = chain_of(&nodes, idx);
                let thetas: Vec<f64> = chain.iter().map(|c| c.0).collect();
                let rs: Vec<f64> = chain.iter().flat_map(|c| [c.1 .0, c.1 .1]).collect();
                let mut out = [0.0; 2];
                let rc = unsafe {
                    flare_iknode_compute_s(
                        nodes[idx].r.0,
                        nodes[idx].r.1,
                        chain.len(),
                        thetas.as_ptr(),
                        rs.as_ptr(),
                        out.as_mut_ptr(),
                    )
                };
                assert_eq!(rc, 0);
                assert_eq!(out[0].to_bits(), want.0.to_bits());
                assert_eq!(out[1].to_bits(), want.1.to_bits());
                // m_pos is assigned exactly when there is an ancestor
                assert_eq!(m_pos.is_some(), !chain.is_empty());
            }
        }
    }

    #[test]
    fn rejects_bad_arguments() {
        let mut out = [0.0; 2];
        let one = [0.0f64];
        let two = [0.0f64; 2];
        unsafe {
            assert_eq!(flare_iknode_compute_s(0.0, 0.0, 0, std::ptr::null(), std::ptr::null(), std::ptr::null_mut()), 1);
            assert_eq!(flare_iknode_compute_s(0.0, 0.0, 1, std::ptr::null(), two.as_ptr(), out.as_mut_ptr()), 1);
            assert_eq!(flare_iknode_compute_s(0.0, 0.0, 1, one.as_ptr(), std::ptr::null(), out.as_mut_ptr()), 1);
            assert_eq!(flare_iknode_compute_s(0.0, 0.0, usize::MAX, one.as_ptr(), two.as_ptr(), out.as_mut_ptr()), 1);
            let mut x = 1.0;
            assert_eq!(flare_iknode_rotate_point(&mut x, std::ptr::null_mut(), 0.5), 1);
            assert_eq!(x, 1.0);
        }
    }

    #[test]
    fn rotate_point_ffi_in_place() {
        let (mut x, mut y) = (1.0f64, 0.0f64);
        assert_eq!(unsafe { flare_iknode_rotate_point(&mut x, &mut y, FRAC_PI_2) }, 0);
        assert!(x.abs() < 1e-15 && (y - 1.0).abs() < 1e-15);
    }
}
