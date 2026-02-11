#pragma once

#if !defined(flare_PLUGIN_UTILITIES_HPP__)
#define flare_PLUGIN_UTILITIES_HPP__
#include "flare_hostif.h"
#include <tgeometry.h>

namespace plugin {
namespace utils {

inline bool copy_rect(flare_rect_t *dst, const TRectD &src) {
  dst->x0 = src.x0;
  dst->y0 = src.y0;
  dst->x1 = src.x1;
  dst->y1 = src.y1;
  return true;
}

inline TRectD restore_rect(const flare_rect_t *src) {
  return TRectD(src->x0, src->y0, src->x1, src->y1);
}

inline bool copy_affine(flare_affine_t *dst, const TAffine &src) {
  dst->a11 = src.a11;
  dst->a12 = src.a12;
  dst->a13 = src.a13;
  dst->a21 = src.a21;
  dst->a22 = src.a22;
  dst->a23 = src.a23;
  return true;
}

inline TAffine restore_affine(const flare_affine_t *src) {
  return TAffine(src->a11, src->a12, src->a13, src->a21, src->a22, src->a23);
}
}
}
#endif

