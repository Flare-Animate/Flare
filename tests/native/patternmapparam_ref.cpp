// Differential reference harness for the `CPatternMapParam` port (flarelib/sandor_fxs):
// the original PatternMapParam.h/.cpp, extracted from git, parsing hostile and ordinary
// argument lists. CIL.h and Pattern.h are mocked (the ink list is not under test).
//
// Per case (blank-line separated):
//   C shrink=<n>
//   A <12 arguments, each as hex bytes, space separated>
//   O <maxScale minScale maxDirAngle minDirAngle maxDist minDist density bits> <4 flags>
// gen_sandor_fxs_cases.py writes this to flare/sources/flarelib/sandor_fxs/PatternMapParam_cases.txt.
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>
#include "PatternMapParam.h"

static uint64_t g = 99;
static unsigned rnd() {
  g = g * 6364136223846793005ULL + 1442695040888963407ULL;
  return (unsigned)(g >> 33);
}
static uint64_t bits(double d) {
  uint64_t b;
  memcpy(&b, &d, sizeof b);
  return b;
}

int main() {
  const char *reals[] = {"0",    "1",   "2",    "0.5",  "-3",  "1e2",   "  7",   "abc", "",
                         "12abc", ".5",  "0x10", "inf",  "nan", "-inf",  "1e400", "3.99999",
                         "1e-5", "99999999999", "+4", "-0.0", "1,5", "180", "360.25"};
  const char *ints[]  = {"0", "1", "-1", "2", "abc", "", " 5", "0x1", "1.9", "-0"};
  const int shrinks[] = {0, 1, 2, 3, -1};
  for (int c = 0; c < 400; c++) {
    std::vector<std::string> a(12);
    a[0] = "all";
    for (int i = 1; i <= 4; i++) a[i] = reals[rnd() % (sizeof reals / sizeof *reals)];
    a[5] = ints[rnd() % (sizeof ints / sizeof *ints)];
    for (int i = 6; i <= 8; i++) a[i] = reals[rnd() % (sizeof reals / sizeof *reals)];
    for (int i = 9; i <= 11; i++) a[i] = ints[rnd() % (sizeof ints / sizeof *ints)];
    int shrink = shrinks[c % 5];
    std::vector<const char *> argv;
    for (auto &s : a) argv.push_back(s.c_str());
    CPatternMapParam p((int)argv.size(), argv.data(), shrink);
    printf("C shrink=%d\nA", shrink);
    for (size_t i = 0; i < a.size(); i++) {
      printf(i ? " " : " ");
      for (unsigned char ch : a[i]) printf("%02x", ch);
    }
    printf("\nO %016llx %016llx %016llx %016llx %016llx %016llx %016llx %d %d %d %d\n\n",
           (unsigned long long)bits(p.m_maxScale), (unsigned long long)bits(p.m_minScale),
           (unsigned long long)bits(p.m_maxDirAngle), (unsigned long long)bits(p.m_minDirAngle),
           (unsigned long long)bits(p.m_maxDist), (unsigned long long)bits(p.m_minDist),
           (unsigned long long)bits(p.m_density), (int)p.m_isRandomDir, (int)p.m_isKeepContour,
           (int)p.m_isUseInkColor, (int)p.m_isIncludeAlpha);
  }
  return 0;
}
