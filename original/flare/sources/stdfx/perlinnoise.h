#pragma once

#ifndef PERLINNOISE_H
#define PERLINOISE_H

#include <cstddef>
#include <memory>

#include "tfxparam.h"
#include "tspectrumparam.h"

enum { PNOISE_CLOUDS, PNOISE_WOODS };

// Implemented in perlinnoise.rs (flare_formats). NaN for a bad argument.
extern "C" {
double flare_perlin_linear_noise(const float *noise, size_t len, int size,
                                 int tsize, double x, double y, double t);
double flare_perlin_eval(const float *noise, size_t len, int size, int tsize,
                         int offset, int kind, double u, double v, double k,
                         double grain, int ranged, double min, double max);
}

class PerlinNoise {
  static int Size;
  static int TimeSize;
  static int Offset;
  static double Pixel_size;
  std::unique_ptr<float[]> Noise;
  size_t tableLen() const { return (size_t)Size * Size * TimeSize; }
  double LinearNoise(double x, double y, double t) {
    return flare_perlin_linear_noise(Noise.get(), tableLen(), Size, TimeSize, x,
                                     y, t);
  }
  double eval(int kind, double u, double v, double k, double grain, int ranged,
              double min, double max) {
    Pixel_size = 0.05;  // the original stored it on every call
    return flare_perlin_eval(Noise.get(), tableLen(), Size, TimeSize, Offset,
                             kind, u, v, k, grain, ranged, min, max);
  }

public:
  PerlinNoise();
  double Turbolence(double u, double v, double k, double grain) {
    return eval(0, u, v, k, grain, 0, 0.0, 1.0);
  }
  double Turbolence(double u, double v, double k, double grain, double min,
                    double max) {
    return eval(0, u, v, k, grain, 1, min, max);
  }
  double Marble(double u, double v, double k, double grain) {
    return eval(1, u, v, k, grain, 0, 0.0, 1.0);
  }
  double Marble(double u, double v, double k, double grain, double min,
                double max) {
    return eval(1, u, v, k, grain, 1, min, max);
  }
};
/*---------------------------------------------------------------------------*/
void doClouds(const TRasterP &ras, const TSpectrumParamP colors, TPointD pos,
              double evolution, double size, double min, double max, int type,
              double scale, double frame);

#endif
