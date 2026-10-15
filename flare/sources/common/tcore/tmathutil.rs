//! Port of common/tcore/tmathutil.cpp: LU decomposition, determinants, polynomial
//! root finding (Sturm sequences, closed forms), complex root solvers and the
//! Bezier-helper `quadraticRoot` / `cubicRoot`.  Structure, names and control flow
//! follow the C++ file; the C ABI at the bottom is what tmathutil.h forwards to.
//! (TMathException stays a C++ class: the C ABI returns a status the header turns
//! into the original `throw TMathException(...)`.)

use std::f64::consts::PI;

pub const EPSILON: f64 = 1e-8; // TConsts::epsilon
const M_2PI: f64 = 2.0 * PI;
pub const INFINITE_SOLUTIONS: i32 = -1;

/// maximum order for a polynomial
const MAX_ORDER: usize = 12;
/// smallest relative error we want
const RELERROR: f64 = 1.0e-14;
/// max power of 10 we wish to search to
const MAXPOW: i32 = 32;
/// max number of iterations
const MAXIT: i32 = 800;
/// a coefficient smaller than SMALL_ENOUGH is considered to be zero (0.0).
const SMALL_ENOUGH: f64 = 1.0e-12;

#[inline]
fn get_el(i: i32, j: i32, n: i32) -> usize {
    ((j - 1) + (i - 1) * n) as usize
}

#[inline]
fn sq(x: f64) -> f64 {
    x * x
}

#[inline]
fn tsign(x: f64) -> f64 {
    if x < 0.0 {
        -1.0
    } else if x > 0.0 {
        1.0
    } else {
        0.0
    }
}

pub fn is_almost_zero(val: f64) -> bool {
    -EPSILON < val && val < EPSILON
}

//-------------------------------------------------------------------------
// minimal std::complex<double> (libstdc++ semantics for the ops used)

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cplx {
    pub re: f64,
    pub im: f64,
}

impl Cplx {
    pub fn new(re: f64, im: f64) -> Cplx {
        Cplx { re, im }
    }
    fn real(re: f64) -> Cplx {
        Cplx { re, im: 0.0 }
    }
    fn add(self, o: Cplx) -> Cplx {
        Cplx::new(self.re + o.re, self.im + o.im)
    }
    fn sub(self, o: Cplx) -> Cplx {
        Cplx::new(self.re - o.re, self.im - o.im)
    }
    fn mul(self, o: Cplx) -> Cplx {
        Cplx::new(
            self.re * o.re - self.im * o.im,
            self.re * o.im + self.im * o.re,
        )
    }
    fn scale(self, k: f64) -> Cplx {
        Cplx::new(self.re * k, self.im * k)
    }
    fn div_real(self, k: f64) -> Cplx {
        Cplx::new(self.re / k, self.im / k)
    }
    fn neg(self) -> Cplx {
        Cplx::new(-self.re, -self.im)
    }
    fn sub_real(self, k: f64) -> Cplx {
        Cplx::new(self.re - k, self.im)
    }
    fn add_real(self, k: f64) -> Cplx {
        Cplx::new(self.re + k, self.im)
    }
    /// std::sqrt(complex)
    fn csqrt(self) -> Cplx {
        let (x, y) = (self.re, self.im);
        if x == 0.0 && y == 0.0 {
            return Cplx::new(0.0, y);
        }
        let t = ((x.abs() + x.hypot(y)) / 2.0).sqrt();
        if x >= 0.0 {
            Cplx::new(t, y / (2.0 * t))
        } else {
            Cplx::new(y.abs() / (2.0 * t), t.copysign(y))
        }
    }
    /// std::pow(complex, double)
    fn cpow(self, y: f64) -> Cplx {
        if self.im == 0.0 && self.re > 0.0 {
            return Cplx::real(self.re.powf(y));
        }
        let (lr, li) = (self.re.hypot(self.im).ln(), self.im.atan2(self.re));
        let rho = (y * lr).exp();
        let theta = y * li;
        Cplx::new(rho * theta.cos(), rho * theta.sin())
    }
}

fn is_almost_zero_c(v: Cplx) -> bool {
    v.re * v.re + v.im * v.im < EPSILON * EPSILON
}

//-------------------------------------------------------------------------

/// structure type for representing a polynomial
#[derive(Clone, Copy)]
struct Poly {
    ord: i32,
    coef: [f64; MAX_ORDER + 1],
}

impl Poly {
    fn zero() -> Poly {
        Poly { ord: 0, coef: [0.0; MAX_ORDER + 1] }
    }
}

fn convert_to_poly(v: &[f64], p: &mut Poly) {
    debug_assert!(v.len() <= MAX_ORDER);
    if v.len() > MAX_ORDER {
        return;
    }
    p.ord = v.len() as i32 - 1;
    p.coef[..v.len()].copy_from_slice(v);
}

/// convert(const poly &, vector<double> &): v.resize(p.ord); copy first p.ord coefs
fn convert_from_poly(p: &Poly, v: &mut Vec<f64>) {
    let n = p.ord.max(0) as usize;
    v.resize(n, 0.0);
    v.copy_from_slice(&p.coef[..n]);
}

/*
  modp: calculates the modulus of u(x) / v(x) leaving it in r, returns 0 if r(x)
  is a constant. Assumes the leading coefficient of v is 1 or -1.
*/
fn modp(u: &Poly, v: &Poly, r: &mut Poly) -> i32 {
    for i in 0..=(u.ord as usize) {
        r.coef[i] = u.coef[i];
    }

    if v.coef[v.ord as usize] < 0.0 {
        let mut k = u.ord - v.ord - 1;
        while k >= 0 {
            r.coef[k as usize] = -r.coef[k as usize];
            k -= 2;
        }

        let mut k = u.ord - v.ord;
        while k >= 0 {
            let mut j = v.ord + k - 1;
            while j >= k {
                r.coef[j as usize] = -r.coef[j as usize]
                    - r.coef[(v.ord + k) as usize] * v.coef[(j - k) as usize];
                j -= 1;
            }
            k -= 1;
        }
    } else {
        let mut k = u.ord - v.ord;
        while k >= 0 {
            let mut j = v.ord + k - 1;
            while j >= k {
                r.coef[j as usize] -= r.coef[(v.ord + k) as usize] * v.coef[(j - k) as usize];
                j -= 1;
            }
            k -= 1;
        }
    }

    let mut k = v.ord - 1;
    while k >= 0 && r.coef[k as usize].abs() < SMALL_ENOUGH {
        r.coef[k as usize] = 0.0;
        k -= 1;
    }

    r.ord = if k < 0 { 0 } else { k };

    r.ord
}

