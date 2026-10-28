#pragma once

#include "archive.h"
#include <QVector>

namespace Flash {

struct Point {
  double x = 0;
  double y = 0;
  bool operator==(const Point &other) const {
    return x == other.x && y == other.y;
  }
};

// Model coordinates are pixels, with XFL's downward Y axis. The native scene
// adapter applies the coordinate reflection once at its boundary.
struct Segment {
  Point start;
  Point control;
  Point end;
  bool quadratic     = false;
  bool startsContour = false;
};

struct Affine {
  double a = 1, b = 0, c = 0, d = 1, tx = 0, ty = 0;
  Point apply(Point point) const;
  Affine operator*(const Affine &child) const;
  double determinant() const { return a * d - b * c; }
  void validate() const;
};

// Decimal or signed 24.8 hexadecimal twips, converted to model pixels.
double edgeCoordinate(const QString &token);
QVector<Segment> decodeEdges(const QString &data, quint64 maxSegments = 4000000,
                             const std::function<bool()> &cancelled = {});

}  // namespace Flash
