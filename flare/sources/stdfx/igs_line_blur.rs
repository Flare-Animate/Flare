//! Port of stdfx/igs_line_blur.cpp: "Line Blur" (ino_line_blur): blur an image along the
//! thin lines found in one of its channels, or smudge it along them with a round brush.
//!
//! Pipeline (all of it stays in this file, the C++ was one anonymous namespace):
//! 1. the reference channel (alpha) is copied into a 16-bit plane (`thinnest_ui16_image`):
//!    isolated bright pixels are filled, the plane is binarised at 0x4000 and thinned to
//!    one-pixel wide lines by eating the four sides in turn (at most 100 rounds, else the
//!    C++ threw). The optional 2x scale-up (`exec02`/`exec03`, `xd/yd >= 2`) is never used
//!    by `convert` (the divisors stay 1) but is ported and covered by the golden;
//! 2. the thinned pixels become a point list (`pixel_point_root`), the points are linked
//!    to their 4 nearest neighbours, grouped into lines (end point -> end point or branch,
//!    branches continue along the straightest way), lines shorter than 3 points are
//!    dropped, a middle point is chosen, the body is smoothed `v_smooth_retry` times, each
//!    open end is extended by mirroring the body about the end point's bisector, the
//!    extensions are bent towards the end points of nearby lines (`same way`), smoothed
//!    again, and bounding boxes are taken;
//! 3. every pixel (or sub-pixel) picks the nearest lines and averages the source along
//!    them (`Curve Blur`), or a round brush is dragged along each line (`Smudge Brush`).
//!
//! The C++ linked everything with raw pointers; here the nodes live in index arenas
//! (`List<T>` reproduces `list_root::push/pop/_connect` link for link), `usize::MAX` is
//! `NULL`. Loop order, `int`/`double` promotion and every float expression are kept
//! (`-ffp-contract=off` goldens): the only libm calls are `atan`, `sin`, `cos` and `pow`
//! (same functions as the C++, so bit-identical on the same libm).
//!
//! Kept as in the C++: the right neighbour of the last pixel of a row, and the row below
//! the last row, are the following memory (next row's first sample / zeros) in the noise
//! fill and the thinning; `get_d_radian`'s "zero vector" warning on stderr; the curve blur
//! samples `(int)(x + 0.5)` (truncation, not floor); `assert()`s are compiled out as in a
//! Release build (the `assert(0)` after "bad link" therefore keeps looping until the
//! "too long link" message, exactly like NDEBUG).
//!
//! Diagnostics: the `mv_sw/pv_sw/cv_sw` switches print the same lines to stdout (progress
//! bar, `#  <ctime>  message`) and `debug_save_sw` writes the same `tmpNN_*.txt` files into
//! the current directory; the warnings/errors go to stderr with a BEL. They are
//! unreachable from the app (`ino_line_blur` passes `false`) but are ported and tested.
//! Not ported because nothing calls them: `pixel_point_root::save`,
//! `brush_curve_blur::save`, `pixel_select_curve_blur_root::save`,
//! `pixel_point_node::print_xy_around`, `set_cp_title`, and the verbose switches of the two
//! select roots (their setters are never called, so they stay off).
//!
//! Deviations (only where the C++ was undefined behaviour or could not finish):
//! * `height/width < 0`, `wrap < width`, `channels` outside 1..=4 (the C++ wrote past its
//!   4-channel scratch line), `reference_channel` outside `0..channels` (it read the next
//!   pixel / past the buffer), null buffers: refused as `BAD_ARG` (`std::invalid_argument`
//!   text); a bad `bits` is the original `domain_error`;
//! * anything that would have dereferenced `NULL` or read/written outside an array (e.g. a
//!   smudge window larger than its scratch) is a Rust panic, caught and reported as
//!   `INTERNAL` instead of crashing; `int` overflow in the scratch-size products and
//!   allocation failure are the original `mem_alloc() returns NG`;
//! * `get_near_point` of a line farther than 1000 px from the pixel (with
//!   `b_blur_near_len` = 1000) left the position/point uninitialised: the line is skipped;
//! * `get_d_radian` of NaN components (no quadrant matched, result uninitialised) is 0;
//!   `static_cast<T>(double)` saturates (NaN -> 0) instead of being UB;
//! * image samples are read/written with unaligned accesses (no alignment requirement),
//!   sample offsets are computed in 64 bit; local time for the log prefix is UTC on
//!   non-unix hosts; a failure while tearing down after an error prints the
//!   `mem_free()` lines in the destructor order of the C++ (it did too);
//! * the stdout lines go through Rust's stdout (flushed after every message) instead of
//!   C stdio, so the interleaving with other C++ stdout writers is per message.
//!
//! C ABI: `flare_igs_line_blur_convert` (status codes below) and
//! `flare_igs_line_blur_message`; `igs_line_blur.h` keeps the C++ signature as an inline
//! forwarder and throws `std::domain_error(message)` for every non-zero status.

use std::cell::Cell;
use std::f64::consts::PI;
use std::ffi::c_void;
use std::io::Write as _;
use std::os::raw::{c_char, c_int};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------------------
// status codes

pub const OK: i32 = 0;
/// `std::invalid_argument`-like refusal of an argument the C++ would have run wild on.
pub const BAD_ARG: i32 = 1;
/// A Rust panic where the C++ was undefined behaviour.
pub const INTERNAL: i32 = 2;
pub const THIN_ALLOC: i32 = 3;
pub const BAD_BITS: i32 = 4;
pub const THIN_EXEC: i32 = 5;
pub const POINT_LIST: i32 = 6;
pub const LINE_EXEC: i32 = 7;
pub const SAVE_LINES: i32 = 8;
pub const SAVE_ONE: i32 = 9;
pub const SAVE_ANOTHER: i32 = 10;
pub const SAVE_NOT_INCLUDE: i32 = 11;
pub const SAVE_MIDDLE: i32 = 12;
pub const EXPAND_LINES: i32 = 13;
pub const SAVE_EXPAND_VECTOR: i32 = 14;
pub const SAVE_EXPAND_LINES: i32 = 15;
pub const SAVE_ONE_EXPAND: i32 = 16;
pub const SAVE_ANOTHER_EXPAND: i32 = 17;
pub const CURVE_ALLOC: i32 = 18;
pub const SMUDGE_ALLOC: i32 = 19;
pub const OVER_POINTS: i32 = 20;
pub const OVER_POINTS_FRONT: i32 = 21;
pub const OVER_LINES: i32 = 22;

const MESSAGES: [&[u8]; 23] = [
    b"\0",
    b"igs::line_blur::convert: bad argument\0",
    b"igs::line_blur::convert: internal error (invalid state)\0",
    b"Error : cl_thinnest_ui16_image.mem_alloc() returns NG\0",
    b"Error : bits is not 8 or 16\0",
    b"Error : cl_thinnest_ui16_image.exec05_thin() returns NG\0",
    b"Error : cl_pixel_point_root.alloc_mem_and_list_node() returns NG\0",
    b"Error : cl_pixel_line_root.exec01020304() returns NG\0",
    b"Error : cl_pixel_line_root.save_lines(-) returns NG\0",
    b"Error : cl_pixel_line_root.save_one_point(-) returns NG\0",
    b"Error : cl_pixel_line_root.save_another_point(-) returns NG\0",
    b"Error : cl_pixel_line_root.save_not_include(-) returns NG\0",
    b"Error : cl_pixel_line_root.save_middle_point(-) returns NG\0",
    b"Error : cl_pixel_line_root.exec08_expand_lines(-) returns NG\0",
    b"Error : cl_pixel_line_root.save_expand_vector(-) returns NG\0",
    b"Error : cl_pixel_line_root.save_expand_lines(-) returns NG\0",
    b"Error : cl_pixel_line_root.save_one_expand_point(-) returns NG\0",
    b"Error : cl_pixel_line_root.save_another_expand_point(-) returns NG\0",
    b"Error : cl_brush_curve_blur.mem_alloc() returns NG\0",
    b"Error : cl_brush_smudge_circle.mem_alloc() returns NG\0",
    b"Error : over clp_line->get_i32_point_count()\0",
    b"Error : over clp_line->get_i32_point_count() going front\0",
    b"Error : over cl_pixel_line_root.get_i32_count()\0",
];

/// The `std::domain_error` text of a status (empty for `OK`).
pub fn message(status: i32) -> &'static str {
    let s = MESSAGES.get(status as usize).copied().unwrap_or(MESSAGES[INTERNAL as usize]);
    std::str::from_utf8(&s[..s.len() - 1]).unwrap_or("")
}

type Ng = ();
type Res<T = ()> = Result<T, Ng>;

// ---------------------------------------------------------------------------------------
// diagnostics (pri_funct_*)

#[cfg(test)]
thread_local! {
    /// (is_stderr, text) of everything printed while a test captures.
    static CAPTURE: std::cell::RefCell<Option<Vec<(bool, String)>>> = const { std::cell::RefCell::new(None) };
}

thread_local! {
    /// `pri_param_i32_ysize`, `pri_param_i32_pos_before` (file statics in the C++).
    static CV: Cell<(i32, i32)> = const { Cell::new((0, 0)) };
}

fn emit(to_stderr: bool, s: &str) {
    #[cfg(test)]
    {
        let taken = CAPTURE.with(|c| match c.borrow_mut().as_mut() {
            Some(v) => {
                v.push((to_stderr, s.to_string()));
                true
            }
            None => false,
        });
        if taken {
            return;
        }
    }
    if to_stderr {
        let mut o = std::io::stderr().lock();
        let _ = o.write_all(s.as_bytes());
        let _ = o.flush();
    } else {
        let mut o = std::io::stdout().lock();
        let _ = o.write_all(s.as_bytes());
        let _ = o.flush();
    }
}

/// C `%g` (precision 6).
fn g(x: f64) -> String {
    if x.is_nan() {
        return if x.is_sign_negative() { "-nan" } else { "nan" }.to_string();
    }
    if x.is_infinite() {
        return if x < 0.0 { "-inf" } else { "inf" }.to_string();
    }
    if x == 0.0 {
        return if x.is_sign_negative() { "-0" } else { "0" }.to_string();
    }
    const P: i32 = 6;
    let sci = format!("{:.*e}", (P - 1) as usize, x);
    let (mant, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let e10: i32 = exp.parse().unwrap_or(0);
    let trim = |s: &str| -> String {
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            s.to_string()
        }
    };
    if e10 < -4 || e10 >= P {
        let sign = if e10 < 0 { '-' } else { '+' };
        format!("{}e{}{:02}", trim(mant), sign, e10.abs())
    } else {
        trim(&format!("{:.*}", (P - 1 - e10) as usize, x))
    }
}

fn timestamp() -> String {
    const WD: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const MO: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    #[cfg(unix)]
    let (wd, mo, md, h, mi, s, y) = {
        // SAFETY: localtime_r writes a plain `tm`; `t` is a valid time_t.
        unsafe {
            let t: libc::time_t = libc::time(std::ptr::null_mut());
            let mut tm: libc::tm = std::mem::zeroed();
            libc::localtime_r(&t, &mut tm);
            (tm.tm_wday, tm.tm_mon, tm.tm_mday, tm.tm_hour, tm.tm_min, tm.tm_sec, tm.tm_year + 1900)
        }
    };
    #[cfg(not(unix))]
    let (wd, mo, md, h, mi, s, y) = {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let days = secs.div_euclid(86400);
        let rem = secs.rem_euclid(86400);
        let z = days + 719468;
        let era = z.div_euclid(146097);
        let doe = z.rem_euclid(146097);
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let yy = yoe + era * 400 + i64::from(m <= 2);
        (
            (days + 4).rem_euclid(7) as i32,
            (m - 1) as i32,
            d as i32,
            (rem / 3600) as i32,
            (rem % 3600 / 60) as i32,
            (rem % 60) as i32,
            yy as i32,
        )
    };
    format!(
        "{} {}{:>3} {:02}:{:02}:{:02} {}",
        WD[(wd.rem_euclid(7)) as usize],
        MO[(mo.rem_euclid(12)) as usize],
        md,
        h,
        mi,
        s,
        y
    )
}

/// `pri_funct_msg_ttvr`: `#  <time>  <message>` on stdout.
fn msg(s: &str) {
    emit(false, &format!("#  {}  {}\n", timestamp(), s));
}

/// `pri_funct_err_bttvr`: BEL, `#  <time>  <message>` on stderr.
fn err(s: &str) {
    emit(true, &format!("\x07#  {}  {}\n", timestamp(), s));
}

fn cv_start(ys: i32) {
    emit(false, "0%  10   20   30   40   50   60   70   80   90   100%\n");
    emit(false, "....v....v....v....v....v....v....v....v....v....v\n");
    CV.with(|c| c.set((ys, 0)));
}

fn cv_run(y: i32) {
    let (ysize, before) = CV.with(|c| c.get());
    let current = 50 * (y + 1) / ysize;
    if current == before {
        return;
    }
    let mut s = String::new();
    for ii in before..current {
        if ii + 1 < current {
            s.push('.');
        } else {
            s.push('^');
        }
    }
    emit(false, &s);
    CV.with(|c| c.set((ysize, current)));
}

fn cv_end() {
    emit(false, "\n");
}

#[derive(Clone, Copy, Default)]
struct Sw {
    mv: bool,
    pv: bool,
    cv: bool,
}

// ---------------------------------------------------------------------------------------
// list_node / list_root

const NIL: usize = usize::MAX;

struct Node<T> {
    prev: usize,
    next: usize,
    d: T,
}

struct List<T> {
    v: Vec<Node<T>>,
    first: usize,
    last: usize,
    count: i32,
}

impl<T> List<T> {
    fn new() -> Self {
        List { v: Vec::new(), first: NIL, last: NIL, count: 0 }
    }

    /// `list_root::_connect`.
    fn connect(&mut self, p: usize, n: usize) {
        if p == NIL && n == NIL {
            self.first = NIL;
            self.last = NIL;
        } else if p != NIL && n == NIL {
            self.last = p;
            self.v[p].next = NIL;
        } else if p == NIL && n != NIL {
            self.first = n;
            self.v[n].prev = NIL;
        } else {
            self.v[n].prev = p;
            self.v[p].next = n;
        }
    }

    /// `list_root::push`: new node after `previous` (the head when `NIL`).
    fn push(&mut self, previous: usize, d: T) -> usize {
        let new = self.v.len();
        self.v.push(Node { prev: NIL, next: NIL, d });
        if previous != NIL {
            let nx = self.v[previous].next;
            self.connect(new, nx);
        } else {
            let f = self.first;
            self.connect(new, f);
        }
        self.connect(previous, new);
        self.count += 1;
        new
    }

    /// `list_root::pop` (the slot stays in the arena, unreferenced).
    fn pop(&mut self, old: usize) {
        let (p, n) = (self.v[old].prev, self.v[old].next);
        self.connect(p, n);
        self.count -= 1;
    }

    /// `while (NULL != last) _remove(last)`; returns how many nodes went.
    fn free_all(&mut self) -> i32 {
        let mut n = 0;
        while self.last != NIL {
            let l = self.last;
            self.pop(l);
            n += 1;
        }
        self.v.clear();
        n
    }

    fn next(&self, i: usize) -> usize {
        self.v[i].next
    }
}

// ---------------------------------------------------------------------------------------
// calculator_geometry

/// Angle of the vector in `0..2*PI`.
fn get_d_radian(xv: f64, yv: f64) -> f64 {
    if 0.0 == xv && 0.0 == yv {
        err("Warning : calculator_geometry::get_d_radian(d_xv,d_yv is zero).");
        return 0.0;
    }
    let mut radian = 0.0;
    if 0.0 < xv && 0.0 <= yv {
        radian = (yv / xv).atan();
    } else if xv <= 0.0 && 0.0 < yv {
        radian = (-xv / yv).atan() + PI / 2.0;
    } else if xv < 0.0 && yv <= 0.0 {
        radian = (yv / xv).atan() + PI;
    } else if 0.0 <= xv && yv < 0.0 {
        radian = (xv / -yv).atan() + PI + PI / 2.0;
    }
    radian
}

/// Counter-clockwise angle from vector 1 to vector 2.
fn get_d_radian_by_2_vector(xv1: f64, yv1: f64, xv2: f64, yv2: f64) -> f64 {
    if 0.0 == xv1 && 0.0 == yv1 {
        err("Warning : calculator_geometry::get_d_radian_by_2_vector(d_xv1,d_yv1 is zero).");
    }
    if 0.0 == xv2 && 0.0 == yv2 {
        err("Warning : calculator_geometry::get_d_radian_by_2_vector(d_xv2,d_yv2 is zero).");
    }
    let start = get_d_radian(xv1, yv1);
    let mut end = get_d_radian(xv2, yv2);
    if end < start {
        end += PI * 2.0;
    }
    end - start
}

fn get_dd_rotate(xp1: f64, yp1: f64, radian: f64) -> (f64, f64) {
    (xp1 * radian.cos() - yp1 * radian.sin(), xp1 * radian.sin() + yp1 * radian.cos())
}

fn get_dd_mirror(xp1: f64, yp1: f64, mirror_xpos: f64, mirror_ypos: f64, mirror_radian: f64) -> (f64, f64) {
    let xp1 = xp1 - mirror_xpos;
    let yp1 = yp1 - mirror_ypos;
    let (xp1, yp1) = get_dd_rotate(xp1, yp1, -mirror_radian);
    let (xp1, yp1) = get_dd_rotate(xp1, -yp1, mirror_radian);
    (xp1 + mirror_xpos, yp1 + mirror_ypos)
}

fn get_dd_rotate_by_pos(xp1: f64, yp1: f64, xpos: f64, ypos: f64, radian: f64) -> (f64, f64) {
    let xp1 = xp1 - xpos;
    let yp1 = yp1 - ypos;
    let (xp1, yp1) = get_dd_rotate(xp1, yp1, radian);
    (xp1 + xpos, yp1 + ypos)
}

// ---------------------------------------------------------------------------------------
// pixel_point_node / pixel_point_root

const LINK_NEAR_COUNT: usize = 4;

#[derive(Clone)]
struct PointData {
    xp: i32,
    yp: i32,
    xt: f64,
    yt: f64,
    link_near: [usize; LINK_NEAR_COUNT],
    prev_point: usize,
    next_point: usize,
}

impl PointData {
    fn new() -> Self {
        PointData { xp: 0, yp: 0, xt: 0.0, yt: 0.0, link_near: [NIL; LINK_NEAR_COUNT], prev_point: NIL, next_point: NIL }
    }
}

type Points = List<PointData>;

