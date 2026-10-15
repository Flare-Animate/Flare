#pragma once

#ifndef TRANDOM_INCLUDED
#define TRANDOM_INCLUDED

#include "tcommon.h"

class TRandom;

// Implemented in trandom.rs (flare_formats); TRandom is header-only and forwards
// to it with `this` (the Rust struct mirrors the C++ member layout).
extern "C" {
void flare_trandom_reset(TRandom *r);
void flare_trandom_set_seed(TRandom *r, UINT seed);
UINT flare_trandom_get_uint(TRandom *r, UINT end);
int flare_trandom_get_int(TRandom *r, int begin, int end);
float flare_trandom_get_float(TRandom *r);
float flare_trandom_get_float_end(TRandom *r, float end);
float flare_trandom_get_float_range(TRandom *r, float begin, float end);
bool flare_trandom_get_bool(TRandom *r);
double flare_trandom_get_double(TRandom *r);
}

/*! Generates a pseudorandom sequence
*/
class TRandom {
public:
  TRandom(UINT _seed = 0) : seed(_seed) { reset(); }
  ~TRandom() {}

  /*! resets the pseudorandom engine to the first number of the sequence */
  void reset() { flare_trandom_reset(this); }

  /*! set a new seed for the pseudorandom engine and reset the sequence */
  void setSeed(UINT s) { flare_trandom_set_seed(this, s); }

  /*! returns an unsigned integer number in the range [0, end[ */
  UINT getUInt(UINT end = c_maxuint) {  // [0,end[
    return flare_trandom_get_uint(this, end);
  }

  /*! returns an integer number in the range [begin, end[ */
  int getInt(int begin, int end) {
    return flare_trandom_get_int(this, begin, end);
  }

  /*! returns a float number in the range [0, 1[ */
  float getFloat() { return flare_trandom_get_float(this); }

  /*! returns a float number in the range [0, end[ */
  float getFloat(float end) { return flare_trandom_get_float_end(this, end); }

  /*! returns a float number in the range [begin, end[ */
  float getFloat(float begin, float end) {
    return flare_trandom_get_float_range(this, begin, end);
  }

  /*! returns a random bool value */
  bool getBool() { return flare_trandom_get_bool(this); }

  /*! returns a double number in the range [0, 1[ */
  double getDouble() { return flare_trandom_get_double(this); }

private:
  // Layout is shared with the Rust `TRandom` in trandom.rs (do not reorder).
  UINT seed;
  int idx1, idx2;
  UINT ran[56];
};

#endif  // TRANDOM_INCLUDED
