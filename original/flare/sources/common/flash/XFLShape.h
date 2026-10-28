// XFLShape.h - decode <DOMShape> vector geometry from an XFL document
// Copyright (c) 2026 Flare Project
//
// Adobe stores shape outlines in an `edges` attribute on an <Edge> element.
// The grammar is reverse-engineered; there is no first-party specification.
// See the notes at the bottom of this file for what is corroborated and what is
// not.
//
// Coordinates are absolute, in twips (1/20 pixel). A number is either ASCII
// decimal, or a `#`-prefixed signed 32-bit fixed-point value with 8 fractional
// bits, written as `#<int-hex>.<frac-hex>`.
//
// The opcode set is small and every opcode has a fixed arity, so a single
// left-to-right pass decodes a string completely -- the segment count is
// emergent and never has to be computed from the length.
//
//   !x y           moveTo, starts a subpath
//   |x y           lineTo
//   /x y           lineTo (identical to |; there is NO close-path opcode)
//   [cx cy x y     quadratic bezier: control point, then endpoint
//   ]cx cy x y     same as [
//   Sn             style bitmask (1 fillStyle0, 2 fillStyle1, 4 stroke)
//
// Closure is implicit: a filled contour is closed because the last point
// returns to the first, not because of a command. `S` restates the style
// attributes on the <Edge> element and carries no geometry.
//
// The sibling `cubics` attribute is deliberately ignored. It is a cubic-bezier
// hint for Animate's editor, it is present on more <Edge> elements than `edges`
// is, and on real documents it describes a *different* outline: measured against
// the same shape, it contains segments the rendered path does not have. Decoding
// it instead of `edges` produces a differently-shaped path, which is worse than
// producing nothing.

#ifndef XFLSHAPE_H_
#define XFLSHAPE_H_

#include "tcommon.h"
#include "tfilepath.h"

#include <QPointF>
#include <QString>
#include <QVector>

#undef DVAPI
#undef DVVAR
#ifdef TFLASH_EXPORTS
#define DVAPI DV_EXPORT_API
#define DVVAR DV_EXPORT_VAR
#else
#define DVAPI DV_IMPORT_API
#define DVVAR DV_IMPORT_VAR
#endif

namespace XFL {

// One subpath of a shape. Start-vertex styles run forward along the contour;
// end-vertex styles run backward, which is how Animate encodes a shape whose
// two sides are filled differently.
struct Contour {
    QVector<QPointF> points;   // in pixels (twips / 20)
    // One entry per segment, in the same order as the points that follow
    // points[0]. The type has to be recorded rather than inferred: a straight
    // segment contributes one point and a quadratic contributes two (control,
    // then endpoint), so the point stream is not a regular structure and any
    // "every other point is a control point" assumption silently drops points
    // from any contour containing a line.
    enum class Kind { Line, Quad };
    QVector<Kind> segments;
    bool closed = false;       // last point coincides with the first

    int segmentCount() const { return segments.size(); }

    // Geometry, for reporting and for a sanity check on a parse.
    // Declared inline: these are one-line loops, and out-of-line definitions in
    // a module that is compiled into tnzcore with an import/export guard are
    // easy to leave unexported.
    QPointF minBound() const {
        if (points.isEmpty()) return QPointF();
        QPointF m = points.first();
        for (const QPointF &p : points) {
            m.setX(qMin(m.x(), p.x()));
            m.setY(qMin(m.y(), p.y()));
        }
        return m;
    }
    QPointF maxBound() const {
        if (points.isEmpty()) return QPointF();
        QPointF m = points.first();
        for (const QPointF &p : points) {
            m.setX(qMax(m.x(), p.x()));
            m.setY(qMax(m.y(), p.y()));
        }
        return m;
    }
};

// A decoded shape: the contours, plus which of them carry a stroke.
struct Shape {
    QVector<Contour> contours;
    int strokedContours = 0;
    int quadSegments = 0;
    int lineSegments = 0;
    int moveSegments = 0;

    QPointF minBound() const;
    QPointF maxBound() const;
};

// One <Edge> element's worth of geometry, before contour assembly.
struct Edge {
    QString edges;    // the `edges` attribute
    bool hasFill0 = false;
    bool hasFill1 = false;
    bool hasStroke = false;
};

// Decode one `edges` string. Returns false and fills `error` on malformed
// input; a partially-decoded shape is never returned, because a truncated
// contour would fill as something plausible but wrong.
//
// On success every contour is closed, and `strokedContours` counts those that
// came from a stroked <Edge>.
DVAPI bool decodeEdges(const QString &edges, bool stroked, Shape &out,
                       QString &error);

// Decode a whole <DOMShape>: the inner content of a <DOMShape> element, or the
// whole element. `xml` may be either; the reader looks for <Edge> descendants
// and ignores everything else, so a caller does not have to know which.
DVAPI bool decodeShapeXml(const QString &xml, Shape &out, QString &error);

// Convert a decoded shape to an SVG path, in pixels. Quadratics are kept as
// real quadratic segments rather than being flattened, so the result stays
// exact at any zoom.
//
// An empty shape yields an empty string, which is a valid SVG path: a shape with
// no filled or stroked geometry renders as nothing, which is correct rather
// than an error.
DVAPI QString toSvgPath(const Shape &shape);

// The same shape as a standalone SVG document, sized to the shape's own bounds
// with a small margin. Handy for writing a shape out to disk for inspection.
DVAPI QString toSvgDocument(const Shape &shape, const QString &title = QString());

// The number grammar on its own, exposed for testing: "#BD9.4D", "1539.5".
// Returns twips.
DVAPI double decodeXflNumber(const QString &token, bool &ok);

}  // namespace XFL

#endif  // XFLSHAPE_H_
