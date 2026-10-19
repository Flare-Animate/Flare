#include "fill.h"

#include <algorithm>
#include <cmath>
#include <deque>
#include <limits>
#include <map>
#include <set>
#include <utility>
#include <vector>

namespace Flash {
namespace {

// Adobe SWF 19, SHAPERECORD/StyleChangeRecord (printed pp. 128-129):
// https://open-flash.github.io/mirrors/swf-spec-19.pdf#page=130
// FillStyle0 and FillStyle1 describe opposite sides of a directed edge;
// style 0 means no fill. DefineShape4 separately specifies the winding rule
// (printed p. 133); that SWF flag is not an inferred XFL attribute/default.
// Ruffle's source converter reverses fill0, preserves fill1, and accumulates
// each paint independently, emitting fills in ascending style-ID order:
// https://github.com/ruffle-rs/ruffle/blob/master/render/src/shape_utils.rs
// (PathSegment::flip, ShapeConverter::flush_paths/flush_layer, hit_test).
// This is implementation evidence for per-paint coverage, not an Adobe paint
// precedence rule. Balanced directed edges suffice to report ray crossings,
// including holes, self-intersections and shared vertices; overlapping paints
// still require independently verified visible-color semantics.

using Real = long double;

struct PrecisePoint {
  Real x = 0;
  Real y = 0;
};

PrecisePoint precise(Point point) { return {point.x, point.y}; }

PrecisePoint interpolate(PrecisePoint a, PrecisePoint b, Real t) {
  return {(1 - t) * a.x + t * b.x, (1 - t) * a.y + t * b.y};
}

struct Curve {
  PrecisePoint start;
  PrecisePoint control;
  PrecisePoint end;
};

PrecisePoint position(const Curve &curve, Real t) {
  return interpolate(interpolate(curve.start, curve.control, t),
                     interpolate(curve.control, curve.end, t), t);
}

std::pair<Curve, Curve> split(const Curve &curve, Real t) {
  const auto a = interpolate(curve.start, curve.control, t);
  const auto b = interpolate(curve.control, curve.end, t);
  const auto p = interpolate(a, b, t);
  return {{curve.start, a, p}, {p, b, curve.end}};
}

struct Piece {
  Curve curve;
  Real minX     = 0;
  Real maxX     = 0;
  Real scale    = 1;
  int direction = 0;
};

struct PaintPath {
  QVector<Segment> segments;
  QVector<Piece> pieces;
};

using VertexKey = std::pair<double, double>;
VertexKey key(Point point) { return {point.x, point.y}; }

struct Vertex {
  quint64 incoming    = 0;
  quint64 outgoing    = 0;
  int next            = -1;
  bool closureVisited = false;
};

Real coordinateScale(const Curve &curve) {
  return std::max({Real(1), std::abs(curve.start.x), std::abs(curve.start.y),
                   std::abs(curve.control.x), std::abs(curve.control.y),
                   std::abs(curve.end.x), std::abs(curve.end.y)});
}

}  // namespace

FillError::FillError(const QString &errorCode, const QString &location,
                     const QString &message)
    : Error(QString("Source fill topology [%1]%2: %3")
                .arg(errorCode, location.isEmpty() ? QString() : " " + location,
                     message))
    , code(errorCode)
    , sourceId(location) {}

class FillEvaluator::Impl {
public:
  Impl(const Shape &shape, FillOptions options, QString sourceId)
      : m_options(std::move(options)), m_sourceId(std::move(sourceId)) {
    validateOptions();
    checkCancelled();
    for (const auto &edge : shape.edges) {
      charge();
      validatePaint(shape, edge.fill0);
      validatePaint(shape, edge.fill1);
      for (const auto &source : edge.segments) {
        charge();
        if (m_statistics.sourceSegments >= m_options.maxSourceSegments)
          fail("source-segment-limit", "Source segment budget exceeded.");
        ++m_statistics.sourceSegments;
        validatePoint(source.start);
        validatePoint(source.end);
        if (source.quadratic) validatePoint(source.control);

        // Equal style IDs have no net winding contribution, including zero.
        // A painted one can still complete an open outline of its paint.
        if (edge.fill0 == edge.fill1) {
          if (edge.fill0 && m_options.allowImplicitClosure)
            m_interior[edge.fill0].append(source);
          continue;
        }
        if (edge.fill1) appendSegment(edge.fill1, source);
        if (edge.fill0) {
          auto reverse = source;
          std::swap(reverse.start, reverse.end);
          appendSegment(edge.fill0, reverse);
        }
      }
    }

    for (auto paint = m_paths.begin(); paint != m_paths.end(); ++paint) {
      charge();
      validateGraph(paint.key(), paint.value());
      for (const auto &segment : paint.value().segments) {
        charge();
        prepare(paint.value(), segment);
      }
      // The source Shape remains immutable; only prepared curves are retained.
      paint.value().segments.clear();
      paint.value().segments.squeeze();
    }
    m_interior.clear();
    checkCancelled();
  }

