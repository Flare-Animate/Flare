#pragma once

// PatternPosition.h: interface for the CPatternPosition class.
//
//////////////////////////////////////////////////////////////////////

#if !defined(                                                                  \
    AFX_PATTERNPOSITION_H__9FB82504_34F2_11D6_B9E8_0040F674BE6A__INCLUDED_)
#define AFX_PATTERNPOSITION_H__9FB82504_34F2_11D6_B9E8_0040F674BE6A__INCLUDED_

#include <vector>

#include <stdlib.h>
#include "STColSelPic.h"
#include "SError.h"
#include "SDef.h"

// Implemented in PatternPosition.rs (flare_formats). Both return the number of
// positions and store 2*n ints (x, y pairs) in *out, to be released with
// flare_pattern_position_free; negative = failure (the old SMemAllocError).
// The generator is passed in (::rand here) so the Rust side is testable.
extern "C" {
int flare_pattern_position_random(int nbPat, int nbPixel, int lX, int lY,
                                  const unsigned char *sel, int (*randfn)(void),
                                  int randMax, int **out);
int flare_pattern_position_dd(int lX, int lY, const unsigned char *sel,
                              double minD, double maxD, int (*randfn)(void),
                              int **out);
void flare_pattern_position_free(int *p, int n);
}

class CPatternPosition {
  static int rand_source() { return rand(); }

  void takePositions(int n, int *xy) {
    if (n < 0) throw SMemAllocError("in Pattern Position Generation");
    try {
      for (int i = 0; i < n; i++) {
        SPOINT xyp = {xy[2 * i], xy[2 * i + 1]};
        m_pos.push_back(xyp);
      }
    } catch (...) {
      flare_pattern_position_free(xy, n);
      throw;
    }
    flare_pattern_position_free(xy, n);
  }

  bool isInSet(const int nbSet, const int *set, const int val) {
    for (int i = 0; i < nbSet; i++)
      if (set[i] == val) return true;
    return false;
  }
  void makeRandomPositions(const int nbPat, const int nbPixel, const int lX,
                           const int lY, const UCHAR *sel) {
    int *xy = nullptr;
    int n   = flare_pattern_position_random(nbPat, nbPixel, lX, lY, sel,
                                          &rand_source, RAND_MAX, &xy);
    takePositions(n, xy);
  }
  void makeDDPositions(const int lX, const int lY, UCHAR *sel,
                       const double minD, const double maxD) {
    int *xy = nullptr;
    int n   = flare_pattern_position_dd(lX, lY, sel, minD, maxD, &rand_source,
                                      &xy);
    takePositions(n, xy);
  }

public:
  std::vector<SPOINT> m_pos;

  CPatternPosition() : m_pos(0){};
  virtual ~CPatternPosition() { m_pos.clear(); }

  template <class P>
  CPatternPosition(CSTColSelPic<P> &p, const int nbPixel, const double dens,
                   const double minD, const double maxD) {
    try {
      m_pos.clear();
      if (dens > 0.00001) {
        // Random position
        int nbPat = I_ROUND(((double)nbPixel * dens));
        //			nbPat= nbPat>(int)(0.9*nbPixel) ?
        //(int)(0.9*nbPixel) : nbPat;
        nbPat = nbPat > nbPixel ? nbPixel : nbPat;
        if (nbPat > 0)
          makeRandomPositions(nbPat, nbPixel, p.m_lX, p.m_lY, p.m_sel.get());
      } else {
        // Distance-driven position
        makeDDPositions(p.m_lX, p.m_lY, p.m_sel.get(), minD, maxD);
      }
    } catch (SMemAllocError) {
      throw;
    }
  }

  template <class P>
  void drawTest(CSTColSelPic<P> &pic) {
    for (std::vector<SPOINT>::iterator pv = m_pos.begin(); pv != m_pos.end();
         pv++) {
      int xx = pv->x;
      int yy = pv->y;
      for (int y = yy - 1; y <= (yy + 1); y++)
        for (int x = xx - 1; x <= (xx + 1); x++)
          if (x >= 0 && y >= 0 && x < pic.m_lX && y < pic.m_lY) {
            UC_PIXEL *pp = pic.m_pic + y * pic.m_lX + x;
            pp->r        = 0;
            pp->g        = 0;
            pp->b        = 255;
            pp->m        = 255;
          }
    }
  }
};

#endif  // !defined(AFX_PATTERNPOSITION_H__9FB82504_34F2_11D6_B9E8_0040F674BE6A__INCLUDED_)
