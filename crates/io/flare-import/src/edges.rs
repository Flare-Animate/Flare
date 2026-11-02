//! XFL edge strings and contour assembly.
//!
//! A DOMShape stores its outline as many small directed edges, each remembering which fill
//! lies on its left (`fillStyle0`) and right (`fillStyle1`) and which stroke it carries.
//! To draw a fill you collect every edge that borders it, orient them consistently, and
//! chain them into closed contours. Holes then fall out of the non-zero winding rule
//! because an inner boundary runs the opposite way to the outer one.
//!
//! Coordinates here are in twips (1/20 px) exactly as stored; callers scale.

use std::collections::HashMap;

use thiserror::Error;

pub type Point = (f64, f64);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Segment {
    Line { from: Point, to: Point },
    Quad { from: Point, ctrl: Point, to: Point },
}

impl Segment {
    pub fn from(&self) -> Point {
        match *self {
            Self::Line { from, .. } | Self::Quad { from, .. } => from,
        }
    }

    pub fn to(&self) -> Point {
        match *self {
            Self::Line { to, .. } | Self::Quad { to, .. } => to,
        }
    }

    /// The same curve traversed the other way.
    pub fn reversed(&self) -> Self {
        match *self {
            Self::Line { from, to } => Self::Line { from: to, to: from },
            Self::Quad { from, ctrl, to } => Self::Quad {
                from: to,
                ctrl,
                to: from,
            },
        }
    }
}

/// Control points of the cubic that exactly equals a quadratic Bezier.
pub fn quad_to_cubic(from: Point, ctrl: Point, to: Point) -> (Point, Point) {
    let lerp = |a: Point| {
        (
            a.0 + 2.0 / 3.0 * (ctrl.0 - a.0),
            a.1 + 2.0 / 3.0 * (ctrl.1 - a.1),
        )
    };
    (lerp(from), lerp(to))
}

#[derive(Debug, Error, PartialEq)]
pub enum EdgeError {
    #[error("unknown edge command {0:?}")]
    UnknownCommand(char),
    #[error("bad number {0:?} in edges")]
    BadNumber(String),
    #[error("command {command:?} needs a multiple of {unit} numbers, got {got}")]
    WrongArity {
        command: char,
        unit: usize,
        got: usize,
    },
    #[error("an edge drawing command came before any move")]
    NoStart,
}

/// Parses one coordinate: plain decimal, or `#` hex with an optional `.hh` fraction
/// (a signed 32-bit integer part plus 1/256ths).
pub fn parse_number(token: &str) -> Result<f64, EdgeError> {
    let bad = || EdgeError::BadNumber(token.to_owned());
    if let Some(hex) = token.strip_prefix('#') {
        let (int_part, frac_part) = hex.split_once('.').unwrap_or((hex, ""));
        if int_part.is_empty() || int_part.len() > 8 || frac_part.len() > 2 {
            return Err(bad());
        }
        let int = u32::from_str_radix(int_part, 16).map_err(|_| bad())?;
        // Two's complement: the stored value is a signed 32-bit integer.
        let mut value = f64::from(i32::from_ne_bytes(int.to_ne_bytes()));
        if !frac_part.is_empty() {
            let frac = u32::from_str_radix(frac_part, 16).map_err(|_| bad())?;
            // One digit means sixteenths, two digits mean 256ths.
            let scale = if frac_part.len() == 1 { 16.0 } else { 256.0 };
            value += f64::from(frac) / scale;
        }
        return Ok(value);
    }
    token
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(bad)
}

