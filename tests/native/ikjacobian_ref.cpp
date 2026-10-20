// Differential reference harness for the ikjacobian / ikengine port (flarelib/ikjacobian.rs).
//
// Compiled twice by gen_ikjacobian_cases.py:
//   * against the ORIGINAL flarelib/ikjacobian.cpp (+ headers) rebuilt from git -> the golden;
//   * (--verify-chain) against the NEW inline headers + libflare_formats.a -> must print the
//     same bytes (this proves the C++ forwarders marshal like the old members did).
//
// Output is one case per line:  NAME ints... | inputs (f64 bits, hex) | outputs (f64 bits)
// Everything here is IEEE arithmetic (+ - * / sqrt), so the bytes do not depend on the libm.
// Exceptions: ANG lines (atan2) and, only with argument "drag", trig-based engine runs; the
// latter are never part of the golden.
#include <cassert>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <algorithm>
#include <deque>
#include <fstream>
#include <functional>
#include <iostream>
#include <limits>
#include <list>
#include <map>
#include <memory>
#include <queue>
#include <set>
#include <sstream>
#include <stack>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>
#include <unistd.h>
#include <sys/wait.h>

// The original header leaves new[]-allocated scratch uninitialised in a few places; make
// that deterministic (the Rust port zero-initialises its temporaries).
void *operator new[](std::size_t n) {
  void *p = std::calloc(1, n ? n : 1);
  if (!p) throw std::bad_alloc();
  return p;
}

#define private public
#define protected public
#include "flare/ikengine.h"
#include "flare/ikjacobian.h"
#undef private
#undef protected

static uint64_t g_state = 0x9e3779b97f4a7c15ULL;
static uint64_t next64() {
  g_state = g_state * 6364136223846793005ULL + 1442695040888963407ULL;
  uint64_t z = g_state;
  z ^= z >> 29;
  return z * 0xbf58476d1ce4e5b9ULL;
}
static unsigned rnd(unsigned n) { return (unsigned)((next64() >> 20) % n); }
static double unit() { return (double)(next64() >> 11) / 9007199254740992.0; }  // [0,1)
static double val() {
  switch (rnd(12)) {
  case 0: return 0.0;
  case 1: return rnd(2) ? -0.0 : 0.0;
  case 2: return (double)((int)rnd(9) - 4);
  case 3: return (unit() - 0.5) * 2e-8;
  case 4: return (unit() - 0.5) * 2e5;
  default: return (unit() - 0.5) * 6.0;
  }
}
static double nz() {  // never exactly zero
  double v;
  do v = val(); while (v == 0.0);
  return v;
}
static uint64_t bits(double d) {
  uint64_t b;
  memcpy(&b, &d, sizeof b);
  return b;
}
static void pd(double d) { printf(" %016llx", (unsigned long long)bits(d)); }
static void bar() { printf(" |"); }
static void pmat(const MatrixRmn &m) {
  for (long k = 0; k < m.getNumRows() * m.getNumColumns(); k++) pd(m.GetPtr()[k]);
}
static void pvec(const VectorRn &v) {
  for (long k = 0; k < v.GetLength(); k++) pd(v.GetPtr()[k]);
}
static void fillm(MatrixRmn &m) {
  for (long k = 0; k < m.getNumRows() * m.getNumColumns(); k++) m.GetPtr()[k] = val();
}
static void fillv(VectorRn &v) {
  for (long k = 0; k < v.GetLength(); k++) v.GetPtr()[k] = val();
}
static void line_begin(const char *name) { printf("%s", name); }
static void ints(std::initializer_list<long> l) {
  for (long i : l) printf(" %ld", i);
}

