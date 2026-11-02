#pragma once

#ifndef JACOBIAN_H
#define JACOBIAN_H

#include <stdint.h>
#include <stddef.h>
#include <math.h>
#include <assert.h>
#include <algorithm>
#include <stdexcept>
#include <vector>

#include "iknode.h"
#include "ikskeleton.h"
#include "tgeometry.h"

// The numeric kernels of the former flarelib/ikjacobian.cpp live in
// flarelib/ikjacobian.rs (flare_formats). Every function returns 0 on success, 1 for a
// null / negative / oversized argument, 2 for an out-of-range access or caught panic,
// 3 for a shape the original only assert()ed on. `long` travels as int64_t.
extern "C" {
int flare_ikjac_vec_max_abs(double *x, int64_t n, double *out);
int flare_ikjac_set_diagonal(double *x, int64_t r, int64_t c, double d);
int flare_ikjac_set_diagonal_vec(double *x, int64_t r, int64_t c, double *d, int64_t dn);
int flare_ikjac_set_super_diagonal(double *x, int64_t r, int64_t c, double d);
int flare_ikjac_set_super_diagonal_vec(double *x, int64_t r, int64_t c, double *d, int64_t dn);
int flare_ikjac_set_sub_diagonal(double *x, int64_t r, int64_t c, double d);
int flare_ikjac_set_sub_diagonal_vec(double *x, int64_t r, int64_t c, double *d, int64_t dn);
int flare_ikjac_set_column(double *x, int64_t r, int64_t c, int64_t i, double *d, int64_t dn);
int flare_ikjac_set_row(double *x, int64_t r, int64_t c, int64_t i, double *d, int64_t dn);
int flare_ikjac_set_sequence(double *x, int64_t r, int64_t c, double *d, int64_t dn,
                             int64_t startRow, int64_t startCol, int64_t deltaRow,
                             int64_t deltaCol);
int flare_ikjac_load_as_submatrix(double *x, int64_t r, int64_t c, double *a, int64_t ar,
                                  int64_t ac);
int flare_ikjac_load_as_submatrix_transpose(double *x, int64_t r, int64_t c, double *a,
                                            int64_t ar, int64_t ac);
int flare_ikjac_frobenius_norm(double *x, int64_t r, int64_t c, double *out);
int flare_ikjac_multiply_vec(double *x, int64_t r, int64_t c, double *v, int64_t vn,
                             double *res, int64_t rn);
int flare_ikjac_multiply_transpose_vec(double *x, int64_t r, int64_t c, double *v,
                                       int64_t vn, double *res, int64_t rn);
int flare_ikjac_dot_product_column(double *x, int64_t r, int64_t c, double *v, int64_t vn,
                                   int64_t col, double *out);
int flare_ikjac_add_to_diagonal(double *x, int64_t r, int64_t c, double d);
int flare_ikjac_add_to_diagonal_vec(double *x, int64_t r, int64_t c, double *v, int64_t vn);
int flare_ikjac_multiply_scalar(double *a, int64_t ar, int64_t ac, double k, double *dst,
                                int64_t dr, int64_t dc);
int flare_ikjac_multiply(double *a, int64_t ar, int64_t ac, double *b, int64_t br,
                         int64_t bc, double *dst, int64_t dr, int64_t dc);
int flare_ikjac_transpose_multiply(double *a, int64_t ar, int64_t ac, double *b,
                                   int64_t br, int64_t bc, double *dst, int64_t dr,
                                   int64_t dc);
int flare_ikjac_multiply_transpose(double *a, int64_t ar, int64_t ac, double *b,
                                   int64_t br, int64_t bc, double *dst, int64_t dr,
                                   int64_t dc);
int flare_ikjac_solve(double *x, int64_t r, int64_t c, double *b, int64_t bn, double *xv,
                      int64_t xn);
int flare_ikjac_convert_to_ref_no_free(double *x, int64_t r, int64_t c);
int flare_ikjac_calc_givens_values(double a, double b, double *c, double *s);
int flare_ikjac_post_apply_givens(double *x, int64_t r, int64_t c, double cs, double sn,
                                  int64_t idx1, int64_t idx2);
int flare_ikjac_compute_svd(double *a, int64_t ar, int64_t ac, double *u, int64_t ur,
                            int64_t uc, double *w, int64_t wn, double *v, int64_t vr,
                            int64_t vc);
int flare_ikjac_debug_check_svd(double *a, int64_t ar, int64_t ac, double *u, int64_t ur,
                                int64_t uc, double *w, int64_t wn, double *v, int64_t vr,
                                int64_t vc, int *ok);
int flare_ikjac_reset(double *lambda, double *lambdaSq, double *dampSqV, int64_t dn,
                      double *diagId, int64_t in, double *dsClamp, int64_t cn);
int flare_ikjac_compute_jacobian(double *jend, int64_t nrow, int64_t ncol, double *ds,
                                 int64_t dsn, int64_t effectorCount, size_t nEff,
                                 const int *effNum, const double *effS,
                                 const size_t *effNanc, size_t nAnc, const int *ancJoint,
                                 const unsigned char *ancFrozen, const double *ancS,
                                 const double *target, size_t ntarget);
int flare_ikjac_calc_transpose(double *jx, int64_t jr, int64_t jc, double *ds, int64_t dsn,
                               double *dt, int64_t dtn, double *dth, int64_t dthn);
int flare_ikjac_calc_pseudoinverse(double *jx, int64_t jr, int64_t jc, int64_t numEffector,
                                   double *ds, int64_t dsn, double *dth, int64_t dthn,
                                   double *diagId, int64_t din);
int flare_ikjac_calc_dls(double *jx, int64_t jr, int64_t jc, double *ux, int64_t ur,
                         int64_t uc, double *dampV, int64_t dampn, double *ds, int64_t dsn,
                         double *dt, int64_t dtn, double *dth, int64_t dthn);
int flare_ikjac_calc_dls_with_svd(double *jx, int64_t jr, int64_t jc, double *ux,
                                  int64_t ur, int64_t uc, double *w, int64_t wn, double *vx,
                                  int64_t vr, int64_t vc, double dampingLambdaSq,
                                  double *ds, int64_t dsn, double *dth, int64_t dthn);
int flare_ikjac_calc_sdls(double *jx, int64_t jr, int64_t jc, double *ux, int64_t ur,
                          int64_t uc, double *w, int64_t wn, double *vx, int64_t vr,
                          int64_t vc, int64_t numEffector, double *jn, int64_t jnr,
                          int64_t jnc, double *ds, int64_t dsn, double *dt, int64_t dtn,
                          double *dsClamp, int64_t dcn, double *dpre, int64_t dpn,
                          double *dth, int64_t dthn);
int flare_ikjac_calc_dt_clamped(double *ds, int64_t dsn, double *dt, int64_t dtn,
                                double *dsClamp, int64_t dcn);
int flare_ikjac_updated_sclamp_one(int64_t i, double sx, double sy, double tx, double ty,
                                   double *ds, int64_t dsn, double *dsClamp, int64_t dcn);
}

