#pragma once

/*-----------------------------------------------------
 * Header della libreria metnum
 * Alcuni algoritmi di Metodi Numerici
 * Autore: P. Foggia
 *
 * The solvers (Approx, Cholesky, CholForw, CholBack) are implemented in
 * metnum.rs (flare_formats); the matrix allocators stay inline here.
 *--------------------------------------------------*/

#ifndef METNUM_H
#define METNUM_H

extern "C" {
int flare_metnum_approx(int n, int m, double **x, const double *y, double *c);
int flare_metnum_cholesky(int n, double **A);
int flare_metnum_chol_forw(int n, double **A, const double *b, double *x);
int flare_metnum_chol_back(int n, double **A, const double *b, double *x);
}

namespace MetNum {

// Row-pointer matrix of n rows by m columns; NULL if n < 1 or m < 1.
inline double **AllocMatrix(int n, int m) {
  if ((n < 1) || (m < 1)) return nullptr;
  double **A = new double *[n];
  for (int i = 0; i < n; i++) A[i] = new double[m];
  return A;
}

inline void FreeMatrix(int n, double **A) {
  for (int i = 0; i < n; i++) delete[] A[i];
  delete[] A;
}

// Lower-triangular matrix: row i has i + 1 entries; NULL if n < 1.
inline double **AllocTriangMatrix(int n) {
  if (n < 1) return nullptr;
  double **A = new double *[n];
  for (int i = 0; i < n; i++) A[i] = new double[i + 1];
  return A;
}

inline int Approx(int n, int m, double **x, double y[], double c[]) {
  return flare_metnum_approx(n, m, x, y, c);
}
inline int Cholesky(int n, double **A) { return flare_metnum_cholesky(n, A); }
inline int CholForw(int n, double **A, double b[], double x[]) {
  return flare_metnum_chol_forw(n, A, b, x);
}
inline int CholBack(int n, double **A, double b[], double x[]) {
  return flare_metnum_chol_back(n, A, b, x);
}
}  // namespace MetNum

#endif