/// Decodes an `edges` attribute: `!` move, `|` line, `[` and `]` quadratic (control then
/// anchor). Selection markers (`S` and digits) are skipped.
pub fn decode_edges(text: &str) -> Result<Vec<Segment>, EdgeError> {
    // Split into (command, argument text) chunks.
    let mut chunks: Vec<(char, String)> = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '!' | '|' | '[' | ']' => chunks.push((c, String::new())),
            'S' => {
                while chars.peek().is_some_and(char::is_ascii_digit) {
                    chars.next();
                }
            }
            c if c.is_whitespace() && chunks.is_empty() => {}
            c => match chunks.last_mut() {
                Some((_, args)) => args.push(c),
                None => return Err(EdgeError::UnknownCommand(c)),
            },
        }
    }

    let mut segments = Vec::new();
    let mut current: Option<Point> = None;
    for (command, args) in chunks {
        let numbers = args
            .split_whitespace()
            .map(parse_number)
            .collect::<Result<Vec<_>, _>>()?;
        let unit = if matches!(command, '[' | ']') { 4 } else { 2 };
        if numbers.is_empty() || numbers.len() % unit != 0 {
            return Err(EdgeError::WrongArity {
                command,
                unit,
                got: numbers.len(),
            });
        }
        match command {
            '!' => {
                // A move takes exactly one point.
                if numbers.len() != 2 {
                    return Err(EdgeError::WrongArity {
                        command,
                        unit: 2,
                        got: numbers.len(),
                    });
                }
                current = Some((numbers[0], numbers[1]));
            }
            '|' => {
                for pair in numbers.as_chunks::<2>().0 {
                    let from = current.ok_or(EdgeError::NoStart)?;
                    let to = (pair[0], pair[1]);
                    segments.push(Segment::Line { from, to });
                    current = Some(to);
                }
            }
            _ => {
                for quad in numbers.as_chunks::<4>().0 {
                    let from = current.ok_or(EdgeError::NoStart)?;
                    let (ctrl, to) = ((quad[0], quad[1]), (quad[2], quad[3]));
                    segments.push(Segment::Quad { from, ctrl, to });
                    current = Some(to);
                }
            }
        }
    }
    Ok(segments)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Contour {
    pub segments: Vec<Segment>,
    /// Whether the last segment ends where the first began.
    pub closed: bool,
}

/// Endpoints match when they agree to 1/256 twip, absorbing decoding round-off.
fn key(point: Point) -> (i64, i64) {
    (
        (point.0 * 256.0).round() as i64,
        (point.1 * 256.0).round() as i64,
    )
}

