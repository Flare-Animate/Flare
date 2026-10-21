#pragma once

#include "trastercm.h"
#include "texception.h"
#include "tpixelutils.h"

#undef DVAPI
#undef DVVAR
#ifdef TROP_EXPORTS
#define DVAPI DV_EXPORT_API
#define DVVAR DV_EXPORT_VAR
#else
#define DVAPI DV_IMPORT_API
#define DVVAR DV_IMPORT_VAR
#endif

/* FreeBSD defines atop() macro in standard includes. */
#undef atop

//===============================================================

//  Forward declarations

class TPalette;
typedef TSmartPointerT<TPalette> TPaletteP;

extern "C" {
struct _RASTER;
}

//===============================================================

//********************************************************************************
//    TRopException  definition
//********************************************************************************

class DVAPI TRopException final : public TException {
  std::string message;

public:
  TRopException(const std::string &s) : message(s) {}
  ~TRopException() {}

  TString getMessage() const override;
};

//********************************************************************************
//    TRop  namespace
//********************************************************************************

//! The TRop namespace provides various algorithms to operate with Toonz's
//! raster types
namespace TRop {

enum ResampleFilterType {
  None,
  //! triangle filter
  Triangle,
  //! Mitchell-Netravali filter
  Mitchell,
  //! cubic convolution, a = .5
  Cubic5,
  //! cubic convolution, a = .75
  Cubic75,
  //! cubic convolution, a = 1
  Cubic1,
  //! Hann window, rad = 2
  Hann2,
  //! Hann window, rad = 3
  Hann3,
  //! Hamming window, rad = 2
  Hamming2,
  //! Hamming window, rad = 3
  Hamming3,
  //! Lanczos window, rad = 2
  Lanczos2,
  //! Lanczos window, rad = 3
  Lanczos3,
  //! Gaussian convolution
  Gauss,
  //! 'Quick' filter
  ClosestPixel,
  //! 'Quick' filter
  Bilinear,

