#include "tflashgradientstyle.h"

#include "drawutil.h"
#include "tcolorfunctions.h"
#include "texception.h"
#include "tgl.h"
#include "tregionoutline.h"
#include "tpixelutils.h"
#include "trop.h"
#include "tstencilcontrol.h"
#include "tstrokeoutline.h"
#include "ttessellator.h"
#include "tvectorimage.h"

#include <QCoreApplication>
#include <QStringList>

#include <algorithm>
#include <cmath>
#include <functional>
#include <iomanip>
#include <limits>
#include <locale>
#include <memory>
#include <sstream>

namespace {

constexpr int kPayloadVersion = 1;
constexpr int kMaxStops       = 1024;
// Render textures stay within what every OpenGL 1.1 implementation accepts
// (the Windows GDI fallback used by offline rendering has a 1024 limit) and
// a bounded CPU budget. Gradients are smooth, so a larger region simply gets
// coarser texels with linear filtering instead of failing to draw.
constexpr int kMaxTextureEdge = 1024;
constexpr int kStopTableSize  = 4096;

bool finiteAffine(const TAffine &a) {
  const double v[] = {a.a11, a.a12, a.a13, a.a21, a.a22, a.a23};
  for (double item : v)
    if (!std::isfinite(item)) return false;
  const double scale = std::max(
      {std::abs(a.a11), std::abs(a.a12), std::abs(a.a21), std::abs(a.a22)});
  return scale > 0 && std::abs(a.det()) > 1e-12 * scale * scale;
}

unsigned char quantizeByte(double value) {
  return static_cast<unsigned char>(
      std::lround(std::max(0.0, std::min(255.0, value))));
}

double toLinear(double channel) {
  channel /= 255.0;
  return channel <= 0.04045 ? channel / 12.92
                            : std::pow((channel + 0.055) / 1.055, 2.4);
}

double fromLinear(double channel) {
  return 255.0 * (channel <= 0.0031308
                      ? 12.92 * channel
                      : 1.055 * std::pow(channel, 1.0 / 2.4) - 0.055);
}

double spreadRatio(double ratio, TFlashGradientStyle::Spread spread) {
  if (spread == TFlashGradientStyle::Spread::Pad)
    return std::max(0.0, std::min(1.0, ratio));
  const double period =
      spread == TFlashGradientStyle::Spread::Repeat ? 1.0 : 2.0;
  double wrapped = std::fmod(ratio, period);
  if (wrapped < 0.0) wrapped += period;
  return spread == TFlashGradientStyle::Spread::Reflect && wrapped > 1.0
             ? 2.0 - wrapped
             : wrapped;
}

int powerOfTwo(int size) {
  int result = 1;
  while (result < size && result < 1 << 15) result *= 2;
  return result;
}

// Straight RGBA at a spread ratio in [0, 1], from the ordered stops.
TPixel32 stopColor(const std::vector<TFlashGradientStyle::Stop> &stops,
                   TFlashGradientStyle::Interpolation interpolation,
                   double ratio) {
  const auto next = std::upper_bound(
      stops.begin(), stops.end(), ratio,
      [](double position, const TFlashGradientStyle::Stop &stop) {
        return position < stop.ratio;
      });
  if (next == stops.begin()) {
    const auto &stop = stops.front();
    return TPixel32(stop.color.r, stop.color.g, stop.color.b,
                    quantizeByte(stop.alpha * 255.0));
  }
  if (next == stops.end()) {
    const auto &stop = stops.back();
    return TPixel32(stop.color.r, stop.color.g, stop.color.b,
                    quantizeByte(stop.alpha * 255.0));
  }
  const auto &left    = *(next - 1);
  const auto &right   = *next;
  const double weight = (ratio - left.ratio) / (right.ratio - left.ratio);
  const auto channel  = [&](unsigned char l, unsigned char r) {
    if (interpolation == TFlashGradientStyle::Interpolation::Rgb)
      return quantizeByte(l + weight * (r - l));
    return quantizeByte(
         fromLinear(toLinear(l) + weight * (toLinear(r) - toLinear(l))));
  };
  return TPixel32(
      channel(left.color.r, right.color.r),
      channel(left.color.g, right.color.g),
      channel(left.color.b, right.color.b),
      quantizeByte((left.alpha + weight * (right.alpha - left.alpha)) * 255.0));
}

// Position along the gradient (before spread) of a gradient-space point.
double fieldRatio(const TPointD &q, TFlashGradientStyle::Kind kind,
                  double focal) {
  if (kind != TFlashGradientStyle::Kind::Radial) return (q.x + 1.0) * 0.5;
  const double dx = q.x - focal;
  const double dy = q.y;
  const double fd = focal * dx;
  const double d2 = dx * dx + dy * dy;
  const double radial =
      std::sqrt(std::max(0.0, fd * fd + (1.0 - focal * focal) * d2));
  return (fd + radial) / (1.0 - focal * focal);
}

// A field texture covering `bounds` in drawing-local coordinates; each texel
// holds the field at the center of its cell.
struct FieldTexture {
  TRaster32P raster;
  TRectD bounds;
};

// Evaluates the style over `area` at about one texel per device pixel
// (`pixelSize` local units), clamped to the texture limits above. Texels are
// straight (non-premultiplied) RGBA, like the colors of solid styles, so the
// caller's blending applies to them exactly as it does to a solid color.
bool makeFieldTexture(const TFlashGradientStyle &style,
                      const TColorFunction *cf, const TRectD &area,
                      double pixelSize, FieldTexture &result) {
  if (!std::isfinite(pixelSize) || pixelSize <= 0.0 ||
      !std::isfinite(area.x0) || !std::isfinite(area.y0) ||
      !std::isfinite(area.x1) || !std::isfinite(area.y1) || area.x1 < area.x0 ||
      area.y1 < area.y0)
    return false;
  GLint maxTextureSize = 0;
  glGetIntegerv(GL_MAX_TEXTURE_SIZE, &maxTextureSize);
  const int limit = std::max(
      64,
      std::min<int>(kMaxTextureEdge, maxTextureSize > 0 ? maxTextureSize : 64));
  // Two margin texels on every side keep linear filtering inside the field.
  const double lx       = std::max(area.getLx(), pixelSize);
  const double ly       = std::max(area.getLy(), pixelSize);
  const double wanted[] = {std::ceil(lx / pixelSize) + 4.0,
                           std::ceil(ly / pixelSize) + 4.0};
  if (!std::isfinite(wanted[0]) || !std::isfinite(wanted[1])) return false;
  const int width = powerOfTwo(int(std::min<double>(wanted[0], double(limit))));
  const int height =
      powerOfTwo(int(std::min<double>(wanted[1], double(limit))));
  if (width < 8 || height < 8) return false;
  const double stepX = lx / (width - 4);
  const double stepY = ly / (height - 4);
  result.bounds =
      TRectD(area.x0 - 2.0 * stepX, area.y0 - 2.0 * stepY,
             area.x0 + (width - 2) * stepX, area.y0 + (height - 2) * stepY);

  // The spread ratio is tabulated; the error is far below 8-bit output.
  std::vector<TPixel32> table(kStopTableSize);
  for (int index = 0; index < kStopTableSize; ++index) {
    const TPixel32 color = stopColor(style.stops(), style.interpolation(),
                                     double(index) / (kStopTableSize - 1));
    table[index]         = cf ? (*cf)(color) : color;
  }
  const TAffine toField                    = style.gradientToLocal().inv();
  const TFlashGradientStyle::Kind kind     = style.kind();
  const TFlashGradientStyle::Spread spread = style.spread();
  const double focal                       = style.focalRatio();

  result.raster = TRaster32P(width, height);
  result.raster->lock();
  for (int y = 0; y < height; ++y) {
    TPixel32 *row = result.raster->pixels(y);
    const TPointD start(result.bounds.x0 + 0.5 * stepX,
                        result.bounds.y0 + (y + 0.5) * stepY);
    TPointD q = toField * start;
    const TPointD advance(toField.a11 * stepX, toField.a21 * stepX);
    for (int x = 0; x < width; ++x, q += advance) {
      double ratio = spreadRatio(fieldRatio(q, kind, focal), spread);
      if (!std::isfinite(ratio)) ratio = 0.0;
      row[x] = table[int(ratio * (kStopTableSize - 1) + 0.5)];
    }
  }
  result.raster->unlock();
  return true;
}

// Binds a field texture so that object coordinates select its texels, and
// restores the previous texture state on destruction. Blending is left as
// the caller (the vector renderer) configured it for its current pass.
class FieldTextureBinding {
  GLuint m_id = 0;

public:
  explicit FieldTextureBinding(const FieldTexture &texture) {
    glPushAttrib(GL_ENABLE_BIT | GL_TEXTURE_BIT | GL_CURRENT_BIT);
    const TRectD &bounds    = texture.bounds;
    const GLdouble sPlane[] = {1.0 / bounds.getLx(), 0.0, 0.0,
                               -bounds.x0 / bounds.getLx()};
    const GLdouble tPlane[] = {0.0, 1.0 / bounds.getLy(), 0.0,
                               -bounds.y0 / bounds.getLy()};
    glEnable(GL_TEXTURE_2D);
    glEnable(GL_TEXTURE_GEN_S);
    glEnable(GL_TEXTURE_GEN_T);
    glTexGeni(GL_S, GL_TEXTURE_GEN_MODE, GL_OBJECT_LINEAR);
    glTexGeni(GL_T, GL_TEXTURE_GEN_MODE, GL_OBJECT_LINEAR);
    glTexGendv(GL_S, GL_OBJECT_PLANE, sPlane);
    glTexGendv(GL_T, GL_OBJECT_PLANE, tPlane);

    TextureInfoForGL info;
    texture.raster->lock();
    TRasterP prepared = prepareTexture(texture.raster, info);
    if (prepared != texture.raster) prepared->lock();
    glGenTextures(1, &m_id);
    glBindTexture(GL_TEXTURE_2D, m_id);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
    glTexEnvi(GL_TEXTURE_ENV, GL_TEXTURE_ENV_MODE, GL_MODULATE);
    glPixelStorei(GL_UNPACK_ROW_LENGTH, 0);
    glTexImage2D(GL_TEXTURE_2D, 0, info.internalformat, info.width, info.height,
                 0, info.type, info.format, prepared->getRawData());
    texture.raster->unlock();
    if (prepared != texture.raster) prepared->unlock();
    // The texel is the fragment color.
    glColor4d(1.0, 1.0, 1.0, 1.0);
  }
  ~FieldTextureBinding() {
    glDeleteTextures(1, &m_id);
    glPopAttrib();
  }
  FieldTextureBinding(const FieldTextureBinding &)            = delete;
  FieldTextureBinding &operator=(const FieldTextureBinding &) = delete;
};

void drawOutlineSides(const std::vector<TOutlinePoint> &points) {
  // Even entries run along one side of the stroke, odd entries the other.
  for (size_t side = 0; side < 2 && side < points.size(); ++side) {
    glBegin(GL_LINE_STRIP);
    for (size_t index = side; index < points.size(); index += 2)
      glVertex2d(points[index].x, points[index].y);
    glEnd();
  }
}

// PLI encodes ordinary style doubles on a 1/65536 fractional grid. A new
// gradient payload must preserve authored matrix, stop and alpha decimals, so
// encode its doubles as bounded, locale-independent style strings instead.
std::string exactDouble(double value) {
  std::ostringstream out;
  out.imbue(std::locale::classic());
  out << std::setprecision(std::numeric_limits<double>::max_digits10) << value;
  return out.str();
}

double parseExactDouble(const std::string &value) {
  if (value.empty() || value.size() > 32)
    throw TException("Invalid Flash gradient decimal payload");
  std::istringstream in(value);
  in.imbue(std::locale::classic());
  double parsed = 0.0;
  char trailing = 0;
  if (!(in >> parsed) || (in >> trailing) || !std::isfinite(parsed))
    throw TException("Invalid Flash gradient decimal payload");
  return parsed;
}

double readExactDouble(TInputStreamInterface &is) {
  std::string value;
  is >> value;
  return parseExactDouble(value);
}

TColorStyle::Declaration declaration(new TFlashGradientStyle());

}  // namespace

