// Drop Shadow (also glow) and Bevel FX.
//
// They follow the drop shadow, glow and bevel filters of Flash / Adobe
// Animate, so imported Flash filters stay editable FX with the same result:
// the source's alpha is box blurred (blur X/Y are the box widths; quality is
// the number of passes, each one horizontal then vertical, rounded down to
// 8 bits like the Flash Player), the shadow is offset along the angle and
// composited with the source. Lengths are FX lengths (they scale with the
// column like other FX); the angle and the blur axes are those of the
// rendered image (Flash keeps them aligned with the stage), clockwise from
// the right with 45 degrees pointing down-right.

#include "stdfx.h"
#include "texception.h"
#include "tfxparam.h"
#include "tparamset.h"
#include "tpixelutils.h"

#include <algorithm>
#include <cmath>
#include <vector>

namespace {

// A premultiplied RGBA float copy of a raster region.
struct FloatImage {
  int lx = 0, ly = 0;
  std::vector<float> pixels;  // r, g, b, m per pixel
  float *at(int x, int y) { return &pixels[4 * (size_t(y) * lx + x)]; }
};

template <class PIXEL>
void readRaster(const TRasterPT<PIXEL> &ras, FloatImage &image) {
  image.lx = ras->getLx(), image.ly = ras->getLy();
  image.pixels.assign(4 * size_t(image.lx) * image.ly, 0.f);
  const float max = float(PIXEL::maxChannelValue);
  for (int y = 0; y < image.ly; ++y) {
    const PIXEL *pix = ras->pixels(y);
    for (int x = 0; x < image.lx; ++x, ++pix) {
      float *out = image.at(x, y);
      out[0] = pix->r / max, out[1] = pix->g / max, out[2] = pix->b / max;
      out[3] = pix->m / max;
    }
  }
}

template <>
void readRaster<TPixelF>(const TRasterFP &ras, FloatImage &image) {
  image.lx = ras->getLx(), image.ly = ras->getLy();
  image.pixels.assign(4 * size_t(image.lx) * image.ly, 0.f);
  for (int y = 0; y < image.ly; ++y) {
    const TPixelF *pix = ras->pixels(y);
    for (int x = 0; x < image.lx; ++x, ++pix) {
      float *out = image.at(x, y);
      out[0] = pix->r, out[1] = pix->g, out[2] = pix->b, out[3] = pix->m;
    }
  }
}

template <class PIXEL>
void writePixel(PIXEL &pix, const float *value) {
  const float max = float(PIXEL::maxChannelValue);
  auto channel    = [max](float v) {
    return (typename PIXEL::Channel)std::lround(
           std::max(0.f, std::min(1.f, v)) * max);
  };
  pix.r = channel(value[0]), pix.g = channel(value[1]);
  pix.b = channel(value[2]), pix.m = channel(value[3]);
}

template <>
void writePixel<TPixelF>(TPixelF &pix, const float *value) {
  pix.r = value[0], pix.g = value[1], pix.b = value[2];
  pix.m = std::max(0.f, std::min(1.f, value[3]));
}

// One 1D box pass of width `width` pixels on `values` (stride `step`), as the
// Flash Player blurs: taps -(m+1)..(m+1) around each pixel, the two outer
// ones weighted by the fractional part, divided by the width and rounded
// down to 8 bits. Samples outside are zero.
void boxPass(float *values, int count, int step, double width,
             std::vector<double> &prefix, std::vector<float> &line) {
  if (width <= 1 || count <= 0) return;
  const double radius = (width - 1) / 2;
  const int m         = int(std::ceil(radius)) - 1;
  const double edge   = std::floor((radius - m) * 255) / 255;
  prefix.assign(size_t(count) + 1, 0.0);
  line.resize(count);
  for (int i = 0; i < count; ++i) {
    line[i]       = values[size_t(i) * step];
    prefix[i + 1] = prefix[i] + line[i];
  }
  auto sample = [&](int i) { return i < 0 || i >= count ? 0.f : line[i]; };
  for (int i = 0; i < count; ++i) {
    const int low = std::max(0, i - m), high = std::min(count, i + m + 1);
    double total = high > low ? prefix[high] - prefix[low] : 0.0;
    total += edge * (sample(i - m - 1) + sample(i + m + 1));
    const double v           = std::floor(total / width * 255 + 1e-6) / 255;
    values[size_t(i) * step] = float(std::max(0.0, std::min(1.0, v)));
  }
}

// Blurs the alpha plane `alpha` (lx * ly) in place.
void boxBlur(std::vector<float> &alpha, int lx, int ly, double widthX,
             double widthY, int passes) {
  std::vector<double> prefix;
  std::vector<float> line;
  for (int pass = 0; pass < passes; ++pass) {
    if (widthX > 1)
      for (int y = 0; y < ly; ++y)
        boxPass(&alpha[size_t(y) * lx], lx, 1, widthX, prefix, line);
    if (widthY > 1)
      for (int x = 0; x < lx; ++x)
        boxPass(&alpha[x], ly, lx, widthY, prefix, line);
  }
}

// Pixels a blur of this width and passes reaches on each side.
int blurReach(double width, int passes) {
  return width > 1 ? passes * int(std::ceil((width - 1) / 2)) : 0;
}

void premultipliedColor(const TPixelD &color, float *out) {
  out[0] = float(color.r * color.m), out[1] = float(color.g * color.m);
  out[2] = float(color.b * color.m), out[3] = float(color.m);
}

// Shared geometry of the shadow FX: the blur in output pixels and the
// offset of the shadow (raster Y points up).
struct ShadowGeometry {
  double widthX = 0, widthY = 0;
  int passes = 1;
  int dx = 0, dy = 0;  // rounded to whole pixels, as Flash samples them
  int reachX = 0, reachY = 0;

