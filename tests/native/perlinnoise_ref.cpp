// Golden harness for the `perlinnoise.cpp` port (noise evaluators only).
// Original build: `pn_funcs.inc` is the verbatim slice of the original perlinnoise.cpp from
// `PerlinNoise::LinearNoise` up to (not including) the constructor, compiled against a bare
// copy of the class layout (the real header drags in the fx parameter headers).
// CHAIN build: the same inputs through the Rust entry points (flare_perlin_*), the header
// being a trivial argument shuffle.
// Output, one record per line (replayed by `perlinnoise::tests`):
//   cfg <size> <tsize> <offset>
//   c <u> <v> <k> <grain> <linear> <turb> <turb_ranged> <marble> <marble_ranged>   (hex bits)
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <memory>

static uint32_t lcg_state;
static uint32_t lcg() { return lcg_state = lcg_state * 1664525u + 1013904223u; }
static double unit() { return (lcg() >> 8) / 16777216.0; }
static uint64_t bits(double d) { uint64_t u; memcpy(&u, &d, 8); return u; }

#ifdef CHAIN
extern "C" {
double flare_perlin_linear_noise(const float *noise, size_t len, int size, int tsize, double x, double y, double t);
double flare_perlin_eval(const float *noise, size_t len, int size, int tsize, int offset, int kind, double u,
                         double v, double k, double grain, int ranged, double min, double max);
}
#else
class PerlinNoise {
public:
  static int Size, TimeSize, Offset;
  static double Pixel_size;
  std::unique_ptr<float[]> Noise;
  double LinearNoise(double x, double y, double t);
  double Turbolence(double u, double v, double k, double grain);
  double Turbolence(double u, double v, double k, double grain, double min, double max);
  double Marble(double u, double v, double k, double grain);
  double Marble(double u, double v, double k, double grain, double min, double max);
};
int PerlinNoise::Size = 60, PerlinNoise::TimeSize = 20, PerlinNoise::Offset = 1000000;
double PerlinNoise::Pixel_size = 0.01;
#include "pn_funcs.inc"
#endif

int main() {
  const int cfgs[][3] = {{7, 5, 1000000}, {60, 20, 1000000}, {4, 3, 0}};
  for (auto &c : cfgs) {
    int size = c[0], tsize = c[1], offset = c[2];
    size_t len = (size_t)size * size * tsize;
    std::unique_ptr<float[]> tab(new float[len]);
    lcg_state = 12345;
    for (size_t i = 0; i < len; ++i) tab[i] = (float)((lcg() >> 8) / 16777216.0);
    printf("cfg %d %d %d\n", size, tsize, offset);
    lcg_state = 777;
    for (int n = 0; n < 40; ++n) {
      double u = unit() * 500, v = unit() * 500, k = unit() * 100, grain = 0.5 + unit() * 19.5;
      double lin, tb, tbr, mb, mbr;
#ifdef CHAIN
      lin = flare_perlin_linear_noise(tab.get(), len, size, tsize, u, v, k);
      tb  = flare_perlin_eval(tab.get(), len, size, tsize, offset, 0, u, v, k, grain, 0, 0, 1);
      tbr = flare_perlin_eval(tab.get(), len, size, tsize, offset, 0, u, v, k, grain, 1, 0.2, 0.8);
      mb  = flare_perlin_eval(tab.get(), len, size, tsize, offset, 1, u, v, k, grain, 0, 0, 1);
      mbr = flare_perlin_eval(tab.get(), len, size, tsize, offset, 1, u, v, k, grain, 1, 0.1, 0.9);
#else
      PerlinNoise::Size = size; PerlinNoise::TimeSize = tsize; PerlinNoise::Offset = offset;
      PerlinNoise p; p.Noise.reset(new float[len]); memcpy(p.Noise.get(), tab.get(), len * sizeof(float));
      lin = p.LinearNoise(u, v, k);
      tb  = p.Turbolence(u, v, k, grain);
      tbr = p.Turbolence(u, v, k, grain, 0.2, 0.8);
      mb  = p.Marble(u, v, k, grain);
      mbr = p.Marble(u, v, k, grain, 0.1, 0.9);
#endif
      printf("c %016llx %016llx %016llx %016llx %016llx %016llx %016llx %016llx %016llx\n",
             (unsigned long long)bits(u), (unsigned long long)bits(v), (unsigned long long)bits(k),
             (unsigned long long)bits(grain), (unsigned long long)bits(lin), (unsigned long long)bits(tb),
             (unsigned long long)bits(tbr), (unsigned long long)bits(mb), (unsigned long long)bits(mbr));
    }
  }
  return 0;
}