TFlashGradientStyle::TFlashGradientStyle()
    : TSolidColorStyle(TPixel32::Black)
    , m_kind(Kind::Linear)
    , m_spread(Spread::Pad)
    , m_interpolation(Interpolation::Rgb)
    , m_gradientToLocal(50.0, 0.0, 0.0, 0.0, 50.0, 0.0)
    , m_localToGradient(m_gradientToLocal.inv())
    , m_focalRatio(0.0)
    , m_stops({{0.0, TPixel32::Black, 1.0}, {1.0, TPixel32::White, 1.0}}) {}

bool TFlashGradientStyle::setGradient(Kind kind, const TAffine &gradientToLocal,
                                      const std::vector<Stop> &stops,
                                      Spread spread,
                                      Interpolation interpolation,
                                      double focalRatio) {
  if ((kind != Kind::Linear && kind != Kind::Radial) ||
      (spread != Spread::Pad && spread != Spread::Reflect &&
       spread != Spread::Repeat) ||
      (interpolation != Interpolation::Rgb &&
       interpolation != Interpolation::LinearRgb) ||
      !finiteAffine(gradientToLocal) || !std::isfinite(focalRatio) ||
      std::abs(focalRatio) >= 1.0 ||
      (kind == Kind::Linear && focalRatio != 0.0) || stops.size() < 2 ||
      stops.size() > kMaxStops)
    return false;
  double previous = -1.0;
  for (const Stop &stop : stops) {
    if (!std::isfinite(stop.ratio) || stop.ratio < previous ||
        stop.ratio < 0.0 || stop.ratio > 1.0 || !std::isfinite(stop.alpha) ||
        stop.alpha < 0.0 || stop.alpha > 1.0)
      return false;
    previous = stop.ratio;
  }
  m_kind            = kind;
  m_spread          = spread;
  m_interpolation   = interpolation;
  m_gradientToLocal = gradientToLocal;
  m_localToGradient = gradientToLocal.inv();
  m_focalRatio      = focalRatio;
  m_stops           = stops;
  for (Stop &stop : m_stops) stop.color.m = quantizeByte(stop.alpha * 255.0);
  invalidateIcon();
  updateVersionNumber();
  return true;
}