  FillSample evaluate(Point point) {
    checkCancelled();
    validatePoint(point);
    const quint64 before = m_statistics.operations;
    FillSample sample;
    const auto query = precise(point);
    bool boundary    = false;
    for (auto paint = m_paths.cbegin(); paint != m_paths.cend(); ++paint) {
      charge();
      FillCoverage coverage;
      coverage.fillIndex = paint.key();
      for (const auto &piece : paint.value().pieces) {
        charge();
        const auto contribution = crossing(piece, query);
        coverage.boundary       = coverage.boundary || contribution.boundary;
        coverage.winding += contribution.winding;
      }
      coverage.evenOdd = (coverage.winding % 2) != 0;
      coverage.nonZero = coverage.winding != 0;
      boundary         = boundary || coverage.boundary;
      if (coverage.evenOdd)
        sample.evenOddFillIndices.append(coverage.fillIndex);
      if (coverage.nonZero)
        sample.nonZeroFillIndices.append(coverage.fillIndex);
      sample.coverage.append(coverage);
    }
    if (boundary) {
      sample.status = FillStatus::Boundary;
      sample.reason = "source-fill-boundary";
    } else if (m_options.rule == FillRule::RequireAgreement &&
               sample.evenOddFillIndices != sample.nonZeroFillIndices) {
      sample.status = FillStatus::Unresolved;
      sample.reason = "unverified-source-fill-rule";
    } else {
      sample.fillIndices = m_options.rule == FillRule::EvenOdd
                               ? sample.evenOddFillIndices
                               : sample.nonZeroFillIndices;
    }
    sample.operations = m_statistics.operations - before;
    checkCancelled();
    return sample;
  }

  const QVector<FillClosure> &closures() const { return m_closures; }
  FillStatistics statistics() const { return m_statistics; }

private:
  struct Crossing {
    int winding   = 0;
    bool boundary = false;
  };

  FillOptions m_options;
  QString m_sourceId;
  QMap<int, PaintPath> m_paths;
  // Source segments with this paint on both sides (implicit closure only).
  QMap<int, QVector<Segment>> m_interior;
  QVector<FillClosure> m_closures;
  FillStatistics m_statistics;

  [[noreturn]] void fail(const QString &code, const QString &message) const {
    throw FillError(code, m_sourceId, message);
  }

  void checkCancelled() const {
    if (m_options.cancelled && m_options.cancelled())
      fail("cancelled", "Source fill evaluation cancelled.");
  }

  void charge() {
    if (m_statistics.operations >= m_options.maxWork)
      fail("work-limit", "Cumulative source fill work budget exceeded.");
    const auto before = m_statistics.operations;
    ++m_statistics.operations;
    if ((before >> 8) != (m_statistics.operations >> 8)) checkCancelled();
  }

  void validateOptions() const {
    if (!m_options.maxSourceSegments || !m_options.maxPreparedPieces ||
        !m_options.maxWork)
      fail("invalid-options", "Source fill budgets must be positive.");
    if (!std::isfinite(m_options.boundaryTolerance) ||
        m_options.boundaryTolerance < 0 ||
        Real(m_options.boundaryTolerance) >
            std::numeric_limits<Real>::max() / 1024)
      fail("invalid-options", "Invalid source-pixel boundary tolerance.");
  }

  void validatePoint(Point point) const {
    // The reader already bounds source coordinates. This additionally makes
    // the source-only API safe for directly constructed Shapes and points.
    const Real safe = std::numeric_limits<Real>::max() / 1024;
    if (!std::isfinite(point.x) || !std::isfinite(point.y) ||
        std::abs(Real(point.x)) > safe || std::abs(Real(point.y)) > safe)
      fail("invalid-coordinate", "Non-finite or unsafe source coordinate.");
  }

  void validatePaint(const Shape &shape, int fillIndex) {
    charge();
    if (fillIndex < 0 || (fillIndex && !shape.fills.contains(fillIndex)))
      fail("invalid-fill-index",
           QString("Unknown source fill style %1.").arg(fillIndex));
  }