  ShadowGeometry(const TRenderSettings &info, double blurX, double blurY,
                 int quality, double angle, double distance) {
    const double scale   = std::sqrt(std::abs(info.m_affine.det()));
    widthX               = std::max(0.0, blurX * scale);
    widthY               = std::max(0.0, blurY * scale);
    passes               = std::max(0, std::min(15, quality));
    const double radians = angle * std::acos(-1.0) / 180;
    dx     = int(std::lround(distance * scale * std::cos(radians)));
    dy     = int(std::lround(-distance * scale * std::sin(radians)));
    reachX = blurReach(widthX, passes) + 1;
    reachY = blurReach(widthY, passes) + 1;
  }
};

}  // namespace

//==============================================================================

class DropShadowFx final : public TStandardRasterFx {
  FX_PLUGIN_DECLARATION(DropShadowFx)

  TRasterFxPort m_input;
  TPixelParamP m_color;
  TDoubleParamP m_blurX, m_blurY, m_strength, m_angle, m_distance;
  TIntParamP m_quality;
  TBoolParamP m_inner, m_knockout, m_hideObject;

public:
  DropShadowFx()
      : m_color(TPixel32::Black)
      , m_blurX(2.0)
      , m_blurY(2.0)
      , m_strength(1.0)
      , m_angle(45.0)
      , m_distance(2.0)
      , m_quality(1)
      , m_inner(false)
      , m_knockout(false)
      , m_hideObject(false) {
    m_color->enableMatte(true);
    m_blurX->setMeasureName("fxLength");
    m_blurY->setMeasureName("fxLength");
    m_distance->setMeasureName("fxLength");
    m_angle->setMeasureName("angle");
    bindParam(this, "color", m_color);
    bindParam(this, "blur_x", m_blurX);
    bindParam(this, "blur_y", m_blurY);
    bindParam(this, "quality", m_quality);
    bindParam(this, "strength", m_strength);
    bindParam(this, "angle", m_angle);
    bindParam(this, "distance", m_distance);
    bindParam(this, "inner", m_inner);
    bindParam(this, "knockout", m_knockout);
    bindParam(this, "hide_object", m_hideObject);
    m_blurX->setValueRange(0, (std::numeric_limits<double>::max)());
    m_blurY->setValueRange(0, (std::numeric_limits<double>::max)());
    m_strength->setValueRange(0, 255);
    m_quality->setValueRange(0, 15);
    addInputPort("Source", m_input);
    enableComputeInFloat(true);
  }

