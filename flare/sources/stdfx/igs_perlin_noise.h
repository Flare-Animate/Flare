#pragma once

#ifndef igs_perlin_noise_h
#define igs_perlin_noise_h

#ifndef IGS_PERLIN_NOISE_EXPORT
#define IGS_PERLIN_NOISE_EXPORT
#endif

#include <stdexcept> /* std::domain_error, std::invalid_argument */

// Implemented in igs_perlin_noise.rs (flare_formats); returns 0, 1 (bad bits)
// or 2 (invalid argument).
extern "C" {
int flare_igs_perlin_noise_change(unsigned char *image_array, int height,
                                  int width, int wrap, int channels, int bits,
                                  bool alpha_rendering_sw, double a11,
                                  double a12, double a13, double a21,
                                  double a22, double a23, double zz,
                                  int octaves_start, int octaves_end,
                                  double persistence);
}

namespace igs {
namespace perlin_noise {
inline void change(
    unsigned char *image_array, const int height  // pixel
    ,
    const int width, const int wrap  // pixel
    ,
    const int channels, const int bits, const bool alpha_rendering_sw = true

    ,
    const double a11 = 0.01  // 2D affine transformation
    ,
    const double a12 = 0.0, const double a13 = 0.0, const double a21 = 0.0,
    const double a22 = 0.01, const double a23 = 0.0

    ,
    const double zz = 0.0, const int octaves_start = 3  // 0...
    ,
    const int octaves_end = 9  // 0...
    ,
    const double persistence = 1. / 1.7320508  // not 0
) {
  switch (flare_igs_perlin_noise_change(
      image_array, height, width, wrap, channels, bits, alpha_rendering_sw, a11,
      a12, a13, a21, a22, a23, zz, octaves_start, octaves_end, persistence)) {
  case 1:
    throw std::domain_error("Bad bits,Not uchar/ushort");
  case 2:
    throw std::invalid_argument("igs::perlin_noise::change: invalid argument");
  }
}
}
}  // namespace igs

#endif /* !igs_perlin_noise_h */