// ---------------------------------------------------------------- matrix / vector kernels
static void linear_ops() {
  const long shapes[][2] = {{1, 1}, {1, 4}, {4, 1}, {2, 2}, {2, 3}, {3, 2}, {3, 3}, {4, 5}, {5, 3}};
  for (auto &sh : shapes) {
    long r = sh[0], c = sh[1];
    for (int rep = 0; rep < 1; rep++) {
      // MaxAbs
      {
        VectorRn v(c);
        fillv(v);
        line_begin("maxabs"), ints({c});
        bar(), pvec(v);
        bar(), pd(v.MaxAbs());
        printf("\n");
      }
      // matrix * vector, transpose * vector, dot with a column
      {
        MatrixRmn A(r, c);
        fillm(A);
        VectorRn v(c), res(r);
        fillv(v);
        line_begin("mulv"), ints({r, c});
        bar(), pmat(A), pvec(v);
        A.Multiply(v, res);
        bar(), pvec(res);
        printf("\n");
        VectorRn u(r), res2(c);
        fillv(u);
        line_begin("mulvt"), ints({r, c});
        bar(), pmat(A), pvec(u);
        A.MultiplyTranspose(u, res2);
        bar(), pvec(res2);
        printf("\n");
        long col = rnd(c);
        line_begin("dotcol"), ints({r, c, col});
        bar(), pmat(A), pvec(u);
        bar(), pd(A.DotProductColumn(u, col));
        printf("\n");
        line_begin("frob"), ints({r, c});
        bar(), pmat(A);
        bar(), pd(A.FrobeniusNorm());
        printf("\n");
      }
      // diagonals
      {
        long dl = std::min(r, c);
        MatrixRmn A(r, c);
        VectorRn dv(dl);
        fillv(dv);
        double d = nz();
        struct Op {
          const char *name;
          std::function<void(MatrixRmn &)> f;
          long vlen;
        };
        long sup = std::min(r - 1, c), sub = std::min(r, c) - 1, supS = std::min(r, c - 1);
        VectorRn supv(std::max(sup, 1L)), subv(std::max(sub, 1L));
        fillv(supv);
        fillv(subv);
        std::vector<Op> ops = {
            {"setdiag", [&](MatrixRmn &m) { m.SetDiagonalEntries(d); }, 0},
            {"setdiagv", [&](MatrixRmn &m) { m.SetDiagonalEntries(dv); }, dl},
            {"setsuper", [&](MatrixRmn &m) { m.SetSuperDiagonalEntries(d); }, 0},
            {"setsub", [&](MatrixRmn &m) { m.SetSubDiagonalEntries(d); }, 0},
            {"adddiag", [&](MatrixRmn &m) { m.AddToDiagonal(d); }, 0},
            {"adddiagv", [&](MatrixRmn &m) { m.AddToDiagonal(dv); }, dl},
        };
        if (sup >= 1 && r <= c) ops.push_back({"setsuperv", [&](MatrixRmn &m) { m.SetSuperDiagonalEntries(supv); }, sup});
        if (sub >= 1) ops.push_back({"setsubv", [&](MatrixRmn &m) { m.SetSubDiagonalEntries(subv); }, sub});
        (void)supS;
        for (auto &op : ops) {
          fillm(A);
          line_begin(op.name), ints({r, c});
          bar(), pmat(A);
          bar();
          if (op.vlen == 0) pd(d);
          else if (!strcmp(op.name, "setsuperv")) { for (long k = 0; k < sup; k++) pd(supv.GetPtr()[k]); }
          else if (!strcmp(op.name, "setsubv")) { for (long k = 0; k < sub; k++) pd(subv.GetPtr()[k]); }
          else pvec(dv);
          op.f(A);
          bar(), pmat(A);
          printf("\n");
        }
      }
      // column / row / sequence
      {
        MatrixRmn A(r, c);
        VectorRn col(r), row(c);
        fillv(col);
        fillv(row);
        long i = rnd(c), j = rnd(r);
        fillm(A);
        line_begin("setcol"), ints({r, c, i});
        bar(), pmat(A), pvec(col);
        A.SetColumn(i, col);
        bar(), pmat(A);
        printf("\n");
        // SetRow loops NumRows times (copied from SetColumn): it overruns when r > c
        if (r <= c) {
          fillm(A);
          line_begin("setrow"), ints({r, c, j});
          bar(), pmat(A), pvec(row);
          A.SetRow(j, row);
          bar(), pmat(A);
          printf("\n");
        }
        // down a column / along a row / along the diagonal
        long dr[3] = {1, 0, 1}, dc[3] = {0, 1, 1};
        for (int k = 0; k < 3; k++) {
          long sr = 0, sc = 0;
          long len = std::min(dr[k] ? r : 1000, dc[k] ? c : 1000);
          if (len == 1000) len = 1;
          VectorRn d(len);
          fillv(d);
          fillm(A);
          line_begin("setseq"), ints({r, c, sr, sc, dr[k], dc[k], len});
          bar(), pmat(A), pvec(d);
          A.SetSequence(d, sr, sc, dr[k], dc[k]);
          bar(), pmat(A);
          printf("\n");
        }
      }
      // submatrix loads
      {
        long ar = 1 + rnd(r), ac = 1 + rnd(c);
        MatrixRmn A(ar, ac), M(r, c);
        fillm(A);
        fillm(M);
        line_begin("load"), ints({r, c, ar, ac});
        bar(), pmat(M), pmat(A);
        M.LoadAsSubmatrix(A);
        bar(), pmat(M);
        printf("\n");
        long tr = 1 + rnd(c), tc = 1 + rnd(r);  // A is tr x tc, M must hold A^T: r >= tc, c >= tr
        MatrixRmn B(tr, tc);
        fillm(B);
        fillm(M);
        line_begin("loadt"), ints({r, c, tr, tc});
        bar(), pmat(M), pmat(B);
        M.LoadAsSubmatrixTranspose(B);
        bar(), pmat(M);
        printf("\n");
      }
    }
  }
  // products
  const long tri[][3] = {{1, 1, 1}, {2, 3, 2}, {3, 2, 4}, {4, 4, 4}, {5, 3, 1}, {2, 6, 3}, {3, 1, 5}};
  for (auto &t : tri) {
    long r = t[0], k = t[1], c = t[2];
    for (int rep = 0; rep < 1; rep++) {
      {
        MatrixRmn A(r, k), B(k, c), D(r, c);
        fillm(A), fillm(B);
        line_begin("mul"), ints({r, k, c});
        bar(), pmat(A), pmat(B);
        MatrixRmn::Multiply(A, B, D);
        bar(), pmat(D);
        printf("\n");
      }
      {
        MatrixRmn A(k, r), B(k, c), D(r, c);
        fillm(A), fillm(B);
        line_begin("tmul"), ints({r, k, c});
        bar(), pmat(A), pmat(B);
        MatrixRmn::TransposeMultiply(A, B, D);
        bar(), pmat(D);
        printf("\n");
      }
      {
        MatrixRmn A(r, k), B(c, k), D(r, c);
        fillm(A), fillm(B);
        line_begin("mult"), ints({r, k, c});
        bar(), pmat(A), pmat(B);
        MatrixRmn::MultiplyTranspose(A, B, D);
        bar(), pmat(D);
        printf("\n");
      }
      {
        MatrixRmn A(r, k), D(r, c);
        fillm(A);
        double s = nz();
        fillm(D);
        line_begin("mscalar"), ints({r, k, c});
        bar(), pmat(A), pd(s), pmat(D);
        MatrixRmn::MultiplyScalar(A, s, D);
        bar(), pmat(D);
        printf("\n");
      }
    }
  }
  // Gaussian elimination
  const long sq[] = {1, 2, 3, 4, 6, 8};
  for (long n : sq) {
    for (int rep = 0; rep < 3; rep++) {
      MatrixRmn A(n, n);
      VectorRn b(n), x(n);
      fillm(A);
      if (rep == 2)  // diagonally dominant
        for (long i = 0; i < n; i++) A.GetPtr()[i * n + i] += 40.0;
      for (long i = 0; i < n; i++)
        if (A.GetPtr()[i * n + i] == 0.0) A.GetPtr()[i * n + i] = 1.0;
      fillv(b);
      fillv(x);
      line_begin("solve"), ints({n});
      bar(), pmat(A), pvec(b), pvec(x);
      A.Solve(b, &x);
      bar(), pvec(x);
      printf("\n");
    }
  }
  const long rs[][2] = {{1, 2}, {2, 3}, {3, 4}, {4, 5}, {5, 2}, {4, 3}};
  for (auto &s : rs) {
    for (int rep = 0; rep < 2; rep++) {
      MatrixRmn A(s[0], s[1]);
      fillm(A);
      for (long i = 0; i < std::min(s[0], s[1]); i++)
        if (A.GetPtr()[i * s[0] + i] == 0.0) A.GetPtr()[i * s[0] + i] = 1.0;
      line_begin("ref"), ints({s[0], s[1]});
      bar(), pmat(A);
      A.ConvertToRefNoFree();
      bar(), pmat(A);
      printf("\n");
    }
  }
  // Givens
  for (int k = 0; k < 24; k++) {
    double a = k == 0 ? 0.0 : val(), b = k < 2 ? 0.0 : val(), c, s;
    MatrixRmn::CalcGivensValues(a, b, &c, &s);
    line_begin("givens");
    bar(), pd(a), pd(b);
    bar(), pd(c), pd(s);
    printf("\n");
  }
  for (int k = 0; k < 10; k++) {
    long r = 1 + rnd(4), c = 2 + rnd(3);
    MatrixRmn A(r, c);
    fillm(A);
    double cs = val(), sn = val();
    long i1 = rnd(c), i2 = (i1 + 1 + rnd(c - 1)) % c;
    line_begin("pgivens"), ints({r, c, i1, i2});
    bar(), pmat(A), pd(cs), pd(sn);
    A.PostApplyGivens(cs, sn, i1, i2);
    bar(), pmat(A);
    printf("\n");
    if (i1 + 1 < c) {
      fillm(A);
      line_begin("pgivens1"), ints({r, c, i1});
      bar(), pmat(A), pd(cs), pd(sn);
      A.PostApplyGivens(cs, sn, i1);
      bar(), pmat(A);
      printf("\n");
    }
  }
}

