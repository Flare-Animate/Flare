//! Ticks along the stage rulers.

/// A tick on a ruler: where it is on screen, what stage coordinate it marks, and whether it
/// is a labelled one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tick {
    pub screen: f32,
    pub value: f32,
    pub major: bool,
}

/// Ticks for a ruler along one axis. `origin` is the screen position of stage coordinate 0 and
/// `scale` the screen points per stage unit; ticks are made for the screen span `from..to`. The
/// step between labelled ticks is the first of 1, 2, 5, 10, 20, 50... stage units that leaves at
/// least `min_gap` points between them, and there are ten minor ticks between labelled ones.
pub fn ruler_ticks(origin: f32, scale: f32, from: f32, to: f32, min_gap: f32) -> Vec<Tick> {
    if ![origin, scale, from, to, min_gap]
        .iter()
        .all(|v| v.is_finite())
        || scale <= 0.0
        || to <= from
    {
        return Vec::new();
    }
    let mut step = 1.0_f32;
    let mut guard = 0;
    while step * scale < min_gap && guard < 64 {
        step *= match guard % 3 {
            0 => 2.0,
            1 => 2.5,
            _ => 2.0,
        };
        guard += 1;
    }
    let minor = step / 10.0;
    let first = ((from - origin) / scale / minor).floor() as i64;
    let last = ((to - origin) / scale / minor).ceil() as i64;
    // Never more than a few hundred ticks, whatever the numbers say.
    if last - first > 2_000 {
        return Vec::new();
    }
    (first..=last)
        .filter_map(|i| {
            let value = i as f32 * minor;
            let screen = origin + value * scale;
            // Minor ticks closer than 3 points are noise.
            let major = i % 10 == 0;
            (screen >= from && screen <= to && (major || minor * scale >= 3.0)).then_some(Tick {
                screen,
                value,
                major,
            })
        })
        .collect()
}
