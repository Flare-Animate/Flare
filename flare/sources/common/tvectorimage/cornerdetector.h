#pragma once

#ifndef CORNERDETECTOR_H
#define CORNERDETECTOR_H

#include <algorithm>
#include <vector>
#include "tgeometry.h"

// The detector lives in cornerdetector.rs (flare_formats). T3DPointD is three
// consecutive doubles (x, y, z). Returns the number of corner indexes (<= n),
// writing at most `cap` of them; -1 on bad arguments.
extern "C" {
long long flare_detect_corners(const double *points, size_t n,
                               int minSampleNum, int minDist, int maxDist,
                               double maxAngle, int *out, size_t cap);
}

// Appends the corner indexes to cornerIndexes, then drops any index equal to
// its predecessor over the whole vector, as the original did.
inline void detectCorners(const std::vector<T3DPointD> &inputPoints,
                          int minSampleNum, int minDist, int maxDist,
                          double maxAngle, std::vector<int> &cornerIndexes) {
  static_assert(sizeof(T3DPointD) == 3 * sizeof(double),
                "T3DPointD must be x,y,z doubles");
  if (!inputPoints.empty()) {
    std::vector<int> found(inputPoints.size());
    long long n = flare_detect_corners(
        &inputPoints[0].x, inputPoints.size(), minSampleNum, minDist, maxDist,
        maxAngle, found.data(), found.size());
    if (n > 0)
      cornerIndexes.insert(cornerIndexes.end(), found.begin(),
                           found.begin() + std::min<long long>(n, found.size()));
  }
  cornerIndexes.erase(std::unique(cornerIndexes.begin(), cornerIndexes.end()),
                      cornerIndexes.end());
}

#endif  // CORNERDETECTOR_H
