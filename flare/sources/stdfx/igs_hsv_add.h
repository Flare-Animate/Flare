#pragma once

#ifndef igs_hsv_add_h
#define igs_hsv_add_h

#include <stdexcept> /* std::domain_error, std::invalid_argument */

// Implemented in igs_hsv_add.rs (flare_formats); returns 0, 1 (bad channels),
// 4 ("noise_ref_  no data"), 5 ("noise_ref_  bad zz") -> std::domain_error,
// or 3 (bad buffer/geometry) -> std::invalid_argument.
extern "C" {
int flare_igs_hsv_add_change(float *image_array, int height, int width,
                             int channels, const float *noi_image_array,
                             const float *ref, int xoffset, int yoffset,
                             int from_rgba, double offset, double hue_scale,
                             double sat_scale, double val_scale,
                             double alp_scale, bool add_blend_sw);
}

namespace igs {
namespace hsv_add {
// (header-only now: IGS_HSV_ADD_EXPORT is gone, nothing to export)
inline void change(
    float *image_array, const int height, const int width, const int channels,
    const float *noi_image_array,
    const float *ref,    /* 求める画像と同じ高、幅、channels数 */
    const int xoffset,   /* 0	INT_MIN ... INT_MAX	*/
    const int yoffset,   /* 0	INT_MIN ... INT_MAX	*/
    const int from_rgba, /* 0	0(R),1(G),2(B),3(A)	*/
    const double offset, /* 0.5	-1.0 ... 1.0	*/
    const double hue_scale, /* 0.0	-1.0 ... 1.0	*/
    const double sat_scale, /* 0.0	-1.0 ... 1.0	*/
    const double val_scale, /* 1.0	-1.0 ... 1.0	*/
    const double alp_scale, /* 0.0	-1.0 ... 1.0	*/
    const bool add_blend_sw
    /* true : the change is weighted by alpha
              (composite = under * (1 - alpha) + over)
       false: the change ignores alpha
              (composite = under * (1 - alpha) + over * alpha) */
) {
  switch (flare_igs_hsv_add_change(image_array, height, width, channels,
                                   noi_image_array, ref, xoffset, yoffset,
                                   from_rgba, offset, hue_scale, sat_scale,
                                   val_scale, alp_scale, add_blend_sw)) {
  case 0:
    return;
  case 1:
    throw std::domain_error("Bad channels,Not rgba/rgb/grayscale");
  case 4:
    throw std::domain_error("noise_ref_  no data");
  case 5:
    throw std::domain_error("noise_ref_  bad zz");
  default:
    throw std::invalid_argument("igs::hsv_add::change: bad image");
  }
}
}  // namespace hsv_add
}  // namespace igs
#endif /* !igs_hsv_add_h */
