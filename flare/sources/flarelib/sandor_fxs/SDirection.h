#pragma once

// SDirection.h: interface for the CSDirection class.
//
//////////////////////////////////////////////////////////////////////

#if !defined(AFX_SDIRECTION_H__C672AFF1_1A65_11D6_B99E_0040F674BE6A__INCLUDED_)
#define AFX_SDIRECTION_H__C672AFF1_1A65_11D6_B99E_0040F674BE6A__INCLUDED_

#include "SDef.h"

#include <memory>
#include <string.h>
#include "SError.h"
#include <algorithm>
#include <array>
#include <vector>

//#define MSIZE 3

typedef struct {
  char val;
  UCHAR dir;
} SVD;

#define NBDIR 4

// Implemented in SDirection.rs (flare_formats): in-place passes over the lX*lY byte map.
// Return 0 on success, -1 for bad arguments, -2 when the request is too big (the old
// SMemAllocError).
extern "C" {
int flare_sdirection_prepare(int lX, int lY, unsigned char *dir, int sens,
                             int border);
int flare_sdirection_do_dir(int lX, int lY, unsigned char *dir, int sens);
int flare_sdirection_do_radius(int lX, int lY, unsigned char *dir, double rH,
                               double rLR, double rV, double rRL, int dBlur);
}

class CSDirection {
  int m_lX, m_lY, m_sens;
  std::unique_ptr<UCHAR[]> m_dir;

  void null() {
    m_dir.reset();
    m_lX = m_lY = 0;
  }

  void init(const int lX, const int lY, const UCHAR *sel, const int sens,
            const int border) {
    if (m_lX > 0 && m_lY > 0) {
      const size_t n = (size_t)m_lX * (size_t)m_lY;
      m_dir.reset(new UCHAR[n]);
      memcpy(m_dir.get(), sel, sizeof(UCHAR) * n);
      if (flare_sdirection_prepare(m_lX, m_lY, m_dir.get(), sens, border) !=
          0) {
        null();
        throw SMemAllocError("in directionMap");
      }
    }
  }

public:
  CSDirection() : m_lX(0), m_lY(0), m_sens(0) {}
  CSDirection(const int lX, const int lY, const UCHAR *sel, const int sens)
      : m_lX(lX), m_lY(lY), m_sens(sens) {
    init(lX, lY, sel, sens, 0);
  }
  CSDirection(const int lX, const int lY, const UCHAR *sel, const int sens,
              const int border)
      : m_lX(lX), m_lY(lY), m_sens(sens) {
    init(lX, lY, sel, sens, border);
  }
  virtual ~CSDirection() { null(); }

  void doDir() {
    if (m_lX > 0 && m_lY > 0 && m_dir &&
        flare_sdirection_do_dir(m_lX, m_lY, m_dir.get(), m_sens) != 0)
      throw SMemAllocError("in directionMap");
  }
  void doRadius(const double rH, const double rLR, const double rV,
                const double rRL, const int dBlur) {
    if (m_lX > 0 && m_lY > 0 && m_dir &&
        flare_sdirection_do_radius(m_lX, m_lY, m_dir.get(), rH, rLR, rV, rRL,
                                   dBlur) != 0)
      throw SMemAllocError("in directionMap");
  }
  void getResult(UCHAR *sel) {
    if (m_dir)
      memcpy(sel, m_dir.get(), (size_t)m_lX * (size_t)m_lY * sizeof(UCHAR));
  }
};

#endif  // !defined(AFX_SDIRECTION_H__C672AFF1_1A65_11D6_B99E_0040F674BE6A__INCLUDED_)
