#pragma once

// CIL.h: interface for the CCIL class.
//
//////////////////////////////////////////////////////////////////////

#if !defined(AFX_CIL_H__2B094D96_25D9_11D6_B9C6_0040F674BE6A__INCLUDED_)
#define AFX_CIL_H__2B094D96_25D9_11D6_B9C6_0040F674BE6A__INCLUDED_

#define MAXNBCI 4096  // 512

// Implemented in CIL.rs (flare_formats): parses "0,3-5,12" / "all" / "-1" into a
// sorted, de-duplicated list; returns the count written to out (<= cap).
extern "C" int flare_cil_set(const char *s, int maxIndex, int *out, int cap);

class CCIL {
public:
  int m_nb;
  int m_ci[MAXNBCI];

  CCIL() : m_nb(0){};
  virtual ~CCIL() { m_nb = 0; };
  void set(const char *s, const int maxIndex) {
    m_nb = s ? flare_cil_set(s, maxIndex, m_ci, MAXNBCI) : 0;
  }
  bool isIn(const int ci) {
    for (int i = 0; i < m_nb && ci >= m_ci[i]; i++)
      if (ci == m_ci[i]) return true;
    return false;
  }
  void print() {}
};

#endif  // !defined(AFX_CIL_H__2B094D96_25D9_11D6_B9C6_0040F674BE6A__INCLUDED_)