/*
  buildsturm: build up a sturm sequence for a polynomial in sseq, returning the
  index of the last polynomial in the sequence
*/
fn buildsturm(ord: i32, sseq: &mut [Poly]) -> i32 {
    sseq[0].ord = ord;
    sseq[1].ord = ord - 1;

    //  calculate the derivative and normalise the leadingcoefficient.
    let f = (sseq[0].coef[ord as usize] * ord as f64).abs();
    for i in 1..=(ord as usize) {
        sseq[1].coef[i - 1] = sseq[0].coef[i] * i as f64 / f;
    }

    // construct the rest of the Sturm sequence
    let mut sp = 2usize;
    loop {
        let u = sseq[sp - 2];
        let v = sseq[sp - 1];
        let mut r = sseq[sp];
        let m = modp(&u, &v, &mut r);
        sseq[sp] = r;
        if m == 0 {
            break;
        }
        //  reverse the sign and normalise
        let f = -sseq[sp].coef[sseq[sp].ord as usize].abs();
        let mut k = sseq[sp].ord;
        while k >= 0 {
            sseq[sp].coef[k as usize] /= f;
            k -= 1;
        }
        sp += 1;
    }

    sseq[sp].coef[0] = -sseq[sp].coef[0]; // reverse the sign

    sp as i32
}

/// number of distinct real roots of the polynomial described in sseq
fn numroots(np: i32, sseq: &[Poly], atneg: &mut i32, atpos: &mut i32) -> i32 {
    let mut atposinf = 0;
    let mut atneginf = 0;

    // change at positive infinity
    let mut lf = sseq[0].coef[sseq[0].ord as usize];

    for s in 1..=(np as usize) {
        let f = sseq[s].coef[sseq[s].ord as usize];
        if 0.0 == lf || lf * f < 0.0 {
            atposinf += 1;
        }
        lf = f;
    }

    // change at negative infinity
    if sseq[0].ord & 1 != 0 {
        lf = -sseq[0].coef[sseq[0].ord as usize];
    } else {
        lf = sseq[0].coef[sseq[0].ord as usize];
    }

    for s in 1..=(np as usize) {
        let f = if sseq[s].ord & 1 != 0 {
            -sseq[s].coef[sseq[s].ord as usize]
        } else {
            sseq[s].coef[sseq[s].ord as usize]
        };

        if 0.0 == lf || lf * f < 0.0 {
            atneginf += 1;
        }
        lf = f;
    }

    *atneg = atneginf;
    *atpos = atposinf;

    atneginf - atposinf
}

/// number of sign changes in the Sturm sequence in sseq at the value a.
fn numchanges(np: i32, sseq: &[Poly], a: f64) -> i32 {
    let mut changes = 0;

    let mut lf = evalpoly(sseq[0].ord, &sseq[0].coef, a);

    for s in 1..=(np as usize) {
        let f = evalpoly(sseq[s].ord, &sseq[s].coef, a);
        if lf == 0.0 || lf * f < 0.0 {
            changes += 1;
        }
        lf = f;
    }

    changes
}

/*
  sbisect: uses a bisection based on the sturm sequence for the polynomial
  described in sseq to isolate intervals in which roots occur, the roots are
  returned in the roots array in order of magnitude.
*/
fn sbisect(np: i32, sseq: &[Poly], mut min: f64, mut max: f64, atmin: i32, atmax: i32, roots: &mut [f64]) {
    let mut mid = 0.0;
    let mut n1 = 0;
    let mut n2;
    let mut its;
    let mut atmid;
    let _ = n1;

    if (atmin - atmax) == 1 {
        // first try a less expensive technique.
        let mut val = 0.0;
        if modrf(sseq[0].ord, &sseq[0].coef, min, max, &mut val) {
            roots[0] = val;
            return;
        }

        /*
         * if we get here we have to evaluate the root the hard
         * way by using the Sturm sequence.
         */
        its = 0;
        while its < MAXIT {
            mid = (min + max) / 2.0;

            atmid = numchanges(np, sseq, mid);

            if mid.abs() > RELERROR {
                if ((max - min) / mid).abs() < RELERROR {
                    roots[0] = mid;
                    return;
                }
            } else if (max - min).abs() < RELERROR {
                roots[0] = mid;
                return;
            }

            if (atmin - atmid) == 0 {
                min = mid;
            } else {
                max = mid;
            }
            its += 1;
        }

        if its == MAXIT {
            roots[0] = mid;
        }

        return;
    }

    /*
     * more than one root in the interval, we have to bisect...
     */
    its = 0;
    while its < MAXIT {
        mid = (min + max) / 2.0;

        atmid = numchanges(np, sseq, mid);

        n1 = atmin - atmid;
        n2 = atmid - atmax;

        if n1 != 0 && n2 != 0 {
            sbisect(np, sseq, min, mid, atmin, atmid, roots);
            sbisect(np, sseq, mid, max, atmid, atmax, &mut roots[n1 as usize..]);
            break;
        }

        if n1 == 0 {
            min = mid;
        } else {
            max = mid;
        }
        its += 1;
    }

    if its == MAXIT {
        n1 = atmax;
        while n1 < atmin {
            roots[(n1 - atmax) as usize] = mid;
            n1 += 1;
        }
    }
}

