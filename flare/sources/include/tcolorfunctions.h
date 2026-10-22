#pragma once

#ifndef T_COLOR_FUNCTIONS_INCLUDED
#define T_COLOR_FUNCTIONS_INCLUDED

#include <cassert>
#include <cstring>
#include <stdexcept>

#include "tpixel.h"

// tcolorfunctions.cpp moved to tcolorfunctions.rs (flare_formats): the per-pixel arithmetic
// is called through these scalar entry points (results in r, g, b, m order). Non-zero status
// means a NULL pointer / internal error.
extern "C" {
int flare_tcolorfader_apply(unsigned r, unsigned g, unsigned b, unsigned m, unsigned fr, unsigned fg, unsigned fb,
                            unsigned fm, double fade, unsigned char *out);
int flare_tcolorfader_params(unsigned fr, unsigned fg, unsigned fb, unsigned fm, double fade, double *out);
int flare_tgeneric_apply(unsigned r, unsigned g, unsigned b, unsigned m, const double *mult, const double *add,
                         unsigned char *out);
int flare_ttranspfader_apply(unsigned r, unsigned g, unsigned b, unsigned m, double transp, unsigned char *out);
int flare_tonionfader_apply(unsigned r, unsigned g, unsigned b, unsigned m, unsigned fr, unsigned fg, unsigned fb,
                            unsigned fm, double fade, unsigned char *out);
int flare_tonionfader_params(unsigned fr, unsigned fg, unsigned fb, unsigned fm, double fade, double *out);
int flare_tcolumnfilter_apply(unsigned r, unsigned g, unsigned b, unsigned m, unsigned sr, unsigned sg, unsigned sb,
                              unsigned sm, unsigned char *out);
}

inline void flare_tcolorfunctions_check(int status) {
  if (status) throw std::invalid_argument("tcolorfunctions: invalid argument");
}

#undef DVAPI
#undef DVVAR
#ifdef TCOLOR_EXPORTS
#define DVAPI DV_EXPORT_API
#define DVVAR DV_EXPORT_VAR
#else
#define DVAPI DV_IMPORT_API
#define DVVAR DV_IMPORT_VAR
#endif

//-----------------------------------------------------------------------------

class DVAPI TColorFunction {
public:
  virtual TPixel32 operator()(
      const TPixel32 &color) const = 0;  // {return color;};

  struct Parameters {  // outX = tcrop(inX * m_mX + m_cX, 0, 1); 0<=inX<=1
    double m_mR, m_mG, m_mB, m_mM;
    double m_cR, m_cG, m_cB, m_cM;
    Parameters()
        : m_mR(1)
        , m_mG(1)
        , m_mB(1)
        , m_mM(1)
        , m_cR(0)
        , m_cG(0)
        , m_cB(0)
        , m_cM(0) {}
  };

  virtual TColorFunction *clone() const = 0;
  virtual bool getParameters(
      Parameters &p) const = 0;  //{ p = Parameters(); return true; }

  virtual ~TColorFunction() {}

protected:
  static void setParams(Parameters &p, const double v[8]) {
    p.m_mR = v[0], p.m_mG = v[1], p.m_mB = v[2], p.m_mM = v[3];
    p.m_cR = v[4], p.m_cG = v[5], p.m_cB = v[6], p.m_cM = v[7];
  }
};

//-----------------------------------------------------------------------------

class DVAPI TGenericColorFunction final : public TColorFunction {
  double m_m[4], m_c[4];

public:
  TGenericColorFunction(const double m[4], const double c[4]) {
    memcpy(m_m, m, 4 * sizeof(double));
    memcpy(m_c, c, 4 * sizeof(double));
  }

  TColorFunction *clone() const override {
    return new TGenericColorFunction(m_m, m_c);
  }

