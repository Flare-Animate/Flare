"""Regenerate/check flare/sources/tnzext/potentials_cases.txt from the original C++ (g++)."""
import rw3_golden as g

NAMES = ["Potential", "LinearPotential", "SquarePotential", "NotSymmetricBezierPotential",
         "NotSymmetricExpPotential"]
STUB_STROKE = """#pragma once
// test double for TStroke::getLength (monotone, smooth, exact at 0 and 1)
class TStroke {
  double L;
public:
  explicit TStroke(double l) : L(l) {}
  double getLength(double w) const { return L * (0.7 * w + 0.3 * w * w); }
  double getLength() const { return getLength(1.0); }
};
"""
STUB_CURVES = """#pragma once
#include "tgeometry.h"
// TQuadratic::getPoint verbatim from common/tgeometry/tcurves.cpp
class TQuadratic {
  TPointD m_p0, m_p1, m_p2;
public:
  void setP0(const TPointD &p) { m_p0 = p; }
  void setP1(const TPointD &p) { m_p1 = p; }
  void setP2(const TPointD &p) { m_p2 = p; }
  TPointD getPoint(double t) const {
    double s = 1 - t;
    return m_p0 * s * s + 2 * t * s * m_p1 + t * t * m_p2;
  }
};
"""
STUB_MATH = """#pragma once
// verbatim from tmathutil.h / tutil.h (the real header drags in Qt via tconvert.h)
#include <cmath>
#include "tcommon.h"
#include "tutil.h"  // sq, areAlmostEqual
inline bool isAlmostZero(double val, double eps = TConsts::epsilon) { return -eps < val && val < eps; }
"""
srcs = []
hdr = {}
for n in NAMES:
    srcs.append(f"flare/sources/tnzext/{n}.cpp")
    h = f"flare/sources/include/ext/{n}.h"
    srcs.append(h)
    hdr[h] = f"ext/{n}.h"
raise SystemExit(g.run(
    "potentials", "tnzext/potentials_cases.txt", srcs, hdr_as=hdr,
    stubs={"tstroke.h": STUB_STROKE, "tcurves.h": STUB_CURVES, "tmathutil.h": STUB_MATH},
    compile=[f"{n}.cpp" for n in NAMES],
))