// Status of a flare_ikjac_* call: the old code had undefined behaviour (or an assert) on
// the inputs that make Rust return non-zero, so those become exceptions.
inline void flareIkjacCheck(int rc) {
  if (rc == 0) return;
  if (rc == 1)
    throw std::invalid_argument("ikjacobian: null or invalid buffer argument");
  if (rc == 3)
    throw std::invalid_argument("ikjacobian: matrix/vector shape mismatch");
  throw std::out_of_range("ikjacobian: out-of-range access in IK kernel");
}

#undef DVAPI
#undef DVVAR
#ifdef FLARELIB_EXPORTS
#define DVAPI DV_EXPORT_API
#define DVVAR DV_EXPORT_VAR
#else
#define DVAPI DV_IMPORT_API
#define DVVAR DV_IMPORT_VAR
#endif

//******************** IK Utility *********************
//*****************************************************

//***********************************************************************
// CLASS VectorRN
// void DVAPI Dump(const TPointD &point, double* v);

// Header-only now (every member is inline): no DVAPI, so MSVC never sees a dllimport
// class whose members are defined inline out of class.
class VectorRn {
  friend class MatrixRmn;

public:
  VectorRn();             // Null constructor
  VectorRn(long length);  // Constructor with length
  ~VectorRn();            // Destructor

  void SetLength(long newLength);
  long GetLength() const { return length; }

  void SetZero();
  void Fill(double d);
  void Load(const double *d);
  void LoadScaled(const double *d, double scaleFactor);
  void Set(const VectorRn &src);

  // Two access methods identical in functionality
  // Subscripts are ZERO-BASED!!
  const double &operator[](long i) const {
    assert(0 <= i && i < length);
    return *(x + i);
  }
  double &operator[](long i) {
    assert(0 <= i && i < length);
    return *(x + i);
  }
  double Get(long i) const {
    assert(0 <= i && i < length);
    return *(x + i);
  }

  // Use GetPtr to get pointer into the array (efficient)
  // Is friendly in that anyone can change the array contents (be careful!)
  const double *GetPtr(long i) const {
    assert(0 <= i && i < length);
    return (x + i);
  }
  double *GetPtr(long i) {
    assert(0 <= i && i < length);
    return (x + i);
  }
  const double *GetPtr() const { return x; }
  double *GetPtr() { return x; }

  void Set(long i, double val) { assert(0 <= i && i < length), *(x + i) = val; }
  void SetCouple(long i, const TPointD &u);

  VectorRn &operator+=(const VectorRn &src);
  VectorRn &operator-=(const VectorRn &src);
  void AddScaled(const VectorRn &src, double scaleFactor);

  VectorRn &operator*=(double f);
  double NormSq() const;
  double Norm() const { return sqrt(NormSq()); }

  double MaxAbs() const {
    double result = 0.0;
    flareIkjacCheck(flare_ikjac_vec_max_abs(x, (int64_t)length, &result));
    return result;
  }

private:
  long length;       // Logical or actual length
  long AllocLength;  // Allocated length
  double *x;         // Array of vector entries
};

inline VectorRn::VectorRn() {
  length      = 0;
  AllocLength = 0;
  x           = 0;
}

inline VectorRn::VectorRn(long initLength) {
  length      = 0;
  AllocLength = 0;
  x           = 0;
  SetLength(initLength);
}

inline VectorRn::~VectorRn() { delete x; }

// Resize.
// If the array is shortened, the information about the allocated length is
// lost.
inline void VectorRn::SetLength(long newLength) {
  assert(newLength > 0);
  if (newLength > AllocLength) {
    delete x;
    AllocLength = std::max(newLength, AllocLength << 1);
    x           = new double[AllocLength];
  }
  length = newLength;
}

// Zero out the entire vector
inline void VectorRn::SetZero() {
  double *target = x;
  for (long i = length; i > 0; i--) {
    *(target++) = 0.0;
  }
}

// Set the value of the i-th triple of entries in the vector
inline void VectorRn::SetCouple(long i, const TPointD &u) {
  long j = 2 * i;
  assert(0 <= j && j + 1 < length);
  x[j]     = u.x;
  x[j + 1] = u.y;
}

inline void VectorRn::Fill(double d) {
  double *to = x;
  for (long i = length; i > 0; i--) {
    *(to++) = d;
  }
}

inline void VectorRn::Load(const double *d) {
  double *to = x;
  for (long i = length; i > 0; i--) {
    *(to++) = *(d++);
  }
}

inline void VectorRn::LoadScaled(const double *d, double scaleFactor) {
  double *to = x;
  for (long i = length; i > 0; i--) {
    *(to++) = (*(d++)) * scaleFactor;
  }
}

inline void VectorRn::Set(const VectorRn &src) {
  assert(src.length == this->length);
  double *to   = x;
  double *from = src.x;
  for (long i = length; i > 0; i--) {
    *(to++) = *(from++);
  }
}

