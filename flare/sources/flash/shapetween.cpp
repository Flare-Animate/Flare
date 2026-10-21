#include "shapetween.h"

#include <QHash>
#include <QSet>
#include <QXmlStreamReader>

#include <algorithm>
#include <cmath>
#include <limits>
#include <map>
#include <utility>

namespace Flash {
namespace {

// Bounds the work for one tween; MorphShapes Animate saves have at most a few
// hundred curves.
constexpr int maxMorphCurves = 1000000;
// Source pixels. Animate's saved correspondence lies on the keyframe outlines
// within a fraction of a pixel (at most 1.9 px in Animate documents);
// farther means the MorphShape does not describe these keyframes.
constexpr double morphTolerance = 3.0;

Point add(Point a, Point b) { return {a.x + b.x, a.y + b.y}; }
Point sub(Point a, Point b) { return {a.x - b.x, a.y - b.y}; }
Point scale(Point a, double s) { return {a.x * s, a.y * s}; }
Point mix(Point a, Point b, double t) {
  return {a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t};
}
double dot(Point a, Point b) { return a.x * b.x + a.y * b.y; }
double distance(Point a, Point b) { return std::hypot(a.x - b.x, a.y - b.y); }
Point midpoint(Point a, Point b) { return mix(a, b, 0.5); }

Point pointAt(const Segment &s, double u) {
  return mix(mix(s.start, s.control, u), mix(s.control, s.end, u), u);
}
Point tangentAt(const Segment &s, double u) {
  return add(scale(sub(s.control, s.start), 2 * (1 - u)),
             scale(sub(s.end, s.control), 2 * u));
}
// Rough length: between the chord and the control polygon.
double roughLength(const Segment &s) {
  return (distance(s.start, s.end) + distance(s.start, s.control) +
          distance(s.control, s.end)) /
         2;
}
bool degenerate(const Segment &s) {
  return s.start == s.end && s.control == s.start;
}

Segment line(Point a, Point b) {
  Segment s;
  s.start   = a;
  s.end     = b;
  s.control = midpoint(a, b);
  return s;
}
Segment curve(Point a, Point control, Point b) {
  Segment s;
  s.start     = a;
  s.control   = control;
  s.end       = b;
  s.quadratic = true;
  return s;
}
Segment transformed(const Segment &s, const Affine &m) {
  Segment r = s;
  r.start   = m.apply(s.start);
  r.control = m.apply(s.control);
  r.end     = m.apply(s.end);
  return r;
}
Segment reversed(const Segment &s) {
  Segment r = s;
  std::swap(r.start, r.end);
  return r;
}
QVector<Segment> reversed(const QVector<Segment> &path) {
  QVector<Segment> r;
  r.reserve(path.size());
  for (int i = path.size() - 1; i >= 0; --i) r.push_back(reversed(path[i]));
  return r;
}
// The part of s between parameters u0 and u1 (quadratic blossoms).
Segment part(const Segment &s, double u0, double u1) {
  auto blossom = [&](double u, double v) {
    const double w0 = (1 - u) * (1 - v), w1 = (1 - u) * v + u * (1 - v),
                 w2 = u * v;
    return Point{w0 * s.start.x + w1 * s.control.x + w2 * s.end.x,
                 w0 * s.start.y + w1 * s.control.y + w2 * s.end.y};
  };
  Segment r = s;
  r.start   = u0 == 0 ? s.start : blossom(u0, u0);
  r.control = blossom(u0, u1);
  r.end     = u1 == 1 ? s.end : blossom(u1, u1);
  return r;
}

std::optional<Affine> inverse(const Affine &m) {
  const double det = m.determinant();
  const double size =
      std::max({std::abs(m.a), std::abs(m.b), std::abs(m.c), std::abs(m.d)});
  if (!std::isfinite(det) || size == 0 || std::abs(det) <= 1e-12 * size * size)
    return std::nullopt;
  Affine r;
  r.a  = m.d / det;
  r.b  = -m.b / det;
  r.c  = -m.c / det;
  r.d  = m.a / det;
  r.tx = -(r.a * m.tx + r.c * m.ty);
  r.ty = -(r.b * m.tx + r.d * m.ty);
  return r;
}
Affine mixMatrix(const Affine &a, const Affine &b, double t) {
  return {a.a + (b.a - a.a) * t,    a.b + (b.b - a.b) * t,
          a.c + (b.c - a.c) * t,    a.d + (b.d - a.d) * t,
          a.tx + (b.tx - a.tx) * t, a.ty + (b.ty - a.ty) * t};
}

// Nearest-point queries against sampled segments.
class NearestIndex {
public:
  struct Sample {
    Point point;
    int item;
    double u;
  };
  explicit NearestIndex(double cell = 2.0) : m_cell(cell) {}
  void add(const Segment &s, int item, double step = 0.5) {
    const int n =
        std::max(1, std::min(256, int(std::ceil(roughLength(s) / step))));
    for (int k = 0; k <= n; ++k) {
      const double u = double(k) / n;
      const Point p  = pointAt(s, u);
      if (!std::isfinite(p.x) || !std::isfinite(p.y)) continue;
      m_cells[key(cellOf(p.x), cellOf(p.y))].push_back(m_samples.size());
      m_samples.push_back({p, item, u});
    }
  }
  // The nearest sample within radius, or nullptr.
  const Sample *nearest(Point p, double radius) const {
    const qint64 cx = cellOf(p.x), cy = cellOf(p.y);
    const qint64 reach  = qint64(std::ceil(radius / m_cell));
    const Sample *best  = nullptr;
    double bestDistance = radius;
    for (qint64 x = cx - reach; x <= cx + reach; ++x)
      for (qint64 y = cy - reach; y <= cy + reach; ++y) {
        const auto cell = m_cells.constFind(key(x, y));
        if (cell == m_cells.cend()) continue;
        for (int index : cell.value()) {
          const double d = distance(m_samples[index].point, p);
          if (d <= bestDistance) {
            bestDistance = d;
            best         = &m_samples[index];
          }
        }
      }
    return best;
  }

private:
  qint64 cellOf(double v) const {
    const double c = std::floor(v / m_cell);
    return qint64(std::max(-1e15, std::min(1e15, c)));
  }
  static qint64 key(qint64 x, qint64 y) {
    return (x * 73856093) ^ (y * 19349663);
  }
  double m_cell;
  QHash<qint64, QVector<int>> m_cells;
  QVector<Sample> m_samples;
};

// The shape elements of a keyframe, with their drawable segments in layer
// space.
struct Keyframe {
  struct Drawn {
    int shape;
    Segment segment;
    int fill0, fill1, stroke;
  };
  QVector<int> elementSlots;  // element index of each shape ordinal
  QVector<Shape> shapes;
  QVector<Affine> matrices;
  QVector<Drawn> drawn;
};

Keyframe keyframe(const Frame &frame) {
  Keyframe result;
  for (int slot = 0; slot < frame.elements.size(); ++slot) {
    const auto &element = frame.elements[slot];
    if (element.kind != Element::Kind::Shape || !element.shape) continue;
    const int shape = result.shapes.size();
    result.elementSlots.push_back(slot);
    result.shapes.push_back(*element.shape);
    result.matrices.push_back(element.matrix);
    for (const auto &edge : element.shape->edges) {
      const bool boundary = edge.fill0 != edge.fill1 || edge.stroke;
      if (!boundary) continue;
      for (const auto &segment : edge.segments)
        if (!degenerate(segment))
          result.drawn.push_back({shape, transformed(segment, element.matrix),
                                  edge.fill0, edge.fill1, edge.stroke});
    }
  }
  return result;
}

// Paints without geometry: in-betweens only read styles.
Shape paintsOnly(const Shape &shape) {
  Shape result;
  result.fills   = shape.fills;
  result.strokes = shape.strokes;
  return result;
}

bool gradient(const Paint &paint) {
  return paint.kind == Paint::Kind::LinearGradient ||
         paint.kind == Paint::Kind::RadialGradient;
}

Color mixColor(const Color &a, const Color &b, double t) {
  auto channel = [t](unsigned char x, unsigned char y) {
    return (unsigned char)std::lround(
        std::max(0.0, std::min(255.0, x + (double(y) - x) * t)));
  };
  return {channel(a.red, b.red), channel(a.green, b.green),
          channel(a.blue, b.blue), channel(a.alpha, b.alpha)};
}

QString hexColor(const Color &color) {
  return QString("#%1%2%3")
      .arg(int(color.red), 2, 16, QChar('0'))
      .arg(int(color.green), 2, 16, QChar('0'))
      .arg(int(color.blue), 2, 16, QChar('0'))
      .toUpper();
}

// A solid paint seen as the other paint's gradient, as Animate morphs a
// solid fill into a gradient.
Paint asGradient(const Paint &solid, const Paint &shape) {
  Paint result = shape;
  for (auto &stop : result.stops) stop.color = solid.color;
  return result;
}

// Whether two paints interpolate continuously.
bool interpolates(const Paint &a, const Paint &b) {
  if (a.kind == Paint::Kind::Solid && b.kind == Paint::Kind::Solid) return true;
  if (gradient(a) && gradient(b))
    return a.kind == b.kind && a.stops.size() == b.stops.size();
  if (gradient(a) && b.kind == Paint::Kind::Solid) return true;
  if (gradient(b) && a.kind == Paint::Kind::Solid) return true;
  return false;
}

// Interpolates paints given in their shapes' local spaces (matrices ma, mb)
// into the space toLocal maps layer coordinates to.
Paint mixPaint(Paint a, Affine ma, Paint b, Affine mb, double t,
               const Affine &toLocal) {
  if (!interpolates(a, b)) {
    // Switches at the middle; a paint field moves into the target space.
    Paint result  = t < 0.5 ? a : b;
    result.matrix = toLocal * (t < 0.5 ? ma : mb) * result.matrix;
    return result;
  }
  if (a.kind == Paint::Kind::Solid && b.kind == Paint::Kind::Solid) {
    a.color = mixColor(a.color, b.color, t);
    return a;
  }
  if (a.kind == Paint::Kind::Solid) {
    a  = asGradient(a, b);
    ma = mb;
  }
  if (b.kind == Paint::Kind::Solid) {
    b  = asGradient(b, a);
    mb = ma;
  }
  Paint result  = a;
  result.matrix = toLocal * mixMatrix(ma * a.matrix, mb * b.matrix, t);
  for (int i = 0; i < result.stops.size(); ++i) {
    auto &stop = result.stops[i];
    stop.ratio = a.stops[i].ratio + (b.stops[i].ratio - a.stops[i].ratio) * t;
    stop.color = mixColor(a.stops[i].color, b.stops[i].color, t);
    // Keep the validated lexical fields in step with the values.
    stop.properties.insert("ratio", QString::number(stop.ratio, 'g', 17));
    stop.properties.insert("color", hexColor(stop.color));
    stop.properties.insert("alpha",
                           QString::number(stop.color.alpha / 255.0, 'g', 17));
  }
  if (a.properties.contains("focalPointRatio") ||
      b.properties.contains("focalPointRatio")) {
    const double fa = a.properties.value("focalPointRatio", "0").toDouble();
    const double fb = b.properties.value("focalPointRatio", "0").toDouble();
    result.properties.insert("focalPointRatio",
                             QString::number(fa + (fb - fa) * t, 'g', 17));
  }
  return result;
}

Stroke mixStroke(const Stroke &a, const Affine &ma, const Stroke &b,
                 const Affine &mb, double t, const Affine &toLocal) {
  if (a.type != b.type) return t < 0.5 ? a : b;
  Stroke result = a;
  result.weight = a.weight + (b.weight - a.weight) * t;
  result.paint  = mixPaint(a.paint, ma, b.paint, mb, t, toLocal);
  return result;
}

// A path of connected segments of one style, used by the fallback matching.
struct Contour {
  bool filled = false;  // fill on the right
  int shape   = -1;
  int style   = 0;
  QVector<Segment> segments;
  bool closed   = false;
  double length = 0;
  double area   = 0;  // signed, downward Y
  Point centroid;
  // Cumulative arc length at each segment start, and per segment samples.
  QVector<double> starts;
  QVector<QVector<double>> table;  // arc length at u = k / samples
};

constexpr int tableSamples = 16;

void measure(Contour &contour) {
  contour.length = 0;
  contour.area   = 0;
  contour.starts.clear();
  contour.table.clear();
  Point sum;
  double weight = 0;
  for (const auto &segment : contour.segments) {
    contour.starts.push_back(contour.length);
    QVector<double> table{0};
    Point previous = segment.start;
    double length  = 0;
    for (int k = 1; k <= tableSamples; ++k) {
      const Point p  = pointAt(segment, double(k) / tableSamples);
      const double d = distance(previous, p);
      length += d;
      table.push_back(length);
      sum = add(sum, scale(midpoint(previous, p), d));
      weight += d;
      contour.area += previous.x * p.y - p.x * previous.y;
      previous = p;
    }
    contour.table.push_back(table);
    contour.length += length;
  }
  contour.area /= 2;
  contour.centroid = weight > 0 ? scale(sum, 1 / weight)
                     : contour.segments.isEmpty()
                         ? Point{}
                         : contour.segments.front().start;
}

// Segment index and parameter at arc length s along the contour.
std::pair<int, double> locate(const Contour &contour, double s) {
  const int count = contour.segments.size();
  int index =
      int(std::upper_bound(contour.starts.cbegin(), contour.starts.cend(), s) -
          contour.starts.cbegin()) -
      1;
  index              = std::max(0, std::min(count - 1, index));
  const auto &table  = contour.table[index];
  const double local = s - contour.starts[index];
  if (table.back() <= 0) return {index, 0};
  if (local <= 0) return {index, 0};
  if (local >= table.back()) return {index, 1};
  const int k = int(std::upper_bound(table.cbegin(), table.cend(), local) -
                    table.cbegin()) -
                1;
  const double span = table[k + 1] - table[k];
  const double f    = span > 0 ? (local - table[k]) / span : 0;
  return {index, (k + f) / tableSamples};
}

Point contourPoint(const Contour &contour, double s) {
  const auto at = locate(contour, s);
  return pointAt(contour.segments[at.first], at.second);
}

// Chains segments into paths by matching end and start points.
QVector<QVector<Segment>> chain(const QVector<Segment> &segments,
                                QVector<bool> *closedPaths) {
  auto keyOf = [](Point p) {
    return qMakePair(qint64(std::llround(p.x * 4096)),
                     qint64(std::llround(p.y * 4096)));
  };
  QMultiHash<QPair<qint64, qint64>, int> byStart;
  for (int i = segments.size() - 1; i >= 0; --i)
    byStart.insert(keyOf(segments[i].start), i);
  QVector<bool> used(segments.size(), false);
  QVector<QVector<Segment>> paths;
  for (int first = 0; first < segments.size(); ++first) {
    if (used[first]) continue;
    QVector<Segment> path{segments[first]};
    used[first]     = true;
    const auto home = keyOf(segments[first].start);
    auto at         = keyOf(segments[first].end);
    while (at != home) {
      int next = -1;
      for (auto it = byStart.constFind(at);
           it != byStart.cend() && it.key() == at; ++it)
        if (!used[it.value()]) {
          next = it.value();
          break;
        }
      if (next < 0) break;
      used[next] = true;
      path.push_back(segments[next]);
      at = keyOf(segments[next].end);
    }
    closedPaths->push_back(at == home);
    paths.push_back(path);
  }
  return paths;
}

// Fill loops (fill on the right) and stroke paths of one keyframe shape.
QVector<Contour> contours(const Keyframe &keyframe, int shape) {
  QVector<Contour> result;
  const Shape &source  = keyframe.shapes[shape];
  const Affine &matrix = keyframe.matrices[shape];
  auto addPaths        = [&](const QVector<Segment> &segments, bool filled,
                      int style) {
    QVector<bool> closed;
    const auto paths = chain(segments, &closed);
    for (int i = 0; i < paths.size(); ++i) {
      Contour contour;
      contour.filled   = filled;
      contour.shape    = shape;
      contour.style    = style;
      contour.segments = paths[i];
      contour.closed   = closed[i];
      for (int k = 0; k < contour.segments.size(); ++k)
        contour.segments[k].startsContour = k == 0;
      measure(contour);
      if (contour.length > 0) result.push_back(contour);
    }
  };
  for (auto it = source.fills.cbegin(); it != source.fills.cend(); ++it) {
    QVector<Segment> directed;
    for (const auto &edge : source.edges) {
      if (edge.fill0 == edge.fill1) continue;
      for (const auto &segment : edge.segments) {
        if (degenerate(segment)) continue;
        const Segment placed = transformed(segment, matrix);
        if (edge.fill1 == it.key()) directed.push_back(placed);
      }
      if (edge.fill0 == it.key())
        for (int k = edge.segments.size() - 1; k >= 0; --k)
          if (!degenerate(edge.segments[k]))
            directed.push_back(reversed(transformed(edge.segments[k], matrix)));
    }
    addPaths(directed, true, it.key());
  }
  for (auto it = source.strokes.cbegin(); it != source.strokes.cend(); ++it) {
    QVector<Segment> drawn;
    for (const auto &edge : source.edges)
      if (edge.stroke == it.key())
        for (const auto &segment : edge.segments)
          if (!degenerate(segment))
            drawn.push_back(transformed(segment, matrix));
    addPaths(drawn, false, it.key());
  }
  return result;
}

// Starts a closed contour at arc length s.
Contour rotated(const Contour &contour, double s) {
  if (!contour.closed || contour.segments.isEmpty()) return contour;
  const auto at  = locate(contour, s);
  Contour result = contour;
  result.segments.clear();
  const Segment &cut = contour.segments[at.first];
  const int count    = contour.segments.size();
  if (at.second < 1) result.segments.push_back(part(cut, at.second, 1));
  for (int k = 1; k < count; ++k)
    result.segments.push_back(contour.segments[(at.first + k) % count]);
  if (at.second > 0) result.segments.push_back(part(cut, 0, at.second));
  // Close exactly at the cut point.
  result.segments.back().end = result.segments.front().start;
  for (int k = 0; k < result.segments.size(); ++k)
    result.segments[k].startsContour = k == 0;
  measure(result);
  return result;
}

Contour reversedContour(const Contour &contour) {
  Contour result  = contour;
  result.segments = reversed(contour.segments);
  for (int k = 0; k < result.segments.size(); ++k)
    result.segments[k].startsContour = k == 0;
  measure(result);
  return result;
}

// Splits both contours at the union of their normalized segment boundaries,
// so that the pieces correspond one to one and each keyframe keeps its exact
// geometry.
void resample(const Contour &a, const Contour &b, QVector<Segment> &outA,
              QVector<Segment> &outB) {
  QVector<double> cuts;
  for (const Contour *c : {&a, &b})
    for (double start : c->starts)
      cuts.push_back(c->length > 0 ? start / c->length : 0);
  cuts.push_back(1);
  std::sort(cuts.begin(), cuts.end());
  QVector<double> unique;
  for (double cut : cuts)
    if (unique.isEmpty() || cut - unique.back() > 1e-9) unique.push_back(cut);
  auto piece = [](const Contour &c, double s0, double s1) {
    const double middle = (s0 + s1) / 2 * c.length;
    const int index     = locate(c, middle).first;
    auto param          = [&](double s) {
      // The parameter of arc length s within segment `index`.
      const auto &table  = c.table[index];
      const double local = s * c.length - c.starts[index];
      if (table.back() <= 0 || local <= 0) return 0.0;
      if (local >= table.back()) return 1.0;
      const int k = int(std::upper_bound(table.cbegin(), table.cend(), local) -
                        table.cbegin()) -
                    1;
      const double span = table[k + 1] - table[k];
      return (k + (span > 0 ? (local - table[k]) / span : 0)) / tableSamples;
    };
    return part(c.segments[index], param(s0), param(s1));
  };
  for (int i = 1; i < unique.size(); ++i) {
    Segment pa = piece(a, unique[i - 1], unique[i]);
    Segment pb = piece(b, unique[i - 1], unique[i]);
    // Keep the pieces connected exactly.
    if (!outA.isEmpty()) pa.start = outA.back().end;
    if (!outB.isEmpty()) pb.start = outB.back().end;
    pa.startsContour = pb.startsContour = outA.isEmpty();
    outA.push_back(pa);
    outB.push_back(pb);
  }
  if (a.closed && !outA.isEmpty()) outA.back().end = outA.front().start;
  if (b.closed && !outB.isEmpty()) outB.back().end = outB.front().start;
}

// The same contour shrunk to its centroid, for contours without a partner.
QVector<Segment> collapsed(const Contour &contour) {
  QVector<Segment> result;
  for (int k = 0; k < contour.segments.size(); ++k) {
    Segment s = curve(contour.centroid, contour.centroid, contour.centroid);
    s.startsContour = k == 0;
    result.push_back(s);
  }
  return result;
}

// The distance from p to the contour; `s` receives the nearest arc length.
double nearestOn(const Contour &contour, Point p, double &s) {
  double best       = std::numeric_limits<double>::infinity();
  const int samples = 256;
  for (int k = 0; k < samples; ++k) {
    const double at = contour.length * k / samples;
    const double d  = distance(contourPoint(contour, at), p);
    if (d < best) {
      best = d;
      s    = at;
    }
  }
  return best;
}

}  // namespace

std::optional<ShapeMorph> shapeMorph(const Frame &frame, QString &problem) {
  problem.clear();
  auto fail = [&problem](const QString &why) -> std::optional<ShapeMorph> {
    problem = why;
    return std::nullopt;
  };
  auto point = [](const QString &text, Point &result) {
    const int comma = text.indexOf(',');
    if (comma < 0) return false;
    try {
      // edgeCoordinate gives twips / 20; MorphShape counts 256 twips.
      result.x = edgeCoordinate(text.left(comma).trimmed()) * 256.0;
      result.y = edgeCoordinate(text.mid(comma + 1).trimmed()) * 256.0;
    } catch (const Error &) {
      return false;
    }
    return std::isfinite(result.x) && std::isfinite(result.y);
  };
  const Extra *morph = nullptr;
  QVector<const Extra *> hintLists;
  for (const auto &extra : frame.extras) {
    if (extra.tag == "MorphShape") {
      if (morph) return fail("The frame has more than one MorphShape.");
      morph = &extra;
    } else if (extra.tag == "morphHintsList")
      hintLists.push_back(&extra);
  }
  if (!morph) return std::nullopt;
  hintLists.push_back(morph);
  ShapeMorph result;
  int curves = 0;
  {
    QXmlStreamReader xml(morph->xml);
    int segment = -1;
    while (!xml.atEnd()) {
      xml.readNext();
      if (xml.isStartElement() && xml.name() == QLatin1String("MorphSegment")) {
        const auto attributes = xml.attributes();
        MorphSegment value;
        if (!point(attributes.value("startPointA").toString(), value.startA) ||
            !point(attributes.value("startPointB").toString(), value.startB))
          return fail("A MorphSegment has an invalid start point.");
        value.filled = attributes.hasAttribute("fillIndex1") ||
                       attributes.hasAttribute("fillIndex2");
        result.segments.push_back(value);
        segment = result.segments.size() - 1;
      } else if (xml.isStartElement() &&
                 xml.name() == QLatin1String("MorphCurves")) {
        if (segment < 0) return fail("MorphCurves outside a MorphSegment.");
        if (++curves > maxMorphCurves)
          return fail("The MorphShape exceeds the curve budget.");
        const auto attributes = xml.attributes();
        MorphCurve value;
        if (!point(attributes.value("controlPointA").toString(),
                   value.controlA) ||
            !point(attributes.value("anchorPointA").toString(),
                   value.anchorA) ||
            !point(attributes.value("controlPointB").toString(),
                   value.controlB) ||
            !point(attributes.value("anchorPointB").toString(), value.anchorB))
          return fail("A MorphCurves record has an invalid point.");
        value.line = attributes.value("isLine") == QLatin1String("true");
        result.segments[segment].curves.push_back(value);
      } else if (xml.isEndElement() &&
                 xml.name() == QLatin1String("MorphSegment"))
        segment = -1;
    }
    if (xml.hasError())
      return fail("Malformed MorphShape: " + xml.errorString());
  }
  for (const Extra *list : hintLists) {
    QXmlStreamReader xml(list->xml);
    while (!xml.atEnd()) {
      xml.readNext();
      if (!xml.isStartElement() || xml.name() != QLatin1String("MorphHint"))
        continue;
      MorphHint hint;
      if (point(xml.attributes().value("startPoint").toString(), hint.start) &&
          point(xml.attributes().value("endPoint").toString(), hint.end))
        result.hints.push_back(hint);
    }
  }
  result.segments.erase(
      std::remove_if(result.segments.begin(), result.segments.end(),
                     [](const MorphSegment &s) { return s.curves.isEmpty(); }),
      result.segments.end());
  return result;
}

bool shapeMorphExtra(const Extra &extra) {
  return extra.tag == "MorphShape" || extra.tag == "morphHintsList";
}

namespace {
// A style of the start or end keyframe shapes (see ShapeTween::StyleRef).
template <class Map>
const typename Map::mapped_type *style(const QVector<Shape> &a,
                                       const QVector<Shape> &b,
                                       const ShapeTween::StyleRef &ref,
                                       Map Shape::*member) {
  const QVector<Shape> &shapes = ref.end ? b : a;
  if (!ref.valid() || ref.shape >= shapes.size()) return nullptr;
  const auto &map  = shapes[ref.shape].*member;
  const auto found = map.constFind(ref.index);
  return found == map.cend() ? nullptr : &found.value();
}
}  // namespace

class ShapeTweenPlanner {
public:
  using Path     = ShapeTween::Path;
  using StyleRef = ShapeTween::StyleRef;

