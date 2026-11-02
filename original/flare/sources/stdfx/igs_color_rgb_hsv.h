#pragma once

#ifndef igs_color_rgb_hsv_h
#define igs_color_rgb_hsv_h

// Implemented in igs_color_rgb_hsv.rs (flare_formats).
extern "C" {
void flare_igs_rgb_to_hsv(double red, double gre, double blu, double *hue,
                          double *sat, double *val);
void flare_igs_hsv_to_rgb(double hue, double sat, double val, double *red,
                          double *gre, double *blu);
}

namespace igs {
namespace color {
// Use add_hsv & noise_hsva
inline void rgb_to_hsv(const double red, const double gre, const double blu,
                       double &hue, double &sat, double &val) {
  flare_igs_rgb_to_hsv(red, gre, blu, &hue, &sat, &val);
}
inline void hsv_to_rgb(const double hue, const double sat, const double val,
                       double &red, double &gre, double &blu) {
  flare_igs_hsv_to_rgb(hue, sat, val, &red, &gre, &blu);
}
}
}

#endif /* !igs_color_rgb_hsv_h */
