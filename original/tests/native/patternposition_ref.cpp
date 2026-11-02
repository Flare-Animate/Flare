// Differential reference harness for the `CPatternPosition` port (flarelib/sandor_fxs).
//
// Built against the *original* PatternPosition.h/.cpp (extracted from git; the .cpp is
// compiled through pp_orig.cpp, which redirects its `rand()` to ref_rand below, a fixed
// 64-bit LCG that the Rust test replays). STColSelPic.h is replaced by a minimal mock,
// because the real one pulls in the whole raster stack.
//
// Per case (blank-line separated):
//   C <lX> <lY> <density bits> <minD bits> <maxD bits> <nbPixel> <seed>
//   M <mask bytes, 2 hex digits each>
//   O <n> <x0> <y0> ...
// gen_sandor_fxs_cases.py writes this to flare/sources/flarelib/sandor_fxs/PatternPosition_cases.txt.
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <vector>
#include "PatternPosition.h"

static uint64_t g_rng = 0;
extern "C" int ref_rand(void) {
  g_rng = g_rng * 6364136223846793005ULL + 1442695040888963407ULL;
  return (int)(g_rng >> 33);
}

static uint64_t g_mask = 0x1234567;
static unsigned mrand_() {
  g_mask = g_mask * 6364136223846793005ULL + 1442695040888963407ULL;
  return (unsigned)(g_mask >> 33);
}

static uint64_t bits(double d) {
  uint64_t b;
  memcpy(&b, &d, sizeof b);
  return b;
}

static void run(int lX, int lY, const std::vector<UCHAR> &mask, double dens, double minD,
                double maxD, uint64_t seed) {
  int nbPixel = 0;
  for (UCHAR v : mask) nbPixel += v > 0;
  CSTColSelPic<UC_PIXEL> pic;
  pic.m_lX = lX;
  pic.m_lY = lY;
  pic.m_sel.reset(new UCHAR[mask.size()], std::default_delete<UCHAR[]>());
  memcpy(pic.m_sel.get(), mask.data(), mask.size());
  g_rng = seed;
  CPatternPosition pos(pic, nbPixel, dens, minD, maxD);
  printf("C %d %d %016llx %016llx %016llx %d %llu\nM ", lX, lY, (unsigned long long)bits(dens),
         (unsigned long long)bits(minD), (unsigned long long)bits(maxD), nbPixel,
         (unsigned long long)seed);
  for (UCHAR v : mask) printf("%02x", v);
  printf("\nO %d", (int)pos.m_pos.size());
  for (const SPOINT &p : pos.m_pos) printf(" %d %d", p.x, p.y);
  printf("\n\n");
}

int main() {
  const int sizes[][2] = {{1, 1}, {5, 3}, {16, 16}, {40, 25}, {1, 30}, {30, 1}};
  // mask kinds: 0 full, 1 random ~50%, 2 blobs (values 0/1/255 mixed), 3 empty, 4 one pixel
  const double dens[]  = {0.0, 0.0, 0.0, 0.0, 0.0, 0.05, 0.3, 0.5, 1.0};
  const double dmin[]  = {3.0, 2.0, 1.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0};
  const double dmax[]  = {3.0, 6.0, 1.0, 0.0, 2.5, 1.0, 1.0, 1.0, 1.0};
  int ncase = 0;
  for (auto &sz : sizes)
    for (int kind = 0; kind < 5; kind++)
      for (size_t k = 0; k < sizeof dens / sizeof *dens; k++) {
        int lX = sz[0], lY = sz[1];
        std::vector<UCHAR> m(lX * lY, 0);
        for (int y = 0; y < lY; y++)
          for (int x = 0; x < lX; x++) {
            UCHAR v = 0;
            if (kind == 0) v = 1;
            else if (kind == 1) v = (mrand_() & 1) ? 1 : 0;
            else if (kind == 2) {
              unsigned r = mrand_() % 6;
              v = r < 2 ? 0 : (r < 4 ? 1 : 255);
              if (((x / 4) + (y / 3)) % 2) v = v ? v : 255;
            } else if (kind == 4) v = (x == lX / 2 && y == lY / 2) ? 255 : 0;
            m[y * lX + x] = v;
          }
        run(lX, lY, m, dens[k], dmin[k], dmax[k], 1000 + ncase);
        ncase++;
      }
  // minD > maxD (early out), and a wide-range DD case
  std::vector<UCHAR> full(20 * 20, 1);
  run(20, 20, full, 0.0, 5.0, 2.0, 7);
  run(20, 20, full, 0.0, 1.0, 9.0, 8);
  return 0;
}