/// evalpoly: evaluate polynomial defined in coef returning its value.
fn evalpoly(ord: i32, coef: &[f64], x: f64) -> f64 {
    let mut f = coef[ord as usize];
    let mut i = ord - 1;
    while i >= 0 {
        f = x * f + coef[i as usize];
        i -= 1;
    }
    f
}

/*
  modrf: uses the modified regula-falsi method to evaluate the root in interval
  [a,b] of the polynomial described in coef. The root is returned in *val.
  Returns false if it can't converge.
*/
fn modrf(ord: i32, coef: &[f64], mut a: f64, mut b: f64, val: &mut f64) -> bool {
    let ecoef = ord as usize;

    let mut fb = coef[ecoef];
    let mut fa = fb;
    let mut p = ecoef as i32 - 1;
    while p >= 0 {
        fa = a * fa + coef[p as usize];
        fb = b * fb + coef[p as usize];
        p -= 1;
    }

    //  if there is no sign difference the method won't work
    if fa * fb > 0.0 {
        return false;
    }

    if fa.abs() < RELERROR {
        *val = a;
        return true;
    }

    if fb.abs() < RELERROR {
        *val = b;
        return true;
    }

    let mut lfx = fa;

    for _its in 0..MAXIT {
        let x = (fb * a - fa * b) / (fb - fa);

        let mut fx = coef[ecoef];
        let mut p = ecoef as i32 - 1;
        while p >= 0 {
            fx = x * fx + coef[p as usize];
            p -= 1;
        }

        if x.abs() > RELERROR {
            if (fx / x).abs() < RELERROR {
                *val = x;
                return true;
            }
        } else if fx.abs() < RELERROR {
            *val = x;
            return true;
        }

        if (fa * fx) < 0.0 {
            b = x;
            fb = fx;
            if (lfx * fx) > 0.0 {
                fa /= 2.0;
            }
        } else {
            a = x;
            fa = fx;
            if (lfx * fx) > 0.0 {
                fb /= 2.0;
            }
        }

        lfx = fx;
    }

    false
}

/// a x^2 + b x + c   (v[0] = c, v[1] = b, v[2] = a)
fn root_for_quadratic_equation(v: &[f64], sol: &mut Vec<f64>) -> i32 {
    if is_almost_zero(v[1]) {
        let mut q = -v[0] / v[2];
        if q < 0.0 {
            return 0;
        } else if is_almost_zero(q) {
            sol.push(0.0);
            return 1;
        }

        q = q.sqrt();
        sol.push(-q);
        sol.push(q);

        return 2;
    }

    let delta = sq(v[1]) - 4.0 * v[0] * v[2];

    if delta < 0.0 {
        return 0;
    }

    debug_assert!(v[2] != 0.0);

    if is_almost_zero(delta) {
        sol.push(-v[1] / (v[2] * 2.0));
        return 1;
    }

    let q = -0.5 * (v[1] + tsign(v[1]) * delta.sqrt());

    debug_assert!(q != 0.0);

    sol.push(v[0] / q);
    sol.push(q / v[2]);

    2
}

/// a x^3+b x^2 + c x + d   (p[0] = d, p[1] = c, p[2] = b, p[3] = a)
fn root_for_cubic_equation(p: &[f64], sol: &mut Vec<f64>) -> i32 {
    if is_almost_zero(p[0]) {
        let red_pol = [p[1], p[2], p[3]];

        let mut number_of_sol = root_for_quadratic_equation(&red_pol, sol);

        for i in 0..number_of_sol as usize {
            if 0.0 == sol[i] {
                return number_of_sol;
            }
        }

        // altrimenti devo contare la soluzione nulla
        number_of_sol += 1;
        sol.push(0.0);
        return number_of_sol;
    }

    let inv_v3 = 1.0 / p[3];
    let a = p[2] * inv_v3;
    let b = p[1] * inv_v3;
    let c = p[0] * inv_v3;

    let inv_3 = 1.0 / 3.0;
    let inv_9 = 1.0 / 9.0;
    let inv_54 = 1.0 / 54.0;

    let q = (sq(a) - 3.0 * b) * inv_9;
    let r = (2.0 * sq(a) * a - 9.0 * a * b + 27.0 * c) * inv_54;

    let r_2 = sq(r);
    let q_3 = sq(q) * q;

    if r_2 < q_3 {
        let q_sqrt = q.sqrt();
        let theta = (r / (q * q_sqrt)).acos();
        sol.push(-2.0 * q_sqrt * (theta * inv_3).cos() - a * inv_3);
        sol.push(-2.0 * q_sqrt * ((theta - M_2PI) * inv_3).cos() - a * inv_3);
        sol.push(-2.0 * q_sqrt * ((theta + M_2PI) * inv_3).cos() - a * inv_3);
        sol.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
        return 3;
    }

    let aa = -tsign(r) * ((r.abs() + (r_2 - q_3).sqrt()).powf(inv_3));
    let bb = if aa != 0.0 { q / aa } else { 0.0 };
    sol.push((aa + bb) - a * inv_3);
    1
}

fn root_for_greater_than_three_equation(p: &[f64], sol: &mut Vec<f64>) -> i32 {
    let mut sseq = [Poly::zero(); MAX_ORDER + 2];

    convert_to_poly(p, &mut sseq[0]);

    let np = buildsturm(p.len() as i32 - 1, &mut sseq);

    let mut atmin = 0;
    let mut atmax = 0;

    let nroot = numroots(np, &sseq, &mut atmin, &mut atmax);

    if nroot == 0 {
        return 0;
    }

    let mut min_val = -1.0;

    let mut nchanges = numchanges(np, &sseq, min_val);

    let mut i = 0;
    while nchanges != atmin && i != MAXPOW {
        min_val *= 10.0;
        nchanges = numchanges(np, &sseq, min_val);
        i += 1;
    }

    if nchanges != atmin {
        atmin = nchanges;
    }

    let mut max_val = 1.0;
    nchanges = numchanges(np, &sseq, max_val);

    i = 0;
    while nchanges != atmax && i != MAXPOW {
        max_val *= 10.0;
        nchanges = numchanges(np, &sseq, max_val);
        i += 1;
    }

    if nchanges != atmax {
        atmax = nchanges;
    }

    let nroot = atmin - atmax;

    debug_assert!(nroot > 0);

    let mut out_poly = Poly::zero();

    out_poly.ord = nroot;

    // (the original passes outPoly.coef, MAX_ORDER+1 doubles)
    sbisect(np, &sseq, min_val, max_val, atmin, atmax, &mut out_poly.coef);

    convert_from_poly(&out_poly, sol);

    if nroot < 0 {
        -1
    } else {
        nroot
    }
}

