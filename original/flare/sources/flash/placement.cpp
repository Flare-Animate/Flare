#include "placement.h"

#include <cmath>

namespace Flash {

StagePlacement stagePlacement(const Affine &matrix, double dpi,
                              Point sourcePivot) {
  matrix.validate();
  if (!std::isfinite(dpi) || dpi <= 0 || !std::isfinite(sourcePivot.x) ||
      !std::isfinite(sourcePivot.y))
    throw Error("Invalid native placement scale or pivot.");

  StagePlacement result;
  result.center             = {sourcePivot.x / dpi, -sourcePivot.y / dpi};
  const Point pivotPosition = matrix.apply(sourcePivot);
  result.position           = {pivotPosition.x / dpi, -pivotPosition.y / dpi};
  // F * A * F changes from downward source Y to upward native Y.
  const double a = matrix.a, b = -matrix.b, c = -matrix.c, d = matrix.d;
  const double firstLength  = std::hypot(a, b);
  const double secondLength = std::hypot(c, d);
  double radians            = 0;
  if (firstLength == 0) {
    result.scaleX = 0;
    result.scaleY = secondLength;
    if (secondLength != 0) radians = std::atan2(-c, d);
  } else {
    // Rotation followed by X shear and scale is the conventional QR form.
    radians             = std::atan2(b, a);
    const double cosine = a / firstLength, sine = b / firstLength;
    const double upper = cosine * c + sine * d;
    const double lower = -sine * c + cosine * d;
    result.scaleX      = firstLength;
    result.scaleY      = lower;
    result.shearX      = lower == 0 ? 0 : upper / lower;
    // Rank-one matrices with two nonzero columns cannot be represented by
    // QR's single shear. A rotated basis permits two finite native shears.
    // It also avoids unstable controls for nearly collapsed transforms.
    if (secondLength != 0 && (lower == 0 || !std::isfinite(result.shearX) ||
                              std::abs(result.shearX) > 1000000)) {
      radians += std::acos(-1.0) / 4;
      const double cs = std::cos(radians), sn = std::sin(radians);
      result.scaleX = cs * a + sn * b;
      result.scaleY = -sn * c + cs * d;
      result.shearX = (cs * c + sn * d) / result.scaleY;
      result.shearY = (-sn * a + cs * b) / result.scaleX;
    }
  }
  result.angle = radians * 180 / std::acos(-1.0);
  for (double value : {result.position.x, result.position.y, result.center.x,
                       result.center.y, result.angle, result.scaleX,
                       result.scaleY, result.shearX, result.shearY})
    if (!std::isfinite(value))
      throw Error("Native placement exceeds finite coordinate limits.");
  return result;
}

}  // namespace Flash