inline VectorRn &VectorRn::operator+=(const VectorRn &src) {
  assert(src.length == this->length);
  double *to   = x;
  double *from = src.x;
  for (long i = length; i > 0; i--) {
    *(to++) += *(from++);
  }
  return *this;
}

inline VectorRn &VectorRn::operator-=(const VectorRn &src) {
  assert(src.length == this->length);
  double *to   = x;
  double *from = src.x;
  for (long i = length; i > 0; i--) {
    *(to++) -= *(from++);
  }
  return *this;
}

inline void VectorRn::AddScaled(const VectorRn &src, double scaleFactor) {
  assert(src.length == this->length);
  double *to   = x;
  double *from = src.x;
  for (long i = length; i > 0; i--) {
    *(to++) += (*(from++)) * scaleFactor;
  }
}

inline VectorRn &VectorRn::operator*=(double f) {
  double *target = x;
  for (long i = length; i > 0; i--) {
    *(target++) *= f;
  }
  return *this;
}

inline double VectorRn::NormSq() const {
  double *target = x;
  double res     = 0.0;
  for (long i = length; i > 0; i--) {
    res += (*target) * (*target);
    target++;
  }
  return res;
}

inline double Dot(const VectorRn &u, const VectorRn &v) {
  assert(u.GetLength() == v.GetLength());
  double res      = 0.0;
  const double *p = u.GetPtr();
  const double *q = v.GetPtr();
  for (long i = u.GetLength(); i > 0; i--) {
    res += (*(p++)) * (*(q++));
  }
  return res;
}

//*******************************************************************
// MatrixRmn

class MatrixRmn {
public:
  MatrixRmn();                            // Null constructor
  MatrixRmn(long numRows, long numCols);  // Constructor with length
  ~MatrixRmn();                           // Destructor

  void SetSize(long numRows, long numCols);
  long getNumRows() const { return NumRows; }
  long getNumColumns() const { return NumCols; }
  void SetZero();

  // Return entry in row i and column j.
  double Get(long i, long j) const;
  void GetCouple(long i, long j, TPointD *retValue) const;

  // Use GetPtr to get pointer into the array (efficient)
  // Is friendly in that anyone can change the array contents (be careful!)
  // The entries are in column order!!!
  // Use this with care.  You may call GetRowStride and GetColStride to navigate
  //			within the matrix.  I do not expect these values to ever
  // change.
  const double *GetPtr() const;
  double *GetPtr();
  const double *GetPtr(long i, long j) const;
  double *GetPtr(long i, long j);
  const double *GetColumnPtr(long j) const;
  double *GetColumnPtr(long j);
  const double *GetRowPtr(long i) const;
  double *GetRowPtr(long i);
  long GetRowStride() const {
    return NumRows;
  }                                        // Step size (stride) along a row
  long GetColStride() const { return 1; }  // Step size (stide) along a column

  void Set(long i, long j, double val);
  void SetCouple(long i, long c, const TPointD &u);

  void SetIdentity();
  void SetDiagonalEntries(double d);
  void SetDiagonalEntries(const VectorRn &d);
  void SetSuperDiagonalEntries(double d);
  void SetSuperDiagonalEntries(const VectorRn &d);
  void SetSubDiagonalEntries(double d);
  void SetSubDiagonalEntries(const VectorRn &d);
  void SetColumn(long i, const VectorRn &d);
  void SetRow(long i, const VectorRn &d);
  void SetSequence(const VectorRn &d, long startRow, long startCol,
                   long deltaRow, long deltaCol);

  // Loads matrix in as a sub-matrix.  (i,j) is the base point. Defaults to
  // (0,0).
  // The "Transpose" versions load the transpose of A.
  void LoadAsSubmatrix(const MatrixRmn &A);
  void LoadAsSubmatrix(long i, long j, const MatrixRmn &A);
  void LoadAsSubmatrixTranspose(const MatrixRmn &A);
  void LoadAsSubmatrixTranspose(long i, long j, const MatrixRmn &A);

  // Norms
  double FrobeniusNormSq() const;
  double FrobeniusNorm() const;

  // Operations on VectorRn's
  void Multiply(const VectorRn &v,
                VectorRn &result) const;  // result = (this)*(v)
  void MultiplyTranspose(const VectorRn &v, VectorRn &result)
      const;  // Equivalent to mult by row vector on left
  double DotProductColumn(const VectorRn &v, long colNum)
      const;  // Returns dot product of v with i-th column

  // Operations on MatrixRmn's
  MatrixRmn &operator*=(double);
  MatrixRmn &operator/=(double d) {
    assert(d != 0.0);
    *this *= (1.0 / d);
    return *this;
  }
  MatrixRmn &AddScaled(const MatrixRmn &B, double factor);
  MatrixRmn &operator+=(const MatrixRmn &B);
  MatrixRmn &operator-=(const MatrixRmn &B);
  static MatrixRmn &Multiply(const MatrixRmn &A, const MatrixRmn &B,
                             MatrixRmn &dst);  // Sets dst = A*B.
  static MatrixRmn &MultiplyScalar(const MatrixRmn &A, double k,
                                   MatrixRmn &result);
  static MatrixRmn &MultiplyTranspose(
      const MatrixRmn &A, const MatrixRmn &B,
      MatrixRmn &dst);  // Sets dst = A*(B-tranpose).
  static MatrixRmn &TransposeMultiply(
      const MatrixRmn &A, const MatrixRmn &B,
      MatrixRmn &dst);  // Sets dst = (A-transpose)*B.

  // Miscellaneous operation
  MatrixRmn &AddToDiagonal(double d);  // Adds d to each diagonal
  MatrixRmn &AddToDiagonal(
      const VectorRn &v);  // Adds vector elements to diagonal

  // Solving systems of linear equations
  void Solve(const VectorRn &b, VectorRn *x) const;  // Solves the equation
                                                     // (*this)*x = b;    Uses
                                                     // row operations.  Assumes
                                                     // *this is invertible.