  // Builds the paths of Animate's MorphShape, each assigned to the start
  // shape whose outline it follows. Returns false when the MorphShape does
  // not describe these keyframes.
  static bool morph(const ShapeMorph &morph, const Keyframe &a,
                    const Keyframe &b, QVector<QVector<Path>> &paths,
                    QString *why) {
    // Both keyframes' outlines must lie on the saved paths.
    NearestIndex morphA, morphB;
    int item = 0;
    for (const auto &segment : morph.segments) {
      Point pa = segment.startA, pb = segment.startB;
      for (const auto &c : segment.curves) {
        morphA.add(
            c.line ? line(pa, c.anchorA) : curve(pa, c.controlA, c.anchorA),
            item);
        morphB.add(
            c.line ? line(pb, c.anchorB) : curve(pb, c.controlB, c.anchorB),
            item++);
        pa = c.anchorA;
        pb = c.anchorB;
      }
    }
    auto covered = [](const Keyframe &keyframe, const NearestIndex &index) {
      for (const auto &drawn : keyframe.drawn)
        for (double u : {0.25, 0.5, 0.75})
          if (!index.nearest(pointAt(drawn.segment, u), morphTolerance))
            return false;
      return true;
    };
    if (!covered(a, morphA) || !covered(b, morphB)) {
      *why =
          "Animate's saved morph (MorphShape) does not match the keyframe "
          "shapes";
      return false;
    }
    // Each path takes its paints from the keyframe edges it follows.
    NearestIndex fillsA, strokesA, fillsB, strokesB;
    for (int i = 0; i < a.drawn.size(); ++i) {
      if (a.drawn[i].fill0 != a.drawn[i].fill1)
        fillsA.add(a.drawn[i].segment, i);
      if (a.drawn[i].stroke) strokesA.add(a.drawn[i].segment, i);
    }
    for (int i = 0; i < b.drawn.size(); ++i) {
      if (b.drawn[i].fill0 != b.drawn[i].fill1)
        fillsB.add(b.drawn[i].segment, i);
      if (b.drawn[i].stroke) strokesB.add(b.drawn[i].segment, i);
    }
    QSet<QByteArray> seen;
    for (const auto &segment : morph.segments) {
      Path path;
      Point pa = segment.startA, pb = segment.startB;
      QByteArray identity(1, segment.filled ? 'f' : 's');
      auto append = [&identity](Point p) {
        identity.append(reinterpret_cast<const char *>(&p.x), sizeof p.x);
        identity.append(reinterpret_cast<const char *>(&p.y), sizeof p.y);
      };
      append(pa);
      append(pb);
      for (const auto &c : segment.curves) {
        Segment sa =
            c.line ? line(pa, c.anchorA) : curve(pa, c.controlA, c.anchorA);
        Segment sb =
            c.line ? line(pb, c.anchorB) : curve(pb, c.controlB, c.anchorB);
        append(sa.control);
        append(sa.end);
        append(sb.control);
        append(sb.end);
        pa = c.anchorA;
        pb = c.anchorB;
        if (degenerate(sa) && degenerate(sb)) continue;
        sa.startsContour = sb.startsContour = path.a.isEmpty();
        path.a.push_back(sa);
        path.b.push_back(sb);
      }
      // Animate lists a closed stroke once per side: draw it once.
      if (path.a.isEmpty() || seen.contains(identity)) continue;
      seen.insert(identity);
      // Vote over the followed edges for the start shape and the paints, as
      // (shape, style) on the right and on the left of the path.
      using Vote = QPair<int, int>;
      QMap<int, int> shapes;
      QMap<Vote, int> right[2], left[2];
      const int count = path.a.size();
      const int step  = std::max(1, count / 64);
      for (int k = 0; k < count; k += step)
        for (int side = 0; side < 2; ++side) {
          const Segment &s          = side ? path.b[k] : path.a[k];
          const Keyframe &frame     = side ? b : a;
          const NearestIndex &index = segment.filled
                                          ? (side ? fillsB : fillsA)
                                          : (side ? strokesB : strokesA);
          const auto *found = index.nearest(pointAt(s, 0.5), morphTolerance);
          if (!found) continue;
          const auto &drawn = frame.drawn[found->item];
          if (!side) ++shapes[drawn.shape];
          if (!segment.filled) {
            if (drawn.stroke)
              ++right[side][qMakePair(drawn.shape, drawn.stroke)];
            continue;
          }
          const bool same =
              dot(tangentAt(s, 0.5), tangentAt(drawn.segment, found->u)) >= 0;
          const int r = same ? drawn.fill1 : drawn.fill0;
          const int l = same ? drawn.fill0 : drawn.fill1;
          if (r) ++right[side][qMakePair(drawn.shape, r)];
          if (l) ++left[side][qMakePair(drawn.shape, l)];
        }
      auto winner = [](const QMap<Vote, int> &votes, int *total) {
        Vote best = qMakePair(-1, 0);
        int most = 0, sum = 0;
        for (auto it = votes.cbegin(); it != votes.cend(); ++it) {
          sum += it.value();
          if (it.value() > most) {
            best = it.key();
            most = it.value();
          }
        }
        *total = sum;
        return best;
      };
      int startShape = 0, startVotes = 0;
      for (auto it = shapes.cbegin(); it != shapes.cend(); ++it)
        if (it.value() > startVotes) {
          startShape = it.key();
          startVotes = it.value();
        }
      int rightVotes[2] = {0, 0}, leftVotes[2] = {0, 0};
      Vote styleA = winner(right[0], &rightVotes[0]);
      Vote styleB = winner(right[1], &rightVotes[1]);
      if (segment.filled) {
        const Vote leftA = winner(left[0], &leftVotes[0]);
        const Vote leftB = winner(left[1], &leftVotes[1]);
        // Animate keeps fills on the right; follow the evidence otherwise.
        if (rightVotes[0] + rightVotes[1] == 0 &&
            leftVotes[0] + leftVotes[1] > 0) {
          path.a = reversed(path.a);
          path.b = reversed(path.b);
          for (int k = 0; k < path.a.size(); ++k)
            path.a[k].startsContour = path.b[k].startsContour = k == 0;
          styleA = leftA;
          styleB = leftB;
        }
      }
      StyleRef refA{styleA.first, styleA.second, false};
      StyleRef refB{styleB.first, styleB.second, true};
      if (!refA.valid() && !refB.valid()) {
        // No paint evidence: the start shape's first style of this kind.
        const auto &shape = a.shapes[startShape];
        if (segment.filled ? shape.fills.isEmpty() : shape.strokes.isEmpty())
          continue;
        refA = {
            startShape,
            segment.filled ? shape.fills.firstKey() : shape.strokes.firstKey(),
            false};
      }
      (segment.filled ? path.fill1A : path.strokeA) = refA;
      (segment.filled ? path.fill1B : path.strokeB) = refB;
      paths[startShape].push_back(path);
    }
    return true;
  }