  void appendSegment(int fillIndex, const Segment &segment) {
    charge();
    // Every directed segment produces at least one prepared piece. Fail
    // before retaining a source-sized intermediate that cannot fit its bound.
    if (m_statistics.directedSegments >= m_options.maxPreparedPieces)
      fail("prepared-piece-limit", "Prepared fill piece budget exceeded.");
    auto &segments = m_paths[fillIndex].segments;
    if (segments.size() == std::numeric_limits<int>::max())
      fail("prepared-piece-limit", "A fill path exceeds container limits.");
    segments.append(segment);
    ++m_statistics.directedSegments;
  }

  void appendClosure(int fillIndex, const Segment &segment, bool sourceEdge) {
    appendSegment(fillIndex, segment);
    FillClosure closure;
    closure.fillIndex  = fillIndex;
    closure.segment    = segment;
    closure.sourceEdge = sourceEdge;
    closure.sourceId =
        m_sourceId + QString(sourceEdge ? "/fill[%1]/interior-edge-closure[%2]"
                                        : "/fill[%1]/implicit-closure[%2]")
                         .arg(fillIndex)
                         .arg(m_statistics.implicitClosures);
    m_closures.append(std::move(closure));
    ++m_statistics.implicitClosures;
  }

  bool balancedGraph(const PaintPath &path,
                     std::map<VertexKey, Vertex> &vertices) {
    vertices.clear();
    for (int i = 0; i < path.segments.size(); ++i) {
      charge();
      const auto &segment = path.segments.at(i);
      auto &start         = vertices[key(segment.start)];
      ++start.outgoing;
      start.next = i;
      ++vertices[key(segment.end)].incoming;
    }
    bool balanced = true;
    for (const auto &entry : vertices) {
      charge();
      if (entry.second.incoming != entry.second.outgoing) balanced = false;
    }
    return balanced;
  }

  // Links each open chain end (more incoming than outgoing segments) to the
  // nearest chain start by the fewest unused source edges that carry this
  // paint on both sides, traversed in either direction.
  void closeAlongInteriorEdges(int fillIndex,
                               std::map<VertexKey, Vertex> &vertices) {
    const auto found = m_interior.constFind(fillIndex);
    if (found == m_interior.cend()) return;
    const QVector<Segment> &interior = found.value();
    std::map<VertexKey, std::vector<std::pair<int, bool>>> adjacency;
    for (int i = 0; i < interior.size(); ++i) {
      charge();
      adjacency[key(interior[i].start)].push_back({i, true});
      adjacency[key(interior[i].end)].push_back({i, false});
    }
    std::vector<bool> used(size_t(interior.size()), false);
    auto deficit = [&](const VertexKey &vertex) {
      const auto entry = vertices.find(vertex);
      return entry == vertices.end() ? qint64(0)
                                     : qint64(entry->second.outgoing) -
                                           qint64(entry->second.incoming);
    };
    std::vector<VertexKey> ends;
    for (const auto &entry : vertices)
      if (entry.second.incoming > entry.second.outgoing)
        ends.push_back(entry.first);
    for (const auto &end : ends) {
      while (deficit(end) < 0) {
        std::map<VertexKey, std::pair<int, bool>> parent;
        std::set<VertexKey> seen{end};
        std::deque<VertexKey> queue{end};
        bool reached = false;
        VertexKey target;
        while (!queue.empty() && !reached) {
          const VertexKey vertex = queue.front();
          queue.pop_front();
          const auto links = adjacency.find(vertex);
          if (links == adjacency.end()) continue;
          for (const auto &link : links->second) {
            charge();
            if (used[size_t(link.first)]) continue;
            const auto &segment = interior[link.first];
            const VertexKey next =
                key(link.second ? segment.end : segment.start);
            if (!seen.insert(next).second) continue;
            parent[next] = link;
            if (deficit(next) > 0) {
              reached = true;
              target  = next;
              break;
            }
            queue.push_back(next);
          }
        }
        if (!reached) break;
        std::vector<std::pair<int, Segment>> bridge;
        for (VertexKey vertex = target; vertex != end;) {
          charge();
          const auto link  = parent.at(vertex);
          Segment directed = interior[link.first];
          if (!link.second) std::swap(directed.start, directed.end);
          bridge.push_back({link.first, directed});
          vertex = key(directed.start);
        }
        // Flash closes the open chain itself with the straight chord. Only
        // follow interior edges that stay within half a pixel of it (the
        // curve lies in the hull of its controls); otherwise the straight
        // closure below applies.
        const Point from{end.first, end.second},
            to{target.first, target.second};
        const Real dx = Real(to.x) - from.x, dy = Real(to.y) - from.y;
        const Real length2 = dx * dx + dy * dy;
        auto distance      = [&](Point point) {
          Real t       = length2 > 0 ? ((Real(point.x) - from.x) * dx +
                                  (Real(point.y) - from.y) * dy) /
                                     length2
                                          : 0;
          t            = std::max(Real(0), std::min(Real(1), t));
          const Real x = Real(point.x) - (from.x + t * dx);
          const Real y = Real(point.y) - (from.y + t * dy);
          return std::sqrt(x * x + y * y);
        };
        bool nearChord = true;
        for (const auto &link : bridge)
          for (const Point &point :
               {link.second.start, link.second.end, link.second.control})
            nearChord = nearChord && distance(point) <= Real(0.5);
        if (!nearChord) break;
        for (const auto &link : bridge) {
          used[size_t(link.first)] = true;
          ++vertices[key(link.second.start)].outgoing;
          ++vertices[key(link.second.end)].incoming;
          appendClosure(fillIndex, link.second, true);
        }
      }
    }
  }

