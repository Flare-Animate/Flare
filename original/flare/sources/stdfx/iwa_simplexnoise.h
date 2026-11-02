#pragma once

/*--------------------------------------------------------------------
 Simplex Noise C++ implementation
 Based on a public domain code by Stefan Gustavson.
 (The original header is below)
--------------------------------------------------------------------*/
/*-- start of the original header --*/
/*
* A speed-improved simplex noise algorithm for 2D, 3D and 4D in Java.
*
* Based on example code by Stefan Gustavson (stegu@itn.liu.se).
* Optimisations by Peter Eastman (peastman@drizzle.stanford.edu).
* Better rank ordering method by Stefan Gustavson in 2012.
*
* This could be speeded up even further, but it's useful as it is.
*
* Version 2012-03-09
*
* This code was placed in the public domain by its original author,
* Stefan Gustavson. You may use it as you see fit, but
* attribution is appreciated.
*
*/
/*-- end of the original header --*/

#ifndef IWA_SIMPLEXNOISE_H
#define IWA_SIMPLEXNOISE_H

struct Grad {
  int x, y, z, w;
  Grad(int x_, int y_, int z_) : x(x_), y(y_), z(z_), w(0) {}
  Grad(int x_, int y_, int z_, int w_) : x(x_), y(y_), z(z_), w(w_) {}
};

struct CellIds {
  int i, j, k;
  int i1, j1, k1;
  int i2, j2, k2;
  CellIds(int _i = 0, int _j = 0, int _k = 0, int _i1 = 0, int _j1 = 0,
          int _k1 = 0, int _i2 = 0, int _j2 = 0, int _k2 = 0) {
    i  = _i;
    j  = _j;
    k  = _k;
    i1 = _i1;
    j1 = _j1;
    k1 = _k1;
    i2 = _i2;
    j2 = _j2;
    k2 = _k2;
  }

  bool operator==(const CellIds &right) const {
    return i == right.i && j == right.j && k == right.k && i1 == right.i1 &&
           j1 == right.j1 && k1 == right.k1 && i2 == right.i2 &&
           j2 == right.j2 && k2 == right.k2;
  }
};

// Implemented in iwa_simplexnoise.rs (flare_formats).
extern "C" {
double flare_simplex_noise2(double x, double y);
double flare_simplex_noise3(double x, double y, double z);
double flare_simplex_noise4(double x, double y, double z, double w);
// writes i, j, k, i1, j1, k1, i2, j2, k2; returns 0 (-1 for a null `out`)
int flare_simplex_cell_ids(double x, double y, double z, int *out);
}

class SimplexNoise {
public:
  SimplexNoise();
  // 2D simplex noise
  static double noise(double xin, double yin) {
    return flare_simplex_noise2(xin, yin);
  }
  // 3D simplex noise
  static double noise(double xin, double yin, double zin) {
    return flare_simplex_noise3(xin, yin, zin);
  }
  // 4D simplex noise
  static double noise(double x, double y, double z, double w) {
    return flare_simplex_noise4(x, y, z, w);
  }

  /*- セルまたぎを防ぐために、現在の所属セルを得る -*/
  static CellIds getCellIds(double xin, double yin, double zin) {
    int v[9] = {0, 0, 0, 0, 0, 0, 0, 0, 0};
    flare_simplex_cell_ids(xin, yin, zin, v);
    return CellIds(v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8]);
  }
};

#endif