  TPixel32 operator()(const TPixel32 &color) const override {
    unsigned char o[4];
    flare_tcolorfunctions_check(
        flare_tgeneric_apply(color.r, color.g, color.b, color.m, m_m, m_c, o));
    return TPixel32(o[0], o[1], o[2], o[3]);
  }
  bool getParameters(Parameters &p) const override {
    p.m_mR = m_m[0], p.m_mG = m_m[1], p.m_mB = m_m[2], p.m_mM = m_m[3];
    p.m_cR = m_c[0], p.m_cG = m_c[1], p.m_cB = m_c[2], p.m_cM = m_c[3];
    return true;
  }
};

//-----------------------------------------------------------------------------

class DVAPI TColorFader final : public TColorFunction {
  TPixel32 m_color;
  double m_fade;

public:
  TColorFader() : m_color(), m_fade(0.5) {}
  TColorFader(const TPixel32 &color, double fade)
      : m_color(color), m_fade(fade) {}

  TColorFunction *clone() const override {
    return new TColorFader(m_color, m_fade);
  }

  TPixel32 operator()(const TPixel32 &color) const override {
    unsigned char o[4];
    flare_tcolorfunctions_check(flare_tcolorfader_apply(
        color.r, color.g, color.b, color.m, m_color.r, m_color.g, m_color.b,
        m_color.m, m_fade, o));
    return TPixel32(o[0], o[1], o[2], o[3]);
  }
  bool getParameters(Parameters &p) const override {
    double v[8];
    flare_tcolorfunctions_check(flare_tcolorfader_params(
        m_color.r, m_color.g, m_color.b, m_color.m, m_fade, v));
    setParams(p, v);
    return true;
  }
};

//-----------------------------------------------------------------------------

class DVAPI TOnionFader final : public TColorFunction {
  TPixel32 m_color;
  double m_fade;

public:
  TOnionFader() : m_color(), m_fade(0.5) {}
  TOnionFader(const TPixel32 &color, double fade)
      : m_color(color), m_fade(fade) {}

  TColorFunction *clone() const override {
    return new TOnionFader(m_color, m_fade);
  }

  TPixel32 operator()(const TPixel32 &color) const override {
    unsigned char o[4];
    flare_tcolorfunctions_check(flare_tonionfader_apply(
        color.r, color.g, color.b, color.m, m_color.r, m_color.g, m_color.b,
        m_color.m, m_fade, o));
    return TPixel32(o[0], o[1], o[2], o[3]);
  }
  bool getParameters(Parameters &p) const override {
    double v[8];
    flare_tcolorfunctions_check(flare_tonionfader_params(
        m_color.r, m_color.g, m_color.b, m_color.m, m_fade, v));
    setParams(p, v);
    return true;
  }
};

class DVAPI TTranspFader final : public TColorFunction {
  double m_transp;

public:
  TTranspFader() : m_transp(0.5) {}
  TTranspFader(double transp) : m_transp(transp) {}

  TColorFunction *clone() const override { return new TTranspFader(m_transp); }

  TPixel32 operator()(const TPixel32 &color) const override {
    unsigned char o[4];
    flare_tcolorfunctions_check(flare_ttranspfader_apply(
        color.r, color.g, color.b, color.m, m_transp, o));
    return TPixel32(o[0], o[1], o[2], o[3]);
  }
  bool getParameters(Parameters &p) const override {
    assert(false);
    return true;
  }
};

//-----------------------------------------------------------------------------

class DVAPI TColumnColorFilterFunction final : public TColorFunction {
  TPixel32 m_colorScale;

public:
  TColumnColorFilterFunction() : m_colorScale() {}
  TColumnColorFilterFunction(const TPixel32 &color) : m_colorScale(color) {}

  TColorFunction *clone() const override {
    return new TColumnColorFilterFunction(m_colorScale);
  }

  TPixel32 operator()(const TPixel32 &color) const override {
    unsigned char o[4];
    flare_tcolorfunctions_check(flare_tcolumnfilter_apply(
        color.r, color.g, color.b, color.m, m_colorScale.r, m_colorScale.g,
        m_colorScale.b, m_colorScale.m, o));
    return TPixel32(o[0], o[1], o[2], o[3]);
  }
  bool getParameters(Parameters &p) const override {
    assert(false);
    return true;
  }
};

#endif