  HowMany
};

enum ColorMask { RChan = 0x1, GChan = 0x2, BChan = 0x4, MChan = 0x8 };

enum ErodilateMaskType { ED_rectangular, ED_circular };

//! Applies first order mappings to each of \b rin's channels.
/*! \note The input and output rasters must have the same size and pixel type.
            Supports rout == rin. */
DVAPI void rgbmScale(TRasterP rout, TRasterP rin, const double *k,
                     const double *a, const int *out0, const int *out1);

DVAPI void rgbmScale(TRasterP rout, TRasterP rin, double kr, double kg,
                     double kb, double km, double ar = 0.0, double ag = 0.0,
                     double ab = 0.0, double am = 0.0);

/*! Transforms each input channel interval into the corresponding channel
   interval,
      cropping values if necessary.

      Specified pointers are associated to int[5] arrays representing the 5-ple
      (RGB, R, G, B, M) - where RGBM adjustment apply first. Then, Global apply
      equally on RGB channels (but not on M).*/
/*! \note The input and output rasters must have the same size and pixel type.
            Supports rout == rin. */
DVAPI void rgbmAdjust(TRasterP rout, TRasterP rin, const int *in0,
                      const int *in1, const int *out0, const int *out1);

//! Make a checkoboard in \b rout
/*! Two variables \b TPixel32 describe the checkboard color,
      \b TDimension are checkboard dimension and \b TPoinD the position
  \include checkBoard_ex.cpp*/
inline void checkBoard(TRasterP rout, const TPixel32 &, const TPixel32 &,
                       const TDimensionD &, const TPointD &);

//********************************************************************************
//    TRop::checkBoard  implementation
//
// The pattern computation lives in common/trop/rust/checkboard.rs
// (flare_formats); only the raster-type dispatch remains here. Note that
// \b dim drives the pattern frequency while the loop runs over the raster's
// own size -- the C++ treated these as independent and so does this.
//********************************************************************************

extern "C" {
void flare_trop_checkboard_u8(void *out, int wrap, int lx, int ly,
                              const void *pix1, const void *pix2, double dimLx,
                              double dimLy, double offsetX, double offsetY);
void flare_trop_checkboard_u16(void *out, int wrap, int lx, int ly,
                               const void *pix1, const void *pix2, double dimLx,
                               double dimLy, double offsetX, double offsetY);
void flare_trop_checkboard_f32(void *out, int wrap, int lx, int ly,
                               const void *pix1, const void *pix2, double dimLx,
                               double dimLy, double offsetX, double offsetY);
}

inline void checkBoard(TRasterP rout, const TPixel32 &pix1,
                       const TPixel32 &pix2, const TDimensionD &dim,
                       const TPointD &offset) {
  const int wrap = rout->getWrap(), lx = rout->getLx(), ly = rout->getLy();
  const double dimLx = dim.lx, dimLy = dim.ly, offX = offset.x,
               offY = offset.y;

  if (TRaster32P rout32 = rout) {
    flare_trop_checkboard_u8(rout32->pixels(), wrap, lx, ly, &pix1, &pix2, dimLx,
                             dimLy, offX, offY);
  } else if (TRaster64P rout64 = rout) {
    // Convert first, exactly as the original did, then hand over the bytes.
    const TPixel64 p1 = toPixel64(pix1), p2 = toPixel64(pix2);
    flare_trop_checkboard_u16(rout64->pixels(), wrap, lx, ly, &p1, &p2, dimLx,
                              dimLy, offX, offY);
  } else if (TRasterFP routF = rout) {
    const TPixelF p1 = toPixelF(pix1), p2 = toPixelF(pix2);
    flare_trop_checkboard_f32(routF->pixels(), wrap, lx, ly, &p1, &p2, dimLx,
                              dimLy, offX, offY);
  } else {
    throw TRopException("unsupported pixel type");
  }
}

//! Make the image over of the \b up raster on the \b dn raster and put the
//! resulting raster in \b rout
/*! \include over_ex1.cpp*/
DVAPI void over(const TRasterP &out, const TRasterP &dn, const TRasterP &up);
//! Make the image over of the \b up raster on the \b out raster starting from
//! \b pos
DVAPI void over(const TRasterP &out, const TRasterP &up,
                const TPoint &pos = TPoint());
//! Make the image over of the \b up raster on the \b out raster
DVAPI void over(const TRasterP &out, const TRasterP &up, const TAffine &aff,
                ResampleFilterType filterType = Triangle);
//! Make the image over of the \b up raster on the \b out raster starting from
//! \b pos.
//! Position \b pos must be already transformed using \b aff.
DVAPI void over(const TRasterP &out, const TRasterP &up, const TPoint &pos,
                const TAffine &aff, ResampleFilterType filterType = Triangle);
//! Make the image over of the \b up raster on the \b dn raster using a \b mask
//! and put the resulting raster in \b rout
DVAPI void over(const TRasterP &out, const TRasterP &dn, const TRasterP &up,
                const TRasterGR8P &mask);

//! Make a coloring of \b rup with \b color and put the resulting raster in \b
//! rout
DVAPI void over(TRaster32P rout, const TRasterGR8P &rup, const TPixel32 &color);

DVAPI void over(TRasterP rout, const TRasterCM32P &rup, TPalette *palette,
                const TPoint &point = TPoint(), const TAffine &aff = TAffine());

//! Apply the transformation \b aff to the raster \b in and put the resulting
//! raster in \b rout
/*!
      It can be specified a resample filter type and a blur factor.
    \include resample_ex.cpp
  */
DVAPI void resample(const TRasterP &out, const TRasterP &in, const TAffine &aff,
                    ResampleFilterType filterType = Triangle, double blur = 1.);

//! Like the over function, but only uses closest_pixel filter
DVAPI void quickPut(const TRasterP &out, const TRasterP &up, const TAffine &aff,
                    const TPixel32 &colorScale = TPixel::Black,
                    bool doPremultiply = false, bool whiteTransp = false,
                    bool firstColumn               = false,
                    bool doRasterDarkenBlendedView = false);

//! Like the over function but up image must be cmapped.
/*! Only uses closest_pixel filter.
    GlobalMatte is the opacity, used for onionskinning
  */

DVAPI void quickPut(const TRasterP &out, const TRasterCM32P &up,
                    const TPaletteP &plt, const TAffine &aff,
                    const TPixel32 &globalColorScale = TPixel::Black,
                    bool inksOnly                    = false);

// for transparency check, ink check and paint check

class CmappedQuickputSettings {
public:
  TPixel32 m_globalColorScale, m_transpCheckPaint, m_transpCheckBg,
      m_transpCheckInk;

