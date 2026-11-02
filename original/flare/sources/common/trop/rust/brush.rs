//! Thick-line brush rasteriser, ported from `brush.cpp`.
//!
//! `TRop::brush` draws a thick segment (or point, horizontal/vertical line)
//! of a given radius and colour into a `TPixel32` raster. The whole algorithm
//! is a Bresenham-style midpoint rasteriser with circular caps and trapezoidal
//! joins; this port keeps the structure identical and only swaps the C++
//! helpers for their Rust equivalents.
//!
//! Channel order follows `include/tmachine.h`: the colour is written as four
//! raw bytes in platform order (BGRM on Windows/Linux/FreeBSD/Haiku, MRGB on
//! macOS), exactly as the C++ writes a `TPixel32`.

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd", target_os = "haiku"))]
const R: usize = 2;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd", target_os = "haiku"))]
const G: usize = 1;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd", target_os = "haiku"))]
const B: usize = 0;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd", target_os = "haiku"))]
const M: usize = 3;
#[cfg(target_os = "macos")]
const R: usize = 1;
#[cfg(target_os = "macos")]
const G: usize = 2;
#[cfg(target_os = "macos")]
const B: usize = 3;
#[cfg(target_os = "macos")]
const M: usize = 0;

#[inline] fn tfloor(v: f64) -> i32 { v.floor() as i32 }
#[inline] fn tceil(v: f64) -> i32 { v.ceil() as i32 }
#[inline] fn tround(v: f64) -> i32 { v.round() as i32 }
#[inline] fn iabs(v: i32) -> i32 { v.abs() }

struct HalfCord { m: Vec<i32>, radius: i32 }

impl HalfCord {
    fn new(radius: i32) -> Self {
        assert!(radius >= 0);
        let mut m = vec![0i32; (radius + 1) as usize];
        let mut d_circle: f32 = 1.25 - radius as f32;
        let mut y = radius;
        let mut x = 0;
        while y >= x {
            m[y as usize] = m[y as usize].max(x);
            m[x as usize] = y;
            if d_circle <= 0.0 {
                d_circle = d_circle + 2.0 * x as f32 + 3.0;
            } else {
                y -= 1;
                d_circle = d_circle + 2.0 * (x - y) as f32 + 5.0;
            }
            x += 1;
        }
        Self { m, radius }
    }
    fn get(&self, x: i32) -> i32 {
        assert!(0 <= x && x <= self.radius);
        self.m[x as usize]
    }
}

fn write_px(buf: &mut [u8], off: usize, col: &[u8; 4]) {
    // col is in RGBM order: [0]=red, [1]=green, [2]=blue, [3]=matte.
    buf[off + R] = col[0];
    buf[off + G] = col[1];
    buf[off + B] = col[2];
    buf[off + M] = col[3];
}

