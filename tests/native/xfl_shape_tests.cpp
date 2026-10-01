// Tests for the XFL <DOMShape> decoder.
//
// The hex fixed-point decoder is where real implementations get it wrong, so
// those cases are pinned from the format description rather than from whatever
// the code happens to produce. The real-document tests then check the whole
// pipeline on geometry taken verbatim from a real FLA.
//
// usage: xfl_shape_tests <extracted-fla-dir>
#include "XFLShape.h"

#include <QCoreApplication>
#include <cstdio>
#include <cmath>

static int gFail = 0;
static int gChecks = 0;

static void check(bool ok, const char *what, const QString &detail = {}) {
    ++gChecks;
    fprintf(stderr, "   [%s] %s", ok ? "ok  " : "FAIL", what);
    if (!detail.isEmpty()) fprintf(stderr, "  (%s)", qPrintable(detail));
    fprintf(stderr, "\n");
    if (!ok) ++gFail;
}

static void checkNear(double got, double want, const char *what,
                      double tol = 1e-4) {
    const bool ok = std::fabs(got - want) <= tol;
    check(ok, what, QString("%1 vs %2").arg(got, 0, 'f', 6).arg(want, 0, 'f', 6));
}

// ---------------------------------------------------------------------------
// Numbers
// ---------------------------------------------------------------------------
static void test_numbers() {
    fprintf(stderr, "\n-- number grammar --\n");

    bool ok = false;

    // Decimal is twips already.
    checkNear(XFL::decodeXflNumber("0", ok), 0.0, "0", 0);
    check(ok, "0 decodes");
    checkNear(XFL::decodeXflNumber("1539.5", ok), 1539.5, "1539.5");
    checkNear(XFL::decodeXflNumber("-493.5", ok), -493.5, "-493.5");
    checkNear(XFL::decodeXflNumber("-219.5", ok), -219.5, "-219.5");

    // #<int-hex>.<frac-hex> is a signed 32-bit value with 8 fractional bits.
    // The int part is padded LEFT to six digits, the frac part RIGHT to two.
    checkNear(XFL::decodeXflNumber("#BD9.4D", ok), 3033.30078125,
              "#BD9.4D", 1e-9);
    // The single-digit-fraction case: a one-digit frac means 0x..20, not 0x..2.
    // Getting this wrong is a known trap in published implementations.
    checkNear(XFL::decodeXflNumber("#19F.2", ok), 415.125, "#19F.2", 1e-9);
    checkNear(XFL::decodeXflNumber("#6.F7", ok), 6.96484375, "#6.F7", 1e-9);
    checkNear(XFL::decodeXflNumber("#FFFD68.46", ok), -663.7265625,
              "#FFFD68.46", 1e-9);
    checkNear(XFL::decodeXflNumber("#FFFF9B.F9", ok), -100.02734375,
              "#FFFF9B.F9", 1e-9);
    // An 8-digit integer part exceeds the format's 6, so this is malformed.
    // (A note on the reverse-engineered format lists it as a value, having
    // padded it; that padding is not part of the grammar.)
    check(!XFL::decodeXflNumber("#FFFF9BF9", ok), "#FFFF9BF9 is rejected");

    // A two-digit fraction is used as-is.
    checkNear(XFL::decodeXflNumber("#0.FF", ok), 255.0 / 256.0, "#0.FF", 1e-9);

    // Sign extension across the whole 32 bits, not per part. Six integer digits
    // is 0xFFFFFF00, which as a signed 32-bit is -256, hence -1.0 twips.
    checkNear(XFL::decodeXflNumber("#FFFFFF", ok), -1.0, "#FFFFFF", 1e-9);
    // One digit lower in the integer part is 256 raw units lower, so exactly
    // 1.0 twip lower. (The absent fraction pads to "00", not to nothing.)
    checkNear(XFL::decodeXflNumber("#FFFFFE", ok), -2.0, "#FFFFFE", 1e-9);

    // Malformed input must be rejected rather than silently yielding 0.
    for (const char *bad : {"", "#", "#.", "#GG", "#1234567", "#12.345",
                            "#12.34567", "abc", "-", "+", "#-1"}) {
        bool bad_ok = true;
        const double v = XFL::decodeXflNumber(QString::fromLatin1(bad), bad_ok);
        check(!bad_ok, QString("rejects \"%1\"").arg(QString::fromLatin1(bad))
                          .toLatin1().constData());
        Q_UNUSED(v);
    }
}

