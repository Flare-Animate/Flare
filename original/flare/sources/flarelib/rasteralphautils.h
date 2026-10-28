#pragma once

#include "tpixelutils.h"
#include "traster.h"

#include <algorithm>

namespace RasterAlphaUtils {

// LevelProperties::doPremultiply() describes straight source pixels. Normalize
// those pixels before applying opacity so the result can use premultiplied
// over.
inline TPixel32 preparePixel(TPixel32 pixel, bool doPremultiply,
                             double opacity) {
  if (doPremultiply) premult(pixel);
  opacity = std::max(0.0, std::min(1.0, opacity));
  pixel.r = static_cast<TPixel32::Channel>(pixel.r * opacity + 0.5);
  pixel.g = static_cast<TPixel32::Channel>(pixel.g * opacity + 0.5);
  pixel.b = static_cast<TPixel32::Channel>(pixel.b * opacity + 0.5);
  pixel.m = static_cast<TPixel32::Channel>(pixel.m * opacity + 0.5);
  return pixel;
}

// The unmodified fast path may share the source; every pixel transformation
// owns a copy. In particular, onion skin must never alter cached level pixels.
inline TRaster32P prepareRaster(const TRaster32P &source, bool doPremultiply,
                                double opacity) {
  if (!source) return TRaster32P();
  opacity = std::max(0.0, std::min(1.0, opacity));
  if (!doPremultiply && opacity == 1.0) return source;

  TRaster32P result = source->clone();
  result->lock();
  for (int y = 0; y < result->getLy(); ++y) {
    TPixel32 *pixels = result->pixels(y);
    for (int x = 0; x < result->getLx(); ++x)
      pixels[x] = preparePixel(pixels[x], doPremultiply, opacity);
  }
  result->unlock();
  return result;
}

}  // namespace RasterAlphaUtils
