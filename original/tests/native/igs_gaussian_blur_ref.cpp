// Golden harness for the `igs_gaussian_blur.cpp` port.
//
// Compiled against the *original* igs_gaussian_blur.cpp and igs_gauss_distribution.cpp
// (recovered from git history by `gen_igs_gaussian_blur_cases.py`). One `case` line
// (its own parameters) followed by one `out` line with the output floats as raw
// IEEE-754 hex. Replayed by `igs_gaussian_blur::tests::matches_the_cpp_reference`.
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <vector>

#include "igs_gaussian_blur.h"

static std::vector<float> make_img(int h, int w, int ch, int style) {
  std::vector<float> v((size_t)h * w * ch);
  for (int i = 0; i < h * w * ch; ++i)
    v[i] = (float)((i * 37 + (i / 3) * 11 + (i / (w * ch)) * 5) % 256) / 255.0f;
  if (style == 1) {
    for (int p = 0; p < h * w; ++p)
      for (int c = 1; c < (ch < 3 ? ch : 3); ++c) v[p * ch + c] = v[p * ch];
  }
  return v;
}

struct Case {
  int h, w, ch, r;
  double real, sigma;
  int use_ref, style;
};

static uint64_t bits64(double d) {
  uint64_t u;
  std::memcpy(&u, &d, 8);
  return u;
}

int main() {
  const Case cases[] = {
      {9, 11, 4, 2, 2.0, 0.25, 0, 0},  {9, 11, 4, 2, 1.5, 0.25, 0, 0},
      {7, 7, 4, 1, 0.3, 0.25, 0, 0},   {9, 11, 3, 2, 2.0, 0.25, 0, 0},
      {9, 11, 1, 2, 2.0, 0.25, 0, 0},  {9, 11, 4, 2, 2.0, 0.25, 1, 0},
      {9, 11, 3, 2, 1.7, 0.25, 1, 0},  {9, 11, 4, 2, 2.0, 0.25, 0, 1},
      {12, 8, 4, 3, 2.6, 0.25, 0, 0},  {12, 8, 3, 3, 2.6, 0.25, 1, 0},
      {9, 11, 4, 2, 2.0, 0.4, 0, 0},   {9, 11, 1, 1, 0.0, 0.25, 0, 0},
      {15, 15, 4, 4, 3.2, 0.25, 1, 0}, {9, 11, 4, 2, 2.0, 0.25, 1, 1},
  };
  for (const Case &c : cases) {
    std::vector<float> in = make_img(c.h, c.w, c.ch, c.style);
    std::vector<float> ref((size_t)c.h * c.w);
    for (int i = 0; i < c.h * c.w; ++i) ref[i] = (float)((i * 37) % 101) / 100.0f;
    const int hn = c.h - 2 * c.r, wn = c.w - 2 * c.r;
    std::vector<float> out((size_t)hn * wn * c.ch, -1.0f);
    const int bb = igs::gaussian_blur_hv::buffer_bytes(c.h, c.w, c.r);
    std::vector<double> buf(bb / 8 + 1);
    igs::gaussian_blur_hv::convert(in.data(), out.data(), c.h, c.w, c.ch,
                                   c.use_ref ? ref.data() : nullptr, buf.data(),
                                   bb, c.r, c.real, c.sigma);
    std::printf("case %d %d %d %d %016llx %016llx %d %d\nout", c.h, c.w, c.ch, c.r,
                (unsigned long long)bits64(c.real),
                (unsigned long long)bits64(c.sigma), c.use_ref, c.style);
    for (float f : out) {
      uint32_t u;
      std::memcpy(&u, &f, 4);
      std::printf(" %08x", u);
    }
    std::printf("\n");
  }
  return 0;
}