  void validateGraph(int fillIndex, PaintPath &path) {
    std::map<VertexKey, Vertex> vertices;
    if (balancedGraph(path, vertices)) return;
    if (!m_options.allowImplicitClosure)
      fail("unclosed-fill-path",
           QString("Fill style %1 has an unbalanced directed edge graph.")
               .arg(fillIndex));
    closeAlongInteriorEdges(fillIndex, vertices);
    if (balancedGraph(path, vertices)) return;

    // Only unique open chains may be closed. Balanced components can still
    // contain shared vertices; an open chain may not run into such a branch.
    for (const auto &entry : vertices) {
      charge();
      const auto &vertex = entry.second;
      if (vertex.incoming != vertex.outgoing &&
          !(vertex.incoming == 0 && vertex.outgoing == 1) &&
          !(vertex.incoming == 1 && vertex.outgoing == 0))
        fail("ambiguous-fill-closure",
             QString("Fill style %1 has an ambiguous open branch.")
                 .arg(fillIndex));
    }
    for (auto &entry : vertices) {
      charge();
      if (entry.second.incoming || entry.second.outgoing != 1) continue;
      const auto first = entry.first;
      auto current     = first;
      while (true) {
        charge();
        auto found = vertices.find(current);
        if (found == vertices.end())
          fail("ambiguous-fill-closure", "Open chain endpoint is missing.");
        auto &vertex = found->second;
        if (vertex.closureVisited || vertex.incoming > 1 || vertex.outgoing > 1)
          fail("ambiguous-fill-closure",
               QString("Fill style %1 cannot be closed unambiguously.")
                   .arg(fillIndex));
        vertex.closureVisited = true;
        if (!vertex.outgoing) break;
        current = key(path.segments.at(vertex.next).end);
      }
      Segment closing;
      closing.start   = {current.first, current.second};
      closing.end     = {first.first, first.second};
      closing.control = {closing.start.x / 2 + closing.end.x / 2,
                         closing.start.y / 2 + closing.end.y / 2};
      appendClosure(fillIndex, closing, false);
    }
  }

  void appendPiece(PaintPath &path, const Curve &curve) {
    charge();
    if (m_statistics.preparedPieces >= m_options.maxPreparedPieces ||
        path.pieces.size() == std::numeric_limits<int>::max())
      fail("prepared-piece-limit", "Prepared fill piece budget exceeded.");
    Piece piece;
    piece.curve = curve;
    piece.minX  = std::min({curve.start.x, curve.control.x, curve.end.x});
    piece.maxX  = std::max({curve.start.x, curve.control.x, curve.end.x});
    piece.scale = coordinateScale(curve);
    piece.direction =
        (curve.end.y > curve.start.y) - (curve.end.y < curve.start.y);
    path.pieces.append(piece);
    ++m_statistics.preparedPieces;
  }

  void prepare(PaintPath &path, const Segment &segment) {
    Curve curve{precise(segment.start), precise(segment.control),
                precise(segment.end)};
    if (!segment.quadratic)
      curve.control = interpolate(curve.start, curve.end, Real(0.5));
    // Split at the exact quadratic Y extremum, without flattening. Each
    // resulting piece is monotone, so a half-open ray crossing has one root.
    const Real denominator = curve.start.y - 2 * curve.control.y + curve.end.y;
    if (denominator != 0) {
      const Real t = (curve.start.y - curve.control.y) / denominator;
      if (t > 0 && t < 1) {
        const auto halves = split(curve, t);
        appendPiece(path, halves.first);
        appendPiece(path, halves.second);
        return;
      }
    }
    appendPiece(path, curve);
  }