  // Row Echelon Form and Reduced Row Echelon Form routines
  // Row echelon form here allows non-negative entries (instead of 1's) in the
  // positions of lead variables.
  void ConvertToRefNoFree();  // Converts the matrix in place to row echelon
                              // form -- assumption is no free variables will be
                              // found
  void ConvertToRef(int numVars);  // Converts the matrix in place to row
                                   // echelon form -- numVars is number of
                                   // columns to work with.
  void ConvertToRef(
      int numVars,
      double eps);  // Same, but eps is the measure of closeness to zero

  // Givens transformation
  static void CalcGivensValues(double a, double b, double *c, double *s);
  void PostApplyGivens(
      double c, double s,
      long idx);  // Applies Givens transform to columns idx and idx+1.
  void PostApplyGivens(
      double c, double s, long idx1,
      long idx2);  // Applies Givens transform to columns idx1 and idx2.

  // Singular value decomposition
  void ComputeSVD(MatrixRmn &U, VectorRn &w, MatrixRmn &V) const;
  // Good for debugging SVD computations (I recommend this be used for any new
  // application to check for bugs/instability).
  bool DebugCheckSVD(const MatrixRmn &U, const VectorRn &w,
                     const MatrixRmn &V) const;

  // Some useful routines for experts who understand the inner workings of these
  // classes.
  inline static double DotArray(long length, const double *ptrA, long strideA,
                                const double *ptrB, long strideB);
  inline static void CopyArrayScale(long length, const double *from,
                                    long fromStride, double *to, long toStride,
                                    double scale);
  inline static void AddArrayScale(long length, const double *from,
                                   long fromStride, double *to, long toStride,
                                   double scale);

private:
  long NumRows;    // Number of rows
  long NumCols;    // Number of columns
  double *x;       // Array of vector entries - stored in column order
  long AllocSize;  // Allocated size of the x array

  // The SVD helpers (CalcBidiagonal, ConvertBidiagToDiagonal, SvdHouseholder,
  // ExpandHouseholders, UpdateBidiagIndices, ApplyGivensCBTD, Clear*WithDiagonalZero) and
  // the static WorkMatrix / WorkVector scratch objects were private to ikjacobian.cpp; they
  // are internal to ikjacobian.rs now (scratch buffers are locals there).
};

inline MatrixRmn::MatrixRmn() {
  NumRows   = 0;
  NumCols   = 0;
  x         = 0;
  AllocSize = 0;
}

inline MatrixRmn::MatrixRmn(long numRows, long numCols) {
  NumRows   = 0;
  NumCols   = 0;
  x         = 0;
  AllocSize = 0;
  SetSize(numRows, numCols);
}

inline MatrixRmn::~MatrixRmn() { delete x; }

// Resize.
// If the array space is decreased, the information about the allocated length
// is lost.
inline void MatrixRmn::SetSize(long numRows, long numCols) {
  assert(numRows > 0 && numCols > 0);
  long newLength = numRows * numCols;
  if (newLength > AllocSize) {
    delete x;
    AllocSize = std::max(newLength, AllocSize << 1);
    x         = new double[AllocSize];
  }
  NumRows = numRows;
  NumCols = numCols;
}

// Zero out the entire vector
inline void MatrixRmn::SetZero() {
  double *target = x;
  for (long i = NumRows * NumCols; i > 0; i--) {
    *(target++) = 0.0;
  }
}

// Return entry in row i and column j.
inline double MatrixRmn::Get(long i, long j) const {
  assert(i < NumRows && j < NumCols);
  return *(x + j * NumRows + i);
}

// Return a VectorR3 out of a column.  Starts at row 3*i, in column j.
inline void MatrixRmn::GetCouple(long i, long j, TPointD *retValue) const {
  assert(i < 0);  // messo perchè sono sicuro non entra mai in questa funzione!
                  // e quindi commento ->Load alla riga successiva
  long ii = 2 * i;
  assert(0 <= i && ii + 1 < NumRows && 0 <= j && j < NumCols);
  // retValue->Load( x+j*NumRows + ii );
}

// Get a pointer to the (0,0) entry.
// The entries are in column order.
// This version gives read-only pointer
inline const double *MatrixRmn::GetPtr() const { return x; }

// Get a pointer to the (0,0) entry.
// The entries are in column order.
inline double *MatrixRmn::GetPtr() { return x; }

// Get a pointer to the (i,j) entry.
// The entries are in column order.
// This version gives read-only pointer
inline const double *MatrixRmn::GetPtr(long i, long j) const {
  assert(0 <= i && i < NumRows && 0 <= j && j < NumCols);
  return (x + j * NumRows + i);
}

// Get a pointer to the (i,j) entry.
// The entries are in column order.
// This version gives pointer to writable data
inline double *MatrixRmn::GetPtr(long i, long j) {
  assert(i < NumRows && j < NumCols);
  return (x + j * NumRows + i);
}

// Get a pointer to the j-th column.
// The entries are in column order.
// This version gives read-only pointer
inline const double *MatrixRmn::GetColumnPtr(long j) const {
  assert(0 <= j && j < NumCols);
  return (x + j * NumRows);
}

// Get a pointer to the j-th column.
// This version gives pointer to writable data
inline double *MatrixRmn::GetColumnPtr(long j) {
  assert(0 <= j && j < NumCols);
  return (x + j * NumRows);
}

/// Get a pointer to the i-th row
// The entries are in column order.
// This version gives read-only pointer
inline const double *MatrixRmn::GetRowPtr(long i) const {
  assert(0 <= i && i < NumRows);
  return (x + i);
}

// Get a pointer to the i-th row
// This version gives pointer to writable data
inline double *MatrixRmn::GetRowPtr(long i) {
  assert(0 <= i && i < NumRows);
  return (x + i);
}

// Set the (i,j) entry of the matrix
inline void MatrixRmn::Set(long i, long j, double val) {
  assert(i < NumRows && j < NumCols);
  *(x + j * NumRows + i) = val;
}

