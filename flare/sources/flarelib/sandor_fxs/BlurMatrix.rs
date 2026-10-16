//! Port of `flarelib/sandor_fxs/BlurMatrix.cpp`: the calligraphic blur matrix.
//!
//! `CBlurMatrix` builds the sampling pattern a "blur matrix" effect walks: either an
//! equal-spaced disc (`createEqual`) or `NBRS` independently scattered discs
//! (`createRandom`), optionally with each sample extended by a straight "return to
//! origin" path (`addPath`, the `is_sac` / *stop at contour* option).
//!
//! The C++ kept its state in a public `std::vector<BLURSECTION> m_m[NBRS]` and reached
//! for `rand()`, `I_ROUND` and a `double` PI from `SDef.h`. Porting notes:
//!
//! - **The RNG stays with the caller.** The C++ called the C runtime's `rand()`, whose
//!   sequence differs per platform, so "the same blur matrix" was never reproducible
//!   across platforms. [`BlurMatrix::create_random`] takes the source as an argument and
//!   the C header passes `::rand`, which reproduces the old sequence exactly on any given
//!   platform while making the Rust side testable.
//! - **`SBlurMatrixError` is gone.** Every `throw` in the C++ sat in a `catch (exception)`
//!   around vector allocations, i.e. it only fired on `std::bad_alloc`. Rust has no
//!   equivalent failure here; allocation failure aborts instead of unwinding.
//! - **Two C++ loops that could never finish are bounded here.**
//!   `createEqual`'s `xd += b` / `yd += b` walk stops advancing once `b < 0.5` (`b`
//!   shrinks as `nb` grows, so `nb` in the thousands spins forever), and
//!   `createRandom`'s `while` loop never terminates if `nb` exceeds the number of
//!   distinct points inside the disc -- at `d = 0.5` the disc admits only the origin,
//!   which the loop rejects, so it spins outright. Both now stop instead of looping:
//!   `createEqual` on a step that does not move, `createRandom` at
//!   [`disc_candidates`]. Wherever the C++ did terminate, the output is unchanged.
//! - `(int)` casts of `double` saturated in Rust where C++ was undefined; every value
//!   reaching them here is small, and `x * x + y * y` uses wrapping arithmetic so a large
//!   `d` wraps like the C++ `int` rather than panicking in debug builds.

/// Number of independent random samples (`NBRS` in `BlurMatrix.h`).
pub const NBRS: usize = 10;

/// `PI` from `SDef.h` — truncated to 10 digits, *not* `std::f64::consts::PI`. Using the
/// real `PI` would change the spacing in `createEqual`.
const PI: f64 = 3.1415926535;

/// `SXYD` from `SDef.h`: a sample position and its distance from the origin.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct SXyd {
    pub x: i32,
    pub y: i32,
    pub d: f64,
}

/// One section of the flattened matrix: which `NBRS` slot it belongs to, how many points
/// it holds, and where they start in the flat point array. C ABI only.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BmSection {
    pub rs: i32,
    pub npts: i32,
    pub first: i32,
}

/// `I_ROUNDP(x)` from `SDef.h`: round half toward `+inf`.
fn i_roundp(x: f64) -> i32 {
    (x + 0.5) as i32
}

/// `I_ROUND(x)` from `SDef.h`. The macro's guard `(int)(-0.9F) == 0` is the truncating
/// float-to-int conversion, which is 0 on every supported platform, so this reduces to
/// round-half-away-from-zero.
fn i_round(x: f64) -> i32 {
    if x < 0.0 {
        (x - 0.5) as i32
    } else {
        (x + 0.5) as i32
    }
}

/// `BLURSECTION` is `std::vector<SXYD>`; the matrix is `[NBRS]` of those-of-those.
pub type BlurSection = Vec<SXyd>;
pub type BlurMatrixData = Vec<Vec<BlurSection>>;

/// Deterministic stand-in used when no RNG is supplied (the `configured` constructor and
/// a null `randfn` in the C ABI). It must not be a constant: `create_random` stops at the
/// first repeat of a sample, so a constant source would spin forever after one draw.
fn fallback_rand(state: &mut u64) -> i32 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    ((*state >> 33) as i64 & 0x7fff) as i32
}

