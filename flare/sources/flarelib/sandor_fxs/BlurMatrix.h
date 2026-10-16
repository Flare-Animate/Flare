#pragma once

// BlurMatrix.h: interface for the CBlurMatrix class.
//
//////////////////////////////////////////////////////////////////////

#if !defined(AFX_BLURMATRIX_H__8298C171_0035_11D6_B94F_0040F674BE6A__INCLUDED_)
#define AFX_BLURMATRIX_H__8298C171_0035_11D6_B94F_0040F674BE6A__INCLUDED_

#include <algorithm>
#include <cmath>
#include <cstdlib>
#include <vector>

#include "SDef.h"  // SXYD, I_ROUND

// The matrix itself is built in Rust (flarelib/sandor_fxs/BlurMatrix.rs, in the
// flare_formats crate). These are the flat interfaces it exposes; the class below keeps
// its original shape, including the public m_m, and marshals through them.
extern "C" {

// A flattened sample. Same layout as SXYD.
struct flare_bm_point {
  int x;
  int y;
  double d;
};

// One section: which random-sample slot (0..NBRS-1) it belongs to, how many points it
// holds, and where they start in the point array.
struct flare_bm_section {
  int rs;
  int npts;
  int first;
};

// Points a build call can emit. Each built section holds exactly one point, so the
// section count and the point count agree before addPath() runs.
int flare_blurmatrix_max_points_random(double d, int nb);
int flare_blurmatrix_max_points_equal(double d, int nb);

// Build the sampling pattern. Returns the number of points written (-1 if the buffers
// are too small); secs[i] describes pts[secs[i].first ..][npts].
// createRandom draws `randfn()` twice per attempt, which is how the original consumed
// rand(); pass ::rand to keep the old sequence.
int flare_blurmatrix_build_random(double d, int nb, int (*randfn)(void),
                                  struct flare_bm_section* secs, int sec_cap,
                                  struct flare_bm_point* pts, int pt_cap);
int flare_blurmatrix_build_equal(double d, int nb,
                                 struct flare_bm_section* secs, int sec_cap,
                                 struct flare_bm_point* pts, int pt_cap);

// Appends one section's return-to-origin path. Writes only the appended points and
// returns how many; with a null/short buffer it returns the count without writing.
int flare_blurmatrix_add_path(const struct flare_bm_point* sec, int n,
                              struct flare_bm_point* out, int cap);

// Is (x, y) the first point of any of the n sections?
int flare_blurmatrix_is_in(const struct flare_bm_point* firsts, int n, int x, int y);
}

#define NBRS 10  //  Number of Random Samples
typedef std::vector<SXYD> BLURSECTION;

class CBlurMatrix {
public:
  bool m_isSAC;  // Stop At Contour
  bool m_isRS;   // Random Sampling
  std::vector<BLURSECTION> m_m[NBRS];

  CBlurMatrix() : m_isSAC(false), m_isRS(false){};
  virtual ~CBlurMatrix() {}

  CBlurMatrix(const CBlurMatrix& m) : m_isSAC(m.m_isSAC), m_isRS(m.m_isRS) {
    for (int i = 0; i < NBRS; i++) m_m[i] = m.m_m[i];
  }

  CBlurMatrix(const double d, const int nb, const bool isSAC, const bool isRS)
    : m_isSAC(isSAC), m_isRS(isRS) {
    if (isRS)
      createRandom(d, nb);
    else
      createEqual(d, nb);
    if (isSAC) addPath();
  }

  // Drop every sample. The original did this in the constructor only; createRandom and
  // createEqual used to append to whatever was already there, which could interleave two
  // matrices. They now replace the contents, so this is called from both.
  void clear() {
    for (int i = 0; i < NBRS; i++) m_m[i].clear();
  }

  void createRandom(const double d, const int nb) {
    clear();
    build(true, d, nb);
  }

  void createEqual(const double d, const int nb) {
    clear();
    build(false, d, nb);
  }

  // Rebuild m_m from the flattened sections.
  void build(const bool random, const double d, const int nb) {
    const int cap = random ? flare_blurmatrix_max_points_random(d, nb)
                           : flare_blurmatrix_max_points_equal(d, nb);
    if (cap <= 0) return;
    std::vector<flare_bm_section> secs(cap);
    std::vector<flare_bm_point> pts(cap);
    const int n = random ? flare_blurmatrix_build_random(d, nb, &rand_source, secs.data(),
                                                         cap, pts.data(), cap)
                         : flare_blurmatrix_build_equal(d, nb, secs.data(), cap, pts.data(),
                                                        cap);
    if (n < 0) return;
    for (int k = 0; k < n; k++) {
      const flare_bm_section& s = secs[k];
      if (s.rs < 0 || s.rs >= NBRS || s.npts < 0) return;
      BLURSECTION b;
      for (int j = 0; j < s.npts; j++) {
        const flare_bm_point& p = pts[s.first + j];
        b.push_back(SXYD{p.x, p.y, p.d});
      }
      m_m[s.rs].push_back(b);
    }
  }

  void addPath(std::vector<BLURSECTION>::iterator pBS) {
    if (pBS->size() == 0) return;
    const SXYD& first = pBS->front();
    // Upper bound on what the path can append: one step per unit of the longer axis.
    const int l = std::max(std::abs(first.x), std::abs(first.y));
    std::vector<flare_bm_point> buf(l);
    const int n = flare_blurmatrix_add_path(
      reinterpret_cast<const flare_bm_point*>(&first), (int)pBS->size(), buf.data(), l);
    if (n <= 0) return;
    for (int i = 0; i < n && i < l; i++)
      pBS->push_back(SXYD{buf[i].x, buf[i].y, buf[i].d});
  }

  void addPath() {
    for (int i = 0; i < (m_isRS ? NBRS : 1); i++)
      for (std::vector<BLURSECTION>::iterator pBS = m_m[i].begin(); pBS != m_m[i].end();
           ++pBS)
        addPath(pBS);
  }

  void print() const {}

  bool isIn(const std::vector<BLURSECTION>& m, const SXYD& xyd) const {
    if (m.empty()) return false;
    std::vector<flare_bm_point> firsts(m.size());
    for (size_t i = 0; i < m.size(); i++)
      firsts[i] = flare_bm_point{m[i].front().x, m[i].front().y, 0.0};
    return flare_blurmatrix_is_in(firsts.data(), (int)firsts.size(), xyd.x, xyd.y) != 0;
  }

private:
  static int rand_source() { return std::rand(); }
};

#endif  // !defined(AFX_BLURMATRIX_H__8298C171_0035_11D6_B94F_0040F674BE6A__INCLUDED_)