// Set the i-th triple in the j-th column to u's three values
inline void MatrixRmn::SetCouple(long i, long j, const TPointD &u) {
  long ii = 2 * i;
  assert(0 <= i && ii + 1 < NumRows && 0 <= j && j < NumCols);
  // u.Dump( x+j*NumRows + ii );
  double *v = x + j * NumRows + ii;
  v[0]      = u.x;
  v[1]      = u.y;
}

// Set to be equal to the identity matrix
inline void MatrixRmn::SetIdentity() {
  assert(NumRows == NumCols);
  SetZero();
  SetDiagonalEntries(1.0);
}

inline MatrixRmn &MatrixRmn::operator*=(double mult) {
  double *aPtr = x;
  for (long i = NumRows * NumCols; i > 0; i--) {
    (*(aPtr++)) *= mult;
  }
  return (*this);
}

inline MatrixRmn &MatrixRmn::AddScaled(const MatrixRmn &B, double factor) {
  assert(NumRows == B.NumRows && NumCols == B.NumCols);
  double *aPtr = x;
  double *bPtr = B.x;
  for (long i = NumRows * NumCols; i > 0; i--) {
    (*(aPtr++)) += (*(bPtr++)) * factor;
  }
  return (*this);
}

inline MatrixRmn &MatrixRmn::operator+=(const MatrixRmn &B) {
  assert(NumRows == B.NumRows && NumCols == B.NumCols);
  double *aPtr = x;
  double *bPtr = B.x;
  for (long i = NumRows * NumCols; i > 0; i--) {
    (*(aPtr++)) += *(bPtr++);
  }
  return (*this);
}

inline MatrixRmn &MatrixRmn::operator-=(const MatrixRmn &B) {
  assert(NumRows == B.NumRows && NumCols == B.NumCols);
  double *aPtr = x;
  double *bPtr = B.x;
  for (long i = NumRows * NumCols; i > 0; i--) {
    (*(aPtr++)) -= *(bPtr++);
  }
  return (*this);
}

template <class T>
inline T Square(T x) {
  return (x * x);
}

inline double MatrixRmn::FrobeniusNormSq() const {
  double *aPtr  = x;
  double result = 0.0;
  for (long i = NumRows * NumCols; i > 0; i--) {
    result += Square(*(aPtr++));
  }
  return result;
}

// Helper routine to calculate dot product
inline double MatrixRmn::DotArray(long length, const double *ptrA, long strideA,
                                  const double *ptrB, long strideB) {
  double result = 0.0;
  for (; length > 0; length--) {
    result += (*ptrA) * (*ptrB);
    ptrA += strideA;
    ptrB += strideB;
  }
  return result;
}

// Helper routine: copies and scales an array (src and dest may be equal, or
// overlap)
inline void MatrixRmn::CopyArrayScale(long length, const double *from,
                                      long fromStride, double *to,
                                      long toStride, double scale) {
  for (; length > 0; length--) {
    *to = (*from) * scale;
    from += fromStride;
    to += toStride;
  }
}

// Helper routine: adds a scaled array
//	fromArray = toArray*scale.
inline void MatrixRmn::AddArrayScale(long length, const double *from,
                                     long fromStride, double *to, long toStride,
                                     double scale) {
  for (; length > 0; length--) {
    *to += (*from) * scale;
    from += fromStride;
    to += toStride;
  }
}


//-----------------------------------------------------------------------------
// MatrixRmn members that used to live in ikjacobian.cpp: forwarders into Rust.

#define IKJAC_M(m) (m).x, (int64_t)(m).NumRows, (int64_t)(m).NumCols

