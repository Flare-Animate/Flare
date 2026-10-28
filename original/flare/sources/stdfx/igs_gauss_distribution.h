#pragma once

#ifndef igs_gauss_distribution_h
#define igs_gauss_distribution_h

// Implemented in igs_gauss_distribution.rs (flare_formats).  This header
// replaces the old textual `#include "igs_gauss_distribution.cpp"`.
extern "C" {
void flare_igs_gauss_distribution_1d(double *sequence, int int_diameter,
                                     int int_radius, double real_radius,
                                     double sigma);
}

namespace {
inline void gauss_distribution_1d_(double *sequence,
                                   const int int_diameter /* length of sequence */,
                                   const int int_radius, const double real_radius,
                                   const double sigma /* = 0.25 */
) {
  flare_igs_gauss_distribution_1d(sequence, int_diameter, int_radius,
                                  real_radius, sigma);
}
}  // namespace

#endif /* !igs_gauss_distribution_h */