// Some inputs make the original Golub-Reinsch loop spin forever (unbounded `while (true)`).
// Run such cases in a child with an alarm; a case that does not finish is not recorded (the
// Rust port caps the loop and reports an error instead, see the unit test for it).
static int g_skipped = 0;
static void guarded(const std::function<void()> &f) {
  fflush(stdout);
  int fd[2];
  if (pipe(fd) != 0) abort();
  pid_t pid = fork();
  if (pid == 0) {
    close(fd[0]);
    dup2(fd[1], 1);
    alarm(3);
    f();
    fflush(stdout);
    _exit(0);
  }
  close(fd[1]);
  std::string buf;
  char tmp[4096];
  ssize_t n;
  while ((n = read(fd[0], tmp, sizeof tmp)) > 0) buf.append(tmp, (size_t)n);
  close(fd[0]);
  int st = 0;
  waitpid(pid, &st, 0);
  if (WIFEXITED(st) && WEXITSTATUS(st) == 0) fwrite(buf.data(), 1, buf.size(), stdout);
  else g_skipped++;
}

// ---------------------------------------------------------------------------------- SVD
static void svd_case_unguarded(const char *tag, MatrixRmn &A) {
  long r = A.getNumRows(), c = A.getNumColumns();
  MatrixRmn U(r, r), V(c, c);
  VectorRn w(std::min(r, c));
  for (long k = 0; k < r * r; k++) U.GetPtr()[k] = 1.0 + 0.01 * k;
  for (long k = 0; k < c * c; k++) V.GetPtr()[k] = 1.0 + 0.01 * k;
  w.SetZero();
  line_begin("svd"), ints({r, c});
  printf(" %s", tag);
  bar(), pmat(A);
  A.ComputeSVD(U, w, V);
  bool ok = A.DebugCheckSVD(U, w, V);
  bar(), pmat(U), pvec(w), pmat(V);
  printf(" %d\n", ok ? 1 : 0);
}
static void svd_case(const char *tag, MatrixRmn &A) {
  guarded([&] { svd_case_unguarded(tag, A); });
}
static void svd_ops() {
  const long shapes[][2] = {{2, 2}, {3, 3}, {4, 4}, {5, 5}, {2, 3}, {2, 5}, {3, 2}, {5, 2}, {3, 6}, {6, 3}};
  for (auto &sh : shapes) {
    for (int rep = 0; rep < 1; rep++) {
      MatrixRmn A(sh[0], sh[1]);
      fillm(A);
      svd_case("rand", A);
    }
    MatrixRmn Z(sh[0], sh[1]);  // sparse small integers: exercises zero diagonal handling
    for (long k = 0; k < sh[0] * sh[1]; k++) Z.GetPtr()[k] = rnd(2) ? 0.0 : (double)((int)rnd(5) - 2);
    svd_case("sparse", Z);
    MatrixRmn R1(sh[0], sh[1]);  // rank one
    for (long i = 0; i < sh[0]; i++)
      for (long j = 0; j < sh[1]; j++) R1.GetPtr()[j * sh[0] + i] = (double)(i + 1) * (double)(j + 2);
    svd_case("rank1", R1);
    MatrixRmn ZC(sh[0], sh[1]);  // zero column
    fillm(ZC);
    for (long i = 0; i < sh[0]; i++) ZC.GetPtr()[(sh[1] / 2) * sh[0] + i] = 0.0;
    svd_case("zerocol", ZC);
  }
  for (long n : {2L, 3L, 5L}) {
    MatrixRmn I(n, n), Z(n, n);
    I.SetIdentity();
    Z.SetZero();
    svd_case("ident", I);
    svd_case("zero", Z);
  }
  MatrixRmn Big(8, 8);
  fillm(Big);
  svd_case("rand", Big);
}

