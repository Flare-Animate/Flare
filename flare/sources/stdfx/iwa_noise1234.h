#pragma once

/*-------------------------------------------------------------
 iwa_noise1234.h
 Identical to a public domain code "noise1234.h" by Stefan Gustavson.
 Just changed file name for handling of files.
-------------------------------------------------------------*/
/*-- The original code starts here --*/

// Noise1234
// Author: Stefan Gustavson (stegu@itn.liu.se)
//
// This library is public domain software, released by the author
// into the public domain in February 2011. You may do anything
// you like with it. You may even remove all attributions,
// but of course I'd appreciate it if you kept my name somewhere.
//
// This library is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.

/** \file
                \brief Declares the Noise1234 class for producing Perlin noise.
                \author Stefan Gustavson (stegu@itn.liu.se)
*/

/*
 * This is a clean, fast, modern and free Perlin noise class in C++.
 * Being a stand-alone class with no external dependencies, it is
 * highly reusable without source code modifications.
 *
 * Note:
 * Replacing the "float" type with "double" can actually make this run faster
 * on some platforms. A templatized version of Noise1234 could be useful.
 */

// Implemented in iwa_noise1234.rs (flare_formats). Scalar in, scalar out.
extern "C" {
float flare_noise1234_noise1(float x);
float flare_noise1234_noise2(float x, float y);
float flare_noise1234_noise3(float x, float y, float z);
float flare_noise1234_noise4(float x, float y, float z, float w);
float flare_noise1234_pnoise1(float x, int px);
float flare_noise1234_pnoise2(float x, float y, int px, int py);
float flare_noise1234_pnoise3(float x, float y, float z, int px, int py,
                              int pz);
float flare_noise1234_pnoise4(float x, float y, float z, float w, int px,
                              int py, int pz, int pw);
}

class Noise1234 {
public:
  Noise1234() {}
  ~Noise1234() {}

  /** 1D, 2D, 3D and 4D float Perlin noise, SL "noise()"
*/
  static float noise(float x) { return flare_noise1234_noise1(x); }
  static float noise(float x, float y) { return flare_noise1234_noise2(x, y); }
  static float noise(float x, float y, float z) {
    return flare_noise1234_noise3(x, y, z);
  }
  static float noise(float x, float y, float z, float w) {
    return flare_noise1234_noise4(x, y, z, w);
  }

  /** 1D, 2D, 3D and 4D float Perlin periodic noise, SL "pnoise()"
*/
  static float pnoise(float x, int px) {
    return flare_noise1234_pnoise1(x, px);
  }
  static float pnoise(float x, float y, int px, int py) {
    return flare_noise1234_pnoise2(x, y, px, py);
  }
  static float pnoise(float x, float y, float z, int px, int py, int pz) {
    return flare_noise1234_pnoise3(x, y, z, px, py, pz);
  }
  static float pnoise(float x, float y, float z, float w, int px, int py,
                      int pz, int pw) {
    return flare_noise1234_pnoise4(x, y, z, w, px, py, pz, pw);
  }
};