//-----------------------------------------------------------------------------

/// Err(()) is the original `throw TMathException("Singular matrix in routine tLUDecomposition\n")`
pub fn t_lu_decomposition(a: &mut [f64], n: i32, indx: &mut [i32], d: &mut f64) -> Result<(), ()> {
    let mut imax: i32 = 0;
    let mut vv = vec![0.0f64; n as usize];

    *d = 1.0;
    for i in 1..=n {
        let mut big = 0.0;
        for j in 1..=n {
            let temp = a[get_el(i, j, n)].abs();
            if temp > big {
                big = temp;
            }
        }

        if big == 0.0 {
            return Err(());
        }

        vv[(i - 1) as usize] = 1.0 / big;
    }

    for j in 1..=n {
        for i in 1..j {
            let mut sum = a[get_el(i, j, n)];
            for k in 1..i {
                sum -= a[get_el(i, k, n)] * a[get_el(k, j, n)];
            }
            a[get_el(i, j, n)] = sum;
        }
        let mut big = 0.0;
        for i in j..=n {
            let mut sum = a[get_el(i, j, n)];
            for k in 1..j {
                sum -= a[get_el(i, k, n)] * a[get_el(k, j, n)];
            }
            a[get_el(i, j, n)] = sum;
            let dum = vv[(i - 1) as usize] * sum.abs();
            if dum >= big {
                big = dum;
                imax = i;
            }
        }

        if j != imax {
            for k in 1..=n {
                let dum = a[get_el(imax, k, n)];
                a[get_el(imax, k, n)] = a[get_el(j, k, n)];
                a[get_el(j, k, n)] = dum;
            }
            *d = -*d;
            vv[(imax - 1) as usize] = vv[(j - 1) as usize];
        }

        indx[(j - 1) as usize] = imax;

        if a[get_el(j, j, n)] == 0.0 {
            a[get_el(j, j, n)] = EPSILON;
        }

        if j != n {
            let dum = 1.0 / a[get_el(j, j, n)];
            for i in (j + 1)..=n {
                a[get_el(i, j, n)] *= dum;
            }
        }
    }
    Ok(())
}

pub fn t_back_substitution(a: &[f64], n: i32, indx: &[i32], b: &mut [f64]) {
    let mut ii = 0;

    for i in 1..=n {
        let ip = indx[(i - 1) as usize];
        let mut sum = b[(ip - 1) as usize];
        b[(ip - 1) as usize] = b[(i - 1) as usize];
        if ii != 0 {
            for j in ii..=(i - 1) {
                sum -= a[get_el(i, j, n)] * b[(j - 1) as usize];
            }
        } else if sum != 0.0 {
            ii = i;
        }
        b[(i - 1) as usize] = sum;
    }
    let mut i = n;
    while i >= 1 {
        let mut sum = b[(i - 1) as usize];
        for j in (i + 1)..=n {
            sum -= a[get_el(i, j, n)] * b[(j - 1) as usize];
        }
        b[(i - 1) as usize] = sum / a[get_el(i, i, n)];
        i -= 1;
    }
}

/// tdet(LUa, n, d)
pub fn tdet_lu(lua: &[f64], n: i32, mut d: f64) -> f64 {
    for i in 1..=n {
        d *= lua[get_el(i, i, n)];
    }
    d
}

/// tdet(a, n)  (a is overwritten by its LU decomposition)
pub fn tdet(a: &mut [f64], n: i32) -> Result<f64, ()> {
    let mut d = 0.0;
    let mut indx = vec![0i32; n as usize];

    t_lu_decomposition(a, n, &mut indx, &mut d)?;
    for i in 1..=n {
        d *= a[get_el(i, i, n)];
    }

    Ok(d)
}

pub fn tsolve_sistem(a: &mut [f64], n: i32, res: &mut [f64]) -> Result<(), ()> {
    let mut d = 0.0;
    let mut indx = vec![0i32; n as usize];

    t_lu_decomposition(a, n, &mut indx, &mut d)?;
    debug_assert!(tdet_lu(a, n, d) != 0.0);
    t_back_substitution(a, n, &indx, res);
    Ok(())
}

/// Returns the C++ return value; `sol_touched` is false when the original returns
/// before `sol.clear()` (so the caller must leave `sol` as it was).
pub fn root_finding(in_poly: &[f64], sol: &mut Vec<f64>, sol_touched: &mut bool) -> i32 {
    // per ora risolvo solo i polinomi di grado al piu' pari a 3
    debug_assert!(in_poly.len() <= MAX_ORDER);
    *sol_touched = false;
    if in_poly.is_empty() || in_poly.len() > MAX_ORDER {
        return -1;
    }

    let mut p: Vec<f64> = in_poly.to_vec();

    // eat zero in poly
    while !p.is_empty() && is_almost_zero(*p.last().unwrap()) {
        p.pop();
    }

    sol.clear();
    *sol_touched = true;

    while !p.is_empty() && p[0] == 0.0 {
        sol.push(0.0);
        p.remove(0); // se i coefficienti bassi sono zero, ci sono soluzioni pari a 0.0.
    } // le metto, e abbasso il grado del polinomio(piu' veloce)

    match p.len() {
        0 => {
            if sol.is_empty() {
                return INFINITE_SOLUTIONS;
            }
        }
        1 => {} // no solutions
        2 => sol.push(-p[0] / p[1]),
        3 => {
            root_for_quadratic_equation(&p, sol);
        }
        4 => {
            root_for_cubic_equation(&p, sol);
        }
        _ => {
            root_for_greater_than_three_equation(&p, sol);
        }
    }
    sol.len() as i32
}

