//! Morphological antialiasing on RGBA8 buffers — the Toonz image filter's behaviour
//! (`tantialias`, Alexander Reshetov's staircase smoothing), re-implemented clean-room
//! from that behaviour on our own row-major, 4-bytes-per-pixel buffers: red, green, blue,
//! alpha, in that order. The old machine's BGRA channel order and its ink/paint pixel
//! format do not carry over; this crate has no model for them.
//!
//! A straight boundary between two flats is already clean and stays untouched: the filter
//! exists for staircases, where runs of one colour end inside a line instead of at its
//! edge, and those ends are pulled toward the neighbouring line over a shrinking ramp.

/// Why an antialias pass could not run.
#[derive(Debug, PartialEq, Eq)]
pub enum AntialiasError {
    /// The buffer's length does not match `width * height * 4`, or an extent is empty.
    BadBuffer,
    /// A threshold outside 1..=255 counts nothing or everything as equal, and a softness
    /// past 100 would ramp over more pixels than a sheet can hold; a softness of zero is
    /// not an error but the identity, so it never reaches this check.
    BadParameters,
}

/// Antialiases `buf` in place (4 bytes per pixel, straight alpha, row-major).
///
/// Two passes read the incoming pixels and write blends into a working copy the caller
/// receives back through `buf`: horizontally over each pair of neighbouring rows, then
/// vertically over each pair of neighbouring columns. Pixels are "equal" when every
/// channel (the alpha included) differs by less than `threshold`. Where the two lines of
/// a pair disagree, the run of disagreement's inner ends are blended toward the other
/// line: the first `softness / 100` steps of the run each take the area of a triangle
/// that starts half a pixel tall and loses `50 / softness` of height per step, and a
/// partial step takes the remaining sliver — so a softness of 50 pulls the first run
/// pixel 12.5% toward the neighbour and a softness of 100 pulls it 25%, with the ramp
/// fading into the run.
pub fn tantialias(
    buf: &mut [u8],
    w: u32,
    h: u32,
    threshold: u8,
    softness: u8,
) -> Result<(), AntialiasError> {
    if w == 0 || h == 0 || buf.len() as u64 != u64::from(w) * u64::from(h) * 4 {
        return Err(AntialiasError::BadBuffer);
    }
    if softness == 0 {
        return Ok(());
    }
    if threshold == 0 || softness > 100 {
        return Err(AntialiasError::BadParameters);
    }
    let (w, h) = (w as usize, h as usize);
    let slope = 50.0 / f64::from(softness);
    let h_start = 0.5;
    let base = h_start / slope;
    let starting_run = base.floor() as usize;
    // The working copy carries the writes; the source stays untouched through both
    // passes, so what each pass reads is what the caller gave.
    let mut out = buf.to_vec();
    let src = buf.to_vec();

    let equal = |a: usize, b: usize| -> bool {
        src[a * 4..a * 4 + 4]
            .iter()
            .zip(&src[b * 4..b * 4 + 4])
            .all(|(x, y)| x.abs_diff(*y) < threshold)
    };
    // Blends the pixel at `at` of the working copy toward the source pixel at `other` by
    // the ramp's area share; the read pixel keeps the rest.
    let pull = |out: &mut [u8], at: usize, other: usize, share: f64| {
        for channel in 0..4 {
            let keep = f64::from(out[at * 4 + channel]) * (1.0 - share);
            let take = f64::from(src[other * 4 + channel]) * share;
            out[at * 4 + channel] = (keep + take).round().clamp(0.0, 255.0) as u8;
        }
    };

    // One line pair of either axis. `at` maps a position on the line to the pixel's index
    // in the buffer; `lx` is the line's length. The filtering mirrors the original's
    // processLine: skip the positions where the pair agrees, then at each disagreement
    // ramp the first line's pixels toward the second and the second's back toward the
    // first, from the run's inner ends, with the height falling by `slope` per step.

    for y in 0..h - 1 {
        let l = |i: usize| y * w + i;
        let u = |i: usize| (y + 1) * w + i;
        filter_pair(
            &mut out,
            l,
            u,
            w,
            equal,
            pull,
            h_start,
            slope,
            starting_run,
            base,
        );
    }
    for x in 0..w - 1 {
        let l = |i: usize| i * w + x;
        let u = |i: usize| i * w + x + 1;
        filter_pair(
            &mut out,
            l,
            u,
            h,
            equal,
            pull,
            h_start,
            slope,
            starting_run,
            base,
        );
    }
    buf.copy_from_slice(&out);
    Ok(())
}