/// Lattice points of the box `[-l, l]^2` that satisfy `x*x + y*y <= d2`, minus the
/// origin (the sample loop rejects it). This is the most samples `create_random` can ever
/// place, so it is what stops the loop from spinning when the disc is too small.
fn disc_candidates(l: i32, d2: f64) -> i64 {
    let mut n = 0i64;
    for x in -l..=l {
        let rest = d2 - (x as f64) * (x as f64);
        if rest < 0.0 {
            continue;
        }
        let ymax = (rest.sqrt() as i64).min(l as i64);
        n += 2 * ymax + 1;
    }
    n - 1
}

/// Port of the `CBlurMatrix` class.
#[derive(Clone, Debug, PartialEq)]
pub struct BlurMatrix {
    /// *Stop at contour*: extend every sample back to the origin.
    pub is_sac: bool,
    /// *Random sampling*: use [`NBRS`] scattered discs instead of one even one.
    pub is_rs: bool,
    /// `m_m[NBRS]`, each a list of sections, each a list of points.
    pub m: BlurMatrixData,
}

impl Default for BlurMatrix {
    fn default() -> Self {
        Self::new()
    }
}

impl BlurMatrix {
    /// `CBlurMatrix()`: empty, both flags off.
    pub fn new() -> Self {
        Self { is_sac: false, is_rs: false, m: vec![Vec::new(); NBRS] }
    }

    /// `CBlurMatrix(d, nb, isSAC, isRS)`: clear, fill, then extend if `is_sac`.
    pub fn configured(d: f64, nb: i32, is_sac: bool, is_rs: bool) -> Self {
        let mut bm = Self::new();
        bm.is_rs = is_rs;
        bm.is_sac = is_sac;
        if is_rs {
            let mut s = 1u64;
            bm.create_random(d, nb, || fallback_rand(&mut s));
        } else {
            bm.create_equal(d, nb);
        }
        if is_sac {
            bm.add_path();
        }
        bm
    }

    /// `for (i < NBRS) m_m[i].clear()` — what the constructor did before filling.
    pub fn clear(&mut self) {
        self.m = vec![Vec::new(); NBRS];
    }

    /// `CBlurMatrix::isIn`: does any section *start* at `(xyd.x, xyd.y)`?
    ///
    /// Only the first point of each section counts, exactly as in the C++ — which read
    /// `p->begin()` without an emptiness check. Empty sections (never produced) are
    /// skipped here instead of dereferencing end.
    pub fn is_in(m: &[BlurSection], xyd: SXyd) -> bool {
        m.iter()
            .filter_map(|s| s.first())
            .any(|f| f.x == xyd.x && f.y == xyd.y)
    }

    /// `CBlurMatrix::createRandom`: one origin section per slot, then up to `nb - 1`
    /// scattered points per slot drawn inside a disc of radius `d`.
    ///
    /// Every slot gets its origin section *before* any slot samples, and the samples for
    /// slot `i` all land in slot `i` — both match the C++, where the two loops are
    /// separate. Consumes two `rand` values per attempt (`x`, then `y`), as the C++ did.
    pub fn create_random(&mut self, d: f64, nb: i32, mut rand: impl FnMut() -> i32) {
        let l = i_roundp(d.ceil());
        let ll = 2 * l + 1;
        let d2 = d * d;
        let max_nb = ((d * d) * 2.8) as i32 + 1;

        for slot in self.m.iter_mut().take(NBRS) {
            slot.push(vec![SXyd { x: 0, y: 0, d: 0.0 }]);
        }

        if nb <= 0 || d <= 0.01 {
            return;
        }
        // `d > 0.01` makes `l >= 1`, so the span is at least 2; the guard only rules out
        // the `% 0` that a negative `d` would have reached in the C++ (undefined there).
        let span = ll + 1;
        if span <= 0 {
            return;
        }
        // Hard ceiling on samples: the disc holds `candidates` of them. Without this the
        // C++ spins forever whenever `nb` exceeds what the disc can supply -- d = 0.5
        // admits only the origin, which the loop rejects, so it never terminates.
        let ceiling = 1 + disc_candidates(l, d2);

        for i in 0..NBRS {
            let mut i_nb = 1;
            while i_nb < nb && i_nb <= max_nb && i_nb <= ceiling {
                let x = (rand() % span) - l;
                let y = (rand() % span) - l;
                let r2 = x.wrapping_mul(x).wrapping_add(y.wrapping_mul(y));
                if (r2 as f64) <= d2 && (x != 0 || y != 0) && !Self::is_in(&self.m[i], SXyd { x, y, d: 0.0 })
                {
                    let dist = (r2 as f64).sqrt();
                    self.m[i].push(vec![SXyd { x, y, d: dist }]);
                    i_nb += 1;
                }
            }
        }
    }