pub fn solve_equation2(roots: &mut [Cplx], a: f64, b: f64, c: f64) -> i32 {
    if is_almost_zero(a) {
        if is_almost_zero(b) {
            return 0;
        }
        roots[0] = Cplx::real(-c / b);
        return 1;
    }

    let dd = b * b - a * c * 4.0;
    let k = 0.5 / a;
    let d = if dd < 0.0 {
        Cplx::new(0.0, (-dd).sqrt())
    } else {
        Cplx::real(dd.sqrt())
    };
    roots[0] = Cplx::real(-b).sub(d).scale(k);
    roots[1] = Cplx::real(-b).add(d).scale(k);
    2
}

pub fn solve_equation3(roots: &mut [Cplx], a: f64, b: f64, c: f64, d: f64) -> i32 {
    if is_almost_zero(a) {
        return solve_equation2(roots, b, c, d);
    }

    // x = y - b/(3*a)
    // y*y*y + p*y + q = 0
    let p = (3.0 * a * c - b * b) / (3.0 * a * a);
    let q = (27.0 * a * a * d - 9.0 * a * b * c + 2.0 * b * b * b) / (27.0 * a * a * a);

    let qq = p * p * p / 27.0 + q * q / 4.0;
    let qs = if qq < 0.0 {
        Cplx::new(0.0, (-qq).sqrt())
    } else {
        Cplx::real(qq.sqrt())
    };
    let ca = Cplx::real(-q / 2.0).add(qs).cpow(1.0 / 3.0);
    let mut cb = Cplx::real(-q / 2.0).sub(qs).cpow(1.0 / 3.0);

    // choose complimentary B for A (A*B must be equal -p/3)
    let rot = Cplx::new(-0.5, 3.0f64.sqrt() / 2.0);
    if !is_almost_zero_c(ca.mul(cb).add_real(p / 3.0)) {
        cb = cb.mul(rot);
    }
    if !is_almost_zero_c(ca.mul(cb).add_real(p / 3.0)) {
        cb = cb.mul(rot);
    }

    let yy = ca.sub(cb).mul(Cplx::new(0.0, 3.0f64.sqrt()));
    let y0 = ca.add(cb);
    let y1 = y0.neg().sub(yy).div_real(2.0);
    let y2 = y0.neg().add(yy).div_real(2.0);

    let dd = b / (3.0 * a);
    roots[0] = y0.sub_real(dd);
    roots[1] = y1.sub_real(dd);
    roots[2] = y2.sub_real(dd);
    3
}

pub fn solve_equation4(roots: &mut [Cplx], a: f64, b: f64, c: f64, d: f64, e: f64) -> i32 {
    if is_almost_zero(a) {
        return solve_equation3(roots, b, c, d, e);
    }

    // x = y - b/(4*a)
    // y^4 + p*y^2 + q*y + r = 0
    let dd = b / (4.0 * a);
    let p = (8.0 * a * c - 3.0 * b * b) / (8.0 * a * a);
    let q = (8.0 * a * a * d - 4.0 * a * b * c + b * b * b) / (8.0 * a * a * a);
    let r = (256.0 * a * a * a * e - 64.0 * a * a * b * d + 16.0 * a * b * b * c
        - 3.0 * b * b * b * b)
        / (256.0 * a * a * a * a);

    if is_almost_zero(q) {
        // biquadratic equation
        // y^4 + p*y^2 + r = 0
        // handling this special case will give us more accurate results
        let mut y = [Cplx::real(0.0); 2];
        solve_equation2(&mut y, 1.0, p, r);
        y[0] = y[0].csqrt();
        y[1] = y[1].csqrt();
        roots[0] = y[0].neg().sub_real(dd);
        roots[1] = y[0].sub_real(dd);
        roots[2] = y[1].neg().sub_real(dd);
        roots[3] = y[1].sub_real(dd);
        return 4;
    }

    // solve cubic equation
    // z*z*z + (p/2)*z*z + ((p*p - 4*r)/16)*z - q*q/64 = 0
    let pp = p / 2.0;
    let qq = (p * p - 4.0 * r) / 16.0;
    let rr = -q * q / 64.0;
    let mut z = [Cplx::real(0.0); 3];
    solve_equation3(&mut z, 1.0, pp, qq, rr);

    z[0] = z[0].csqrt();
    z[1] = z[1].csqrt();
    z[2] = z[2].csqrt();

    // we need to find signs combination where following is valid:
    // (+-z0)*(+-z1)*(+-z2) = -q/8
    // magnitudes are always equals, we need to check signs only
    let zzz = z[0].mul(z[1]).mul(z[2]);
    if (zzz.re > 0.0) == (q > 0.0) {
        z[0] = z[0].neg();
    }

    roots[0] = z[0].sub(z[1]).sub(z[2]).sub_real(dd);
    roots[1] = z[0].neg().add(z[1]).sub(z[2]).sub_real(dd);
    roots[2] = z[0].neg().sub(z[1]).add(z[2]).sub_real(dd);
    roots[3] = z[0].add(z[1]).add(z[2]).sub_real(dd);
    4
}

/// number of roots of a poly in interval [min, max]; -1 if order > MAX_ORDER
pub fn number_of_roots_in_interval(mut order: i32, poly_h: &[f64], min: f64, max: f64) -> i32 {
    let mut sseq = [Poly::zero(); MAX_ORDER + 2];

    if order > MAX_ORDER as i32 {
        return -1;
    }

    while poly_h[order as usize] == 0.0 && order > 0 {
        order -= 1;
    }

    // init a sturm's sequence with our polynomious
    let mut i = order;
    while i >= 0 {
        sseq[0].coef[i as usize] = poly_h[i as usize];
        i -= 1;
    }

    //  build the Sturm sequence
    let np = buildsturm(order, &mut sseq);

    // compute number of variation on 0.0
    let nchanges_0 = numchanges(np, &sseq, min);

    let nchanges_1 = numchanges(np, &sseq, max);

    nchanges_0 - nchanges_1
}