/// `pixel_point_node::get_clp_link_near`.
fn link_near_of(pts: &Points, i: usize, n: usize) -> usize {
    if LINK_NEAR_COUNT <= n {
        NIL
    } else {
        pts.v[i].d.link_near[n]
    }
}

/// `pixel_point_node::link_near`: the slot used, or `Err` when all four are taken.
fn link_near(pts: &mut Points, this: usize, other: usize) -> Res<usize> {
    for ii in 0..LINK_NEAR_COUNT {
        if pts.v[this].d.link_near[ii] == NIL {
            pts.v[this].d.link_near[ii] = other;
            return Ok(ii);
        }
    }
    err(&format!("Error : no link_near point, over {}.", LINK_NEAR_COUNT - 1));
    err(&format!("this   x {} y {}", pts.v[this].d.xp, pts.v[this].d.yp));
    for ii in 0..LINK_NEAR_COUNT {
        let l = pts.v[this].d.link_near[ii];
        err(&format!("link_near {} x {} y {}", ii, pts.v[l].d.xp, pts.v[l].d.yp));
    }
    Err(())
}

struct PointRoot {
    sw: Sw,
    l: Points,
}

impl PointRoot {
    fn new() -> Self {
        PointRoot { sw: Sw::default(), l: List::new() }
    }

    fn mem_free(&mut self) {
        if self.l.last != NIL {
            if self.sw.mv {
                msg("pixel_point_root::mem_free()");
            }
            let ii = self.l.free_all();
            if self.sw.pv {
                msg(&format!("free point node {}", ii));
            }
        }
    }

