#pragma once

#ifndef TMATH_UTIL_H
#define TMATH_UTIL_H

//#include "tcommon.h"
#include "texception.h"
#include "tconvert.h"

#include <numeric>
#include <complex>

#ifndef __sgi
#include <cmath>
#else
#include <math.h>
#endif

#undef DVAPI
#undef DVVAR
#ifdef TNZCORE_EXPORTS
#define DVAPI DV_EXPORT_API
#define DVVAR DV_EXPORT_VAR
#else
#define DVAPI DV_IMPORT_API
#define DVVAR DV_IMPORT_VAR
#endif

enum TMathError { INFINITE_SOLUTIONS = -1 };

//-----------------------------------------------------------------------------
// Implemented in tmathutil.rs (flare_formats); the functions below are thin
// inline forwarders so every C++ caller compiles unchanged.  The Rust side
// reports a singular matrix with a status code and the forwarders `throw`.

extern "C" {
int flare_tLUDecomposition(double *A, int n, int *indx, double *d);
void flare_tbackSubstitution(const double *A, int n, const int *indx,
                             double *b);
int flare_tsolveSistem(double *A, int n, double *res);
int flare_tdet(double *A, int n, double *out);
double flare_tdet_lu(const double *LUa, int n, double d);
int flare_rootFinding(const double *poly, int n, double *out, int cap,
                      int *outCount);
int flare_solveEquation2(double *roots, double a, double b, double c);
int flare_solveEquation3(double *roots, double a, double b, double c,
                         double d);
int flare_solveEquation4(double *roots, double a, double b, double c, double d,
                         double e);
int flare_numberOfRootsInInterval(int order, const double *polyH, double min,
                                  double max);
double flare_cubicRoot(double a, double b, double c, double d);
double flare_quadraticRoot(double a, double b, double c);
}

//-----------------------------------------------------------------------------

// (header-only now: no DVAPI, the members are inline)
class TMathException final : public TException {
  TString m_msg;

public:
  TMathException(std::string msg) : m_msg(::to_wstring(msg)) {}
  virtual ~TMathException() {}

  TString getMessage() const override { return m_msg; }
};

//-----------------------------------------------------------------------------

/*!
  Decompose a square matrix [A] dim(A)=[n x n]  using the LU method.
  A is a matrix stored for row.
  \par A array of coeff
  \par n size of matrix
  \par indx array with permutation from pivoting
  \par d +1/-1 and depend whether the number of row interchanges (even or odd)
  \note If error a TException is thrown.
  \include mathutils_ex2.cpp
 */
inline void tLUDecomposition(double *A, int n, int *indx, double &d) {
  if (flare_tLUDecomposition(A, n, indx, &d))
    throw TMathException("Singular matrix in routine tLUDecomposition\n");
}

//-----------------------------------------------------------------------------

/*!
  Use the back-substitution method to solve the linear
  system A*x=b.
  \par A matrix exited from tLUDecomposition
  \par n size of matrix
  \par indx array exited from tLUDecomposition
  \par b in input is array of element
  \ret b in output contains result
  \note A is the square matrix [n x n] returned from tLUDecomposition routine.
  \sa tLUDecomposition
 */
inline void tbackSubstitution(double *A, int n, int *indx, double *b) {
  flare_tbackSubstitution(A, n, indx, b);
}

//-----------------------------------------------------------------------------

/*!
  Solve the system A*x=b.
  It`s needed to insert b in res.
  A and res are modified in the routine.
  \par A matrix
  \par n size of matrix
  \note A is square [n x n]
  \include mathutils_ex1.cpp
 */
inline void tsolveSistem(double *A, int n, double *res) {
  if (flare_tsolveSistem(A, n, res))
    throw TMathException("Singular matrix in routine tLUDecomposition\n");
}

//-----------------------------------------------------------------------------

/*!
  Solve the system A*x=b.
  It`s needed to insert b in res.
  A and res are modified in the routine.
  \note A is square [n x n]
  \include mathutils_ex1.cpp
 */
inline void tsolveSistem(std::vector<double> &A, std::vector<double> &res) {
  assert(res.size() * res.size() == A.size());
  tsolveSistem(&A[0], res.size(), &res[0]);
}

//-----------------------------------------------------------------------------

/*!
  Find determinant of square matrix A
  \par A is matrix to test
  \par n size of matrix
 */
inline double tdet(double *A, int n) {
  double d;
  if (flare_tdet(A, n, &d))
    throw TMathException("Singular matrix in routine tLUDecomposition\n");
  return d;
}

//-----------------------------------------------------------------------------