pub fn quadratic_root(a: f64, b: f64, c: f64) -> f64 {
    // caso lineare
    if a.abs() < EPSILON {
        if b.abs() >= EPSILON {
            return -c / b;
        }
        return 1.0;
    }
    let bb = b * b;
    let mut q = bb - 4.0 * a * c;
    if q < 0.0 {
        return 1.0;
    }
    q = q.sqrt();
    if b < 0.0 {
        q = -q;
    }
    q = -0.5 * (b + q);
    let mut root1 = -1.0;
    let mut root2 = -1.0;
    if q.abs() >= EPSILON {
        root1 = c / q;
    }
    if a.abs() >= EPSILON {
        root2 = q / a;
    }
    if 0.0 - EPSILON <= root1 && root1 <= 1.0 + EPSILON {
        return root1;
    }
    if 0.0 - EPSILON <= root2 && root2 <= 1.0 + EPSILON {
        return root2;
    }
    1.0
}

pub fn cubic_root(a: f64, mut b: f64, mut c: f64, mut d: f64) -> f64 {
    /* Test for a quadratic or linear degeneracy */
    if a.abs() < EPSILON {
        return quadratic_root(b, c, d);
    }
    /* Normalize */
    b /= a;
    c /= a;
    d /= a;
    /* Compute discriminants */
    let q = (b * b - 3.0 * c) / 9.0;
    let qqq = q * q * q;
    let r = (2.0 * b * b * b - 9.0 * b * c + 27.0 * d) / 54.0;
    let rr = r * r;
    /* Three real roots */
    if rr < qqq {
        let theta = (r / qqq.sqrt()).acos();
        let mut root = [-2.0 * q.sqrt(); 3];
        root[0] *= (theta / 3.0).cos();
        root[1] *= ((theta + M_2PI) / 3.0).cos();
        root[2] *= ((theta - M_2PI) / 3.0).cos();
        root[0] -= b / 3.0;
        root[1] -= b / 3.0;
        root[2] -= b / 3.0;
        if 0.0 - EPSILON < root[0] && root[0] < 1.0 + EPSILON {
            return root[0];
        }
        if 0.0 - EPSILON < root[1] && root[1] < 1.0 + EPSILON {
            return root[1];
        }
        if 0.0 - EPSILON < root[2] && root[2] < 1.0 + EPSILON {
            return root[2];
        }
        1.0
    }
    /* One real root */
    else {
        let mut root = 0.0;
        let mut aa = -((r.abs() + (rr - qqq).sqrt()).powf(1.0 / 3.0));
        if aa != 0.0 {
            if r < 0.0 {
                aa = -aa;
            }
            root = aa + q / aa;
        }
        root -= b / 3.0;
        if 0.0 - EPSILON < root && root < 1.0 + EPSILON {
            return root;
        }
        1.0
    }
}

//=============================================================================
// C ABI (tmathutil.h inline wrappers call these)

use std::slice;

/// Returns 1 when the C++ side must `throw TMathException(...)` (singular matrix).
///
/// # Safety
/// `a` n*n doubles, `indx` n ints, `d` valid.
#[no_mangle]
pub unsafe extern "C" fn flare_tLUDecomposition(a: *mut f64, n: i32, indx: *mut i32, d: *mut f64) -> i32 {
    let nn = n.max(0) as usize;
    match t_lu_decomposition(
        slice::from_raw_parts_mut(a, nn * nn),
        n,
        slice::from_raw_parts_mut(indx, nn),
        &mut *d,
    ) {
        Ok(()) => 0,
        Err(()) => 1,
    }
}

/// # Safety
/// `a` n*n doubles, `indx` n ints, `b` n doubles.
#[no_mangle]
pub unsafe extern "C" fn flare_tbackSubstitution(a: *const f64, n: i32, indx: *const i32, b: *mut f64) {
    let nn = n.max(0) as usize;
    t_back_substitution(
        slice::from_raw_parts(a, nn * nn),
        n,
        slice::from_raw_parts(indx, nn),
        slice::from_raw_parts_mut(b, nn),
    );
}

/// Returns 1 when the C++ side must throw (singular matrix).
///
/// # Safety
/// `a` n*n doubles, `res` n doubles.
#[no_mangle]
pub unsafe extern "C" fn flare_tsolveSistem(a: *mut f64, n: i32, res: *mut f64) -> i32 {
    let nn = n.max(0) as usize;
    match tsolve_sistem(slice::from_raw_parts_mut(a, nn * nn), n, slice::from_raw_parts_mut(res, nn)) {
        Ok(()) => 0,
        Err(()) => 1,
    }
}

/// tdet(A, n); returns 1 when the C++ side must throw (singular matrix).
///
/// # Safety
/// `a` n*n doubles, `out` valid.
#[no_mangle]
pub unsafe extern "C" fn flare_tdet(a: *mut f64, n: i32, out: *mut f64) -> i32 {
    let nn = n.max(0) as usize;
    match tdet(slice::from_raw_parts_mut(a, nn * nn), n) {
        Ok(v) => {
            *out = v;
            0
        }
        Err(()) => 1,
    }
}

/// tdet(LUa, n, d)
///
/// # Safety
/// `lua` n*n doubles.
#[no_mangle]
pub unsafe extern "C" fn flare_tdet_lu(lua: *const f64, n: i32, d: f64) -> f64 {
    let nn = n.max(0) as usize;
    tdet_lu(slice::from_raw_parts(lua, nn * nn), n, d)
}

