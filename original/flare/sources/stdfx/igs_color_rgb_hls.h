#pragma once

#ifndef igs_color_rgb_hls_h
#define igs_color_rgb_hls_h

// Implemented in igs_color_rgb_hls.rs (flare_formats).
extern "C" {
void flare_igs_rgb_to_hls(double red, double gre, double blu, double *hue,
                          double *lig, double *sat, bool cylindrical);
void flare_igs_hls_to_rgb(double hue, double lig, double sat, double *red,
                          double *gre, double *blu, bool cylindrical);
}

namespace igs {
namespace color {
// Use add_hls & noise_hlsa
inline void rgb_to_hls(const double red, const double gre, const double blu,
                       double &hue, double &lig, double &sat,
                       bool cylindrical = true) {
  flare_igs_rgb_to_hls(red, gre, blu, &hue, &lig, &sat, cylindrical);
}
inline void hls_to_rgb(const double hue, const double lig, const double sat,
                       double &red, double &gre, double &blu,
                       bool cylindrical = true) {
  flare_igs_hls_to_rgb(hue, lig, sat, &red, &gre, &blu, cylindrical);
}
}  // namespace color
}  // namespace igs

#endif /* !igs_color_rgb_hls_h */