TPixel32 TFlashGradientStyle::sample(const TPointD &localPoint) const {
  return sampleField(m_localToGradient * localPoint);
}

TPixel32 TFlashGradientStyle::sampleField(const TPointD &q) const {
  const double ratio =
      spreadRatio(fieldRatio(q, m_kind, m_focalRatio), m_spread);
  return stopColor(m_stops, m_interpolation, std::isfinite(ratio) ? ratio : 0);
}

TColorStyle *TFlashGradientStyle::clone() const {
  return new TFlashGradientStyle(*this);
}

QString TFlashGradientStyle::getDescription() const {
  return QCoreApplication::translate("TFlashGradientStyle", "Gradient Fill");
}

TPixel32 TFlashGradientStyle::getMainColor() const {
  return m_stops.front().color;
}

void TFlashGradientStyle::setMainColor(const TPixel32 &color) {
  setColorParamValue(0, color);
}

TPixel32 TFlashGradientStyle::getAverageColor() const {
  double r = 0, g = 0, b = 0, m = 0;
  for (const Stop &stop : m_stops) {
    r += stop.color.r;
    g += stop.color.g;
    b += stop.color.b;
    m += stop.color.m;
  }
  const double n = double(m_stops.size());
  return TPixel32(quantizeByte(r / n), quantizeByte(g / n), quantizeByte(b / n),
                  quantizeByte(m / n));
}

int TFlashGradientStyle::getColorParamCount() const {
  return static_cast<int>(m_stops.size());
}

TPixel32 TFlashGradientStyle::getColorParamValue(int index) const {
  return m_stops.at(index).color;
}

void TFlashGradientStyle::setColorParamValue(int index, const TPixel32 &color) {
  Stop &stop = m_stops.at(index);
  stop.color = color;
  stop.alpha = color.m / 255.0;
  invalidateIcon();
  updateVersionNumber();
}

int TFlashGradientStyle::getParamCount() const {
  return 10 + 2 * static_cast<int>(m_stops.size());
}

TColorStyle::ParamType TFlashGradientStyle::getParamType(int index) const {
  return index < 3 ? ENUM : DOUBLE;
}

QString TFlashGradientStyle::getParamNames(int index) const {
  static const char *names[] = {
      "Kind",     "Spread",    "Interpolation", "Focal Ratio", "Matrix a",
      "Matrix c", "Matrix tx", "Matrix b",      "Matrix d",    "Matrix ty"};
  if (index < 10) return QString::fromLatin1(names[index]);
  return QString("Stop %1 %2")
      .arg((index - 10) / 2 + 1)
      .arg(index % 2 == 0 ? "Ratio" : "Alpha");
}

void TFlashGradientStyle::getParamRange(int index, double &min,
                                        double &max) const {
  if (index == 3) {
    min = -0.999999;
    max = 0.999999;
  } else if (index >= 10) {
    min = 0.0;
    max = 1.0;
  } else {
    min = -1000000000.0;
    max = 1000000000.0;
  }
}