  int m_inkIndex, m_paintIndex;

  bool m_inksOnly, m_transparencyCheck, m_blackBgCheck, m_isOnionSkin;
  int m_gapCheckIndex = -1;

  CmappedQuickputSettings()
      : m_globalColorScale(TPixel32::Black)
      , m_inksOnly(false)
      , m_transparencyCheck(false)
      , m_blackBgCheck(false)
      , m_isOnionSkin(false)
      , m_gapCheckIndex(-1)
      , m_inkIndex(-1)
      , m_paintIndex(-1) {}
};

DVAPI void quickPut(const TRasterP &dn, const TRasterCM32P &upCM32,
                    const TPaletteP &plt, const TAffine &aff,
                    const CmappedQuickputSettings &settings);

DVAPI void quickResampleColorFilter(const TRasterP &dn, const TRasterP &up,
                                    const TAffine &aff, const TPaletteP &plt,
                                    UCHAR colorMask);

DVAPI void convolve_3_i(TRasterP rout, TRasterP rin, int dx, int dy,
                        double conv[]);

DVAPI void convolve_i(TRasterP rout, TRasterP rin, int dx, int dy,
                      double conv[], int radius);

DVAPI void fracmove(TRasterP rout, TRasterP rin, double dx, double dy);

//! Convert \b src raster in raster type \b dst
DVAPI void convert(TRasterP dst, const TRasterP &src);

//********************************************************************************
//    TRop::convert  implementation
//
// The per-conversion kernels live in common/trop/rust/convert.rs (flare_formats);
// only the raster-type dispatch (and locking) remains here.
//********************************************************************************

extern "C" {
void flare_convert_64_from_32(const void *src, void *dst, int wrapSrc,
                              int wrapDst, int lx, int ly);
void flare_convert_gr8_from_32(const void *src, void *dst, int wrapSrc,
                               int wrapDst, int lx, int ly);
void flare_convert_gr16_from_32(const void *src, void *dst, int wrapSrc,
                                int wrapDst, int lx, int ly);
void flare_convert_gr16_from_64(const void *src, void *dst, int wrapSrc,
                                int wrapDst, int lx, int ly);
void flare_convert_32_from_gr8(const void *src, void *dst, int wrapSrc,
                               int wrapDst, int lx, int ly);
void flare_dither_convert_64_to_32(const void *src, void *dst, int wrapSrc,
                                   int wrapDst, int lx, int ly);
void flare_convert_cm32_from_gr8(const void *src, void *dst, int wrapSrc,
                                 int wrapDst, int lx, int ly);
void flare_convert_cm32_from_32(const void *src, void *dst, int wrapSrc,
                                int wrapDst, int lx, int ly);
void flare_convert_yuv_from_32(const void *src, void *dst, int wrapSrc,
                               int wrapDst, int lx, int ly);
void flare_convert_32_from_yuv(const void *src, void *dst, int wrapSrc,
                               int wrapDst, int lx, int ly);
void flare_convert_fp_from_32(const void *src, void *dst, int wrapSrc,
                              int wrapDst, int lx, int ly);
void flare_convert_fp_from_64(const void *src, void *dst, int wrapSrc,
                              int wrapDst, int lx, int ly);
void flare_convert_32_from_fp(const void *src, void *dst, int wrapSrc,
                              int wrapDst, int lx, int ly);
void flare_convert_64_from_fp(const void *src, void *dst, int wrapSrc,
                              int wrapDst, int lx, int ly);
}

void TRop::convert(TRasterP dst, const TRasterP &src) {
  if (dst->getSize() != src->getSize())
    throw TRopException("convert: size mismatch");

  TRaster32P dst32 = dst;
  TRasterGR8P dst8 = dst;
  TRasterGR16P dst16 = dst;
  TRaster64P dst64 = dst;
  TRasterCM32P dstCm = dst;
  TRasterYUV422P dstYUV = dst;
  TRasterFP dstF = dst;

  TRaster32P src32 = src;
  TRasterGR8P src8 = src;
  TRaster64P src64 = src;
  TRasterYUV422P srcYUV = src;
  TRasterFP srcF = src;

  const int lx = src->getLx(), ly = src->getLy();
  src->lock();
  dst->lock();

  bool handled = true;
  if (dst64 && src32) {
    flare_convert_64_from_32(src32->pixels(), dst64->pixels(),
                             src32->getWrap(), dst64->getWrap(), lx, ly);
  } else if (dst8 && src32) {
    flare_convert_gr8_from_32(src32->pixels(), dst8->pixels(),
                              src32->getWrap(), dst8->getWrap(), lx, ly);
  } else if (dst16 && src32) {
    flare_convert_gr16_from_32(src32->pixels(), dst16->pixels(),
                               src32->getWrap(), dst16->getWrap(), lx, ly);
  } else if (dst32 && src64) {
    flare_dither_convert_64_to_32(src64->pixels(), dst32->pixels(),
                                  src64->getWrap(), dst32->getWrap(), lx, ly);
  } else if (dst16 && src64) {
    flare_convert_gr16_from_64(src64->pixels(), dst16->pixels(),
                               src64->getWrap(), dst16->getWrap(), lx, ly);
  } else if (dst32 && src8) {
    flare_convert_32_from_gr8(src8->pixels(), dst32->pixels(),
                              src8->getWrap(), dst32->getWrap(), lx, ly);
  } else if (dstYUV && src32) {
    flare_convert_yuv_from_32(src32->pixels(), dstYUV->getRawData(),
                              src32->getWrap(), dstYUV->getRowSize(), lx, ly);
  } else if (dst32 && srcYUV) {
    flare_convert_32_from_yuv(srcYUV->getRawData(), dst32->pixels(),
                              srcYUV->getRowSize(), dst32->getWrap(), lx, ly);
  } else if (dstCm && src32) {
    flare_convert_cm32_from_32(src32->pixels(), dstCm->pixels(),
                               src32->getWrap(), dstCm->getWrap(), lx, ly);
  } else if (dstCm && src8) {
    flare_convert_cm32_from_gr8(src8->pixels(), dstCm->pixels(),
                                src8->getWrap(), dstCm->getWrap(), lx, ly);
  } else if (dstF && src32) {
    flare_convert_fp_from_32(src32->pixels(), dstF->pixels(),
                             src32->getWrap(), dstF->getWrap(), lx, ly);
  } else if (dstF && src64) {
    flare_convert_fp_from_64(src64->pixels(), dstF->pixels(),
                             src64->getWrap(), dstF->getWrap(), lx, ly);
  } else if (dst32 && srcF) {
    flare_convert_32_from_fp(srcF->pixels(), dst32->pixels(),
                             srcF->getWrap(), dst32->getWrap(), lx, ly);
  } else if (dst64 && srcF) {
    flare_convert_64_from_fp(srcF->pixels(), dst64->pixels(),
                             srcF->getWrap(), dst64->getWrap(), lx, ly);
  } else {
    handled = false;
  }

  if (!handled) {
    dst->unlock();
    src->unlock();
    throw TRopException("unsupported pixel type");
  }

  dst->setLinear(src->isLinear());
  dst->unlock();
  src->unlock();
}

//! Copy \b src in \b dst
DVAPI void copy(TRasterP dst, const TRasterP &src);

DVAPI void brush(TRaster32P ras, const TPoint &a, const TPoint &b, int radius,
                 const TPixel32 &color);

//! Get \b bbox of \b ras raster
inline void computeBBox(TRasterP ras, TRect &bbox);

//********************************************************************************
//    TRop::computeBBox  implementation
//
// The scan lives in common/trop/rust/bbox.rs (flare_formats); only the
// raster-type dispatch remains here. TRectT<int>() is (0,0,-1,-1), which is
// what the Rust reports as Rect::EMPTY, and getBounds() is (0,0,lx-1,ly-1).
//********************************************************************************

struct FlareTRect {
  int x0, y0, x1, y1;
};

extern "C" {
void flare_trop_bbox_u8(const void *ras, int wrap, int lx, int ly,
                        struct FlareTRect *out);
void flare_trop_bbox_u16(const void *ras, int wrap, int lx, int ly,
                         struct FlareTRect *out);
void flare_trop_bbox_cm32(const void *ras, int wrap, int lx, int ly,
                          struct FlareTRect *out);
}

inline void computeBBox(TRasterP ras, TRect &bbox) {
  const int lx = ras->getLx(), ly = ras->getLy();

  // GR8 has no matte, so every pixel is opaque: the bounds are the answer.
  if (ras) {
    bbox = ras->getBounds();
  }

  FlareTRect r = {0, 0, 0, 0};
  if (TRaster32P ras32 = ras) {
    flare_trop_bbox_u8(ras32->pixels(), ras32->getWrap(), lx, ly, &r);
  } else if (TRaster64P ras64 = ras) {
    flare_trop_bbox_u16(ras64->pixels(), ras64->getWrap(), lx, ly, &r);
  } else if (TRasterCM32P rasCM32 = ras) {
    flare_trop_bbox_cm32(rasCM32->pixels(), rasCM32->getWrap(), lx, ly, &r);
  } else if (ras) {
    // Not a raster we can scan (GR8 and friends): bounds already assigned.
    return;
  } else {
    assert(0);
    return;
  }

  bbox.x0 = r.x0;
  bbox.y0 = r.y0;
  bbox.x1 = r.x1;
  bbox.y1 = r.y1;
}

//! Add background of color \b col to the \b ras raster
DVAPI void addBackground(TRasterP ras, const TPixel32 &col);

//! Return true if \b ras is opaque
DVAPI bool isOpaque(TRasterP ras);

//! Invert \b ras color in according with \b invRed, \b invGreen, \b invBlue, \b
//! invMatte
//!
//! The pixel kernels live in common/trop/rust/invert.rs (flare_formats); only
//! the raster-type dispatch remains, in common/trop/trop.cpp. There is no
//! invert.cpp any more.
inline void invert(TRasterP ras, bool invRed = true, bool invGreen = true,
                   bool invBlue = true, bool invMatte = false);

//! Add \b rdown pixels to \b rup pixels in according with the factor \b v and
//! put the resulting raster in \b rout
DVAPI void add(const TRasterP &rup, const TRasterP &rdown, const TRasterP &rout,
               double v);

//! Add \b rdown pixels to \b rup pixels and put the resulting raster in \b rout
DVAPI void add(const TRasterP &rup, const TRasterP &rdown,
               const TRasterP &rout);

DVAPI void colordodge(const TRasterP &rup, const TRasterP &rdown,
                      const TRasterP &rout);

DVAPI void colorburn(const TRasterP &rup, const TRasterP &rdown,
                     const TRasterP &rout);

DVAPI void screen(const TRasterP &rup, const TRasterP &rdown,
                  const TRasterP &rout);

//! Subtract \b rdown pixels to \b rup pixels and put the resulting raster in \b
//! rout
DVAPI void sub(const TRasterP &rup, const TRasterP &rdown, const TRasterP &rout,
               bool matte);

DVAPI void mult(const TRasterP &rup, const TRasterP &rdown,
                const TRasterP &rout, int v, bool matte);  //[-255..+255]

DVAPI void ropin(const TRasterP &source, const TRasterP &matte,
                 const TRasterP &rout);

DVAPI void ropout(const TRasterP &source, const TRasterP &matte,
                  const TRasterP &rout);

DVAPI void atop(const TRasterP &rup, const TRasterP &rdown,
                const TRasterP &rout);

DVAPI void txor(const TRasterP &rup, const TRasterP &rdown,
                const TRasterP &rout);

DVAPI void crossDissolve(const TRasterP &rup, const TRasterP &rdown,
                         const TRasterP &rout, UCHAR v);  //[0..255]

DVAPI void lighten(const TRasterP &rup, const TRasterP &rdown,
                   const TRasterP &rout);

DVAPI void darken(const TRasterP &rup, const TRasterP &rdown,
                  const TRasterP &rout);

DVAPI void ropmin(const TRasterP &rup, const TRasterP &rdown,
                  const TRasterP &rout, bool matte);

DVAPI void ropmax(const TRasterP &rup, const TRasterP &rdown,
                  const TRasterP &rout);
// DVAPI void linearburn(const TRasterP &rup, const TRasterP &rdown,
//                       const TRasterP &rout);
// DVAPI void overlay(const TRasterP &rup, const TRasterP &rdown,
//                    const TRasterP &rout);

//! Make a premultiply of all raster pixels
DVAPI void premultiply(const TRasterP &ras);

//! Make a depremultiply of all raster pixels
DVAPI void depremultiply(const TRasterP &ras);

// called from meshtexturizer in order to remove unwanted black contour
// appears at the border of the plastic-deformed texture.
// this function will "expand" color channels of the border pixels to
// neighbor full-transparent pixels.
// by doing this the linear interpolation at the border pixels will not
// decay color info and can get ideal antialiased result
DVAPI void expandColor(const TRaster32P &ras32, bool precise);

//! all white pixels are set to transparent
DVAPI void whiteTransp(const TRasterP &ras);

//! apply color filter and semi-transparency of column to the render result
//  implemented in toperators.cpp
DVAPI void applyColorScale(const TRasterP &ras, const TPixel32 &colorScale);

DVAPI TRasterP shrink(TRasterP rin, int shrink);

//! Make a \b gamma correct in the \b raster
DVAPI void gammaCorrect(TRasterP raster, double gamma);

//! Make a \b gamma correct in the \b raster
DVAPI void gammaCorrectRGBM(TRasterP raster, double gammar, double gammag,
                            double gammab, double gammam);

//! Return the pixel size of the \b blur border
DVAPI int getBlurBorder(double blur);

//! Inserts antialias around jaggy lines. Threshold is a pixel distance intended
//! from 0 to 256.
//! Softness may vary from 0 to 100.
DVAPI void antialias(const TRasterP &src, const TRasterP &dst, int threshold,
                     int softness);

//********************************************************************************
//    TRop::antialias  implementation
//
// The morphological antialias lives in common/trop/rust/antialias.rs
// (flare_formats); only the raster-type dispatch remains here.
//********************************************************************************

extern "C" {
void flare_antialias(const void *src, void *dst, int wrapSrc, int wrapDst,
                     int lx, int ly, int threshold, int softness, int isCm32,
                     int is64);
}

void TRop::antialias(const TRasterP &src, const TRasterP &dst, int threshold,
                     int softness) {
  assert(src->getSize() == dst->getSize());

  TRaster32P src32(src), dst32(dst);
  TRaster64P src64(src), dst64(dst);
  TRasterCM32P srcCM(src), dstCM(dst);

  const bool is_cm32 = (bool)srcCM && (bool)dstCM;
  const bool is_64 = (bool)src64 && (bool)dst64;

  const void *srcPix = nullptr;
  void *dstPix = nullptr;
  int srcWrap = 0, dstWrap = 0;
  if (src32 && dst32) {
    srcPix = src32->pixels();
    dstPix = dst32->pixels();
    srcWrap = src32->getWrap();
    dstWrap = dst32->getWrap();
  } else if (src64 && dst64) {
    srcPix = src64->pixels();
    dstPix = dst64->pixels();
    srcWrap = src64->getWrap();
    dstWrap = dst64->getWrap();
  } else if (srcCM && dstCM) {
    srcPix = srcCM->pixels();
    dstPix = dstCM->pixels();
    srcWrap = srcCM->getWrap();
    dstWrap = dstCM->getWrap();
  } else {
    assert(!"Source and destination rasters must be of the same type!");
    return;
  }

  src->lock();
  dst->lock();

  flare_antialias(srcPix, dstPix, srcWrap, dstWrap, src->getLx(), src->getLy(),
                  threshold, softness, is_cm32 ? 1 : 0, is_64 ? 1 : 0);

  dst->unlock();
  src->unlock();
}

//! Performs despeckling by replacing the color of small uniform areas (those
//! enclosable in a
//! square of specified edge length) with that most occurring along the area
//! border.
DVAPI void majorityDespeckle(const TRasterP &ras, int sizeThreshold);

//! Make blur effect on \b srcRas raster and put the result in \b dstRas.
DVAPI void blur(const TRasterP &dstRas, const TRasterP &srcRas, double blur,
                int dx, int dy, bool useSSE = false);

struct RaylitParams {
  TPixel m_color;
  T3DPoint m_lightOriginSrc;
  T3DPoint m_lightOriginDst;
  double m_smoothness;
  double m_decay;
  double m_intensity;
  double m_scale;
  bool m_invert;
  bool m_includeInput;
  double m_radius;
};

//! Make raylit effect on \b srcRas raster and put the result in \b dstRas.
DVAPI void raylit(const TRasterP &dstRas, const TRasterP &srcRas,
                  const RaylitParams &params);
DVAPI void glassRaylit(const TRasterP &dstRas, const TRasterP &srcRas,
                       const RaylitParams &params);

DVAPI void despeckle(const TRasterP &ras, int sizeThreshold, bool check,
                     bool trasparentIsWhite = false);
DVAPI void despeckle(const TRasterP &rout, const TRasterP &rin,
                     int sizeThreshold, bool check);

//! Return a raster with r b channels swapped
DVAPI TRaster32P copyAndSwapRBChannels(const TRaster32P &srcRaster);

// result in 'left'
DVAPI void makeStereoRaster(const TRasterP &left, const TRasterP &right);

DVAPI void setChannel(const TRasterP &rin, TRasterP rout, UCHAR chan,
                      bool greytones);

/*!
    Applies classical morphological erosion/dilation to the specified input
   raster.
    \note Supports rin == rout.
  */
DVAPI void erodilate(const TRasterP &rin, const TRasterP &rout, double radius,
                     ErodilateMaskType type);

#ifdef TNZ_MACHINE_CHANNEL_ORDER_MRGB
DVAPI void swapRBChannels(const TRaster32P &r);
#endif

///////////////////////////////////////
//  Utilities for Toonz 4.6 porting  //
///////////////////////////////////////

//! Convert TRasterP in an old toonz raster!
/*! Use the same buffer, not creates a new raster (the palette is new instead!)
 */
DVAPI _RASTER *convertRaster50to46(const TRasterP &inRas,
                                   const TPaletteP &inPalette);

//! Release the old toonz raster.
/*! If the \b doReleaseBuffer is true, the buffer is released
    (the palette is always deleted!)
  */
DVAPI void releaseRaster46(_RASTER *&r, bool doReleaseBuffer = false);

//! Read an image in the path \b filename and put it in an old toonz raster
DVAPI _RASTER *readRaster46(const char *filename);

//! Retrieves the input raster from Toonz internal cache. Need to call it before
//! using the raster.
DVAPI void lockRaster(_RASTER *raster);

//! Surrenders the input raster's buffer to the cache. Should be called before
//! inactivity periods.
DVAPI void unlockRaster(_RASTER *raster);

// conversion between sRGB <--> Linear RGB
DVAPI void toLinearRGB(TRasterP raster, double gamma,
                       bool sourceIsPremultiplied = true);
DVAPI void tosRGB(TRasterP raster, double gamma,
                  bool sourceIsPremultiplied = true);

DVAPI void adjustGain(TRasterP raster, int gainStep, double gamma);

}  // namespace TRop

