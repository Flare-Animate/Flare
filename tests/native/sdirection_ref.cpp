// Differential reference harness for the `CSDirection` port (flarelib/sandor_fxs): the
// original SDirection.h/.cpp, extracted from git, driven through its public interface.
//
// Per case (blank-line separated):
//   C <lX> <lY> <sens> <border> <mode> <rH bits> <rLR bits> <rV bits> <rRL bits> <dBlur>
//   M <input mask bytes, hex>
//   O <result bytes, hex>
// mode bit 0 = doDir, bit 1 = doRadius; border 0 = the 4-argument constructor.
// gen_sandor_fxs_cases.py writes this to flare/sources/flarelib/sandor_fxs/SDirection_cases.txt.
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <vector>
#include "SDirection.h"

static uint64_t g = 0xabcdef;
static unsigned rnd() {
  g = g * 6364136223846793005ULL + 1442695040888963407ULL;
  return (unsigned)(g >> 33);
}
static uint64_t bits(double d) {
  uint64_t b;
  memcpy(&b, &d, sizeof b);
  return b;
}

static void run(int lX, int lY, int sens, int border, int mode, double rH, double rLR, double rV,
                double rRL, int dBlur, const std::vector<UCHAR> &m) {
  CSDirection d = border > 0 ? CSDirection(lX, lY, m.data(), sens, border)
                             : CSDirection(lX, lY, m.data(), sens);
  if (mode & 1) d.doDir();
  if (mode & 2) d.doRadius(rH, rLR, rV, rRL, dBlur);
  std::vector<UCHAR> out(m.size());
  d.getResult(out.data());
  printf("C %d %d %d %d %d %016llx %016llx %016llx %016llx %d\nM ", lX, lY, sens, border, mode,
         (unsigned long long)bits(rH), (unsigned long long)bits(rLR), (unsigned long long)bits(rV),
         (unsigned long long)bits(rRL), dBlur);
  for (UCHAR v : m) printf("%02x", v);
  printf("\nO ");
  for (UCHAR v : out) printf("%02x", v);
  printf("\n\n");
}

int main() {
  const int sizes[][2] = {{1, 1}, {3, 3}, {7, 5}, {16, 16}, {23, 11}, {1, 9}, {9, 1}};
  const int senses[]   = {0, 1, 2, 3, 5};
  const int borders[]  = {0, 1, 2};
  const double radii[][4] = {
      {0.0, 0.0, 0.0, 0.0}, {1.0, 1.0, 1.0, 1.0}, {0.2, 0.5, 0.8, 1.0},
      {1.0, 0.0, 1.0, 0.0}, {-1.0, 2.0, 0.5, 0.25}, {0.3, 0.9, 0.1, 0.6}};
  const int blurs[] = {0, 1, 3};
  int n = 0;
  for (auto &sz : sizes) {
    int lX = sz[0], lY = sz[1];
    for (int kind = 0; kind < 4; kind++) {
      std::vector<UCHAR> m(lX * lY);
      for (int y = 0; y < lY; y++)
        for (int x = 0; x < lX; x++) {
          UCHAR v;
          switch (kind) {
          case 0: v = 255; break;
          case 1: v = (rnd() % 3) ? (UCHAR)(1 + rnd() % 255) : 0; break;
          case 2: v = ((x + y) % 5 < 2) ? 1 : 0; break;                     // diagonal stripes
          default: v = (x * 2 + y < lX + 3 && y * 2 > x) ? (UCHAR)200 : 0;  // wedge
          }
          m[y * lX + x] = v;
        }
      for (int sens : senses)
        for (int border : borders) {
          int mode = (n % 3) + 1;  // 1, 2 or 3 in turn
          const double *r = radii[n % 6];
          run(lX, lY, sens, border, mode, r[0], r[1], r[2], r[3], blurs[n % 3], m);
          // always also the full pipeline, so every geometry sees doDir + doRadius
          run(lX, lY, sens, border, 3, r[3], r[2], r[1], r[0], blurs[(n + 1) % 3], m);
          n++;
        }
    }
  }
  return 0;
}