void TFlashGradientStyle::getParamRange(int index, int &min, int &max) const {
  min = 0;
  max = index == 0 ? 1 : 2;
  if (index == 2) max = 1;
}

void TFlashGradientStyle::getParamRange(int index, QStringList &items) const {
  if (index == 0)
    items << "Linear"
          << "Radial";
  else if (index == 1)
    items << "Pad"
          << "Reflect"
          << "Repeat";
  else if (index == 2)
    items << "RGB"
          << "Linear RGB";
}

double TFlashGradientStyle::getParamValue(double_tag, int index) const {
  const double matrix[] = {m_gradientToLocal.a11, m_gradientToLocal.a12,
                           m_gradientToLocal.a13, m_gradientToLocal.a21,
                           m_gradientToLocal.a22, m_gradientToLocal.a23};
  if (index == 3) return m_focalRatio;
  if (index < 10) return matrix[index - 4];
  const Stop &stop = m_stops.at((index - 10) / 2);
  return index % 2 == 0 ? stop.ratio : stop.alpha;
}

int TFlashGradientStyle::getParamValue(int_tag, int index) const {
  if (index == 0) return static_cast<int>(m_kind);
  if (index == 1) return static_cast<int>(m_spread);
  return static_cast<int>(m_interpolation);
}

void TFlashGradientStyle::setParamValue(int index, double value) {
  if (!std::isfinite(value)) return;
  TAffine matrix       = m_gradientToLocal;
  std::vector<Stop> ss = m_stops;
  double focal         = m_focalRatio;
  if (index == 3)
    focal = value;
  else if (index >= 4 && index < 10) {
    double *parts[]   = {&matrix.a11, &matrix.a12, &matrix.a13,
                         &matrix.a21, &matrix.a22, &matrix.a23};
    *parts[index - 4] = value;
  } else if (index >= 10) {
    Stop &stop = ss.at((index - 10) / 2);
    if (index % 2 == 0)
      stop.ratio = value;
    else
      stop.alpha = value;
  } else
    return;
  setGradient(m_kind, matrix, ss, m_spread, m_interpolation, focal);
}

void TFlashGradientStyle::setParamValue(int index, int value) {
  if (index == 0)
    setGradient(static_cast<Kind>(value), m_gradientToLocal, m_stops, m_spread,
                m_interpolation, 0.0);
  else if (index == 1)
    setGradient(m_kind, m_gradientToLocal, m_stops, static_cast<Spread>(value),
                m_interpolation, m_focalRatio);
  else if (index == 2)
    setGradient(m_kind, m_gradientToLocal, m_stops, m_spread,
                static_cast<Interpolation>(value), m_focalRatio);
}

void TFlashGradientStyle::saveData(TOutputStreamInterface &os) const {
  os << kPayloadVersion << static_cast<int>(m_kind)
     << static_cast<int>(m_spread) << static_cast<int>(m_interpolation)
     << exactDouble(m_gradientToLocal.a11) << exactDouble(m_gradientToLocal.a12)
     << exactDouble(m_gradientToLocal.a13) << exactDouble(m_gradientToLocal.a21)
     << exactDouble(m_gradientToLocal.a22) << exactDouble(m_gradientToLocal.a23)
     << exactDouble(m_focalRatio) << static_cast<int>(m_stops.size());
  for (const Stop &stop : m_stops)
    os << exactDouble(stop.ratio) << stop.color << exactDouble(stop.alpha);
}

void TFlashGradientStyle::loadData(TInputStreamInterface &is) {
  int version = 0, kind = 0, spread = 0, interpolation = 0, count = 0;
  TAffine matrix;
  double focal = 0.0;
  is >> version >> kind >> spread >> interpolation;
  matrix.a11 = readExactDouble(is);
  matrix.a12 = readExactDouble(is);
  matrix.a13 = readExactDouble(is);
  matrix.a21 = readExactDouble(is);
  matrix.a22 = readExactDouble(is);
  matrix.a23 = readExactDouble(is);
  focal      = readExactDouble(is);
  is >> count;
  if (version != kPayloadVersion || count < 2 || count > kMaxStops)
    throw TException("Unsupported Flash gradient style payload");
  std::vector<Stop> stops(count);
  for (Stop &stop : stops) {
    stop.ratio = readExactDouble(is);
    is >> stop.color;
    stop.alpha = readExactDouble(is);
  }
  if (!setGradient(static_cast<Kind>(kind), matrix, stops,
                   static_cast<Spread>(spread),
                   static_cast<Interpolation>(interpolation), focal))
    throw TException("Invalid Flash gradient style payload");
}

void TFlashGradientStyle::drawRegion(const TColorFunction *cf,
                                     bool antiAliasing,
                                     TRegionOutline &boundary) const {
  TglTessellator tessellator;
  FieldTexture texture;
  if (!makeFieldTexture(*this, cf, boundary.m_bbox,
                        std::sqrt(tglGetPixelSize2()), texture)) {
    // Never fail a render: an unusable view draws the mean stop color.
    tessellator.tessellate(cf, antiAliasing, boundary, getAverageColor());
    return;
  }
  FieldTextureBinding binding(texture);
  // White modulated by the texture: each fragment takes the field's color.
  tessellator.tessellate(nullptr, antiAliasing, boundary, TPixel32::White);
}

// Line smoothing of the outline sides is configured by the vector renderer,
// so the antialiased and aliased variants share one implementation.
void TFlashGradientStyle::drawStroke(const TColorFunction *cf,
                                     TStrokeOutline *outline,
                                     const TStroke *) const {
  drawFieldStroke(cf, outline);
}