// ----------------------------------------------------------------------------- Jacobian
struct SkelCase {
  IKSkeleton sk;
  std::vector<TPointD> target;
  int n = 0, nEff = 0;
  std::vector<int> parent, purpose, frozen, jointNum, effNum;
  std::vector<TPointD> S;
};

static bool build_skeleton(SkelCase &sc, int minJoints, int minEff) {
  int n = 3 + rnd(6);
  sc.n = n;
  sc.parent.assign(n, -1);
  for (int i = 0; i < n; i++) {
    sc.sk.addNode(new IKNode());
    sc.sk.setNode(i, TPointD(val(), val()), IKNode::JOINT);
    if (i > 0) {
      int p = rnd(i);
      if (rnd(3)) p = i - 1;  // mostly chains
      sc.sk.setParent(i, p);
      sc.parent[i] = p;
    }
  }
  // effectors: leaves only (so no joint number is ever read from an effector)
  std::vector<int> hasChild(n, 0);
  for (int i = 1; i < n; i++) hasChild[sc.parent[i]] = 1;
  std::vector<int> leaves;
  for (int i = 1; i < n; i++)
    if (!hasChild[i]) leaves.push_back(i);
  int want = std::max(minEff, 1 + (int)rnd((unsigned)std::min<size_t>(leaves.size(), 3)));
  if ((int)leaves.size() < minEff) return false;
  sc.purpose.assign(n, 0);
  for (int k = 0; k < want; k++) {
    int pick = leaves[rnd(leaves.size())];
    if (sc.purpose[pick]) continue;
    sc.purpose[pick] = 1;
    sc.sk.setPurpose(pick, IKNode::EFFECTOR);
  }
  sc.nEff = sc.sk.getNumEffector();
  int joints = n - sc.nEff;
  if (joints < minJoints) return false;
  sc.jointNum.assign(n, -1);
  sc.effNum.assign(n, -1);
  sc.frozen.assign(n, 0);
  sc.S.resize(n);
  int j = 0;
  for (int i = 0; i < n; i++) {
    IKNode *nd = sc.sk.getNode(i);
    if (sc.purpose[i]) {
      sc.effNum[i] = nd->getEffectorNum();
    } else {
      nd->setSeqNumJoint(j);
      sc.jointNum[i] = j++;
      if (rnd(6) == 0) {
        nd->freeze();
        sc.frozen[i] = 1;
      }
    }
    TPointD s(val(), val());
    nd->setS(s);
    sc.S[i] = s;
  }
  for (int k = 0; k < sc.nEff; k++) sc.target.push_back(TPointD(val(), val()));
  return true;
}
static void print_skeleton(const SkelCase &sc) {
  bar();
  for (int i = 0; i < sc.n; i++) {
    printf(" %d %d %d %d %d", sc.parent[i], sc.purpose[i], sc.frozen[i], sc.jointNum[i], sc.effNum[i]);
    pd(sc.S[i].x), pd(sc.S[i].y);
  }
  bar();
  for (auto &t : sc.target) pd(t.x), pd(t.y);
}

