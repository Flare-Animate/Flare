// Decode one DefineShape body and report what came out.
//
// usage: swfshape_probe <body.bin> <version 1-4>
//
// Reads the tag body exactly as it appears in the tag stream, which is what
// SWF::decodeShape expects. Prints the declared bounds, the record and segment
// counts, and the outline, so a wrong decode is visible as a number rather than
// as an absence.
#include "SWFShape.h"

#include <QCoreApplication>
#include <QFile>
#include <QFileInfo>
#include <cstdio>

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

  const SWF::Shape s =
      SWF::decodeShape(body, version);

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
  printf("records   : %d\n", s.records);
  printf("contours  : %d\n", int(s.contours.size()));
  printf("segments  : %d\n", s.segmentCount());

  for (int i = 0; i < s.contours.size(); ++i) {
    const SWF::Contour &c = s.contours.at(i);
    printf("  contour %d: start (%g,%g), %d segment(s)\n", i, c.start.x(),
           c.start.y(), int(c.segments.size()));
    int curves = 0;
    for (const SWF::Segment &sg : c.segments)
      if (sg.curve) ++curves;
    printf("            %d straight, %d curved\n",
           int(c.segments.size()) - curves, curves);
  }

  const QString path = SWF::toSvgPath(s);
  if (path.isEmpty()) {
    printf("svg       : (empty)\n");
    return 0;
  }
  // The outline must lie inside the declared bounds. A decode that has lost sync
  // with the bit stream still produces points -- just in the wrong place -- so
  // this is the check that separates "read the records" from "read them right".
  double minX = 0, minY = 0, maxX = 0, maxY = 0;
  bool first = true;
  bool outside = false;
  for (const SWF::Contour &c : s.contours) {
    const QPointF pts[1] = {c.start};
    for (int k = 0; k < 1; ++k) {
      if (first) { minX = maxX = pts[k].x(); minY = maxY = pts[k].y();
                   first = false; }
      minX = qMin(minX, pts[k].x()); maxX = qMax(maxX, pts[k].x());
      minY = qMin(minY, pts[k].y()); maxY = qMax(maxY, pts[k].y());
    }
    for (const SWF::Segment &sg : c.segments) {
      const QPointF pts2[2] = {sg.curve ? sg.control : sg.end, sg.end};
      for (const QPointF &p : pts2) {
        minX = qMin(minX, p.x()); maxX = qMax(maxX, p.x());
        minY = qMin(minY, p.y()); maxY = qMax(maxY, p.y());
      }
    }
  }
  const QPointF lo = s.hasEdgeBounds ? s.edgeMin : s.boundsMin;
  const QPointF hi = s.hasEdgeBounds ? s.edgeMax : s.boundsMax;
  // EdgeBounds include the pen width of a stroke, so allow a little slack.
  const double slack = 2.0;
  if (minX < lo.x() - slack || maxX > hi.x() + slack ||
      minY < lo.y() - slack || maxY > hi.y() + slack) {
    outside = true;
    printf("CHECK     : outline (%g,%g)..(%g,%g) is OUTSIDE the declared "
           "bounds (%g,%g)..(%g,%g)\n",
           minX, minY, maxX, maxY, lo.x(), lo.y(), hi.x(), hi.y());
  } else {
    printf("CHECK     : outline lies inside the declared bounds\n");
  }
  printf("extent    : (%g,%g) .. (%g,%g)\n", minX, minY, maxX, maxY);
  printf("svg       : %s\n", qPrintable(path.left(400)));
  return outside ? 3 : 0;
}
