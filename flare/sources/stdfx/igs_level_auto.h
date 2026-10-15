#pragma once

#ifndef igs_level_auto_h
#define igs_level_auto_h

#include <stdexcept> /* std::domain_error, std::invalid_argument */

// Implemented in igs_level_auto.rs (flare_formats); returns 0, 1 (bad
// channels), 2 (bad bits) -> std::domain_error, or 3 (null array / negative
// size) -> std::invalid_argument.
extern "C" {
int flare_igs_level_auto_change(unsigned char *image_array, int height,
                                int width, int channels, int bits,
                                const bool *act_sw, const double *in_min_shift,
                                const double *in_max_shift,
                                const double *out_min, const double *out_max,
                                const double *gamma);
}

namespace igs {
namespace level_auto {
// (header-only now: IGS_LEVEL_AUTO_EXPORT is gone, nothing to export)
inline void change(unsigned char *image_array,

                   const int height, const int width, const int channels,
                   const int bits,

                   const bool *act_sw,          // true(false/true)
                   const double *in_min_shift,  // 0(-1...1)
                   const double *in_max_shift,  // 0(-1...1)
                   const double *out_min,       // 0(0...1)
                   const double *out_max,       // 1(0...1)
                   const double *gamma          // 1(0.01...100)
) {
  switch (flare_igs_level_auto_change(image_array, height, width, channels,
                                      bits, act_sw, in_min_shift, in_max_shift,
                                      out_min, out_max, gamma)) {
  case 0:
    return;
  case 1:
    throw std::domain_error("Bad channels,Not rgba/rgb/grayscale");
  case 2:
    throw std::domain_error("Bad bits,Not uchar/ushort");
  default:
    throw std::invalid_argument("igs::level_auto::change: bad image");
  }
}
}
}

#endif /* !igs_level_auto_h */
