#pragma once

/*---------------------------------------------------------
 * Interfaccia di predict3d (implemented in predict3d.rs, flare_formats)
 -------------------------------------------------------*/

#ifndef PREDICT3D_H
#define PREDICT3D_H

namespace Predict3D {

struct Point {
  double x, y;
};
}  // namespace Predict3D

extern "C" bool flare_predict3d(int k, const Predict3D::Point *initial,
                                Predict3D::Point *current,
                                const unsigned char *visible);

namespace Predict3D {

// current[i] is filled for every !visible[i]; false on < 4 visible points or a
// degenerate (collinear) configuration.
inline bool Predict(int k, Point initial[], Point current[], bool visible[]) {
  return flare_predict3d(k, initial, current,
                         reinterpret_cast<const unsigned char *>(visible));
}
}  // namespace Predict3D

#endif