/// rootFinding. `out` receives the solutions (capacity `cap`, 16 is always enough);
/// `*out_count` is -1 when the C++ `sol` vector must be left untouched, otherwise
/// the new size of `sol`.  Returns the C++ return value.
///
/// # Safety
/// `poly` has `n` doubles, `out` has `cap` doubles, `out_count` valid.
#[no_mangle]
pub unsafe extern "C" fn flare_rootFinding(poly: *const f64, n: i32, out: *mut f64, cap: i32, out_count: *mut i32) -> i32 {
    // (std::vector::data() of an empty vector may be null)
    let input: &[f64] = if n > 0 { slice::from_raw_parts(poly, n as usize) } else { &[] };
    let mut sol = Vec::new();
    let mut touched = false;
    let rc = root_finding(input, &mut sol, &mut touched);
    if !touched {
        *out_count = -1;
        return rc;
    }
    let m = sol.len().min(cap.max(0) as usize);
    slice::from_raw_parts_mut(out, m).copy_from_slice(&sol[..m]);
    *out_count = m as i32;
    rc
}

/// `roots` is a `std::complex<double>[N]` (interleaved re, im), N >= 2.
///
/// # Safety
/// `roots` valid for 2 complex values.
#[no_mangle]
pub unsafe extern "C" fn flare_solveEquation2(roots: *mut Cplx, a: f64, b: f64, c: f64) -> i32 {
    solve_equation2(slice::from_raw_parts_mut(roots, 2), a, b, c)
}

/// # Safety
/// `roots` valid for 3 complex values.
#[no_mangle]
pub unsafe extern "C" fn flare_solveEquation3(roots: *mut Cplx, a: f64, b: f64, c: f64, d: f64) -> i32 {
    solve_equation3(slice::from_raw_parts_mut(roots, 3), a, b, c, d)
}

/// # Safety
/// `roots` valid for 4 complex values.
#[no_mangle]
pub unsafe extern "C" fn flare_solveEquation4(roots: *mut Cplx, a: f64, b: f64, c: f64, d: f64, e: f64) -> i32 {
    solve_equation4(slice::from_raw_parts_mut(roots, 4), a, b, c, d, e)
}

/// # Safety
/// `poly_h` has `order + 1` doubles (when order <= 12).
#[no_mangle]
pub unsafe extern "C" fn flare_numberOfRootsInInterval(order: i32, poly_h: *const f64, min: f64, max: f64) -> i32 {
    if order > MAX_ORDER as i32 {
        return -1;
    }
    number_of_roots_in_interval(order, slice::from_raw_parts(poly_h, (order + 1).max(0) as usize), min, max)
}

#[no_mangle]
pub extern "C" fn flare_cubicRoot(a: f64, b: f64, c: f64, d: f64) -> f64 {
    cubic_root(a, b, c, d)
}

#[no_mangle]
pub extern "C" fn flare_quadraticRoot(a: f64, b: f64, c: f64) -> f64 {
    quadratic_root(a, b, c)
}