// ---------------------------------------------------------------------------
// Opcode arity and semantics
// ---------------------------------------------------------------------------
static void test_opcodes() {
    fprintf(stderr, "\n-- opcodes --\n");

    XFL::Shape s;
    QString err;

    // A moveTo plus two lineTos. Coordinates are twips, so /20 gives pixels.
    const bool ok = XFL::decodeEdges("!0 0|400 0|400 400", false, s, err);
    check(ok, "a line-only shape decodes", err);
    check(s.contours.size() == 1, "one contour", QString::number(s.contours.size()));
    if (!s.contours.isEmpty()) {
        const XFL::Contour &c = s.contours.first();
        check(c.points.size() == 3, "three points",
              QString::number(c.points.size()));
        checkNear(c.points.at(0).x(), 0.0, "starts at x=0");
        checkNear(c.points.at(1).x(), 20.0, "lineTo x=20px (400 twips)");
        checkNear(c.points.at(2).y(), 20.0, "lineTo y=20px (400 twips)");
    }
    check(s.lineSegments == 2, "two line segments",
          QString::number(s.lineSegments));
    check(s.moveSegments == 1, "one move segment",
          QString::number(s.moveSegments));

    // '/' is a lineTo, identical to '|'. There is no close-path opcode in this
    // format. Treating '/' as closePath is a common misreading that yields a
    // differently-shaped path, so assert the segment is produced at all.
    XFL::Shape s2;
    check(XFL::decodeEdges("!0 0/200 0|200 200", false, s2, err),
          "a shape using '/' decodes", err);
    check(s2.lineSegments == 2,
          "'/' produces a lineTo like '|', not a close",
          QString::number(s2.lineSegments));
    check(s2.contours.size() == 1, "'/' does not start a new contour",
          QString::number(s2.contours.size()));

    // A quadratic keeps its control point, then its endpoint.
    XFL::Shape s3;
    check(XFL::decodeEdges("!0 0[100 -100 200 0", false, s3, err),
          "a shape with a quadratic decodes", err);
    check(s3.quadSegments == 1, "one quadratic", QString::number(s3.quadSegments));
    if (!s3.contours.isEmpty()) {
        const auto &p = s3.contours.first().points;
        check(p.size() == 3, "move + control + endpoint",
              QString::number(p.size()));
        if (p.size() == 3) {
            checkNear(p.at(1).x(), 5.0, "control x = 100/20");
            checkNear(p.at(1).y(), -5.0, "control y = -100/20");
            checkNear(p.at(2).x(), 10.0, "endpoint x = 200/20");
        }
    }

    // ']' is a quadratic too, per the grammar.
    XFL::Shape s4;
    check(XFL::decodeEdges("!0 0]100 -100 200 0", false, s4, err),
          "']' decodes", err);
    check(s4.quadSegments == 1, "']' is a quadratic",
          QString::number(s4.quadSegments));

    // The redundant moveTo after every quadratic: Flash restates the current
    // point. If those were treated as subpath breaks the fill would be wrong.
    XFL::Shape s5;
    const QString restated =
        "!375 -340 [256 -470 38 -487 !38 -487 [-69 -495 -157 -472";
    check(XFL::decodeEdges(restated, false, s5, err),
          "a restated-moveTo shape decodes", err);
    check(s5.contours.size() == 1,
          "a restated moveTo does not split the contour",
          QString::number(s5.contours.size()));
    if (!s5.contours.isEmpty()) {
        // "!375 -340 [c e !e [c e" : two moveTos (one a restatement) and two
        // quadratics, so 1 + 2 + 2 = 5 points, and 2 segments. The restated
        // point must not appear: a duplicate would desynchronise the point list
        // from the segment list and shift every control point after it.
        const XFL::Contour &c = s5.contours.first();
        check(c.points.size() == 5, "a restatement adds no point",
              QString::number(c.points.size()));
        check(c.segmentCount() == 2, "two segments recorded",
              QString::number(c.segmentCount()));
    }

    // The S bitmask carries no geometry.
    XFL::Shape s6;
    check(XFL::decodeEdges("!0 0 S3|200 0", false, s6, err),
          "an S opcode decodes", err);
    check(s6.lineSegments == 1, "S adds no segment",
          QString::number(s6.lineSegments));

    // Whitespace, including newlines, is insignificant -- real attribute
    // values contain them.
    XFL::Shape s7;
    check(XFL::decodeEdges("!0 0\n|200 0\n[100 100 300 0", false, s7, err),
          "newlines in the attribute are tolerated", err);
    check(s7.lineSegments == 1 && s7.quadSegments == 1,
          "newlines do not disturb the parse");

    // Leading whitespace, which real data has.
    XFL::Shape s8;
    check(XFL::decodeEdges(" !2347 -1311|1940 -1284", false, s8, err),
          "leading whitespace is tolerated", err);
}

