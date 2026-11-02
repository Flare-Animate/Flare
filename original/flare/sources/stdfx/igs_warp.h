#pragma once

#ifndef igs_warp_h
#define igs_warp_h

#include <stdexcept> /* std::invalid_argument */

// Implemented in igs_warp_hori.rs / igs_warp_vert.rs (flare_formats).
// Return 0, or 3 for a null buffer / non-positive geometry / bad reference channel.
extern "C" {
int flare_igs_warp_hori_change(float *image, int height, int width,
                               int channels, const float *refer,
                               int refchannels, int refcc, double maxlen,
                               bool alpha_rendering_sw, bool anti_aliasing_sw);
int flare_igs_warp_vert_change(float *image, int height, int width,
                               int channels, const float *refer,
                               int refchannels, int refcc, double maxlen,
                               bool alpha_rendering_sw, bool anti_aliasing_sw);
}

namespace igs {
namespace warp {
// (header-only now: IGS_WARP_EXPORT is gone, nothing to export from the dll)
inline void hori_change(
    float *image, const int height, const int width, const int channels,
    const float *refer,  // by height,width,channels
    const int refchannels, const int refcc, const double maxlen = 1.0,
    const bool alpha_rendering_sw = true, const bool anti_aliasing_sw = true) {
  if (flare_igs_warp_hori_change(image, height, width, channels, refer,
                                 refchannels, refcc, maxlen,
                                 alpha_rendering_sw, anti_aliasing_sw))
    throw std::invalid_argument("igs::warp::hori_change: bad image/reference");
}
inline void vert_change(
    float *image, const int height, const int width, const int channels,
    const float *refer,  // by height,width,channels
    const int refchannels, const int refcc, const double maxlen = 1.0,
    const bool alpha_rendering_sw = true, const bool anti_aliasing_sw = true) {
  if (flare_igs_warp_vert_change(image, height, width, channels, refer,
                                 refchannels, refcc, maxlen,
                                 alpha_rendering_sw, anti_aliasing_sw))
    throw std::invalid_argument("igs::warp::vert_change: bad image/reference");
}
}  // namespace warp
}  // namespace igs

#endif /* !igs_warp_h */
