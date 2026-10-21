// Golden harness for the `igs_motion_blur.cpp` port.
//
// The *original* igs_motion_blur.cpp (recovered from git history by
// `gen_igs_motion_blur_cases.py`) is #included so the harness can reach its
// anonymous-namespace `set_smooth_` / `convert_template_`: the kernel (weights as raw
// IEEE hex, pixel offsets) is recorded first, then 12 images (8/16-bit x 4/3/1
// channels x alpha-blur on/off) are blurred through the public `convert` and hashed
// with FNV-1a 64. Replayed by `igs_motion_blur::tests::matches_the_cpp_reference`.
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <vector>

#include "igs_motion_blur.cpp"

static unsigned val(int bits, int i, int y) {
  if (bits == 8) return (unsigned)((i * 37 + (i / 3) * 11 + y * 5) % 256);
  return (unsigned)((i * 2579 + y * 97 + 13) % 65536);
}

static std::vector<unsigned char> make(int bits, int cc, int w, int h) {
  const int bytes = bits / 8;
  std::vector<unsigned char> buf((size_t)w * h * cc * bytes);
  for (int i = 0; i < w * h * cc; ++i) {
    unsigned v = val(bits, i, i / (w * cc));
    if (cc == 4 && i % 4 == 3 && (i / 4) % 5 == 0) v = 0;
    buf[(size_t)i * bytes] = (unsigned char)(v & 0xff);
    if (bytes == 2) buf[(size_t)i * bytes + 1] = (unsigned char)(v >> 8);
  }
  return buf;
}

static uint64_t fnv(const std::vector<unsigned char> &b) {
  uint64_t h = 0xcbf29ce484222325ULL;
  for (unsigned char c : b) {
    h ^= c;
    h *= 0x100000001b3ULL;
  }
  return h;
}

struct Cfg {
  double xv, yv, scale, curve;
  int zl;
  double zp;
};

int main() {
  const Cfg cfgs[] = {
      {3.0, 0.0, 1.0, 1.0, 0, 1.0},   {0.0, -4.0, 1.0, 1.0, 0, 1.0},
      {5.5, 3.2, 1.0, 1.0, 0, 1.0},   {-6.0, 2.0, 1.0, 1.0, 0, 1.0},
      {-2.5, -7.5, 1.0, 1.0, 0, 1.0}, {1.0, 1.0, 1.0, 1.0, 0, 1.0},
      {0.4, 0.0, 1.0, 1.0, 0, 1.0},   {9.0, 9.0, 0.5, 1.0, 0, 1.0},
      {6.0, 1.5, 1.0, 2.0, 0, 1.0},   {6.0, 1.5, 1.0, 0.5, 0, 1.0},
      {6.0, 1.5, 1.0, 0.0, 0, 1.0},   {6.0, 1.5, 1.0, -1.0, 0, 1.0},
      {8.0, 0.0, 1.0, 1.0, 3, 0.5},   {8.0, 3.0, 1.0, 1.0, 5, 1.0},
      {-7.0, 4.0, 1.0, 1.5, 4, 0.25}, {0.0, 0.0, 1.0, 1.0, 0, 1.0},
  };
  const int w = 13, h = 9;
  for (int ci = 0; ci < (int)(sizeof(cfgs) / sizeof(cfgs[0])); ++ci) {
    const Cfg &c = cfgs[ci];
    std::vector<double> ra;
    std::vector<int> xa, ya;
    set_smooth_(c.xv, c.yv, c.scale, c.curve, c.zl, c.zp, ra, xa, ya);
    std::printf("cfg %d\nsize %zu\nr", ci, ra.size());
    for (double d : ra) {
      uint64_t u;
      std::memcpy(&u, &d, 8);
      std::printf(" %016llx", (unsigned long long)u);
    }
    std::printf("\nx");
    for (int v : xa) std::printf(" %d", v);
    std::printf("\ny");
    for (int v : ya) std::printf(" %d", v);
    std::printf("\n");
    for (int bits = 8; bits <= 16; bits += 8)
      for (int cc : {4, 3, 1})
        for (int alpha = 0; alpha < 2; ++alpha) {
          std::vector<unsigned char> in = make(bits, cc, w, h);
          std::vector<unsigned char> out(in.size(), 0);
          igs::motion_blur::convert(in.data(), out.data(), h, w, cc, bits, c.xv,
                                    c.yv, c.scale, c.curve, c.zl, c.zp,
                                    alpha != 0);
          std::printf("img %d %d %d %016llx\n", bits, cc, alpha,
                      (unsigned long long)fnv(out));
        }
  }
  return 0;
}
