//! Which pixels of a bitmap the wand has picked, and how they are picked: by how close each
//! pixel's color is to the one clicked, through its neighbours or across the whole picture.

use flare_document::Bitmap;

/// A set of pixels of one bitmap, the size of that bitmap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mask {
    width: u32,
    height: u32,
    picked: Vec<bool>,
}

impl Mask {
    /// No pixel of a `width` by `height` bitmap.
    pub fn empty(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            picked: vec![false; width as usize * height as usize],
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// One flag per pixel, rows top to bottom.
    pub fn pixels(&self) -> &[bool] {
        &self.picked
    }

    pub fn contains(&self, x: u32, y: u32) -> bool {
        x < self.width && y < self.height && self.picked[self.index(x, y)]
    }

    pub fn count(&self) -> usize {
        self.picked.iter().filter(|p| **p).count()
    }

    pub fn is_empty(&self) -> bool {
        !self.picked.contains(&true)
    }

    /// Adds the pixels of `other`. A mask of another size changes nothing.
    pub fn add(&mut self, other: &Mask) {
        if self.same_size(other) {
            for (mine, theirs) in self.picked.iter_mut().zip(&other.picked) {
                *mine |= *theirs;
            }
        }
    }

    /// Takes away the pixels of `other`. A mask of another size changes nothing.
    pub fn subtract(&mut self, other: &Mask) {
        if self.same_size(other) {
            for (mine, theirs) in self.picked.iter_mut().zip(&other.picked) {
                *mine &= !*theirs;
            }
        }
    }

    /// Every pixel that was not picked, and none that was.
    pub fn invert(&mut self) {
        for pixel in &mut self.picked {
            *pixel = !*pixel;
        }
    }

    /// The same pixels at most `max_side` to a side, each kept pixel standing for the block it
    /// covers (picked when the pixel at its middle is), for drawing over the stage: a picture's
    /// worth of flags, not one per pixel of a large bitmap.
    pub fn preview(&self, max_side: u32) -> Mask {
        let longest = self.width.max(self.height);
        if longest <= max_side.max(1) {
            return self.clone();
        }
        let scale = f64::from(max_side.max(1)) / f64::from(longest);
        let side = |n: u32| ((f64::from(n) * scale).round() as u32).max(1);
        let (w, h) = (side(self.width), side(self.height));
        let mut out = Mask::empty(w, h);
        for y in 0..h {
            let sy = ((f64::from(y) + 0.5) * f64::from(self.height) / f64::from(h)) as u32;
            for x in 0..w {
                let sx = ((f64::from(x) + 0.5) * f64::from(self.width) / f64::from(w)) as u32;
                let i = out.index(x, y);
                out.picked[i] = self.contains(sx, sy);
            }
        }
        out
    }

    fn same_size(&self, other: &Mask) -> bool {
        self.width == other.width && self.height == other.height
    }

    fn index(&self, x: u32, y: u32) -> usize {
        y as usize * self.width as usize + x as usize
    }
}

/// The pixels whose color is within `threshold` of the pixel at `(x, y)`: each of red, green,
/// blue and alpha, premultiplied so every clear pixel is the same color, differs by at most
/// `threshold`. `contiguous` keeps only those reached from the clicked one through picked
/// neighbours (left, right, up and down), as Animate's wand does; otherwise every such pixel in
/// the picture is picked. `None` when `(x, y)` is outside the bitmap.
pub fn pick(bitmap: &Bitmap, x: u32, y: u32, threshold: u8, contiguous: bool) -> Option<Mask> {
    let (w, h) = (bitmap.width(), bitmap.height());
    if x >= w || y >= h {
        return None;
    }
    let pixels = bitmap.pixels();
    let seed = y as usize * w as usize + x as usize;
    let target = premultiplied(pixels, seed);
    let near = |i: usize| close(premultiplied(pixels, i), target, threshold);
    let mut mask = Mask::empty(w, h);
    if contiguous {
        flood(&mut mask.picked, w as usize, seed, near);
    } else {
        for (i, picked) in mask.picked.iter_mut().enumerate() {
            *picked = near(i);
        }
    }
    Some(mask)
}

fn premultiplied(pixels: &[u8], index: usize) -> [u8; 4] {
    let p = &pixels[index * 4..index * 4 + 4];
    let a = u16::from(p[3]);
    let scale = |c: u8| ((u16::from(c) * a + 127) / 255) as u8;
    [scale(p[0]), scale(p[1]), scale(p[2]), p[3]]
}

fn close(a: [u8; 4], b: [u8; 4], threshold: u8) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= threshold)
}