inline void MatrixRmn::SetDiagonalEntries(double d) {
  flareIkjacCheck(flare_ikjac_set_diagonal(IKJAC_M(*this), d));
}
inline void MatrixRmn::SetDiagonalEntries(const VectorRn &d) {
  flareIkjacCheck(
      flare_ikjac_set_diagonal_vec(IKJAC_M(*this), d.x, (int64_t)d.length));
}
inline void MatrixRmn::SetSuperDiagonalEntries(double d) {
  flareIkjacCheck(flare_ikjac_set_super_diagonal(IKJAC_M(*this), d));
}
inline void MatrixRmn::SetSuperDiagonalEntries(const VectorRn &d) {
  flareIkjacCheck(
      flare_ikjac_set_super_diagonal_vec(IKJAC_M(*this), d.x, (int64_t)d.length));
}
inline void MatrixRmn::SetSubDiagonalEntries(double d) {
  flareIkjacCheck(flare_ikjac_set_sub_diagonal(IKJAC_M(*this), d));
}
inline void MatrixRmn::SetSubDiagonalEntries(const VectorRn &d) {
  flareIkjacCheck(
      flare_ikjac_set_sub_diagonal_vec(IKJAC_M(*this), d.x, (int64_t)d.length));
}
inline void MatrixRmn::SetColumn(long i, const VectorRn &d) {
  flareIkjacCheck(
      flare_ikjac_set_column(IKJAC_M(*this), (int64_t)i, d.x, (int64_t)d.length));
}
inline void MatrixRmn::SetRow(long i, const VectorRn &d) {
  flareIkjacCheck(
      flare_ikjac_set_row(IKJAC_M(*this), (int64_t)i, d.x, (int64_t)d.length));
}
inline void MatrixRmn::SetSequence(const VectorRn &d, long startRow, long startCol,
                                   long deltaRow, long deltaCol) {
  flareIkjacCheck(flare_ikjac_set_sequence(IKJAC_M(*this), d.x, (int64_t)d.length,
                                           (int64_t)startRow, (int64_t)startCol,
                                           (int64_t)deltaRow, (int64_t)deltaCol));
}
inline void MatrixRmn::LoadAsSubmatrix(const MatrixRmn &A) {
  flareIkjacCheck(flare_ikjac_load_as_submatrix(IKJAC_M(*this), A.x, (int64_t)A.NumRows,
                                                (int64_t)A.NumCols));
}
inline void MatrixRmn::LoadAsSubmatrixTranspose(const MatrixRmn &A) {
  flareIkjacCheck(flare_ikjac_load_as_submatrix_transpose(
      IKJAC_M(*this), A.x, (int64_t)A.NumRows, (int64_t)A.NumCols));
}
inline double MatrixRmn::FrobeniusNorm() const {
  double r = 0.0;
  flareIkjacCheck(flare_ikjac_frobenius_norm(IKJAC_M(*this), &r));
  return r;
}
inline void MatrixRmn::Multiply(const VectorRn &v, VectorRn &result) const {
  assert(v.GetLength() == NumCols && result.GetLength() == NumRows);
  flareIkjacCheck(flare_ikjac_multiply_vec(IKJAC_M(*this), v.x, (int64_t)v.length,
                                           result.x, (int64_t)result.length));
}
inline void MatrixRmn::MultiplyTranspose(const VectorRn &v, VectorRn &result) const {
  assert(v.GetLength() == NumRows && result.GetLength() == NumCols);
  flareIkjacCheck(flare_ikjac_multiply_transpose_vec(
      IKJAC_M(*this), v.x, (int64_t)v.length, result.x, (int64_t)result.length));
}
inline double MatrixRmn::DotProductColumn(const VectorRn &v, long colNum) const {
  assert(v.GetLength() == NumRows);
  double r = 0.0;
  flareIkjacCheck(flare_ikjac_dot_product_column(IKJAC_M(*this), v.x, (int64_t)v.length,
                                                 (int64_t)colNum, &r));
  return r;
}
inline MatrixRmn &MatrixRmn::AddToDiagonal(double d) {
  flareIkjacCheck(flare_ikjac_add_to_diagonal(IKJAC_M(*this), d));
  return *this;
}
inline MatrixRmn &MatrixRmn::AddToDiagonal(const VectorRn &v) {
  flareIkjacCheck(
      flare_ikjac_add_to_diagonal_vec(IKJAC_M(*this), v.x, (int64_t)v.length));
  return *this;
}
inline MatrixRmn &MatrixRmn::MultiplyScalar(const MatrixRmn &A, double k,
                                            MatrixRmn &dst) {
  flareIkjacCheck(flare_ikjac_multiply_scalar(A.x, (int64_t)A.NumRows, (int64_t)A.NumCols,
                                              k, IKJAC_M(dst)));
  return dst;
}
inline MatrixRmn &MatrixRmn::Multiply(const MatrixRmn &A, const MatrixRmn &B,
                                      MatrixRmn &dst) {
  assert(A.NumCols == B.NumRows && A.NumRows == dst.NumRows && B.NumCols == dst.NumCols);
  flareIkjacCheck(flare_ikjac_multiply(A.x, (int64_t)A.NumRows, (int64_t)A.NumCols, B.x,
                                       (int64_t)B.NumRows, (int64_t)B.NumCols,
                                       IKJAC_M(dst)));
  return dst;
}
inline MatrixRmn &MatrixRmn::TransposeMultiply(const MatrixRmn &A, const MatrixRmn &B,
                                               MatrixRmn &dst) {
  assert(A.NumRows == B.NumRows && A.NumCols == dst.NumRows && B.NumCols == dst.NumCols);
  flareIkjacCheck(flare_ikjac_transpose_multiply(
      A.x, (int64_t)A.NumRows, (int64_t)A.NumCols, B.x, (int64_t)B.NumRows,
      (int64_t)B.NumCols, IKJAC_M(dst)));
  return dst;
}
inline MatrixRmn &MatrixRmn::MultiplyTranspose(const MatrixRmn &A, const MatrixRmn &B,
                                               MatrixRmn &dst) {
  assert(A.NumCols == B.NumCols && A.NumRows == dst.NumRows && B.NumRows == dst.NumCols);
  flareIkjacCheck(flare_ikjac_multiply_transpose(
      A.x, (int64_t)A.NumRows, (int64_t)A.NumCols, B.x, (int64_t)B.NumRows,
      (int64_t)B.NumCols, IKJAC_M(dst)));
  return dst;
}
inline void MatrixRmn::Solve(const VectorRn &b, VectorRn *xVec) const {
  assert(NumRows == NumCols && NumCols == xVec->GetLength() && NumRows == b.GetLength());
  flareIkjacCheck(flare_ikjac_solve(IKJAC_M(*this), b.x, (int64_t)b.length, xVec->x,
                                    (int64_t)xVec->length));
}
inline void MatrixRmn::ConvertToRefNoFree() {
  flareIkjacCheck(flare_ikjac_convert_to_ref_no_free(IKJAC_M(*this)));
}
inline void MatrixRmn::CalcGivensValues(double a, double b, double *c, double *s) {
  flareIkjacCheck(flare_ikjac_calc_givens_values(a, b, c, s));
}
inline void MatrixRmn::PostApplyGivens(double c, double s, long idx) {
  assert(0 <= idx && idx < NumCols);
  flareIkjacCheck(
      flare_ikjac_post_apply_givens(IKJAC_M(*this), c, s, (int64_t)idx, (int64_t)idx + 1));
}
inline void MatrixRmn::PostApplyGivens(double c, double s, long idx1, long idx2) {
  assert(idx1 != idx2 && 0 <= idx1 && idx1 < NumCols && 0 <= idx2 && idx2 < NumCols);
  flareIkjacCheck(flare_ikjac_post_apply_givens(IKJAC_M(*this), c, s, (int64_t)idx1,
                                                (int64_t)idx2));
}
inline void MatrixRmn::ComputeSVD(MatrixRmn &U, VectorRn &w, MatrixRmn &V) const {
  assert(U.NumRows == NumRows && V.NumCols == NumCols && U.NumRows == U.NumCols &&
         V.NumRows == V.NumCols && w.GetLength() == std::min(NumRows, NumCols));
  flareIkjacCheck(flare_ikjac_compute_svd(IKJAC_M(*this), IKJAC_M(U), w.x,
                                          (int64_t)w.length, V.x, (int64_t)V.NumRows,
                                          (int64_t)V.NumCols));
}
inline bool MatrixRmn::DebugCheckSVD(const MatrixRmn &U, const VectorRn &w,
                                     const MatrixRmn &V) const {
  int ok = 0;
  flareIkjacCheck(flare_ikjac_debug_check_svd(
      IKJAC_M(*this), U.x, (int64_t)U.NumRows, (int64_t)U.NumCols, w.x, (int64_t)w.length,
      V.x, (int64_t)V.NumRows, (int64_t)V.NumCols, &ok));
  assert(ok);
  return ok != 0;
}