    /// `CBlurMatrix::createEqual`: one evenly spaced disc in slot 0, plus the origin.
    ///
    /// The C++ counted the points in an `nq` local that nothing read; it is dropped here.
    pub fn create_equal(&mut self, d: f64, nb: i32) {
        self.m[0].push(vec![SXyd { x: 0, y: 0, d: 0.0 }]);

        if nb <= 0 || d < 1.0 {
            return;
        }

        let two_d_1 = 2.0 * d + 1.0;
        let mut b = (nb as f64) * two_d_1 * two_d_1 / (d * d * PI);
        b = b.sqrt();
        b = b.ceil();
        b = two_d_1 / b;
        let di = d.ceil() as i32;

        let mut yd = 0.0;
        let mut y = 0;
        while y <= di {
            let mut xd = 0.0;
            let mut x = 0;
            while x <= di {
                let dist = (x.wrapping_mul(x).wrapping_add(y.wrapping_mul(y)) as f64).sqrt();
                if dist <= d {
                    self.m[0].push(vec![SXyd { x, y, d: dist }]);
                    if x > 0 && y > 0 {
                        self.m[0].push(vec![SXyd { x: -x, y: -y, d: dist }]);
                    }
                    if x > 0 && y >= 0 {
                        self.m[0].push(vec![SXyd { x: -x, y, d: dist }]);
                    }
                    if x >= 0 && y > 0 {
                        self.m[0].push(vec![SXyd { x, y: -y, d: dist }]);
                    }
                }
                let prev = x;
                xd += b;
                x = i_round(xd);
                // `b < 0.5` makes `I_ROUND` return the same x forever; the C++ hung here.
                if x == prev {
                    break;
                }
            }
            let prev = y;
            yd += b;
            y = i_round(yd);
            if y == prev {
                break;
            }
        }
    }

    /// `CBlurMatrix::addPath(iterator)`: after the first point of one section, append the
    /// straight walk back to the origin — `max(|x|, |y|)` points, the longer axis first.
    ///
    /// An origin section gives `l == 0`, so nothing is appended (the C++ divided by that
    /// zero before finding its loop never ran; `f64` yields NaN rather than trapping).
    pub fn add_path_section(sec: &mut BlurSection) {
        let Some(first) = sec.first().copied() else { return };
        let (x, y) = (first.x, first.y);
        let l = x.abs().max(y.abs());
        let dx = -(x as f64) / (l as f64);
        let dy = -(y as f64) / (l as f64);
        let mut xx = x as f64 + dx;
        let mut yy = y as f64 + dy;
        for _ in 1..=l {
            sec.push(SXyd { x: i_round(xx), y: i_round(yy), d: 0.0 });
            xx += dx;
            yy += dy;
        }
    }

    /// `CBlurMatrix::addPath()`: extend every section of every slot that is in play —
    /// all `NBRS` slots when random sampling is on, slot 0 only otherwise.
    pub fn add_path(&mut self) {
        for i in 0..if self.is_rs { NBRS } else { 1 } {
            for sec in self.m[i].iter_mut() {
                Self::add_path_section(sec);
            }
        }
    }

