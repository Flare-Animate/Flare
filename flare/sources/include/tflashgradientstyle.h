#pragma once

#include "tsimplecolorstyles.h"

#include <vector>

// A palette-owned, editable gradient field in drawing-local coordinates. The
// gradient-to-local matrix maps [-1, 1]^2 to the drawing. Source-specific
// coordinate conversion belongs in the importer, not in this native style.
class DVAPI TFlashGradientStyle final : public TSolidColorStyle {
public:
  enum class Kind { Linear = 0, Radial = 1 };
  enum class Spread { Pad = 0, Reflect = 1, Repeat = 2 };
  enum class Interpolation { Rgb = 0, LinearRgb = 1 };

  struct Stop {
    double ratio = 0.0;
    TPixel32 color;
    // Retain authored alpha precision beyond TPixel32's 8-bit editor value.
    double alpha = 1.0;
  };

  TFlashGradientStyle();

  bool setGradient(Kind kind, const TAffine &gradientToLocal,
                   const std::vector<Stop> &stops, Spread spread = Spread::Pad,
                   Interpolation interpolation = Interpolation::Rgb,
                   double focalRatio           = 0.0);

  Kind kind() const { return m_kind; }
  const TAffine &gradientToLocal() const { return m_gradientToLocal; }
  const std::vector<Stop> &stops() const { return m_stops; }
  Spread spread() const { return m_spread; }
  Interpolation interpolation() const { return m_interpolation; }
  double focalRatio() const { return m_focalRatio; }

  // Evaluates a point in local drawing space. Output is straight RGBA.
  TPixel32 sample(const TPointD &localPoint) const;
  // Evaluates a point in gradient space, where [-1, 1] spans the field.
  TPixel32 sampleField(const TPointD &gradientPoint) const;

  TColorStyle *clone() const override;
  int getTagId() const override { return 6500; }
  QString getDescription() const override;
  std::string getBrushIdName() const override {
    return "FlashGradientFillStyle";
  }
  // Flash also paints strokes with gradients, in the same drawing space.
  bool isStrokeStyle() const override { return true; }

  TPixel32 getMainColor() const override;
  void setMainColor(const TPixel32 &color) override;
  // Mean of the stops; used where Flare needs one representative color.
  TPixel32 getAverageColor() const override;
  int getColorParamCount() const override;
  TPixel32 getColorParamValue(int index) const override;
  void setColorParamValue(int index, const TPixel32 &color) override;

  int getParamCount() const override;
  ParamType getParamType(int index) const override;
  QString getParamNames(int index) const override;
  void getParamRange(int index, double &min, double &max) const override;
  void getParamRange(int index, int &min, int &max) const override;
  void getParamRange(int index, QStringList &items) const override;
  double getParamValue(double_tag, int index) const override;
  int getParamValue(int_tag, int index) const override;
  void setParamValue(int index, double value) override;
  void setParamValue(int index, int value) override;

  // Rendering evaluates the field on the CPU into a bounded texture mapped
  // to drawing-local coordinates, so it also works on OpenGL 1.1 contexts.
  void drawRegion(const TColorFunction *cf, bool antiAliasing,
                  TRegionOutline &boundary) const override;
  void drawStroke(const TColorFunction *cf, TStrokeOutline *outline,
                  const TStroke *stroke) const override;
  void drawAliasedStroke(const TColorFunction *cf, TStrokeOutline *outline,
                         const TStroke *stroke) const override;

protected:
  void makeIcon(const TDimension &size) override;
  void loadData(TInputStreamInterface &is) override;
  void saveData(TOutputStreamInterface &os) const override;

private:
  void drawFieldStroke(const TColorFunction *cf, TStrokeOutline *outline) const;

  Kind m_kind;
  Spread m_spread;
  Interpolation m_interpolation;
  TAffine m_gradientToLocal;
  TAffine m_localToGradient;
  double m_focalRatio;
  std::vector<Stop> m_stops;
};

// A palette-owned Flash bitmap fill: a bitmap placed in the drawing by an
// affine map, repeated or clamped beyond its edges and sampled smoothly
// (bilinear) or by nearest pixel, as Flash draws bitmap fills. The bitmap is
// kept in the style (and in the PLI palette), straight RGBA, rows bottom-up
// like every Flare raster. bitmapToLocal maps bitmap coordinates, where
// pixel (i, j) covers [i, i + 1] x [j, j + 1], to drawing-local coordinates.
class DVAPI TFlashBitmapStyle final : public TSolidColorStyle {
public:
  TFlashBitmapStyle();

  // `name` identifies the bitmap (an importer's library item name).
  bool setBitmap(const TRaster32P &bitmap, const TAffine &bitmapToLocal,
                 bool repeat, bool smooth, const std::string &name);

  const TRaster32P &bitmap() const { return m_bitmap; }
  const TAffine &bitmapToLocal() const { return m_bitmapToLocal; }
  bool repeat() const { return m_repeat; }
  bool smooth() const { return m_smooth; }
  const std::string &name() const { return m_name; }

  // Straight RGBA at a point in local drawing space.
  TPixel32 sample(const TPointD &localPoint) const;
  // Straight RGBA at a point in bitmap coordinates.
  TPixel32 sampleBitmap(const TPointD &bitmapPoint) const;

  TColorStyle *clone() const override;
  int getTagId() const override { return 6501; }
  QString getDescription() const override;
  std::string getBrushIdName() const override { return "FlashBitmapFillStyle"; }
  bool isStrokeStyle() const override { return true; }

  // The mean bitmap color stands for the fill where one color is needed.
  TPixel32 getMainColor() const override { return m_average; }
  void setMainColor(const TPixel32 &) override {}
  TPixel32 getAverageColor() const override { return m_average; }
  int getColorParamCount() const override { return 1; }
  TPixel32 getColorParamValue(int) const override { return m_average; }
  void setColorParamValue(int, const TPixel32 &) override {}

  int getParamCount() const override;
  ParamType getParamType(int index) const override;
  QString getParamNames(int index) const override;
  void getParamRange(int index, double &min, double &max) const override;
  void getParamRange(int index, int &min, int &max) const override;
  void getParamRange(int index, QStringList &items) const override;
  double getParamValue(double_tag, int index) const override;
  int getParamValue(int_tag, int index) const override;
  void setParamValue(int index, double value) override;
  void setParamValue(int index, int value) override;

  // Rendered like TFlashGradientStyle: the fill is evaluated on the CPU at
  // device resolution into textures mapped to drawing-local coordinates.
  void drawRegion(const TColorFunction *cf, bool antiAliasing,
                  TRegionOutline &boundary) const override;
  void drawStroke(const TColorFunction *cf, TStrokeOutline *outline,
                  const TStroke *stroke) const override;
  void drawAliasedStroke(const TColorFunction *cf, TStrokeOutline *outline,
                         const TStroke *stroke) const override;

protected:
  void makeIcon(const TDimension &size) override;
  void loadData(TInputStreamInterface &is) override;
  void saveData(TOutputStreamInterface &os) const override;

private:
  void drawBitmapStroke(const TColorFunction *cf,
                        TStrokeOutline *outline) const;

  TRaster32P m_bitmap;
  TAffine m_bitmapToLocal;
  TAffine m_localToBitmap;
  bool m_repeat;
  bool m_smooth;
  std::string m_name;
  TPixel32 m_average;
};
