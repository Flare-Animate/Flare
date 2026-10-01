// XFLShape.cpp - decode <DOMShape> vector geometry from an XFL document
// Copyright (c) 2026 Flare Project
//
// See XFLShape.h for the format and for why `cubics` is ignored.

#include "XFLShape.h"

#include <QXmlStreamReader>

namespace XFL {

namespace {

// Twips per pixel. Flash stores shape coordinates as twips throughout.
constexpr double kTwipsPerPixel = 20.0;

// A guard against pathological input: a shape with more segments than this is
// malformed or hostile, and allocating for it would be worse than rejecting it.
constexpr int kMaxSegments = 1 << 22;

// How far a moveTo may land from the current point and still count as a
// restatement rather than a subpath break. Flash restates the current point
// bit-identically, so this only has to absorb rounding; it is absolute so that
// the answer does not depend on the coordinate's magnitude.
constexpr double kPointEpsilon = 1e-6;

// --- tokenizer -------------------------------------------------------------
//
// Hand-written rather than regex-based, for two reasons that both bite on real
// data: an 'S' opcode is immediately followed by a digit that must not be
// lexed as a number, and attribute values contain raw newlines.

enum class Op { Move, Line, Quad, Style, Number };

struct Token {
    Op op = Op::Move;
    // A coordinate, for Op::Number. Kept in its own member rather than reusing
    // the unused a/b/c/d: a number used to be tagged Op::Line, the same enum
    // value as a '|' opcode, so a number in an opcode position was silently
    // read as an opcode with coordinate 0 instead of being rejected.
    double value = 0.0;
};

// For an error message: what a token turned out to be. Saying "found a lineTo"
// is actionable; saying "found 0" is not.
QString describeToken(const Token &t) {
    switch (t.op) {
        case Op::Move:  return QObject::tr("a moveTo");
        case Op::Line:  return QObject::tr("a lineTo");
        case Op::Quad:  return QObject::tr("a quadratic");
        case Op::Style: return QObject::tr("a style marker");
        case Op::Number: return QObject::tr("the number %1")
                               .arg(t.value, 0, 'g', 12);
    }
    return QObject::tr("an unknown token");
}

inline bool isDigit(char c) { return c >= '0' && c <= '9'; }
inline bool isHex(char c) {
    return isDigit(c) || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F');
}
inline int hexVal(char c) {
    if (c <= '9') return c - '0';
    return (c | 0x20) - 'a' + 10;
}

// Tokenize the whole string. Returns false on a character that cannot start
// any token, which is the one way a malformed string is detectable up front.
bool tokenize(const QString &s, QVector<Token> &out, QString &error) {
    const int n = s.size();
    int i = 0;
    auto skipSpace = [&] {
        while (i < n && (s.at(i) == ' ' || s.at(i) == '\t' ||
                         s.at(i) == '\n' || s.at(i) == '\r'))
            ++i;
    };

    while (true) {
        skipSpace();
        if (i >= n) return true;

        const QChar ch = s.at(i);
        switch (ch.unicode()) {
            case '!':
            case '|':
            case '/':
            case '[':
            case ']':
                // '/' and ']' are aliases: '/' is a lineTo and ']' is a
                // quadratic, byte-for-byte identical to '|' and '['.
                out.append(Token{(ch == '!'                    ? Op::Move
                                  : (ch == '[' || ch == ']')  ? Op::Quad
                                                             : Op::Line),
                                 0.0});
                ++i;
                break;
            case 'S': {
                // S is immediately followed by a single style digit 1..7. It is
                // a restatement of the <Edge> style attributes, not a geometry
                // change, so the value is read and discarded.
                const int sPos = i;
                ++i;
                if (i >= n || !isDigit(s.at(i).toLatin1())) {
                    error = QObject::tr(
                        "S opcode is not followed by a style digit at offset %1")
                                .arg(sPos);
                    return false;
                }
                out.append(Token{Op::Style, double(s.at(i).toLatin1() - '0')});
                ++i;
                break;
            }
            default:
                if (ch == '#' || ch == '-' || ch == '+' || isDigit(ch.toLatin1())) {
                    const int start = i;
                    if (ch == '#') {
                        ++i;
                        const int hexStart = i;
                        while (i < n && isHex(s.at(i).toLatin1())) ++i;
                        if (i < n && s.at(i) == '.') {
                            ++i;
                            while (i < n && isHex(s.at(i).toLatin1())) ++i;
                        }
                        if (i == hexStart) {
                            error = QObject::tr(
                                        "malformed hex number at offset %1")
                                        .arg(start);
                            return false;
                        }
                    } else {
                        if (ch == '-' || ch == '+') ++i;
                        while (i < n && isDigit(s.at(i).toLatin1())) ++i;
                        if (i < n && s.at(i) == '.') {
                            ++i;
                            while (i < n && isDigit(s.at(i).toLatin1())) ++i;
                        }
                        if (i == start || (i == start + 1 &&
                                           (s.at(start) == '-' ||
                                            s.at(start) == '+'))) {
                            error = QObject::tr("malformed number at offset %1")
                                        .arg(start);
                            return false;
                        }
                    }
                    const QString tok = s.mid(start, i - start);
                    bool ok = false;
                    const double v = decodeXflNumber(tok, ok);
                    if (!ok) {
                        error = QObject::tr("cannot decode number \"%1\"").arg(tok);
                        return false;
                    }
                    out.append(Token{Op::Number, v});
                    break;
                }
                error = QObject::tr("unexpected character '%1' at offset %2")
                            .arg(ch).arg(i);
                return false;
        }
    }
}

}  // namespace

// ---------------------------------------------------------------------------
// Numbers
// ---------------------------------------------------------------------------

double decodeXflNumber(const QString &token, bool &ok) {
    ok = false;
    if (token.isEmpty()) return 0.0;

    if (!token.startsWith(QLatin1Char('#'))) {
        bool conv = false;
        const double v = token.toDouble(&conv);
        if (!conv) return 0.0;
        ok = true;
        return v;  // already twips
    }

    // #<int-hex>.<frac-hex>: a signed 32-bit value with 8 fractional bits.
    // The integer part is padded on the LEFT to six digits and the fractional
    // part on the RIGHT to two, so the two together are exactly eight hex
    // digits of a 32-bit two's-complement number.
    const QString body = token.mid(1);
    const int dot = body.indexOf(QLatin1Char('.'));
    QString intHex = (dot < 0) ? body : body.left(dot);
    QString fracHex = (dot < 0) ? QString() : body.mid(dot + 1);

    if (intHex.isEmpty()) return 0.0;
    for (int i = 0; i < intHex.size(); ++i)
        if (!isHex(intHex.at(i).toLatin1())) return 0.0;
    for (int i = 0; i < fracHex.size(); ++i)
        if (!isHex(fracHex.at(i).toLatin1())) return 0.0;

    if (intHex.size() > 6 || fracHex.size() > 2) return 0.0;

    intHex = intHex.rightJustified(6, QLatin1Char('0'));
    fracHex = fracHex.leftJustified(2, QLatin1Char('0'));

    quint32 raw = 0;
    for (const QChar c : intHex + fracHex)
        raw = (raw << 4) | static_cast<quint32>(hexVal(c.toLatin1()));

    // The high bit is a sign bit; cast through qint32 to get the value.
    const qint32 v = static_cast<qint32>(raw);
    ok = true;
    return static_cast<double>(v) / 256.0;
}

// ---------------------------------------------------------------------------
// Shape bounds. Contour::minBound/maxBound are inline in the header.
// ---------------------------------------------------------------------------

QPointF Shape::minBound() const {
    if (contours.isEmpty()) return QPointF();
    QPointF m = contours.first().minBound();
    for (const Contour &c : contours) {
        const QPointF cMin = c.minBound();
        m.setX(qMin(m.x(), cMin.x()));
        m.setY(qMin(m.y(), cMin.y()));
    }
    return m;
}

QPointF Shape::maxBound() const {
    if (contours.isEmpty()) return QPointF();
    QPointF m = contours.first().maxBound();
    for (const Contour &c : contours) {
        const QPointF cMax = c.maxBound();
        m.setX(qMax(m.x(), cMax.x()));
        m.setY(qMax(m.y(), cMax.y()));
    }
    return m;
}

// ---------------------------------------------------------------------------
// Edges
// ---------------------------------------------------------------------------

bool decodeEdges(const QString &edges, bool stroked, Shape &out, QString &error) {
    out = Shape();
    error.clear();

    QVector<Token> toks;
    if (!tokenize(edges, toks, error)) return false;
    if (toks.isEmpty()) {
        // An <Edge> with an empty or whitespace-only string carries no geometry.
        // That is legitimate, not an error: <Edge> elements exist for styling
        // only in some documents.
        return true;
    }

    // Pair each opcode with the numbers that follow it. The tokenizer stored
    // every number as an Op::Line token with its value in `a`, so the arity of
    // the opcode that precedes them tells us how many to take.
    struct Raw {
        char op;   // '!', '|', '/', '[' or 'n' for a number
        double v;  // pixels, for 'n'
    };
    QVector<Raw> raw;
    raw.reserve(toks.size());
    int ti = 0;
    while (ti < toks.size()) {
        const Token &t = toks.at(ti);
        if (t.op == Op::Style) {  // no geometry
            ++ti;
            continue;
        }
        const int arity = (t.op == Op::Quad) ? 4 : 2;
        if (ti + arity >= toks.size()) {
            error = QObject::tr("truncated coordinate list");
            return false;
        }
        // The consumed tokens must be numbers. Without this, a number sitting
        // where an opcode belongs would be read as an opcode with a zero
        // coordinate, and an opcode sitting where a number belongs would be read
        // as the number 0 -- either way producing a plausible-looking contour
        // instead of an error.
        for (int k = 1; k <= arity; ++k) {
            if (toks.at(ti + k).op != Op::Number) {
                error = QObject::tr(
                            "expected a coordinate at offset %1 but found %2")
                            .arg(ti + k)
                            .arg(describeToken(toks.at(ti + k)));
                return false;
            }
        }
        // '/' and ']' were normalised to '|' and '[' by the tokenizer, so the
        // character is a label for the segment kind, not a distinct opcode.
        const char op = (t.op == Op::Move)  ? '!'
                        : (t.op == Op::Quad) ? '['
                                             : '|';
        raw.append(Raw{op, 0});
        for (int k = 1; k <= arity; ++k)
            raw.append(Raw{'n', toks.at(ti + k).value / kTwipsPerPixel});
        ti += arity + 1;
    }

    if (raw.isEmpty()) return true;

    // A contour has to start with a moveTo; anything else is a truncated shape.
    if (raw.first().op != '!') {
        error = QObject::tr("edge data does not begin with a moveTo");
        return false;
    }

    Contour cur;
    QPointF at(0, 0);
    int segments = 0;

    auto flush = [&] {
        if (cur.points.size() >= 2) {
            // Closure is implicit in this format: a filled contour's last point
            // returns to its first. Record whether it does, rather than
            // synthesising a close.
            const QPointF f = cur.points.first();
            const QPointF l = cur.points.last();
            cur.closed = qAbs(f.x() - l.x()) < kPointEpsilon &&
                         qAbs(f.y() - l.y()) < kPointEpsilon;
            out.contours.append(cur);
            if (stroked) ++out.strokedContours;
        }
        cur = Contour();
    };

    int i = 0;
    while (i < raw.size()) {
        if (++segments > kMaxSegments) {
            error = QObject::tr("edge data exceeds the segment limit");
            return false;
        }
        const char op = raw.at(i).op;
        if (op == '!') {
            if (i + 2 >= raw.size() || raw.at(i + 1).op != 'n' ||
                raw.at(i + 2).op != 'n') {
                error = QObject::tr("moveTo without two coordinates");
                return false;
            }
            const QPointF dest(raw.at(i + 1).v, raw.at(i + 2).v);
            // Flash restates the current point as a fresh moveTo after every
            // quadratic. Those are not subpath breaks: treating them as breaks
            // splits each outline into disconnected fragments, and the fill
            // comes out wrong. A moveTo that goes nowhere is a restatement.
            // An absolute tolerance, not qFuzzyCompare: that is a relative
            // comparison whose tolerance scales with the magnitude, so it is
            // neither the test the format describes nor the one the Python
            // reference in tests/native/differential_shape.py implements. The
            // two must agree or the differential check compares different rules.
            const bool restates =
                !cur.points.isEmpty() && qAbs(at.x() - dest.x()) < kPointEpsilon &&
                qAbs(at.y() - dest.y()) < kPointEpsilon;
            if (!restates) flush();
            at = dest;
            // A restatement adds no geometry, so it must not add a point
            // either. Appending one would desynchronise the point list from
            // the segment list -- the duplicate would then be read as a
            // segment's first point, shifting every control point after it and
            // truncating the contour.
            if (!restates) cur.points.append(at);
            ++out.moveSegments;
            i += 3;
        } else if (op == '|') {
            if (i + 2 >= raw.size() || raw.at(i + 1).op != 'n' ||
                raw.at(i + 2).op != 'n') {
                error = QObject::tr("lineTo without two coordinates");
                return false;
            }
            // A line. The tokenizer has already mapped '/' here, because '/'
            // is a lineTo and this format has no close-path opcode: treating it
            // as one is a common and wrong reading that yields a
            // differently-shaped path.
            at = QPointF(raw.at(i + 1).v, raw.at(i + 2).v);
            if (cur.points.isEmpty()) {
                // A line before any move: start a contour here rather than
                // emitting a segment from an invented origin.
                cur.points.append(QPointF(0, 0));
            }
            cur.points.append(at);
            cur.segments.append(Contour::Kind::Line);
            ++out.lineSegments;
            i += 3;
        } else if (op == '[') {
            if (i + 4 >= raw.size()) {
                error = QObject::tr("truncated quadratic");
                return false;
            }
            const QPointF ctrl(raw.at(i + 1).v, raw.at(i + 2).v);
            const QPointF end(raw.at(i + 3).v, raw.at(i + 4).v);
            if (cur.points.isEmpty()) cur.points.append(QPointF(0, 0));
            // Keep the control point so a renderer can use the real quadratic;
            // consumers that only want the hull can drop it.
            cur.points.append(ctrl);
            cur.points.append(end);
            cur.segments.append(Contour::Kind::Quad);
            at = end;
            ++out.quadSegments;
            i += 5;
        } else {
            error = QObject::tr("unexpected token in edge data");
            return false;
        }
    }
    flush();
    return true;
}

bool decodeShapeXml(const QString &xml, Shape &out, QString &error) {
    out = Shape();
    error.clear();

    // A <DOMShape>'s inner content is a sequence of sibling elements (<edges>,
    // <fills>, <strokes>), which is not a well-formed document. QXmlStreamReader
    // rejects that, so wrap the fragment in a synthetic root -- the decoder only
    // looks for <Edge> descendants, so the wrapper is invisible to it.
    QXmlStreamReader xr(QStringLiteral("<shape>") + xml + QStringLiteral("</shape>"));
    while (!xr.atEnd()) {
        xr.readNext();
        if (!xr.isStartElement()) continue;
        if (xr.name() != QLatin1String("Edge")) continue;

        const QXmlStreamAttributes a = xr.attributes();
        const QString edges = a.value(QLatin1String("edges")).toString();
        if (edges.isEmpty()) {
            // A <Edge> with only `cubics` and no `edges` carries no rendered
            // geometry -- it is an editor hint. Skipping it is correct, not a
            // loss: decoding `cubics` instead would produce a path that does
            // not match what Animate draws.
            continue;
        }
        Edge e;
        e.edges = edges;
        e.hasFill0 = a.hasAttribute(QLatin1String("fillStyle0"));
        e.hasFill1 = a.hasAttribute(QLatin1String("fillStyle1"));
        e.hasStroke = a.hasAttribute(QLatin1String("strokeStyle"));

        Shape part;
        if (!decodeEdges(e.edges, e.hasStroke, part, error)) {
            out = Shape();
            return false;
        }
        out.contours += part.contours;
        out.strokedContours += part.strokedContours;
        out.quadSegments += part.quadSegments;
        out.lineSegments += part.lineSegments;
        out.moveSegments += part.moveSegments;
    }
    if (xr.hasError()) {
        error = xr.errorString();
        out = Shape();
        return false;
    }
    return true;
}

// ---------------------------------------------------------------------------
// SVG output
// ---------------------------------------------------------------------------

namespace {

// Format a coordinate compactly: enough decimals to be exact at 1/1000 px, and
// no trailing zeros.
QString num(double v) {
    QString s = QString::number(v, 'f', 3);
    while (s.contains(QLatin1Char('.')) &&
           (s.endsWith(QLatin1Char('0')) || s.endsWith(QLatin1Char('.'))))
        s.chop(1);
    return s;
}

}  // namespace

QString toSvgPath(const Shape &shape) {
    QString d;
    for (const Contour &c : shape.contours) {
        if (c.points.size() < 2 || c.segments.isEmpty()) continue;

        d += QLatin1Char('M') + num(c.points.at(0).x()) + QLatin1Char(' ') +
             num(c.points.at(0).y());

        // Walk the recorded segment kinds, consuming one point for a line and
        // two for a quadratic. Stepping by two unconditionally would drop
        // every point after a straight segment and produce a short path that
        // still looks like a shape.
        int p = 1;
        for (const Contour::Kind k : c.segments) {
            if (k == Contour::Kind::Line) {
                if (p >= c.points.size()) break;
                const QPointF &e = c.points.at(p);
                d += QLatin1Char('L') + num(e.x()) + QLatin1Char(' ') + num(e.y());
                p += 1;
            } else {
                if (p + 1 >= c.points.size()) break;
                const QPointF &ctrl = c.points.at(p);
                const QPointF &e = c.points.at(p + 1);
                d += QLatin1Char('Q') + num(ctrl.x()) + QLatin1Char(' ') +
                     num(ctrl.y()) + QLatin1Char(' ') + num(e.x()) +
                     QLatin1Char(' ') + num(e.y());
                p += 2;
            }
        }

        // Closure is implicit in this format, so state it explicitly: a filled
        // contour always closes, and a stroked one closes when its last point
        // returns to its first.
        if (c.closed) d += QLatin1Char('Z');
    }
    return d;
}

QString toSvgDocument(const Shape &shape, const QString &title) {
    if (shape.contours.isEmpty()) {
        // An empty shape is legitimate -- it draws nothing. Emitting a valid
        // empty document is more useful to a consumer than emitting nothing.
        return QStringLiteral(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"0\" height=\"0\""
            " viewBox=\"0 0 0 0\"/>\n");
    }

    const QPointF lo = shape.minBound();
    const QPointF hi = shape.maxBound();
    // A small margin so a stroke is not clipped at the edge.
    const double margin = 2.0;
    const double w = qMax(0.0, hi.x() - lo.x()) + 2 * margin;
    const double h = qMax(0.0, hi.y() - lo.y()) + 2 * margin;

    QString svg;
    svg += QStringLiteral("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    svg += QStringLiteral(
               "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"%1\" "
               "height=\"%2\" viewBox=\"%3 %4 %1 %2\">\n")
               .arg(num(w), num(h), num(lo.x() - margin), num(lo.y() - margin));
    if (!title.isEmpty())
        svg += QStringLiteral("  <title>%1</title>\n").arg(title.toHtmlEscaped());
    svg += QStringLiteral(
        "  <!-- %1 contour(s), %2 quadratic, %3 line, %4 stroked. "
        "Coordinates are absolute in twips/20; closure is implicit in the "
        "source format. -->\n")
        .arg(shape.contours.size())
        .arg(shape.quadSegments)
        .arg(shape.lineSegments)
        .arg(shape.strokedContours);
    // A black fill is the default and matches Flash's rendering of a shape
    // whose fill is set but whose colour is not carried in the geometry.
    svg += QStringLiteral("  <path d=\"%1\" fill=\"#000000\"/>\n").arg(toSvgPath(shape));
    svg += QStringLiteral("</svg>\n");
    return svg;
}

}  // namespace XFL
