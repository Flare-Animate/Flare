// Decode one DefineShape body and report what came out.
//
// usage: swfshape_probe <body.bin> <version 1-4>
//
// Reads the tag body exactly as it appears in the tag stream, which is what
// SWF::decodeShape expects. Prints the declared bounds, the record and segment
// counts, and the whole outline vertex by vertex, so a wrong decode is visible as a
// number rather than as an absence.
//
// Every segment is printed, not a summary of them and not a truncated path. Both of
// those were wrong here: the SVG path was cut at 400 characters, so for any real
// shape -- a 3.5 MB SWF has outlines of several thousand characters -- the probe
// silently reported a fraction of the geometry while looking as though it had
// reported all of it. The 400-character cut also hid the outline from the geometry
// cross-check against an independent decoder, which silently compared prefixes.
#include "SWFShape.h"

#include <QCoreApplication>
#include <QFile>
#include <QFileInfo>
#include <cmath>
#include <cstdio>

namespace {

// The extremes of a quadratic Bezier, per axis.
//
// A curve's control point can sit well outside the declared bounds and the outline
// can still be entirely inside them, because the curve is bounded by the convex hull
// of its three points and does not reach the control one. Counting the control point
// as if the outline passed through it therefore reports "outside the bounds" for
// perfectly good decodes -- 123 of 250 real shapes here.
//
// f(t) = (1-t)^2 p0 + 2(1-t)t c + t^2 p2, so f'(t) = 0 at
// t = (c-p0) / ((c-p0) + (p2-c)). The extremes are the endpoints plus that, when it
// falls strictly inside (0, 1).
void curveExtremes(double p0, double c, double p2, double *lo, double *hi) {
  *lo = qMin(p0, p2);
  *hi = qMax(p0, p2);
  const double a = c - p0;
  const double b = p2 - c;
  const double denom = a + b;
  if (denom == 0.0) return;
  const double t = a / denom;
  if (!(t > 0.0 && t < 1.0)) return;
  const double u = 1.0 - t;
  const double v = u * u * p0 + 2.0 * u * t * c + t * t * p2;
  *lo = qMin(*lo, v);
  *hi = qMax(*hi, v);
}

}  // namespace

int main(int argc, char **argv) {
  QCoreApplication app(argc, argv);
  if (argc < 3) {
    fprintf(stderr, "usage: swfshape_probe <body.bin> <version>\n");
    return 2;
  }
  QFile f(QString::fromLocal8Bit(argv[1]));
  if (!f.open(QIODevice::ReadOnly)) {
    fprintf(stderr, "cannot open %s\n", argv[1]);
    return 1;
  }
  const QByteArray body = f.readAll();
  const int version = QString::fromLocal8Bit(argv[2]).toInt();

  const SWF::Shape s = SWF::decodeShape(body, version);

  printf("file      : %s (%lld bytes)  version %d\n",
         qPrintable(QFileInfo(argv[1]).fileName()), (long long)body.size(),
         version);
  printf("ok        : %d%s%s\n", int(s.ok),
         s.error.isEmpty() ? "" : "  error: ", qPrintable(s.error));
  printf("id        : %d\n", s.id);
  printf("bounds    : (%g,%g) .. (%g,%g)\n", s.boundsMin.x(), s.boundsMin.y(),
         s.boundsMax.x(), s.boundsMax.y());
  if (s.hasEdgeBounds)
    printf("edgeBounds: (%g,%g) .. (%g,%g)\n", s.edgeMin.x(), s.edgeMin.y(),
           s.edgeMax.x(), s.edgeMax.y());
  printf("styles    : %d fill, %d line\n", s.numFillStyles, s.numLineStyles);
  printf("widths    : NumFillBits=%d NumLineBits=%d\n", s.numFillBits,
         s.numLineBits);
  printf("records   : %d\n", s.records);
  printf("contours  : %d\n", int(s.contours.size()));
  printf("segments  : %d\n", s.segmentCount());

  // The outline must lie inside the declared bounds. A decode that has lost sync
  // with the bit stream still produces points -- just in the wrong place -- so this
  // is the check that separates "read the records" from "read them right".
  //
  // ShapeBounds is the bounds *including* any stroke; EdgeBounds, which only
  // DefineShape4 carries, excludes it. The outline is inside EdgeBounds and
  // EdgeBounds is inside ShapeBounds, so EdgeBounds is the tighter of the two and
  // is what this checks when it is available. For the earlier versions the two are
  // one value.
  double minX = 0, minY = 0, maxX = 0, maxY = 0;
  bool first = true;
  const auto see = [&](double x, double y) {
    if (first) {
      minX = maxX = x;
      minY = maxY = y;
      first = false;
      return;
    }
    minX = qMin(minX, x);
    maxX = qMax(maxX, x);
    minY = qMin(minY, y);
    maxY = qMax(maxY, y);
  };

  for (int i = 0; i < s.contours.size(); ++i) {
    const SWF::Contour &c = s.contours.at(i);
    printf("  contour %d: start (%g,%g), %d segment(s)\n", i, c.start.x(),
           c.start.y(), int(c.segments.size()));
    see(c.start.x(), c.start.y());
    for (int k = 0; k < c.segments.size(); ++k) {
      const SWF::Segment &sg = c.segments.at(k);
      const QPointF from = k == 0 ? c.start : c.segments.at(k - 1).end;
      if (sg.curve) {
        printf("    seg %d: curve control (%g,%g) from (%g,%g) to (%g,%g)\n", k,
               sg.control.x(), sg.control.y(), from.x(), from.y(), sg.end.x(),
               sg.end.y());
        double lo, hi;
        curveExtremes(from.x(), sg.control.x(), sg.end.x(), &lo, &hi);
        if (!first) {
          minX = qMin(minX, lo);
          maxX = qMax(maxX, hi);
        }
        curveExtremes(from.y(), sg.control.y(), sg.end.y(), &lo, &hi);
        if (!first) {
          minY = qMin(minY, lo);
          maxY = qMax(maxY, hi);
        }
      } else {
        printf("    seg %d: line from (%g,%g) to (%g,%g)\n", k, from.x(),
               from.y(), sg.end.x(), sg.end.y());
      }
      see(sg.end.x(), sg.end.y());
    }
  }

  const QString path = SWF::toSvgPath(s);
  printf("svg       : %s\n",
         path.isEmpty() ? "(empty)" : qPrintable(path.toUtf8().constData()));

  if (s.contours.isEmpty()) return 0;

  const QPointF lo = s.hasEdgeBounds ? s.edgeMin : s.boundsMin;
  const QPointF hi = s.hasEdgeBounds ? s.edgeMax : s.boundsMax;
  // A stroke's width may exceed the declared bounds on a shape whose edges are
  // inside them, so a small tolerance avoids calling that a decode fault. It is
  // one twip-fraction, not a fudge factor: the real failures are thousands out.
  const double slack = 2.0;
  if (minX < lo.x() - slack || maxX > hi.x() + slack ||
      minY < lo.y() - slack || maxY > hi.y() + slack) {
    printf("CHECK     : outline (%g,%g)..(%g,%g) is OUTSIDE the declared "
           "bounds (%g,%g)..(%g,%g)\n",
           minX, minY, maxX, maxY, lo.x(), lo.y(), hi.x(), hi.y());
    return 3;
  }
  printf("CHECK     : outline lies inside the declared bounds\n");
  printf("extent    : (%g,%g) .. (%g,%g)\n", minX, minY, maxX, maxY);
  return 0;
}
