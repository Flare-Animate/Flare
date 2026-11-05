//! Layer 3.3: Magic Brush selection algorithms on pixel masks. All bounded by the mask size
//! (flood fills use an explicit stack, never recursion) and by an iteration cap.

use super::ViewSpaceError;
use super::features::RegionMask;
use super::keyframe::Bitmap;

/// Largest grow/shrink/smooth radius or iteration count.
pub const MAX_ITERATIONS: u32 = 256;

/// Magic wand settings. `tolerance` is the largest RGBA distance (0..=255 scale, Euclidean over
/// four channels divided by 2) from the seed colour; `edge_bias` (0..=1) makes the fill stop at
/// strong local edges (line art): a step to a pixel whose colour differs from its neighbour in the
/// fill by more than `tolerance * (1 - edge_bias)` is refused.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WandSettings {
    pub tolerance: f32,
    pub edge_bias: f32,
    pub contiguous: bool,
}

fn dist(a: &[u8], b: &[u8]) -> f32 {
    let s: f32 = a
        .iter()
        .zip(b)
        .map(|(x, y)| (f32::from(*x) - f32::from(*y)).powi(2))
        .sum();
    s.sqrt() / 2.0
}

fn px(img: &Bitmap, x: usize, y: usize) -> &[u8] {
    let i = (y * img.width as usize + x) * 4;
    img.data.get(i..i + 4).unwrap_or(&[0, 0, 0, 0])
}

/// Select by colour from a seed pixel.
pub fn wand(
    img: &Bitmap,
    seed: (u32, u32),
    s: &WandSettings,
) -> Result<RegionMask, ViewSpaceError> {
    if seed.0 >= img.width || seed.1 >= img.height {
        return Err(ViewSpaceError::Invalid("seed is outside the image".into()));
    }
    if !s.tolerance.is_finite() || !s.edge_bias.is_finite() || s.tolerance < 0.0 {
        return Err(ViewSpaceError::Invalid(
            "wand tolerance must be finite and not negative".into(),
        ));
    }
    let (w, h) = (img.width as usize, img.height as usize);
    let mut out = RegionMask::new(img.width, img.height)?;
    let seed_px = px(img, seed.0 as usize, seed.1 as usize).to_vec();
    if !s.contiguous {
        for y in 0..h {
            for x in 0..w {
                if dist(px(img, x, y), &seed_px) <= s.tolerance {
                    out.data[y * w + x] = 255;
                }
            }
        }
        return Ok(out);
    }
    let bias = s.edge_bias.clamp(0.0, 1.0);
    // No bias: only the distance from the seed limits the fill, as in a plain wand.
    let step_tol = if bias <= 0.0 {
        f32::INFINITY
    } else {
        s.tolerance * (1.0 - bias)
    };
    let mut stack = vec![(seed.0 as usize, seed.1 as usize)];
    out.data[seed.1 as usize * w + seed.0 as usize] = 255;
    while let Some((x, y)) = stack.pop() {
        let here = px(img, x, y).to_vec();
        for (dx, dy) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
            let (nx, ny) = (x as i64 + dx, y as i64 + dy);
            if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                continue;
            }
            let (nx, ny) = (nx as usize, ny as usize);
            if out.data[ny * w + nx] != 0 {
                continue;
            }
            let p = px(img, nx, ny);
            if dist(p, &seed_px) <= s.tolerance && dist(p, &here) <= step_tol {
                out.data[ny * w + nx] = 255;
                stack.push((nx, ny));
            }
        }
    }
    Ok(out)
}

fn check_iters(n: u32) -> Result<(), ViewSpaceError> {
    if n > MAX_ITERATIONS {
        return Err(ViewSpaceError::Limit {
            what: "iterations",
            max: MAX_ITERATIONS as usize,
        });
    }
    Ok(())
}