static void jacobian_ops() {
  // 0 transpose 1 pseudo 2 dls 3 dlssvd 4 clamp 5 upd-sclamp
  const int plan[] = {0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 4, 4, 5, 5};
  for (int pi = 0; pi < (int)(sizeof plan / sizeof plan[0]) * 2; pi++) {
    int method = plan[pi % (sizeof plan / sizeof plan[0])];
    SkelCase *scp = nullptr;
    for (int tries = 0; tries < 50 && !scp; tries++) {
      scp = new SkelCase();
      int minEff = (method == 1 && (pi & 1)) ? 2 : 1;  // half of the pseudo-inverse cases are multi-effector
      if (!build_skeleton(*scp, (method == 1 || method == 3) ? 2 : 1, minEff)) {
        delete scp;
        scp = nullptr;
      }
    }
    if (!scp) continue;
    SkelCase &sc = *scp;
    guarded([&] {
    static const char *names[] = {"jtranspose", "jpseudo", "jdls", "jdlssvd", "jclamp", "jupdclamp"};
    Jacobian jac(&sc.sk, sc.target);
    jac.computeJacobian();
    line_begin(names[method]), ints({sc.n, sc.nEff});
    print_skeleton(sc);
    bar();
    VectorRn clampIn(sc.nEff);
    if (method == 4 || method == 5) {
      for (int k = 0; k < sc.nEff; k++) clampIn[k] = rnd(3) ? std::fabs(val()) : HUGE_VAL;
      jac.dSclamp.Load(clampIn.GetPtr());
      pvec(clampIn);
    }
    switch (method) {
    case 0: jac.CalcDeltaThetasTranspose(); break;
    case 1: jac.CalcDeltaThetasPseudoinverse(); break;
    case 2: jac.CalcDeltaThetasDLS(); break;
    case 3: jac.CalcDeltaThetasDLSwithSVD(); break;
    case 4: jac.CalcdTClampedFromdS(); break;
    case 5: jac.UpdatedSClampValue(); break;
    }
    bar(), pmat(jac.Jend), pvec(jac.dS), pvec(jac.dTheta), pvec(jac.dT), pvec(jac.dSclamp);
    if (method == 2) pmat(jac.U);
    printf("\n");
    });
    delete scp;
  }
}