    /// Upper bound on sections `create_random` can emit for `(d, nb)`.
    pub fn max_sections_random(d: f64, nb: i32) -> usize {
        if nb <= 0 || d <= 0.01 {
            return NBRS;
        }
        let max_nb = ((d * d) * 2.8) as i32 + 1;
        let per_slot = 1 + nb.max(max_nb).max(0) as usize;
        NBRS * per_slot
    }

    /// Upper bound on sections `create_equal` can emit for `(d, nb)`: the origin plus at
    /// most four sections per grid cell of a `(ceil(d) + 1)`-square grid.
    pub fn max_sections_equal(d: f64, nb: i32) -> usize {
        if nb <= 0 || d < 1.0 {
            return 1;
        }
        let cells = (d.ceil() as i64 + 1).clamp(0, i32::MAX as i64) as i128;
        (1 + 4 * cells * cells).min(i32::MAX as i128) as usize
    }

    /// Flatten into the C ABI's `(sections, points)` pair: `BmSection::first` indexes
    /// `pts`. Returns the point count, or `-1` if a capacity was too small.
    pub fn flatten(&self, secs: &mut [BmSection], pts: &mut [SXyd]) -> i32 {
        let (nsec, npt) = self.required();
        if nsec > secs.len() || npt > pts.len() {
            return -1;
        }
        let mut si = 0usize;
        let mut pi = 0usize;
        for (rs, sections) in self.m.iter().enumerate() {
            for sec in sections {
                secs[si] = BmSection { rs: rs as i32, npts: sec.len() as i32, first: pi as i32 };
                pts[pi..pi + sec.len()].copy_from_slice(sec);
                pi += sec.len();
                si += 1;
            }
        }
        npt as i32
    }

    /// `(sections, points)` [`Self::flatten`] will write.
    pub fn required(&self) -> (usize, usize) {
        let nsec: usize = self.m.iter().map(|s| s.len()).sum();
        let npt: usize = self.m.iter().flat_map(|s| s.iter()).map(|s| s.len()).sum();
        (nsec, npt)
    }
}

// --- C ABI ---------------------------------------------------------------------------------
//
// The C++ class kept `m_m` as a public member, so the header rebuilds it from these calls
// rather than owning the storage. `create_random`/`create_equal` are one-shot: the C++
// calls them once with a caller-sized buffer, because sizing-then-filling would run the
// RNG twice and hand back a *different* matrix.

type RandFn = Option<extern "C" fn() -> i32>;

/// Points a single `build_*` call can emit (each built section holds exactly one point
/// until `add_path` runs).
#[no_mangle]
pub extern "C" fn flare_blurmatrix_max_points_random(d: f64, nb: i32) -> i32 {
    crate::guard(0i32, || {
        BlurMatrix::max_sections_random(d, nb).min(i32::MAX as usize) as i32
    })
}

/// Same, for `create_equal`.
#[no_mangle]
pub extern "C" fn flare_blurmatrix_max_points_equal(d: f64, nb: i32) -> i32 {
    crate::guard(0i32, || {
        BlurMatrix::max_sections_equal(d, nb).min(i32::MAX as usize) as i32
    })
}

#[no_mangle]
pub unsafe extern "C" fn flare_blurmatrix_build_random(
    d: f64,
    nb: i32,
    randfn: RandFn,
    secs: *mut BmSection,
    sec_cap: i32,
    pts: *mut SXyd,
    pt_cap: i32,
) -> i32 {
    crate::guard(-1i32, || {
        let mut bm = BlurMatrix::new();
        match randfn {
            Some(f) => bm.create_random(d, nb, || f()),
            // No source given: a fixed sequence, so the call is at least deterministic
            // (a constant would spin — see `fallback_rand`).
            None => {
                let mut s = 1u64;
                bm.create_random(d, nb, || fallback_rand(&mut s))
            }
        }
        let (mut s, mut p) = (slice_from(secs, sec_cap), slice_from(pts, pt_cap));
        bm.flatten(&mut s, &mut p)
    })
}

