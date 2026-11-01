//! Text objects: the outline of a [`TextSource`] as a shape, and the fonts there are to set it in.

use flare_core::{DocumentPoint, Rgba8, ShapeId};
use flare_document::{PathSegment, Shape, TextSource};

use crate::{TextEngine, TextError, TextOutline, TextStyle};

/// Most font families listed, so a machine with a strange number of them cannot make a huge menu.
pub const MAX_FAMILIES: usize = 2_000;

impl TextOutline {
    /// This outline as the shape of `source`: its corner at the source's origin, filled with
    /// `fill`, and remembering the source so the text can be edited again.
    pub fn into_shape(
        self,
        id: ShapeId,
        fill: Rgba8,
        source: TextSource,
    ) -> Result<Shape, TextError> {
        let origin = source.origin().as_vec();
        let shift = |p: &DocumentPoint| {
            let q = p.as_vec() + origin;
            DocumentPoint::new(q.x, q.y).map_err(|_| TextError::OutOfRange)
        };
        let path = self
            .path
            .iter()
            .map(|segment| {
                Ok(match segment {
                    PathSegment::MoveTo(p) => PathSegment::MoveTo(shift(p)?),
                    PathSegment::LineTo(p) => PathSegment::LineTo(shift(p)?),
                    PathSegment::CubicTo(a, b, c) => {
                        PathSegment::CubicTo(shift(a)?, shift(b)?, shift(c)?)
                    }
                    PathSegment::Close => PathSegment::Close,
                })
            })
            .collect::<Result<Vec<_>, TextError>>()?;
        Ok(Shape::new(id, path, fill)
            .map_err(|_| TextError::OutOfRange)?
            .with_text(source))
    }
}

impl TextEngine {
    /// The shape `source` sets, with identity `id` and `fill`.
    pub fn shape(
        &mut self,
        source: &TextSource,
        id: ShapeId,
        fill: Rgba8,
    ) -> Result<Shape, TextError> {
        let style = TextStyle {
            family: source.family().to_owned(),
            size: source.size(),
            bold: source.bold(),
            italic: source.italic(),
        };
        self.outline(source.text(), &style)?
            .into_shape(id, fill, source.clone())
    }

    /// The names of the font families that can be set, sorted and without repeats.
    pub fn families(&mut self) -> Vec<String> {
        let mut names: Vec<String> = self
            .fonts
            .collection
            .family_names()
            .map(str::to_owned)
            .collect();
        names.sort_by_key(|n| n.to_lowercase());
        names.dedup();
        names.truncate(MAX_FAMILIES);
        names
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn engine() -> TextEngine {
        let mut engine = TextEngine::new();
        engine.register_font(epaint_default_fonts::HACK_REGULAR.to_vec());
        engine
    }

    fn source(text: &str, at: (f64, f64)) -> TextSource {
        TextSource::new(
            text,
            "Hack",
            24.0,
            false,
            false,
            DocumentPoint::new(at.0, at.1).expect("point"),
        )
        .expect("source")
    }

    fn left_top(shape: &Shape) -> (f64, f64) {
        let points = shape.path().iter().flat_map(|s| match s {
            PathSegment::MoveTo(p) | PathSegment::LineTo(p) => vec![p.as_vec()],
            PathSegment::CubicTo(a, b, c) => vec![a.as_vec(), b.as_vec(), c.as_vec()],
            PathSegment::Close => vec![],
        });
        let (mut x, mut y) = (f64::MAX, f64::MAX);
        for p in points {
            x = x.min(p.x);
            y = y.min(p.y);
        }
        (x, y)
    }

    #[test]
    fn a_source_sets_a_shape_at_its_origin_that_remembers_the_source() {
        let mut engine = engine();
        let here = engine
            .shape(&source("Hi", (0.0, 0.0)), ShapeId::new(), Rgba8::WHITE)
            .expect("shape");
        let there = engine
            .shape(&source("Hi", (100.0, 50.0)), ShapeId::new(), Rgba8::WHITE)
            .expect("shape");
        let (a, b) = (left_top(&here), left_top(&there));
        assert!((b.0 - a.0 - 100.0).abs() < 1e-6 && (b.1 - a.1 - 50.0).abs() < 1e-6);
        assert_eq!(there.text().map(TextSource::text), Some("Hi"));
        assert_eq!(there.text().map(|t| t.origin().as_vec().x), Some(100.0));
    }

    #[test]
    fn text_with_nothing_to_draw_is_an_error_not_an_empty_shape() {
        let mut engine = engine();
        // A space has no outline.
        assert!(
            engine
                .shape(&source(" x ", (0.0, 0.0)), ShapeId::new(), Rgba8::WHITE)
                .is_ok()
        );
        let blank = TextSource::new(
            "\u{200b}",
            "Hack",
            24.0,
            false,
            false,
            DocumentPoint::new(0.0, 0.0).expect("point"),
        )
        .expect("source");
        assert!(engine.shape(&blank, ShapeId::new(), Rgba8::WHITE).is_err());
    }

    #[test]
    fn the_bundled_font_is_always_there_to_set_text_in() {
        let mut engine = TextEngine::new();
        assert!(engine.families().iter().any(|f| f == crate::BUNDLED_FAMILY));
        let source = TextSource::new(
            "Hi",
            crate::DEFAULT_FAMILY,
            24.0,
            false,
            false,
            DocumentPoint::new(0.0, 0.0).expect("point"),
        )
        .expect("source");
        assert!(engine.shape(&source, ShapeId::new(), Rgba8::WHITE).is_ok());
        let only_bundled = TextSource::new(
            "Hi",
            crate::BUNDLED_FAMILY,
            24.0,
            false,
            false,
            DocumentPoint::new(0.0, 0.0).expect("point"),
        )
        .expect("source");
        assert!(
            engine
                .shape(&only_bundled, ShapeId::new(), Rgba8::WHITE)
                .is_ok()
        );
    }

    #[test]
    fn the_registered_font_is_listed_once_among_the_families() {
        let mut engine = engine();
        let families = engine.families();
        assert_eq!(families.iter().filter(|f| *f == "Hack").count(), 1);
        assert!(
            families
                .windows(2)
                .all(|w| w[0].to_lowercase() <= w[1].to_lowercase())
        );
        assert!(families.len() <= MAX_FAMILIES);
    }
}
