// Golden harness for the `igs_attenuation_distribution` port.
//
// Compiled against the *original* C++ (extracted from git history by
// `gen_igs_attenuation_cases.py`, which also supplies the archived
// `igs_attenuation_distribution.cpp` next to this file). The original's helpers live
// in an anonymous namespace, so including the .cpp here cannot collide with the
// Rust FFI in `libflare_formats`.
//
// Dump format (one block per case, blank line between blocks):
//   case <radius> <curve> <polygon> <degree> diam <diam>
//   row <y> offset <o> size <s> start_delta <d>      # d = lens_starts[y] - &matrix[y][0]
//   m <016llx>                                        # matrix[y][x] bits, row-major
// The Rust test `igs_attenuation_distribution::tests::matches_the_cpp_reference`
// replays this table. Structural fields (diam/offset/size/start_delta) are compared
// exactly; matrix values within a few ULP because `sqrt`/`pow`/`cos`/`sin` differ
// between MSVC's CRT and the host libm.
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <vector>

#include "igs_attenuation_distribution.cpp"

static uint64_t bits(double d) {
  uint64_t u;
  std::memcpy(&u, &d, 8);
  return u;
}

static void run_case(double radius, double curve, int polygon, double degree) {
  std::vector<std::vector<double>> lens_matrix;
  std::vector<int> lens_offsets;
  std::vector<double *> lens_starts;
  std::vector<int> lens_sizes;
  int odd_diameter = 0;

  attenuation_distribution_(lens_matrix, lens_offsets, lens_starts, lens_sizes,
                            odd_diameter, radius, curve, polygon, degree);

  std::printf("case %.17g %.17g %d %.17g diam %d\n", radius, curve, polygon,
              degree, odd_diameter);
  for (int y = 0; y < odd_diameter; ++y) {
    // start_delta pins "lens_starts points at the first covered pixel".
    long delta = (lens_starts[y] != 0) ? (lens_starts[y] - &lens_matrix[y][0]) : -1;
    std::printf("row %d offset %d size %d start_delta %ld\n", y,
                lens_offsets[y], lens_sizes[y], delta);
  }
  for (int y = 0; y < odd_diameter; ++y) {
    for (int x = 0; x < odd_diameter; ++x) {
      std::printf("m %016llx\n", (unsigned long long)bits(lens_matrix[y][x]));
    }
  }
  std::printf("\n");
}

int main() {
  const double radii[] = {1.01, 2.0, 3.5, 5.0, 8.9};
  const double curves[] = {0.5, 1.0, 2.5};
  const int polys[] = {2, 6};
  const double degrees[] = {0.0, 90.0};

  for (double r : radii) {
    for (double c : curves) {
      for (int p : polys) {
        for (double d : degrees) {
          run_case(r, c, p, d);
        }
      }
    }
  }

  // Error paths: curve == 0 and diameter <= 1 both throw in the original.
  for (int which = 0; which < 2; ++which) {
    try {
      std::vector<std::vector<double>> m;
      std::vector<int> off, sz;
      std::vector<double *> st;
      int diam = 0;
      if (which == 0) {
        attenuation_distribution_(m, off, st, sz, diam, 5.0, 0.0, 2, 0.0);
      } else {
        attenuation_distribution_(m, off, st, sz, diam, 0.4, 1.0, 2, 0.0);
      }
      std::printf("err %d none\n\n", which);
    } catch (const std::exception &e) {
      std::printf("err %d %s\n\n", which, e.what());
    }
  }
  return 0;
}