    /// Plot the pixels `> 0` of a 16-bit one-channel image.
    fn alloc_mem_and_list_node(&mut self, xs: i32, ys: i32, src: &[u16]) -> Res {
        if self.sw.mv {
            msg("pixel_point_root::alloc_mem_and_list_node()");
        }
        if self.sw.cv {
            cv_start(ys);
        }
        let mut pp = NIL;
        let mut si = 0usize;
        for yy in 0..ys {
            if self.sw.cv {
                cv_run(yy);
            }
            for xx in 0..xs {
                if 0 < src[si] {
                    pp = self.l.push(pp, PointData::new());
                    self.l.v[pp].d.xp = xx;
                    self.l.v[pp].d.yp = yy;
                }
                si += 1;
            }
        }
        if self.sw.cv {
            cv_end();
        }
        if self.sw.pv {
            msg(&format!("alloc and list {} points", self.l.count));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------
// pixel_line_node

#[derive(Clone)]
struct LineData {
    point_count: i32,
    same_way_radian_one: f64,
    same_way_radian_another: f64,
    bbox_x_min: f64,
    bbox_x_max: f64,
    bbox_y_min: f64,
    bbox_y_max: f64,
    /// one, another, middle, one_expand, another_expand
    link: [usize; 5],
}

const L_ONE: usize = 0;
const L_ANOTHER: usize = 1;
const L_MIDDLE: usize = 2;
const L_ONE_EXPAND: usize = 3;
const L_ANOTHER_EXPAND: usize = 4;

impl LineData {
    fn new() -> Self {
        LineData {
            point_count: 0,
            same_way_radian_one: 0.0,
            same_way_radian_another: 0.0,
            bbox_x_min: 0.0,
            bbox_x_max: 0.0,
            bbox_y_min: 0.0,
            bbox_y_max: 0.0,
            link: [NIL; 5],
        }
    }

    fn expand_line_from_one(
        &mut self,
        pts: &mut Points,
        one: usize,
        another: usize,
        radian: f64,
    ) -> Res {
        let mut before = one;
        let mut exist = pts.v[one].d.next_point;
        while exist != NIL {
            let last_node = pts.last;
            let last = pts.push(last_node, PointData::new());
            pts.v[before].d.prev_point = last;
            pts.v[last].d.next_point = before;
            self.link[L_ONE_EXPAND] = last;
            self.point_count += 1;

            let (xp, yp) = get_dd_mirror(pts.v[exist].d.xt, pts.v[exist].d.yt, pts.v[one].d.xt, pts.v[one].d.yt, radian);
            pts.v[last].d.xt = xp;
            pts.v[last].d.yt = yp;
            before = last;
            if another == exist {
                break;
            }
            exist = pts.v[exist].d.next_point;
        }
        Ok(())
    }

    fn expand_line_from_another(
        &mut self,
        pts: &mut Points,
        one: usize,
        another: usize,
        radian: f64,
    ) -> Res {
        let mut before = another;
        let mut exist = pts.v[another].d.prev_point;
        while exist != NIL {
            let last_node = pts.last;
            let last = pts.push(last_node, PointData::new());
            pts.v[before].d.next_point = last;
            pts.v[last].d.prev_point = before;
            self.link[L_ANOTHER_EXPAND] = last;
            self.point_count += 1;

            let (xp, yp) = get_dd_mirror(
                pts.v[exist].d.xt,
                pts.v[exist].d.yt,
                pts.v[another].d.xt,
                pts.v[another].d.yt,
                radian,
            );
            pts.v[last].d.xt = xp;
            pts.v[last].d.yt = yp;
            before = last;
            if one == exist {
                break;
            }
            exist = pts.v[exist].d.prev_point;
        }
        Ok(())
    }

    fn expand_line(&mut self, pts: &mut Points) -> Res {
        if self.point_count < 3 {
            return Ok(());
        }
        let one = self.link[L_ONE];
        let another = self.link[L_ANOTHER];
        let middle = self.link[L_MIDDLE];

        let (o, m, a) = (&pts.v[one].d, &pts.v[middle].d, &pts.v[another].d);
        let mut radian = get_d_radian_by_2_vector(
            o.xp.wrapping_sub(m.xp) as f64,
            o.yp.wrapping_sub(m.yp) as f64,
            a.xp.wrapping_sub(m.xp) as f64,
            a.yp.wrapping_sub(m.yp) as f64,
        );
        radian /= 2.0;
        let mut radian_one = get_d_radian(m.xp.wrapping_sub(o.xp) as f64, m.yp.wrapping_sub(o.yp) as f64);
        radian_one -= radian;
        let mut radian_another = get_d_radian(m.xp.wrapping_sub(a.xp) as f64, m.yp.wrapping_sub(a.yp) as f64);
        radian_another += radian;

        if link_near_of(pts, one, 1) == NIL {
            if self.expand_line_from_one(pts, one, another, radian_one).is_err() {
                err("Error : this->_expand_line_from_one(-) returns NULL.");
                return Err(());
            }
        }
        if link_near_of(pts, another, 1) == NIL {
            if self.expand_line_from_another(pts, one, another, radian_another).is_err() {
                err("Error : this->_expand_line_from_another(-) returns NULL.");
                return Err(());
            }
        }
        Ok(())
    }

    /// `(position, point, distance)` of the expanded-line point nearest to `(xp, yp)`;
    /// `None` when none is closer than 1000.
    fn get_near_point(&self, pts: &Points, xp: f64, yp: f64) -> Option<(i32, usize, f64)> {
        let mut length = 1000.0;
        let mut found = None;
        let mut p = self.link[L_ONE_EXPAND];
        let mut ii = 0;
        while p != NIL {
            let d = &pts.v[p].d;
            let l = ((d.xt - xp) * (d.xt - xp) + (d.yt - yp) * (d.yt - yp)).sqrt();
            if l < length {
                found = Some((ii, p));
                length = l;
            }
            ii += 1;
            p = d.next_point;
        }
        found.map(|(i, p)| (i, p, length))
    }

    fn int2double_body(&self, pts: &mut Points) {
        let mut p = self.link[L_ONE];
        while p != NIL {
            let d = &mut pts.v[p].d;
            d.xt = d.xp as f64;
            d.yt = d.yp as f64;
            p = d.next_point;
        }
    }

    /// `_get_link_line_selecter_vector`.
    fn selecter_vector(pts: &Points, crnt: usize, next: usize, count: i32) -> (f64, f64) {
        let start = crnt;
        let mut crnt = crnt;
        let mut next = next;
        let mut ii = 0;
        while next != NIL && ii < count {
            let nd = &pts.v[next].d;
            if nd.next_point != NIL || nd.prev_point != NIL {
                break;
            }
            if nd.link_near[1] == NIL {
                crnt = next;
                break;
            }
            if nd.link_near[2] != NIL {
                crnt = next;
                break;
            }
            if crnt == nd.link_near[0] {
                crnt = next;
                next = nd.link_near[1];
            } else if crnt == nd.link_near[1] {
                crnt = next;
                next = nd.link_near[0];
            } else {
                err("Error : bad link");
                // assert(0) is compiled out
            }
            ii += 1;
        }
        (
            pts.v[crnt].d.xp.wrapping_sub(pts.v[start].d.xp) as f64,
            pts.v[crnt].d.yp.wrapping_sub(pts.v[start].d.yp) as f64,
        )
    }

    /// `_get_link_line_selecter`: the branch that continues straightest, or `NIL`.
    fn selecter(pts: &Points, xv: f64, yv: f64, crnt: usize, count: i32) -> usize {
        let mut pos = 0usize;
        let mut da_xv = [0.0f64; LINK_NEAR_COUNT];
        let mut da_yv = [0.0f64; LINK_NEAR_COUNT];
        let mut da_radian = [0.0f64; LINK_NEAR_COUNT];

        for ii in 0..LINK_NEAR_COUNT {
            let l = pts.v[crnt].d.link_near[ii];
            if l == NIL {
                break;
            }
            let (a, b) = Self::selecter_vector(pts, crnt, l, count);
            da_xv[ii] = a;
            da_yv[ii] = b;
        }
        for ii in 0..LINK_NEAR_COUNT {
            if pts.v[crnt].d.link_near[ii] == NIL {
                break;
            }
            if 0.0 != da_xv[ii] || 0.0 != da_yv[ii] {
                da_radian[ii] = get_d_radian_by_2_vector(xv, yv, da_xv[ii], da_yv[ii]);
                if PI < da_radian[ii] {
                    da_radian[ii] = PI - (da_radian[ii] - PI);
                }
            } else {
                da_radian[ii] = PI;
            }
        }
        let mut radian = PI * 2.0;
        for ii in 0..LINK_NEAR_COUNT {
            if pts.v[crnt].d.link_near[ii] == NIL {
                break;
            }
            if da_radian[ii] < radian {
                pos = ii;
                radian = da_radian[ii];
            }
        }
        if PI / 2.0 <= radian && radian <= PI * 3.0 / 2.0 {
            return NIL;
        }
        pts.v[crnt].d.link_near[pos]
    }

    fn link_line(&mut self, pts: &mut Points, crnt: usize, next: usize, count: i32) {
        let mut crnt = crnt;
        let mut next = next;
        self.link[L_ONE] = crnt;
        self.point_count += 1;

        let mut tmp1 = self.link[L_ONE];
        let mut ii = 0;
        while next != NIL && ii < count {
            if pts.v[next].d.next_point != NIL || pts.v[next].d.prev_point != NIL {
                self.link[L_ANOTHER] = crnt;
                return;
            }
            pts.v[crnt].d.next_point = next;
            pts.v[next].d.prev_point = crnt;
            self.point_count += 1;

            if pts.v[next].d.link_near[1] == NIL {
                self.link[L_ANOTHER] = next;
                return;
            }
            if pts.v[next].d.link_near[2] != NIL {
                let tmp2 = tmp1;
                tmp1 = next;
                let xv = pts.v[tmp1].d.xp.wrapping_sub(pts.v[tmp2].d.xp) as f64;
                let yv = pts.v[tmp1].d.yp.wrapping_sub(pts.v[tmp2].d.yp) as f64;
                crnt = next;
                next = Self::selecter(pts, xv, yv, next, count);
                if next == NIL {
                    self.link[L_ANOTHER] = crnt;
                    return;
                }
            } else if crnt == pts.v[next].d.link_near[0] {
                crnt = next;
                next = pts.v[next].d.link_near[1];
            } else if crnt == pts.v[next].d.link_near[1] {
                crnt = next;
                next = pts.v[next].d.link_near[0];
            } else {
                err("Error : bad link");
                // assert(0) is compiled out
            }
            ii += 1;
        }
        err("Error : too long link");
        // assert(0) is compiled out
    }

    fn get_next_point_by_count(pts: &Points, point: usize, count: i32) -> usize {
        let mut p = point;
        let mut c = count;
        while 0 < c && p != NIL {
            p = pts.v[p].d.next_point;
            c -= 1;
        }
        p
    }

    fn get_prev_point_by_count(pts: &Points, point: usize, count: i32) -> usize {
        let mut p = point;
        let mut c = count;
        while 0 < c && p != NIL {
            p = pts.v[p].d.prev_point;
            c -= 1;
        }
        p
    }

    fn set_bbox(&mut self, pts: &Points) {
        let mut p = self.link[L_ONE_EXPAND];
        if p == NIL {
            p = self.link[L_ONE];
        }
        let mut ii = 0;
        while p != NIL {
            let d = &pts.v[p].d;
            if 0 == ii {
                self.bbox_x_min = d.xt;
                self.bbox_x_max = d.xt;
                self.bbox_y_min = d.yt;
                self.bbox_y_max = d.yt;
            } else {
                if d.xt < self.bbox_x_min {
                    self.bbox_x_min = d.xt;
                } else if self.bbox_x_max < d.xt {
                    self.bbox_x_max = d.xt;
                }
                if d.yt < self.bbox_y_min {
                    self.bbox_y_min = d.yt;
                } else if self.bbox_y_max < d.yt {
                    self.bbox_y_max = d.yt;
                }
            }
            ii += 1;
            p = d.next_point;
        }
    }

    fn set_middle(&mut self, pts: &Points) {
        let mut middle = self.link[L_ONE];
        let mut ii = 0;
        while ii < self.point_count / 2 && middle != NIL {
            if pts.v[middle].d.next_point != NIL {
                middle = pts.v[middle].d.next_point;
            }
            ii += 1;
        }
        self.link[L_MIDDLE] = middle;
    }

    fn smooth_point(x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64) -> (f64, f64) {
        ((x2 + ((x1 + x3) / 2.0)) / 2.0, (y2 + ((y1 + y3) / 2.0)) / 2.0)
    }

    fn smooth_body(&self, pts: &mut Points, retry: i32) {
        for _kk in 0..retry {
            let mut c1 = self.link[L_ONE];
            let mut c2 = NIL;
            let mut c3 = NIL;
            let (mut x2, mut y2, mut x3, mut y3) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
            while c1 != NIL {
                let (x1, y1) = (pts.v[c1].d.xt, pts.v[c1].d.yt);
                if c3 != NIL {
                    let (x, y) = Self::smooth_point(x1, y1, x2, y2, x3, y3);
                    pts.v[c2].d.xt = x;
                    pts.v[c2].d.yt = y;
                }
                if self.link[L_ANOTHER] == c1 {
                    break;
                }
                c3 = c2;
                x3 = x2;
                y3 = y2;
                c2 = c1;
                x2 = x1;
                y2 = y1;
                c1 = pts.v[c1].d.next_point;
            }
        }
    }

    fn smooth_expand(&self, pts: &mut Points, retry: i32) {
        let (mut x2, mut y2) = (0.0f64, 0.0f64);
        let (mut x3, mut y3) = (0.0f64, 0.0f64);
        for kk in 0..retry {
            let mut c1 = self.link[L_ONE_EXPAND];
            let mut c2 = NIL;
            let mut c3 = NIL;
            let mut ii = 0;
            while c1 != NIL {
                let (x1, y1) = (pts.v[c1].d.xt, pts.v[c1].d.yt);
                if kk == 0 && ii == 0 {
                    x2 = x1;
                    y2 = y1;
                }
                if c3 != NIL {
                    let (x, y) = Self::smooth_point(x1, y1, x2, y2, x3, y3);
                    pts.v[c2].d.xt = x;
                    pts.v[c2].d.yt = y;
                }
                if self.link[L_ANOTHER_EXPAND] == c1 {
                    break;
                }
                c3 = c2;
                x3 = x2;
                y3 = y2;
                c2 = c1;
                x2 = x1;
                y2 = y1;
                c1 = pts.v[c1].d.next_point;
                ii += 1;
            }
        }
    }

    // --- debug text (save_*) ---

    fn text_line(&self, pts: &Points, out: &mut String) {
        let mut p = self.link[L_ONE];
        while p != NIL {
            out.push_str(&format!("{} {}\n", pts.v[p].d.xp, pts.v[p].d.yp));
            p = pts.v[p].d.next_point;
        }
    }

    fn text_int_point(&self, pts: &Points, which: usize, out: &mut String) {
        let p = self.link[which];
        if p != NIL {
            out.push_str(&format!("{} {}\n", pts.v[p].d.xp, pts.v[p].d.yp));
        }
    }

    fn text_expand_line(&self, pts: &Points, out: &mut String) {
        let mut p = self.link[L_ONE_EXPAND];
        while p != NIL {
            out.push_str(&format!("{} {}\n", g(pts.v[p].d.xt), g(pts.v[p].d.yt)));
            p = pts.v[p].d.next_point;
        }
    }

    fn text_expand_point(&self, pts: &Points, which: usize, out: &mut String) {
        let p = self.link[which];
        if p != NIL {
            out.push_str(&format!("{} {}\n", g(pts.v[p].d.xt), g(pts.v[p].d.yt)));
        }
    }

    fn text_expand_vector(&self, pts: &Points, out: &mut String) {
        for (t, e) in [(L_ONE, L_ONE_EXPAND), (L_ANOTHER, L_ANOTHER_EXPAND)] {
            let (term, expa) = (self.link[t], self.link[e]);
            if term != NIL && expa != NIL {
                let (a, b) = (&pts.v[term].d, &pts.v[expa].d);
                out.push_str(&format!("{} {} {} {}\n", g(a.xt), g(a.yt), g(b.xt - a.xt), g(b.yt - a.yt)));
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// pixel_select_same_way_root

#[derive(Clone, Copy)]
#[allow(dead_code)] // `point_middle` is only stored (the C++ node kept it too)
struct SameWayNode {
    point_middle: usize,
    point_term: usize,
    point_expand: usize,
    length: f64,
}

struct SameWayRoot {
    l: List<SameWayNode>,
    count_max: i32,
    length_max: f64,
}

impl SameWayRoot {
    fn new() -> Self {
        SameWayRoot { l: List::new(), count_max: 4, length_max: 160.0 }
    }

    fn mem_free(&mut self) {
        self.l.free_all();
    }

    fn sort_append(&mut self, src: &SameWayNode) {
        let mut lp = self.l.first;
        let prev;
        if lp == NIL {
            prev = NIL;
        } else if self.l.v[lp].next == NIL {
            prev = if src.length < self.l.v[lp].d.length { NIL } else { lp };
        } else {
            let mut p = self.l.last;
            while lp != NIL {
                if src.length < self.l.v[lp].d.length {
                    p = self.l.v[lp].prev;
                    break;
                }
                lp = self.l.v[lp].next;
            }
            prev = p;
        }
        self.l.push(prev, *src);
    }

    fn term_length(&self, pts: &Points, middle1: usize, term1: usize, middle2: usize, term2: usize) -> f64 {
        let (t1, t2) = (&pts.v[term1].d, &pts.v[term2].d);
        let length = ((t2.xt - t1.xt) * (t2.xt - t1.xt) + (t2.yt - t1.yt) * (t2.yt - t1.yt)).sqrt();
        let (m1, m2) = (&pts.v[middle1].d, &pts.v[middle2].d);
        let radian = get_d_radian_by_2_vector(t1.xt - m1.xt, t1.yt - m1.yt, t2.xt - m2.xt, t2.yt - m2.yt);
        if PI / 2.0 <= radian && radian <= PI * 3.0 / 2.0 {
            return -1.0;
        }
        if self.length_max <= length {
            return -1.0;
        }
        length
    }

    fn exec(&mut self, lines: &List<LineData>, pts: &Points, point_middle: usize, point_term: usize) {
        self.mem_free();
        let mut cl = lines.first;
        while cl != NIL {
            let line = &lines.v[cl].d;
            if line.link[L_ONE] != point_term && line.link[L_MIDDLE] != line.link[L_ONE] {
                let length = self.term_length(pts, point_middle, point_term, line.link[L_MIDDLE], line.link[L_ONE]);
                if 0.0 < length && line.link[L_ONE_EXPAND] != NIL {
                    let sel = SameWayNode {
                        point_middle: line.link[L_MIDDLE],
                        point_term: line.link[L_ONE],
                        point_expand: line.link[L_ONE_EXPAND],
                        length,
                    };
                    self.sort_append(&sel);
                }
            }
            if line.link[L_ANOTHER] != point_term && line.link[L_MIDDLE] != line.link[L_ANOTHER] {
                let length = self.term_length(pts, point_middle, point_term, line.link[L_MIDDLE], line.link[L_ANOTHER]);
                if 0.0 < length && line.link[L_ANOTHER_EXPAND] != NIL {
                    let sel = SameWayNode {
                        point_middle: line.link[L_MIDDLE],
                        point_term: line.link[L_ANOTHER],
                        point_expand: line.link[L_ANOTHER_EXPAND],
                        length,
                    };
                    self.sort_append(&sel);
                }
            }
            cl = lines.v[cl].next;
        }
    }

    fn get_vector(&self, pts: &Points, xv: &mut f64, yv: &mut f64) {
        let mut lp = self.l.first;
        let mut ii = 0;
        while lp != NIL && ii < self.count_max {
            let n = &self.l.v[lp].d;
            *xv += pts.v[n.point_expand].d.xt - pts.v[n.point_term].d.xt;
            *yv += pts.v[n.point_expand].d.yt - pts.v[n.point_term].d.yt;
            lp = self.l.v[lp].next;
            ii += 1;
        }
    }
}

// ---------------------------------------------------------------------------------------
// pixel_line_root

struct LineRoot {
    sw: Sw,
    l: List<LineData>,
    bbox_x_min: f64,
    bbox_x_max: f64,
    bbox_y_min: f64,
    bbox_y_max: f64,
    smooth_retry: i32,
    same_way_exec_sw: bool,
}

impl LineRoot {
    fn new() -> Self {
        LineRoot {
            sw: Sw::default(),
            l: List::new(),
            bbox_x_min: 0.0,
            bbox_x_max: 0.0,
            bbox_y_min: 0.0,
            bbox_y_max: 0.0,
            smooth_retry: 100,
            same_way_exec_sw: true,
        }
    }

    fn mem_free(&mut self) {
        if self.l.last != NIL {
            if self.sw.mv {
                msg("pixel_line_root::mem_free()");
            }
            let ii = self.l.free_all();
            if self.sw.pv {
                msg(&format!("free line node {}", ii));
            }
        }
    }

    fn exec01020304(&mut self, pr: &mut PointRoot) -> Res {
        if self.exec01_link_left_right(&mut pr.l).is_err() {
            err("Error : this->_exec01_link_left_right() returns NG.");
            return Err(());
        }
        if self.exec02_link_up_down(&mut pr.l).is_err() {
            err("Error : this->_exec02_link_up_down() returns NG.");
            return Err(());
        }
        if self.exec03_link_slant(&mut pr.l).is_err() {
            err("Error : this->_exec03_link_slant() returns NG.");
            return Err(());
        }
        if self.exec04_grouping(&mut pr.l).is_err() {
            err("Error : this->_exec04_grouping() returns NG.");
            return Err(());
        }
        Ok(())
    }

    fn both_link(pts: &mut Points, a: usize, b: usize, ii: i32) -> Res {
        if link_near(pts, a, b).is_err() {
            err(&format!("Error : count {} : clp_point->link_near() returns NG.", ii));
            return Err(());
        }
        if link_near(pts, b, a).is_err() {
            err(&format!("Error : count {} : clp_point2->link_near() returns NG.", ii));
            return Err(());
        }
        Ok(())
    }

    fn exec01_link_left_right(&mut self, pts: &mut Points) -> Res {
        if self.sw.mv {
            msg("pixel_line_root::_exec01_link_left_right()");
        }
        let mut ii = 0;
        let mut point2 = NIL;
        let mut point = pts.first;
        while point != NIL {
            if point2 != NIL
                && pts.v[point].d.yp == pts.v[point2].d.yp
                && pts.v[point2].d.xp.wrapping_add(1) == pts.v[point].d.xp
            {
                Self::both_link(pts, point, point2, ii)?;
                ii += 1;
            }
            point2 = point;
            point = pts.v[point].next;
        }
        if self.sw.pv {
            msg(&format!(" link left right {}", ii));
        }
        Ok(())
    }

    fn exec02_link_up_down(&mut self, pts: &mut Points) -> Res {
        if self.sw.mv {
            msg("pixel_line_root::_exec02_link_up_down()");
        }
        let mut ii = 0;
        let mut point = pts.first;
        while point != NIL {
            let mut point2 = pts.v[point].next;
            while point2 != NIL && pts.v[point2].d.yp <= pts.v[point].d.yp.wrapping_add(1) {
                if pts.v[point2].d.yp == pts.v[point].d.yp.wrapping_add(1) && pts.v[point].d.xp == pts.v[point2].d.xp {
                    Self::both_link(pts, point, point2, ii)?;
                    ii += 1;
                    break;
                }
                point2 = pts.v[point2].next;
            }
            point = pts.v[point].next;
        }
        if self.sw.pv {
            msg(&format!(" link up down {}", ii));
        }
        Ok(())
    }

    fn exec03_link_slant(&mut self, pts: &mut Points) -> Res {
        if self.sw.mv {
            msg("pixel_line_root::_exec03_link_slant()");
        }
        let mut ii = 0;
        let mut point = pts.first;
        while point != NIL {
            let next_point = pts.v[point].next;
            if pts.v[point].d.link_near[LINK_NEAR_COUNT - 1] != NIL {
                point = next_point;
                continue;
            }
            let (mut left, mut right, mut up) = (false, false, false);
            let (px, py) = (pts.v[point].d.xp, pts.v[point].d.yp);
            for jj in 0..LINK_NEAR_COUNT {
                let link = pts.v[point].d.link_near[jj];
                if link == NIL {
                    break;
                }
                let (lx, ly) = (pts.v[link].d.xp, pts.v[link].d.yp);
                if lx == px && ly == py.wrapping_add(1) {
                    up = true;
                }
                if lx == px.wrapping_sub(1) && ly == py {
                    left = true;
                }
                if lx == px.wrapping_add(1) && ly == py {
                    right = true;
                }
            }
            if left && right {
                point = next_point;
                continue;
            }
            if up {
                point = next_point;
                continue;
            }
            let mut point2 = next_point;
            while point2 != NIL && pts.v[point2].d.yp <= py.wrapping_add(1) {
                let (x2, y2) = (pts.v[point2].d.xp, pts.v[point2].d.yp);
                if y2 != py.wrapping_add(1) {
                    point2 = pts.v[point2].next;
                    continue;
                }
                if !left && x2 == px.wrapping_sub(1) && y2 == py.wrapping_add(1) {
                    Self::both_link(pts, point, point2, ii)?;
                    ii += 1;
                }
                if !right && x2 == px.wrapping_add(1) && y2 == py.wrapping_add(1) {
                    Self::both_link(pts, point, point2, ii)?;
                    ii += 1;
                }
                point2 = pts.v[point2].next;
            }
            point = next_point;
        }
        if self.sw.pv {
            msg(&format!(" link slant {}", ii));
        }
        Ok(())
    }

    fn exec04_grouping(&mut self, pts: &mut Points) -> Res {
        if self.sw.mv {
            msg("pixel_line_root::_exec04_grouping()");
        }
        let mut line_before = NIL;
        let mut point = pts.first;
        while point != NIL {
            let next_point = pts.v[point].next;
            if pts.v[point].d.next_point != NIL || pts.v[point].d.prev_point != NIL {
                point = next_point;
                continue;
            }
            if pts.v[point].d.link_near[0] != NIL && pts.v[point].d.link_near[1] == NIL {
                let crnt = self.l.push(line_before, LineData::new());
                let first_link = pts.v[point].d.link_near[0];
                let total = pts.count;
                self.l.v[crnt].d.link_line(pts, point, first_link, total);
                if self.l.v[crnt].d.point_count < 3 {
                    self.l.pop(crnt);
                } else {
                    line_before = crnt;
                }
            }
            point = next_point;
        }
        if self.sw.pv {
            msg(&format!(" make {} lines", self.l.count));
        }
        Ok(())
    }

    fn exec05_set_middle(&mut self, pts: &Points) {
        if self.sw.mv {
            msg("pixel_line_root::exec05_set_middle()");
        }
        let mut ii = 0;
        let mut cl = self.l.first;
        while cl != NIL {
            self.l.v[cl].d.set_middle(pts);
            ii += 1;
            cl = self.l.v[cl].next;
        }
        if self.sw.pv {
            msg(&format!(" set middle point about {} lines", ii));
        }
    }

    fn exec06_int2double_body(&mut self, pts: &mut Points) {
        if self.sw.mv {
            msg("pixel_line_root::exec06_int2double_body()");
        }
        let mut ii = 0;
        let mut cl = self.l.first;
        while cl != NIL {
            self.l.v[cl].d.int2double_body(pts);
            ii += 1;
            cl = self.l.v[cl].next;
        }
        if self.sw.pv {
            msg(&format!(" int to double {} lines", ii));
        }
    }

    fn smooth_all(&mut self, pts: &mut Points, expand: bool) {
        if self.sw.pv {
            msg(&format!(" smooth retry {}", self.smooth_retry as u32));
        }
        if self.sw.cv {
            cv_start(self.l.count);
        }
        let mut ii = 0;
        let mut cl = self.l.first;
        while cl != NIL {
            if self.sw.cv {
                cv_run(ii);
            }
            if expand {
                self.l.v[cl].d.smooth_expand(pts, self.smooth_retry);
            } else {
                self.l.v[cl].d.smooth_body(pts, self.smooth_retry);
            }
            ii += 1;
            cl = self.l.v[cl].next;
        }
        if self.sw.cv {
            cv_end();
        }
        if self.sw.pv {
            msg(&format!(" smooth {} lines", ii));
        }
    }

    fn exec07_smooth_body(&mut self, pts: &mut Points) {
        if self.sw.mv {
            msg("pixel_line_root::exec07_smooth_body()");
        }
        self.smooth_all(pts, false);
    }

    fn exec10_smooth_expand(&mut self, pts: &mut Points) {
        if self.sw.mv {
            msg("pixel_line_root::exec10_smooth_expand()");
        }
        self.smooth_all(pts, true);
    }

    fn exec08_expand_lines(&mut self, pts: &mut Points) -> Res {
        if self.sw.mv {
            msg("pixel_line_root::exec08_expand_lines()");
        }
        let mut ii = 0;
        let mut cl = self.l.first;
        while cl != NIL {
            if self.l.v[cl].d.expand_line(pts).is_err() {
                err(&format!("Error : line number {} : clp_line->expand_line() returns NG", ii));
                return Err(());
            }
            ii += 1;
            cl = self.l.v[cl].next;
        }
        if self.sw.pv {
            msg(&format!(" expand {} lines", ii));
        }
        Ok(())
    }

    fn same_way_expand_radian_diff(
        &self,
        pts: &Points,
        point_middle: usize,
        point_term: usize,
        point_term_expand: usize,
        select: &mut SameWayRoot,
    ) -> f64 {
        select.exec(&self.l, pts, point_middle, point_term);
        if select.l.count <= 0 {
            return 0.0;
        }
        let (mut xv, mut yv) = (0.0f64, 0.0f64);
        select.get_vector(pts, &mut xv, &mut yv);
        if 0.0 == xv && 0.0 == yv {
            return 0.0;
        }
        let (e, t) = (&pts.v[point_term_expand].d, &pts.v[point_term].d);
        xv += (e.xt - t.xt) * 2.0;
        yv += (e.yt - t.yt) * 2.0;
        get_d_radian_by_2_vector(e.xt - t.xt, e.yt - t.yt, xv, yv)
    }

    fn exec09_same_way_expand(&mut self, pts: &mut Points, select: &mut SameWayRoot) {
        if !self.same_way_exec_sw {
            return;
        }
        if self.sw.mv {
            msg("pixel_line_root::exec09_same_way_expand()");
        }
        if self.sw.pv {
            msg(&format!(" select max length {} count {}", g(select.length_max), select.count_max));
        }
        let mut cl = self.l.first;
        while cl != NIL {
            let line = self.l.v[cl].d.link;
            if line[L_ONE_EXPAND] != NIL {
                let r = self.same_way_expand_radian_diff(pts, line[L_MIDDLE], line[L_ONE], line[L_ONE_EXPAND], select);
                self.l.v[cl].d.same_way_radian_one = r;
            }
            if line[L_ANOTHER_EXPAND] != NIL {
                let r = self.same_way_expand_radian_diff(
                    pts,
                    line[L_MIDDLE],
                    line[L_ANOTHER],
                    line[L_ANOTHER_EXPAND],
                    select,
                );
                self.l.v[cl].d.same_way_radian_another = r;
            }
            cl = self.l.v[cl].next;
        }

        let (mut count_one, mut count_another) = (0, 0);
        let mut cl = self.l.first;
        while cl != NIL {
            let line = self.l.v[cl].d.clone();
            if 0.0 < line.same_way_radian_one {
                let start = line.link[L_ONE];
                let mut tmp = pts.v[start].d.prev_point;
                while tmp != NIL {
                    let (x, y) = get_dd_rotate_by_pos(
                        pts.v[tmp].d.xt,
                        pts.v[tmp].d.yt,
                        pts.v[start].d.xt,
                        pts.v[start].d.yt,
                        line.same_way_radian_one,
                    );
                    pts.v[tmp].d.xt = x;
                    pts.v[tmp].d.yt = y;
                    tmp = pts.v[tmp].d.prev_point;
                }
                count_one += 1;
            }
            if 0.0 < line.same_way_radian_another {
                let start = line.link[L_ANOTHER];
                let mut tmp = pts.v[start].d.next_point;
                while tmp != NIL {
                    let (x, y) = get_dd_rotate_by_pos(
                        pts.v[tmp].d.xt,
                        pts.v[tmp].d.yt,
                        pts.v[start].d.xt,
                        pts.v[start].d.yt,
                        line.same_way_radian_another,
                    );
                    pts.v[tmp].d.xt = x;
                    pts.v[tmp].d.yt = y;
                    tmp = pts.v[tmp].d.next_point;
                }
                count_another += 1;
            }
            cl = self.l.v[cl].next;
        }
        select.mem_free();
        if self.sw.pv {
            msg(&format!(" same way expand  one {} another {}", count_one, count_another));
        }
    }

    fn exec11_set_bbox(&mut self, pts: &Points) {
        if self.sw.mv {
            msg("pixel_line_root::exec11_set_bbox()");
        }
        let mut ii = 0;
        let mut cl = self.l.first;
        while cl != NIL {
            self.l.v[cl].d.set_bbox(pts);
            let b = self.l.v[cl].d.clone();
            if 0 == ii {
                self.bbox_x_min = b.bbox_x_min;
                self.bbox_x_max = b.bbox_x_max;
                self.bbox_y_min = b.bbox_y_min;
                self.bbox_y_max = b.bbox_y_max;
            } else {
                if b.bbox_x_min < self.bbox_x_min {
                    self.bbox_x_min = b.bbox_x_min;
                } else if self.bbox_x_max < b.bbox_x_max {
                    self.bbox_x_max = b.bbox_x_max;
                }
                if b.bbox_y_min < self.bbox_y_min {
                    self.bbox_y_min = b.bbox_y_min;
                } else if self.bbox_y_max < b.bbox_y_max {
                    self.bbox_y_max = b.bbox_y_max;
                }
            }
            ii += 1;
            cl = self.l.v[cl].next;
        }
        if self.sw.pv {
            msg(&format!(
                " set bbox {} lines : min x {} y {} : max x {} y {}",
                ii,
                g(self.bbox_x_min),
                g(self.bbox_y_min),
                g(self.bbox_x_max),
                g(self.bbox_y_max)
            ));
        }
    }

    // --- debug files ---

    fn text_header(&self, label: &str) -> String {
        format!("# {} count {}\n", label, self.l.count)
    }

    fn text_per_line(
        &self,
        label: &str,
        item: &str,
        gap: bool,
        mut body: impl FnMut(&LineData, &mut String),
    ) -> String {
        let mut out = self.text_header(label);
        let mut ii = 0;
        let mut cl = self.l.first;
        while cl != NIL {
            let line = &self.l.v[cl].d;
            if item.is_empty() {
                out.push_str(&format!("# line number {}  and points count {}\n", ii, line.point_count));
            } else {
                out.push_str(&format!("# line number {} ... {}\n", ii, item));
            }
            body(line, &mut out);
            if gap && self.l.v[cl].next != NIL {
                out.push_str("\n\n");
            }
            ii += 1;
            cl = self.l.v[cl].next;
        }
        out
    }
}

// ---------------------------------------------------------------------------------------
// pixel_select_curve_blur_root

const NOT_USE_PARAMETER_VAL: f64 = -10000.0;

#[derive(Clone, Copy)]
struct CurveSelNode {
    line: usize,
    start_point: usize,
    near_point: usize,
    near_point_pos: i32,
    length: f64,
    reverse: bool,
}

struct CurveSelRoot {
    l: List<CurveSelNode>,
    count_max: i32,
    length_max: f64,
}

impl CurveSelRoot {
    fn new() -> Self {
        CurveSelRoot { l: List::new(), count_max: 4, length_max: 160.0 }
    }

    fn mem_free(&mut self) {
        self.l.free_all();
    }

    fn sort_append(&mut self, src: &CurveSelNode) {
        let mut prev = self.l.last;
        while prev != NIL {
            if self.l.v[prev].d.length < src.length {
                break;
            }
            prev = self.l.v[prev].prev;
        }
        self.l.push(prev, *src);
    }

    fn exec(&mut self, x: f64, y: f64, lr: &LineRoot, pts: &Points, blur_count: i32, effect_radius: f64) {
        self.mem_free();
        if x < lr.bbox_x_min - effect_radius
            || lr.bbox_x_max + effect_radius < x
            || y < lr.bbox_y_min - effect_radius
            || lr.bbox_y_max + effect_radius < y
        {
            return;
        }
        let mut radius_1st = NOT_USE_PARAMETER_VAL;
        let mut cl = lr.l.first;
        while cl != NIL {
            let line = &lr.l.v[cl].d;
            let next_line = lr.l.v[cl].next;
            if x < line.bbox_x_min - effect_radius
                || line.bbox_x_max + effect_radius < x
                || y < line.bbox_y_min - effect_radius
                || line.bbox_y_max + effect_radius < y
            {
                cl = next_line;
                continue;
            }
            // C++: with no point closer than 1000 the position/point stay uninitialised
            // (UB); such a line is skipped here.
            let (pos, near_point, length) = match line.get_near_point(pts, x, y) {
                Some(t) => t,
                None => {
                    cl = next_line;
                    continue;
                }
            };
            if self.length_max < length {
                cl = next_line;
                continue;
            }
            if pos < blur_count / 2 || (line.point_count - 1 - pos) < blur_count / 2 {
                cl = next_line;
                continue;
            }
            let mut reverse = false;
            let mut start_point = LineData::get_prev_point_by_count(pts, near_point, blur_count / 2);
            let (sp, np) = (&pts.v[start_point].d, &pts.v[near_point].d);
            let mut radius = get_d_radian(sp.xt - np.xt, sp.yt - np.yt);
            if NOT_USE_PARAMETER_VAL == radius_1st {
                radius_1st = radius;
            } else {
                if radius < radius_1st {
                    radius += PI * 2.0;
                }
                radius -= radius_1st;
                if PI / 2.0 < radius && radius < PI * 3.0 / 2.0 {
                    start_point = LineData::get_next_point_by_count(pts, near_point, blur_count / 2);
                    reverse = true;
                }
            }
            let sel = CurveSelNode {
                line: cl,
                start_point,
                near_point,
                near_point_pos: pos,
                length,
                reverse,
            };
            self.sort_append(&sel);
            cl = next_line;
        }
    }

    fn line_accum_count(sel: &CurveSelNode, lines: &List<LineData>, blur_count: i32) -> f64 {
        let pos = sel.near_point_pos - (blur_count / 2);
        let count = lines.v[sel.line].d.point_count - (blur_count / 2) - (blur_count / 2);
        let mut ratio = ((pos as f64) + 0.5) / (count as f64);
        ratio = 1.0 + (2.0 * PI * ratio - PI / 2.0).sin();
        ratio
    }

    /// Blend the selected lines into `xv/yv` (offsets from the near point, `blur_count`
    /// entries); `Err` when nothing is selected.
    fn get_line(&self, lines: &List<LineData>, pts: &Points, blur_count: i32, xv: &mut [f64], yv: &mut [f64]) -> Res {
        for ii in 0..blur_count as usize {
            xv[ii] = 0.0;
            yv[ii] = 0.0;
        }
        let mut count = 0;
        let mut accum = 0.0f64;
        let mut sel = self.l.first;
        let mut ii = 0;
        while sel != NIL && ii < self.count_max {
            let s = self.l.v[sel].d;
            let ratio = Self::line_accum_count(&s, lines, blur_count);
            accum += ratio;
            let xp = pts.v[s.near_point].d.xt;
            let yp = pts.v[s.near_point].d.yt;
            let mut point = s.start_point;
            for jj in 0..blur_count as usize {
                xv[jj] += (pts.v[point].d.xt - xp) * ratio;
                yv[jj] += (pts.v[point].d.yt - yp) * ratio;
                point = if !s.reverse { pts.v[point].d.next_point } else { pts.v[point].d.prev_point };
            }
            sel = self.l.v[sel].next;
            ii += 1;
            count += 1;
        }
        if count <= 0 {
            return Err(());
        }
        for ii in 0..blur_count as usize {
            xv[ii] /= accum;
            yv[ii] /= accum;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------
// thinnest_ui16_image

const UINT16_MAX: u16 = 65535;

struct Thin {
    sw: Sw,
    xs: i32,
    ys: i32,
    xd: i32,
    yd: i32,
    threshold: u16,
    exec_loop_count: i32,
    mem: Vec<u16>,
    allocated: bool,
    /// offsets of the source and target planes inside `mem`
    ch: [usize; 2],
}

/// `size_t` pointer-ish arithmetic on `Option<usize>` plane positions.
fn plus(p: Option<usize>, n: usize) -> Option<usize> {
    p.map(|i| i + n)
}

impl Thin {
    fn new() -> Self {
        Thin {
            sw: Sw::default(),
            xs: 0,
            ys: 0,
            xd: 1,
            yd: 1,
            threshold: 0x4000,
            exec_loop_count: 100,
            mem: Vec::new(),
            allocated: false,
            ch: [0, 0],
        }
    }

    fn src(&self) -> usize {
        self.ch[0]
    }

    fn tgt(&self) -> usize {
        self.ch[1]
    }

    fn swap_channel(&mut self) {
        self.ch.swap(0, 1);
    }

    fn mem_free(&mut self) {
        if self.allocated {
            if self.sw.mv {
                msg(&format!("thinnest_ui16_image::mem_free() <{:x}>", self.mem.as_ptr() as usize as u32));
            }
            self.mem = Vec::new();
            self.allocated = false;
            self.ch = [0, 0];
        }
    }

    fn mem_alloc(&mut self) -> Res {
        self.mem_free();
        if self.sw.mv {
            msg("thinnest_ui16_image::mem_alloc()");
        }
        if self.sw.pv {
            msg(&format!(
                "alloc ui16_image memory ({}+{}) x ({}+{}) x {} x {} bytes",
                self.xs.wrapping_mul(self.xd),
                2,
                self.ys.wrapping_mul(self.yd),
                2,
                2,
                2
            ));
        }
        // calloc((xs*xd + 2) * (ys*yd + 2) * 2, sizeof(uint16_t)) with `int` arithmetic
        let w = self.xs.checked_mul(self.xd).and_then(|v| v.checked_add(2));
        let h = self.ys.checked_mul(self.yd).and_then(|v| v.checked_add(2));
        let plane = match (w, h) {
            (Some(w), Some(h)) => w.checked_mul(h),
            _ => None,
        };
        let total = plane.and_then(|p| p.checked_mul(2));
        let (plane, total) = match (plane, total) {
            (Some(p), Some(t)) if p >= 0 && t >= 0 => (p as usize, t as usize),
            _ => {
                err("Error : calloc(-) returns NULL.");
                return Err(());
            }
        };
        let mut mem: Vec<u16> = Vec::new();
        if mem.try_reserve_exact(total).is_err() {
            err("Error : calloc(-) returns NULL.");
            return Err(());
        }
        mem.resize(total, 0);
        self.mem = mem;
        self.allocated = true;
        if self.sw.pv {
            msg(&format!("thinnest_ui16_image::mem_alloc() memory <{:x}>", self.mem.as_ptr() as usize as u32));
        }
        self.ch = [0, plane];
        Ok(())
    }

    fn rd(&self, p: Option<usize>) -> Option<u16> {
        p.map(|i| self.mem[i])
    }

    // --- exec01 ---

    fn exec01_fill_noise_pixel(&mut self) -> i32 {
        if self.sw.mv {
            msg("thinnest_ui16_image::exec01_fill_noise_pixel()");
        }
        if self.sw.cv {
            cv_start(self.ys);
        }
        let xs = self.xs as usize;
        let mut y1 = Some(self.src() + xs);
        let mut y2 = Some(self.src());
        let mut y3: Option<usize> = None;
        let mut fill = 0;
        for yy in 0..self.ys {
            if self.sw.cv {
                cv_run(yy);
            }
            fill += self.exec01_scanline(y1, y2, y3);
            y3 = y2;
            y2 = y1;
            if (yy + 1) < self.ys {
                y1 = plus(y1, xs);
            } else {
                y1 = None;
            }
        }
        if self.sw.cv {
            cv_end();
        }
        fill
    }

    fn exec01_scanline(&mut self, y1: Option<usize>, y2: Option<usize>, y3: Option<usize>) -> i32 {
        let (mut x1a, mut x1b, mut x1c) = (None, None, None);
        let (mut x2a, mut x2b, mut x2c) = (None, None, None);
        let (mut x3a, mut x3b, mut x3c) = (None, None, None);
        if let Some(y) = y1 {
            x1a = Some(y + 1);
            x1b = Some(y);
        }
        if let Some(y) = y2 {
            x2a = Some(y + 1);
            x2b = Some(y);
        }
        if let Some(y) = y3 {
            x3a = Some(y + 1);
            x3b = Some(y);
        }
        let mut fill = 0;
        for xx in 0..self.xs {
            fill += self.exec01_pixel(x1a, x1b, x1c, x2a, x2b, x2c, x3a, x3b, x3c);
            x1c = x1b;
            x1b = x1a;
            x2c = x2b;
            x2b = x2a;
            x3c = x3b;
            x3b = x3a;
            if (xx + 1) < self.xs {
                x1a = plus(x1a, 1);
                x2a = plus(x2a, 1);
                x3a = plus(x3a, 1);
            } else {
                x1a = None;
                x2a = None;
                x3a = None;
            }
        }
        fill
    }

    #[allow(clippy::too_many_arguments)]
    fn exec01_pixel(
        &mut self,
        x1a: Option<usize>,
        x1b: Option<usize>,
        x1c: Option<usize>,
        x2a: Option<usize>,
        x2b: Option<usize>,
        x2c: Option<usize>,
        x3a: Option<usize>,
        x3b: Option<usize>,
        x3c: Option<usize>,
    ) -> i32 {
        let mut eqgt = 0;
        let mut exist = 0;
        for p in [x1a, x1b, x1c, x2c, x3c, x3b, x3a, x2a] {
            if let Some(v) = self.rd(p) {
                if self.threshold <= v {
                    eqgt += 1;
                }
                exist += 1;
            }
        }
        if exist == eqgt {
            let i = x2b.expect("x2b");
            self.mem[i] = UINT16_MAX;
            return 1;
        }
        0
    }

    // --- exec02 / exec03 (scale-up; xd,yd < 2 cancels them) ---

    fn edge_value(a: u16, b: u16) -> u16 {
        // neighbour * 2 - next neighbour, clamped to 0..=65535
        let mut t1 = i32::from(a);
        let t2 = i32::from(b);
        t1 *= 2;
        t1 -= t2;
        if t1 < 0 {
            0
        } else if i32::from(UINT16_MAX) < t1 {
            UINT16_MAX
        } else {
            t1 as u16
        }
    }

    fn exec02_scale_add_edge_pixel(&mut self) {
        if self.xs < 2 || self.ys < 2 || self.xd < 2 || self.yd < 2 {
            return;
        }
        if self.sw.mv {
            msg("thinnest_ui16_image::exec02_scale_add_edge_pixel()");
        }
        if self.sw.cv {
            cv_start(self.ys);
        }
        let (xs, ys) = (self.xs as usize, self.ys as usize);
        let (s, t) = (self.src(), self.tgt());
        let mut src = s;
        let mut tgt1 = t + (xs + 2) + 1;
        for yy in 0..self.ys {
            if self.sw.cv {
                cv_run(yy);
            }
            let mut tgt2 = tgt1;
            for _ in 0..xs {
                self.mem[tgt2] = self.mem[src];
                src += 1;
                tgt2 += 1;
            }
            tgt1 += xs + 2;
        }
        if self.sw.cv {
            cv_end();
        }
        // top
        let (mut s1, mut s2, mut tp) = (s, s + xs, t + 1);
        for _ in 0..xs {
            self.mem[tp] = Self::edge_value(self.mem[s1], self.mem[s2]);
            s1 += 1;
            s2 += 1;
            tp += 1;
        }
        // bottom
        let (mut s1, mut s2, mut tp) = (s + (ys - 1) * xs, s + (ys - 2) * xs, t + (ys + 2 - 1) * (xs + 2) + 1);
        for _ in 0..xs {
            self.mem[tp] = Self::edge_value(self.mem[s1], self.mem[s2]);
            s1 += 1;
            s2 += 1;
            tp += 1;
        }
        // left
        let (mut s1, mut s2, mut tp) = (s, s + 1, t + (xs + 2));
        for _ in 0..ys {
            self.mem[tp] = Self::edge_value(self.mem[s1], self.mem[s2]);
            s1 += xs;
            s2 += xs;
            tp += xs + 2;
        }
        // right
        let (mut s1, mut s2, mut tp) = (s + xs - 1, s + xs - 1 - 1, t + (xs + 2) * 2 - 1);
        for _ in 0..ys {
            self.mem[tp] = Self::edge_value(self.mem[s1], self.mem[s2]);
            s1 += xs;
            s2 += xs;
            tp += xs + 2;
        }
        self.xs += 2;
        self.ys += 2;
        self.swap_channel();
    }

    fn exec03_scale_liner(&mut self) {
        if self.xs < 2 || self.ys < 2 || self.xd < 2 || self.yd < 2 {
            return;
        }
        if self.sw.mv {
            msg("thinnest_ui16_image::exec03_scale_liner()");
        }
        if self.sw.mv {
            msg(&format!("thi : Scale {} x {}", self.xd, self.yd));
        }
        if self.sw.cv {
            cv_start((self.ys - 2) * self.yd);
        }
        let s = self.src();
        let mut tgt = self.tgt();
        let tgt_ys = (self.ys - 2) * self.yd;
        let tgt_xs = (self.xs - 2) * self.xd;
        for yy in 0..tgt_ys {
            for xx in 0..tgt_xs {
                if self.sw.cv {
                    cv_run(yy);
                }
                let tgt_xp = ((xx as f64) + 0.5) / (tgt_xs as f64);
                let tgt_yp = ((yy as f64) + 0.5) / (tgt_ys as f64);
                let src_xp = ((self.xs as f64) - 2.0) * tgt_xp + 0.5;
                let src_yp = ((self.ys as f64) - 2.0) * tgt_yp + 0.5;
                let xpos1 = src_xp.floor() as i32;
                let ypos1 = src_yp.floor() as i32;
                let xpos2 = src_xp.ceil() as i32;
                let ypos2 = src_yp.ceil() as i32;
                let (xratio1, xratio2) = if xpos1 == xpos2 {
                    (0.0, 1.0)
                } else {
                    (src_xp - src_xp.floor(), src_xp.ceil() - src_xp)
                };
                let (yratio1, yratio2) = if ypos1 == ypos2 {
                    (0.0, 1.0)
                } else {
                    (src_yp - src_yp.floor(), src_yp.ceil() - src_yp)
                };
                let at = |y: i32, x: i32| -> f64 { f64::from(self.mem[s + (self.xs * y + x) as usize]) };
                let tmp = (at(ypos1, xpos1) * xratio2 + at(ypos1, xpos2) * xratio1) * yratio2
                    + (at(ypos2, xpos1) * xratio2 + at(ypos2, xpos2) * xratio1) * yratio1;
                self.mem[tgt] = if f64::from(UINT16_MAX) <= tmp { UINT16_MAX } else { tmp as u16 };
                tgt += 1;
            }
        }
        if self.sw.cv {
            cv_end();
        }
        self.xs = (self.xs - 2) * self.xd;
        self.ys = (self.ys - 2) * self.yd;
        self.swap_channel();
    }

    fn exec04_bw(&mut self) {
        if self.sw.mv {
            msg("thinnest_ui16_image::exec04_bw()");
        }
        if self.sw.pv {
            msg(&format!("thi : threshold {}", self.threshold));
        }
        if self.sw.cv {
            cv_start(self.ys);
        }
        let (mut s, mut t) = (self.src(), self.tgt());
        for yy in 0..self.ys {
            if self.sw.cv {
                cv_run(yy);
            }
            for _ in 0..self.xs {
                self.mem[t] = if self.threshold <= self.mem[s] { UINT16_MAX } else { 0 };
                s += 1;
                t += 1;
            }
        }
        if self.sw.cv {
            cv_end();
        }
        self.swap_channel();
    }

    fn exec05_thin(&mut self) -> Res {
        if self.sw.mv {
            msg("thinnest_ui16_image::exec05_thin()");
        }
        let mut total = 0i32;
        let mut one_side = [0i32; 4];
        let mut ii = 0i32;
        loop {
            if self.exec_loop_count <= ii {
                err(&format!("Error : loop counter over {}.", self.exec_loop_count));
                return Err(());
            }
            let mut one_round = 0i32;
            for jj in 0..4 {
                let n = self.one_side_thinner();
                one_round += n;
                total += n;
                one_side[jj] += n;
                self.rot90_by_clockwork();
                if self.sw.cv {
                    emit(false, ".");
                }
            }
            if self.sw.cv {
                emit(false, "|");
                if 9 == (ii % 10) {
                    emit(false, "\n");
                }
            }
            if one_round <= 0 {
                break;
            }
            ii += 1;
        }
        if self.sw.cv {
            emit(false, "\nthin line ... end.\n");
        }
        if self.sw.pv {
            msg(&format!("thi : total {} loop, and {} pixel deleted", ii, total));
            msg(&format!(
                "thi : and each r<{}>+ t<{}>+ l<{}>+ b<{}> pixel deleted",
                one_side[0], one_side[1], one_side[2], one_side[3]
            ));
        }
        Ok(())
    }

    fn one_side_thinner(&mut self) -> i32 {
        let xs = self.xs as usize;
        let mut y1 = Some(self.src() + xs);
        let mut y2 = Some(self.src());
        let mut y3: Option<usize> = None;
        let mut tgt = self.tgt();
        let mut deleted = 0;
        for yy in 0..self.ys {
            deleted += self.one_side_thinner_scanline(y1, y2, y3, tgt);
            y3 = y2;
            y2 = y1;
            if (yy + 1) < self.ys {
                y1 = plus(y1, xs);
            } else {
                y1 = None;
            }
            tgt += xs;
        }
        self.swap_channel();
        deleted
    }

    fn one_side_thinner_scanline(
        &mut self,
        y1: Option<usize>,
        y2: Option<usize>,
        y3: Option<usize>,
        tgt: usize,
    ) -> i32 {
        let (mut x1a, mut x1b, mut x1c) = (None, None, None);
        let (mut x2a, mut x2b, mut x2c) = (None, None, None);
        let (mut x3a, mut x3b, mut x3c) = (None, None, None);
        if let Some(y) = y1 {
            x1a = Some(y + 1);
            x1b = Some(y);
        }
        if let Some(y) = y2 {
            x2a = Some(y + 1);
            x2b = Some(y);
        }
        if let Some(y) = y3 {
            x3a = Some(y + 1);
            x3b = Some(y);
        }
        let mut x2c_before: u16 = 0;
        let mut tgt = tgt;
        let mut deleted = 0;
        for xx in 0..self.xs {
            let cur = x2b.expect("x2b");
            let x2b_before = self.mem[cur];
            if (x2c.is_none() || 0 == x2c_before) && 0 < self.mem[cur] {
                deleted += self.one_side_thinner_pixel(x1a, x1b, x1c, x2a, x2b, x2c, x3a, x3b, x3c, tgt);
            } else {
                self.mem[tgt] = self.mem[cur];
            }
            x2c_before = x2b_before;
            x1c = x1b;
            x1b = x1a;
            x2c = x2b;
            x2b = x2a;
            x3c = x3b;
            x3b = x3a;
            if (xx + 1) < self.xs {
                x1a = plus(x1a, 1);
                x2a = plus(x2a, 1);
                x3a = plus(x3a, 1);
            } else {
                x1a = None;
                x2a = None;
                x3a = None;
            }
            tgt += 1;
        }
        deleted
    }

    #[allow(clippy::too_many_arguments)]
    fn one_side_thinner_pixel(
        &mut self,
        x1a: Option<usize>,
        x1b: Option<usize>,
        x1c: Option<usize>,
        x2a: Option<usize>,
        x2b: Option<usize>,
        x2c: Option<usize>,
        x3a: Option<usize>,
        x3b: Option<usize>,
        x3c: Option<usize>,
        tgt: usize,
    ) -> i32 {
        let mut deleted = 0;
        let mut off_count = 0i64;
        let mut white_count = 0i64;
        let mut sw2 = false;
        for p in [x1a, x1b, x1c, x2c, x3c, x3b, x3a, x2a] {
            if let Some(v) = self.rd(p) {
                let sw = 0 < v;
                if sw {
                    white_count += 1;
                }
                if !sw && sw2 {
                    off_count += 1;
                }
                sw2 = sw;
            }
        }
        if let Some(v) = self.rd(x1a) {
            let sw = 0 < v;
            if !sw && sw2 {
                off_count += 1;
            }
            let _ = sw;
        }
        if white_count <= 0 {
            self.mem[tgt] = 0;
            deleted += 1;
        } else if 1 == off_count && 3 <= white_count {
            self.mem[tgt] = 0;
            deleted += 1;
        } else {
            self.mem[tgt] = self.mem[x2b.expect("x2b")];
        }
        deleted
    }

    fn rot90_by_clockwork(&mut self) {
        let (xs, ys) = (self.xs as usize, self.ys as usize);
        let mut src = self.src();
        let mut tgt_y = self.tgt().wrapping_add(ys.wrapping_sub(1));
        for _ in 0..ys {
            let mut tgt_x = tgt_y;
            for _ in 0..xs {
                self.mem[tgt_x] = self.mem[src];
                src += 1;
                tgt_x = tgt_x.wrapping_add(ys);
            }
            tgt_y = tgt_y.wrapping_sub(1);
        }
        std::mem::swap(&mut self.xs, &mut self.ys);
        std::mem::swap(&mut self.xd, &mut self.yd);
        self.swap_channel();
    }
}

// ---------------------------------------------------------------------------------------
// brush_curve_blur

const CHANNEL_COUNT: usize = 4;

struct CurveBlur {
    sw: Sw,
    count: i32,
    subpixel_divide: i32,
    effect_area_radius: f64,
    power: f64,
    allocated: bool,
    ratio: Vec<f64>,
    linepixels: Vec<f64>,
    xp: Vec<f64>,
    yp: Vec<f64>,
    subpixel: Vec<f64>,
    pixel: [f64; CHANNEL_COUNT],
}

impl CurveBlur {
    fn new() -> Self {
        CurveBlur {
            sw: Sw::default(),
            count: 51,
            subpixel_divide: 2,
            effect_area_radius: 25.0,
            power: 1.0,
            allocated: false,
            ratio: Vec::new(),
            linepixels: Vec::new(),
            xp: Vec::new(),
            yp: Vec::new(),
            subpixel: Vec::new(),
            pixel: [0.0; CHANNEL_COUNT],
        }
    }

    fn mem_free(&mut self) {
        if self.allocated {
            if self.sw.mv {
                msg("brush_curve_blur::mem_free()");
            }
            self.ratio = Vec::new();
            self.linepixels = Vec::new();
            self.xp = Vec::new();
            self.yp = Vec::new();
            self.subpixel = Vec::new();
            self.allocated = false;
        }
    }

    fn mem_alloc(&mut self) -> Res {
        self.mem_free();
        if self.count <= 0 {
            return Ok(());
        }
        if self.sw.mv {
            msg("brush_curve_blur::mem_alloc()");
        }
        if self.sw.pv {
            msg(&format!(
                "alloc brush_curve_blur memory (({} * {}) + {}) x {} bytes",
                self.count,
                1 + CHANNEL_COUNT + 2,
                self.subpixel_divide.wrapping_mul(self.subpixel_divide).wrapping_mul(CHANNEL_COUNT as i32),
                8
            ));
        }
        // calloc(count*(1+4+2) + sd*sd*4, sizeof(double)) with `int` arithmetic
        let sub = self
            .subpixel_divide
            .checked_mul(self.subpixel_divide)
            .and_then(|v| v.checked_mul(CHANNEL_COUNT as i32));
        let line = self.count.checked_mul((1 + CHANNEL_COUNT + 2) as i32);
        let total = match (line, sub) {
            (Some(a), Some(b)) => a.checked_add(b),
            _ => None,
        };
        let (n, sub) = match (total, sub) {
            (Some(t), Some(s)) if t >= 0 => (self.count as usize, s as usize),
            _ => {
                err("Error : calloc(-) returns NULL.");
                return Err(());
            }
        };
        let alloc = |len: usize| -> Option<Vec<f64>> {
            let mut v: Vec<f64> = Vec::new();
            v.try_reserve_exact(len).ok()?;
            v.resize(len, 0.0);
            Some(v)
        };
        match (alloc(n), alloc(n * CHANNEL_COUNT), alloc(n), alloc(n), alloc(sub)) {
            (Some(a), Some(b), Some(c), Some(d), Some(e)) => {
                self.ratio = a;
                self.linepixels = b;
                self.xp = c;
                self.yp = d;
                self.subpixel = e;
                self.allocated = true;
                Ok(())
            }
            _ => {
                err("Error : calloc(-) returns NULL.");
                Err(())
            }
        }
    }

    fn init_ratio_array(&mut self) {
        if self.count <= 0 {
            return;
        }
        let count = self.count;
        for ii in 1..count / 2 {
            self.ratio[ii as usize] = f64::from(ii);
        }
        for ii in count / 2..count {
            self.ratio[ii as usize] = f64::from(count - ii);
        }
        for ii in 0..count as usize {
            self.ratio[ii] = self.ratio[ii].powf(1.0 / self.power);
        }
        let mut total = 0.0f64;
        for ii in 0..count as usize {
            total += self.ratio[ii];
        }
        for ii in 0..count as usize {
            self.ratio[ii] /= total;
        }
    }

    fn set_subpixel_value(&mut self, x_sub: i32, y_sub: i32) {
        if self.count <= 0 {
            return;
        }
        for zz in 0..CHANNEL_COUNT {
            let idx = ((self.subpixel_divide * y_sub + x_sub) as usize) * CHANNEL_COUNT + zz;
            self.subpixel[idx] = 0.0;
            let mut accum = 0.0f64;
            for ii in 0..self.count as usize {
                if self.linepixels[ii * CHANNEL_COUNT + zz] < 0.0 {
                    continue;
                }
                self.subpixel[idx] += self.linepixels[ii * CHANNEL_COUNT + zz] * self.ratio[ii];
                accum += self.ratio[ii];
            }
            self.subpixel[idx] /= accum;
        }
    }

    fn set_pixel_value(&mut self) {
        if self.count <= 0 {
            return;
        }
        for zz in 0..CHANNEL_COUNT {
            self.pixel[zz] = 0.0;
            for yy in 0..self.subpixel_divide {
                for xx in 0..self.subpixel_divide {
                    self.pixel[zz] += self.subpixel[((self.subpixel_divide * yy + xx) as usize) * CHANNEL_COUNT + zz];
                }
            }
            self.pixel[zz] /= f64::from(self.subpixel_divide.wrapping_mul(self.subpixel_divide));
        }
    }
}

// ---------------------------------------------------------------------------------------
// brush_smudge_circle

struct Smudge {
    sw: Sw,
    size_by_pixel: i32,
    subpixel_divide: i32,
    ratio: f64,
    allocated: bool,
    brush: Vec<f64>,
    subpixel_image: Vec<f64>,
    pixel_image: Vec<f64>,
}

impl Smudge {
    fn new() -> Self {
        Smudge {
            sw: Sw::default(),
            size_by_pixel: 7,
            subpixel_divide: 4,
            ratio: 0.85,
            allocated: false,
            brush: Vec::new(),
            subpixel_image: Vec::new(),
            pixel_image: Vec::new(),
        }
    }

    fn mem_free(&mut self) {
        if self.allocated {
            if self.sw.mv {
                msg("brush_smudge_circle::mem_free()");
            }
            self.brush = Vec::new();
            self.subpixel_image = Vec::new();
            self.pixel_image = Vec::new();
            self.allocated = false;
        }
    }

    fn mem_alloc(&mut self) -> Res {
        self.mem_free();
        let sz = self.size_by_pixel.wrapping_mul(self.subpixel_divide);
        if self.sw.mv {
            msg("brush_smudge_circle::mem_alloc()");
        }
        if self.sw.pv {
            msg(&format!(
                "calloc(({} x {} + {} x {} + {} x {}) x ({} x {}))",
                sz,
                sz,
                sz,
                sz,
                self.size_by_pixel.wrapping_add(1),
                self.size_by_pixel.wrapping_add(1),
                8,
                5
            ));
        }
        let sq = self.size_by_pixel.checked_mul(self.subpixel_divide).and_then(|s| s.checked_mul(s));
        let side = self.size_by_pixel.checked_add(1).and_then(|s| s.checked_mul(s));
        let total = match (sq, side) {
            (Some(q), Some(p)) => q.checked_add(q).and_then(|v| v.checked_add(p)),
            _ => None,
        };
        let (q, p) = match (total, sq, side) {
            (Some(t), Some(q), Some(p)) if t >= 0 && q >= 0 && p >= 0 => (q as usize, p as usize),
            _ => {
                err("Error : calloc(-) returns NULL.");
                return Err(());
            }
        };
        let alloc = |len: usize| -> Option<Vec<f64>> {
            let mut v: Vec<f64> = Vec::new();
            v.try_reserve_exact(len).ok()?;
            v.resize(len, 0.0);
            Some(v)
        };
        match (alloc(q * 5), alloc(q * 5), alloc(p * 5)) {
            (Some(a), Some(b), Some(c)) => {
                self.brush = a;
                self.subpixel_image = b;
                self.pixel_image = c;
                self.allocated = true;
                Ok(())
            }
            _ => {
                err("Error : calloc(-) returns NULL.");
                Err(())
            }
        }
    }

    fn get_dp_area(&self, xp: f64, yp: f64) -> (f64, f64, f64, f64) {
        let x1 = xp + 0.5 - f64::from(self.size_by_pixel) / 2.0;
        let x2 = x1 + f64::from(self.size_by_pixel);
        let y1 = yp + 0.5 - f64::from(self.size_by_pixel) / 2.0;
        let y2 = y1 + f64::from(self.size_by_pixel);
        (x1, y1, x2, y2)
    }

    fn set_brush_circle(&mut self) {
        let size = self.size_by_pixel.wrapping_mul(self.subpixel_divide);
        let radius = f64::from(size) / 2.0;
        let mut b = 0usize;
        for yy in 0..size {
            for xx in 0..size {
                let tmp = ((f64::from(xx) + 0.5 - radius) * (f64::from(xx) + 0.5 - radius)
                    + (f64::from(yy) + 0.5 - radius) * (f64::from(yy) + 0.5 - radius))
                    .sqrt();
                self.brush[b + 4] = if tmp < radius { 1.0 } else { 0.0 };
                b += 5;
            }
        }
    }

    fn copy_to_brush_from_image(&mut self) {
        let size = self.size_by_pixel.wrapping_mul(self.subpixel_divide);
        let mut o = 0usize;
        for _yy in 0..size {
            for _xx in 0..size {
                if 0.0 < self.brush[o + 4] {
                    for zz in 0..4 {
                        self.brush[o + zz] = self.subpixel_image[o + zz];
                    }
                }
                o += 5;
            }
        }
    }

    fn exec(&mut self) {
        let size = self.size_by_pixel.wrapping_mul(self.subpixel_divide);
        let mut o = 0usize;
        for _yy in 0..size {
            for _xx in 0..size {
                if 0.0 < self.brush[o + 4] {
                    for zz in 0..4 {
                        self.subpixel_image[o + zz] +=
                            (self.brush[o + zz] - self.subpixel_image[o + zz]) * self.ratio;
                        self.brush[o + zz] = self.subpixel_image[o + zz];
                    }
                }
                o += 5;
            }
        }
    }

    fn to_subpixel_from_pixel(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
        let subsize = 1.0 / f64::from(self.subpixel_divide);
        let x1floor = (x1 + subsize / 2.0).floor();
        let y1floor = (y1 + subsize / 2.0).floor();
        let xsize = ((x2 - subsize / 2.0).floor() as i32).wrapping_sub(x1floor as i32).wrapping_add(1);

        let mut img = 0usize;
        let mut y = y1 + subsize / 2.0;
        while y < y2 {
            let mut x = x1 + subsize / 2.0;
            while x < x2 {
                let xsave = x - x1floor;
                let ysave = y - y1floor;
                let base = (ysave as i32).wrapping_mul(xsize).wrapping_mul(5).wrapping_add((xsave as i32).wrapping_mul(5));
                for zz in 0..5 {
                    self.subpixel_image[img + zz] = self.pixel_image[(base as isize + zz as isize) as usize];
                }
                x += subsize;
                img += 5;
            }
            y += subsize;
        }
    }

    fn to_pixel_from_subpixel(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
        let subsize = 1.0 / f64::from(self.subpixel_divide);
        for v in self.pixel_image.iter_mut().take((self.size_by_pixel as usize + 1) * (self.size_by_pixel as usize + 1) * 5) {
            *v = 0.0;
        }
        let x1floor = (x1 + subsize / 2.0).floor();
        let y1floor = (y1 + subsize / 2.0).floor();
        let xsize = ((x2 - subsize / 2.0).floor() as i32).wrapping_sub(x1floor as i32).wrapping_add(1);

        let mut img = 0usize;
        let mut y = y1 + subsize / 2.0;
        while y < y2 {
            let mut x = x1 + subsize / 2.0;
            while x < x2 {
                let xsave = x - x1floor;
                let ysave = y - y1floor;
                let base = (ysave as i32).wrapping_mul(xsize).wrapping_mul(5).wrapping_add((xsave as i32).wrapping_mul(5));
                for zz in 0..5 {
                    self.pixel_image[(base as isize + zz as isize) as usize] += self.subpixel_image[img + zz];
                }
                x += subsize;
                img += 5;
            }
            y += subsize;
        }
        let n = (self.size_by_pixel as usize + 1) * (self.size_by_pixel as usize + 1) * 5;
        let div = f64::from(self.subpixel_divide.wrapping_mul(self.subpixel_divide));
        for v in self.pixel_image.iter_mut().take(n) {
            *v /= div;
        }
    }
}

// ---------------------------------------------------------------------------------------
// image access and the brush drivers (igs_line_blur_*_)

/// A sample type of the image (`unsigned char` / `unsigned short`).
trait Sample: Copy {
    const BYTES: usize;
    fn to_f64(self) -> f64;
    fn from_f64(v: f64) -> Self;
}
impl Sample for u8 {
    const BYTES: usize = 1;
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
    fn from_f64(v: f64) -> Self {
        v as u8
    }
}
impl Sample for u16 {
    const BYTES: usize = 2;
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
    fn from_f64(v: f64) -> Self {
        v as u16
    }
}

/// Read-only view of the caller's image. The pointer, strides and sizes are the ones
/// `convert` validated; indices follow the C++ `y * channels * wrap + x * channels + z`.
#[derive(Clone, Copy)]
struct Src<T> {
    p: *const T,
}
impl<T: Sample> Src<T> {
    fn at(&self, idx: i64) -> T {
        // SAFETY: the caller of `convert` guarantees the image extent (height x wrap x
        // channels); every access below stays inside it like the C++ did.
        unsafe { std::ptr::read_unaligned(self.p.offset(idx as isize)) }
    }
}
#[derive(Clone, Copy)]
struct Dst<T> {
    p: *mut T,
}
impl<T: Sample> Dst<T> {
    fn at(&self, idx: i64) -> T {
        // SAFETY: see `Src::at`.
        unsafe { std::ptr::read_unaligned(self.p.offset(idx as isize)) }
    }
    fn set(&self, idx: i64, v: T) {
        // SAFETY: see `Src::at`.
        unsafe { std::ptr::write_unaligned(self.p.offset(idx as isize), v) }
    }
}

/// Geometry and strides of both images.
#[derive(Clone, Copy)]
struct Geo {
    height: i32,
    width: i32,
    in_wrap: i32,
    out_wrap: i32,
    channels: i32,
}

impl Geo {
    fn in_idx(&self, y: i32, x: i32, z: i32) -> i64 {
        i64::from(y) * i64::from(self.channels) * i64::from(self.in_wrap) + i64::from(x) * i64::from(self.channels) + i64::from(z)
    }
    fn out_idx(&self, y: i32, x: i32, z: i32) -> i64 {
        i64::from(y) * i64::from(self.channels) * i64::from(self.out_wrap) + i64::from(x) * i64::from(self.channels) + i64::from(z)
    }
}

fn curve_point_put_image<T: Sample>(cb: &CurveBlur, xp: i32, yp: i32, geo: &Geo, out: Dst<T>) {
    for zz in 0..geo.channels {
        out.set(geo.out_idx(yp, xp, zz), T::from_f64(cb.pixel[zz as usize]));
    }
}

fn curve_line_get_image<T: Sample>(cb: &mut CurveBlur, geo: &Geo, inp: Src<T>, xp: f64, yp: f64) {
    for ii in 0..cb.count as usize {
        let xx = (cb.xp[ii] + xp + 0.5) as i32;
        let yy = (cb.yp[ii] + yp + 0.5) as i32;
        if 0 <= xx && xx < geo.width && 0 <= yy && yy < geo.height {
            for zz in 0..geo.channels {
                cb.linepixels[ii * CHANNEL_COUNT + zz as usize] = inp.at(geo.in_idx(yy, xx, zz)).to_f64() + 0.999999;
            }
        } else {
            for zz in 0..geo.channels {
                cb.linepixels[ii * CHANNEL_COUNT + zz as usize] = -1.0;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn curve_blur_subpixel<T: Sample>(
    cb: &mut CurveBlur,
    sel: &mut CurveSelRoot,
    lr: &LineRoot,
    pts: &Points,
    geo: &Geo,
    inp: Src<T>,
    xx: i32,
    yy: i32,
) -> Res {
    let sub = cb.subpixel_divide;
    for ysub in 0..sub {
        for xsub in 0..sub {
            let px = f64::from(xx) + f64::from(xsub) / f64::from(sub) - 0.5;
            let py = f64::from(yy) + f64::from(ysub) / f64::from(sub) - 0.5;
            sel.exec(px, py, lr, pts, cb.count, cb.effect_area_radius);
            if sel.l.count <= 0 {
                return Err(());
            }
            let count = cb.count;
            let (mut xv, mut yv) = (std::mem::take(&mut cb.xp), std::mem::take(&mut cb.yp));
            let r = sel.get_line(&lr.l, pts, count, &mut xv, &mut yv);
            cb.xp = xv;
            cb.yp = yv;
            let _ = r;
            curve_line_get_image(cb, geo, inp, px, py);
            cb.set_subpixel_value(xsub, ysub);
        }
    }
    Ok(())
}

/// Row-wise copy of the image from `in` to `out` (the C++ `memcpy` loop).
fn copy_rows(geo: &Geo, bytes: usize, inp: *const u8, out: *mut u8) {
    let row = geo.width as usize * geo.channels as usize * bytes;
    for yy in 0..geo.height as i64 {
        let so = yy * i64::from(geo.in_wrap) * i64::from(geo.channels) * bytes as i64;
        let dof = yy * i64::from(geo.out_wrap) * i64::from(geo.channels) * bytes as i64;
        // SAFETY: both images hold `height` rows of `wrap * channels` samples (caller's
        // contract); `copy` tolerates identical/overlapping buffers.
        unsafe { std::ptr::copy(inp.offset(so as isize), out.offset(dof as isize), row) };
    }
}

struct Ctx {
    thin: Thin,
    points: PointRoot,
    lines: LineRoot,
    same_way: SameWayRoot,
    curve_sel: CurveSelRoot,
    smudge: Smudge,
    curve: CurveBlur,
}

impl Ctx {
    #[allow(clippy::too_many_arguments)]
    fn curve_blur_all<T: Sample>(
        &mut self,
        sw: Sw,
        geo: &Geo,
        inp: Src<T>,
        out: Dst<T>,
        in_bytes: *const u8,
        out_bytes: *mut u8,
        bytes: usize,
    ) -> Result<(), i32> {
        if sw.mv {
            emit(false, "igs::line_blur::_brush_curve_blur_all()\n");
        }
        if sw.pv {
            emit(
                false,
                &format!(
                    " curve blur points count is \n{}\n power is \n{}\n subpixel divide is \n{}\n clip area for speedup is \n{}\n",
                    self.curve.count,
                    g(self.curve.power),
                    self.curve.subpixel_divide,
                    g(self.curve.effect_area_radius)
                ),
            );
        }
        if self.curve.mem_alloc().is_err() {
            return Err(CURVE_ALLOC);
        }
        self.curve.init_ratio_array();
        copy_rows(geo, bytes, in_bytes, out_bytes);
        if sw.cv {
            cv_start(geo.height);
        }
        for yy in 0..geo.height {
            if sw.cv {
                cv_run(yy);
            }
            for xx in 0..geo.width {
                if curve_blur_subpixel(
                    &mut self.curve,
                    &mut self.curve_sel,
                    &self.lines,
                    &self.points.l,
                    geo,
                    inp,
                    xx,
                    yy,
                )
                .is_ok()
                {
                    self.curve.set_pixel_value();
                    curve_point_put_image(&self.curve, xx, yy, geo, out);
                }
            }
        }
        if sw.cv {
            cv_end();
        }
        self.curve.mem_free();
        Ok(())
    }

    fn smudge_get_image<T: Sample>(&mut self, geo: &Geo, inp: Src<T>, xp: f64, yp: f64) {
        let (x1, y1, x2, y2) = self.smudge.get_dp_area(xp, yp);
        let subsize = 1.0 / f64::from(self.smudge.subpixel_divide);
        let x1p = (x1 + subsize / 2.0).floor() as i32;
        let x2p = (x2 - subsize / 2.0).floor() as i32;
        let y1p = (y1 + subsize / 2.0).floor() as i32;
        let y2p = (y2 - subsize / 2.0).floor() as i32;
        let mut o = 0usize;
        let mut yy = y1p;
        while yy <= y2p {
            let mut xx = x1p;
            while xx <= x2p {
                let img = &mut self.smudge.pixel_image;
                if 0 <= xx && xx < geo.width && 0 <= yy && yy < geo.height {
                    for zz in 0..4 {
                        if zz < geo.channels {
                            img[o + zz as usize] = inp.at(geo.in_idx(yy, xx, zz)).to_f64() + 0.999999;
                        } else {
                            img[o + zz as usize] = 0.0;
                        }
                    }
                    img[o + 4] = 1.0;
                } else {
                    img[o + 4] = 0.0;
                }
                xx += 1;
                o += 5;
            }
            yy += 1;
        }
    }

    fn smudge_put_image<T: Sample>(&mut self, geo: &Geo, out: Dst<T>, xp: f64, yp: f64) {
        let (x1, y1, x2, y2) = self.smudge.get_dp_area(xp, yp);
        let subsize = 1.0 / f64::from(self.smudge.subpixel_divide);
        let x1p = (x1 + subsize / 2.0).floor() as i32;
        let x2p = (x2 - subsize / 2.0).floor() as i32;
        let y1p = (y1 + subsize / 2.0).floor() as i32;
        let y2p = (y2 - subsize / 2.0).floor() as i32;
        let mut o = 0usize;
        let mut yy = y1p;
        while yy <= y2p {
            let mut xx = x1p;
            while xx <= x2p {
                let img = &self.smudge.pixel_image;
                if 0.0 < img[o + 4] && 0 <= xx && xx < geo.width && 0 <= yy && yy < geo.height {
                    for zz in 0..geo.channels {
                        let idx = geo.out_idx(yy, xx, zz);
                        out.set(idx, T::from_f64(out.at(idx).to_f64() * (1.0 - img[o + 4]) + img[o + zz as usize]));
                    }
                }
                xx += 1;
                o += 5;
            }
            yy += 1;
        }
    }

    /// Drag the brush from `start` along `next_point` (`forward`) or `prev_point`.
    fn smudge_stroke<T: Sample>(
        &mut self,
        geo: &Geo,
        inp: Src<T>,
        out: Dst<T>,
        line: &LineData,
        forward: bool,
    ) -> Result<(), i32> {
        let pts = &self.points.l;
        let mut crnt = line.link[L_MIDDLE];
        let (cx, cy) = (pts.v[crnt].d.xt, pts.v[crnt].d.yt);
        self.smudge_get_image(geo, inp, cx, cy);
        let (x1, y1, x2, y2) = self.smudge.get_dp_area(cx, cy);
        self.smudge.to_subpixel_from_pixel(x1, y1, x2, y2);
        self.smudge.copy_to_brush_from_image();

        let step = |pts: &Points, i: usize| if forward { pts.v[i].d.next_point } else { pts.v[i].d.prev_point };
        crnt = step(&self.points.l, crnt);
        let mut ii = 0;
        while crnt != NIL {
            if line.point_count <= ii {
                return Err(if forward { OVER_POINTS } else { OVER_POINTS_FRONT });
            }
            let (px, py) = (self.points.l.v[crnt].d.xt, self.points.l.v[crnt].d.yt);
            let (x1, y1, x2, y2) = self.smudge.get_dp_area(px, py);
            if 0.0 <= x2 && x1 < f64::from(geo.width) && 0.0 <= y2 && y1 < f64::from(geo.height) {
                self.smudge_get_image(geo, inp, px, py);
                self.smudge.to_subpixel_from_pixel(x1, y1, x2, y2);
                self.smudge.exec();
                self.smudge.to_pixel_from_subpixel(x1, y1, x2, y2);
                self.smudge_put_image(geo, out, px, py);
            }
            crnt = step(&self.points.l, crnt);
            ii += 1;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn smudge_all<T: Sample>(
        &mut self,
        sw: Sw,
        geo: &Geo,
        inp: Src<T>,
        out: Dst<T>,
        in_bytes: *const u8,
        out_bytes: *mut u8,
        bytes: usize,
    ) -> Result<(), i32> {
        if sw.mv {
            emit(false, "igs::line_expand::_brush_smudge_all()\n");
        }
        if sw.pv {
            emit(
                false,
                &format!(
                    " smudge ratio {}\n smudge brush size by pixel {}\n smudge subpixel divide {}\n",
                    g(self.smudge.ratio),
                    self.smudge.size_by_pixel,
                    self.smudge.subpixel_divide
                ),
            );
        }
        if self.smudge.mem_alloc().is_err() {
            return Err(SMUDGE_ALLOC);
        }
        copy_rows(geo, bytes, in_bytes, out_bytes);
        if sw.cv {
            cv_start(self.lines.l.count);
        }
        let mut cl = self.lines.l.first;
        let mut ii = 0;
        while cl != NIL {
            if self.lines.l.count <= ii {
                return Err(OVER_LINES);
            }
            if sw.cv {
                cv_run(ii);
            }
            let line = self.lines.l.v[cl].d.clone();
            self.smudge.set_brush_circle();
            self.smudge_stroke(geo, inp, out, &line, true)?;
            self.smudge_stroke(geo, inp, out, &line, false)?;
            cl = self.lines.l.next(cl);
            ii += 1;
        }
        if sw.cv {
            cv_end();
        }
        Ok(())
    }

    /// Destructor order of the C++ locals (reverse of construction).
    fn teardown(&mut self) {
        self.curve.mem_free();
        self.smudge.mem_free();
        self.curve_sel.mem_free();
        self.same_way.mem_free();
        self.lines.mem_free();
        self.points.mem_free();
        self.thin.mem_free();
    }
}

/// All the arguments of `igs::line_blur::convert`.
#[derive(Clone, Copy)]
pub struct Args {
    pub inp: *const c_void,
    pub out: *mut c_void,
    pub height: i32,
    pub width: i32,
    pub in_wrap: i32,
    pub out_wrap: i32,
    pub channels: i32,
    pub bits: i32,
    pub b_blur_count: i32,
    pub b_blur_power: f64,
    pub b_subpixel: i32,
    pub b_blur_near_ref: i32,
    pub b_blur_near_len: i32,
    pub b_smudge_thick: i32,
    pub b_smudge_remain: f64,
    pub v_smooth_retry: i32,
    pub v_near_ref: i32,
    pub v_near_len: i32,
    pub mv_sw: bool,
    pub pv_sw: bool,
    pub cv_sw: bool,
    pub reference_channel: i64,
    pub debug_save_sw: bool,
    pub brush_action: i32,
}

/// The extent of a debug file write: `fopen(path, "w")` + content.
fn save_file(dir: &Option<PathBuf>, name: &str, content: &str) -> Res {
    let path: PathBuf = match dir {
        Some(d) => d.join(name),
        None => PathBuf::from(name),
    };
    match std::fs::write(&path, content) {
        Ok(()) => Ok(()),
        Err(_) => {
            err(&format!("Error : fopen({},w) returns NULL", name));
            Err(())
        }
    }
}

fn image_get<T: Sample>(
    sw: Sw,
    reference_channel: i64,
    thin: &mut Thin,
    inp: Src<T>,
    in_wrap: i32,
    channels: i32,
    conv: impl Fn(T) -> u16,
) {
    if sw.mv {
        emit(
            false,
            &format!("igs_line_blur_image_get_()\ncom : reference channel {}\n", reference_channel),
        );
    }
    let xs = thin.xs;
    let ys = thin.ys;
    let mut out_incr = thin.src();
    if sw.cv {
        cv_start(ys);
    }
    for yy in 0..ys {
        if sw.cv {
            cv_run(yy);
        }
        let mut idx = i64::from(yy) * i64::from(in_wrap) * i64::from(channels) + reference_channel;
        for _xx in 0..xs {
            thin.mem[out_incr] = conv(inp.at(idx));
            idx += i64::from(channels);
            out_incr += 1;
        }
    }
    if sw.cv {
        cv_end();
    }
}

fn run(ctx: &mut Ctx, a: &Args, dir: &Option<PathBuf>) -> Result<(), i32> {
    let sw = Sw { mv: a.mv_sw, pv: a.pv_sw, cv: a.cv_sw };
    ctx.thin.sw = sw;
    ctx.points.sw = sw;
    ctx.lines.sw = sw;
    ctx.smudge.sw = sw;
    ctx.curve.sw = sw;

    if sw.mv {
        emit(false, "igs::line_blur::convert()\n");
    }
    ctx.thin.xs = a.width;
    ctx.thin.ys = a.height;

    ctx.curve.count = a.b_blur_count;
    ctx.curve.effect_area_radius = f64::from(a.b_blur_count / 2);
    ctx.curve.power = a.b_blur_power;
    ctx.curve.subpixel_divide = a.b_subpixel;
    ctx.curve_sel.count_max = a.b_blur_near_ref;
    ctx.curve_sel.length_max = f64::from(a.b_blur_near_len);
    ctx.smudge.size_by_pixel = a.b_smudge_thick;
    ctx.smudge.ratio = a.b_smudge_remain;
    ctx.lines.smooth_retry = a.v_smooth_retry;
    ctx.same_way.count_max = a.v_near_ref;
    ctx.same_way.length_max = f64::from(a.v_near_len);

    if ctx.thin.mem_alloc().is_err() {
        return Err(THIN_ALLOC);
    }

    let geo = Geo {
        height: a.height,
        width: a.width,
        in_wrap: a.in_wrap,
        out_wrap: a.out_wrap,
        channels: a.channels,
    };
    match a.bits {
        8 => image_get(sw, a.reference_channel, &mut ctx.thin, Src { p: a.inp as *const u8 }, a.in_wrap, a.channels, |v: u8| {
            (u16::from(v) << 8) + u16::from(v)
        }),
        16 => image_get(sw, a.reference_channel, &mut ctx.thin, Src { p: a.inp as *const u16 }, a.in_wrap, a.channels, |v: u16| v),
        _ => return Err(BAD_BITS),
    }

    // thinning
    ctx.thin.exec01_fill_noise_pixel();
    ctx.thin.exec02_scale_add_edge_pixel();
    ctx.thin.exec03_scale_liner();
    ctx.thin.exec04_bw();
    if ctx.thin.exec05_thin().is_err() {
        return Err(THIN_EXEC);
    }

    // vector lists
    let (xs, ys) = (ctx.thin.xs, ctx.thin.ys);
    let (s0, n) = (ctx.thin.src(), (xs as usize) * (ys as usize));
    if ctx.points.alloc_mem_and_list_node(xs, ys, &ctx.thin.mem[s0..s0 + n]).is_err() {
        return Err(POINT_LIST);
    }
    if ctx.lines.exec01020304(&mut ctx.points).is_err() {
        return Err(LINE_EXEC);
    }
    if a.debug_save_sw {
        let lr = &ctx.lines;
        let pts = &ctx.points.l;
        save_file(dir, "tmp08_jaggy_lines.txt", &lr.text_per_line("lines", "", true, |l, o| l.text_line(pts, o)))
            .map_err(|_| SAVE_LINES)?;
        save_file(
            dir,
            "tmp09_one_point.txt",
            &lr.text_per_line("line", "start point", false, |l, o| l.text_int_point(pts, L_ONE, o)),
        )
        .map_err(|_| SAVE_ONE)?;
        save_file(
            dir,
            "tmp10_another_point.txt",
            &lr.text_per_line("line", "end point", false, |l, o| l.text_int_point(pts, L_ANOTHER, o)),
        )
        .map_err(|_| SAVE_ANOTHER)?;
        let mut s = format!("# all point count {}\n", pts.count);
        let mut not_include = 0;
        let mut p = pts.first;
        while p != NIL {
            if pts.v[p].d.next_point == NIL && pts.v[p].d.prev_point == NIL {
                s.push_str(&format!("{} {}\n", pts.v[p].d.xp, pts.v[p].d.yp));
                not_include += 1;
            }
            p = pts.v[p].next;
        }
        s.push_str(&format!("# not include is {}\n", not_include));
        save_file(dir, "tmp11_not_include.txt", &s).map_err(|_| SAVE_NOT_INCLUDE)?;
    }

    ctx.lines.exec05_set_middle(&ctx.points.l);
    if a.debug_save_sw {
        let pts = &ctx.points.l;
        save_file(
            dir,
            "tmp12_middle_point.txt",
            &ctx.lines.text_per_line("line", "start point", false, |l, o| l.text_int_point(pts, L_MIDDLE, o)),
        )
        .map_err(|_| SAVE_MIDDLE)?;
    }

    ctx.lines.exec06_int2double_body(&mut ctx.points.l);
    ctx.lines.exec07_smooth_body(&mut ctx.points.l);
    if ctx.lines.exec08_expand_lines(&mut ctx.points.l).is_err() {
        return Err(EXPAND_LINES);
    }

    let vector_text = |lr: &LineRoot, pts: &Points| lr.text_per_line("lines", "", false, |l, o| l.text_expand_vector(pts, o));
    if a.debug_save_sw {
        save_file(dir, "tmp13_expand_vector.txt", &vector_text(&ctx.lines, &ctx.points.l)).map_err(|_| SAVE_EXPAND_VECTOR)?;
    }
    ctx.lines.exec09_same_way_expand(&mut ctx.points.l, &mut ctx.same_way);
    if a.debug_save_sw {
        save_file(dir, "tmp14_same_way_vector.txt", &vector_text(&ctx.lines, &ctx.points.l)).map_err(|_| SAVE_EXPAND_VECTOR)?;
    }

    ctx.lines.exec10_smooth_expand(&mut ctx.points.l);
    if a.debug_save_sw {
        let lr = &ctx.lines;
        let pts = &ctx.points.l;
        save_file(dir, "tmp15_expand_lines.txt", &lr.text_per_line("lines", "", true, |l, o| l.text_expand_line(pts, o)))
            .map_err(|_| SAVE_EXPAND_LINES)?;
        save_file(
            dir,
            "tmp16_one_expand_point.txt",
            &lr.text_per_line("line", "one expand point", false, |l, o| l.text_expand_point(pts, L_ONE_EXPAND, o)),
        )
        .map_err(|_| SAVE_ONE_EXPAND)?;
        save_file(
            dir,
            "tmp17_another_expand_point.txt",
            &lr.text_per_line("line", "another expand point", false, |l, o| l.text_expand_point(pts, L_ANOTHER_EXPAND, o)),
        )
        .map_err(|_| SAVE_ANOTHER_EXPAND)?;
    }

    ctx.lines.exec11_set_bbox(&ctx.points.l);

    // brushes
    macro_rules! brush {
        ($t:ty) => {{
            let inp = Src { p: a.inp as *const $t };
            let out = Dst { p: a.out as *mut $t };
            if 0 == a.brush_action {
                ctx.curve_blur_all(sw, &geo, inp, out, a.inp as *const u8, a.out as *mut u8, <$t as Sample>::BYTES)?;
            } else if 1 == a.brush_action {
                ctx.smudge_all(sw, &geo, inp, out, a.inp as *const u8, a.out as *mut u8, <$t as Sample>::BYTES)?;
            }
        }};
    }
    if 16 == a.bits {
        brush!(u16);
    } else {
        brush!(u8);
    }

    ctx.smudge.mem_free();
    ctx.curve.mem_free();
    ctx.curve_sel.mem_free();
    ctx.same_way.mem_free();
    ctx.lines.mem_free();
    ctx.points.mem_free();
    ctx.thin.mem_free();
    Ok(())
}

/// `igs::line_blur::convert` with a directory for the debug files (`None` = cwd).
///
/// # Safety
/// `a.inp`/`a.out` must point to `height` rows of `wrap * channels` samples of `bits`
/// bits each (the usual raster contract); the two images must not partially overlap.
pub unsafe fn convert_in_dir(a: &Args, dir: Option<&Path>) -> i32 {
    if a.inp.is_null()
        || a.out.is_null()
        || a.height < 0
        || a.width < 0
        || a.in_wrap < a.width
        || a.out_wrap < a.width
        || !(1..=CHANNEL_COUNT as i32).contains(&a.channels)
        || a.reference_channel < 0
        || a.reference_channel >= i64::from(a.channels)
    {
        return BAD_ARG;
    }
    if a.bits != 8 && a.bits != 16 {
        return BAD_BITS;
    }
    let dir = dir.map(|d| d.to_path_buf());
    let mut ctx = Ctx {
        thin: Thin::new(),
        points: PointRoot::new(),
        lines: LineRoot::new(),
        same_way: SameWayRoot::new(),
        curve_sel: CurveSelRoot::new(),
        smudge: Smudge::new(),
        curve: CurveBlur::new(),
    };
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&mut ctx, a, &dir)));
    let status = match r {
        Ok(Ok(())) => OK,
        Ok(Err(code)) => code,
        Err(_) => INTERNAL,
    };
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ctx.teardown()));
    status
}

#[no_mangle]
pub unsafe extern "C" fn flare_igs_line_blur_message(status: c_int) -> *const c_char {
    let i = if (0..MESSAGES.len() as c_int).contains(&status) { status as usize } else { INTERNAL as usize };
    MESSAGES[i].as_ptr() as *const c_char
}

#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_igs_line_blur_convert(
    inp: *const c_void,
    out: *mut c_void,
    height: c_int,
    width: c_int,
    in_wrap: c_int,
    out_wrap: c_int,
    channels: c_int,
    bits: c_int,
    b_blur_count: c_int,
    b_blur_power: f64,
    b_subpixel: c_int,
    b_blur_near_ref: c_int,
    b_blur_near_len: c_int,
    b_smudge_thick: c_int,
    b_smudge_remain: f64,
    v_smooth_retry: c_int,
    v_near_ref: c_int,
    v_near_len: c_int,
    mv_sw: bool,
    pv_sw: bool,
    cv_sw: bool,
    reference_channel: i64,
    debug_save_sw: bool,
    brush_action: c_int,
) -> c_int {
    let a = Args {
        inp,
        out,
        height,
        width,
        in_wrap,
        out_wrap,
        channels,
        bits,
        b_blur_count,
        b_blur_power,
        b_subpixel,
        b_blur_near_ref,
        b_blur_near_len,
        b_smudge_thick,
        b_smudge_remain,
        v_smooth_retry,
        v_near_ref,
        v_near_len,
        mv_sw,
        pv_sw,
        cv_sw,
        reference_channel,
        debug_save_sw,
        brush_action,
    };
    convert_in_dir(&a, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- unit tests ------------------------------------------------------------------

    #[test]
    fn g_matches_printf() {
        assert_eq!(g(0.0), "0");
        assert_eq!(g(-0.0), "-0");
        assert_eq!(g(0.1), "0.1");
        assert_eq!(g(160.0), "160");
        assert_eq!(g(23.38891234), "23.3889");
        assert_eq!(g(-13.88224), "-13.8822");
        assert_eq!(g(1e-5), "1e-05");
        assert_eq!(g(0.0001), "0.0001");
        assert_eq!(g(123456789.0), "1.23457e+08");
        assert_eq!(g(999999.5), "1e+06");
        assert_eq!(g(100000.0), "100000");
        assert_eq!(g(1.5e300), "1.5e+300");
        assert_eq!(g(f64::INFINITY), "inf");
        assert_eq!(g(f64::NEG_INFINITY), "-inf");
    }

    #[test]
    fn list_matches_list_root() {
        let mut l: List<i32> = List::new();
        let a = l.push(NIL, 1);
        let b = l.push(a, 2);
        let c = l.push(b, 3);
        let d = l.push(a, 4); // between a and b
        assert_eq!((l.first, l.last, l.count), (a, c, 4));
        let order = |l: &List<i32>| {
            let mut v = vec![];
            let mut i = l.first;
            while i != NIL {
                v.push(l.v[i].d);
                i = l.v[i].next;
            }
            v
        };
        assert_eq!(order(&l), vec![1, 4, 2, 3]);
        let e = l.push(NIL, 5); // new head
        assert_eq!(order(&l), vec![5, 1, 4, 2, 3]);
        assert_eq!(l.v[a].prev, e);
        l.pop(d);
        assert_eq!(order(&l), vec![5, 1, 2, 3]);
        assert_eq!(l.v[b].prev, a);
        l.pop(c);
        assert_eq!(l.last, b);
        assert_eq!(l.v[b].next, NIL);
        assert_eq!(l.free_all(), 3);
        assert_eq!((l.first, l.last, l.count), (NIL, NIL, 0));
    }

    #[test]
    fn messages_cover_every_status() {
        assert_eq!(message(OK), "");
        assert_eq!(message(THIN_EXEC), "Error : cl_thinnest_ui16_image.exec05_thin() returns NG");
        assert_eq!(message(OVER_LINES), "Error : over cl_pixel_line_root.get_i32_count()");
        assert_eq!(message(999), message(INTERNAL));
        for s in 1..=OVER_LINES {
            assert!(message(s).len() > 8, "status {s}");
        }
        let p = unsafe { flare_igs_line_blur_message(BAD_BITS) };
        let c = unsafe { std::ffi::CStr::from_ptr(p) };
        assert_eq!(c.to_str().unwrap(), "Error : bits is not 8 or 16");
    }

    fn args(inp: &[u8], out: &mut [u8], w: i32, h: i32, cc: i32, bits: i32) -> Args {
        Args {
            inp: inp.as_ptr() as *const c_void,
            out: out.as_mut_ptr() as *mut c_void,
            height: h,
            width: w,
            in_wrap: w,
            out_wrap: w,
            channels: cc,
            bits,
            b_blur_count: 9,
            b_blur_power: 1.0,
            b_subpixel: 1,
            b_blur_near_ref: 4,
            b_blur_near_len: 160,
            b_smudge_thick: 7,
            b_smudge_remain: 0.85,
            v_smooth_retry: 20,
            v_near_ref: 4,
            v_near_len: 160,
            mv_sw: false,
            pv_sw: false,
            cv_sw: false,
            reference_channel: 3,
            debug_save_sw: false,
            brush_action: 0,
        }
    }

    #[test]
    fn argument_checks() {
        let inp = vec![0u8; 8 * 8 * 4];
        let mut out = vec![0u8; 8 * 8 * 4];
        let ok = |a: Args| unsafe { convert_in_dir(&a, None) };
        assert_eq!(ok(args(&inp, &mut out, 8, 8, 4, 8)), OK);
        assert_eq!(ok(args(&inp, &mut out, 8, 8, 4, 12)), BAD_BITS);
        assert_eq!(message(BAD_BITS), "Error : bits is not 8 or 16");
        assert_eq!(ok(args(&inp, &mut out, 8, -1, 4, 8)), BAD_ARG);
        assert_eq!(ok(args(&inp, &mut out, 8, 8, 5, 8)), BAD_ARG);
        assert_eq!(ok(args(&inp, &mut out, 8, 8, 0, 8)), BAD_ARG);
        let mut a = args(&inp, &mut out, 8, 8, 4, 8);
        a.reference_channel = 4;
        assert_eq!(ok(a), BAD_ARG);
        a.reference_channel = -1;
        assert_eq!(ok(a), BAD_ARG);
        let mut a = args(&inp, &mut out, 8, 8, 4, 8);
        a.in_wrap = 7;
        assert_eq!(ok(a), BAD_ARG);
        let mut a = args(&inp, &mut out, 8, 8, 4, 8);
        a.inp = std::ptr::null();
        assert_eq!(ok(a), BAD_ARG);
        // empty images are a no-op
        assert_eq!(ok(args(&inp, &mut out, 0, 8, 4, 8)), OK);
        assert_eq!(ok(args(&inp, &mut out, 8, 0, 4, 8)), OK);
    }

    #[test]
    fn nothing_to_blur_copies_the_image() {
        // no line at all: every pixel selects nothing and keeps the copy of the input
        let inp: Vec<u8> = (0..8 * 6 * 4).map(|i| (i * 7 % 251) as u8).collect();
        let mut inp = inp;
        for p in 0..8 * 6 {
            inp[p * 4 + 3] = 0;
        }
        for action in [0, 1, 2] {
            let mut out = vec![0xabu8; inp.len()];
            let mut a = args(&inp, &mut out, 8, 6, 4, 8);
            a.brush_action = action;
            let st = unsafe { convert_in_dir(&a, None) };
            assert_eq!(st, OK);
            if action == 2 {
                assert!(out.iter().all(|&b| b == 0xab), "action 2 leaves the output untouched");
            } else {
                assert_eq!(out, inp, "action {action}");
            }
        }
    }

    #[test]
    fn a_horizontal_line_blurs_along_itself() {
        // a 1-px line in alpha, a bright dot of colour on it: blurred along the line only
        let (w, h) = (40usize, 12usize);
        let mut inp = vec![0u8; w * h * 4];
        for x in 3..w - 3 {
            inp[(6 * w + x) * 4 + 3] = 255;
        }
        inp[(6 * w + 20) * 4] = 255; // R impulse on the line
        inp[(2 * w + 20) * 4] = 200; // R impulse away from the line
        let mut out = vec![0u8; inp.len()];
        let a = args(&inp, &mut out, w as i32, h as i32, 4, 8);
        assert_eq!(unsafe { convert_in_dir(&a, None) }, OK);
        // the impulse spreads sideways along row 6 (9 taps, triangular weights)
        let r = |x: usize, y: usize| out[(y * w + x) * 4];
        assert!(r(20, 6) < 255 && r(18, 6) > 0 && r(22, 6) > 0, "{} {} {}", r(18, 6), r(20, 6), r(22, 6));
        assert_eq!(r(30, 6), 0);
        // ... and not across it
        assert_eq!(r(20, 5), 0);
    }

    // --- golden replay ---------------------------------------------------------------

    fn art_level(art: i32, x: i32, y: i32, w: i32, h: i32) -> i32 {
        let (cx, cy) = (w / 2, h / 2);
        let on = match art {
            0 => {
                (y >= 3 && y <= h - 4 && (x == cx - 1 || x == cx))
                    || (x >= 3 && x <= w - 4 && (y == cy - 1 || y == cy))
            }
            1 => {
                let t = y * (w - 1) / (h - 1);
                y >= 2 && y <= h - 3 && (x == t || x == t + 1)
            }
            2 => {
                let r = w.min(h) / 3;
                let d2 = (x - cx) * (x - cx) + (y - cy) * (y - cy);
                (d2 - r * r).abs() <= r && !(x > cx && (y - cy).abs() < 2)
            }
            3 => {
                let tri = ((x % 16) - 8).abs() - 4;
                let yy = cy + tri;
                x >= 2 && x <= w - 3 && (y == yy || y == yy + 1)
            }
            4 => {
                (x == cx && y >= cy && y <= h - 3)
                    || (y < cy && y >= 3 && (x == cx - (cy - y) || x == cx + (cy - y)))
            }
            5 => {
                (y == h / 3 && x >= 2 && x <= w / 2 - 4)
                    || (y == h / 3 + 1 && x >= w / 2 + 2 && x <= w - 3)
            }
            6 => {
                (y == h / 3 || y == h / 3 + 1 || y == 2 * h / 3 || y == 2 * h / 3 + 1) && x >= 2 && x <= w - 3
            }
            7 => x >= 4 && x <= w - 5 && y >= cy - 2 && y <= cy + 2,
            8 => x >= 2 && x <= w - 3 && y >= 2 && y <= h - 3,
            _ => false,
        };
        if on {
            255
        } else if (x * 7 + y * 13) % 97 == 0 {
            40
        } else {
            0
        }
    }

    fn other_val(bits: i32, x: i32, y: i32, z: i32) -> u32 {
        if bits == 8 {
            ((x * 37 + y * 91 + z * 53 + (x / 3) * 11) % 256) as u32
        } else {
            ((x * 2579 + y * 97 + z * 7919 + 13) % 65536) as u32
        }
    }

    #[derive(Clone, Debug)]
    struct Cfg {
        w: i32,
        h: i32,
        art: i32,
        cc: i32,
        bits: i32,
        reference: i32,
        padin: i32,
        padout: i32,
        blur_count: i32,
        power: f64,
        subpixel: i32,
        near_ref: i32,
        near_len: i32,
        thick: i32,
        remain: f64,
        retry: i32,
        v_ref: i32,
        v_len: i32,
        action: i32,
    }

    fn hex_f64(t: &str) -> f64 {
        f64::from_bits(u64::from_str_radix(t, 16).unwrap())
    }

    fn parse_cfg(line: &str) -> Cfg {
        let t: Vec<&str> = line.split_whitespace().skip(1).collect();
        let i = |k: usize| t[k].parse::<i32>().unwrap();
        Cfg {
            w: i(0),
            h: i(1),
            art: i(2),
            cc: i(3),
            bits: i(4),
            reference: i(5),
            padin: i(6),
            padout: i(7),
            blur_count: i(8),
            power: hex_f64(t[9]),
            subpixel: i(10),
            near_ref: i(11),
            near_len: i(12),
            thick: i(13),
            remain: hex_f64(t[14]),
            retry: i(15),
            v_ref: i(16),
            v_len: i(17),
            action: i(18),
        }
    }

    fn make_in(c: &Cfg) -> Vec<u8> {
        let bytes = (c.bits / 8) as usize;
        let mut buf = vec![0x5cu8; (c.h * (c.w + c.padin) * c.cc) as usize * bytes];
        for y in 0..c.h {
            for x in 0..c.w {
                for z in 0..c.cc {
                    let mut v = if z == c.reference {
                        let a = art_level(c.art, x, y, c.w, c.h) as u32;
                        if c.bits == 16 {
                            a * 257
                        } else {
                            a
                        }
                    } else {
                        other_val(c.bits, x, y, z)
                    };
                    v &= 0xffff;
                    let i = ((y * (c.w + c.padin) * c.cc + x * c.cc + z) as usize) * bytes;
                    buf[i] = (v & 0xff) as u8;
                    if bytes == 2 {
                        buf[i + 1] = (v >> 8) as u8;
                    }
                }
            }
        }
        buf
    }

    fn args_of(c: &Cfg, inp: &[u8], out: &mut [u8]) -> Args {
        Args {
            inp: inp.as_ptr() as *const c_void,
            out: out.as_mut_ptr() as *mut c_void,
            height: c.h,
            width: c.w,
            in_wrap: c.w + c.padin,
            out_wrap: c.w + c.padout,
            channels: c.cc,
            bits: c.bits,
            b_blur_count: c.blur_count,
            b_blur_power: c.power,
            b_subpixel: c.subpixel,
            b_blur_near_ref: c.near_ref,
            b_blur_near_len: c.near_len,
            b_smudge_thick: c.thick,
            b_smudge_remain: c.remain,
            v_smooth_retry: c.retry,
            v_near_ref: c.v_ref,
            v_near_len: c.v_len,
            mv_sw: false,
            pv_sw: false,
            cv_sw: false,
            reference_channel: i64::from(c.reference),
            debug_save_sw: false,
            brush_action: c.action,
        }
    }

    fn fnv(bytes: &[u8], mut h: u64) -> u64 {
        for &b in bytes {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    }
    const FNV0: u64 = 0xcbf29ce484222325;

    /// The vector stage of `run`, as the harness's `pipe_summary` prints it.
    fn pipe_summary(a: &Args) -> Option<String> {
        let mut ctx = Ctx {
            thin: Thin::new(),
            points: PointRoot::new(),
            lines: LineRoot::new(),
            same_way: SameWayRoot::new(),
            curve_sel: CurveSelRoot::new(),
            smudge: Smudge::new(),
            curve: CurveBlur::new(),
        };
        ctx.thin.xs = a.width;
        ctx.thin.ys = a.height;
        ctx.lines.smooth_retry = a.v_smooth_retry;
        ctx.same_way.count_max = a.v_near_ref;
        ctx.same_way.length_max = f64::from(a.v_near_len);
        ctx.thin.mem_alloc().ok()?;
        match a.bits {
            8 => image_get(Sw::default(), a.reference_channel, &mut ctx.thin, Src { p: a.inp as *const u8 }, a.in_wrap, a.channels, |v: u8| {
                (u16::from(v) << 8) + u16::from(v)
            }),
            _ => image_get(Sw::default(), a.reference_channel, &mut ctx.thin, Src { p: a.inp as *const u16 }, a.in_wrap, a.channels, |v: u16| v),
        }
        ctx.thin.exec01_fill_noise_pixel();
        ctx.thin.exec02_scale_add_edge_pixel();
        ctx.thin.exec03_scale_liner();
        ctx.thin.exec04_bw();
        ctx.thin.exec05_thin().ok()?;
        let (xs, ys, s0) = (ctx.thin.xs, ctx.thin.ys, ctx.thin.src());
        let n = (xs as usize) * (ys as usize);
        ctx.points.alloc_mem_and_list_node(xs, ys, &ctx.thin.mem[s0..s0 + n]).ok()?;
        ctx.lines.exec01020304(&mut ctx.points).ok()?;
        ctx.lines.exec05_set_middle(&ctx.points.l);
        ctx.lines.exec06_int2double_body(&mut ctx.points.l);
        ctx.lines.exec07_smooth_body(&mut ctx.points.l);
        ctx.lines.exec08_expand_lines(&mut ctx.points.l).ok()?;
        ctx.lines.exec09_same_way_expand(&mut ctx.points.l, &mut ctx.same_way);
        ctx.lines.exec10_smooth_expand(&mut ctx.points.l);
        ctx.lines.exec11_set_bbox(&ctx.points.l);
        let mut h = FNV0;
        let mut n = 0;
        let mut p = ctx.points.l.first;
        while p != NIL {
            let d = &ctx.points.l.v[p].d;
            h = fnv(&d.xp.to_le_bytes(), h);
            h = fnv(&d.yp.to_le_bytes(), h);
            h = fnv(&d.xt.to_bits().to_le_bytes(), h);
            h = fnv(&d.yt.to_bits().to_le_bytes(), h);
            n += 1;
            p = ctx.points.l.v[p].next;
        }
        let l = &ctx.lines;
        Some(format!(
            "pipe {} {} {:016x} {:016x} {:016x} {:016x} {:016x}",
            l.l.count,
            n,
            l.bbox_x_min.to_bits(),
            l.bbox_x_max.to_bits(),
            l.bbox_y_min.to_bits(),
            l.bbox_y_max.to_bits(),
            h
        ))
    }

    fn ulp_dist(a: f64, b: f64) -> u64 {
        let ord = |x: f64| {
            let u = x.to_bits() as i64;
            if u < 0 {
                i64::MIN - u
            } else {
                u
            }
        };
        ord(a).abs_diff(ord(b))
    }

    const STRICT: bool = cfg!(all(target_os = "linux", target_env = "gnu"));

    fn golden() -> &'static str {
        include_str!("igs_line_blur_cases.txt")
    }

    /// The libm-dependent helpers (`atan`, `sin`, `cos`, `pow`) within 4 ULP of the
    /// recorded values; returns whether every one was bit-identical.
    #[test]
    fn libm_helpers_match_the_recording() {
        let mut exact = true;
        let mut n = 0;
        for l in golden().lines() {
            let t: Vec<&str> = l.split_whitespace().collect();
            match t.first().copied() {
                Some("rad") => {
                    let (x, y, want) = (hex_f64(t[1]), hex_f64(t[2]), hex_f64(t[3]));
                    let got = CAPTURE_SILENT(|| get_d_radian(x, y));
                    assert!(ulp_dist(got, want) <= 4, "{l}: {got:e}");
                    exact &= got.to_bits() == want.to_bits();
                    n += 1;
                }
                Some("ratio") => {
                    let count: i32 = t[1].parse().unwrap();
                    let power = hex_f64(t[2]);
                    let mut cb = CurveBlur::new();
                    cb.count = count;
                    cb.power = power;
                    cb.mem_alloc().unwrap();
                    cb.init_ratio_array();
                    assert_eq!(cb.ratio.len(), count as usize);
                    for (i, w) in t[3..].iter().enumerate() {
                        let want = hex_f64(w);
                        assert!(ulp_dist(cb.ratio[i], want) <= 4, "{l}: [{i}] {:e}", cb.ratio[i]);
                        exact &= cb.ratio[i].to_bits() == want.to_bits();
                    }
                    n += 1;
                }
                Some("acc") => {
                    let (bc, cnt, pos): (i32, i32, i32) = (t[1].parse().unwrap(), t[2].parse().unwrap(), t[3].parse().unwrap());
                    let want = hex_f64(t[4]);
                    let mut lines: List<LineData> = List::new();
                    let mut ld = LineData::new();
                    ld.point_count = cnt;
                    lines.push(NIL, ld);
                    let sel = CurveSelNode {
                        line: 0,
                        start_point: NIL,
                        near_point: NIL,
                        near_point_pos: pos,
                        length: 1.0,
                        reverse: false,
                    };
                    let got = CurveSelRoot::line_accum_count(&sel, &lines, bc);
                    assert!(ulp_dist(got, want) <= 4, "{l}: {got:e}");
                    exact &= got.to_bits() == want.to_bits();
                    n += 1;
                }
                _ => {}
            }
        }
        assert!(n > 400, "golden has {n} libm records");
        if STRICT {
            assert!(exact, "glibc libm must reproduce the recording bit for bit");
        }
    }

    #[allow(non_snake_case)]
    fn CAPTURE_SILENT<R>(f: impl FnOnce() -> R) -> R {
        CAPTURE.with(|c| *c.borrow_mut() = Some(Vec::new()));
        let r = f();
        CAPTURE.with(|c| *c.borrow_mut() = None);
        r
    }

    #[test]
    fn rotation_matches_the_recording() {
        let mut lines = golden().lines().peekable();
        let vecs: Vec<(f64, f64)> = {
            let mut v = vec![];
            for l in golden().lines() {
                let t: Vec<&str> = l.split_whitespace().collect();
                if t.first() == Some(&"rad") {
                    v.push((hex_f64(t[1]), hex_f64(t[2])));
                }
            }
            v
        };
        let mut k = 0;
        for l in &mut lines {
            let t: Vec<&str> = l.split_whitespace().collect();
            if t.first() == Some(&"rot") {
                let (vx, vy) = vecs[k];
                k += 1;
                assert_eq!(hex_f64(t[1]).to_bits(), (vx * 0.37).to_bits());
                let (rx, ry) = get_dd_rotate(vx + 2.5, vy - 1.5, vx * 0.37);
                assert!(ulp_dist(rx, hex_f64(t[2])) <= 4, "{l}: {rx:e}");
                assert!(ulp_dist(ry, hex_f64(t[3])) <= 4, "{l}: {ry:e}");
            }
        }
        assert_eq!(k, vecs.len());
    }

    #[test]
    fn thinning_matches_the_cpp_reference() {
        let mut lines = golden().lines();
        let mut n = 0;
        while let Some(l) = lines.next() {
            if !l.starts_with("tcase ") {
                continue;
            }
            let t: Vec<i32> = l.split_whitespace().skip(1).map(|v| v.parse().unwrap()).collect();
            let want = lines.next().unwrap();
            let mut th = Thin::new();
            th.xs = t[0];
            th.ys = t[1];
            th.xd = t[3];
            th.yd = t[4];
            th.mem_alloc().unwrap();
            let s = th.src();
            for y in 0..t[1] {
                for x in 0..t[0] {
                    th.mem[s + (y * t[0] + x) as usize] = (art_level(t[2], x, y, t[0], t[1]) * 257) as u16;
                }
            }
            if t[0] > 6 && t[1] > 6 {
                th.mem[s + (2 * t[0] + 3) as usize] = 65535;
            }
            th.exec01_fill_noise_pixel();
            th.exec02_scale_add_edge_pixel();
            th.exec03_scale_liner();
            th.exec04_bw();
            let rc = if th.exec05_thin().is_ok() { 0 } else { -1 };
            let s = th.src();
            let cnt = (th.xs * th.ys) as usize;
            let bytes: Vec<u8> = th.mem[s..s + cnt].iter().flat_map(|v| v.to_le_bytes()).collect();
            let got = format!("thin {} {} {} {:016x}", rc, th.xs, th.ys, fnv(&bytes, FNV0));
            assert_eq!(got, want, "{l}");
            n += 1;
        }
        assert_eq!(n, 12);
    }

    /// Replays `gen_igs_line_blur_cases.py`: the original `igs_line_blur.cpp` (from git)
    /// over 43 parameter sets. The vector stage goes through the platform libm, so the
    /// structure summary and the image hash are required exactly on glibc, and elsewhere
    /// only when this libm reproduced the recorded vector structure.
    #[test]
    fn matches_the_cpp_reference() {
        let mut lines = golden().lines().peekable();
        let mut checked = 0;
        let mut compared_images = 0;
        while let Some(l) = lines.next() {
            if !l.starts_with("case ") {
                continue;
            }
            let c = parse_cfg(l);
            let inp = make_in(&c);
            let mut out = vec![0xabu8; (c.h * (c.w + c.padout) * c.cc) as usize * (c.bits / 8) as usize];
            let a = args_of(&c, &inp, &mut out);
            let st = CAPTURE_SILENT(|| unsafe { convert_in_dir(&a, None) });
            let rec = lines.next().unwrap();
            if let Some(text) = rec.strip_prefix("throw ") {
                assert_ne!(st, OK, "{l}");
                assert_eq!(message(st), text, "{l}");
                checked += 1;
                continue;
            }
            assert_eq!(st, OK, "{l}: {}", message(st));
            let pipe_rec = lines.next().unwrap();
            assert!(pipe_rec.starts_with("pipe "), "{pipe_rec}");
            let mine = CAPTURE_SILENT(|| pipe_summary(&a)).expect("vector stage");
            let same_structure = mine == pipe_rec;
            if STRICT {
                assert_eq!(mine, pipe_rec, "{l}: vector structure");
            } else if !same_structure {
                eprintln!("note: libm differs from the recording for [{l}]; image hash not compared");
            }
            if same_structure {
                let want = u64::from_str_radix(rec.strip_prefix("img ").unwrap(), 16).unwrap();
                assert_eq!(fnv(&out, FNV0), want, "{l}: image");
                compared_images += 1;
            }
            checked += 1;
        }
        assert_eq!(checked, 43);
        assert!(compared_images >= 40 || !STRICT, "only {compared_images} images compared");
    }

    fn norm_line(l: &str) -> String {
        let mut l = l.to_string();
        if let Some(rest) = l.strip_prefix('\x07') {
            l = format!("<BEL>{rest}");
        }
        let off = if l.starts_with("<BEL>") { 5 } else { 0 };
        let b = l.as_bytes();
        if l[off..].starts_with("#  ") && b.len() >= off + 29 && b[off + 16] == b':' && b[off + 27] == b' ' && b[off + 28] == b' ' {
            l.replace_range(off + 3..off + 27, "<TS>");
        }
        if l.contains("thinnest_ui16_image::mem_free() <") || l.contains("thinnest_ui16_image::mem_alloc() memory <") {
            let from = l.find("ui16_image::mem_").unwrap();
            let a = from + l[from..].find('<').unwrap();
            let b = a + l[a..].find('>').unwrap();
            l.replace_range(a..=b, "<PTR>");
        }
        l
    }

    fn log_lines(tag: &str, text: &str) -> Vec<String> {
        let mut out = vec![];
        let mut pos = 0;
        loop {
            match text[pos..].find('\n') {
                Some(i) => {
                    out.push(format!("{tag} {}", norm_line(&text[pos..pos + i])));
                    pos += i + 1;
                }
                None => {
                    if pos < text.len() {
                        out.push(format!("{tag} {}|", norm_line(&text[pos..])));
                    }
                    break;
                }
            }
        }
        out
    }

    /// `mv_sw/pv_sw/cv_sw` and `debug_save_sw` of both brushes: the printed lines (time
    /// stamps and addresses normalised) and the ten files, against the original.
    #[test]
    fn verbose_output_and_debug_files_match_the_cpp_reference() {
        let g = golden();
        for action in 0..2 {
            let start = g.lines().position(|l| l == format!("vcase {action}")).unwrap();
            let want: Vec<&str> = g.lines().skip(start + 1).take_while(|l| !l.starts_with("vcase ")).collect();
            let c = Cfg {
                w: 30,
                h: 24,
                art: 4,
                cc: 4,
                bits: 8,
                reference: 3,
                padin: 1,
                padout: 2,
                blur_count: 5,
                power: 1.0,
                subpixel: 1,
                near_ref: 3,
                near_len: 160,
                thick: 3,
                remain: 0.85,
                retry: 3,
                v_ref: 4,
                v_len: 160,
                action,
            };
            let inp = make_in(&c);
            let mut out = vec![0xabu8; (c.h * (c.w + c.padout) * c.cc) as usize];
            let mut a = args_of(&c, &inp, &mut out);
            a.mv_sw = true;
            a.pv_sw = true;
            a.cv_sw = true;
            a.debug_save_sw = true;
            let dir = std::env::temp_dir().join(format!("igslb-test-{}-{}", std::process::id(), action));
            std::fs::create_dir_all(&dir).unwrap();
            CAPTURE.with(|c| *c.borrow_mut() = Some(Vec::new()));
            let st = unsafe { convert_in_dir(&a, Some(&dir)) };
            let cap = CAPTURE.with(|c| c.borrow_mut().take()).unwrap();
            assert_eq!(st, OK);
            let so: String = cap.iter().filter(|(e, _)| !e).map(|(_, s)| s.as_str()).collect();
            let se: String = cap.iter().filter(|(e, _)| *e).map(|(_, s)| s.as_str()).collect();
            let mut got = log_lines("log", &so);
            got.extend(log_lines("err", &se));
            got.push(format!("vimg {:016x}", fnv(&out, FNV0)));
            for f in [
                "tmp08_jaggy_lines.txt",
                "tmp09_one_point.txt",
                "tmp10_another_point.txt",
                "tmp11_not_include.txt",
                "tmp12_middle_point.txt",
                "tmp13_expand_vector.txt",
                "tmp14_same_way_vector.txt",
                "tmp15_expand_lines.txt",
                "tmp16_one_expand_point.txt",
                "tmp17_another_expand_point.txt",
            ] {
                let s = std::fs::read(dir.join(f)).unwrap();
                got.push(format!("file {f} {} {:016x}", s.len(), fnv(&s, FNV0)));
            }
            let _ = std::fs::remove_dir_all(&dir);
            let want: Vec<String> = want.iter().map(|s| s.to_string()).collect();
            // coordinates of the vector stage depend on libm; text and image on glibc exactly
            if STRICT {
                for (i, (a, b)) in got.iter().zip(&want).enumerate() {
                    assert_eq!(a, b, "action {action}, line {i}");
                }
                assert_eq!(got.len(), want.len(), "action {action}");
            } else {
                let strip = |v: &Vec<String>| v.iter().filter(|l| l.starts_with("log ") || l.starts_with("err ")).cloned().collect::<Vec<_>>();
                let (a, b) = (strip(&got), strip(&want));
                let same = a.iter().zip(&b).filter(|(x, y)| x == y).count();
                assert!(same * 10 >= a.len().max(b.len()) * 9, "action {action}: log text differs");
            }
        }
    }

    #[test]
    fn debug_save_failure_is_reported() {
        let c = Cfg {
            w: 30,
            h: 24,
            art: 4,
            cc: 4,
            bits: 8,
            reference: 3,
            padin: 0,
            padout: 0,
            blur_count: 5,
            power: 1.0,
            subpixel: 1,
            near_ref: 3,
            near_len: 160,
            thick: 3,
            remain: 0.85,
            retry: 3,
            v_ref: 4,
            v_len: 160,
            action: 0,
        };
        let inp = make_in(&c);
        let mut out = vec![0u8; inp.len()];
        let mut a = args_of(&c, &inp, &mut out);
        a.debug_save_sw = true;
        let dir = std::env::temp_dir().join("igslb-no-such-dir-for-test").join("deeper");
        let st = CAPTURE_SILENT(|| unsafe { convert_in_dir(&a, Some(&dir)) });
        assert_eq!(st, SAVE_LINES);
        assert_eq!(message(st), "Error : cl_pixel_line_root.save_lines(-) returns NG");
    }

    #[test]
    fn hostile_parameters_do_not_crash() {
        // the C++ ran wild (or never finished) on these; here every one returns a status
        let c = Cfg {
            w: 24,
            h: 20,
            art: 4,
            cc: 4,
            bits: 8,
            reference: 3,
            padin: 0,
            padout: 0,
            blur_count: 5,
            power: 1.0,
            subpixel: 1,
            near_ref: 3,
            near_len: 160,
            thick: 3,
            remain: 0.85,
            retry: 3,
            v_ref: 4,
            v_len: 160,
            action: 0,
        };
        let inp = make_in(&c);
        let tweaks: Vec<Box<dyn Fn(&mut Cfg)>> = vec![
            Box::new(|c| c.subpixel = 0),
            Box::new(|c| c.subpixel = -2),
            Box::new(|c| c.power = 0.0),
            Box::new(|c| c.power = -1.0),
            Box::new(|c| c.power = f64::NAN),
            Box::new(|c| c.blur_count = -4),
            Box::new(|c| c.blur_count = 4000),
            Box::new(|c| c.near_ref = -1),
            Box::new(|c| c.near_len = 0),
            Box::new(|c| c.retry = -3),
            Box::new(|c| {
                c.action = 1;
                c.thick = -3;
            }),
            Box::new(|c| {
                c.action = 1;
                c.thick = 0;
            }),
            Box::new(|c| {
                c.action = 1;
                c.remain = f64::NAN;
            }),
            Box::new(|c| {
                c.action = 1;
                c.thick = 100_000;
            }),
            Box::new(|c| c.action = 7),
        ];
        for (i, t) in tweaks.iter().enumerate() {
            let mut c = c.clone();
            t(&mut c);
            let mut out = vec![0u8; inp.len()];
            let a = args_of(&c, &inp, &mut out);
            let st = CAPTURE_SILENT(|| unsafe { convert_in_dir(&a, None) });
            assert!((0..=OVER_LINES).contains(&st), "tweak {i}: status {st}");
        }
    }
}