  bool canHandle(const TRenderSettings &info, double frame) override {
    return true;
  }

  ShadowGeometry geometry(double frame, const TRenderSettings &info) {
    return ShadowGeometry(info, m_blurX->getValue(frame),
                          m_blurY->getValue(frame), m_quality->getValue(),
                          m_angle->getValue(frame),
                          m_distance->getValue(frame));
  }

  bool doGetBBox(double frame, TRectD &bBox,
                 const TRenderSettings &info) override {
    if (!m_input.isConnected()) {
      bBox = TRectD();
      return false;
    }
    const bool ret = m_input->doGetBBox(frame, bBox, info);
    // An inner shadow stays inside the source.
    if (m_inner->getValue() || bBox == TConsts::infiniteRectD) return ret;
    const ShadowGeometry g = geometry(frame, info);
    TRectD shadow = bBox.enlarge(g.reachX, g.reachY) + TPointD(g.dx, g.dy);
    bBox          = m_hideObject->getValue() && !m_knockout->getValue() ? shadow
                                                                        : bBox + shadow;
    return ret;
  }

  TRectD inputRect(const TRectD &rect, const ShadowGeometry &g) {
    return rect.enlarge(g.reachX + std::abs(g.dx), g.reachY + std::abs(g.dy));
  }

  void doDryCompute(TRectD &rect, double frame,
                    const TRenderSettings &info) override {
    if (!m_input.isConnected()) return;
    TRectD in = inputRect(rect, geometry(frame, info));
    m_input->dryCompute(in, frame, info);
  }

  int getMemoryRequirement(const TRectD &rect, double frame,
                           const TRenderSettings &info) override {
    return TRasterFx::memorySize(inputRect(rect, geometry(frame, info)),
                                 sizeof(float) << 3);
  }

  void doCompute(TTile &tile, double frame,
                 const TRenderSettings &info) override;
};

void DropShadowFx::doCompute(TTile &tile, double frame,
                             const TRenderSettings &info) {
  if (!m_input.isConnected()) return;
  const ShadowGeometry g = geometry(frame, info);
  const int padX = g.reachX + std::abs(g.dx), padY = g.reachY + std::abs(g.dy);
  const TRasterP out = tile.getRaster();
  const int lx = out->getLx(), ly = out->getLy();
  TTile source;
  m_input->allocateAndCompute(source, tile.m_pos - TPointD(padX, padY),
                              TDimension(lx + 2 * padX, ly + 2 * padY), out,
                              frame, info);
  FloatImage image;
  if (TRaster32P ras = source.getRaster())
    readRaster(ras, image);
  else if (TRaster64P ras = source.getRaster())
    readRaster(ras, image);
  else if (TRasterFP ras = source.getRaster())
    readRaster(ras, image);
  else
    throw TException("Drop Shadow: unsupported pixel type");
  std::vector<float> alpha(size_t(image.lx) * image.ly);
  for (size_t i = 0; i < alpha.size(); ++i) alpha[i] = image.pixels[4 * i + 3];
  boxBlur(alpha, image.lx, image.ly, g.widthX, g.widthY, g.passes);

  float color[4];
  premultipliedColor(toPixelD(m_color->getValue(frame)), color);
  const double colorAlpha = color[3];
  const double strength   = m_strength->getValue(frame);
  const bool inner = m_inner->getValue(), knockout = m_knockout->getValue();
  const bool composite = !m_hideObject->getValue();
  auto blurred         = [&](int x, int y) {
    return x < 0 || y < 0 || x >= image.lx || y >= image.ly
                       ? 0.f
                       : alpha[size_t(y) * image.lx + x];
  };
  auto clamp01 = [](double v) { return std::max(0.0, std::min(1.0, v)); };
  out->lock();
  source.getRaster()->lock();
  for (int y = 0; y < ly; ++y)
    for (int x = 0; x < lx; ++x) {
      const float *s = image.at(x + padX, y + padY);
      // The shadow at a pixel is the blurred alpha `offset` pixels back.
      const double b = blurred(x + padX - g.dx, y + padY - g.dy);
      float result[4];
      if (inner) {
        const double a = clamp01((1 - b) * strength);  // times the color alpha
        for (int c = 0; c < 4; ++c) {
          const double glow = color[c] * a * s[3];
          result[c]         = float(knockout || !composite
                                        ? glow
                                        : glow + s[c] * (1 - a * colorAlpha));
        }
      } else {
        const double a = clamp01(b * strength);
        for (int c = 0; c < 4; ++c) {
          const double glow = color[c] * a;
          result[c]         = float(knockout    ? glow * (1 - s[3])
                                    : composite ? glow * (1 - s[3]) + s[c]
                                                : glow);
        }
      }
      if (TRaster32P ras = out)
        writePixel(ras->pixels(y)[x], result);
      else if (TRaster64P ras = out)
        writePixel(ras->pixels(y)[x], result);
      else if (TRasterFP ras = out)
        writePixel(ras->pixels(y)[x], result);
    }
  source.getRaster()->unlock();
  out->unlock();
}