//=============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn quadratic_root_cases() {
        // linear degeneracy: -c/b
        assert!(near(quadratic_root(0.0, 2.0, -1.0), 0.5));
        // a ~ 0 and b ~ 0 -> 1
        assert_eq!(quadratic_root(0.0, 0.0, 3.0), 1.0);
        // negative discriminant -> 1
        assert_eq!(quadratic_root(1.0, 0.0, 1.0), 1.0);
        // x^2 - x + 0.21: roots 0.3 / 0.7, first in range is c/q = 0.3
        assert!(near(quadratic_root(1.0, -1.0, 0.21), 0.3));
        // roots 2 and 3: none in [0,1] -> 1
        assert_eq!(quadratic_root(1.0, -5.0, 6.0), 1.0);
    }

    #[test]
    fn cubic_root_cases() {
        // degenerates to quadratic
        assert!(near(cubic_root(0.0, 1.0, -1.0, 0.21), 0.3));
        // x(x-.5)(x-1): first of the three roots in range is root[0] ~ 0
        assert!(cubic_root(1.0, -1.5, 0.5, 0.0).abs() < 1e-9);
        // one real root: x^3 - 0.125 -> 0.5
        assert!(near(cubic_root(1.0, 0.0, 0.0, -0.125), 0.5));
        // root outside [0,1] -> 1: x^3 + 8
        assert_eq!(cubic_root(1.0, 0.0, 0.0, 8.0), 1.0);
    }

    #[test]
    fn root_finding_low_degrees() {
        let mut sol = vec![];
        let mut t = false;
        assert_eq!(root_finding(&[], &mut sol, &mut t), -1);
        assert!(!t); // sol untouched
        assert_eq!(root_finding(&[0.0, 0.0], &mut sol, &mut t), INFINITE_SOLUTIONS);
        assert!(t); // cleared
        assert_eq!(root_finding(&[-2.0, 1.0], &mut sol, &mut t), 1);
        assert!(near(sol[0], 2.0));
        assert_eq!(root_finding(&[5.0], &mut sol, &mut t), 0); // constant: no roots
        // x^2 * 1: two zero roots pulled out first, then constant
        assert_eq!(root_finding(&[0.0, 0.0, 1.0], &mut sol, &mut t), 2);
        assert_eq!(sol, vec![0.0, 0.0]);
        // x^2 - 5x + 6
        assert_eq!(root_finding(&[6.0, -5.0, 1.0], &mut sol, &mut t), 2);
        assert!(near(sol[0], 2.0) && near(sol[1], 3.0));
        // x^2 - 4 : symmetric branch (b == 0)
        assert_eq!(root_finding(&[-4.0, 0.0, 1.0], &mut sol, &mut t), 2);
        assert!(near(sol[0], -2.0) && near(sol[1], 2.0));
        // x^2 + 1: no real roots
        assert_eq!(root_finding(&[1.0, 0.0, 1.0], &mut sol, &mut t), 0);
    }

    // the C++ `assert((int)in_poly.size() <= MAX_ORDER)` aborts debug builds; release returns -1
    #[cfg(not(debug_assertions))]
    #[test]
    fn root_finding_too_many_coefficients() {
        let mut sol = vec![];
        let mut t = false;
        assert_eq!(root_finding(&[1.0; 13], &mut sol, &mut t), -1);
    }

    #[test]
    fn root_finding_cubic_and_quartic() {
        let mut sol = vec![];
        let mut t = false;
        // (x-1)(x-2)(x-3)
        assert_eq!(root_finding(&[-6.0, 11.0, -6.0, 1.0], &mut sol, &mut t), 3);
        assert!(near(sol[0], 1.0) && near(sol[1], 2.0) && near(sol[2], 3.0));
        // x(x^2-1): zero root appended after the quadratic's roots
        assert_eq!(root_finding(&[0.0, -1.0, 0.0, 1.0], &mut sol, &mut t), 3);
        assert!(sol.iter().any(|v| near(*v, 0.0)));
        // one real root: x^3 - 8
        assert_eq!(root_finding(&[-8.0, 0.0, 0.0, 1.0], &mut sol, &mut t), 1);
        assert!(near(sol[0], 2.0));
        // Sturm path: (x-1)(x-2)(x-3)(x-4)
        assert_eq!(root_finding(&[24.0, -50.0, 35.0, -10.0, 1.0], &mut sol, &mut t), 4);
        let mut s = sol.clone();
        s.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for (r, e) in s.iter().zip([1.0, 2.0, 3.0, 4.0]) {
            assert!((r - e).abs() < 1e-6, "{:?}", s);
        }
    }

    #[test]
    fn number_of_roots() {
        // (x-1)(x-2)(x-3): 3 roots in [0,4], 1 in [0,1.5], 2 in [1.5,3.5]
        let p = [-6.0, 11.0, -6.0, 1.0];
        assert_eq!(number_of_roots_in_interval(3, &p, 0.0, 4.0), 3);
        assert_eq!(number_of_roots_in_interval(3, &p, 0.0, 1.5), 1);
        assert_eq!(number_of_roots_in_interval(3, &p, 1.5, 3.5), 2);
        assert_eq!(number_of_roots_in_interval(13, &p, 0.0, 1.0), -1);
        // leading zero coefficient is dropped
        let q = [-6.0, 11.0, -6.0, 1.0, 0.0];
        assert_eq!(number_of_roots_in_interval(4, &q, 0.0, 4.0), 3);
    }

    #[test]
    fn lu_solve_det() {
        // 2x + y = 3 ; x + 3y = 5  -> x = 0.8, y = 1.4
        let mut a = [2.0, 1.0, 1.0, 3.0];
        let mut b = [3.0, 5.0];
        tsolve_sistem(&mut a, 2, &mut b).unwrap();
        assert!(near(b[0], 0.8) && near(b[1], 1.4));
        let mut a = [2.0, 1.0, 1.0, 3.0];
        assert!(near(tdet(&mut a, 2).unwrap(), 5.0));
        let mut a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 10.0];
        assert!(near(tdet(&mut a, 3).unwrap(), -3.0));
        // all-zero row -> singular
        let mut z = [0.0; 4];
        let mut indx = [0; 2];
        let mut d = 0.0;
        assert!(t_lu_decomposition(&mut z, 2, &mut indx, &mut d).is_err());
        // a zero pivot after elimination is replaced by epsilon (no error)
        let mut s = [1.0, 1.0, 1.0, 1.0];
        assert!(t_lu_decomposition(&mut s, 2, &mut indx, &mut d).is_ok());
        assert_eq!(s[3], EPSILON);
    }

    #[test]
    fn complex_equations() {
        let mut r = [Cplx::real(0.0); 4];
        assert_eq!(solve_equation2(&mut r, 0.0, 0.0, 1.0), 0);
        assert_eq!(solve_equation2(&mut r, 0.0, 2.0, -1.0), 1);
        assert!(near(r[0].re, 0.5) && r[0].im == 0.0);
        // x^2 + 1 = 0 -> -i, +i
        assert_eq!(solve_equation2(&mut r, 1.0, 0.0, 1.0), 2);
        assert!(near(r[0].im, -1.0) && near(r[1].im, 1.0) && near(r[0].re, 0.0));
        // x^3 - 1 = 0: roots 1, -1/2 +- i sqrt(3)/2
        assert_eq!(solve_equation3(&mut r, 1.0, 0.0, 0.0, -1.0), 3);
        let has = |re: f64, im: f64, r: &[Cplx]| r.iter().any(|c| near(c.re, re) && near(c.im, im));
        assert!(has(1.0, 0.0, &r[..3]));
        assert!(has(-0.5, 3f64.sqrt() / 2.0, &r[..3]));
        assert!(has(-0.5, -(3f64.sqrt()) / 2.0, &r[..3]));
        // biquadratic x^4 - 5x^2 + 4: +-1, +-2
        assert_eq!(solve_equation4(&mut r, 1.0, 0.0, -5.0, 0.0, 4.0), 4);
        for e in [-2.0, -1.0, 1.0, 2.0] {
            assert!(has(e, 0.0, &r), "{} missing in {:?}", e, r);
        }
        // general quartic (x-1)(x-2)(x-3)(x-4)
        assert_eq!(solve_equation4(&mut r, 1.0, -10.0, 35.0, -50.0, 24.0), 4);
        for e in [1.0, 2.0, 3.0, 4.0] {
            assert!(r.iter().any(|c| (c.re - e).abs() < 1e-6 && c.im.abs() < 1e-6), "{} in {:?}", e, r);
        }
        // a ~ 0 falls through to the cubic solver
        assert_eq!(solve_equation4(&mut r, 0.0, 1.0, 0.0, 0.0, -1.0), 3);
    }

    #[test]
    fn ffi_root_finding_roundtrip() {
        let poly = [6.0, -5.0, 1.0];
        let mut out = [0.0; 16];
        let mut cnt = 0;
        let rc = unsafe { flare_rootFinding(poly.as_ptr(), 3, out.as_mut_ptr(), 16, &mut cnt) };
        assert_eq!((rc, cnt), (2, 2));
        assert!(near(out[0], 2.0) && near(out[1], 3.0));
        let rc = unsafe { flare_rootFinding(poly.as_ptr(), 0, out.as_mut_ptr(), 16, &mut cnt) };
        assert_eq!((rc, cnt), (-1, -1));
        assert!(near(flare_cubicRoot(1.0, 0.0, 0.0, -0.125), 0.5));
    }
}
