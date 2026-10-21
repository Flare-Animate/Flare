#pragma once

#ifndef igs_motion_wind_h
#define igs_motion_wind_h

#include <stdexcept> /* std::domain_error, std::invalid_argument */
#include <new>       /* std::bad_alloc */

// Implemented in igs_motion_wind.rs (flare_formats); returns 0, or 2 for the
// std::domain_error below, 3 for std::invalid_argument, 4 for std::bad_alloc.
extern "C" {
int flare_igs_motion_wind_change(
    unsigned char *in, int hh, int ww, int cc, int bb, const unsigned char *ref,
    int rh, int rw, int rc, int rb, int ref_mode, int direction,
    bool blow_dark_sw, bool blow_alpha_sw, unsigned long length_random_seed,
    double length_min, double length_max, double length_bias, bool length_ref_sw,
    unsigned long force_random_seed, double force_min, double force_max,
    double force_bias, bool force_ref_sw, unsigned long density_random_seed,
    double density_min, double density_max, double density_bias,
    bool density_ref_sw);
}

namespace igs {
namespace motion_wind {
inline void change(
    unsigned char *in

    ,
    const int hh, const int ww, const int cc, const int bb

    ,
    const unsigned char *ref

    ,
    const int rh, const int rw, const int rc, const int rb

    //, const int rz
    ,
    const int ref_mode /* 0=R,1=G,2=B,3=A,4=Luminance,5=Nothing */

    ,
    const int direction = 0  // 0(LtoR),1(BtoT),2(RtoL),3(TtoB)
    ,
    const bool blow_dark_sw = false  // false,true
    ,
    const bool blow_alpha_sw = true  // false,true

    ,
    const unsigned long length_random_seed = 0  // 0...
    ,
    const double length_min = 0.0, const double length_max = 18.0,
    const double length_bias = 0.0  // ...-1.0...0.0...1.0..
    ,
    const bool length_ref_sw = false  // false,true

    ,
    const unsigned long force_random_seed = 0  // 0...
    ,
    const double force_min = 0.0, const double force_max = 1.0,
    const double force_bias = 0.0  // ...-1.0...0.0...1.0..
    ,
    const bool force_ref_sw = false  // false,true

    ,
    const unsigned long density_random_seed = 0  // 0...
    ,
    const double density_min = 0.0, const double density_max = 1.0,
    const double density_bias = 0.0  // ...-1.0...0.0...1.0..
    ,
    const bool density_ref_sw = false  // false,true
    ) {
  switch (flare_igs_motion_wind_change(
      in, hh, ww, cc, bb, ref, rh, rw, rc, rb, ref_mode, direction,
      blow_dark_sw, blow_alpha_sw, length_random_seed, length_min, length_max,
      length_bias, length_ref_sw, force_random_seed, force_min, force_max,
      force_bias, force_ref_sw, density_random_seed, density_min, density_max,
      density_bias, density_ref_sw)) {
  case 2:
    throw std::domain_error("Bad bits,Not uchar/ushort");
  case 3:
    throw std::invalid_argument("igs::motion_wind::change: bad argument");
  case 4:
    throw std::bad_alloc();
  }
}
}
}

#endif /* !igs_motion_wind_h */
