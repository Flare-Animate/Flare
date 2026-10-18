// Contract probe for the C++ wrapper in igs_attenuation_distribution.h.
//
// TRANSITIONAL FFI — delete this probe when `stdfx/igs_fog.cpp` is ported to Rust:
// it exercises the inline wrapper, so it goes away with `igs_attenuation_distribution.h`
// and the two `flare_igs_attenuation_*` exports. The golden test stays.
//
// The Rust golden (`igs_attenuation_cases.txt`) encodes the *original* C++ behaviour,
// including where `lens_starts[y]` points. The Rust replay test therefore cannot prove
// the wrapper itself is correct -- that check has to run against the wrapper. This probe
// does that: it asserts the contract the wrapper must uphold after the port:
//
//   * lens_starts[y] == &lens_matrix[y][0] + lens_offsets[y]   (first covered pixel)
//   * lens_starts[y] == nullptr                               (rows with no coverage)
//   * lens_offsets[y] + lens_sizes[y] <= odd_diameter          (run stays in the row)
//   * the matrix sums to 1.0                                   (normalisation)
//   * curve == 0 and diameter <= 1 surface as std::domain_error
//
// These are exactly the properties the port got wrong once (lens_starts pointed at the
// row start, shifting every fog sample). Run it after touching the header:
//
//   clang++ -O2 -std=c++17 -ffp-contract=off -I flare/sources/stdfx \
//       tests/native/igs_attenuation_wrapper_probe.cpp \
//       build/libflare_formats.a -o /tmp/att_wrapper_probe && /tmp/att_wrapper_probe
//
// Exit code 0 = contract holds. It is a probe, not a suite target: tests/native/CMakeLists
// is MSVC/tnzcore-only, so nothing in CI runs it yet.
#include "igs_attenuation_distribution.h"

#include <cmath>
#include <cstdio>
#include <stdexcept>
#include <vector>

int main() {
  int fails = 0;

  for (double r : {1.01, 2.0, 5.0, 8.9}) {
    for (int poly : {2, 6}) {
      std::vector<std::vector<double>> m;
      std::vector<int> off, sz;
      std::vector<double *> st;
      int diam = 0;

      attenuation_distribution_(m, off, st, sz, diam, r, 1.0, poly, 0.0);

      for (int y = 0; y < diam; ++y) {
        if (off[y] < 0) {
          if (st[y] != nullptr) {
            std::printf("FAIL r=%.2f y=%d: empty row is not null\n", r, y);
            ++fails;
          }
        } else if (st[y] != &m[y][0] + off[y]) {
          std::printf("FAIL r=%.2f y=%d: starts at row+%td, expected row+%d\n", r, y,
                      st[y] - &m[y][0], off[y]);
          ++fails;
        }
        if (off[y] >= 0 && off[y] + sz[y] > diam) {
          std::printf("FAIL r=%.2f y=%d: offset+size runs past the row\n", r, y);
          ++fails;
        }
      }

      double total = 0;
      for (const auto &row : m) {
        for (double v : row) total += v;
      }
      if (std::fabs(total - 1.0) > 1e-12) {
        std::printf("FAIL r=%.2f: matrix sums to %f\n", r, total);
        ++fails;
      }
    }
  }

  try {
    std::vector<std::vector<double>> m;
    std::vector<int> off, sz;
    std::vector<double *> st;
    int diam = 0;
    attenuation_distribution_(m, off, st, sz, diam, 5.0, 0.0, 2, 0.0);
    std::printf("FAIL: curve == 0 did not throw\n");
    ++fails;
  } catch (const std::domain_error &) {
  }

  try {
    std::vector<std::vector<double>> m;
    std::vector<int> off, sz;
    std::vector<double *> st;
    int diam = 0;
    attenuation_distribution_(m, off, st, sz, diam, 0.4, 1.0, 2, 0.0);
    std::printf("FAIL: diameter <= 1 did not throw\n");
    ++fails;
  } catch (const std::domain_error &) {
  }

  if (fails == 0) {
    std::printf("PROBE OK: lens_starts, sizes, normalisation and error mapping hold\n");
  }
  return fails != 0;
}
