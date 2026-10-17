#pragma once

#ifndef igs_density_h
#define igs_density_h

#include <stdexcept> /* std::domain_error */

// Implemented in igs_density.rs (flare_formats); returns 1 for the
// std::domain_error case below.
extern "C" {
int flare_igs_density_change(float *image_array, int height, int width,
                             int channels, const float *ref, double density);
}

namespace igs {
namespace density {
// (header-only now: IGS_DENSITY_EXPORT is gone, nothing to export from the dll)
inline void change(float *image_array, const int height, const int width,
                   const int channels, /* 4(=RGBAでなければならない) */
                   const float *ref, const double density = 1.0) {
  if (flare_igs_density_change(image_array, height, width, channels, ref,
                               density))
    throw std::domain_error("Bad channels,Not rgba");
}
}
}  // namespace igs

#endif /* !igs_density_h */
