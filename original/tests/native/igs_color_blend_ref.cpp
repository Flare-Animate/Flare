// Differential reference harness for the `igs_color_blend` port.
//
// Compiled against the *original* C++ (`stdfx/igs_color_blend.cpp`) with MSVC,
// this prints one line per (function, case): the four output channels as raw
// IEEE-754 bit patterns.  `gen_igs_color_blend_cases.py` runs it and writes the
// result to `flare/sources/stdfx/igs_color_blend_cases.txt`, which the Rust unit
// test `igs_color_blend::tests::matches_the_cpp_reference_bit_for_bit` checks
// the port against, bit for bit.
//
// The same case table is mirrored in that Rust test -- keep the two in sync.
#include <cstdio>
#include <cstring>
#include <cstdint>
#include "igs_color_blend.h"

using namespace igs::color;

struct Case {
  double dn[4];
  double up[4];
  double op;
  bool clamp;
};

static const Case CASES[] = {
    {{0, 0, 0, 0}, {0, 0, 0, 0}, 1.0, true},
    {{0, 0, 0, 0}, {0.5, 0.25, 0.75, 1.0}, 1.0, true},
    {{1, 1, 1, 1}, {1, 1, 1, 1}, 1.0, true},
    {{0.5, 0.5, 0.5, 1.0}, {0.5, 0.5, 0.5, 1.0}, 1.0, true},
    {{0.2, 0.4, 0.6, 1.0}, {0.8, 0.6, 0.4, 0.5}, 1.0, true},
    {{0.2, 0.4, 0.6, 0.8}, {0.8, 0.6, 0.4, 0.3}, 0.5, true},
    {{0.9, 0.1, 0.5, 0.7}, {0.1, 0.9, 0.5, 0.7}, 0.25, true},
    {{0.3, 0.3, 0.3, 0.3}, {0.3, 0.3, 0.3, 0.3}, 0.3, false},
    {{1.5, -0.5, 0.5, 1.2}, {0.25, 0.75, 1.5, -0.5}, 1.0, true},
    {{1.5, -0.5, 0.5, 1.2}, {0.25, 0.75, 1.5, -0.5}, 1.0, false},
    {{0.0, 1.0, 0.5, 1.0}, {1.0, 0.0, 0.5, 1.0}, 1.0, true},
    {{0.6, 0.6, 0.6, 1.0}, {0.4, 0.9, 0.1, 1.0}, 1.0, true},
    {{0.24, 0.26, 0.1, 1.0}, {0.6, 0.4, 0.9, 1.0}, 1.0, true},
    {{0.5, 0.5, 0.5, 0.5}, {0.5, 0.5, 0.5, 0.5}, 0.0, true},
    {{0.5, 0.5, 0.5, 0.5}, {0.5, 0.5, 0.5, 0.5}, 0.0, false},
    {{0.0, 0.0, 0.0, 1.0}, {0.5, 0.5, 0.5, 0.5}, 0.6, true},
    {{0.7, 0.2, 0.2, 1.0}, {0.2, 0.7, 0.2, 1.0}, 0.9, true},
    {{0.2, 0.2, 0.7, 1.0}, {0.7, 0.2, 0.2, 1.0}, 0.8, true},
    {{0.5, 0.6, 0.7, 0.6}, {0.6, 0.5, 0.4, 0.4}, 0.5, false},
    {{0.95, 0.05, 0.5, 1.0}, {0.05, 0.95, 0.5, 1.0}, 1.0, true},
};

static uint64_t bits(double d) {
  uint64_t u;
  std::memcpy(&u, &d, 8);
  return u;
}

#define OUT(name)                                                        \
  std::printf("%s %016llx %016llx %016llx %016llx\n", name,              \
              (unsigned long long)bits(r), (unsigned long long)bits(g),  \
              (unsigned long long)bits(b), (unsigned long long)bits(a))

#define TRIAL(name, extra)                          \
  do {                                              \
    double r = c.dn[0], g = c.dn[1], b = c.dn[2], a = c.dn[3]; \
    name(r, g, b, a, c.up[0], c.up[1], c.up[2], c.up[3], c.op, extra); \
    OUT(#name);                                     \
  } while (0)

int main() {
  for (int ci = 0; ci < (int)(sizeof(CASES) / sizeof(CASES[0])); ++ci) {
    const Case &c = CASES[ci];
    bool alpha_sw = (ci & 1) != 0;
    TRIAL(over, c.clamp);
    TRIAL(darken, c.clamp);
    TRIAL(multiply, c.clamp);
    TRIAL(color_burn, c.clamp);
    TRIAL(linear_burn, c.clamp);
    TRIAL(darker_color, c.clamp);
    TRIAL(lighten, c.clamp);
    TRIAL(screen, c.clamp);
    TRIAL(color_dodge, c.clamp);
    TRIAL(linear_dodge, c.clamp);
    TRIAL(lighter_color, c.clamp);
    TRIAL(overlay, c.clamp);
    TRIAL(soft_light, c.clamp);
    TRIAL(hard_light, c.clamp);
    TRIAL(vivid_light, c.clamp);
    TRIAL(linear_light, c.clamp);
    TRIAL(pin_light, c.clamp);
    TRIAL(hard_mix, c.clamp);
    TRIAL(cross_dissolve, c.clamp);
    TRIAL(add, c.clamp);
    TRIAL(divide, c.clamp);
    {
      double r = c.dn[0], g = c.dn[1], b = c.dn[2], a = c.dn[3];
      subtract(r, g, b, a, c.up[0], c.up[1], c.up[2], c.up[3], c.op, alpha_sw,
               c.clamp);
      OUT("subtract");
    }
  }
  return 0;
}
