#pragma once

#ifndef igs_hsv_adjust_h
#define igs_hsv_adjust_h

#include <stdexcept> /* std::domain_error, std::invalid_argument */

// Implemented in igs_hsv_adjust.rs (flare_formats); returns 0, 1 (bad
// channels -> std::domain_error) or 3 (bad buffer/geometry).
extern "C" {
int flare_igs_hsv_adjust_change(float *image_array, int height, int width,
                                int channels, const float *ref,
                                double hue_pivot, double hue_scale,
                                double hue_shift, double sat_pivot,
                                double sat_scale, double sat_shift,
                                double val_pivot, double val_scale,
                                double val_shift, bool add_blend_sw);
}

namespace igs {
namespace hsv_adjust {
// (header-only now: IGS_HSV_ADJUST_EXPORT is gone, nothing to export)
inline void change(
    float *image_array, const int height, const int width, const int channels,
    const float *ref, /* 求める画像と同じ高、幅、channels数 */
    const double hue_pivot, /* 0.0  ...0...360... */
    const double hue_scale, /* 1.0  ...1...       */
    const double hue_shift, /* 0.0  ...0...360... */
    const double sat_pivot, /* 0.0  ...0...1...   */
    const double sat_scale, /* 1.0  ...1...       */
    const double sat_shift, /* 0.0  ...0...1...   */
    const double val_pivot, /* 0.0  ...0...1...   */
    const double val_scale, /* 1.0  ...1...       */
    const double val_shift, /* 0.0  ...0...1...   */
    const bool add_blend_sw
    /* true : the change is weighted by alpha
              (composite = under * (1 - alpha) + over)
       false: the change ignores alpha
              (composite = under * (1 - alpha) + over * alpha) */
) {
  switch (flare_igs_hsv_adjust_change(image_array, height, width, channels, ref,
                                      hue_pivot, hue_scale, hue_shift,
                                      sat_pivot, sat_scale, sat_shift,
                                      val_pivot, val_scale, val_shift,
                                      add_blend_sw)) {
  case 0:
    return;
  case 1:
    throw std::domain_error("Bad channels,Not rgba/rgb/grayscale");
  default:
    throw std::invalid_argument("igs::hsv_adjust::change: bad image");
  }
}
}
}  // namespace igs

#endif /* !igs_hsv_adjust_h */