void TFlashGradientStyle::drawAliasedStroke(const TColorFunction *cf,
                                            TStrokeOutline *outline,
                                            const TStroke *) const {
  drawFieldStroke(cf, outline);
}

void TFlashGradientStyle::drawFieldStroke(const TColorFunction *cf,
                                          TStrokeOutline *outline) const {
  if (!outline) return;
  const std::vector<TOutlinePoint> &points = outline->getArray();
  if (points.size() < 2) return;
  TRectD area(points.front().x, points.front().y, points.front().x,
              points.front().y);
  for (const TOutlinePoint &point : points) {
    area.x0 = std::min(area.x0, point.x);
    area.y0 = std::min(area.y0, point.y);
    area.x1 = std::max(area.x1, point.x);
    area.y1 = std::max(area.y1, point.y);
  }
  FieldTexture texture;
  std::unique_ptr<FieldTextureBinding> binding;
  if (makeFieldTexture(*this, cf, area, std::sqrt(tglGetPixelSize2()), texture))
    binding.reset(new FieldTextureBinding(texture));
  else
    tglColor(cf ? (*cf)(getAverageColor()) : getAverageColor());

  auto fill = [&] {
    glBegin(GL_QUAD_STRIP);
    for (const TOutlinePoint &point : points) glVertex2d(point.x, point.y);
    glEnd();
  };
  bool translucent = cf != nullptr;
  for (const Stop &stop : m_stops)
    translucent = translucent || stop.color.m < 255;
  if (translucent) {
    // As for translucent solid strokes: overlapping outline quads must not
    // accumulate, so each pixel is painted once.
    TStencilControl *stencil = TStencilControl::instance();
    stencil->beginMask(TStencilControl::DRAW_ON_SCREEN_ONLY_ONCE);
    fill();
    stencil->endMask();
    stencil->enableMask(TStencilControl::SHOW_OUTSIDE);
    drawOutlineSides(points);
    stencil->disableMask();
  } else {
    drawOutlineSides(points);
    fill();
  }
}

void TFlashGradientStyle::makeIcon(const TDimension &size) {
  // The authored field lives in drawing coordinates and would usually fall
  // outside an icon; show it in its own gradient space instead.
  if (!m_icon || m_icon->getSize() != size) m_icon = TRaster32P(size);
  TRaster32P field(size);
  field->lock();
  for (int y = 0; y < size.ly; ++y) {
    TPixel32 *row = field->pixels(y);
    for (int x = 0; x < size.lx; ++x)
      row[x] = premultiply(sampleField(TPointD(
          -1.0 + 2.0 * (x + 0.5) / size.lx, -1.0 + 2.0 * (y + 0.5) / size.ly)));
  }
  field->unlock();
  TRop::checkBoard(m_icon, TPixel32::Black, TPixel32::White, TDimensionD(6, 6),
                   TPointD());
  TRop::over(m_icon, field);
}

//=============================================================================
// TFlashBitmapStyle
//=============================================================================

