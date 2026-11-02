#pragma once

#include "model.h"

#include <array>

namespace Flash {

// Rendering effects of a placed instance, evaluated from its XFL subtrees
// with Animate's defaults applied: the color effect (<color><Color/>), the
// filter list (<filters>) and the blendMode attribute. Never throws: values
// that cannot be read keep their defaults and are listed in `unmapped` as
// "<sourceId>: <detail>".

// Flash's color transform on unpremultiplied channels (0..255):
// c' = clamp(c * multiplier + offset). Index 0..3 is red, green, blue, alpha.
// Flash leaves fully transparent pixels transparent whatever the offsets.
struct ColorTransform {
  std::array<double, 4> multiplier{{1, 1, 1, 1}};
  std::array<double, 4> offset{{0, 0, 0, 0}};
  bool colorIdentity() const;  // red, green and blue untouched
};

struct ColorEffect {
  QString sourceId;  // the Color record
  ColorTransform transform;
  QStringList unmapped;
};

// Animate's color effect styles all become one transform: brightness b
// (-1..1) is a fade to black (b < 0) or white, tint t of color C is
// c * (1 - t) + C * t, and the advanced (or alpha) style gives the
// multipliers and offsets themselves (which also take precedence when
// Animate writes them beside the tint or brightness fields).
ColorEffect colorEffect(const Element &element);

struct Filter {
  enum class Kind {
    DropShadow,
    Blur,
    Glow,
    Bevel,
    GradientGlow,
    GradientBevel,
    AdjustColor,
  };
  Kind kind = Kind::Blur;
  QString sourceId;
  // Animate's defaults for omitted attributes (its filter panel defaults).
  double blurX = 5, blurY = 5;      // box width in stage pixels
  int quality     = 1;              // box blur passes (low 1, medium 2, high 3)
  double strength = 1;              // 1 is 100%
  double angle = 45, distance = 5;  // degrees (clockwise, Y down), pixels
  bool inner = false, knockout = false, hideObject = false;
  QString type = "inner";          // bevels: inner, outer or full
  Color color;                     // drop shadow and glow (with alpha)
  Color highlight, shadow;         // bevels
  QVector<GradientStop> gradient;  // gradient glow and bevel
  // Adjust color, as in Animate's filter panel.
  double brightness = 0, contrast = 0, saturation = 0, hue = 0;
  // Whether the filter changes the image at all (Flash drops filters that
  // do nothing, e.g. a blur of at most one pixel).
  bool effective() const;
};

struct FilterList {
  QString sourceId;
  QVector<Filter> filters;  // enabled filters, in the order Flash applies them
  QStringList unmapped;
};

FilterList elementFilters(const Element &element);

// The instance's blendMode, "normal" when omitted.
QString blendMode(const Element &element);

// The 4x5 color matrix (rows red, green, blue, alpha; columns red, green,
// blue, alpha, offset in 0..255 units) Animate's AdjustColor builds from the
// filter panel values: brightness, then contrast, saturation and hue
// (fl.motion.AdjustColor, which mirrors the authoring tool).
std::array<double, 20> adjustColorMatrix(double brightness, double contrast,
                                         double saturation, double hue);

// The radius (in pixels) of Flare's Blur FX whose triangle filter
// spreads like Flash's box blur of these widths and passes (equal variance;
// exact for medium quality with an odd whole width, two passes of a box
// being a triangle).
double blurRadius(double blurX, double blurY, int passes);

// The Flare layer blending FX for a Flash blend mode, one whose second
// input is the Xsheet port, so the column blends with everything below it
// in its Xsheet. Empty when there is none.
QString blendFxType(const QString &mode);

// Classic tweens interpolate color transforms and filter parameters
// component by component. Filters of one keyframe pair interpolate when their
// lists have the same kinds in the same order.
ColorTransform interpolate(const ColorTransform &a, const ColorTransform &b,
                           double t);
bool sameFilterKinds(const QVector<Filter> &a, const QVector<Filter> &b);
Filter interpolate(const Filter &a, const Filter &b, double t);

}  // namespace Flash
