#pragma once

#ifndef igs_color_blend_h
#define igs_color_blend_h

// Blend-mode kernels (Photoshop / PDF blend modes) used by the `ino_blend_*`
// FX.  The implementation live in Rust (`igs_color_blend.rs`, ported from the
// former `igs_color_blend.cpp`); this header only forwards the original
// `igs::color::*` API to the C ABI so callers need no changes.
//
// Each `flare_igs_color_<name>` entry takes `dn` pointing at four doubles
// (r, g, b, a) which are updated in place.

extern "C" {
#define FLARE_IGS_COLOR_DECL(NAME)                       \
  void flare_igs_color_##NAME(double *dn, double up_r,   \
                              double up_g, double up_b,  \
                              double up_a, double up_opacity, int do_clamp);

FLARE_IGS_COLOR_DECL(over)         // 01
FLARE_IGS_COLOR_DECL(darken)       // 02
FLARE_IGS_COLOR_DECL(multiply)     // 03
FLARE_IGS_COLOR_DECL(color_burn)   // 04
FLARE_IGS_COLOR_DECL(linear_burn)  // 05
FLARE_IGS_COLOR_DECL(darker_color) // 06
FLARE_IGS_COLOR_DECL(lighten)      // 07
FLARE_IGS_COLOR_DECL(screen)       // 08
FLARE_IGS_COLOR_DECL(color_dodge)  // 09
FLARE_IGS_COLOR_DECL(linear_dodge) // 10
FLARE_IGS_COLOR_DECL(lighter_color) // 11
FLARE_IGS_COLOR_DECL(overlay)      // 12
FLARE_IGS_COLOR_DECL(soft_light)   // 13
FLARE_IGS_COLOR_DECL(hard_light)   // 14
FLARE_IGS_COLOR_DECL(vivid_light)  // 15
FLARE_IGS_COLOR_DECL(linear_light) // 16
FLARE_IGS_COLOR_DECL(pin_light)    // 17
FLARE_IGS_COLOR_DECL(hard_mix)     // 18
FLARE_IGS_COLOR_DECL(cross_dissolve) // 19
FLARE_IGS_COLOR_DECL(add)          // 21
FLARE_IGS_COLOR_DECL(divide)       // 22
#undef FLARE_IGS_COLOR_DECL

// 20: `subtract` carries the extra `alpha_rendering_sw` flag.
void flare_igs_color_subtract(double *dn, double up_r, double up_g, double up_b,
                              double up_a, double up_opacity,
                              int alpha_rendering_sw, int do_clamp);
}  // extern "C"

namespace igs {
namespace color {

// The 21 kernels that share the plain signature.
#define FLARE_IGS_COLOR_FWD(NAME)                                             \
  inline void NAME(double &dn_r, double &dn_g, double &dn_b, double &dn_a,    \
                   const double up_r, double up_g, double up_b, double up_a,  \
                   const double up_opacity, const bool do_clamp = true) {     \
    double dn[4] = {dn_r, dn_g, dn_b, dn_a};                                  \
    flare_igs_color_##NAME(dn, up_r, up_g, up_b, up_a, up_opacity,            \
                           do_clamp ? 1 : 0);                                 \
    dn_r = dn[0];                                                             \
    dn_g = dn[1];                                                             \
    dn_b = dn[2];                                                             \
    dn_a = dn[3];                                                             \
  }

FLARE_IGS_COLOR_FWD(over)         // 01 Normal
FLARE_IGS_COLOR_FWD(darken)       // 02 Darken
FLARE_IGS_COLOR_FWD(multiply)     // 03 Multiply
FLARE_IGS_COLOR_FWD(color_burn)   // 04 Color Burn
FLARE_IGS_COLOR_FWD(linear_burn)  // 05 Linear Burn
FLARE_IGS_COLOR_FWD(darker_color) // 06 Darker Color
FLARE_IGS_COLOR_FWD(lighten)      // 07 Lighten
FLARE_IGS_COLOR_FWD(screen)       // 08 Screen
FLARE_IGS_COLOR_FWD(color_dodge)  // 09 Color Dodge
FLARE_IGS_COLOR_FWD(linear_dodge) // 10 Linear Dodge
FLARE_IGS_COLOR_FWD(lighter_color) // 11 Lighter Color
FLARE_IGS_COLOR_FWD(overlay)      // 12 Overlay
FLARE_IGS_COLOR_FWD(soft_light)   // 13 Soft Light
FLARE_IGS_COLOR_FWD(hard_light)   // 14 Hard Light
FLARE_IGS_COLOR_FWD(vivid_light)  // 15 Vivid Light
FLARE_IGS_COLOR_FWD(linear_light) // 16 Linear Light
FLARE_IGS_COLOR_FWD(pin_light)    // 17 Pin Light
FLARE_IGS_COLOR_FWD(hard_mix)     // 18 Hard Mix
FLARE_IGS_COLOR_FWD(cross_dissolve) // 19 Cross Dissolve
FLARE_IGS_COLOR_FWD(add)          // 21 Add
FLARE_IGS_COLOR_FWD(divide)       // 22 Divide
#undef FLARE_IGS_COLOR_FWD

// 20 Subtract
inline void subtract(double &dn_r, double &dn_g, double &dn_b, double &dn_a,
                     const double up_r, double up_g, double up_b, double up_a,
                     const double up_opacity, const bool alpha_rendering_sw,
                     const bool do_clamp = true) {
  double dn[4] = {dn_r, dn_g, dn_b, dn_a};
  flare_igs_color_subtract(dn, up_r, up_g, up_b, up_a, up_opacity,
                           alpha_rendering_sw ? 1 : 0, do_clamp ? 1 : 0);
  dn_r = dn[0];
  dn_g = dn[1];
  dn_b = dn[2];
  dn_a = dn[3];
}

}  // namespace color
}  // namespace igs
#endif /* !igs_color_blend_h */