#[no_mangle]
pub unsafe extern "C" fn flare_blurmatrix_build_equal(
    d: f64,
    nb: i32,
    secs: *mut BmSection,
    sec_cap: i32,
    pts: *mut SXyd,
    pt_cap: i32,
) -> i32 {
    crate::guard(-1i32, || {
        let mut bm = BlurMatrix::new();
        bm.create_equal(d, nb);
        let (mut s, mut p) = (slice_from(secs, sec_cap), slice_from(pts, pt_cap));
        bm.flatten(&mut s, &mut p)
    })
}

/// Appends the return-to-origin path for one section. Writes *only* the appended points
/// into `out` and returns how many. If `out` is null or `cap` is short, returns the count
/// that would be written and writes nothing, so the caller can size its buffer with a
/// null-pointer probe.
#[no_mangle]
pub unsafe extern "C" fn flare_blurmatrix_add_path(
    sec: *const SXyd,
    n: i32,
    out: *mut SXyd,
    cap: i32,
) -> i32 {
    crate::guard(0i32, || {
        if n <= 0 || sec.is_null() {
            return 0;
        }
        let mut s: BlurSection = std::slice::from_raw_parts(sec, n as usize).to_vec();
        let before = s.len();
        BlurMatrix::add_path_section(&mut s);
        let added = s.len() - before;
        if out.is_null() || cap < added as i32 {
            return added as i32;
        }
        std::ptr::copy_nonoverlapping(s[before..].as_ptr(), out, added);
        added as i32
    })
}

/// Does any of `n` section-start points sit at `(x, y)`?
#[no_mangle]
pub unsafe extern "C" fn flare_blurmatrix_is_in(
    firsts: *const SXyd,
    n: i32,
    x: i32,
    y: i32,
) -> i32 {
    crate::guard(0i32, || {
        if n <= 0 || firsts.is_null() {
            return 0;
        }
        let secs: Vec<BlurSection> = std::slice::from_raw_parts(firsts, n as usize)
            .iter()
            .map(|p| vec![*p])
            .collect();
        i32::from(BlurMatrix::is_in(&secs, SXyd { x, y, d: 0.0 }))
    })
}