pub fn brush(
    ras: *mut u8, wrap: usize, lx: i32, ly: i32,
    ax: i32, ay: i32, bx: i32, by: i32, radius: i32, col: [u8; 4],
) {
    // Work on a mutable copy of endpoints.
    let mut a = (ax, ay);
    let mut b = (bx, by);
    if a.1 > b.1 { std::mem::swap(&mut a, &mut b); }

    let lx = lx;
    let ly = ly;

    // radius == 0
    if radius == 0 {
        let mut k = 1i32;
        let mut dy = b.1 - a.1;
        let mut dx = b.0 - a.0;
        if dx < 0 { dx = -dx; k = -1; }
        assert!(dx >= 0);
        assert!(dy >= 0);

        let m = if dx > 0 { dy as f64 / dx as f64 } else { 0.0 };
        let alpha = dy;
        let beta = -dx;
        let inc_e = alpha;
        let inc_ne = alpha + beta;
        let inc_n = beta;

        let y_min = a.1.max(0) - a.1;
        let y_max = b.1.min(ly - 1) - a.1;

        if dx > 0 && m <= 1.0 {
            let mut segm_x: i32;
            let mut segm_y = y_min;
            if dy == 0 {
                segm_x = 0;
            } else {
                segm_x = tceil((y_min as f64 - 0.5) / m);
            }
            let mut d_segm = tfloor(alpha as f64 * (segm_x as f64 + 1.0)
                + beta as f64 * (segm_y as f64 + 0.5));
            while segm_y <= y_max {
                let mut count = 0;
                while d_segm < 0 && segm_x <= dx {
                    d_segm += inc_e;
                    segm_x += 1;
                    count += 1;
                }
                let (x_min, x_max) = if k > 0 {
                    ( (a.0 + segm_x - count).max(a.0).max(0),
                      (a.0 + segm_x).min(b.0).min(lx - 1) )
                } else {
                    ( (a.0 - segm_x).max(a.0 - dx).max(0),
                      (a.0 - segm_x + count).min(a.0).min(lx - 1) )
                };
                let row_off = (segm_y + a.1) as usize * wrap;
                // write properly:
                {
                    let base = unsafe { ras.add(row_off) };
                    let mut pp = x_min as usize;
                    while pp <= x_max as usize {
                        write_px(unsafe { std::slice::from_raw_parts_mut(base, wrap) }, pp * 4, &col);
                        pp += 1;
                    }
                }
                d_segm += inc_ne;
                segm_x += 1;
                segm_y += 1;
            }
        } else {
            // m > 1 or vertical
            let mut segm_x: i32;
            let mut segm_y = y_min;
            if dx == 0 {
                segm_x = 0;
            } else {
                segm_x = tround(y_min as f64 / m);
            }
            let mut d_segm = tfloor(alpha as f64 * (segm_x as f64 + 0.5)
                + beta as f64 * (segm_y as f64 + 1.0));
            while segm_y <= y_max {
                let (x_min, x_max) = if k > 0 {
                    ( (a.0 + segm_x).max(0), (a.0 + segm_x).min(lx - 1) )
                } else {
                    ( (a.0 - segm_x).max(0), (a.0 - segm_x).min(lx - 1) )
                };
                let row_off = (segm_y + a.1) as usize * wrap;
                {
                    let base = unsafe { ras.add(row_off) };
                    let mut pp = x_min as usize;
                    while pp <= x_max as usize {
                        write_px(unsafe { std::slice::from_raw_parts_mut(base, wrap) }, pp * 4, &col);
                        pp += 1;
                    }
                }
                if d_segm <= 0 {
                    d_segm += inc_ne;
                    segm_x += 1;
                } else {
                    d_segm += inc_n;
                }
                segm_y += 1;
            }
        }
        unsafe { ras.add(0) };
        return;
    }

    // radius > 0
    let half = HalfCord::new(radius);

    // coincident points -> circle
    if a == b {
        let y_min = (a.1 - radius).max(0);
        let y_max = (a.1 + radius).min(ly - 1);
        for y in y_min..=y_max {
            let deltay = iabs(y - a.1);
            let r = half.get(deltay);
            let x_min = (a.0 - r).max(0);
            let x_max = (a.0 + r).min(lx - 1);
            let row_off = y as usize * wrap;
            let base = unsafe { ras.add(row_off) };
            let mut pp = x_min as usize;
            while pp <= x_max as usize {
                write_px(unsafe { std::slice::from_raw_parts_mut(base, wrap) }, pp * 4, &col);
                pp += 1;
            }
        }
        return;
    }

    // horizontal
    if a.1 == b.1 {
        let y_min = (a.1 - radius).max(0);
        let y_max = (a.1 + radius).min(ly - 1);
        let x_left = a.0.min(b.0);
        let x_right = a.0.max(b.0);
        for y in y_min..=y_max {
            let deltay = iabs(y - a.1);
            let r = half.get(deltay);
            let x_min = (x_left - r).max(0);
            let x_max = (x_right + r).min(lx - 1);
            let row_off = y as usize * wrap;
            let base = unsafe { ras.add(row_off) };
            let mut pp = x_min as usize;
            while pp <= x_max as usize {
                write_px(unsafe { std::slice::from_raw_parts_mut(base, wrap) }, pp * 4, &col);
                pp += 1;
            }
        }
        return;
    }

    // vertical
    if a.0 == b.0 {
        let x_min = (a.0 - radius).max(0);
        let x_max = (a.0 + radius).min(lx - 1);
        for x in x_min..=x_max {
            let deltax = iabs(x - a.0);
            let r = half.get(deltax);
            let y_min = (a.1 - r).max(0);
            let y_max = (b.1 + r).min(ly - 1);
            if y_min <= y_max {
                let col_off = x as usize * 4;
                let mut p = y_min as usize * wrap + col_off;
                let end = y_max as usize * wrap + col_off;
                while p <= end {
                    let base = unsafe { ras.add(p) };
                    let sl = unsafe { std::slice::from_raw_parts_mut(base, 4) };
                    sl[R] = col[0]; sl[G] = col[1]; sl[B] = col[2]; sl[M] = col[3];
                    p += wrap;
                }
            }
        }
        return;
    }

    // inclined
    let mut k = 1i32;
    let mut dx = b.0 - a.0;
    if dx < 0 { dx = -dx; k = -1; }
    let dy = b.1 - a.1;
    assert!(dx > 0);
    assert!(dy > 0);

    let length = ((dx * dx + dy * dy) as f64).sqrt();
    let m = dy as f64 / dx as f64;

    let up_x = -radius as f64 * dy as f64 / length;
    let up_y = radius as f64 * dx as f64 / length;

    let half_ampl_cap = tfloor(-up_x);

    let mut cut_ext: i32;
    let mut cut_in: i32;
    let mut right_up = (0.0f64, 0.0f64);
    let mut right_down = (0.0f64, 0.0f64);
    let mut left_up = (0.0f64, 0.0f64);
    let mut left_down = (0.0f64, 0.0f64);
    let mut m_parall: f64;

    if radius > 1 {
        cut_ext = radius;
        while cut_ext >= 0 && half.get(cut_ext) <= half_ampl_cap {
            cut_ext -= 1;
        }
        cut_in = cut_ext;
        right_up = (dx as f64 + half.get(cut_in) as f64, dy as f64 - cut_in as f64);
        right_down = (half.get(cut_in) as f64, -(cut_in as f64));
        left_up = (dx as f64 - half.get(cut_in) as f64, dy as f64 + cut_in as f64);
        left_down = (-(half.get(cut_in) as f64), cut_in as f64);
        m_parall = m;
    } else {
        cut_ext = radius;
        cut_in = 0;
        right_up = (dx as f64 - up_x, dy as f64 - up_y);
        right_down = (-up_x, -up_y);
        left_up = (dx as f64 + up_x, dy as f64 + up_y);
        left_down = (up_x, up_y);
        m_parall = m;
    }

    // circular caps
    // lower cap
    let y_min = (a.1 - radius).max(0);
    let y_max = (a.1 - cut_ext - 1).min(ly - 1);
    for y in y_min..=y_max {
        let r = half.get(a.1 - y);
        let x_min = (a.0 - r).max(0);
        let x_max = (a.0 + r).min(lx - 1);
        let row_off = y as usize * wrap;
        let base = unsafe { ras.add(row_off) };
        let mut pp = x_min as usize;
        while pp <= x_max as usize {
            write_px(unsafe { std::slice::from_raw_parts_mut(base, wrap) }, pp * 4, &col);
            pp += 1;
        }
    }
    // upper cap
    let y_min = (b.1 + cut_ext + 1).max(0);
    let y_max = (b.1 + radius).min(ly - 1);
    for y in y_min..=y_max {
        let r = half.get(y - b.1);
        let x_min = (b.0 - r).max(0);
        let x_max = (b.0 + r).min(lx - 1);
        let row_off = y as usize * wrap;
        let base = unsafe { ras.add(row_off) };
        let mut pp = x_min as usize;
        while pp <= x_max as usize {
            write_px(unsafe { std::slice::from_raw_parts_mut(base, wrap) }, pp * 4, &col);
            pp += 1;
        }
    }

    let x_segm_max = tround(dx as f64 - up_x);
    let x_segm_min = tround(up_x);

    let alpha_i = dy;
    let beta_i = -dx;
    let gamma_right = right_down.1 * dx as f64 - right_down.0 * dy as f64;
    let gamma_left = left_down.1 * dx as f64 - left_down.0 * dy as f64;
    let inc_e = alpha_i;
    let inc_ne = alpha_i + beta_i;
    let inc_n = beta_i;

    // lower trapezoid
    let y_min = (a.1 - cut_ext).max(0) - a.1;
    let y_max = (a.1 + cut_in).min(b.1 - cut_in - 1).min(ly - 1) - a.1;
    // Actually original: yMax = min({a.y+cutIn, b.y-cutIn-1, ly-1}) - a.y
    let y_max = (a.1 + cut_in).min(b.1 - cut_in - 1).min(ly - 1) - a.1;
    if m <= 1.0 {
        let mut segm_right_x = tceil((y_min as f64 + 0.5 - right_down.1) / m_parall + right_down.0) - 1;
        let mut segm_right_y = y_min;
        let mut d = tfloor(alpha_i as f64 * (segm_right_x as f64 + 1.0)
            + beta_i as f64 * (segm_right_y as f64 + 0.5) + gamma_right);
        while segm_right_y <= y_max {
            if d < 0 {
                d += inc_e;
                segm_right_x += 1;
            } else {
                let x_min = if k > 0 {
                    (a.0 - half.get(iabs(segm_right_y))).max(0)
                } else {
                    (a.0 - (segm_right_x.min(x_segm_max))).max(0)
                };
                let x_max = if k > 0 {
                    (a.0 + (segm_right_x.min(x_segm_max))).min(lx - 1)
                } else {
                    (a.0 + half.get(iabs(segm_right_y))).min(lx - 1)
                };
                let row_off = (segm_right_y + a.1) as usize * wrap;
                let base = unsafe { ras.add(row_off) };
                let mut pp = x_min as usize;
                while pp <= x_max as usize {
                    write_px(unsafe { std::slice::from_raw_parts_mut(base, wrap) }, pp * 4, &col);
                    pp += 1;
                }
                d += inc_ne;
                segm_right_x += 1;
                segm_right_y += 1;
            }
        }
    } else {
        let mut segm_right_x = tround((y_min as f64 - right_down.1) / m_parall + right_down.0);
        let mut segm_right_y = y_min;
        let mut d = tfloor(alpha_i as f64 * (segm_right_x as f64 + 0.5)
            + beta_i as f64 * (segm_right_y as f64 + 1.0) + gamma_right);
        while segm_right_y <= y_max {
            let x_min = if k > 0 {
                (a.0 - half.get(iabs(segm_right_y))).max(0)
            } else {
                (a.0 - segm_right_x).max(0)
            };
            let x_max = if k > 0 {
                (a.0 + segm_right_x).min(lx - 1)
            } else {
                (a.0 + half.get(iabs(segm_right_y))).min(lx - 1)
            };
            let row_off = (segm_right_y + a.1) as usize * wrap;
            let base = unsafe { ras.add(row_off) };
            let mut pp = x_min as usize;
            while pp <= x_max as usize {
                write_px(unsafe { std::slice::from_raw_parts_mut(base, wrap) }, pp * 4, &col);
                pp += 1;
            }
            if d <= 0 {
                d += inc_ne;
                segm_right_x += 1;
            } else {
                d += inc_n;
            }
            segm_right_y += 1;
        }
    }

    // upper trapezoid
    let y_min = (b.1 - cut_in).max(a.1 + cut_in + 1).max(0) - b.1;
    let y_max = (b.1 + cut_ext).min(ly - 1) - b.1;
    if m <= 1.0 {
        let mut segm_left_x = tceil((y_min as f64 - 0.5 - left_down.1) / m_parall + left_down.0);
        let mut segm_left_y = y_min;
        let mut d = tfloor(alpha_i as f64 * (segm_left_x as f64 + 1.0)
            + beta_i as f64 * (segm_left_y as f64 + 0.5) + gamma_left);
        while segm_left_y <= y_max {
            let x_min = if k > 0 {
                (b.0 + (segm_left_x.max(x_segm_min - dx))).max(0)
            } else {
                (b.0 - half.get(iabs(segm_left_y))).max(0)
            };
            let x_max = if k > 0 {
                (b.0 + half.get(iabs(segm_left_y))).min(lx - 1)
            } else {
                (b.0 - (segm_left_x.max(x_segm_min - dx))).min(lx - 1)
            };
            let row_off = (segm_left_y + b.1) as usize * wrap;
            let base = unsafe { ras.add(row_off) };
            let mut pp = x_min as usize;
            while pp <= x_max as usize {
                write_px(unsafe { std::slice::from_raw_parts_mut(base, wrap) }, pp * 4, &col);
                pp += 1;
            }
            while d < 0 {
                d += inc_e;
                segm_left_x += 1;
            }
            d += inc_ne;
            segm_left_x += 1;
            segm_left_y += 1;
        }
    } else {
        let mut segm_left_x = tround((y_min as f64 - left_down.1) / m_parall + left_down.0);
        let mut segm_left_y = y_min;
        let mut d = tfloor(alpha_i as f64 * (segm_left_x as f64 + 0.5)
            + beta_i as f64 * (segm_left_y as f64 + 1.0) + gamma_left);
        while segm_left_y <= y_max {
            let x_min = if k > 0 {
                (b.0 + segm_left_x).max(0)
            } else {
                (b.0 - half.get(iabs(segm_left_y))).max(0)
            };
            let x_max = if k > 0 {
                (b.0 + half.get(iabs(segm_left_y))).min(lx - 1)
            } else {
                (b.0 - segm_left_x).min(lx - 1)
            };
            let row_off = (segm_left_y + b.1) as usize * wrap;
            let base = unsafe { ras.add(row_off) };
            let mut pp = x_min as usize;
            while pp <= x_max as usize {
                write_px(unsafe { std::slice::from_raw_parts_mut(base, wrap) }, pp * 4, &col);
                pp += 1;
            }
            if d <= 0 {
                d += inc_ne;
                segm_left_x += 1;
            } else {
                d += inc_n;
            }
            segm_left_y += 1;
        }
    }

    // parallelogram
    let y_min = (a.1 + cut_in + 1).max(0) - a.1;
    let y_max = (b.1 - cut_in - 1).min(ly - 1) - a.1;
    if m <= 1.0 {
        let mut segm_right_x = tceil((y_min as f64 + 0.5 - right_down.1) / m_parall + right_down.0) - 1;
        let mut segm_left_x = tceil((y_min as f64 - 0.5 - left_down.1) / m_parall + left_down.0);
        let mut segm_right_y = y_min;
        let mut d_right = tfloor(alpha_i as f64 * (segm_right_x as f64 + 1.0)
            + beta_i as f64 * (segm_right_y as f64 + 0.5) + gamma_right);
        let mut d_left = tfloor(alpha_i as f64 * (segm_left_x as f64 + 1.0)
            + beta_i as f64 * (segm_right_y as f64 + 0.5) + gamma_left);
        while segm_right_y <= y_max {
            if d_right < 0 {
                d_right += inc_e;
                segm_right_x += 1;
            } else {
                let x_min = if k > 0 {
                    (a.0 + (segm_left_x.max(x_segm_min))).max(0)
                } else {
                    (a.0 - (segm_right_x.min(x_segm_max))).max(0)
                };
                let x_max = if k > 0 {
                    (a.0 + (segm_right_x.min(x_segm_max))).min(lx - 1)
                } else {
                    (a.0 - (segm_left_x.max(x_segm_min))).min(lx - 1)
                };
                let row_off = (segm_right_y + a.1) as usize * wrap;
                let base = unsafe { ras.add(row_off) };
                let mut pp = x_min as usize;
                while pp <= x_max as usize {
                    write_px(unsafe { std::slice::from_raw_parts_mut(base, wrap) }, pp * 4, &col);
                    pp += 1;
                }
                d_right += inc_ne;
                segm_right_x += 1;
                segm_right_y += 1;
                while d_left < 0 {
                    d_left += inc_e;
                    segm_left_x += 1;
                }
                d_left += inc_ne;
                segm_left_x += 1;
            }
        }
    } else {
        let mut segm_right_x = tround((y_min as f64 - right_down.1) / m_parall + right_down.0);
        let mut segm_left_x = tround((y_min as f64 - left_down.1) / m_parall + left_down.0);
        let mut segm_right_y = y_min;
        let mut d_right = tfloor(alpha_i as f64 * (segm_right_x as f64 + 0.5)
            + beta_i as f64 * (segm_right_y as f64 + 1.0) + gamma_right);
        let mut d_left = tfloor(alpha_i as f64 * (segm_left_x as f64 + 0.5)
            + beta_i as f64 * (segm_right_y as f64 + 1.0) + gamma_left);
        while segm_right_y <= y_max {
            let x_min = if k > 0 {
                (a.0 + segm_left_x).max(0)
            } else {
                (a.0 - segm_right_x).max(0)
            };
            let x_max = if k > 0 {
                (a.0 + segm_right_x).min(lx - 1)
            } else {
                (a.0 - segm_left_x).min(lx - 1)
            };
            let row_off = (segm_right_y + a.1) as usize * wrap;
            let base = unsafe { ras.add(row_off) };
            let mut pp = x_min as usize;
            while pp <= x_max as usize {
                write_px(unsafe { std::slice::from_raw_parts_mut(base, wrap) }, pp * 4, &col);
                pp += 1;
            }
            if d_right <= 0 {
                d_right += inc_ne;
                segm_right_x += 1;
            } else {
                d_right += inc_n;
            }
            segm_right_y += 1;
            if d_left <= 0 {
                d_left += inc_ne;
                segm_left_x += 1;
            } else {
                d_left += inc_n;
            }
        }
    }

    // circular parallelogramoid
    let y_min = (b.1 - cut_in).max(0);
    let y_max = (a.1 + cut_in).min(ly - 1);
    for y in y_min..=y_max {
        let x_min = if k > 0 {
            (a.0 - half.get(iabs(y - a.1))).max(0)
        } else {
            (b.0 - half.get(iabs(b.1 - y))).max(0)
        };
        let x_max = if k > 0 {
            (b.0 + half.get(iabs(b.1 - y))).min(lx - 1)
        } else {
            (a.0 + half.get(iabs(y - a.1))).min(lx - 1)
        };
        let row_off = y as usize * wrap;
        let base = unsafe { ras.add(row_off) };
        let mut pp = x_min as usize;
        while pp <= x_max as usize {
            write_px(unsafe { std::slice::from_raw_parts_mut(base, wrap) }, pp * 4, &col);
            pp += 1;
        }
    }
}

#[no_mangle]
pub extern "C" fn flare_brush(
    ras: *mut u8, wrap: i32, lx: i32, ly: i32,
    ax: i32, ay: i32, bx: i32, by: i32, radius: i32,
    cr: u8, cg: u8, cb: u8, cm: u8,
) {
    brush(ras, wrap.max(0) as usize, lx, ly, ax, ay, bx, by, radius,
          [cr, cg, cb, cm]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_draws_circle() {
        let mut buf = vec![0u8; 16 * 16 * 4];
        brush(buf.as_mut_ptr(), 16 * 4, 16, 16, 8, 8, 8, 8, 2,
              [255, 0, 0, 255]);
        // center pixel should be coloured
        let off = (8 * 16 + 8) * 4;
        assert_eq!(buf[off + R], 255);
    }

    #[test]
    fn horizontal_line() {
        let mut buf = vec![0u8; 8 * 8 * 4];
        brush(buf.as_mut_ptr(), 8 * 4, 8, 8, 1, 4, 6, 4, 0,
              [0, 255, 0, 255]);
        // pixels along y=4, x=1..6 should be green
        for x in 1..=6 {
            let off = (4 * 8 + x) * 4;
            assert_eq!(buf[off + G], 255, "x={}", x);
        }
    }
}