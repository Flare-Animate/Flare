// Golden harness for the tnzext potential ports (Potential, LinearPotential, SquarePotential,
// NotSymmetricBezierPotential, NotSymmetricExpPotential).
// Original build: the five original .cpp files + their original headers. CHAIN build: the
// live inline headers over the Rust library. Both see the stub `tstroke.h` (a stroke whose
// length curve is L*(0.7w + 0.3w^2)) and, in the original build, a stub `tcurves.h` whose
// TQuadratic::getPoint is the verbatim text of common/tgeometry/tcurves.cpp.
// Records (hex bits for doubles; replayed by `potential_goldens` in tnzext/Potential.rs):
//   <kind> <L> <par> <al> then 8 x <w>:<value> | throw <what>   (hex bits)
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <stdexcept>

#include "ext/LinearPotential.h"
#include "ext/NotSymmetricBezierPotential.h"
#include "ext/NotSymmetricExpPotential.h"
#include "ext/SquarePotential.h"
#include "tstroke.h"

static uint32_t st;
static uint32_t lcg() { return st = st * 1664525u + 1013904223u; }
static double unit() { return (lcg() >> 8) / 16777216.0; }
static uint64_t bits(double d) { uint64_t u; memcpy(&u, &d, 8); return u; }

template <class P>
static void run(const char *kind) {
  st = 4242;
  for (int n = 0; n < 150; ++n) {
    double L = 20.0 + unit() * 400.0;
    double par = unit();
    double al = unit() * 300.0;
    if (n % 11 == 0) par = 0.0;
    if (n % 13 == 0) par = 1.0;
    if (n % 17 == 0) al = 0.0;
    if (n % 19 == 0) par = unit() * 0.004;
    if (n % 23 == 0) par = 1.0 - unit() * 0.004;
    TStroke s(L);
    P p;
    printf("%s %016llx %016llx %016llx", kind, (unsigned long long)bits(L), (unsigned long long)bits(par),
           (unsigned long long)bits(al));
    try {
      p.setParameters(&s, par, al);
      for (int k = 0; k < 8; ++k) {
        double w = (k == 0) ? 0.0 : (k == 1) ? 1.0 : unit();
        printf(" %016llx:%016llx", (unsigned long long)bits(w), (unsigned long long)bits(p.value(w)));
      }
      printf("\n");
    } catch (const std::invalid_argument &e) {
      printf(" throw %s\n", e.what());
    }
  }
}

int main() {
  run<ToonzExt::LinearPotential>("lin");
  run<ToonzExt::SquarePotential>("sq");
  run<ToonzExt::NotSymmetricBezierPotential>("bez");
  run<ToonzExt::NotSymmetricExpPotential>("exp");
  // out of range / uninitialised paths
  ToonzExt::LinearPotential u;
  try { u.value(0.5); } catch (const std::range_error &e) { printf("uninit %s\n", e.what()); }
  TStroke s(100.0);
  try { u.setParameters(0, 0.5, 1.0); } catch (const std::invalid_argument &e) { printf("nostroke %s\n", e.what()); }
  try { u.setParameters(&s, 1.5, 1.0); } catch (const std::invalid_argument &e) { printf("badw %s\n", e.what()); }
  try { u.setParameters(&s, -0.5, 1.0); } catch (const std::invalid_argument &e) { printf("badw2 %s\n", e.what()); }
  u.setParameters(&s, 0.4, 50.0);
  printf("clamp %016llx %016llx\n", (unsigned long long)bits(u.value(-3.0)), (unsigned long long)bits(u.value(7.0)));
  return 0;
}
