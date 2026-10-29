//! Mirrored drawing: the reflections of a stroke across an axis, or two, that cross at a point.

use flare_input::InputSample;
use glamx::{DAffine2, DVec2};

/// Which axes a stroke is mirrored across.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Symmetry {
    #[default]
    Off,
    /// Across a vertical axis: left and right are mirror images.
    Vertical,
    /// Across a horizontal axis: top and bottom are mirror images.
    Horizontal,
    /// Across both: four-way symmetry.
    Both,
}

/// One reflection: which coordinates are mirrored about the axes' crossing point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reflection {
    pub flip_x: bool,
    pub flip_y: bool,
}

impl Symmetry {
    pub const ALL: [Self; 4] = [Self::Off, Self::Vertical, Self::Horizontal, Self::Both];

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Vertical => "Left-right",
            Self::Horizontal => "Top-bottom",
            Self::Both => "Four-way",
        }
    }

    /// The copies made besides the stroke itself.
    pub fn reflections(self) -> &'static [Reflection] {
        const X: Reflection = Reflection {
            flip_x: true,
            flip_y: false,
        };
        const Y: Reflection = Reflection {
            flip_x: false,
            flip_y: true,
        };
        const XY: Reflection = Reflection {
            flip_x: true,
            flip_y: true,
        };
        match self {
            Self::Off => &[],
            Self::Vertical => &[X],
            Self::Horizontal => &[Y],
            Self::Both => &[X, Y, XY],
        }
    }

    /// Whether a vertical axis is drawn, and whether a horizontal one is.
    pub fn axes(self) -> (bool, bool) {
        match self {
            Self::Off => (false, false),
            Self::Vertical => (true, false),
            Self::Horizontal => (false, true),
            Self::Both => (true, true),
        }
    }
}

impl Reflection {
    /// The transform that mirrors the plane about the point `about`.
    pub fn matrix(self, about: DVec2) -> DAffine2 {
        let scale = DVec2::new(
            if self.flip_x { -1.0 } else { 1.0 },
            if self.flip_y { -1.0 } else { 1.0 },
        );
        DAffine2::from_translation(about)
            * DAffine2::from_scale(scale)
            * DAffine2::from_translation(-about)
    }

    /// `samples` mirrored about `about`; pressure and kind are kept.
    pub fn samples(self, samples: &[InputSample], about: DVec2) -> Vec<InputSample> {
        let matrix = self.matrix(about);
        samples
            .iter()
            .filter_map(|s| {
                let p = matrix.transform_point2(DVec2::new(s.x, s.y));
                InputSample::new(p.x, p.y, s.pressure, s.kind)
            })
            .collect()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use flare_input::PointerKind;

    fn sample(x: f64, y: f64) -> InputSample {
        InputSample::new(x, y, 0.5, PointerKind::Pen).expect("finite")
    }

    #[test]
    fn each_mode_makes_the_copies_it_names() {
        let counts: Vec<usize> = Symmetry::ALL
            .iter()
            .map(|s| s.reflections().len())
            .collect();
        assert_eq!(counts, [0, 1, 1, 3]);
        assert_eq!(Symmetry::Both.axes(), (true, true));
        assert_eq!(Symmetry::Off.axes(), (false, false));
    }

    #[test]
    fn a_left_right_mirror_swaps_sides_and_keeps_height_and_pressure() {
        let r = Symmetry::Vertical.reflections()[0];
        let out = r.samples(&[sample(30.0, 7.0)], DVec2::new(100.0, 50.0));
        assert_eq!((out[0].x, out[0].y), (170.0, 7.0));
        assert_eq!(out[0].pressure, 0.5);
        assert_eq!(out[0].kind, PointerKind::Pen);
    }

    #[test]
    fn four_way_symmetry_reaches_every_quadrant() {
        let about = DVec2::new(10.0, 10.0);
        let mut seen: Vec<(f64, f64)> = Symmetry::Both
            .reflections()
            .iter()
            .map(|r| {
                let s = r.samples(&[sample(12.0, 13.0)], about);
                (s[0].x, s[0].y)
            })
            .collect();
        seen.sort_by(f64_pair);
        assert_eq!(seen, [(8.0, 7.0), (8.0, 13.0), (12.0, 7.0)]);
    }

    fn f64_pair(a: &(f64, f64), b: &(f64, f64)) -> std::cmp::Ordering {
        a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1))
    }

    #[test]
    fn a_point_on_the_axis_does_not_move() {
        let r = Symmetry::Vertical.reflections()[0];
        let out = r.samples(&[sample(100.0, 3.0)], DVec2::new(100.0, 0.0));
        assert_eq!(out[0].x, 100.0);
    }

    #[test]
    fn mirroring_twice_returns_the_original() {
        let r = Symmetry::Both.reflections()[2];
        let about = DVec2::new(3.5, -2.0);
        let once = r.samples(&[sample(9.0, 4.0)], about);
        let twice = r.samples(&once, about);
        assert!((twice[0].x - 9.0).abs() < 1e-9 && (twice[0].y - 4.0).abs() < 1e-9);
    }
}
