#include "effects.h"

#include <QXmlStreamReader>
#include <QMap>
#include <QSet>
#include <cmath>

namespace Flash {
namespace {

constexpr auto xflNamespace = "http://ns.adobe.com/xfl/2008/";

// The element's XFL container `tag`; a repeated one is listed and ignored.
const Extra *extraOf(const Element &element, const char *tag,
                     QStringList &unmapped) {
  const Extra *found = nullptr;
  for (const auto &extra : element.extras)
    if (extra.tag == QLatin1String(tag) &&
        (extra.namespaceUri.isEmpty() || extra.namespaceUri == xflNamespace)) {
      if (found) {
        unmapped.push_back(element.sourceId + "/" + tag +
                           ": Repeated container; the first one is used");
        break;
      }
      found = &extra;
    }
  return found;
}

// Attribute evaluation with defaults: a value that cannot be read keeps the
// default and is listed.
struct AttributeReader {
  QXmlStreamAttributes attributes;
  QString sourceId;
  QStringList *unmapped;
  QSet<QString> known;

  bool has(const QString &name) {
    known.insert(name);
    return attributes.hasAttribute(name);
  }
  QString text(const QString &name) {
    known.insert(name);
    return attributes.value(name).toString().trimmed();
  }
  double number(const QString &name, double fallback, double low, double high) {
    if (!has(name)) return fallback;
    bool ok            = false;
    const double value = text(name).toDouble(&ok);
    if (!ok || !std::isfinite(value)) {
      unmapped->push_back(sourceId + ": Unreadable " + name + " \"" +
                          text(name).left(64) + "\"");
      return fallback;
    }
    if (value < low || value > high)
      unmapped->push_back(sourceId + ": " + name + " " + text(name) +
                          " is outside the range Flash plays; clamped");
    return std::max(low, std::min(high, value));
  }
  bool boolean(const QString &name, bool fallback) {
    if (!has(name)) return fallback;
    const QString value = text(name);
    if (value == "true") return true;
    if (value == "false") return false;
    unmapped->push_back(sourceId + ": Unreadable " + name + " \"" +
                        value.left(64) + "\"");
    return fallback;
  }
  Color color(const QString &name, const QString &alphaName, Color fallback) {
    Color result = fallback;
    if (has(name) && !parseHexRgb(text(name), result)) {
      unmapped->push_back(sourceId + ": Unreadable " + name + " \"" +
                          text(name).left(64) + "\"");
      result = fallback;
    }
    result.alpha = fallback.alpha;
    if (!alphaName.isEmpty())
      result.alpha = (unsigned char)std::lround(
          number(alphaName, fallback.alpha / 255.0, 0, 1) * 255);
    return result;
  }
  void reportUnknown() {
    for (const auto &attribute : attributes)
      if (!known.contains(attribute.qualifiedName().toString()))
        unmapped->push_back(sourceId + ": Unmapped attribute " +
                            attribute.qualifiedName().toString().left(64));
  }
};

bool xflElement(const QXmlStreamReader &xml) {
  return xml.namespaceUri().isEmpty() || xml.namespaceUri() == xflNamespace;
}

// Reads the elements of a retained container one level down; nested
// children are handed to `child` with the reader on their start tag.
template <class Visit>
void children(const Extra &extra, QStringList &unmapped, Visit visit) {
  QXmlStreamReader xml(extra.xml);
  int depth = 0, index = 0;
  while (!xml.atEnd()) {
    const auto token = xml.readNext();
    if (token == QXmlStreamReader::StartElement) {
      ++depth;
      if (depth == 2) {
        visit(xml, index++);
        --depth;  // visit() consumed the element
      } else if (depth > 2)
        xml.skipCurrentElement(), --depth;
    } else if (token == QXmlStreamReader::EndElement)
      --depth;
  }
  if (xml.hasError())
    unmapped.push_back(extra.sourceId + ": Unreadable XML (" +
                       xml.errorString().left(128) + ")");
}

using Matrix5 = std::array<double, 25>;

Matrix5 identity5() {
  Matrix5 m{};
  for (int i = 0; i < 5; ++i) m[i * 5 + i] = 1;
  return m;
}

// a * b, row-major 5x5.
Matrix5 multiply(const Matrix5 &a, const Matrix5 &b) {
  Matrix5 m{};
  for (int row = 0; row < 5; ++row)
    for (int col = 0; col < 5; ++col) {
      double sum = 0;
      for (int k = 0; k < 5; ++k) sum += a[row * 5 + k] * b[k * 5 + col];
      m[row * 5 + col] = sum;
    }
  return m;
}

double lerp(double a, double b, double t) { return a + (b - a) * t; }

unsigned char lerp(unsigned char a, unsigned char b, double t) {
  return (unsigned char)std::lround(
      std::max(0.0, std::min(255.0, lerp(double(a), double(b), t))));
}

Color lerp(const Color &a, const Color &b, double t) {
  return {lerp(a.red, b.red, t), lerp(a.green, b.green, t),
          lerp(a.blue, b.blue, t), lerp(a.alpha, b.alpha, t)};
}

// Variance of Flash's box kernel of width w (one 1D pass): taps -(m+1)..(m+1),
// the outer ones weighted by the fractional part (rounded down to 8 bits).
double boxVariance(double width) {
  if (width <= 1) return 0;
  const double radius = (width - 1) / 2;
  const double m      = std::ceil(radius) - 1;
  const double edge   = std::floor((radius - m) * 255) / 255;
  const double weight = 2 * m + 1 + 2 * edge;
  return (m * (m + 1) * (2 * m + 1) / 3 + 2 * edge * (m + 1) * (m + 1)) /
         weight;
}

// Variance of the Blur FX's triangle filter of radius r (TRop::blur):
// weights 1 - |i| / r for |i| < ceil(r).
double triangleVariance(double radius) {
  const int reach = int(std::ceil(radius));
  double sum = 0, moment = 0;
  for (int i = 1 - reach; i < reach; ++i) {
    const double w = 1 - std::abs(i) / radius;
    sum += w, moment += w * i * i;
  }
  return sum > 0 ? moment / sum : 0;
}

}  // namespace

bool ColorTransform::colorIdentity() const {
  for (int channel = 0; channel < 3; ++channel)
    if (multiplier[channel] != 1 || offset[channel] != 0) return false;
  return true;
}

ColorEffect colorEffect(const Element &element) {
  ColorEffect result;
  const Extra *extra = extraOf(element, "color", result.unmapped);
  if (!extra) return result;
  result.sourceId = element.sourceId + "/color/Color";
  int records     = 0;
  children(*extra, result.unmapped, [&](QXmlStreamReader &xml, int) {
    if (!xflElement(xml) || xml.name() != QLatin1String("Color")) {
      result.unmapped.push_back(element.sourceId + "/color: Unmapped " +
                                xml.name().toString().left(64));
      xml.skipCurrentElement();
      return;
    }
    if (++records > 1) {
      result.unmapped.push_back(element.sourceId +
                                "/color: Only the first Color record is used");
      xml.skipCurrentElement();
      return;
    }
    AttributeReader in{xml.attributes(), result.sourceId, &result.unmapped, {}};
    ColorTransform &t             = result.transform;
    static const char *channels[] = {"red", "green", "blue", "alpha"};
    bool explicitColor            = false;
    for (int channel = 0; channel < 4; ++channel) {
      const QString name = channels[channel];
      if (channel < 3)
        explicitColor |= in.has(name + "Multiplier") || in.has(name + "Offset");
      t.multiplier[channel] = in.number(name + "Multiplier", 1, -128, 128);
      t.offset[channel]     = in.number(name + "Offset", 0, -512, 512);
    }
    if (!explicitColor) {
      if (in.has("tintMultiplier") || in.has("tintColor")) {
        const double amount = in.number("tintMultiplier", 0, -1, 1);
        const Color tint    = in.color("tintColor", "", Color{});
        const double rgb[]  = {double(tint.red), double(tint.green),
                               double(tint.blue)};
        for (int channel = 0; channel < 3; ++channel) {
          t.multiplier[channel] = 1 - amount;
          t.offset[channel]     = rgb[channel] * amount;
        }
      } else if (in.has("brightness")) {
        const double brightness = in.number("brightness", 0, -1, 1);
        for (int channel = 0; channel < 3; ++channel) {
          t.multiplier[channel] = 1 - std::abs(brightness);
          t.offset[channel]     = brightness > 0 ? 255 * brightness : 0;
        }
      }
    } else {
      // Animate writes the style fields beside the transform it publishes.
      in.has("tintMultiplier");
      in.has("tintColor");
      in.has("brightness");
    }
    in.reportUnknown();
    while (xml.readNextStartElement()) {
      result.unmapped.push_back(result.sourceId + ": Unmapped " +
                                xml.name().toString().left(64));
      xml.skipCurrentElement();
    }
  });
  return result;
}

bool Filter::effective() const {
  switch (kind) {
  case Kind::Blur:
    return quality > 0 && (blurX > 1 || blurY > 1);
  case Kind::AdjustColor:
    return brightness != 0 || contrast != 0 || saturation != 0 || hue != 0;
  case Kind::Bevel:
  case Kind::GradientBevel:
    // Without an offset the two lookups cancel out (knockout still hides).
    return strength > 0 && (distance != 0 || knockout);
  case Kind::GradientGlow:
    return knockout || strength > 0;
  default:
    // A knocked-out or hidden object changes even without a visible glow.
    return knockout || hideObject || (strength > 0 && color.alpha > 0);
  }
}

FilterList elementFilters(const Element &element) {
  FilterList result;
  const Extra *extra = extraOf(element, "filters", result.unmapped);
  if (!extra) return result;
  result.sourceId = element.sourceId + "/filters";
  children(*extra, result.unmapped, [&](QXmlStreamReader &xml, int index) {
    Filter filter;
    const QString tag = xml.name().toString();
    filter.sourceId   = result.sourceId + "/" + tag + QString::number(index);
    static const QMap<QString, Filter::Kind> kinds{
        {"DropShadowFilter", Filter::Kind::DropShadow},
        {"BlurFilter", Filter::Kind::Blur},
        {"GlowFilter", Filter::Kind::Glow},
        {"BevelFilter", Filter::Kind::Bevel},
        {"GradientGlowFilter", Filter::Kind::GradientGlow},
        {"GradientBevelFilter", Filter::Kind::GradientBevel},
        {"AdjustColorFilter", Filter::Kind::AdjustColor}};
    if (!xflElement(xml) || !kinds.contains(tag)) {
      result.unmapped.push_back(filter.sourceId + ": Unmapped filter " +
                                tag.left(64));
      xml.skipCurrentElement();
      return;
    }
    filter.kind = kinds.value(tag);
    AttributeReader in{xml.attributes(), filter.sourceId, &result.unmapped, {}};
    const bool enabled =
        in.boolean("isEnabled", true) && in.boolean("enabled", true);
    using Kind      = Filter::Kind;
    const Kind kind = filter.kind;
    if (kind == Kind::AdjustColor) {
      filter.brightness = in.number("brightness", 0, -100, 100);
      filter.contrast   = in.number("contrast", 0, -100, 100);
      filter.saturation = in.number("saturation", 0, -100, 100);
      filter.hue        = in.number("hue", 0, -180, 180);
    } else {
      filter.blurX = in.number("blurX", 5, 0, 255);
      filter.blurY = in.number("blurY", 5, 0, 255);
      if (in.has("quality")) {
        const QString quality = in.text("quality");
        if (quality == "low")
          filter.quality = 1;
        else if (quality == "medium")
          filter.quality = 2;
        else if (quality == "high")
          filter.quality = 3;
        else
          filter.quality = int(std::lround(in.number("quality", 1, 0, 15)));
      }
    }
    if (kind != Kind::Blur && kind != Kind::AdjustColor) {
      filter.strength = in.number("strength", 1, 0, 255);
      filter.knockout = in.boolean("knockout", false);
    }
    if (kind == Kind::DropShadow || kind == Kind::Bevel ||
        kind == Kind::GradientGlow || kind == Kind::GradientBevel) {
      filter.angle    = in.number("angle", 45, -1e6, 1e6);
      filter.distance = in.number("distance", 5, -1e4, 1e4);
    }
    if (kind == Kind::DropShadow || kind == Kind::Glow) {
      filter.inner = in.boolean("inner", false);
      filter.color = in.color(
          "color", "alpha",
          kind == Kind::Glow ? Color{255, 0, 0, 255} : Color{0, 0, 0, 255});
    }
    if (kind == Kind::DropShadow)
      filter.hideObject = in.boolean("hideObject", false);
    if (kind == Kind::Bevel) {
      filter.highlight =
          in.color("highlightColor", "highlightAlpha", {255, 255, 255, 255});
      filter.shadow = in.color("shadowColor", "shadowAlpha", {0, 0, 0, 255});
    }
    if (kind == Kind::Bevel || kind == Kind::GradientBevel ||
        kind == Kind::GradientGlow) {
      filter.type = in.has("type")               ? in.text("type")
                    : kind == Kind::GradientGlow ? "outer"
                                                 : "inner";
      if (filter.type != "inner" && filter.type != "outer" &&
          filter.type != "full") {
        result.unmapped.push_back(filter.sourceId + ": Unknown type \"" +
                                  filter.type.left(32) + "\"; inner used");
        filter.type = "inner";
      }
    }
    if (kind == Kind::GradientGlow || kind == Kind::GradientBevel) {
      while (xml.readNextStartElement()) {
        if (!xflElement(xml) || xml.name() != QLatin1String("GradientEntry")) {
          result.unmapped.push_back(filter.sourceId + ": Unmapped " +
                                    xml.name().toString().left(64));
          xml.skipCurrentElement();
          continue;
        }
        GradientStop stop;
        stop.sourceId = filter.sourceId + "/GradientEntry" +
                        QString::number(filter.gradient.size());
        AttributeReader entry{
            xml.attributes(), stop.sourceId, &result.unmapped, {}};
        stop.color = entry.color("color", "alpha", {0, 0, 0, 255});
        stop.ratio = entry.number("ratio", 0, 0, 1);
        entry.reportUnknown();
        filter.gradient.push_back(stop);
        xml.skipCurrentElement();
      }
      if (filter.gradient.isEmpty())
        result.unmapped.push_back(filter.sourceId +
                                  ": Gradient filter without entries");
    } else
      while (xml.readNextStartElement()) {
        result.unmapped.push_back(filter.sourceId + ": Unmapped " +
                                  xml.name().toString().left(64));
        xml.skipCurrentElement();
      }
    in.reportUnknown();
    if (enabled) result.filters.push_back(filter);
  });
  return result;
}

QString blendMode(const Element &element) {
  return element.properties.value("blendMode", "normal");
}

double blurRadius(double blurX, double blurY, int passes) {
  const double variance =
      std::max(0, passes) * (boxVariance(blurX) + boxVariance(blurY)) / 2;
  if (variance <= 0) return 0;
  double low = 1, high = 2;
  while (triangleVariance(high) < variance && high < 1e5) high *= 2;
  for (int step = 0; step < 60; ++step) {
    const double mid                                = (low + high) / 2;
    (triangleVariance(mid) < variance ? low : high) = mid;
  }
  return (low + high) / 2;
}

QString blendFxType(const QString &mode) {
  static const QMap<QString, QString> types{
      {"multiply", "multFx"}, {"screen", "screenFx"}, {"lighten", "maxFx"},
      {"darken", "minFx"},    {"add", "addFx"},       {"subtract", "subFx"}};
  return types.value(mode);
}

std::array<double, 20> adjustColorMatrix(double brightness, double contrast,
                                         double saturation, double hue) {
  // fl.motion.AdjustColor: final = hue * saturation * contrast * brightness
  // (each Multiply prepends), applied to column vectors (r, g, b, a, 1).
  static const double deltaIndex[] = {
      0,    0.01, 0.02, 0.04, 0.05, 0.06, 0.07, 0.08, 0.1,  0.11, 0.12, 0.14,
      0.15, 0.16, 0.17, 0.18, 0.20, 0.21, 0.22, 0.24, 0.25, 0.27, 0.28, 0.30,
      0.32, 0.34, 0.36, 0.38, 0.40, 0.42, 0.44, 0.46, 0.48, 0.5,  0.53, 0.56,
      0.59, 0.62, 0.65, 0.68, 0.71, 0.74, 0.77, 0.80, 0.83, 0.86, 0.89, 0.92,
      0.95, 0.98, 1.0,  1.06, 1.12, 1.18, 1.24, 1.30, 1.36, 1.42, 1.48, 1.54,
      1.60, 1.66, 1.72, 1.78, 1.84, 1.90, 1.96, 2.0,  2.12, 2.25, 2.37, 2.50,
      2.62, 2.75, 2.87, 3.0,  3.2,  3.4,  3.6,  3.8,  4.0,  4.3,  4.7,  4.9,
      5.0,  5.5,  6.0,  6.5,  6.8,  7.0,  7.3,  7.5,  7.8,  8.0,  8.4,  8.7,
      9.0,  9.4,  9.6,  9.8,  10.0};
  Matrix5 b = identity5(), c = identity5(), s = identity5(), h = identity5();
  for (int row = 0; row < 3; ++row) b[row * 5 + 4] = brightness;
  {
    const double x = contrast == 0 ? 127
                     : contrast > 0
                         ? deltaIndex[std::min(100, int(contrast))] * 127 + 127
                         : contrast / 100 * 127 + 127;
    for (int row = 0; row < 3; ++row) {
      c[row * 5 + row] = x / 127;
      c[row * 5 + 4]   = 0.5 * (127 - x);
    }
  }
  {
    const double x           = saturation == 0  ? 1
                               : saturation > 0 ? 1 + 3 * saturation / 100
                                                : saturation / 100 + 1;
    const double luminance[] = {0.3086, 0.6094, 0.0820};
    for (int row = 0; row < 3; ++row)
      for (int col = 0; col < 3; ++col)
        s[row * 5 + col] = (1 - x) * luminance[col] + (row == col ? x : 0);
  }
  if (hue != 0) {
    const double angle  = hue * std::acos(-1.0) / 180;
    const double cosine = std::cos(angle), sine = std::sin(angle);
    const double lum[]         = {0.213, 0.715, 0.072};
    const double cosTerm[3][3] = {{1 - lum[0], -lum[1], -lum[2]},
                                  {-lum[0], 1 - lum[1], -lum[2]},
                                  {-lum[0], -lum[1], 1 - lum[2]}};
    const double sinTerm[3][3] = {{-lum[0], -lum[1], 1 - lum[2]},
                                  {0.143, 0.140, -0.283},
                                  {-(1 - lum[0]), lum[1], lum[2]}};
    for (int row = 0; row < 3; ++row)
      for (int col = 0; col < 3; ++col)
        h[row * 5 + col] =
            lum[col] + cosine * cosTerm[row][col] + sine * sinTerm[row][col];
  }
  const Matrix5 m = multiply(h, multiply(s, multiply(c, b)));
  std::array<double, 20> result;
  for (int index = 0; index < 20; ++index) result[index] = m[index];
  return result;
}

ColorTransform interpolate(const ColorTransform &a, const ColorTransform &b,
                           double t) {
  ColorTransform result;
  for (int channel = 0; channel < 4; ++channel) {
    result.multiplier[channel] =
        lerp(a.multiplier[channel], b.multiplier[channel], t);
    result.offset[channel] = lerp(a.offset[channel], b.offset[channel], t);
  }
  return result;
}

bool sameFilterKinds(const QVector<Filter> &a, const QVector<Filter> &b) {
  if (a.size() != b.size()) return false;
  for (int index = 0; index < a.size(); ++index)
    if (a[index].kind != b[index].kind ||
        a[index].gradient.size() != b[index].gradient.size())
      return false;
  return true;
}

Filter interpolate(const Filter &a, const Filter &b, double t) {
  Filter result     = t < 1 ? a : b;  // flags and types switch at the end key
  result.blurX      = lerp(a.blurX, b.blurX, t);
  result.blurY      = lerp(a.blurY, b.blurY, t);
  result.strength   = lerp(a.strength, b.strength, t);
  result.angle      = lerp(a.angle, b.angle, t);
  result.distance   = lerp(a.distance, b.distance, t);
  result.color      = lerp(a.color, b.color, t);
  result.highlight  = lerp(a.highlight, b.highlight, t);
  result.shadow     = lerp(a.shadow, b.shadow, t);
  result.brightness = lerp(a.brightness, b.brightness, t);
  result.contrast   = lerp(a.contrast, b.contrast, t);
  result.saturation = lerp(a.saturation, b.saturation, t);
  result.hue        = lerp(a.hue, b.hue, t);
  if (a.gradient.size() == b.gradient.size())
    for (int index = 0; index < a.gradient.size(); ++index) {
      result.gradient[index].ratio =
          lerp(a.gradient[index].ratio, b.gradient[index].ratio, t);
      result.gradient[index].color =
          lerp(a.gradient[index].color, b.gradient[index].color, t);
    }
  return result;
}

}  // namespace Flash