  static bool sameStructure(const Shape &a, const Shape &b) {
    if (a.edges.size() != b.edges.size() || a.fills.keys() != b.fills.keys() ||
        a.strokes.keys() != b.strokes.keys())
      return false;
    for (int e = 0; e < a.edges.size(); ++e) {
      const auto &ea = a.edges[e], &eb = b.edges[e];
      if (ea.fill0 != eb.fill0 || ea.fill1 != eb.fill1 ||
          ea.stroke != eb.stroke || ea.segments.size() != eb.segments.size())
        return false;
      for (int s = 0; s < ea.segments.size(); ++s)
        if (ea.segments[s].startsContour != eb.segments[s].startsContour)
          return false;
    }
    return true;
  }

  // Identical edge structure: every segment moves to its counterpart.
  static QVector<Path> structure(const Keyframe &a, int shapeA,
                                 const Keyframe &b, int shapeB) {
    QVector<Path> result;
    const Shape &sa = a.shapes[shapeA], &sb = b.shapes[shapeB];
    for (int e = 0; e < sa.edges.size(); ++e) {
      const auto &ea = sa.edges[e], &eb = sb.edges[e];
      if (ea.segments.isEmpty()) continue;
      Path path;
      for (int s = 0; s < ea.segments.size(); ++s) {
        path.a.push_back(transformed(ea.segments[s], a.matrices[shapeA]));
        path.b.push_back(transformed(eb.segments[s], b.matrices[shapeB]));
      }
      path.a.front().startsContour = path.b.front().startsContour = true;
      path.fill0A  = {shapeA, ea.fill0, false};
      path.fill0B  = {shapeB, eb.fill0, true};
      path.fill1A  = {shapeA, ea.fill1, false};
      path.fill1B  = {shapeB, eb.fill1, true};
      path.strokeA = {shapeA, ea.stroke, false};
      path.strokeB = {shapeB, eb.stroke, true};
      result.push_back(path);
    }
    return result;
  }

