//! Measuring, and drawing along a straight edge.

use flare_input::InputSample;

/// The distance and direction between two points on the stage.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measurement {
    pub dx: f64,
    pub dy: f64,
    pub distance: f64,
    /// Degrees counter-clockwise from the positive x axis, with up positive as on a protractor,
    /// in `(-180, 180]`. Zero for two equal points.
    pub angle: f64,
}

impl Measurement {
    /// From `a` to `b` in stage units, or `None` if a coordinate is not finite.
    pub fn between(a: (f32, f32), b: (f32, f32)) -> Option<Self> {
        let (dx, dy) = (f64::from(b.0 - a.0), f64::from(b.1 - a.1));
        if !(dx.is_finite() && dy.is_finite()) {
            return None;
        }
        let angle = if dx == 0.0 && dy == 0.0 {
            0.0
        } else {
            // A flat leftward line has a negative zero for `-dy`, which would read as -180.
            let degrees = (-dy).atan2(dx).to_degrees();
            if degrees <= -180.0 {
                degrees + 360.0
            } else {
                degrees
            }
        };
        Some(Self {
            dx,
            dy,
            distance: dx.hypot(dy),
            angle,
        })
    }

    /// The reading shown beside the ruler.
    pub fn label(&self) -> String {
        format!(
            "{:.1} px at {:.1}°  (Δx {:.1}, Δy {:.1})",
            self.distance, self.angle, self.dx, self.dy
        )
    }
}

/// A stroke reduced to the straight line from its first point to its last, so the pencil or
/// brush draws along an edge. A stroke of fewer than two points is returned unchanged.
pub fn straighten(samples: &[InputSample]) -> Vec<InputSample> {
    match (samples.first(), samples.last()) {
        (Some(first), Some(last)) if samples.len() > 1 => vec![*first, *last],
        _ => samples.to_vec(),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use flare_input::PointerKind;

    fn sample(x: f64, y: f64) -> InputSample {
        InputSample::new(x, y, 1.0, PointerKind::Mouse).expect("finite")
    }

    #[test]
    fn a_3_4_5_triangle_measures_five() {
        let m = Measurement::between((0.0, 0.0), (3.0, 4.0)).expect("finite");
        assert_eq!(m.distance, 5.0);
        assert_eq!((m.dx, m.dy), (3.0, 4.0));
    }

    #[test]
    fn angles_read_like_a_protractor_with_up_positive() {
        let at = |b: (f32, f32)| Measurement::between((0.0, 0.0), b).expect("finite").angle;
        assert_eq!(at((10.0, 0.0)), 0.0);
        assert_eq!(at((0.0, -10.0)), 90.0);
        assert_eq!(at((-10.0, 0.0)), 180.0);
        assert_eq!(at((0.0, 10.0)), -90.0);
        assert!((at((10.0, -10.0)) - 45.0).abs() < 1e-9);
    }

    #[test]
    fn two_equal_points_measure_nothing_and_non_numbers_are_refused() {
        let m = Measurement::between((2.0, 2.0), (2.0, 2.0)).expect("finite");
        assert_eq!((m.distance, m.angle), (0.0, 0.0));
        assert!(Measurement::between((0.0, 0.0), (f32::NAN, 1.0)).is_none());
        assert!(Measurement::between((0.0, 0.0), (f32::INFINITY, 1.0)).is_none());
    }

    #[test]
    fn the_label_gives_distance_angle_and_offsets() {
        let m = Measurement::between((0.0, 0.0), (3.0, -4.0)).expect("finite");
        assert_eq!(m.label(), "5.0 px at 53.1°  (Δx 3.0, Δy -4.0)");
    }

    #[test]
    fn a_wobbly_stroke_becomes_its_two_ends() {
        let wobble = [
            sample(0.0, 0.0),
            sample(5.0, 3.0),
            sample(9.0, -2.0),
            sample(20.0, 10.0),
        ];
        let line = straighten(&wobble);
        assert_eq!(line, [wobble[0], wobble[3]]);
    }

    #[test]
    fn a_single_point_or_nothing_is_left_alone() {
        assert!(straighten(&[]).is_empty());
        assert_eq!(straighten(&[sample(1.0, 1.0)]).len(), 1);
    }
}