/*!
  Find determinant of square matrix A [n x n], using information
  from tLUDecomposition decomposition.
  \par LUa output matrix from tLUDecomposition
  \par n size of matrix
  \par d parameter from tLUDecomposition
  \sa tLUDecomposition
 */
inline double tdet(double *LUa, int n, double d) {
  return flare_tdet_lu(LUa, n, d);
}

//-----------------------------------------------------------------------------

/*!
  Find real root of a polynomious
  \par poly is a vector with coeff of polynomious in crescent order.
  \ret sol is the vector of solution and contain real found solution.
 */
inline int rootFinding(const std::vector<double> &poly,
                       std::vector<double> &sol) {
  double out[16];
  int count = -1;
  int ret   = flare_rootFinding(poly.data(), (int)poly.size(), out, 16, &count);
  if (count >= 0) sol.assign(out, out + count);  // count < 0: sol untouched
  return ret;
}

//-----------------------------------------------------------------------------

/*!
  Find complex roots of a quadratic equation
    a*x^2 + b*x + c = 0
  \ret count of roots found:
    0 if a and b are almost zero
    1 if a is almost zero
    2 if a is not almost zero
    roots may have duplicates, count depends only from type of equation
 */
inline int solveEquation2(std::complex<double> *roots, double a, double b,
                          double c) {
  return flare_solveEquation2(reinterpret_cast<double *>(roots), a, b, c);
}

//-----------------------------------------------------------------------------

/*!
  Find complex roots of a cubic equation:
    a*x^3 + b*x^2 + c*x + d = 0
  \ret count of roots found:
    0 if a, b and c are almost zero
    1 if a and b are almost zero
    2 if a is almost zero
    3 if a is not almost zero
    roots may have duplicates, count depends only from type of equation
 */
inline int solveEquation3(std::complex<double> *roots, double a, double b,
                          double c, double d) {
  return flare_solveEquation3(reinterpret_cast<double *>(roots), a, b, c, d);
}

//-----------------------------------------------------------------------------

/*!
  Find complex roots of a power of four equation:
    a*x^4 + b*x^3 + c*x^2 + d*x + e = 0
  \ret count of roots found:
    0 if a, b, c and d are almost zero
    1 if a, b and c are almost zero
    2 if a and b are almost zero
    3 if a is almost zero
    4 if a is not almost zero
    roots may have duplicates, count depends only from type of equation
 */
inline int solveEquation4(std::complex<double> *roots, double a, double b,
                          double c, double d, double e) {
  return flare_solveEquation4(reinterpret_cast<double *>(roots), a, b, c, d, e);
}

//-----------------------------------------------------------------------------

/*!
  Check if val is nearest to epsilon.
  \par val value to test
  \par eps tolerance required
  \ret true if value is nearest to zero
 */
inline bool isAlmostZero(double val, double eps = TConsts::epsilon) {
  return -eps < val && val < eps;
}

//-----------------------------------------------------------------------------

/*!
  Check if complex val is nearest to epsilon.
  \par val value to test
  \par eps tolerance required
  \ret true if value is nearest to zero
 */
inline bool isAlmostZero(std::complex<double> val, double eps = TConsts::epsilon) {
  return val.real()*val.real() + val.imag()*val.imag() < eps*eps;
}


//-----------------------------------------------------------------------------

/*!
  Find number of roots of a poly in interval min max.
  \par order degree of polynomious
  \par coeff of poly in crescent order
  \par lesser extreme
  \par greater extreme
  \ret number of root
 */
inline int numberOfRootsInInterval(int order, const double *polyH, double min,
                                   double max) {
  return flare_numberOfRootsInInterval(order, polyH, min, max);
}

//-----------------------------------------------------------------------------

/*!
  Find the real root of the poly a*x^3+b*x^2+c*x+d in [0,1]
  \par a coeff of x^3
  \par b coeff of x^2
  \par c coeff of x^1
  \par d coeff of x^0
  \ret the real root of the poly, or 1.0 if the root is not in [0,1]
*/
inline double cubicRoot(double a, double b, double c, double d) {
  return flare_cubicRoot(a, b, c, d);
}

//-----------------------------------------------------------------------------

/*!
  Find the root of the poly a*x^2+b*x+c
  \par a coeff of x^2
  \par b coeff of x^1
  \par c coeff of x^0
  \ret the root of the poly, or 1.0 if it does not exist
*/
inline double quadraticRoot(double a, double b, double c) {
  return flare_quadraticRoot(a, b, c);
}

//-----------------------------------------------------------------------------

#endif  // #ifndef TMATH_UTIL_H
//-----------------------------------------------------------------------------
// End Of File
//-----------------------------------------------------------------------------
