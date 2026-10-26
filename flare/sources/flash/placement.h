#pragma once

#include "geometry.h"

// Implemented in flash/placement.rs (flare_formats).
extern "C" int flare_flash_stage_placement(const double *matrix, double dpi,
                                           double pivotX, double pivotY,
                                           double *out);

namespace Flash {

// Values consumed by Flare stage objects: translation and pivot in
// inches, rotation in degrees, scale and shear as factors. Local drawing
// geometry is reflected and scaled separately by Stage::inch / dpi.
struct StagePlacement {
  Point position;
  Point center;
  double angle  = 0;
  double scaleX = 1, scaleY = 1;
  double shearX = 0, shearY = 0;
};

// Keeps the source affine exact while exposing ordinary editable native
// controls. Singular transforms are legal (e.g. a zero-scale animation key).
inline StagePlacement stagePlacement(const Affine &matrix, double dpi,
                                     Point sourcePivot = {}) {
  const double m[6] = {matrix.a, matrix.b, matrix.c, matrix.d, matrix.tx, matrix.ty};
  double o[9];
  switch (flare_flash_stage_placement(m, dpi, sourcePivot.x, sourcePivot.y, o)) {
  case 0: break;
  case 1: throw Error("Invalid XFL transformation matrix.");
  case 2: throw Error("Invalid native placement scale or pivot.");
  default: throw Error("Native placement exceeds finite coordinate limits.");
  }
  StagePlacement result;
  result.position = {o[0], o[1]};
  result.center   = {o[2], o[3]};
  result.angle    = o[4];
  result.scaleX   = o[5];
  result.scaleY   = o[6];
  result.shearX   = o[7];
  result.shearY   = o[8];
  return result;
}

}  // namespace Flash