#undef IKJAC_M

//=============================================================================

class Jacobian {
public:
  enum UpdateMode {
    JACOB_Undefined         = 0,
    JACOB_JacobianTranspose = 1,
    JACOB_PseudoInverse     = 2,
    JACOB_DLS               = 3,
    JACOB_SDLS              = 4
  };

  Jacobian(IKSkeleton *skeleton, std::vector<TPointD> &target);

  void computeJacobian();

  // const MatrixRmn& ActiveJacobian() const { return *Jactive; }
  // void SetJendActive() { Jactive = &Jend; }

  void addTarget(TPointD targetPos) { target.push_back(targetPos); }
  void deletLastTarget() { target.pop_back(); }
  void setTarget(int i, TPointD targetPos) { target[i] = targetPos; }

  void ZeroDeltaThetas();
  void CalcDeltaThetasTranspose();
  void CalcDeltaThetasPseudoinverse();
  void CalcDeltaThetasDLS();
  void CalcDeltaThetasDLSwithSVD();
  void CalcDeltaThetasSDLS();

  void UpdateThetas();
  bool checkJointsLimit();

  void UpdatedSClampValue();
  void DrawEigenVectors() const;

  void SetCurrentMode(UpdateMode mode) { CurrentUpdateMode = mode; }
  UpdateMode GetCurrentMode() const { return CurrentUpdateMode; }
  void SetDampingDLS(double lambda) {
    DampingLambda   = lambda;
    DampingLambdaSq = lambda * lambda;
  }

  void Reset();

private:
  IKSkeleton *skeleton;  // skeleton associated with this Jacobian matrix
  std::vector<TPointD> target;
  int nEffector;  // Number of end effectors
  int nJoint;     // Number of Joints
  int nRow;       // matrix rows J(= 2 * number of end effectors)
  int nCol;       // number of columns of J

  MatrixRmn
      Jend;  // Jacobian matrix based on the positions of the end effectors
  MatrixRmn Jtarget;
  MatrixRmn Jnorms;  // Norms of 2-vectors in active Jacobian (solo SDLS)

  MatrixRmn U;  // J = U * Diag(w) * V^T	(SVD Singular Value
                // Decomposition)
  VectorRn w;
  MatrixRmn V;

  UpdateMode CurrentUpdateMode;

  VectorRn dS;  // delta s
  VectorRn dT;  // delta t
  VectorRn dSclamp;
  VectorRn dTheta;  // delta theta

  VectorRn dPreTheta;  // (vale solo per SDLS)

  // The tuning constants (pseudo-inverse threshold factor, default damping 1.1, the
  // MaxAngle* per-step limits, BaseMaxTargetDist) are private to ikjacobian.rs now.

  // Parameters for the Damped Least Squares method
  double DampingLambda;
  double DampingLambdaSq;
  VectorRn DampingLambdaSqV;
  VectorRn diagMatIdentity;
  // double DampingLambdaSDLS;

  // MatrixRmn* Jactive;

  void CalcdTClampedFromdS();
};

//=============================================================================
// Jacobian members that used to live in ikjacobian.cpp. The skeleton walks stay
// here (they traverse IKNode pointers); the arithmetic is in ikjacobian.rs.

inline Jacobian::Jacobian(IKSkeleton *skeleton, std::vector<TPointD> &targetPos) {
  Jacobian::skeleton = skeleton;
  nEffector          = skeleton->getNumEffector();
  nJoint             = skeleton->getNodeCount() -
           nEffector;  // numero dei giunti meno gli effettori
  nRow = 2 * nEffector;
  nCol = nJoint;

  target = (targetPos);

  Jend.SetSize(nRow, nCol);  // Matrice jacobiana
  Jend.SetZero();

  Jtarget.SetSize(
      nRow,
      nCol);  // Matrice jacobiana basta sulle posizioni dei targets (non usata)
  Jtarget.SetZero();

  U.SetSize(nRow, nRow);  // matrice U per il calcolo SVD
  w.SetLength(std::min(nRow, nCol));
  V.SetSize(nCol, nCol);  // matrice V per il calcolo SVD

  dS.SetLength(nRow);      // (Posizione Target ) - (posizione End effector)
  dTheta.SetLength(nCol);  // Cambiamenti degli angoli dei Joints
  dPreTheta.SetLength(nCol);

  // Usato nel: metodo del trasposto dello Jacobiano  & DLS & SDLS
  dT.SetLength(nRow);

  // Usato nel Selectively Damped Least Squares Method
  dSclamp.SetLength(nEffector);

  Jnorms.SetSize(nEffector, nCol);  // Memorizza le norme della matrice attiva J

  DampingLambdaSqV.SetLength(nRow);
  diagMatIdentity.SetLength(nCol);

  Reset();
}

inline void Jacobian::Reset() {
  // DampingLambda = 1.1, DampingLambdaSq = its square, DampingLambdaSqV and
  // diagMatIdentity filled, dSclamp = HUGE_VAL.
  flareIkjacCheck(flare_ikjac_reset(
      &DampingLambda, &DampingLambdaSq, DampingLambdaSqV.GetPtr(),
      (int64_t)DampingLambdaSqV.GetLength(), diagMatIdentity.GetPtr(),
      (int64_t)diagMatIdentity.GetLength(), dSclamp.GetPtr(), (int64_t)dSclamp.GetLength()));
}