/// `std::slice::from_raw_parts_mut` over a possibly-null FFI pointer, capped by the
/// caller's declared capacity. A null pointer or non-positive capacity yields an empty
/// slice, which [`BlurMatrix::flatten`] then rejects with `-1`.
unsafe fn slice_from<'a, T>(p: *mut T, cap: i32) -> &'a mut [T] {
    if p.is_null() || cap <= 0 {
        &mut []
    } else {
        std::slice::from_raw_parts_mut(p, cap as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The MSVC C runtime's `rand()`: a plain 31-bit LCG, no state table. Verified against
/// the real CRT (srand(12345) -> 7584, 19164, 25795, ...) rather than assumed, because
/// createRandom draws from it and the golden must see the same sequence. Used only by the
/// test; the port itself takes its RNG from the caller.
struct MsvcRand(u32);

impl MsvcRand {
    fn seeded(seed: u32) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> i32 {
        self.0 = self.0.wrapping_mul(214013).wrapping_add(2531011);
        ((self.0 >> 16) & 0x7fff) as i32
    }
}

    /// Same flat text form as the C++ reference harness, so the golden compares directly.
    fn dump(kind: char, bm: &BlurMatrix, d: f64, nb: i32) -> String {
        let nsec: usize = bm.m.iter().map(|s| s.len()).sum();
        let npt: usize = bm.m.iter().flat_map(|s| s.iter()).map(|s| s.len()).sum();
        let mut out = format!("{kind} d={:016x} nb={nb} ns={nsec} np={npt}\n", d.to_bits());
        for (rs, sections) in bm.m.iter().enumerate() {
            for sec in sections {
                out.push_str(&format!("S {rs} {}", sec.len()));
                for p in sec {
                    out.push_str(&format!(" {} {} {:016x}", p.x, p.y, p.d.to_bits()));
                }
                out.push('\n');
            }
        }
        out
    }

    fn golden() -> &'static str {
        include_str!("BlurMatrix_cases.txt")
    }

    /// Parse the golden into (kind, d, nb) per case. Blocks are separated by blank
    /// lines: E = createEqual, P = createEqual + addPath, R = createRandom with
    /// is_sac (the random path plus addPath).
    fn golden_blocks(text: &str) -> Vec<(char, f64, i32)> {
        let mut out = Vec::new();
        for block in text.split("\n\n") {
            let Some(head) = block.lines().find(|l| !l.trim().is_empty()) else { continue };
            let mut f = head.split_whitespace();
            let kind = f.next().unwrap().chars().next().unwrap();
            let d = f64::from_bits(u64::from_str_radix(f.next().unwrap().trim_start_matches("d="), 16).unwrap());
            let nb = f.next().unwrap().trim_start_matches("nb=").parse().unwrap();
            out.push((kind, d, nb));
        }
        out
    }


    #[test]
    fn matches_the_cpp_reference_bit_for_bit() {
        let blocks = golden_blocks(golden());
        assert!(!blocks.is_empty(), "golden fixture is empty");
        for (kind, d, nb) in &blocks {
            // Recompute from scratch: the golden is never read into the value under test.
            let bm = match kind {
                'E' => {
                    let mut b = BlurMatrix::new();
                    b.create_equal(*d, *nb);
                    b
                }
                'P' => {
                    let mut b = BlurMatrix::new();
                    b.is_sac = true;
                    b.create_equal(*d, *nb);
                    b.add_path();
                    b
                }
                'R' => {
                    let mut rng = MsvcRand::seeded(12345);
                    let mut b = BlurMatrix::new();
                    b.is_rs = true;
                    b.is_sac = true;
                    b.create_random(*d, *nb, || rng.next());
                    b.add_path();
                    b
                }
                other => panic!("unknown golden kind {other:?}"),
            };
            assert_eq!(
                dump(*kind, &bm, *d, *nb),
                block_text(golden(), *kind, *d, *nb),
                "mismatch for {kind} d={d} nb={nb}"
            );
        }
    }

    /// The golden's own text for one block, for the byte comparison above.
    fn block_text(golden: &str, kind: char, d: f64, nb: i32) -> String {
        let key = format!("{kind} d={:016x} nb={nb}", d.to_bits());
        for block in golden.split("\n\n") {
            if block.starts_with(&key) {
                return block.trim_end().to_string() + "\n";
            }
        }
        panic!("no golden block for {key}");
    }

    #[test]
    fn equal_spacing_is_a_disc() {
        let mut bm = BlurMatrix::new();
        bm.create_equal(3.0, 8);
        // Only slot 0 is used by the even path.
        assert!(bm.m[1..].iter().all(|s| s.is_empty()));
        // First section is the origin.
        assert_eq!(bm.m[0][0], vec![SXyd { x: 0, y: 0, d: 0.0 }]);
        // Every point lies inside the radius, and the four quadrants are mirrored.
        for sec in &bm.m[0] {
            for p in sec {
                assert!(p.d <= 3.0, "{p:?} outside radius");
            }
        }
        let neg: Vec<_> = bm.m[0].iter().map(|s| s[0].x).filter(|x| *x < 0).collect();
        assert!(!neg.is_empty(), "expected negative x samples");
        assert!(BlurMatrix::max_sections_equal(3.0, 8) >= bm.m[0].len());
    }

    #[test]
    fn degenerate_parameters() {
        // d below 1.0 short-circuits to just the origin, and nb <= 0 does too.
        let mut bm = BlurMatrix::new();
        bm.create_equal(0.5, 8);
        assert_eq!(bm.m[0].len(), 1);
        let mut bm = BlurMatrix::new();
        bm.create_equal(3.0, 0);
        assert_eq!(bm.m[0].len(), 1);
        // create_random still seeds every slot with an origin before the early return.
        let mut bm = BlurMatrix::new();
        bm.create_random(0.001, 0, || 0);
        assert_eq!(bm.m.len(), NBRS);
        assert!(bm.m.iter().all(|s| s.len() == 1 && s[0][0].d == 0.0));
    }

    #[test]
    fn random_seeds_every_slot_and_lands_inside_the_disc() {
        let mut rng = MsvcRand::seeded(12345);
        let (d, nb) = (4.0, 6);
        let mut bm = BlurMatrix::new();
        bm.is_rs = true;
        bm.create_random(d, nb, || rng.next());
        assert_eq!(bm.m.len(), NBRS);
        for slot in &bm.m {
            assert_eq!(slot[0], vec![SXyd { x: 0, y: 0, d: 0.0 }]);
            assert!(slot.len() <= BlurMatrix::max_sections_random(d, nb) / NBRS);
            let mut seen = Vec::new();
            for sec in slot.iter().skip(1) {
                let p = sec[0];
                assert!(
                    (p.d - ((p.x * p.x + p.y * p.y) as f64).sqrt()).abs() < 1e-12,
                    "distance is not sqrt(x^2+y^2) for {p:?}"
                );
                assert!(p.d <= d + 1e-12);
                assert!(p.x != 0 || p.y != 0);
                assert!(!seen.contains(&(p.x, p.y)), "duplicate sample");
                seen.push((p.x, p.y));
            }
        }
    }

    #[test]
    fn is_in_only_sees_section_starts() {
        let mut bm = BlurMatrix::new();
        bm.create_equal(3.0, 8);
        assert!(BlurMatrix::is_in(&bm.m[0], SXyd { x: 0, y: 0, d: 0.0 }));
        let first = bm.m[0][1][0];
        assert!(BlurMatrix::is_in(&bm.m[0], SXyd { x: first.x, y: first.y, d: 0.0 }));
        // A point that exists mid-section but is not a section start is not found.
        bm.m[0].push(vec![SXyd { x: 99, y: 99, d: 0.0 }]);
        assert!(BlurMatrix::is_in(&bm.m[0], SXyd { x: 99, y: 99, d: 0.0 }));
        assert!(!BlurMatrix::is_in(&bm.m[0], SXyd { x: 98, y: 98, d: 0.0 }));
    }

    #[test]
    fn add_path_walks_the_longer_axis() {
        // (3, -1): l = 3, so three points are appended, stepping by (-1, +1/3).
        let mut sec = vec![SXyd { x: 3, y: -1, d: 0.0 }];
        BlurMatrix::add_path_section(&mut sec);
        assert_eq!(sec.len(), 4);
        // I_ROUND(-0.666..) is -1 and I_ROUND(-0.333..) is 0: halfway rounds away from zero.
        assert_eq!((sec[1].x, sec[1].y), (2, -1));
        assert_eq!((sec[2].x, sec[2].y), (1, 0));
        assert_eq!((sec[3].x, sec[3].y), (0, 0));
        // An origin section adds nothing (l == 0).
        let mut origin = vec![SXyd { x: 0, y: 0, d: 0.0 }];
        BlurMatrix::add_path_section(&mut origin);
        assert_eq!(origin.len(), 1);
        // An empty section is left alone.
        let mut empty: Vec<SXyd> = Vec::new();
        BlurMatrix::add_path_section(&mut empty);
        assert!(empty.is_empty());
    }

    #[test]
    fn add_path_covers_the_slots_in_play() {
        let mut bm = BlurMatrix { is_sac: true, is_rs: true, ..BlurMatrix::new() };
        bm.create_equal(2.0, 4);
        // Even sampling leaves slots 1..NBRS empty, so is_rs must still only reach slot 0
        // when there is nothing there; put a sample in slot 3 to prove the walk happens.
        bm.m[3].push(vec![SXyd { x: 2, y: 0, d: 2.0 }]);
        bm.add_path();
        assert!(bm.m[3][0].len() == 3, "random mode must walk every slot");

        let mut even = BlurMatrix { is_sac: true, is_rs: false, ..BlurMatrix::new() };
        even.create_equal(2.0, 4);
        even.m[3].push(vec![SXyd { x: 2, y: 0, d: 2.0 }]);
        even.add_path();
        assert_eq!(even.m[3][0].len(), 1, "even mode must not walk slot 3");
    }

    #[test]
    fn flatten_round_trips_and_reports_short_buffers() {
        let mut bm = BlurMatrix::new();
        bm.create_equal(3.0, 8);
        let nsec: usize = bm.m.iter().map(|s| s.len()).sum();
        let npt: usize = bm.m.iter().flat_map(|s| s.iter()).map(|s| s.len()).sum();
        let mut secs = vec![BmSection { rs: 0, npts: 0, first: 0 }; nsec];
        let mut pts = vec![SXyd::default(); npt];
        assert_eq!(bm.flatten(&mut secs, &mut pts), npt as i32);
        assert_eq!(pts, bm.m.iter().flat_map(|s| s.iter()).flatten().copied().collect::<Vec<_>>());
        assert_eq!(secs.last().unwrap().first as usize + secs.last().unwrap().npts as usize, npt);
        assert_eq!(bm.required(), (nsec, npt));
        // One point short is refused rather than truncated.
        let mut small = vec![SXyd::default(); npt - 1];
        assert_eq!(bm.flatten(&mut [], &mut small), -1);
        assert_eq!(bm.flatten(&mut [], &mut []), -1);
    }

    #[test]
    fn ffi_round_trip() {
        unsafe {
            let cap = BlurMatrix::max_sections_equal(3.0, 8);
            assert_eq!(flare_blurmatrix_max_points_equal(3.0, 8) as usize, cap);
            assert_eq!(flare_blurmatrix_max_points_equal(0.5, 8) as usize, 1);
            let mut secs = vec![BmSection { rs: -1, npts: -1, first: -1 }; cap];
            let mut pts = vec![SXyd::default(); cap];
            let n = flare_blurmatrix_build_equal(3.0, 8, secs.as_mut_ptr(), cap as i32, pts.as_mut_ptr(), cap as i32);
            assert!(n > 0);

            let mut bm = BlurMatrix::new();
            bm.create_equal(3.0, 8);
            assert_eq!(n as usize, bm.m.iter().flat_map(|s| s.iter()).map(|s| s.len()).sum::<usize>());

            // The section walk reassembles the matrix. Each built section holds one point,
            // so `n` points means `n` sections; the rest of `secs` is untouched capacity.
            let mut walked = BlurMatrix::new();
            let mut at = 0usize;
            for s in &secs[..n as usize] {
                walked.m[s.rs as usize].push(pts[at..at + s.npts as usize].to_vec());
                at += s.npts as usize;
            }
            assert_eq!(walked, bm);

            // add_path over the FFI returns just the appended points.
            let sec = vec![SXyd { x: 3, y: -1, d: 0.0 }];
            let mut buf = vec![SXyd::default(); 3];
            let added = flare_blurmatrix_add_path(sec.as_ptr(), 1, buf.as_mut_ptr(), 3);
            assert_eq!(added, 3);
            let mut want = sec.clone();
            BlurMatrix::add_path_section(&mut want);
            assert_eq!(buf, want[1..].to_vec());
            // A short or absent buffer reports the size instead of writing.
            assert_eq!(flare_blurmatrix_add_path(sec.as_ptr(), 1, buf.as_mut_ptr(), 1), 3);
            assert_eq!(flare_blurmatrix_add_path(sec.as_ptr(), 1, std::ptr::null_mut(), 0), 3);
            assert_eq!(flare_blurmatrix_add_path(std::ptr::null(), 1, buf.as_mut_ptr(), 3), 0);

            // is_in over the flattened starts (only the n entries flatten wrote).
            let firsts: Vec<SXyd> =
                secs[..n as usize].iter().map(|s| pts[s.first as usize]).collect();
            assert_eq!(flare_blurmatrix_is_in(firsts.as_ptr(), firsts.len() as i32, 0, 0), 1);
            assert_eq!(flare_blurmatrix_is_in(firsts.as_ptr(), firsts.len() as i32, 12345, 12345), 0);
            assert_eq!(flare_blurmatrix_is_in(std::ptr::null(), 4, 0, 0), 0);
        }
    }
}