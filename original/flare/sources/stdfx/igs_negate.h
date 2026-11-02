#pragma once

#ifndef igs_negate_h
#define igs_negate_h

#include <stdexcept> /* std::domain_error */

// Implemented in igs_negate.rs (flare_formats); returns 0, or 1 / 2 for the
// two std::domain_error cases below.
extern "C" {
int flare_igs_negate_change(unsigned char *image_array, int height, int width,
                            int channels, int bits, const bool *sw_array);
}

namespace igs {
namespace negate {
// (header-only now: IGS_NEGATE_EXPORT is gone, nothing to export from the dll)
inline void change(unsigned char *image_array, const int height,
                   const int width, const int channels, const int bits,
                   const bool *sw_array /* channels size array */
) {
  switch (flare_igs_negate_change(image_array, height, width, channels, bits,
                                  sw_array)) {
  case 1:
    throw std::domain_error("Bad channels,Not rgba/rgb/grayscale");
  case 2:
    throw std::domain_error("Bad bits,Not uchar/ushort/bit");
  }
}
}
}

#endif /* !igs_negate_h */
