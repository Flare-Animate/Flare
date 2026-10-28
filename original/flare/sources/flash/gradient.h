#pragma once

#include "model.h"

#include <functional>
#include <optional>

namespace Flash {

// A validated semantic view of an XFL gradient. The original Paint and package
// snapshot remain authoritative for authored spelling and source provenance.
// The full matrix is still in XFL's local, downward-Y pixel coordinates.
struct GradientSpec {
  enum class Spread { Pad, Reflect, Repeat };
  enum class Interpolation { Rgb, LinearRgb };

  struct Stop {
    double ratio = 0;
    Color color;
    // Keep the source alpha's numeric precision; Color is an 8-bit convenience.
    double alpha = 1;
  };

  Paint::Kind kind = Paint::Kind::RadialGradient;
  Affine matrix;
  Spread spread               = Spread::Pad;
  Interpolation interpolation = Interpolation::Rgb;
  double focalPointRatio      = 0;
  QVector<Stop> stops;
  // A singular matrix (determinant exactly zero: all-zero, or scaled to zero
  // along one axis) collapses the gradient field. Animate publishes it
  // unchanged, and the published shape is filled with the color at ratio 0,
  // the first stop's color and alpha, for every kind and spread (reference
  // renders of synthetic SWFs; SWFs Animate publishes keep such a fill too).
  // Its native style is that solid color (firstStopColor).
  bool singular = false;
};

struct GradientIssue {
  QString code;
  QString sourceId;
  QString message;
};

using GradientIssueSink = std::function<void(const GradientIssue &)>;

// Structural/model errors and source semantics without a verified mapping are
// distinguished by their issue codes. A valid result maps to the native
// TFlashGradientStyle (see FlashNative::gradientFieldToLocal) when that style
// can represent it; anything else keeps the reported solid fallback.
std::optional<GradientSpec> validateGradient(const Paint &paint,
                                             const GradientIssueSink &sink);

}  // namespace Flash