// Calcola il vettore deltaS vector, dS, (l' errore tra end effector e target
// Calcola le matrce jacobiana J
inline void Jacobian::computeJacobian() {
  // Scorro tutto lo skeleton per trovare tutti gli end effectors, e raccolgo per
  // ognuno la catena degli antenati (dal piu' vicino): numero del giunto, flag
  // "frozen" e posizione globale. Il calcolo e' in Rust.
  std::vector<int> effNum;
  std::vector<double> effS;
  std::vector<size_t> effNanc;
  std::vector<int> ancJoint;
  std::vector<unsigned char> ancFrozen;
  std::vector<double> ancS;

  int numNode = skeleton->getNodeCount();
  for (int index = 0; index < numNode; index++) {
    IKNode *n = skeleton->getNode(index);
    if (!n->IsEffector()) continue;
    effNum.push_back(n->getEffectorNum());
    effS.push_back(n->GetS().x);
    effS.push_back(n->GetS().y);
    size_t count = 0;
    for (IKNode *m = skeleton->getParent(n); m; m = skeleton->getParent(m)) {
      ancJoint.push_back(m->getJointNum());
      ancFrozen.push_back(m->isFrozen() ? 1 : 0);
      ancS.push_back(m->GetS().x);
      ancS.push_back(m->GetS().y);
      count++;
    }
    effNanc.push_back(count);
  }
  std::vector<double> tgt;
  for (size_t k = 0; k < target.size(); k++) {
    tgt.push_back(target[k].x);
    tgt.push_back(target[k].y);
  }
  flareIkjacCheck(flare_ikjac_compute_jacobian(
      Jend.GetPtr(), (int64_t)Jend.getNumRows(), (int64_t)Jend.getNumColumns(),
      dS.GetPtr(), (int64_t)dS.GetLength(), (int64_t)skeleton->getNumEffector(),
      effNum.size(), effNum.data(), effS.data(), effNanc.data(), ancJoint.size(),
      ancJoint.data(), ancFrozen.data(), ancS.data(), tgt.data(), target.size()));
}

// The delta theta values have been computed in dTheta array
// Apply the delta theta values to the joints
// Nothing is done about joint limits for now.
inline void Jacobian::UpdateThetas() {
  // Update the joint angles
  for (int index = 0; index < skeleton->getNodeCount(); index++) {
    IKNode *n = skeleton->getNode(index);
    if (n->IsJoint()) {
      int i = n->getJointNum();
      n->AddToTheta(dTheta[i]);
    }
  }
  // Aggiorno le posizioni dei joint
  skeleton->compute();
}

inline bool Jacobian::checkJointsLimit() {
  // The joint-limit code was commented out in the original; no limit is ever hit.
  bool clampingDetected = false;
  return clampingDetected;
}

inline void Jacobian::ZeroDeltaThetas() { dTheta.SetZero(); }

#define IKJAC_J(m) (m).GetPtr(), (int64_t)(m).getNumRows(), (int64_t)(m).getNumColumns()
#define IKJAC_VEC(v) (v).GetPtr(), (int64_t)(v).GetLength()

// Find the delta theta values using inverse Jacobian method
// Uses a greedy method to decide scaling factor
inline void Jacobian::CalcDeltaThetasTranspose() {
  flareIkjacCheck(flare_ikjac_calc_transpose(IKJAC_J(Jend), IKJAC_VEC(dS), IKJAC_VEC(dT),
                                             IKJAC_VEC(dTheta)));
}

inline void Jacobian::CalcDeltaThetasPseudoinverse() {
  flareIkjacCheck(flare_ikjac_calc_pseudoinverse(
      IKJAC_J(Jend), (int64_t)skeleton->getNumEffector(), IKJAC_VEC(dS), IKJAC_VEC(dTheta),
      IKJAC_VEC(diagMatIdentity)));
}

inline void Jacobian::CalcDeltaThetasDLS() {
  flareIkjacCheck(flare_ikjac_calc_dls(IKJAC_J(Jend), IKJAC_J(U), IKJAC_VEC(DampingLambdaSqV),
                                       IKJAC_VEC(dS), IKJAC_VEC(dT), IKJAC_VEC(dTheta)));
}

inline void Jacobian::CalcDeltaThetasDLSwithSVD() {
  flareIkjacCheck(flare_ikjac_calc_dls_with_svd(
      IKJAC_J(Jend), IKJAC_J(U), IKJAC_VEC(w), IKJAC_J(V), DampingLambdaSq, IKJAC_VEC(dS),
      IKJAC_VEC(dTheta)));
}

inline void Jacobian::CalcDeltaThetasSDLS() {
  flareIkjacCheck(flare_ikjac_calc_sdls(
      IKJAC_J(Jend), IKJAC_J(U), IKJAC_VEC(w), IKJAC_J(V),
      (int64_t)skeleton->getNumEffector(), IKJAC_J(Jnorms), IKJAC_VEC(dS), IKJAC_VEC(dT),
      IKJAC_VEC(dSclamp), IKJAC_VEC(dPreTheta), IKJAC_VEC(dTheta)));
}

inline void Jacobian::CalcdTClampedFromdS() {
  flareIkjacCheck(flare_ikjac_calc_dt_clamped(IKJAC_VEC(dS), IKJAC_VEC(dT),
                                              IKJAC_VEC(dSclamp)));
}

inline void Jacobian::UpdatedSClampValue() {
  // Traverse skeleton to find all end effectors
  int numNode = skeleton->getNodeCount();
  for (int k = 0; k < numNode; k++) {
    IKNode *n = skeleton->getNode(k);
    if (n->IsEffector()) {
      int i                    = n->getEffectorNum();
      const TPointD &targetPos = target[i];
      flareIkjacCheck(flare_ikjac_updated_sclamp_one(
          i, n->GetS().x, n->GetS().y, targetPos.x, targetPos.y, IKJAC_VEC(dS),
          IKJAC_VEC(dSclamp)));
    }
  }
}

#undef IKJAC_J
#undef IKJAC_VEC

#endif  // JACOBIAN_H