// ---------------------------------------------------------------------------
// Closure
// ---------------------------------------------------------------------------
static void test_closure() {
    fprintf(stderr, "\n-- closure --\n");

    XFL::Shape s;
    QString err;
    // A contour whose last point returns to its first is closed. Closure is a
    // property of the geometry, not a command.
    check(XFL::decodeEdges("!0 0|200 0|200 200|0 0", false, s, err),
          "a returning contour decodes", err);
    if (!s.contours.isEmpty())
        check(s.contours.first().closed, "a contour returning to its start is closed");

    XFL::Shape s2;
    check(XFL::decodeEdges("!0 0|200 0|200 200", false, s2, err),
          "a non-returning contour decodes", err);
    if (!s2.contours.isEmpty())
        check(!s2.contours.first().closed,
              "a contour that does not return is not closed");

    // Two moveTos make two contours.
    XFL::Shape s3;
    check(XFL::decodeEdges("!0 0|200 0!400 0|600 0", false, s3, err),
          "a two-contour shape decodes", err);
    check(s3.contours.size() == 2, "two contours",
          QString::number(s3.contours.size()));
}

// ---------------------------------------------------------------------------
// Malformed input: a half-decoded contour would fill as something plausible
// but wrong, so every one of these must be rejected outright.
// ---------------------------------------------------------------------------
static void test_malformed() {
    fprintf(stderr, "\n-- malformed input --\n");

    const char *cases[] = {
        "!",        // moveTo with no coordinates
        "!0",       // moveTo with one coordinate
        "!0 0|",    // lineTo with nothing after it
        "!0 0|200", // lineTo with one coordinate
        "!0 0[100", // quadratic with too few numbers
        "!0 0[1 2 3",
        "!0 0?",    // not an opcode
        "0 0",      // does not start with a moveTo
        "|200 0",   // a line before any move
        "!0 0S",    // S with no style digit
    };
    for (const char *c : cases) {
        XFL::Shape s;
        QString err;
        const bool ok = XFL::decodeEdges(QString::fromLatin1(c), false, s, err);
        check(!ok, QString("rejects \"%1\"").arg(QString::fromLatin1(c))
                      .toLatin1().constData());
        check(!err.isEmpty(), "the rejection carries a reason");
    }

    // An empty string is not an error: <Edge> elements with only a `cubics`
    // attribute and no geometry exist in real documents.
    XFL::Shape s;
    QString err;
    check(XFL::decodeEdges("", false, s, err), "an empty string is not an error");
    check(s.contours.isEmpty(), "an empty string yields no contours");
}

// ---------------------------------------------------------------------------
// Whole-shape parsing, including the `cubics` decision.
// ---------------------------------------------------------------------------
static void test_shape_xml() {
    fprintf(stderr, "\n-- shape XML --\n");

    XFL::Shape s;
    QString err;
    const QString xml =
        "<edges>"
        "<Edge fillStyle0=\"1\" edges=\"!0 0|400 0|400 400\"/>"
        "<Edge strokeStyle=\"1\" edges=\"!0 0|200 0\"/>"
        "</edges>";
    check(XFL::decodeShapeXml(xml, s, err), "a shape with two edges decodes", err);
    check(s.contours.size() == 2, "both edges contribute",
          QString::number(s.contours.size()));
    check(s.strokedContours == 1, "one edge is marked as stroked",
          QString::number(s.strokedContours));

    // An <Edge> carrying only `cubics` has no rendered geometry. Decoding
    // `cubics` instead of `edges` is the documented wrong answer: on real
    // documents the two describe different outlines, so it would produce a
    // differently-shaped path.
    XFL::Shape s2;
    const QString cubicsOnly =
        "<edges><Edge cubics=\"!0 0(;100,-100 200,0 300,0)\"/></edges>";
    check(XFL::decodeShapeXml(cubicsOnly, s2, err), "a cubics-only edge decodes",
          err);
    check(s2.contours.isEmpty(),
          "a cubics-only edge contributes no geometry");

    // An <Edge> with both: `edges` is authoritative.
    XFL::Shape s3;
    const QString both =
        "<edges><Edge fillStyle0=\"1\" "
        "edges=\"!0 0|100 0\" "
        "cubics=\"!0 0(;50,-50 100,0 150,0)\"/></edges>";
    check(XFL::decodeShapeXml(both, s3, err), "an edge with both decodes", err);
    check(s3.contours.size() == 1, "one contour from the edges attribute",
          QString::number(s3.contours.size()));
    if (!s3.contours.isEmpty()) {
        checkNear(s3.contours.first().points.last().x(), 5.0,
                  "the endpoints come from `edges`, not `cubics`");
    }

    // Malformed XML.
    XFL::Shape s4;
    check(!XFL::decodeShapeXml("<edges><Edge edges=\"!0 0|\"", s4, err),
          "malformed edge data is rejected");
    check(!err.isEmpty(), "the rejection carries a reason");
}

