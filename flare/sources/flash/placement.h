#pragma once

#include "geometry.h"

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
StagePlacement stagePlacement(const Affine &matrix, double dpi,
                              Point sourcePivot = {});

}  // namespace Flash
