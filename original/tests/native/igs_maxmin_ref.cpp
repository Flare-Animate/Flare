// Golden harness for the `igs_maxmin*.cpp` ports.
//
// Compiled against the *original* igs_maxmin.cpp, igs_maxmin_lens_matrix.cpp and
// igs_maxmin_slrender.cpp (+ the headers they use, recovered from git history by
// `gen_igs_maxmin_cases.py`) and the live igs_resource_multithread on pthreads.
//   lens/rlens: the lens matrix of each configuration, fresh (alloc_and_shape) and
//               reshaped to 0.6x the radius; weights as raw IEEE-754 hex, only the
//               `size` used entries of each row
//   e2e:        public convert() on synthetic images, FNV-1a 64 of the output bytes
// Replayed by `igs_maxmin::tests::matches_the_cpp_reference`.
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <vector>

#include "igs_maxmin.h"
#ifndef CHAIN
#include "igs_maxmin_lens_matrix.h"
#endif

struct Cfg {
  double radius, smooth;
  int poly;
  double roll;
  bool min_sw, alpha_sw;
  int threads;
};

static unsigned val(int bits, int i, int y) {
  if (bits == 16) return (unsigned)((i * 2579 + y * 97 + 13) % 65536);
  return (unsigned)((i * 37 + (i / 3) * 11 + y * 5) % 256);
}

static uint64_t fnv(const std::vector<unsigned char> &b) {
  uint64_t h = 0xcbf29ce484222325ULL;
  for (unsigned char c : b) {
    h ^= c;
    h *= 0x100000001b3ULL;
  }
  return h;
}

#ifndef CHAIN
static void dump_rows(const std::vector<int> &offs, const std::vector<int> &sizes,
                      const std::vector<std::vector<double>> &ratio) {
  for (size_t yy = 0; yy < offs.size(); ++yy) {
    std::printf("row %zu %d %d", yy, offs[yy], sizes[yy]);
    for (int i = 0; i < sizes[yy]; ++i) {
      uint64_t u;
      std::memcpy(&u, &ratio[yy][i], 8);
      std::printf(" %016llx", (unsigned long long)u);
    }
    std::printf("\n");
  }
}

#endif

int main() {
  const Cfg cfgs[] = {
      {1.0, 2.0, 2, 0.0, false, true, 1},  {2.5, 1.0, 2, 0.0, false, true, 1},
      {2.5, 1.0, 2, 0.0, true, true, 2},   {3.0, 0.0, 2, 0.0, false, false, 3},
      {0.5, 2.0, 2, 0.0, false, true, 1},  {0.0, 1.0, 2, 0.0, false, true, 1},
      {2.0, 1.5, 4, 0.0, false, true, 1},  {2.0, 1.5, 5, 30.0, true, true, 2},
      {3.0, 1.0, 6, 180.0, false, true, 1}, {2.2, 2.0, 3, 45.0, false, true, 3},
      {1.5, 0.5, 8, 22.5, true, false, 1}, {2.0, 1.0, 2, 0.0, false, true, 100},
      {4.0, 1.0, 2, 0.0, true, true, 4},   {1.0, 0.0, 16, 0.0, false, true, 1},
  };
  const int ncfg = (int)(sizeof(cfgs) / sizeof(cfgs[0]));

#ifndef CHAIN  // the lens API only exists in the original
  for (int ci = 0; ci < ncfg; ++ci) {
    const Cfg &c = cfgs[ci];
    std::vector<int> offs, sizes;
    std::vector<std::vector<double>> ratio;
    const int odd = igs::maxmin::alloc_and_shape_lens_matrix(
        c.radius, c.radius + c.smooth, c.poly, c.roll, offs, sizes, ratio);
    std::printf("lens %d %d\n", ci, odd);
    dump_rows(offs, sizes, ratio);
    std::printf("rlens %d %d\n", ci, odd);
    if (odd > 0) {
      const double r2 = c.radius * 0.6;
      igs::maxmin::reshape_lens_matrix(
          r2, igs::maxmin::outer_radius_from_radius(r2, c.smooth), odd, c.poly,
          c.roll, offs, sizes, ratio);
      dump_rows(offs, sizes, ratio);
    }
  }
#endif

  const int w = 14, h = 10;
  struct Img {
    int bits, cc;
  };
  const Img imgs[] = {{8, 4}, {8, 3}, {8, 1}, {16, 4}, {16, 3}, {24, 4}, {24, 1}};
  auto run = [&](int ci, int bits, int cc, int rbits, int rmode) {
    const Cfg &c = cfgs[ci];
    const int n = w * h * cc;
    const int bytes = bits == 8 ? 1 : bits == 16 ? 2 : 4;
    std::vector<unsigned char> in((size_t)n * bytes), out((size_t)n * bytes, 0);
    for (int i = 0; i < n; ++i) {
      const int y = i / (w * cc);
      if (bits == 8) {
        in[i] = (unsigned char)val(8, i, y);
      } else if (bits == 16) {
        unsigned v = val(16, i, y);
        in[2 * i] = v & 0xff;
        in[2 * i + 1] = v >> 8;
      } else {
        float f = (float)val(8, i, y) / 255.0f;
        std::memcpy(&in[4 * i], &f, 4);
      }
    }
    std::vector<unsigned char> ref;
    if (rbits == 8) {
      ref.resize(n);
      for (int i = 0; i < n; ++i) ref[i] = (unsigned char)((val(8, i, i / (w * cc)) * 7 + 3) % 256);
    } else if (rbits == 16) {
      ref.resize((size_t)n * 2);
      for (int i = 0; i < n; ++i) {
        unsigned v = (val(16, i, i / (w * cc)) * 5 + 1) % 65536;
        ref[2 * i] = v & 0xff;
        ref[2 * i + 1] = v >> 8;
      }
    }
    // threads: the original races on its shared lens for RGBA / reference input
    const int nt = (cc == 4 || rbits != 0) ? 1 : c.threads;
    igs::maxmin::convert(in.data(), out.data(), h, w, cc, bits,
                         rbits ? ref.data() : nullptr, rbits, rmode, c.radius,
                         c.smooth, c.poly, c.roll, c.min_sw, c.alpha_sw, false, nt);
    std::printf("e2e %d %d %d %d %d %016llx\n", ci, bits, cc, rbits, rmode,
                (unsigned long long)fnv(out));
  };
  for (int ci = 0; ci < ncfg; ++ci)
    for (const Img &im : imgs) run(ci, im.bits, im.cc, 0, 5);
  const int ref_cfgs[] = {1, 2, 6, 7, 10, 12};
  for (int ci : ref_cfgs) {
    run(ci, 8, 4, 8, 4);
    run(ci, 16, 4, 16, 0);
    run(ci, 8, 3, 8, 3);
  }
  return 0;
}
