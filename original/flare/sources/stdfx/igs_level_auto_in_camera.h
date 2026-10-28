#pragma once

#ifndef igs_level_auto_in_camera_h
#define igs_level_auto_in_camera_h

#include <stdexcept> /* std::domain_error, std::invalid_argument */

// Implemented in igs_level_auto_in_camera.rs (flare_formats); returns 0, 1
// (bad channels), 2 (bad bits) -> std::domain_error, or 3 (null array /
// negative size) -> std::invalid_argument. The camera rectangle is clamped
// to the image.
extern "C" {
int flare_igs_level_auto_in_camera_change(
    void *image_array, int height, int width, int channels, int bits,
    const bool *act_sw, const double *in_min_shift, const double *in_max_shift,
    const double *out_min, const double *out_max, const double *gamma,
    int camera_x, int camera_y, int camera_w, int camera_h);
}

namespace igs {
namespace level_auto_in_camera {
// (header-only now: IGS_LEVEL_AUTO_IN_CAMERA_EXPORT is gone, nothing to export)
inline void change(void *image_array, const int height, const int width,
                   const int channels, const int bits,
                   const bool *act_sw,          // true(false/true)
                   const double *in_min_shift,  // 0(-1...1)
                   const double *in_max_shift,  // 0(-1...1)
                   const double *out_min,       // 0(0...1)
                   const double *out_max,       // 1(0...1)
                   const double *gamma,         // 1(0.01...100)
                   const int camera_x, const int camera_y, const int camera_w,
                   const int camera_h) {
  switch (flare_igs_level_auto_in_camera_change(
      image_array, height, width, channels, bits, act_sw, in_min_shift,
      in_max_shift, out_min, out_max, gamma, camera_x, camera_y, camera_w,
      camera_h)) {
  case 0:
    return;
  case 1:
    throw std::domain_error("Bad channels,Not rgba/rgb/grayscale");
  case 2:
    throw std::domain_error("Bad bits,Not uchar/ushort");
  default:
    throw std::invalid_argument("igs::level_auto_in_camera::change: bad image");
  }
}
}  // namespace level_auto_in_camera
}  // namespace igs

#endif /* !igs_level_auto_in_camera_h */
