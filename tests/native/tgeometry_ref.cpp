// Golden harness for the `tgeometry.cpp` port (TAffine::inv/rotation, TRotation/TScale about
// a centre, TAffine3, TAffine4, TAngleRangeSet).
// Original build: the extracted tgeometry.cpp + header. CHAIN build: the live header, the live
// (trimmed) tgeometry.cpp and the Rust library. Records (doubles as hex bits, replayed by
// `tgeometry::tests::matches_recorded_cpp`):
//   a-inv <6 in> -> <6 out>            a-rot <angle> -> <6>        a-deg <deg> -> <6>
//   a-degc <cx cy deg> -> <6>          a-sc2 <cx cy sx sy> -> <6>  a-sc1 <cx cy s> -> <6>
//   m3 <op> <9 a> [<9 b> | <3 v> | <2 p> | <1 x>] -> <n out>      (op: mul inv get2d tr sc rot vec)
//   m4 <op> <16 a> [...] -> <n out>
//   as <op> a0 a1 argflip same | S flip n a.. | X flip n a.. -> flip n a.. ret
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <vector>

#include "tgeometry.h"

static uint32_t st;
static uint32_t lcg() { return st = st * 1664525u + 1013904223u; }
static uint32_t rnd(uint32_t n) { return (lcg() >> 8) % n; }
static double rdouble() {
  static const double special[] = {0.0, 1.0, -1.0, 2.0, 0.5, 90.0, -90.0, 1e-9};
  if (rnd(6) == 0) return special[rnd(8)];
  return ((int)rnd(2000001) - 1000000) / 1000.0;
}
static void pd(double d) {
  uint64_t u;
  memcpy(&u, &d, 8);
  printf(" %016llx", (unsigned long long)u);
}
static void pv(const double *p, int n) { for (int i = 0; i < n; ++i) pd(p[i]); }
static void paff(const TAffine &a) { double v[6] = {a.a11, a.a12, a.a13, a.a21, a.a22, a.a23}; pv(v, 6); }

static void print_set(const TAngleRangeSet &s) {
  printf(" %d %d", (int)s.isFlipped(), (int)s.angles().size());
  for (unsigned a : s.angles()) printf(" %u", a);
}

static TAngleRangeSet random_set() {
  TAngleRangeSet s;
  int k = rnd(5);
  for (int i = 0; i < k; ++i) s.add(rnd(60), rnd(60));
  if (rnd(8) == 0) s.fill();
  if (rnd(8) == 0) s.clear();
  return s;
}