namespace {

constexpr int kBitmapPayloadVersion = 1;
// Large or deeply zoomed fills are drawn in several textures; past this many
// the texels simply get coarser.
constexpr int kMaxBitmapTiles = 64;

bool invertibleAffine(const TAffine &a) {
  const double v[] = {a.a11, a.a12, a.a13, a.a21, a.a22, a.a23};
  for (double item : v)
    if (!std::isfinite(item)) return false;
  const double det = a.det();
  return det != 0.0 && std::isfinite(det) && std::isfinite(1.0 / det);
}

// The part of `area` (local coordinates) inside the current viewport, for
// the affine (orthographic) views vector images are drawn with; the whole
// area when the view is not affine.
TRectD visibleArea(const TRectD &area) {
  GLdouble model[16], projection[16];
  GLint viewport[4];
  glGetDoublev(GL_MODELVIEW_MATRIX, model);
  glGetDoublev(GL_PROJECTION_MATRIX, projection);
  glGetIntegerv(GL_VIEWPORT, viewport);
  double c[16];  // column-major projection * modelview
  for (int col = 0; col < 4; ++col)
    for (int row = 0; row < 4; ++row) {
      double sum = 0.0;
      for (int k = 0; k < 4; ++k)
        sum += projection[k * 4 + row] * model[col * 4 + k];
      c[col * 4 + row] = sum;
    }
  if (c[3] != 0.0 || c[7] != 0.0 || c[15] == 0.0 || viewport[2] <= 0 ||
      viewport[3] <= 0)
    return area;
  // Normalized device coordinates of a local point.
  const TAffine toDevice(c[0] / c[15], c[4] / c[15], c[12] / c[15],
                         c[1] / c[15], c[5] / c[15], c[13] / c[15]);
  if (!invertibleAffine(toDevice)) return area;
  const TAffine toLocal = toDevice.inv();
  TRectD visible;
  bool first = true;
  for (double x : {-1.0, 1.0})
    for (double y : {-1.0, 1.0}) {
      const TPointD p = toLocal * TPointD(x, y);
      if (!std::isfinite(p.x) || !std::isfinite(p.y)) return area;
      if (first)
        visible = TRectD(p, p);
      else
        visible += TRectD(p, p);
      first = false;
    }
  return area * visible;
}

// Evaluates the bitmap fill over `area` like makeFieldTexture: texel centers
// sampled from the bitmap, straight RGBA, `limit` texels per side at most.
bool makeBitmapTexture(const TFlashBitmapStyle &style, const TColorFunction *cf,
                       const TRectD &area, double pixelSize, int limit,
                       FieldTexture &result) {
  if (!std::isfinite(pixelSize) || pixelSize <= 0.0 ||
      !std::isfinite(area.x0) || !std::isfinite(area.y0) ||
      !std::isfinite(area.x1) || !std::isfinite(area.y1) || area.x1 < area.x0 ||
      area.y1 < area.y0)
    return false;
  const double lx       = std::max(area.getLx(), pixelSize);
  const double ly       = std::max(area.getLy(), pixelSize);
  const double wanted[] = {std::ceil(lx / pixelSize) + 4.0,
                           std::ceil(ly / pixelSize) + 4.0};
  if (!std::isfinite(wanted[0]) || !std::isfinite(wanted[1])) return false;
  const int width = powerOfTwo(int(std::min<double>(wanted[0], double(limit))));
  const int height =
      powerOfTwo(int(std::min<double>(wanted[1], double(limit))));
  if (width < 8 || height < 8) return false;
  const double stepX = lx / (width - 4);
  const double stepY = ly / (height - 4);
  result.bounds =
      TRectD(area.x0 - 2.0 * stepX, area.y0 - 2.0 * stepY,
             area.x0 + (width - 2) * stepX, area.y0 + (height - 2) * stepY);
  const TAffine toBitmap = style.bitmapToLocal().inv();
  result.raster          = TRaster32P(width, height);
  result.raster->lock();
  for (int y = 0; y < height; ++y) {
    TPixel32 *row = result.raster->pixels(y);
    for (int x = 0; x < width; ++x) {
      const TPointD local(result.bounds.x0 + (x + 0.5) * stepX,
                          result.bounds.y0 + (y + 0.5) * stepY);
      const TPixel32 color = style.sampleBitmap(toBitmap * local);
      row[x]               = cf ? (*cf)(color) : color;
    }
  }
  result.raster->unlock();
  return true;
}

// Draws `draw` (geometry in local coordinates) textured with the bitmap fill
// over `bounds`: at about one texel per device pixel over the visible part,
// in tiles clipped by object-space clip planes when one texture is too
// small. Returns false when no texture could be made.
bool drawBitmapTiles(const TFlashBitmapStyle &style, const TColorFunction *cf,
                     const TRectD &bounds, const std::function<void()> &draw) {
  const TRectD area = visibleArea(bounds);
  if (area.x1 <= area.x0 || area.y1 <= area.y0)
    return true;  // nothing of it is visible
  GLint maxTextureSize = 0;
  glGetIntegerv(GL_MAX_TEXTURE_SIZE, &maxTextureSize);
  const int limit = std::max(
      64,
      std::min<int>(kMaxTextureEdge, maxTextureSize > 0 ? maxTextureSize : 64));
  // A power-of-two texture of `limit` texels has limit - 4 inner texels.
  const int inner = limit - 4;
  double pixel    = std::sqrt(tglGetPixelSize2());
  if (!std::isfinite(pixel) || pixel <= 0.0) return false;
  int nx = 1, ny = 1;
  for (int attempt = 0; attempt < 64; ++attempt, pixel *= 2.0) {
    nx = std::max(1, int(std::ceil(area.getLx() / (pixel * inner))));
    ny = std::max(1, int(std::ceil(area.getLy() / (pixel * inner))));
    if (nx * ny <= kMaxBitmapTiles) break;
  }
  if (nx * ny > kMaxBitmapTiles) return false;
  if (nx * ny == 1) {
    FieldTexture texture;
    if (!makeBitmapTexture(style, cf, area, pixel, limit, texture))
      return false;
    FieldTextureBinding binding(texture);
    draw();
    return true;
  }
  glPushAttrib(GL_TRANSFORM_BIT | GL_ENABLE_BIT);
  const double tileX = area.getLx() / nx, tileY = area.getLy() / ny;
  bool drawn = true;
  for (int j = 0; j < ny && drawn; ++j)
    for (int i = 0; i < nx && drawn; ++i) {
      const TRectD tile(area.x0 + i * tileX, area.y0 + j * tileY,
                        i + 1 == nx ? area.x1 : area.x0 + (i + 1) * tileX,
                        j + 1 == ny ? area.y1 : area.y0 + (j + 1) * tileY);
      FieldTexture texture;
      if (!makeBitmapTexture(style, cf, tile, pixel, limit, texture)) {
        drawn = false;
        break;
      }
      const GLdouble planes[4][4] = {{1.0, 0.0, 0.0, -tile.x0},
                                     {-1.0, 0.0, 0.0, tile.x1},
                                     {0.0, 1.0, 0.0, -tile.y0},
                                     {0.0, -1.0, 0.0, tile.y1}};
      for (int plane = 0; plane < 4; ++plane) {
        glClipPlane(GL_CLIP_PLANE0 + plane, planes[plane]);
        glEnable(GL_CLIP_PLANE0 + plane);
      }
      FieldTextureBinding binding(texture);
      draw();
    }
  glPopAttrib();
  return drawn;
}

TColorStyle::Declaration bitmapDeclaration(new TFlashBitmapStyle());

}  // namespace

TFlashBitmapStyle::TFlashBitmapStyle()
    : TSolidColorStyle(TPixel32::Black)
    , m_bitmapToLocal()
    , m_localToBitmap()
    , m_repeat(true)
    , m_smooth(false)
    , m_average(TPixel32::Black) {}