// ---------------------------------------------------------------------------
// Geometry taken verbatim from a real Animate document.
// ---------------------------------------------------------------------------
static void test_real_edge() {
    fprintf(stderr, "\n-- geometry from a real FLA --\n");

    XFL::Shape s;
    QString err;
    // A shape from "Grandfather Clock Body.xml": a moveTo, a line, a moveTo
    // restating it, a quadratic, another restatement, a line, a moveTo, a
    // quadratic in hex notation, and a return to the start.
    const QString real =
        "!2892 -2575/1604 -2524!1604 -2524[1519 -2523 1453 -2499"
        "!1453 -2499[1396 -2479 1333 -2435!1333 -2435/2697.5 -2507"
        "!2697.5 -2507[#AE4.8D #FFF5FB.C1 2892 -2575";

    check(XFL::decodeEdges(real, false, s, err), "a real edge decodes", err);
    if (!s.contours.isEmpty()) {
        const XFL::Contour &c = s.contours.first();
        check(c.closed, "the real shape's contour is closed");
        checkNear(c.points.first().x() * 20.0, 2892.0,
                  "first point x in twips matches the source");
        checkNear(c.points.first().y() * 20.0, -2575.0,
                  "first point y in twips matches the source");

        // The hex tokens are the interesting part: #AE4.8D is 44594.55 twips and
        // #FFF5FB.C1 is -26469.24, and the contour returns exactly to its
        // start, so the decode is self-consistent.
        const QPointF lo = c.minBound();
        const QPointF hi = c.maxBound();
        check(lo.x() < hi.x() && lo.y() < hi.y(), "the contour has extent");
        // The two hex tokens are the final quadratic's control point:
        //   #AE4.8D    -> 0x000AE48D =  2788.55078125 twips
        //   #FFF5FB.C1 -> 0xFFF5FBC1 = -2564.24609375 twips
        // Both are inside the contour's x range, so the bounds are set by the
        // decimal tokens, and the hex ones are verified directly instead.
        checkNear(lo.x() * 20.0, 1333.0, "min x from the decimal tokens", 0.01);
        checkNear(hi.x() * 20.0, 2892.0, "max x from the decimal tokens", 0.01);
        checkNear(lo.y() * 20.0, -2575.0, "min y from the decimal tokens", 0.01);
        checkNear(hi.y() * 20.0, -2435.0, "max y from the decimal tokens", 0.01);
        bool sawHexX = false, sawHexY = false;
        for (const QPointF &p : c.points) {
            if (qAbs(p.x() - 2788.55078125 / 20.0) < 1e-6) sawHexX = true;
            if (qAbs(p.y() + 2564.24609375 / 20.0) < 1e-6) sawHexY = true;
        }
        check(sawHexX, "#AE4.8D decoded to 2788.55 twips as the control x");
        check(sawHexY, "#FFF5FB.C1 decoded to -2564.25 twips as the control y");
    }

    // The shortest real line-only edge.
    XFL::Shape s2;
    check(XFL::decodeEdges(" !2347 -1311|1940 -1284", true, s2, err),
          "a short real edge decodes", err);
    check(s2.contours.size() == 1, "one contour");
    check(s2.strokedContours == 1, "marked as stroked");
    if (!s2.contours.isEmpty()) {
        checkNear(s2.contours.first().points.first().x(), 117.35,
                  "start x = 2347/20");
        checkNear(s2.contours.first().points.last().x(), 97.0, "end x = 1940/20");
    }
}