/// One 4-neighbour morphology step; `grow` dilates, otherwise erodes. Pixels off the mask count
/// as outside, so shrinking pulls the selection in from the image border too.
fn morph(m: &RegionMask, grow: bool) -> RegionMask {
    let mut out = m.clone();
    for y in 0..i64::from(m.height) {
        for x in 0..i64::from(m.width) {
            let n = [
                m.get(x - 1, y),
                m.get(x + 1, y),
                m.get(x, y - 1),
                m.get(x, y + 1),
                m.get(x, y),
            ];
            let v = if grow {
                n.into_iter().max()
            } else {
                n.into_iter().min()
            };
            out.set(x, y, v.unwrap_or(0));
        }
    }
    out
}

pub fn grow(m: &RegionMask, pixels: u32) -> Result<RegionMask, ViewSpaceError> {
    check_iters(pixels)?;
    Ok((0..pixels).fold(m.clone(), |acc, _| morph(&acc, true)))
}

pub fn shrink(m: &RegionMask, pixels: u32) -> Result<RegionMask, ViewSpaceError> {
    check_iters(pixels)?;
    Ok((0..pixels).fold(m.clone(), |acc, _| morph(&acc, false)))
}

/// The ring of `width` pixels just inside the selection edge.
pub fn border(m: &RegionMask, width: u32) -> Result<RegionMask, ViewSpaceError> {
    let inner = shrink(m, width)?;
    let mut out = m.clone();
    for (o, i) in out.data.iter_mut().zip(&inner.data) {
        *o = o.saturating_sub(*i);
    }
    Ok(out)
}

/// Smooth the edge: `passes` of a 3x3 box blur, then re-threshold at 50% so the result is hard
/// again (removes speckles and jaggies).
pub fn smooth(m: &RegionMask, passes: u32) -> Result<RegionMask, ViewSpaceError> {
    check_iters(passes)?;
    let mut cur = m.clone();
    for _ in 0..passes {
        let mut next = cur.clone();
        for y in 0..i64::from(m.height) {
            for x in 0..i64::from(m.width) {
                let mut sum = 0u32;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        sum += u32::from(cur.get(x + dx, y + dy));
                    }
                }
                next.set(x, y, if sum / 9 >= 128 { 255 } else { 0 });
            }
        }
        cur = next;
    }
    Ok(cur)
}

/// Fill a closed polygon (freehand or polygonal lasso; vertices in pixels). Even-odd rule, sampled
/// at pixel centres. Fewer than three points select nothing.
pub fn lasso(width: u32, height: u32, points: &[(f32, f32)]) -> Result<RegionMask, ViewSpaceError> {
    const MAX_POINTS: usize = 100_000;
    if points.len() > MAX_POINTS {
        return Err(ViewSpaceError::Limit {
            what: "lasso points",
            max: MAX_POINTS,
        });
    }
    if points.iter().any(|p| !p.0.is_finite() || !p.1.is_finite()) {
        return Err(ViewSpaceError::Invalid("lasso point is not finite".into()));
    }
    let mut out = RegionMask::new(width, height)?;
    if points.len() < 3 {
        return Ok(out);
    }
    let mut xs: Vec<f32> = Vec::new();
    for y in 0..height {
        let py = y as f32 + 0.5;
        xs.clear();
        for i in 0..points.len() {
            let (a, b) = (points[i], points[(i + 1) % points.len()]);
            if (a.1 <= py) != (b.1 <= py) {
                xs.push(a.0 + (py - a.1) / (b.1 - a.1) * (b.0 - a.0));
            }
        }
        xs.sort_by(f32::total_cmp);
        for pair in xs.as_chunks::<2>().0 {
            let from = (pair[0] - 0.5).ceil().max(0.0) as i64;
            let to = ((pair[1] - 0.5).ceil() as i64 - 1).min(i64::from(width) - 1);
            for x in from..=to {
                out.set(x, i64::from(y), 255);
            }
        }
    }
    Ok(out)
}

/// Brush stamp settings: `size` is the diameter in pixels, `hardness` 0..=1 the fraction of the
/// radius that is fully opaque, `pressure` 0..=1 scales size (to 25%..100%) and opacity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Brush {
    pub size: f32,
    pub hardness: f32,
    pub pressure: f32,
}

