// Golden harness for the `igs_motion_wind.cpp` port (the scanline walk).
//
// Compiled against the *original* igs_motion_wind.cpp + pixel + table (recovered from
// git history by `gen_igs_motion_wind_change_cases.py`). One output line per case:
// `case <n> <FNV-1a 64 of the output bytes>`. The case matrix and the synthetic
// images are replayed verbatim by `igs_motion_wind::tests::matches_the_cpp_reference`.
#include <cstdint>
#include <cstdio>
#include <vector>

#include "igs_motion_wind.h"

static unsigned val(int bits, int i, int y) {
  if (bits == 8) return (unsigned)((i * 37 + (i / 3) * 11 + y * 5) % 256);
  return (unsigned)((i * 2579 + y * 97 + 13) % 65536);
}

static void fill(std::vector<unsigned char> &buf, int n, int per_row, int bits) {
  const int bytes = bits / 8;
  buf.assign((size_t)n * bytes, 0);
  for (int i = 0; i < n; ++i) {
    unsigned v = val(bits, i, i / per_row);
    buf[(size_t)i * bytes] = (unsigned char)(v & 0xff);
    if (bytes == 2) buf[(size_t)i * bytes + 1] = (unsigned char)(v >> 8);
  }
}

static uint64_t fnv(const std::vector<unsigned char> &b) {
  uint64_t h = 0xcbf29ce484222325ULL;
  for (unsigned char c : b) {
    h ^= c;
    h *= 0x100000001b3ULL;
  }
  return h;
}

int main() {
  const int ww = 11, hh = 7, rw = 5, rh = 4;
  int count = 0;
  for (int cfg = 0; cfg < 2; ++cfg)
    for (int bits = 8; bits <= 16; bits += 8)
      for (int ci = 0; ci < 3; ++ci) {
        const int cc = ci == 0 ? 4 : ci == 1 ? 3 : 1;
        for (int dir = 0; dir < 4; ++dir)
          for (int dark = 0; dark < 2; ++dark)
            for (int alpha = 0; alpha < 2; ++alpha)
              for (int refk = 0; refk < 3; ++refk) {
                std::vector<unsigned char> img, ref;
                fill(img, ww * hh * cc, ww * cc, bits);
                const int rbits = refk == 1 ? 8 : 16;
                if (refk != 0) fill(ref, rw * rh * cc, rw * cc, rbits);
                double lmin = 2.0, lmax = 6.0, dmin = 1.0;
                unsigned long ls = 1, fs = 1, ds = 1;
                bool lref = false;
                if (cfg == 1) {
                  ls = 3; lmin = 3.0; lmax = 9.0; lref = true; fs = 5; ds = 7; dmin = 0.8;
                }
                igs::motion_wind::change(
                    img.data(), hh, ww, cc, bits,
                    refk ? ref.data() : nullptr, refk ? rh : 0, refk ? rw : 0,
                    refk ? cc : 0, refk ? rbits : 0, cc == 4 ? 4 : 0, dir,
                    dark != 0, alpha != 0, ls, lmin, lmax, 1.0, lref, fs, 1.0, 1.0,
                    1.0, false, ds, dmin, 1.0, 1.0, false);
                std::printf("case %d %016llx\n", count++,
                            (unsigned long long)fnv(img));
              }
      }
  return 0;
}
