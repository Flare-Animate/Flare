

#include "tgeometry.h"


using namespace std;

const T3DPointD TConsts::nap3d((numeric_limits<double>::max)(),
                               (numeric_limits<double>::max)(),
                               (numeric_limits<double>::max)());

const TThickPoint TConsts::natp((numeric_limits<double>::max)(),
                                (numeric_limits<double>::max)(),
                                (numeric_limits<double>::max)());

const TPointD TConsts::napd((numeric_limits<double>::max)(),
                            (numeric_limits<double>::max)());

const TPointI TConsts::nap((numeric_limits<int>::max)(),
                           (numeric_limits<int>::max)());

const TRectD TConsts::infiniteRectD(-(numeric_limits<double>::max)(),
                                    -(numeric_limits<double>::max)(),
                                    (numeric_limits<double>::max)(),
                                    (numeric_limits<double>::max)());

const TRectI TConsts::infiniteRectI(-(numeric_limits<int>::max)(),
                                    -(numeric_limits<int>::max)(),
                                    (numeric_limits<int>::max)(),
                                    (numeric_limits<int>::max)());

//==================================================================================================

// operazioni fra affini
TAffine &TAffine::operator=(const TAffine &a) {
  a11 = a.a11;
  a12 = a.a12;
  a13 = a.a13;
  a21 = a.a21;
  a22 = a.a22;
  a23 = a.a23;
  return *this;
}
//--------------------------------------------------------------------------------------------------
TAffine TAffine::operator*(const TAffine &b) const {
  return TAffine(a11 * b.a11 + a12 * b.a21, a11 * b.a12 + a12 * b.a22,
                 a11 * b.a13 + a12 * b.a23 + a13,

                 a21 * b.a11 + a22 * b.a21, a21 * b.a12 + a22 * b.a22,
                 a21 * b.a13 + a22 * b.a23 + a23);
}
//--------------------------------------------------------------------------------------------------
TAffine TAffine::operator*=(const TAffine &b) { return *this = *this * b; }
//--------------------------------------------------------------------------------------------------
double TAffine::det() const { return a11 * a22 - a12 * a21; }

//--------------------------------------------------------------------------------------------------

// Confronti tra affini
bool TAffine::operator==(const TAffine &a) const {
  return a11 == a.a11 && a12 == a.a12 && a13 == a.a13 && a21 == a.a21 &&
         a22 == a.a22 && a23 == a.a23;
}
//--------------------------------------------------------------------------------------------------
bool TAffine::operator!=(const TAffine &a) const {
  return a11 != a.a11 || a12 != a.a12 || a13 != a.a13 || a21 != a.a21 ||
         a22 != a.a22 || a23 != a.a23;
}
//--------------------------------------------------------------------------------------------------
bool TAffine::isIdentity(double err) const {
  return ((a11 - 1.0) * (a11 - 1.0) + (a22 - 1.0) * (a22 - 1.0) + a12 * a12 +
          a13 * a13 + a21 * a21 + a23 * a23) <+ err;
}
//--------------------------------------------------------------------------------------------------
bool TAffine::isZero(double err) const {
  return ( a11*a11 + a12*a12 + a13*a13 +
           a21*a21 + a22*a22 + a23*a23 ) <= err;
}
//--------------------------------------------------------------------------------------------------
bool TAffine::isTranslation(double err) const {
  return ((a11 - 1.0) * (a11 - 1.0) + (a22 - 1.0) * (a22 - 1.0) + a12 * a12 +
          a21 * a21) <= err;
}
//--------------------------------------------------------------------------------------------------
bool TAffine::isIsotropic(double err) const {
  return areAlmostEqual(a11, a22, err) && areAlmostEqual(a12, -a21, err);
}

//--------------------------------------------------------------------------------------------------

// applicazione
TPointD TAffine::operator*(const TPointD &p) const {
  return TPointD(p.x * a11 + p.y * a12 + a13, p.x * a21 + p.y * a22 + a23);
}

//--------------------------------------------------------------------------------------------------

TPointD TAffine::transformDirection(const TPointD &p) const {
  return TPointD(p.x * a11 + p.y * a12, p.x * a21 + p.y * a22);
}

//--------------------------------------------------------------------------------------------------

TRectD TAffine::operator*(const TRectD &rect) const {
  if (rect != TConsts::infiniteRectD) {
    TPointD p1 = *this * rect.getP00(), p2 = *this * rect.getP01(),
            p3 = *this * rect.getP10(), p4 = *this * rect.getP11();
    return TRectD(
        std::min({p1.x, p2.x, p3.x, p4.x}), std::min({p1.y, p2.y, p3.y, p4.y}),
        std::max({p1.x, p2.x, p3.x, p4.x}), std::max({p1.y, p2.y, p3.y, p4.y}));
  } else
    return TConsts::infiniteRectD;
}

//--------------------------------------------------------------------------------------------------

TAffine TAffine::place(double u, double v, double x, double y) const {
  return TAffine(a11, a12, x - (a11 * u + a12 * v), a21, a22,
                 y - (a21 * u + a22 * v));
}

//--------------------------------------------------------------------------------------------------

TAffine TAffine::place(const TPointD &pIn, const TPointD &pOut) const {
  return TAffine(a11, a12, pOut.x - (a11 * pIn.x + a12 * pIn.y), a21, a22,
                 pOut.y - (a21 * pIn.x + a22 * pIn.y));
}

//--------------------------------------------------------------------------------------------------

//==================================================================================================

// TAffine::inv / rotation, TRotation / TScale (about a centre), TAffine3, TAffine4 and
// TAngleRangeSet live in tgeometry.rs (inline forwarders at the end of tgeometry.h).