FX_PLUGIN_IDENTIFIER(DropShadowFx, "dropShadowFx")

//==============================================================================

class BevelFx final : public TStandardRasterFx {
  FX_PLUGIN_DECLARATION(BevelFx)

  enum Type { INNER, OUTER, FULL };

  TRasterFxPort m_input;
  TPixelParamP m_highlight, m_shadow;
  TDoubleParamP m_blurX, m_blurY, m_strength, m_angle, m_distance;
  TIntParamP m_quality;
  TIntEnumParamP m_type;
  TBoolParamP m_knockout;

public:
  BevelFx()
      : m_highlight(TPixel32::White)
      , m_shadow(TPixel32::Black)
      , m_blurX(2.0)
      , m_blurY(2.0)
      , m_strength(1.0)
      , m_angle(45.0)
      , m_distance(2.0)
      , m_quality(1)
      , m_type(new TIntEnumParam(INNER, "Inner"))
      , m_knockout(false) {
    m_highlight->enableMatte(true);
    m_shadow->enableMatte(true);
    m_blurX->setMeasureName("fxLength");
    m_blurY->setMeasureName("fxLength");
    m_distance->setMeasureName("fxLength");
    m_angle->setMeasureName("angle");
    m_type->addItem(OUTER, "Outer");
    m_type->addItem(FULL, "Full");
    bindParam(this, "highlight_color", m_highlight);
    bindParam(this, "shadow_color", m_shadow);
    bindParam(this, "blur_x", m_blurX);
    bindParam(this, "blur_y", m_blurY);
    bindParam(this, "quality", m_quality);
    bindParam(this, "strength", m_strength);
    bindParam(this, "angle", m_angle);
    bindParam(this, "distance", m_distance);
    bindParam(this, "type", m_type);
    bindParam(this, "knockout", m_knockout);
    m_blurX->setValueRange(0, (std::numeric_limits<double>::max)());
    m_blurY->setValueRange(0, (std::numeric_limits<double>::max)());
    m_strength->setValueRange(0, 255);
    m_quality->setValueRange(0, 15);
    addInputPort("Source", m_input);
    enableComputeInFloat(true);
  }

  bool canHandle(const TRenderSettings &info, double frame) override {
    return true;
  }

  ShadowGeometry geometry(double frame, const TRenderSettings &info) {
    return ShadowGeometry(info, m_blurX->getValue(frame),
                          m_blurY->getValue(frame), m_quality->getValue(),
                          m_angle->getValue(frame),
                          m_distance->getValue(frame));
  }

  bool doGetBBox(double frame, TRectD &bBox,
                 const TRenderSettings &info) override {
    if (!m_input.isConnected()) {
      bBox = TRectD();
      return false;
    }
    const bool ret = m_input->doGetBBox(frame, bBox, info);
    if (m_type->getValue() == INNER || bBox == TConsts::infiniteRectD)
      return ret;
    const ShadowGeometry g = geometry(frame, info);
    bBox = bBox.enlarge(g.reachX + std::abs(g.dx), g.reachY + std::abs(g.dy));
    return ret;
  }