/// Filters one pair of lines: `l` and `u` map a position along the line (length `lx`) to
/// the pixel's index in the buffer. The `equal` and `pull` operations carry their own
/// view of the source; blends land in `out`.
#[allow(clippy::too_many_arguments)]
fn filter_pair(
    out: &mut [u8],
    l: impl Fn(usize) -> usize,
    u: impl Fn(usize) -> usize,
    lx: usize,
    equal: impl Fn(usize, usize) -> bool,
    pull: impl Fn(&mut [u8], usize, usize, f64),
    h_start: f64,
    slope: f64,
    starting_run: usize,
    base: f64,
) {
    let mut cur = 0;
    // Leading positions where the pair agrees are already clean.
    while cur < lx && equal(l(cur), u(cur)) {
        cur += 1;
    }
    while cur < lx {
        // The run of disagreement: positions whose line-local colours still match the
        // run's first ones. A run that reaches the line's end is a straight boundary
        // between the flats, and stays as it is.
        let mut run_end = cur + 1;
        while run_end < lx && equal(l(cur), l(run_end)) && equal(u(cur), u(run_end)) {
            run_end += 1;
        }
        let length = run_end - cur;
        if run_end == lx {
            break;
        }
        // The first line's side: a falling triangle of height from `h_start`, one step of
        // area per run pixel until the triangle is spent, then the sliver.
        let mut height = h_start;
        let steps = starting_run.min(length);
        for step in 0..steps {
            let next = height - slope;
            pull(out, l(cur + step), u(cur + step), (height + next) * 0.5);
            height = next;
        }
        if steps < length {
            let sliver = (base - steps as f64) * height * 0.5;
            pull(out, l(cur + steps), u(cur + steps), sliver);
        }
        // The second line's side, mirrored from the run's far end back toward the first.
        let mut height = h_start;
        for step in 0..steps {
            let next = height - slope;
            pull(
                out,
                u(run_end - 1 - step),
                l(run_end - 1 - step),
                (height + next) * 0.5,
            );
            height = next;
        }
        if steps < length {
            let sliver = (base - steps as f64) * height * 0.5;
            pull(out, u(run_end - 1 - steps), l(run_end - 1 - steps), sliver);
        }
        // Past the run, skip the pair's next agreeing stretch.
        cur = run_end;
        while cur < lx && equal(l(cur), u(cur)) {
            cur += 1;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    const RED: [u8; 4] = [255, 0, 0, 255];
    const BLUE: [u8; 4] = [0, 0, 255, 255];

    fn set(sheet: &mut [u8], w: usize, x: usize, y: usize, pixel: [u8; 4]) {
        sheet[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&pixel);
    }

    fn get(sheet: &[u8], w: usize, x: usize, y: usize) -> [u8; 4] {
        sheet[(y * w + x) * 4..(y * w + x) * 4 + 4]
            .try_into()
            .unwrap()
    }

    #[test]
    fn zero_softness_is_the_identity() {
        let mut sheet = vec![7u8; 3 * 2 * 4];
        tantialias(&mut sheet, 3, 2, 10, 0).unwrap();
        assert_eq!(sheet, vec![7u8; 3 * 2 * 4]);
    }

    #[test]
    fn one_colour_never_moves() {
        let sheet = [3u8, 9, 40, 255].repeat(6 * 4);
        let mut buf = sheet.clone();
        tantialias(&mut buf, 6, 4, 10, 50).unwrap();
        assert_eq!(buf, sheet);
    }

    /// A red run that ends inside a red line, over blue: at softness 50 the triangle
    /// reaches only half a pixel, so the run's first pixel takes 12.5% of the neighbour.
    #[test]
    fn an_inner_run_end_is_pulled_twelve_and_a_half_percent() {
        // Rows: all red, then red red blue blue red, then all blue.
        let mut sheet = vec![0u8; 5 * 3 * 4];
        for x in 0..5 {
            set(&mut sheet, 5, x, 0, RED);
            set(&mut sheet, 5, x, 2, BLUE);
        }
        for x in 0..2 {
            set(&mut sheet, 5, x, 1, RED);
        }
        for x in 2..4 {
            set(&mut sheet, 5, x, 1, BLUE);
        }
        set(&mut sheet, 5, 4, 1, RED);
        tantialias(&mut sheet, 5, 3, 10, 50).unwrap();
        // The red line's inner end pulled toward the blue under it; the blue line's inner
        // end pulled back toward the red above it.
        assert_eq!(get(&sheet, 5, 2, 0), [223, 0, 32, 255]);
        // The blue line's inner end is pulled twice: 12.5% toward the red above it by the
        // horizontal pass, then 12.5% toward the red beside it by the vertical pair of
        // columns 3 and 4, which disagree at the same corner.
        assert_eq!(get(&sheet, 5, 3, 1), [60, 0, 195, 255]);
        // The flats and the run's untouched parts are exactly what they were — except the
        // corner's right neighbour, which the vertical pair of columns 3 and 4 pulls from
        // its own side (it is that pair's second line), again by 12.5%.
        assert_eq!(get(&sheet, 5, 0, 0), RED);
        assert_eq!(get(&sheet, 5, 4, 1), [223, 0, 32, 255]);
        assert_eq!(get(&sheet, 5, 2, 2), BLUE);
    }

    #[test]
    fn softer_pulls_harder_and_fades_into_the_run() {
        // Same sheet at softness 100: the triangle covers a whole step (25%) and the next
        // run pixel only the sliver (nothing, the height is spent).
        let mut sheet = vec![0u8; 5 * 3 * 4];
        for x in 0..5 {
            set(&mut sheet, 5, x, 0, RED);
            set(&mut sheet, 5, x, 2, BLUE);
        }
        for x in 0..2 {
            set(&mut sheet, 5, x, 1, RED);
        }
        for x in 2..4 {
            set(&mut sheet, 5, x, 1, BLUE);
        }
        set(&mut sheet, 5, 4, 1, RED);
        tantialias(&mut sheet, 5, 3, 10, 100).unwrap();
        assert_eq!(get(&sheet, 5, 2, 0), [191, 0, 64, 255]);
        assert_eq!(get(&sheet, 5, 3, 0), RED, "the sliver carries nothing here");
        // 25% twice: the horizontal pull, then the vertical columns 3 and 4.
        assert_eq!(get(&sheet, 5, 3, 1), [112, 0, 143, 255]);
    }

    /// The alpha takes part in the equality and in the pull: two rows of the same colour
    /// at different coverages ramp their alpha at the run's inner end. Two rows keep the
    /// vertical pass out of the way, so the pair (0, 1) is the only filter.
    #[test]
    fn the_alpha_ramps_with_the_colour() {
        let mut sheet = vec![0u8; 4 * 2 * 4];
        let solid = [10u8, 20, 30, 255];
        let faint = [10u8, 20, 30, 40];
        for x in 0..4 {
            set(&mut sheet, 4, x, 0, solid);
        }
        for x in 0..2 {
            set(&mut sheet, 4, x, 1, solid);
        }
        set(&mut sheet, 4, 2, 1, faint);
        set(&mut sheet, 4, 3, 1, solid);
        tantialias(&mut sheet, 4, 2, 10, 50).unwrap();
        // |255 - 40| = 215, far past the threshold: the top row's inner end takes 12.5%
        // of the faint pixel (255 * 7/8 + 40/8 = 228.125), and the faint pixel itself
        // takes the same share back toward the solid one (40 * 7/8 + 255/8 = 66.875).
        assert_eq!(get(&sheet, 4, 2, 0), [10, 20, 30, 228]);
        assert_eq!(get(&sheet, 4, 2, 1), [10, 20, 30, 67]);
        // The agreeing stretches and the run's far edge are exactly what they were.
        assert_eq!(get(&sheet, 4, 0, 0), solid);
        assert_eq!(get(&sheet, 4, 3, 1), solid);
    }

    #[test]
    fn broken_parameters_are_refused() {
        let mut sheet = vec![0u8; 2 * 2 * 4];
        assert_eq!(
            tantialias(&mut sheet, 2, 2, 0, 50),
            Err(AntialiasError::BadParameters)
        );
        assert_eq!(
            tantialias(&mut sheet, 2, 2, 10, 101),
            Err(AntialiasError::BadParameters)
        );
        assert_eq!(
            tantialias(&mut sheet, 3, 2, 10, 50),
            Err(AntialiasError::BadBuffer)
        );
        assert_eq!(
            tantialias(&mut sheet, 0, 2, 10, 50),
            Err(AntialiasError::BadBuffer)
        );
    }
}