// ---------------------------------------------------------------------------
// SVG output
// ---------------------------------------------------------------------------
static void test_svg() {
    fprintf(stderr, "\n-- SVG output --\n");

    // A triangle: move, two lines, closing back to the start.
    XFL::Shape s;
    QString err;
    check(XFL::decodeEdges("!0 0|400 0|400 400|0 0", false, s, err),
          "the triangle decodes", err);
    const QString d = XFL::toSvgPath(s);
    check(d.startsWith("M0 0"), "path starts with a moveto", d.left(20));
    // The previous form was `d.contains("L400 0") || d.endsWith("Z")`, and the
    // path does end with Z, so the disjunction was unconditionally true and the
    // property it named was never checked.
    check(!d.contains(QLatin1Char('Q')),
          "a line-only contour has no quadratic", d);
    // The input is in twips and the path is in pixels, so 400 -> 20.
    check(d.contains("L20 0") && d.contains("L20 20") && d.contains("L0 0"),
          "every line endpoint is present", d);
    check(d.endsWith("Z"), "a closed contour gets an explicit Z", d.right(8));

    // A quadratic must stay a quadratic, not be flattened.
    XFL::Shape q;
    check(XFL::decodeEdges("!0 0[100 -100 200 0", false, q, err),
          "the quadratic decodes", err);
    const QString qd = XFL::toSvgPath(q);
    check(qd.contains("Q"), "a quadratic is emitted as Q", qd);
    check(qd.contains("Q5 -5 10 0"),
          "the control point and endpoint are in the path", qd);

    // An unclosed contour must not get a Z, or the stroke would join up.
    XFL::Shape o;
    check(XFL::decodeEdges("!0 0|200 0", false, o, err), "the open shape decodes");
    check(!XFL::toSvgPath(o).endsWith("Z"),
          "an unclosed contour gets no Z");

    // A contour that mixes lines and quadratics. A straight segment adds one
    // point and a quadratic adds two, so the point stream is irregular; a
    // "every other point" reading silently drops everything after a line. This
    // is the case that catches it.
    XFL::Shape mix;
    check(XFL::decodeEdges("!0 0|200 0[300 -100 400 0|500 0", false, mix, err),
          "a mixed line/quadratic contour decodes", err);
    check(mix.contours.size() == 1, "one contour");
    if (!mix.contours.isEmpty()) {
        const XFL::Contour &c = mix.contours.first();
        // move + line(1) + quad(2) + line(1) = 5 points, 3 segments.
        check(c.points.size() == 5, "five points",
              QString::number(c.points.size()));
        check(c.segmentCount() == 3, "three segments recorded",
              QString::number(c.segmentCount()));
    }
    const QString md = XFL::toSvgPath(mix);
    check(md.count(QLatin1Char('L')) == 2, "both lines are emitted", md);
    check(md.count(QLatin1Char('Q')) == 1, "the quadratic is emitted", md);
    // Every endpoint must be present. The input is in twips, so the emitted
    // coordinates are twips/20: 200 -> 10, 300/400 -> 15/20, 500 -> 25.
    check(md.contains("L10 0") && md.contains("Q15 -5 20 0") &&
              md.contains("L25 0"),
          "no endpoint is dropped", md);

    // An empty shape is valid input and yields an empty path, not garbage.
    XFL::Shape empty;
    check(XFL::decodeEdges("", false, empty, err), "an empty shape decodes");
    check(XFL::toSvgPath(empty).isEmpty(), "an empty shape yields an empty path");

    // A whole document, sized to the shape's bounds.
    const QString doc = XFL::toSvgDocument(s, "triangle");
    check(doc.contains("<svg"), "the document has an svg root");
    check(doc.contains("viewBox="), "the document has a viewBox");
    check(doc.contains("<title>triangle</title>"), "the title is included");
    // The triangle spans 0..20 px, plus the 2px margin on each side.
    check(doc.contains("width=\"24\""), "the width includes the margin",
          doc.left(200));
    check(doc.contains("viewBox=\"-2 -2 24 24\""), "the viewBox is offset by the margin",
          doc.left(200));

    // An empty shape still yields a parseable document rather than nothing.
    const QString edoc = XFL::toSvgDocument(empty);
    check(edoc.contains("<svg"), "an empty shape still yields an svg document");
    check(edoc.contains("viewBox=\"0 0 0 0\""), "with a zero viewBox");
}

int main(int argc, char **argv) {
    QCoreApplication app(argc, argv);
    // argv[1] is accepted for symmetry with the other suites; these cases are
    // self-contained by design, so a fixture directory is not required.
    (void)argc;
    (void)argv;

    test_numbers();
    test_opcodes();
    test_closure();
    test_malformed();
    test_shape_xml();
    test_real_edge();
    test_svg();

    fprintf(stderr, "\n%s: %d checks, %d failure(s)\n",
            gFail ? "FAILED" : "PASSED", gChecks, gFail);
    return gFail ? 1 : 0;
}