//********************************************************************************
//    TRop::invert  implementation
//
// The pixel kernels live in common/trop/rust/invert.rs (flare_formats); only
// the raster-type dispatch remains here. \b wrap, \b lx and \b ly are in
// pixels, as TRasterT reports them.
//********************************************************************************

extern "C" {
void flare_trop_invert_u8(void *pixels, int wrap, int lx, int ly, bool invR,
                          bool invG, bool invB, bool invM);
void flare_trop_invert_u16(void *pixels, int wrap, int lx, int ly, bool invR,
                           bool invG, bool invB, bool invM);
void flare_trop_invert_f32(void *pixels, int wrap, int lx, int ly, bool invR,
                           bool invG, bool invB, bool invM);
void flare_trop_invert_gr8(void *pixels, int wrap, int lx, int ly);
}

namespace TRop {

inline void invert(TRasterP ras, bool invRed, bool invGreen, bool invBlue,
                   bool invMatte) {
  // Matches the original: a matte flag on its own is a no-op, and GR8 ignores
  // the channel flags entirely.
  if (!invRed && !invGreen && !invBlue) return;

  const int wrap = ras->getWrap(), lx = ras->getLx(), ly = ras->getLy();

  if (TRaster32P ras32 = ras) {
    flare_trop_invert_u8(ras32->pixels(), wrap, lx, ly, invRed, invGreen, invBlue,
                         invMatte);
  } else if (TRaster64P ras64 = ras) {
    flare_trop_invert_u16(ras64->pixels(), wrap, lx, ly, invRed, invGreen, invBlue,
                          invMatte);
  } else if (TRasterFP rasF = ras) {
    flare_trop_invert_f32(rasF->pixels(), wrap, lx, ly, invRed, invGreen, invBlue,
                          invMatte);
  } else if (TRasterGR8P ras8 = ras) {
    flare_trop_invert_gr8(ras8->pixels(), wrap, lx, ly);
  } else {
    throw TRopException("unsupported pixel type");
  }
}

}  // namespace TRop