  // Return a conservative parameter bracket in ascending-Y parameterization
  // (reversed for a decreasing piece). Bisection evaluates the quadratic
  // itself; there is no polyline approximation or geometry-dependent loop.
  std::pair<Real, Real> ascendingParameter(const Piece &piece, Real y) {
    const auto &curve = piece.curve;
    const Real lowY   = std::min(curve.start.y, curve.end.y);
    const Real highY  = std::max(curve.start.y, curve.end.y);
    if (y <= lowY) return {0, 0};
    if (y >= highY) return {1, 1};
    Real low = 0, high = 1;
    for (int i = 0; i < 64; ++i) {
      charge();
      const Real middle = (low + high) / 2;
      if (middle == low || middle == high) break;
      const Real t = piece.direction > 0 ? middle : 1 - middle;
      if (position(curve, t).y < y)
        low = middle;
      else
        high = middle;
    }
    return {low, high};
  }

  std::pair<Real, Real> xRange(const Curve &curve, Real low, Real high) {
    charge();
    const Real a = position(curve, low).x;
    const Real b = position(curve, high).x;
    Real minimum = std::min(a, b), maximum = std::max(a, b);
    const Real denominator = curve.start.x - 2 * curve.control.x + curve.end.x;
    if (denominator != 0) {
      const Real t = (curve.start.x - curve.control.x) / denominator;
      if (t > low && t < high) {
        const Real x = position(curve, t).x;
        minimum      = std::min(minimum, x);
        maximum      = std::max(maximum, x);
      }
    }
    return {minimum, maximum};
  }

  Crossing crossing(const Piece &piece, PrecisePoint query) {
    const auto &curve = piece.curve;
    // The caller's tolerance is in source pixels. Double-input roundoff and
    // bounded root-bracket uncertainty add conservative coordinate slack;
    // it is never used to turn a near-boundary point into a resolved face.
    const Real scale =
        std::max({piece.scale, std::abs(query.x), std::abs(query.y)});
    const Real slack = Real(m_options.boundaryTolerance) +
                       64 * std::numeric_limits<double>::epsilon() * scale;
    const Real lowY  = std::min(curve.start.y, curve.end.y);
    const Real highY = std::max(curve.start.y, curve.end.y);
    const bool rayCrossesY =
        piece.direction && query.y >= lowY && query.y < highY;

    if (query.y < lowY - slack || query.y > highY + slack ||
        query.x > piece.maxX + slack)
      return {};
    if (query.x < piece.minX - slack)
      return {rayCrossesY ? piece.direction : 0, false};

    if (!piece.direction) {
      // A horizontal quadratic can reverse in X. Its true X extrema, rather
      // than the control polygon, determine conservative boundary proximity.
      const auto range = xRange(curve, 0, 1);
      return {
          0, query.x >= range.first - slack && query.x <= range.second + slack};
    }

    // Bound the entire curve slice within the query's Y tolerance band.
    // This also catches endpoints excluded by the half-open crossing rule.
    // A broad slice can conservatively report Boundary; it cannot invent a
    // crossing or resolve a point that might lie on the source boundary.
    const auto lower = ascendingParameter(piece, query.y - slack);
    const auto upper = ascendingParameter(piece, query.y + slack);
    const Real low   = piece.direction > 0 ? lower.first : 1 - upper.second;
    const Real high  = piece.direction > 0 ? upper.second : 1 - lower.first;
    const auto range = xRange(curve, low, high);
    if (query.x >= range.first - slack && query.x <= range.second + slack)
      return {0, true};
    return {rayCrossesY && query.x < range.first - slack ? piece.direction : 0,
            false};
  }
};

FillEvaluator::FillEvaluator(const Shape &shape, FillOptions options,
                             QString sourceId)
    : m_impl(new Impl(shape, std::move(options), std::move(sourceId))) {}

FillEvaluator::~FillEvaluator() = default;

FillSample FillEvaluator::evaluate(Point sourcePoint) {
  return m_impl->evaluate(sourcePoint);
}

const QVector<FillClosure> &FillEvaluator::closures() const {
  return m_impl->closures();
}

FillStatistics FillEvaluator::statistics() const {
  return m_impl->statistics();
}

}  // namespace Flash
