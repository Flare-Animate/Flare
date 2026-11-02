#pragma once

#ifndef HSVUTIL_H
#define HSVUTIL_H

// Implemented in hsvutil.rs (flare_formats).
extern "C" {
void flare_old_rgb2hsv(double r, double g, double b, double *h, double *s,
                       double *v);
void flare_old_hsv2rgb(double hue, double sat, double value, double *red,
                       double *green, double *blue);
}

inline void OLDRGB2HSV(double r, double g, double b, double *h, double *s,
                       double *v) {
  flare_old_rgb2hsv(r, g, b, h, s, v);
}

inline void OLDHSV2RGB(double hue, double sat, double value, double *red,
                       double *green, double *blue) {
  flare_old_hsv2rgb(hue, sat, value, red, green, blue);
}

#endif
