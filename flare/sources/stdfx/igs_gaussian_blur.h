#pragma once

#ifndef igs_gaussian_blur_hv_h
#define igs_gaussian_blur_hv_h

#include <stdexcept> /* std::domain_error, std::invalid_argument */

// Implemented in igs_gaussian_blur.rs (flare_formats). convert returns 0, or
// 1 / 5 / 6 for the std::domain_error cases below and 3 for std::invalid_argument.
extern "C" {
int flare_igs_gaussian_blur_int_radius(double real_radius);
int flare_igs_gaussian_blur_buffer_bytes(int height_with_margin,
                                         int width_with_margin, int int_radius);
int flare_igs_gaussian_blur_convert(const float *in_with_margin,
                                    float *out_no_margin,
                                    int height_with_margin,
                                    int width_with_margin, int channels,
                                    const float *ref, void *buffer,
                                    int buffer_bytes, int int_radius,
                                    double real_radius, double sigma);
}

namespace igs {
namespace gaussian_blur_hv {
inline const int int_radius(const double real_radius) {
  return flare_igs_gaussian_blur_int_radius(real_radius);
}
inline const int buffer_bytes(const int height_with_margin,
                              const int width_with_margin,
                              const int int_radius) {
  return flare_igs_gaussian_blur_buffer_bytes(height_with_margin,
                                              width_with_margin, int_radius);
}
inline void convert(
    /* 入出力画像 */
    const float* in_with_margin, float* out_no_margin,
    const int height_with_margin, const int width_with_margin,
    const int channels,
    /* Pixel毎に効果の強弱 */
    const float* ref, /* 求める画像(out)と同じ高、幅、ch数 */
    /* 計算バッファ */
    void* buffer,
    int buffer_bytes,  // Must be igs::gaussian_blur_hv::buffer_bytes(-)
    /* Action Geometry */
    const int int_radius,  // =margin
    const double real_radius, const double sigma = 0.25) {
  switch (flare_igs_gaussian_blur_convert(
      in_with_margin, out_no_margin, height_with_margin, width_with_margin,
      channels, ref, buffer, buffer_bytes, int_radius, real_radius, sigma)) {
  case 1:
    throw std::domain_error("Bad channels,Not rgba/rgb/grayscale");
  case 3:
    throw std::invalid_argument("igs::gaussian_blur_hv::convert: bad argument");
  case 5:
    throw std::domain_error("buffer_inn is empty");
  case 6:
    throw std::domain_error("buffer_out is empty");
  }
}
}  // namespace gaussian_blur_hv
}  // namespace igs

#endif /* !igs_gaussian_blur_hv_h */