int main() {
  st = 2024;
  for (int n = 0; n < 400; ++n) {
    double v[6];
    for (double &d : v) d = rdouble();
    int kind = rnd(5);
    if (kind == 0) v[1] = v[3] = 0.0;
    if (kind == 1) v[0] = v[4] = 0.0;
    if (kind == 2 && rnd(2)) v[0] = v[1] = v[2] = v[3] = v[4] = v[5] = 0.0;
    if (kind == 3) { v[1] = 0.0; v[3] = 0.0; v[0] = rnd(2) ? 0.0 : v[0]; }
    TAffine a(v[0], v[1], v[2], v[3], v[4], v[5]);
    printf("a-inv"); pv(v, 6); printf(" ->"); paff(a.inv()); printf("\n");
  }
  for (int n = 0; n < 60; ++n) {
    double ang = rdouble() / 100.0;
    printf("a-rot"); pd(ang); printf(" ->"); paff(TAffine::rotation(ang)); printf("\n");
  }
  for (int n = 0; n < 120; ++n) {
    double deg = n < 40 ? (double)((int)rnd(41) - 20) * 90.0 : rdouble();
    if (n >= 40 && n < 60) deg = (double)((int)rnd(721) - 360);
    printf("a-deg"); pd(deg); printf(" ->"); paff(TRotation(deg)); printf("\n");
    double c[2] = {rdouble(), rdouble()};
    printf("a-degc"); pv(c, 2); pd(deg); printf(" ->"); paff(TRotation(TPointD(c[0], c[1]), deg)); printf("\n");
    double sx = rdouble(), sy = rdouble();
    printf("a-sc2"); pv(c, 2); pd(sx); pd(sy); printf(" ->"); paff(TScale(TPointD(c[0], c[1]), sx, sy)); printf("\n");
    printf("a-sc1"); pv(c, 2); pd(sx); printf(" ->"); paff(TScale(TPointD(c[0], c[1]), sx)); printf("\n");
  }
  // TAffine3
  for (int n = 0; n < 150; ++n) {
    TAffine3 a, b;
    for (double &d : a.a) d = rdouble();
    for (double &d : b.a) d = rdouble();
    if (n % 5 == 0) { TAffine3 id; a = id; a.a31 = rdouble(); a.a32 = rdouble(); }
    printf("m3 mul"); pv(a.a, 9); pv(b.a, 9); printf(" ->"); pv((a * b).a, 9); printf("\n");
    printf("m3 inv"); pv(a.a, 9); printf(" ->"); pv(a.inv().a, 9); printf("\n");
    printf("m3 get2d"); pv(a.a, 9); printf(" ->"); paff(a.get2d()); printf("\n");
    double x = rdouble(), y = rdouble();
    printf("m3 tr"); pd(x); pd(y); printf(" ->"); pv(TAffine3::translation2d(x, y).a, 9); printf("\n");
    printf("m3 sc"); pd(x); pd(y); printf(" ->"); pv(TAffine3::scale2d(x, y).a, 9); printf("\n");
    printf("m3 rot"); pd(x); printf(" ->"); pv(TAffine3::rotation2d(x).a, 9); printf("\n");
    T3DPointD p(rdouble(), rdouble(), rdouble());
    T3DPointD q = a * p;
    printf("m3 vec"); pv(a.a, 9); pd(p.x); pd(p.y); pd(p.z); printf(" ->"); pd(q.x); pd(q.y); pd(q.z); printf("\n");
  }
  // TAffine4
  for (int n = 0; n < 150; ++n) {
    TAffine4 a, b;
    for (double &d : a.a) d = rdouble();
    for (double &d : b.a) d = rdouble();
    if (n % 4 == 0) { TAffine4 id; a = id; a.a41 = rdouble(); a.a42 = rdouble(); a.a43 = rdouble(); }
    printf("m4 mul"); pv(a.a, 16); pv(b.a, 16); printf(" ->"); pv((a * b).a, 16); printf("\n");
    printf("m4 inv"); pv(a.a, 16); printf(" ->"); pv(a.inv().a, 16); printf("\n");
    double z = rdouble();
    printf("m4 get2d"); pv(a.a, 16); pd(z); printf(" ->"); paff(a.get2d(z)); printf("\n");
    printf("m4 persp"); pv(a.a, 16); pd(z); printf(" ->"); pv(a.get2dPersp(z).a, 9); printf("\n");
    double x = rdouble(), y = rdouble(), w = rdouble(), ang = rdouble() / 10.0;
    if (n % 7 == 0) x = y = w = 0.0;
    printf("m4 tr"); pd(x); pd(y); pd(w); printf(" ->"); pv(TAffine4::translation(x, y, w).a, 16); printf("\n");
    printf("m4 sc"); pd(x); pd(y); pd(w); printf(" ->"); pv(TAffine4::scale(x, y, w).a, 16); printf("\n");
    printf("m4 rot"); pd(x); pd(y); pd(w); pd(ang); printf(" ->"); pv(TAffine4::rotation(x, y, w, ang).a, 16); printf("\n");
    printf("m4 rx"); pd(ang); printf(" ->"); pv(TAffine4::rotationX(ang).a, 16); printf("\n");
    printf("m4 ry"); pd(ang); printf(" ->"); pv(TAffine4::rotationY(ang).a, 16); printf("\n");
    printf("m4 rz"); pd(ang); printf(" ->"); pv(TAffine4::rotationZ(ang).a, 16); printf("\n");
    T4DPointD p(rdouble(), rdouble(), rdouble(), rdouble());
    T4DPointD q = a * p;
    printf("m4 vec"); pv(a.a, 16); pd(p.x); pd(p.y); pd(p.z); pd(p.w); printf(" ->"); pd(q.x); pd(q.y); pd(q.z); pd(q.w); printf("\n");
  }
  // TAngleRangeSet
  for (int n = 0; n < 3000; ++n) {
    TAngleRangeSet s = random_set();
    TAngleRangeSet x = random_set();
    unsigned a0 = rnd(5) == 0 ? rnd(0xffffffffu) : rnd(60);
    unsigned a1 = rnd(5) == 0 ? rnd(0xffffffffu) : rnd(60);
    int op = 1 + rnd(14);
    bool same = rnd(7) == 0;
    bool argflip = rnd(2);
    const TAngleRangeSet *xs = same ? &s : &x;
    int ret = 0;
    TAngleRangeSet before = s, xcopy = *xs;
    switch (op) {
    case 1: ret = s.contains(a0); break;
    case 2: ret = s.check(); break;
    case 3: s.set(a0, a1); break;
    case 4: s.set(*xs, argflip); break;
    case 5: s.invert(a0, a1); break;
    case 6: if (same) continue; s.invert(*xs); break;
    case 7: s.add(a0, a1); break;
    case 8: s.add(*xs); break;
    case 9: s.subtract(a0, a1); break;
    case 10: s.subtract(*xs); break;
    case 11: s.intersect(a0, a1); break;
    case 12: s.intersect(*xs); break;
    default: continue;  // 13, 14: unused codes
    }
    printf("as %d %u %u %d %d |S", op, a0, a1, (int)argflip, (int)same);
    print_set(before);
    printf(" |X"); print_set(xcopy);
    printf(" ->"); print_set(s); printf(" %d\n", ret);
  }
  return 0;
}
