#pragma once

#ifndef igs_motion_blur_h
#define igs_motion_blur_h

#include <stdexcept> /* std::domain_error, std::invalid_argument */
#include <new>       /* std::bad_alloc */

// Implemented in igs_motion_blur.rs (flare_formats); returns 0, or 2 for the
// std::domain_error below, 3 for std::invalid_argument, 4 for std::bad_alloc.
extern "C" {
int flare_igs_motion_blur_convert(const unsigned char *image_in,
                                  unsigned char *image_out, int height,
                                  int width, int channels, int bits,
                                  double x_vector, double y_vector,
                                  double vector_scale, double curve,
                                  int zanzo_length, double zanzo_power,
                                  bool alpha_rend_sw);
}

namespace igs {
namespace motion_blur {
inline void convert(
    const unsigned char *image_in, unsigned char *image_out,

    const int height, const int width, const int channels, const int bits,

    const double x_vector = 1.0, const double y_vector = 1.0,
    const double vector_scale = 1.0, const double curve = 1.0,
    const int zanzo_length = 0.0, const double zanzo_power = 1.0,
    const bool alpha_rend_sw = true) {
  switch (flare_igs_motion_blur_convert(
      image_in, image_out, height, width, channels, bits, x_vector, y_vector,
      vector_scale, curve, zanzo_length, zanzo_power, alpha_rend_sw)) {
  case 2:
    throw std::domain_error("Bad bits,Not uchar/ushort");
  case 3:
    throw std::invalid_argument("igs::motion_blur::convert: bad argument");
  case 4:
    throw std::bad_alloc();
  }
}
}
}

#endif /* !igs_motion_blur_h */