bool TFlashBitmapStyle::setBitmap(const TRaster32P &bitmap,
                                  const TAffine &bitmapToLocal, bool repeat,
                                  bool smooth, const std::string &name) {
  if (!bitmap || bitmap->getLx() <= 0 || bitmap->getLy() <= 0 ||
      bitmap->getLx() > 65535 || bitmap->getLy() > 65535 ||
      !invertibleAffine(bitmapToLocal) ||
      !invertibleAffine(bitmapToLocal.inv()) || name.size() > 4096)
    return false;
  // Keep a compact copy (the PLI codec writes rows without padding).
  TRaster32P copy(bitmap->getLx(), bitmap->getLy());
  copy->copy(bitmap);
  double r = 0, g = 0, b = 0, m = 0;
  copy->lock();
  for (int y = 0; y < copy->getLy(); ++y) {
    const TPixel32 *row = copy->pixels(y);
    for (int x = 0; x < copy->getLx(); ++x) {
      // Weight colors by coverage: the mean of the premultiplied pixels.
      r += row[x].r * (row[x].m / 255.0);
      g += row[x].g * (row[x].m / 255.0);
      b += row[x].b * (row[x].m / 255.0);
      m += row[x].m;
    }
  }
  copy->unlock();
  const double n     = double(copy->getLx()) * copy->getLy();
  const double cover = m / 255.0;
  m_average          = cover > 0
                           ? TPixel32(quantizeByte(r / cover), quantizeByte(g / cover),
                                      quantizeByte(b / cover), quantizeByte(m / n))
                           : TPixel32::Transparent;
  m_bitmap           = copy;
  m_bitmapToLocal    = bitmapToLocal;
  m_localToBitmap    = bitmapToLocal.inv();
  m_repeat           = repeat;
  m_smooth           = smooth;
  m_name             = name;
  invalidateIcon();
  updateVersionNumber();
  return true;
}

TPixel32 TFlashBitmapStyle::sample(const TPointD &localPoint) const {
  return sampleBitmap(m_localToBitmap * localPoint);
}

TPixel32 TFlashBitmapStyle::sampleBitmap(const TPointD &q) const {
  if (!m_bitmap || !std::isfinite(q.x) || !std::isfinite(q.y))
    return TPixel32::Transparent;
  const int w = m_bitmap->getLx(), h = m_bitmap->getLy();
  // The texel covering coordinate v: wrapped, or clamped to the edge texel
  // as Flash clamps clipped bitmap fills.
  auto index = [&](double v, int size) {
    if (m_repeat) {
      v = std::fmod(v, double(size));
      if (v < 0) v += size;
    } else
      v = std::max(0.0, std::min(double(size) - 0.5, v));
    return std::min(size - 1, std::max(0, int(std::floor(v))));
  };
  if (!m_smooth) return m_bitmap->pixels(index(q.y, h))[index(q.x, w)];
  // Bilinear between texel centers, on premultiplied colors.
  const double x = q.x - 0.5, y = q.y - 0.5;
  const double fx = x - std::floor(x), fy = y - std::floor(y);
  const int x0           = index(std::floor(x) + 0.5, w);
  const int x1           = index(std::floor(x) + 1.5, w);
  const int y0           = index(std::floor(y) + 0.5, h);
  const int y1           = index(std::floor(y) + 1.5, h);
  const TPixel32 p[4]    = {m_bitmap->pixels(y0)[x0], m_bitmap->pixels(y0)[x1],
                            m_bitmap->pixels(y1)[x0], m_bitmap->pixels(y1)[x1]};
  const double weight[4] = {(1 - fx) * (1 - fy), fx * (1 - fy), (1 - fx) * fy,
                            fx * fy};
  double r = 0, g = 0, b = 0, m = 0;
  for (int i = 0; i < 4; ++i) {
    const double alpha = p[i].m / 255.0;
    r += weight[i] * p[i].r * alpha;
    g += weight[i] * p[i].g * alpha;
    b += weight[i] * p[i].b * alpha;
    m += weight[i] * p[i].m;
  }
  if (m <= 0.0) return TPixel32::Transparent;
  const double cover = m / 255.0;
  return TPixel32(quantizeByte(r / cover), quantizeByte(g / cover),
                  quantizeByte(b / cover), quantizeByte(m));
}

TColorStyle *TFlashBitmapStyle::clone() const {
  return new TFlashBitmapStyle(*this);
}

QString TFlashBitmapStyle::getDescription() const {
  return QCoreApplication::translate("TFlashBitmapStyle", "Bitmap Fill");
}

int TFlashBitmapStyle::getParamCount() const { return 8; }

TColorStyle::ParamType TFlashBitmapStyle::getParamType(int index) const {
  return index < 2 ? ENUM : DOUBLE;
}

QString TFlashBitmapStyle::getParamNames(int index) const {
  static const char *names[] = {"Edges",    "Smoothing", "Matrix a",
                                "Matrix c", "Matrix tx", "Matrix b",
                                "Matrix d", "Matrix ty"};
  return index >= 0 && index < 8 ? QString::fromLatin1(names[index])
                                 : QString();
}

void TFlashBitmapStyle::getParamRange(int, double &min, double &max) const {
  min = -1000000000.0;
  max = 1000000000.0;
}

void TFlashBitmapStyle::getParamRange(int, int &min, int &max) const {
  min = 0;
  max = 1;
}

void TFlashBitmapStyle::getParamRange(int index, QStringList &items) const {
  if (index == 0)
    items << "Clamp"
          << "Repeat";
  else if (index == 1)
    items << "Off"
          << "On";
}

double TFlashBitmapStyle::getParamValue(double_tag, int index) const {
  const double matrix[] = {m_bitmapToLocal.a11, m_bitmapToLocal.a12,
                           m_bitmapToLocal.a13, m_bitmapToLocal.a21,
                           m_bitmapToLocal.a22, m_bitmapToLocal.a23};
  return index >= 2 && index < 8 ? matrix[index - 2] : 0.0;
}

int TFlashBitmapStyle::getParamValue(int_tag, int index) const {
  return index == 0 ? int(m_repeat) : int(m_smooth);
}

void TFlashBitmapStyle::setParamValue(int index, double value) {
  if (!std::isfinite(value) || index < 2 || index >= 8 || !m_bitmap) return;
  TAffine matrix    = m_bitmapToLocal;
  double *parts[]   = {&matrix.a11, &matrix.a12, &matrix.a13,
                       &matrix.a21, &matrix.a22, &matrix.a23};
  *parts[index - 2] = value;
  setBitmap(m_bitmap, matrix, m_repeat, m_smooth, m_name);
}