  TRectD inputRect(const TRectD &rect, const ShadowGeometry &g) {
    return rect.enlarge(g.reachX + std::abs(g.dx), g.reachY + std::abs(g.dy));
  }

  void doDryCompute(TRectD &rect, double frame,
                    const TRenderSettings &info) override {
    if (!m_input.isConnected()) return;
    TRectD in = inputRect(rect, geometry(frame, info));
    m_input->dryCompute(in, frame, info);
  }

  int getMemoryRequirement(const TRectD &rect, double frame,
                           const TRenderSettings &info) override {
    return TRasterFx::memorySize(inputRect(rect, geometry(frame, info)),
                                 sizeof(float) << 3);
  }

  void doCompute(TTile &tile, double frame,
                 const TRenderSettings &info) override;
};

void BevelFx::doCompute(TTile &tile, double frame,
                        const TRenderSettings &info) {
  if (!m_input.isConnected()) return;
  const ShadowGeometry g = geometry(frame, info);
  const int padX = g.reachX + std::abs(g.dx), padY = g.reachY + std::abs(g.dy);
  const TRasterP out = tile.getRaster();
  const int lx = out->getLx(), ly = out->getLy();
  TTile source;
  m_input->allocateAndCompute(source, tile.m_pos - TPointD(padX, padY),
                              TDimension(lx + 2 * padX, ly + 2 * padY), out,
                              frame, info);
  FloatImage image;
  if (TRaster32P ras = source.getRaster())
    readRaster(ras, image);
  else if (TRaster64P ras = source.getRaster())
    readRaster(ras, image);
  else if (TRasterFP ras = source.getRaster())
    readRaster(ras, image);
  else
    throw TException("Bevel: unsupported pixel type");
  std::vector<float> alpha(size_t(image.lx) * image.ly);
  for (size_t i = 0; i < alpha.size(); ++i) alpha[i] = image.pixels[4 * i + 3];
  boxBlur(alpha, image.lx, image.ly, g.widthX, g.widthY, g.passes);

  float highlight[4], shadow[4];
  premultipliedColor(toPixelD(m_highlight->getValue(frame)), highlight);
  premultipliedColor(toPixelD(m_shadow->getValue(frame)), shadow);
  const double strength = m_strength->getValue(frame);
  const int type        = m_type->getValue();
  const bool knockout   = m_knockout->getValue();
  auto blurred          = [&](int x, int y) {
    return x < 0 || y < 0 || x >= image.lx || y >= image.ly
                        ? 0.f
                        : alpha[size_t(y) * image.lx + x];
  };
  auto clamp01 = [](double v) { return std::max(0.0, std::min(1.0, v)); };
  out->lock();
  for (int y = 0; y < ly; ++y)
    for (int x = 0; x < lx; ++x) {
      const float *s = image.at(x + padX, y + padY);
      // The light side looks into the shape along the angle.
      const double lit            = blurred(x + padX + g.dx, y + padY + g.dy);
      const double dark           = blurred(x + padX - g.dx, y + padY - g.dy);
      const double highlightAlpha = clamp01((lit - dark) * strength);
      const double shadowAlpha    = clamp01((dark - lit) * strength);
      double glow[4];
      for (int c = 0; c < 4; ++c)
        glow[c] = highlight[c] * highlightAlpha + shadow[c] * shadowAlpha;
      float result[4];
      for (int c = 0; c < 4; ++c) {
        double v;
        if (type == FULL)
          v = knockout ? glow[c] : s[c] - s[c] * glow[3] + glow[c];
        else if (type == INNER)
          v = knockout ? glow[c] * s[3] : glow[c] * s[3] + s[c] * (1 - glow[3]);
        else
          v = knockout ? glow[c] - glow[c] * s[3]
                       : s[c] + glow[c] - glow[c] * s[3];
        result[c] = float(v);
      }
      if (TRaster32P ras = out)
        writePixel(ras->pixels(y)[x], result);
      else if (TRaster64P ras = out)
        writePixel(ras->pixels(y)[x], result);
      else if (TRasterFP ras = out)
        writePixel(ras->pixels(y)[x], result);
    }
  out->unlock();
}

FX_PLUGIN_IDENTIFIER(BevelFx, "bevelFx")
