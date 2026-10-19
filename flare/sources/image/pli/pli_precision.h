#pragma once

#include "tstroke.h"
#include "tcurves.h"
#include "tvectorimage.h"

#include <algorithm>
#include <cmath>
#include <limits>

namespace PliCodec {

constexpr int defaultPrecisionScale   = 16384;
constexpr int maxSourcePrecisionScale = 1048576;

// PLI truncates the first control and rounds each quadratic delta to a signed
// int. PreserveSource frames quantize every absolute control first, then store
// differences of those integers so separately authored shared endpoints stay
// joined. Inspect both absolute controls and deltas before any integer cast.
inline int precisionScaleForImage(const TVectorImageP &image) {
  if (!image) return 0;
  double largest = 0;
  auto include   = [&](double value) {
    if (!std::isfinite(value)) return false;
    largest = std::max(largest, std::abs(value));
    return true;
  };
  for (UINT index = 0; index < image->getStrokeCount(); ++index) {
    const TStroke *stroke = image->getStroke(index);
    if (!stroke) return 0;
    const int chunks = stroke->getChunkCount();
    if (chunks <= 0) continue;
    const TPointD first = stroke->getChunk(0)->getP0();
    if (!include(first.x) || !include(first.y)) return 0;
    for (int chunk = 0; chunk < chunks; ++chunk) {
      const TThickQuadratic *curve = stroke->getChunk(chunk);
      const TPointD p0 = curve->getP0(), p1 = curve->getP1(),
                    p2 = curve->getP2();
      if (!include(p0.x) || !include(p0.y) || !include(p1.x) ||
          !include(p1.y) || !include(p2.x) || !include(p2.y) ||
          !include(p1.x - p0.x) || !include(p1.y - p0.y) ||
          !include(p2.x - p1.x) || !include(p2.y - p1.y))
        return 0;
    }
  }
  // Keep normal Flare drawings byte-compatible. Imported drawings use
  // the finest safe power-of-two grid this PLI version can encode.
  const bool preserveSource = image->getRegionFindingMode() ==
                              TVectorImage::RegionFindingMode::PreserveSource;
  const int maximum =
      preserveSource ? maxSourcePrecisionScale : defaultPrecisionScale;
  // Two spare integer units cover the difference between independently
  // truncated absolute endpoints and a rounded raw delta.
  const double safeLimit = double(std::numeric_limits<int>::max()) - 3.0;
  for (int scale = maximum; scale >= defaultPrecisionScale; scale /= 2)
    if (largest <= safeLimit / scale) return scale;
  return 0;
}

}  // namespace PliCodec
