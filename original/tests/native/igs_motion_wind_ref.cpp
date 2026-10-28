// Golden harness for the `igs_motion_wind_pixel` + `igs_motion_wind_table` ports.
//
// Compiled against the *original* C++ (extracted from git history by
// `gen_igs_motion_wind_cases.py`). The original RNG (`igs_math_random.h`) already
// forwards into `flare_formats`, so both sides consume the identical LCG stream;
// only the table/pixel arithmetic differs, which is what this test isolates.
//
// Dump format: one `cfg <n>` line, then 400 `px <step> <inside> <a> <b> <c> <d>`
// lines with the four channels as raw IEEE-754 hex. The Rust test
// `igs_motion_wind_pixel::tests::matches_the_cpp_reference` replays the same
// deterministic pixel pattern.
#include <cstdint>
#include <cstdio>
#include <cstring>

#include "igs_motion_wind_pixel.h"

static uint64_t bits(double d) {
  uint64_t u;
  std::memcpy(&u, &d, 8);
  return u;
}

struct Cfg {
  bool dark, alpha;
  unsigned long length_seed, force_seed, density_seed;
  double length_min, length_max, length_bias;
  bool length_ref;
  double force_min, force_max, force_bias;
  bool force_ref;
  double density_min, density_max, density_bias;
  bool density_ref;
};

int main() {
  const Cfg cfgs[] = {
      // arithmetic only (bias 1, force 1): values must match exactly
      {false, true, 1, 1, 1, 2.0, 6.0, 1.0, false, 1.0, 1.0, 1.0, false, 1.0, 1.0, 1.0, false},
      // dark blow + gamma-curved force (pow path)
      {true, true, 7, 7, 7, 1.0, 5.0, 0.5, false, 0.5, 1.5, 1.0, false, 0.8, 1.0, 1.0, false},
      // no alpha blow, length driven by the reference pixel
      {false, false, 3, 3, 3, 3.0, 9.0, 1.0, true, 1.0, 1.0, 1.0, false, 1.0, 1.0, 1.0, false},
  };

  for (int ci = 0; ci < (int)(sizeof(cfgs) / sizeof(cfgs[0])); ++ci) {
    const Cfg &c = cfgs[ci];
    igs::motion_wind::pixel p(
        c.dark, c.alpha, c.length_seed, c.length_min, c.length_max, c.length_bias,
        c.length_ref, c.force_seed, c.force_min, c.force_max, c.force_bias,
        c.force_ref, c.density_seed, c.density_min, c.density_max, c.density_bias,
        c.density_ref);

    std::printf("cfg %d\n", ci);
    for (int step = 0; step < 400; ++step) {
      double px[4];
      for (int ch = 0; ch < 4; ++ch) {
        px[ch] = ((step * 37 + ch * 11) % 101) / 100.0;
      }
      const int inside = p.change((step % 16) == 0, -1.0, 4, px);
      std::printf("px %d %d", step, inside);
      for (int ch = 0; ch < 4; ++ch) {
        std::printf(" %016llx", (unsigned long long)bits(px[ch]));
      }
      std::printf("\n");
    }
    p.clear();
    std::printf("\n");
  }
  return 0;
}