/// Picks every pixel `near` says is close and that is joined to `seed` through others, a run of
/// a row at a time so the stack stays short.
fn flood(picked: &mut [bool], w: usize, seed: usize, near: impl Fn(usize) -> bool) {
    let h = picked.len() / w;
    let mut stack = vec![seed];
    while let Some(start) = stack.pop() {
        if picked[start] || !near(start) {
            continue;
        }
        let (y, row) = (start / w, start - start % w);
        let (mut left, mut right) = (start, start);
        while left > row && !picked[left - 1] && near(left - 1) {
            left -= 1;
        }
        while right + 1 < row + w && !picked[right + 1] && near(right + 1) {
            right += 1;
        }
        picked[left..=right].fill(true);
        let rows = [(y > 0).then(|| row - w), (y + 1 < h).then(|| row + w)];
        for next_row in rows.into_iter().flatten() {
            // One seed for each run of open pixels next to this one.
            let mut in_run = false;
            for x in left - row..=right - row {
                let i = next_row + x;
                let open = !picked[i] && near(i);
                if open && !in_run {
                    stack.push(i);
                }
                in_run = open;
            }
        }
    }
}

/// How much of each pixel stays when the picked ones are cleared, from 0 (gone) to 1 (kept): the
/// picked pixels go, and with a `feather` of more than nothing the kept ones within that many
/// pixels of them fade toward the edge, so the cut is not a staircase. The distance is the
/// chamfer one, two passes over the picture whatever the shape of the area.
pub fn coverage(mask: &Mask, feather: f32) -> Vec<f32> {
    let (w, h) = (mask.width as usize, mask.height as usize);
    if feather <= 0.0 {
        return mask
            .picked
            .iter()
            .map(|p| if *p { 0.0 } else { 1.0 })
            .collect();
    }
    // Thirds of a pixel: 3 to a side neighbour, 4 to a corner one.
    const FAR: u16 = u16::MAX / 2;
    let mut dist: Vec<u16> = mask
        .picked
        .iter()
        .map(|p| if *p { 0 } else { FAR })
        .collect();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let mut d = dist[i];
            if x > 0 {
                d = d.min(dist[i - 1] + 3);
            }
            if y > 0 {
                d = d.min(dist[i - w] + 3);
                if x > 0 {
                    d = d.min(dist[i - w - 1] + 4);
                }
                if x + 1 < w {
                    d = d.min(dist[i - w + 1] + 4);
                }
            }
            dist[i] = d;
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let i = y * w + x;
            let mut d = dist[i];
            if x + 1 < w {
                d = d.min(dist[i + 1] + 3);
            }
            if y + 1 < h {
                d = d.min(dist[i + w] + 3);
                if x + 1 < w {
                    d = d.min(dist[i + w + 1] + 4);
                }
                if x > 0 {
                    d = d.min(dist[i + w - 1] + 4);
                }
            }
            dist[i] = d;
        }
    }
    dist.into_iter()
        .map(|d| {
            if d == 0 {
                0.0
            } else {
                ((f32::from(d) / 3.0 - 0.5) / feather).clamp(0.0, 1.0)
            }
        })
        .collect()
}