void TFlashBitmapStyle::setParamValue(int index, int value) {
  if (!m_bitmap || (index != 0 && index != 1)) return;
  setBitmap(m_bitmap, m_bitmapToLocal, index == 0 ? value != 0 : m_repeat,
            index == 1 ? value != 0 : m_smooth, m_name);
}

void TFlashBitmapStyle::saveData(TOutputStreamInterface &os) const {
  os << kBitmapPayloadVersion << m_name << int(m_repeat) << int(m_smooth)
     << exactDouble(m_bitmapToLocal.a11) << exactDouble(m_bitmapToLocal.a12)
     << exactDouble(m_bitmapToLocal.a13) << exactDouble(m_bitmapToLocal.a21)
     << exactDouble(m_bitmapToLocal.a22) << exactDouble(m_bitmapToLocal.a23);
  if (m_bitmap)
    os << m_bitmap;
  else
    os << TRaster32P(1, 1);
}

void TFlashBitmapStyle::loadData(TInputStreamInterface &is) {
  int version = 0, repeat = 0, smooth = 0;
  std::string name;
  TAffine matrix;
  is >> version >> name >> repeat >> smooth;
  if (version != kBitmapPayloadVersion)
    throw TException("Unsupported Flash bitmap style payload");
  matrix.a11 = readExactDouble(is);
  matrix.a12 = readExactDouble(is);
  matrix.a13 = readExactDouble(is);
  matrix.a21 = readExactDouble(is);
  matrix.a22 = readExactDouble(is);
  matrix.a23 = readExactDouble(is);
  TRaster32P bitmap;
  is >> bitmap;
  if (!setBitmap(bitmap, matrix, repeat != 0, smooth != 0, name))
    throw TException("Invalid Flash bitmap style payload");
}

void TFlashBitmapStyle::drawRegion(const TColorFunction *cf, bool antiAliasing,
                                   TRegionOutline &boundary) const {
  TglTessellator tessellator;
  const bool drawn =
      m_bitmap && drawBitmapTiles(*this, cf, boundary.m_bbox, [&] {
        // White modulated by the texture: each fragment takes the fill.
        tessellator.tessellate(nullptr, antiAliasing, boundary,
                               TPixel32::White);
      });
  // Never fail a render: an unusable view draws the mean bitmap color.
  if (!drawn) tessellator.tessellate(cf, antiAliasing, boundary, m_average);
}

void TFlashBitmapStyle::drawStroke(const TColorFunction *cf,
                                   TStrokeOutline *outline,
                                   const TStroke *) const {
  drawBitmapStroke(cf, outline);
}

void TFlashBitmapStyle::drawAliasedStroke(const TColorFunction *cf,
                                          TStrokeOutline *outline,
                                          const TStroke *) const {
  drawBitmapStroke(cf, outline);
}

void TFlashBitmapStyle::drawBitmapStroke(const TColorFunction *cf,
                                         TStrokeOutline *outline) const {
  if (!outline) return;
  const std::vector<TOutlinePoint> &points = outline->getArray();
  if (points.size() < 2) return;
  TRectD area(points.front().x, points.front().y, points.front().x,
              points.front().y);
  for (const TOutlinePoint &point : points) {
    area.x0 = std::min(area.x0, point.x);
    area.y0 = std::min(area.y0, point.y);
    area.x1 = std::max(area.x1, point.x);
    area.y1 = std::max(area.y1, point.y);
  }
  auto fill = [&] {
    glBegin(GL_QUAD_STRIP);
    for (const TOutlinePoint &point : points) glVertex2d(point.x, point.y);
    glEnd();
  };
  // Overlapping outline quads must not accumulate translucent texels, so
  // each pixel is painted once (as for translucent solid strokes).
  auto paint = [&] {
    TStencilControl *stencil = TStencilControl::instance();
    stencil->beginMask(TStencilControl::DRAW_ON_SCREEN_ONLY_ONCE);
    fill();
    stencil->endMask();
    stencil->enableMask(TStencilControl::SHOW_OUTSIDE);
    drawOutlineSides(points);
    stencil->disableMask();
  };
  if (!m_bitmap || !drawBitmapTiles(*this, cf, area, paint)) {
    tglColor(cf ? (*cf)(m_average) : m_average);
    paint();
  }
}

void TFlashBitmapStyle::makeIcon(const TDimension &size) {
  // The whole bitmap, fitted into the icon, over a checkerboard.
  if (!m_icon || m_icon->getSize() != size) m_icon = TRaster32P(size);
  TRop::checkBoard(m_icon, TPixel32::Black, TPixel32::White, TDimensionD(6, 6),
                   TPointD());
  if (!m_bitmap) return;
  TRaster32P picture(size);
  const double scale = std::max(double(m_bitmap->getLx()) / size.lx,
                                double(m_bitmap->getLy()) / size.ly);
  const TPointD offset((m_bitmap->getLx() - scale * size.lx) * 0.5,
                       (m_bitmap->getLy() - scale * size.ly) * 0.5);
  picture->lock();
  m_bitmap->lock();
  for (int y = 0; y < size.ly; ++y) {
    TPixel32 *row = picture->pixels(y);
    for (int x = 0; x < size.lx; ++x) {
      const TPointD q(offset.x + (x + 0.5) * scale,
                      offset.y + (y + 0.5) * scale);
      row[x] = q.x < 0 || q.y < 0 || q.x >= m_bitmap->getLx() ||
                       q.y >= m_bitmap->getLy()
                   ? TPixel32::Transparent
                   : premultiply(m_bitmap->pixels(int(q.y))[int(q.x)]);
    }
  }
  m_bitmap->unlock();
  picture->unlock();
  TRop::over(m_icon, picture);
}