// ---------------------------------------------------------------------------- IKEngine
static void angle_ops() {
  for (int rep = 0; rep < 12; rep++) {
    int n = 3 + rnd(5);
    IKEngine e;
    std::vector<TPointD> pos;
    for (int i = 0; i < n; i++) pos.push_back(TPointD(val(), val()));
    if (rep % 4 == 3) pos[1] = pos[0];  // zero-length first segment: NaN angle
    e.setRoot(pos[0]);
    for (int i = 1; i < n; i++) e.addJoint(pos[i], i - 1);
    for (int idx = 0; idx + 1 < n; idx++) {
      if (idx > 0 && pos[idx] == pos[idx - 1]) continue;
      line_begin("ang"), ints({n, idx});
      bar();
      for (auto &p : pos) pd(p.x), pd(p.y);
      bar(), pd(e.getJointAngle(idx));
      printf("\n");
    }
  }
}

// Not part of the golden (sin/cos): the whole engine, dragged, for the chain comparison.
static void drag_ops() {
  for (int rep = 0; rep < 12; rep++) {
    int n = 3 + rnd(6);
    IKEngine e;
    TPointD p(0, 0);
    e.setRoot(p);
    for (int i = 1; i < n; i++) {
      p = TPointD(p.x + 5 + unit() * 5, p.y + (unit() - 0.5) * 6);
      e.addJoint(p, i - 1);
    }
    if (rep % 3 == 1 && n > 4) e.lock(rnd(n - 2) + 1);
    TPointD target(unit() * 40, (unit() - 0.5) * 40);
    e.drag(target);
    printf("drag %d %d |", rep, n);
    for (int i = 0; i < n; i++) pd(e.getJoint(i).x), pd(e.getJoint(i).y);
    printf("\n");
  }
}

int main(int argc, char **argv) {
  bool drag = argc > 1 && !strcmp(argv[1], "drag");
  if (drag) {
    drag_ops();
    return 0;
  }
  linear_ops();
  svd_ops();
  jacobian_ops();
  angle_ops();
  fprintf(stderr, "skipped (non-terminating in the original): %d\n", g_skipped);
  return 0;
}