/// Paint one soft round dab into `m` (max with what is there; `erase` subtracts instead).
pub fn stamp(
    m: &mut RegionMask,
    centre: (f32, f32),
    b: &Brush,
    erase: bool,
) -> Result<(), ViewSpaceError> {
    if ![b.size, b.hardness, b.pressure, centre.0, centre.1]
        .iter()
        .all(|v| v.is_finite())
        || b.size <= 0.0
        || b.size > 4096.0
    {
        return Err(ViewSpaceError::Invalid(
            "brush values must be finite, size 0 to 4096".into(),
        ));
    }
    let p = b.pressure.clamp(0.0, 1.0);
    let radius = b.size / 2.0 * (0.25 + 0.75 * p);
    let hard = b.hardness.clamp(0.0, 1.0);
    let (x0, x1) = (
        (centre.0 - radius).floor() as i64,
        (centre.0 + radius).ceil() as i64,
    );
    let (y0, y1) = (
        (centre.1 - radius).floor() as i64,
        (centre.1 + radius).ceil() as i64,
    );
    for y in y0.max(0)..=y1.min(i64::from(m.height) - 1) {
        for x in x0.max(0)..=x1.min(i64::from(m.width) - 1) {
            let d = ((x as f32 + 0.5 - centre.0).powi(2) + (y as f32 + 0.5 - centre.1).powi(2))
                .sqrt()
                / radius.max(1e-3);
            if d >= 1.0 {
                continue;
            }
            let fall = if d <= hard {
                1.0
            } else {
                1.0 - (d - hard) / (1.0 - hard).max(1e-3)
            };
            let a = (fall * (0.25 + 0.75 * p) * 255.0).round().clamp(0.0, 255.0) as u8;
            let cur = m.get(x, y);
            m.set(
                x,
                y,
                if erase {
                    cur.saturating_sub(a)
                } else {
                    cur.max(a)
                },
            );
        }
    }
    Ok(())
}

/// Boolean combination of two masks of the same size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combine {
    Add,
    Subtract,
    Intersect,
}

pub fn combine(a: &RegionMask, b: &RegionMask, op: Combine) -> Result<RegionMask, ViewSpaceError> {
    if (a.width, a.height) != (b.width, b.height) {
        return Err(ViewSpaceError::Invalid("masks differ in size".into()));
    }
    let mut out = a.clone();
    for (o, v) in out.data.iter_mut().zip(&b.data) {
        *o = match op {
            Combine::Add => (*o).max(*v),
            Combine::Subtract => o.saturating_sub(*v),
            Combine::Intersect => (*o).min(*v),
        };
    }
    Ok(out)
}

/// Selection that follows line art: a wand that cannot cross dark strokes.
pub fn stroke_aware_select(
    _img: &Bitmap,
    _stroke: &[(f32, f32)],
) -> Result<RegionMask, ViewSpaceError> {
    // TODO(viewspace:magic-stroke-aware): 1. Compute a line-art mask: pixels with luminance below
    // an adaptive threshold or alpha > 0 on a line layer; dilate by 1 so gaps of one pixel close.
    // 2. Seed the brush path as a thin mask. 3. Flood fill from the seed pixels through pixels that
    // are NOT line art (so the fill stops at ink). 4. Snap the stroke's own end points to the nearest
    // line pixel within 6 px. Result is the filled area plus the snapped stroke.
    Err(ViewSpaceError::NotImplemented(
        "stroke-aware selection snapping to line art",
    ))
}

/// Semantic-aware grow: grow but stay inside the same material/feature once seeded.
pub fn semantic_grow(
    _m: &RegionMask,
    _materials: &super::features::MaterialMap,
    _pixels: u32,
) -> Result<RegionMask, ViewSpaceError> {
    // TODO(viewspace:magic-semantic-grow): Read the dominant material under the seed (majority of
    // the mask's pixels). Run `grow` one pixel at a time but accept a newly covered pixel only if its
    // MaterialMap value equals the seed's material (and, when other features already own it, only
    // when the feature is the same). Stop early when a step adds nothing.
    Err(ViewSpaceError::NotImplemented("semantic-aware grow"))
}

#[cfg(test)]
#[path = "magic_tests.rs"]
mod tests;