/// Links segments end-to-start into contours, in input order, using each segment once.
pub fn chain(segments: &[Segment]) -> Vec<Contour> {
    let mut starts: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (index, segment) in segments.iter().enumerate() {
        starts.entry(key(segment.from())).or_default().push(index);
    }
    let mut used = vec![false; segments.len()];
    let mut contours = Vec::new();
    for first in 0..segments.len() {
        if used[first] {
            continue;
        }
        used[first] = true;
        let origin = key(segments[first].from());
        let mut contour = vec![segments[first]];
        let mut closed = false;
        loop {
            let end = key(contour[contour.len() - 1].to());
            if end == origin {
                closed = true;
                break;
            }
            let next = starts
                .get(&end)
                .and_then(|c| c.iter().copied().find(|i| !used[*i]));
            match next {
                Some(i) => {
                    used[i] = true;
                    contour.push(segments[i]);
                }
                None => break,
            }
        }
        contours.push(Contour {
            segments: contour,
            closed,
        });
    }
    contours
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn line(a: Point, b: Point) -> Segment {
        Segment::Line { from: a, to: b }
    }

    #[test]
    fn decodes_moves_lines_and_quadratics() {
        let segments = decode_edges("!0 0|100 0|100 50[150 75 200 50").expect("decode");
        assert_eq!(
            segments,
            vec![
                line((0.0, 0.0), (100.0, 0.0)),
                line((100.0, 0.0), (100.0, 50.0)),
                Segment::Quad {
                    from: (100.0, 50.0),
                    ctrl: (150.0, 75.0),
                    to: (200.0, 50.0)
                },
            ]
        );
        // `]` is also a quadratic, several pairs may follow one `|`, and S markers are skipped.
        let more = decode_edges("!0 0 S3|10 0 20 0]30 5 40 0").expect("decode");
        assert_eq!(more.len(), 3);
        assert_eq!(more[1].to(), (20.0, 0.0));
        assert!(matches!(more[2], Segment::Quad { .. }));
    }

    #[test]
    fn numbers_may_be_hex_with_fractions_and_negative() {
        assert_eq!(parse_number("42").expect("n"), 42.0);
        assert_eq!(parse_number("-12.5").expect("n"), -12.5);
        assert_eq!(parse_number("#C8").expect("n"), 200.0);
        assert_eq!(parse_number("#C8.80").expect("n"), 200.5);
        assert_eq!(parse_number("#C8.8").expect("n"), 200.5);
        assert_eq!(parse_number("#FFFFFFE0").expect("n"), -32.0);
        for bad in [
            "",
            "abc",
            "#",
            "#GG",
            "#123456789",
            "#C8.123",
            "NaN",
            "inf",
            "1e999",
        ] {
            assert!(
                matches!(parse_number(bad), Err(EdgeError::BadNumber(_))),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn malformed_edge_strings_are_errors_not_panics() {
        assert_eq!(decode_edges("|1 1"), Err(EdgeError::NoStart));
        assert_eq!(decode_edges("1 1"), Err(EdgeError::UnknownCommand('1')));
        assert!(matches!(
            decode_edges("!0"),
            Err(EdgeError::WrongArity { .. })
        ));
        assert!(matches!(
            decode_edges("!0 0 1 1"),
            Err(EdgeError::WrongArity { .. })
        ));
        assert!(matches!(
            decode_edges("!0 0|1 1 2"),
            Err(EdgeError::WrongArity { .. })
        ));
        assert!(matches!(
            decode_edges("!0 0[1 1 2"),
            Err(EdgeError::WrongArity { .. })
        ));
        assert!(matches!(
            decode_edges("!0 0|x y"),
            Err(EdgeError::BadNumber(_))
        ));
        assert_eq!(decode_edges("").expect("empty is fine"), vec![]);
    }

    #[test]
    fn quadratics_convert_to_exact_cubics() {
        let (c1, c2) = quad_to_cubic((0.0, 0.0), (30.0, 60.0), (60.0, 0.0));
        assert_eq!((c1, c2), ((20.0, 40.0), (40.0, 40.0)));
    }

    #[test]
    fn edges_chain_into_a_closed_contour_even_when_listed_out_of_order() {
        let square = [
            line((0.0, 0.0), (10.0, 0.0)),
            line((10.0, 10.0), (0.0, 10.0)),
            line((10.0, 0.0), (10.0, 10.0)),
            line((0.0, 10.0), (0.0, 0.0)),
        ];
        let contours = chain(&square);
        assert_eq!(contours.len(), 1);
        assert!(contours[0].closed);
        assert_eq!(contours[0].segments.len(), 4);
        let ends: Vec<Point> = contours[0].segments.iter().map(Segment::to).collect();
        assert_eq!(
            ends,
            vec![(10.0, 0.0), (10.0, 10.0), (0.0, 10.0), (0.0, 0.0)]
        );
    }

    #[test]
    fn separate_shapes_and_open_runs_stay_separate() {
        let a = [
            line((0.0, 0.0), (5.0, 0.0)),
            line((5.0, 0.0), (5.0, 5.0)),
            line((5.0, 5.0), (0.0, 0.0)),
        ];
        let b = [
            line((20.0, 0.0), (25.0, 0.0)),
            line((25.0, 0.0), (25.0, 5.0)),
        ]; // never closes
        let all: Vec<Segment> = a.iter().chain(&b).copied().collect();
        let contours = chain(&all);
        assert_eq!(contours.len(), 2);
        assert!(contours[0].closed && !contours[1].closed);
        assert!(chain(&[]).is_empty());
    }

    #[test]
    fn endpoints_match_despite_tiny_decoding_differences() {
        let almost = [
            line((0.0, 0.0), (10.0, 0.0)),
            line((10.000_001, 0.0), (0.0, 0.0)),
        ];
        let contours = chain(&almost);
        assert_eq!(contours.len(), 1);
        assert!(contours[0].closed);
    }

    #[test]
    fn a_hole_runs_the_opposite_way_to_its_outline() {
        // Outer square counter-clockwise, inner square clockwise: the winding numbers cancel.
        let outer = [
            line((0.0, 0.0), (20.0, 0.0)),
            line((20.0, 0.0), (20.0, 20.0)),
            line((20.0, 20.0), (0.0, 20.0)),
            line((0.0, 20.0), (0.0, 0.0)),
        ];
        let inner = [
            line((5.0, 5.0), (5.0, 15.0)),
            line((5.0, 15.0), (15.0, 15.0)),
            line((15.0, 15.0), (15.0, 5.0)),
            line((15.0, 5.0), (5.0, 5.0)),
        ];
        let all: Vec<Segment> = outer.iter().chain(&inner).copied().collect();
        let contours = chain(&all);
        assert_eq!(contours.len(), 2);
        assert!(contours.iter().all(|c| c.closed));
        let area = |c: &Contour| -> f64 {
            c.segments
                .iter()
                .map(|s| s.from().0 * s.to().1 - s.to().0 * s.from().1)
                .sum::<f64>()
                / 2.0
        };
        assert!(
            area(&contours[0]) * area(&contours[1]) < 0.0,
            "opposite orientations"
        );
    }

    #[test]
    fn reversing_a_segment_swaps_ends_and_keeps_the_control_point() {
        let q = Segment::Quad {
            from: (0.0, 0.0),
            ctrl: (5.0, 9.0),
            to: (10.0, 0.0),
        };
        let r = q.reversed();
        assert_eq!((r.from(), r.to()), ((10.0, 0.0), (0.0, 0.0)));
        assert_eq!(r.reversed(), q);
    }

    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0 >> 33
        }
    }

    #[test]
    fn random_token_soup_never_panics_and_valid_results_chain_without_panicking() {
        let alphabet: Vec<char> = "!|[]S0123456789 .-#abcdefABCDEF\t".chars().collect();
        let mut rng = Rng(0xED6E5);
        let (mut ok, mut err) = (0, 0);
        for _ in 0..20_000 {
            let len = (rng.next() % 60) as usize;
            let text: String = (0..len)
                .map(|_| alphabet[(rng.next() as usize) % alphabet.len()])
                .collect();
            match decode_edges(&text) {
                Ok(segments) => {
                    ok += 1;
                    for contour in chain(&segments) {
                        assert!(!contour.segments.is_empty());
                    }
                }
                Err(_) => err += 1,
            }
        }
        assert!(
            ok > 100 && err > 100,
            "both outcomes are exercised ({ok} ok, {err} err)"
        );
    }

    #[test]
    fn very_long_edge_strings_are_processed_in_linear_time() {
        // 200k segments; a quadratic chaining step would not finish.
        let mut text = String::from("!0 0");
        for i in 1..=200_000_u32 {
            text.push_str(&format!("|{i} {}", i % 7));
        }
        let segments = decode_edges(&text).expect("decode");
        assert_eq!(segments.len(), 200_000);
        let contours = chain(&segments);
        assert_eq!(contours.len(), 1);
        assert!(!contours[0].closed);
    }

    #[test]
    fn a_single_point_closing_loop_and_shared_endpoints_chain_without_looping_forever() {
        // Degenerate: a zero-length segment is its own closed contour.
        let dot = [line((3.0, 3.0), (3.0, 3.0))];
        assert!(chain(&dot)[0].closed);
        // Three segments meeting at one point (a branch): each is used exactly once.
        let star = [
            line((0.0, 0.0), (5.0, 0.0)),
            line((5.0, 0.0), (5.0, 5.0)),
            line((5.0, 0.0), (9.0, 9.0)),
        ];
        let total: usize = chain(&star).iter().map(|c| c.segments.len()).sum();
        assert_eq!(total, 3);
    }
}