  // Different topology: fill loops and stroke paths are matched by kind and
  // position (shape hints first), aligned, and split at the union of their
  // arc-length breakpoints. Unmatched contours shrink to, or grow from, their
  // centroid.
  static QVector<Path> resampled(const Keyframe &a, int shapeA,
                                 const Keyframe &b, int shapeB,
                                 const QVector<MorphHint> &hints,
                                 int *matchedCount, int *unmatchedCount) {
    const QVector<Contour> ca = contours(a, shapeA), cb = contours(b, shapeB);
    double minX = std::numeric_limits<double>::infinity(), minY = minX,
           maxX = -minX, maxY = -minX;
    for (const auto *list : {&ca, &cb})
      for (const auto &c : *list)
        for (const auto &s : c.segments)
          for (const Point &p : {s.start, s.end}) {
            minX = std::min(minX, p.x);
            maxX = std::max(maxX, p.x);
            minY = std::min(minY, p.y);
            maxY = std::max(maxY, p.y);
          }
    const double diagonal =
        std::isfinite(minX)
            ? std::max(1.0, std::hypot(maxX - minX, maxY - minY))
            : 1.0;
    struct Pair {
      int a, b;
      std::optional<std::pair<double, double>> anchor;  // arc lengths
    };
    QVector<Pair> pairs;
    QVector<bool> usedA(ca.size(), false), usedB(cb.size(), false);
    for (const auto &hint : hints) {
      int bestA = -1, bestB = -1;
      double da = 4, db = 4, sa = 0, sb = 0;
      for (int i = 0; i < ca.size(); ++i) {
        double s       = 0;
        const double d = nearestOn(ca[i], hint.start, s);
        if (!usedA[i] && d < da) {
          da    = d;
          bestA = i;
          sa    = s;
        }
      }
      for (int j = 0; j < cb.size(); ++j) {
        double s       = 0;
        const double d = nearestOn(cb[j], hint.end, s);
        if (!usedB[j] && d < db) {
          db    = d;
          bestB = j;
          sb    = s;
        }
      }
      if (bestA < 0 || bestB < 0 || ca[bestA].filled != cb[bestB].filled)
        continue;
      usedA[bestA] = usedB[bestB] = true;
      pairs.push_back({bestA, bestB, std::make_pair(sa, sb)});
    }
    struct Candidate {
      double cost;
      int a, b;
    };
    std::vector<Candidate> candidates;
    for (int i = 0; i < ca.size(); ++i)
      for (int j = 0; j < cb.size(); ++j) {
        if (ca[i].filled != cb[j].filled || usedA[i] || usedB[j]) continue;
        const double size = std::abs(std::log(std::max(1e-9, ca[i].length) /
                                              std::max(1e-9, cb[j].length)));
        candidates.push_back(
            {distance(ca[i].centroid, cb[j].centroid) / diagonal + 0.25 * size +
                 (ca[i].style == cb[j].style ? 0 : 0.05),
             i, j});
      }
    std::sort(
        candidates.begin(), candidates.end(),
        [](const Candidate &x, const Candidate &y) { return x.cost < y.cost; });
    for (const auto &candidate : candidates) {
      if (usedA[candidate.a] || usedB[candidate.b]) continue;
      usedA[candidate.a] = usedB[candidate.b] = true;
      pairs.push_back({candidate.a, candidate.b, std::nullopt});
    }
    QVector<Path> result;
    auto styled = [](Path &path, bool filled, StyleRef from, StyleRef to) {
      (filled ? path.fill1A : path.strokeA) = from;
      (filled ? path.fill1B : path.strokeB) = to;
    };
    for (const auto &pair : pairs) {
      const Contour &first = ca[pair.a];
      Contour second       = cb[pair.b];
      auto anchor          = pair.anchor;
      if (first.closed && second.closed) {
        // Fill loops keep their fill on the right; strokes may run either way.
        if (!first.filled && (first.area < 0) != (second.area < 0)) {
          second = reversedContour(second);
          if (anchor) anchor->second = second.length - anchor->second;
        }
        double offset = 0;
        if (anchor) {
          offset = anchor->second - anchor->first /
                                        std::max(1e-12, first.length) *
                                        second.length;
        } else {
          constexpr int n = 64, m = 256;
          QVector<Point> pa, pb;
          for (int i = 0; i < n; ++i)
            pa.push_back(contourPoint(first, first.length * i / n));
          for (int j = 0; j < m; ++j)
            pb.push_back(contourPoint(second, second.length * j / m));
          double best = std::numeric_limits<double>::infinity();
          for (int k = 0; k < m; ++k) {
            double cost = 0;
            for (int i = 0; i < n && cost < best; ++i) {
              const Point d = sub(pa[i], pb[(i * (m / n) + k) % m]);
              cost += dot(d, d);
            }
            if (cost < best) {
              best   = cost;
              offset = second.length * k / m;
            }
          }
        }
        offset = std::fmod(offset, second.length);
        if (offset < 0) offset += second.length;
        second = rotated(second, offset);
      } else if (!first.closed && !second.closed) {
        const Point a0 = first.segments.front().start,
                    a1 = first.segments.back().end,
                    b0 = second.segments.front().start,
                    b1 = second.segments.back().end;
        if (distance(a0, b1) + distance(a1, b0) <
            distance(a0, b0) + distance(a1, b1))
          second = reversedContour(second);
      }
      Path path;
      resample(first, second, path.a, path.b);
      if (path.a.isEmpty()) continue;
      styled(path, first.filled, {shapeA, first.style, false},
             {shapeB, second.style, true});
      result.push_back(path);
    }
    int unmatched = 0;
    for (int i = 0; i < ca.size(); ++i) {
      if (usedA[i]) continue;
      ++unmatched;
      Path path;
      path.a = ca[i].segments;
      path.b = collapsed(ca[i]);
      const StyleRef ref{shapeA, ca[i].style, false};
      styled(path, ca[i].filled, ref, ref);
      result.push_back(path);
    }
    for (int j = 0; j < cb.size(); ++j) {
      if (usedB[j]) continue;
      ++unmatched;
      Path path;
      path.a = collapsed(cb[j]);
      path.b = cb[j].segments;
      const StyleRef ref{shapeB, cb[j].style, true};
      styled(path, cb[j].filled, ref, ref);
      result.push_back(path);
    }
    *matchedCount   = pairs.size();
    *unmatchedCount = unmatched;
    return result;
  }

