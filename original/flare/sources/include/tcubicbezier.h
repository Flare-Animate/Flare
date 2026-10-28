#pragma once

#ifndef TCUBICBEZIER_INCLUDED
#define TCUBICBEZIER_INCLUDED

//-----------------------------------------------------------------------------

#include "tmathutil.h"
//#include "tcommon.h"
#include "tgeometry.h"
//#include <algorithm>

using TConsts::epsilon;

// Implemented in tcubicbezier.rs (flare_formats); the functions below are thin
// inline forwarders (TPointD arguments are passed as x/y scalars).
extern "C" {
double flare_invCubicBezierX(double x, double ax, double ay, double asx,
                             double asy, double bsx, double bsy, double bx,
                             double by);
double flare_getCubicBezierY(double x, double ax, double ay, double asx,
                             double asy, double bsx, double bsy, double bx,
                             double by);
void flare_getMinMaxCubicBezierY(double ax, double ay, double asx, double asy,
                                 double bsx, double bsy, double bx, double by,
                                 double *out);
}

//-----------------------------------------------------------------------------

/*!
  Find the value of the parameter u0 of the cubic Bezier curve C(u)
  for a specified value x, with C(u0) = (x,y).
  \par x the x-component of a point on the Bezier curve
  \par a first control point of the Bezier curve
  \par aSpeed the tangent vector in a (that is the first control point minus the
  second)
  \par b the third control point
  \par bSpeed the tangent vector in b (that is the third control point minus the
  fourth)
  \ret the parameter value u0 of the Bezier curve C(u) for x, that is C(u0) =
  (x,y)
*/
inline double invCubicBezierX(double x, const TPointD &a, const TPointD &aSpeed,
                              const TPointD &bSpeed, const TPointD &b) {
  return flare_invCubicBezierX(x, a.x, a.y, aSpeed.x, aSpeed.y, bSpeed.x,
                               bSpeed.y, b.x, b.y);
}

//-----------------------------------------------------------------------------

/*!
  for C(u) a cubic Bezier curve, C(u0) = (x,y), find y from x.
  \par x the x-component of a point on the Bezier curve
  \par a first control point of the Bezier curve
  \par aSpeed the tangent vector in a (that is the first control point minus the
  second)
  \par b the third control point
  \par bSpeed the tangent vector in b (that is the third control point minus the
  fourth)
  \ret the y-component of a point P(x,y) on the Bezier curve, with C(u0) = P
*/
inline double getCubicBezierY(double x, const TPointD &a, const TPointD &aSpeed,
                              const TPointD &bSpeed, const TPointD &b) {
  return flare_getCubicBezierY(x, a.x, a.y, aSpeed.x, aSpeed.y, bSpeed.x,
                               bSpeed.y, b.x, b.y);
}

//-----------------------------------------------------------------------------

/*!
  for C(u)=(x,y) a cubic Bezier curve, find minimum and maximum y-values of C(u)
  \par a first control point of the Bezier curve
  \par aSpeed the tangent vector in a (that is the first control point minus the
  second)
  \par b the third control point
  \par bSpeed the tangent vector in b (that is the third control point minus the
  fourth)
  \ret a pair of points which are the minimum and maximum of the cubic Bezier
*/
inline std::pair<TPointD, TPointD> getMinMaxCubicBezierY(
    const TPointD &a, const TPointD &aSpeed, const TPointD &bSpeed,
    const TPointD &b) {
  double out[4];
  flare_getMinMaxCubicBezierY(a.x, a.y, aSpeed.x, aSpeed.y, bSpeed.x, bSpeed.y,
                              b.x, b.y, out);
  return std::pair<TPointD, TPointD>(TPointD(out[0], out[1]),
                                     TPointD(out[2], out[3]));
}

//-----------------------------------------------------------------------------

#endif  // TCUBICBEZIER_INCLUDED

//-----------------------------------------------------------------------------
