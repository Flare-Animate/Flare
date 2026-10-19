#pragma once

// PatternMapParam.h: interface for the CPatternMapParam class.
//
//////////////////////////////////////////////////////////////////////

#if !defined(                                                                  \
    AFX_PATTERNMAPPARAM_H__53513371_34D4_11D6_B9E7_0040F674BE6A__INCLUDED_)
#define AFX_PATTERNMAPPARAM_H__53513371_34D4_11D6_B9E7_0040F674BE6A__INCLUDED_

#ifdef _MSC_VER
#pragma warning(disable : 4996)
#endif
#include <string.h>
#include "CIL.h"

// Implemented in PatternMapParam.rs (flare_formats): parses arguments 1..11 of the
// 12-argument list with the C runtime's atof/atoi. Same layout as the Rust PmpFields.
extern "C" {
struct flare_pmp_fields {
  double maxScale, minScale;
  double maxDirAngle, minDirAngle;
  double maxDist, minDist;
  double density;
  int isRandomDir, isKeepContour, isUseInkColor, isIncludeAlpha;
};
int flare_patternmapparam_parse(int argc, const char *const *argv, int shrink,
                                flare_pmp_fields *out);
}

class CPatternMapParam {
public:
  char m_patternFn[1024];
  CCIL m_ink;
  bool m_isKeepContour;
  bool m_isRandomDir;
  double m_minDirAngle, m_maxDirAngle;
  double m_minScale, m_maxScale;
  bool m_isUseInkColor, m_isIncludeAlpha;
  double m_density, m_minDist, m_maxDist;

  CPatternMapParam() { null(); }
  CPatternMapParam(const int argc, const char *argv[], const int shrink) {
    null();
    if (argc == 12) {
      m_ink.set(argv[0], 4095);
      flare_pmp_fields f;
      if (flare_patternmapparam_parse(argc, argv, shrink, &f) == 0) {
        m_maxScale       = f.maxScale;
        m_minScale       = f.minScale;
        m_maxDirAngle    = f.maxDirAngle;
        m_minDirAngle    = f.minDirAngle;
        m_isRandomDir    = f.isRandomDir != 0;
        m_maxDist        = f.maxDist;
        m_minDist        = f.minDist;
        m_density        = f.density;
        m_isKeepContour  = f.isKeepContour != 0;
        m_isUseInkColor  = f.isUseInkColor != 0;
        m_isIncludeAlpha = f.isIncludeAlpha != 0;
      }
    }
  }
  virtual ~CPatternMapParam() {}

  void null() {
    m_patternFn[0]   = '\0';
    m_ink.m_nb       = 0;
    m_isKeepContour  = false;
    m_isRandomDir    = false;
    m_minDirAngle    = 0.0;
    m_maxDirAngle    = 0.0;
    m_minScale       = 0.2;
    m_maxScale       = 0.2;
    m_isUseInkColor  = true;
    m_isIncludeAlpha = true;
    m_density        = 0.2;
    m_minDist        = 3.0;
    m_maxDist        = 3.0;
  }

  void testValue() {
    strcpy(m_patternFn, "d:\\toonz_fx\\test\\artcontour\\e.ttt");

    m_ink.m_nb    = 1;
    m_ink.m_ci[0] = 0;

    m_isKeepContour = false;

    m_isRandomDir = false;
    m_minDirAngle = 0.0;
    m_maxDirAngle = 0.0;

    m_minScale = 0.3;
    m_maxScale = 0.4;

    m_isUseInkColor  = true;
    m_isIncludeAlpha = true;
    m_density        = 0.0;
    m_minDist        = 3.0;
    m_maxDist        = 3.0;
  }

  bool isOK() { return true; }
};

#endif  // !defined(AFX_PATTERNMAPPARAM_H__53513371_34D4_11D6_B9E7_0040F674BE6A__INCLUDED_)