  static ShapeTween tween(const Keyframe &a, const Keyframe &b,
                          const Affine &toLocal, QVector<Path> paths,
                          ShapeTweenMethod method) {
    ShapeTween result;
    result.method = method;
    for (const auto &s : a.shapes) result.m_shapesA.push_back(paintsOnly(s));
    for (const auto &s : b.shapes) result.m_shapesB.push_back(paintsOnly(s));
    result.m_matricesA = a.matrices;
    result.m_matricesB = b.matrices;
    result.m_toLocal   = toLocal;
    result.m_paths     = std::move(paths);
    // Paints that cannot interpolate switch at the midpoint.
    bool switched = false;
    for (const auto &path : result.m_paths) {
      const std::pair<StyleRef, StyleRef> fills[] = {
          {path.fill0A, path.fill0B}, {path.fill1A, path.fill1B}};
      for (const auto &pair : fills) {
        const Paint *pa = style(result.m_shapesA, result.m_shapesB, pair.first,
                                &Shape::fills);
        const Paint *pb = style(result.m_shapesA, result.m_shapesB, pair.second,
                                &Shape::fills);
        if (pa && pb && !interpolates(*pa, *pb)) switched = true;
      }
      const Stroke *sa = style(result.m_shapesA, result.m_shapesB, path.strokeA,
                               &Shape::strokes);
      const Stroke *sb = style(result.m_shapesA, result.m_shapesB, path.strokeB,
                               &Shape::strokes);
      if (sa && sb &&
          (sa->type != sb->type || !interpolates(sa->paint, sb->paint)))
        switched = true;
    }
    if (switched)
      result.approximations.append(
          "A paint changes kind (solid, gradient type or stop count, bitmap) "
          "or a stroke changes type; it switches at the middle of the tween.");
    return result;
  }
};

Shape ShapeTween::at(double progress) const {
  const double t = std::max(0.0, std::min(1.0, progress));
  Shape result;
  std::map<std::pair<StyleRef, StyleRef>, int> fills, strokes;
  auto matrixOf = [&](const StyleRef &ref) {
    const auto &list = ref.end ? m_matricesB : m_matricesA;
    return ref.shape >= 0 && ref.shape < list.size() ? list[ref.shape]
                                                     : Affine{};
  };
  // A side without a style keeps the other side's paint.
  auto fill = [&](const StyleRef &ra, const StyleRef &rb) {
    const Paint *pa = style(m_shapesA, m_shapesB, ra, &Shape::fills);
    const Paint *pb = style(m_shapesA, m_shapesB, rb, &Shape::fills);
    if (!pa && !pb) return 0;
    const auto key   = std::make_pair(ra, rb);
    const auto found = fills.find(key);
    if (found != fills.end()) return found->second;
    const Affine ma = matrixOf(pa ? ra : rb), mb = matrixOf(pb ? rb : ra);
    const int index = result.fills.size() + 1;
    result.fills.insert(
        index, mixPaint(pa ? *pa : *pb, ma, pb ? *pb : *pa, mb, t, m_toLocal));
    fills.emplace(key, index);
    return index;
  };
  auto stroke = [&](const StyleRef &ra, const StyleRef &rb) {
    const Stroke *sa = style(m_shapesA, m_shapesB, ra, &Shape::strokes);
    const Stroke *sb = style(m_shapesA, m_shapesB, rb, &Shape::strokes);
    if (!sa && !sb) return 0;
    const auto key   = std::make_pair(ra, rb);
    const auto found = strokes.find(key);
    if (found != strokes.end()) return found->second;
    const Affine ma = matrixOf(sa ? ra : rb), mb = matrixOf(sb ? rb : ra);
    const int index = result.strokes.size() + 1;
    result.strokes.insert(
        index, mixStroke(sa ? *sa : *sb, ma, sb ? *sb : *sa, mb, t, m_toLocal));
    strokes.emplace(key, index);
    return index;
  };
  for (const auto &path : m_paths) {
    Edge edge;
    edge.fill0  = fill(path.fill0A, path.fill0B);
    edge.fill1  = fill(path.fill1A, path.fill1B);
    edge.stroke = stroke(path.strokeA, path.strokeB);
    if (edge.fill0 == edge.fill1 && !edge.stroke) continue;
    const int count = std::min(path.a.size(), path.b.size());
    for (int k = 0; k < count; ++k) {
      const Segment &sa = path.a[k], &sb = path.b[k];
      Segment s;
      s.startsContour = k == 0 || sa.startsContour;
      // Connected source segments stay exactly connected.
      s.start     = !s.startsContour ? edge.segments.back().end
                                     : m_toLocal.apply(mix(sa.start, sb.start, t));
      s.end       = m_toLocal.apply(mix(sa.end, sb.end, t));
      s.quadratic = sa.quadratic || sb.quadratic;
      s.control = s.quadratic ? m_toLocal.apply(mix(sa.control, sb.control, t))
                              : midpoint(s.start, s.end);
      edge.segments.push_back(s);
    }
    if (!edge.segments.isEmpty()) result.edges.push_back(edge);
  }
  return result;
}

QVector<std::optional<ShapeTween>> planShapeTweens(const Frame &start,
                                                   const Frame &end,
                                                   QStringList &notes) {
  QVector<std::optional<ShapeTween>> result(start.elements.size());
  auto note        = [&notes](const QString &text) { notes.append(text); };
  const Keyframe a = keyframe(start), b = keyframe(end);
  // Without shapes on both keyframes Animate shows a broken tween, which
  // holds the start keyframe as well.
  if (a.shapes.isEmpty() || b.shapes.isEmpty()) return result;
  QVector<std::optional<Affine>> toLocal;
  for (const auto &matrix : a.matrices) toLocal.push_back(inverse(matrix));
  QString problem;
  const auto saved = shapeMorph(start, problem);
  QVector<QVector<ShapeTween::Path>> paths(a.shapes.size());
  const bool exact =
      saved && ShapeTweenPlanner::morph(*saved, a, b, paths, &problem);
  const QString reason =
      problem.isEmpty()
          ? QString("The shape tween has no saved Animate morph (MorphShape)")
          : problem;
  const QVector<MorphHint> hints = saved ? saved->hints : QVector<MorphHint>();
  for (int shape = 0; shape < a.shapes.size(); ++shape) {
    if (!toLocal[shape]) {
      note("A start shape has a singular matrix; it holds its keyframe.");
      continue;
    }
    if (exact) {
      result[a.elementSlots[shape]] = ShapeTweenPlanner::tween(
          a, b, *toLocal[shape], paths[shape], ShapeTweenMethod::Morph);
      continue;
    }
    if (shape >= b.shapes.size()) {
      note(reason +
           "; a start shape without a counterpart in the next "
           "keyframe holds its keyframe.");
      continue;
    }
    ShapeTween tween;
    if (ShapeTweenPlanner::sameStructure(a.shapes[shape], b.shapes[shape])) {
      tween = ShapeTweenPlanner::tween(
          a, b, *toLocal[shape],
          ShapeTweenPlanner::structure(a, shape, b, shape),
          ShapeTweenMethod::Structure);
      tween.approximations.prepend(
          reason +
          "; in-betweens interpolate the identical edge structure "
          "point by point.");
    } else {
      int matched = 0, unmatched = 0;
      tween = ShapeTweenPlanner::tween(
          a, b, *toLocal[shape],
          ShapeTweenPlanner::resampled(a, shape, b, shape, hints, &matched,
                                       &unmatched),
          ShapeTweenMethod::Resampled);
      tween.approximations.prepend(
          QString("%1; shape tween with unmatched topology approximated by "
                  "contour resampling (%2 matched contours%3%4).")
              .arg(reason)
              .arg(matched)
              .arg(unmatched ? QString(", %1 unmatched contours shrink or "
                                       "grow at their centre")
                                   .arg(unmatched)
                             : QString())
              .arg(hints.isEmpty() ? QString()
                                   : QString(", shape hints used")));
    }
    result[a.elementSlots[shape]] = std::move(tween);
  }
  return result;
}

}  // namespace Flash
