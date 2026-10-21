#pragma once

#ifndef igs_hsv_noise_in_camera_h
#define igs_hsv_noise_in_camera_h

#include <stdexcept> /* std::domain_error, std::invalid_argument */

#ifndef IGS_HSV_NOISE_IN_CAMERA_EXPORT
#define IGS_HSV_NOISE_IN_CAMERA_EXPORT
#endif

// Implemented in igs_hsv_noise_in_camera.rs (flare_formats); returns 0, 1 (bad
// channels -> std::domain_error), 2 (bad bits -> std::domain_error) or 3 (null
// buffer / negative size / misaligned buffer -> std::invalid_argument).
extern "C" {
int flare_igs_hsv_noise_in_camera_change(
    void *image_array, int height, int width, int channels, int bits,
    int camera_x, int camera_y, int camera_w, int camera_h, double hue_range,
    double sat_range, double val_range, double alp_range,
    unsigned long random_seed, double near_blur,
    double sat_effective, double sat_center, int sat_type,
    double val_effective, double val_center, int val_type,
    double alp_effective, double alp_center, int alp_type);
}

namespace igs {
namespace hsv_noise_in_camera {
inline void change(
    void *image_array

    ,
    const int height, const int width, const int channels, const int bits

    ,
    const int camera_x, const int camera_y, const int camera_w,
    const int camera_h

    ,
    const double hue_range = 0.025  // 0 ... 1.0
    ,
    const double sat_range = 0.0  // 0 ... 1.0
    ,
    const double val_range = 0.035  // 0 ... 1.0
    ,
    const double alp_range = 0.0  // 0 ... 1.0
    ,
    const unsigned long random_seed = 1  // 0 ... ULONG_MAX
    ,
    const double near_blur = 0.500  // 0 ... 0.5

    ,
    const double sat_effective = 0.0  // 0 ... 1.0
    ,
    const double sat_center = 0.5  // 0 ... 1.0
    ,
    const int sat_type = 0
    // 0(shift_whole),1(shift_term),2(decrease_whole),3(decrease_term)
    ,
    const double val_effective = 0.0  // 0 ... 1.0
    ,
    const double val_center = 0.5  // 0 ... 1.0
    ,
    const int val_type = 0
    // 0(shift_whole),1(shift_term),2(decrease_whole),3(decrease_term)
    ,
    const double alp_effective = 0.0  // 0 ... 1.0
    ,
    const double alp_center = 0.5  // 0 ... 1.0
    ,
    const int alp_type = 0
    // 0(shift_whole),1(shift_term),2(decrease_whole),3(decrease_term)
    ) {
  switch (flare_igs_hsv_noise_in_camera_change(
      image_array, height, width, channels, bits, camera_x, camera_y, camera_w, camera_h, hue_range, sat_range, val_range, alp_range, random_seed, near_blur, sat_effective, sat_center, sat_type, val_effective, val_center, val_type, alp_effective, alp_center, alp_type)) {
  case 0:
    return;
  case 1:
    throw std::domain_error("Bad channels,Not rgba/rgb/grayscale");
  case 2:
    throw std::domain_error("Bad bits,Not uchar/ushort");
  default:
    throw std::invalid_argument("igs::hsv_noise_in_camera::change: bad image");
  }
}
}  // namespace hsv_noise_in_camera
}  // namespace igs

#endif /* !igs_hsv_noise_in_camera_h */
