#pragma once

#include "model.h"

#include <functional>
#include <memory>

namespace Flash {

// No undocumented XFL property/default is inferred here. A caller with a
// verified source rule may select it; otherwise both rules must agree.
enum class FillRule { RequireAgreement, EvenOdd };
enum class FillStatus { Resolved, Boundary, Unresolved };

struct FillOptions {
  FillRule rule = FillRule::RequireAgreement;
  // Close a paint whose directed edge graph is unbalanced: first along source
  // edges that carry that paint on both sides (Flash draws such an edge as
  // part of the outline it completes), then with one straight line from the
  // end of each remaining unique open chain back to its start (as Flash and
  // Ruffle close an open fill path). Every closure is listed in closures().
  bool allowImplicitClosure = false;
  quint64 maxSourceSegments = 4000000;
  quint64 maxPreparedPieces = 8000000;
  quint64 maxWork           = 200000000;
  double boundaryTolerance = 1e-9;  // source pixels; numerical error adds slack
  std::function<bool()> cancelled;
};

class FillError : public Error {
public:
  const QString code;
  const QString sourceId;
  FillError(const QString &code, const QString &sourceId,
            const QString &message);
};

struct FillCoverage {
  int fillIndex  = 0;
  qint64 winding = 0;  // +1 for a crossing toward increasing source Y
  bool evenOdd   = false;
  bool nonZero   = false;
  bool boundary  = false;
};

struct FillSample {
  FillStatus status = FillStatus::Resolved;
  QString reason;
  // All covering paint IDs, in ascending source style-ID drawing order.
  // Multiple IDs retain overlap/alpha semantics; no topmost/color is guessed.
  // Empty when status is Boundary/Unresolved: callers must not assign a face.
  QVector<int> fillIndices;
  QVector<int> evenOddFillIndices;
  QVector<int> nonZeroFillIndices;
  QVector<FillCoverage> coverage;
  quint64 operations = 0;  // this query, within the cumulative maxWork budget
};

struct FillClosure {
  int fillIndex = 0;
  Segment segment;
  QString sourceId;
  // The closure follows an existing source edge (fill0 == fill1 == fillIndex)
  // rather than adding new geometry.
  bool sourceEdge = false;
};

struct FillStatistics {
  quint64 sourceSegments   = 0;
  quint64 directedSegments = 0;
  quint64 preparedPieces   = 0;
  quint64 implicitClosures = 0;  // including closures along source edges
  quint64 operations       = 0;  // preparation plus all point queries
};

// Evaluates local, untransformed source geometry in pixels, with downward Y.
// This is not a renderer or an element/paint compatibility guard. Callers keep
// the original paints, effects, matrices, strokes and all other source data.
// Exact balanced directed edge graphs do not need an arbitrary contour walk.
// Unbalanced graphs reject unless explicitly permitted unambiguous chains can
// be closed; closures() exposes every additional geometric boundary so a
// native adapter cannot silently evaluate geometry it never constructed.
// Instances are single-threaded; work/cancellation limits are cumulative.
class FillEvaluator {
public:
  explicit FillEvaluator(const Shape &shape, FillOptions options = {},
                         QString sourceId = {});
  ~FillEvaluator();
  FillEvaluator(const FillEvaluator &)            = delete;
  FillEvaluator &operator=(const FillEvaluator &) = delete;

  FillSample evaluate(Point sourcePoint);
  const QVector<FillClosure> &closures() const;
  FillStatistics statistics() const;

private:
  class Impl;
  std::unique_ptr<Impl> m_impl;
};

}  // namespace Flash
