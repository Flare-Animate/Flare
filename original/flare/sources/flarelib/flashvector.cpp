#include "flashvector.h"
#include "flashpreflight.h"

#include "../flash/model.h"
#include "../flash/gradient.h"
#include "../flash/fill.h"
#include "../image/pli/pli_precision.h"
#include "tflashgradientstyle.h"
#include "tpalette.h"
#include "tregion.h"
#include "tsimplecolorstyles.h"
#include "tstroke.h"
#include "tcurves.h"

#include <QSet>
#include <QByteArray>
#include <QHash>
#include <QStringList>
#include <algorithm>
#include <cmath>
#include <limits>
#include <map>
#include <memory>
#include <optional>
#include <set>
#include <tuple>
#include <utility>
#include <vector>

namespace FlashNative {

// Maps an XFL BitmapFill matrix to the TFlashBitmapStyle placement of a
// bitmap bitmapHeight pixels tall, in drawing coordinates.
TAffine bitmapFillToLocal(const Flash::Affine &matrix, int bitmapHeight,
                          double stageUnitsPerPixel);

namespace {

quint32 colorKey(const TPixel32 &color) {
  return (quint32(color.r) << 24) | (quint32(color.g) << 16) |
         (quint32(color.b) << 8) | color.m;
}

bool gradientPaint(const Flash::Paint &paint) {
  return paint.kind == Flash::Paint::Kind::LinearGradient ||
         paint.kind == Flash::Paint::Kind::RadialGradient;
}

// Gradients of DOMShape artwork share the edge coordinate space. Primitive
// objects synthesize their outline from object bounds; their gradient frame
// is not verified, so they keep the solid fallback.
bool nativeGradientsAllowed(const Flash::Element &element) {
  return element.kind == Flash::Element::Kind::Shape;
}

// The native style for a validated source gradient, or null when the field
// has no native equivalent (it then keeps the reported solid fallback).
std::unique_ptr<TFlashGradientStyle> gradientStyle(
    const Flash::GradientSpec &spec, double stageUnitsPerPixel) {
  std::vector<TFlashGradientStyle::Stop> stops;
  stops.reserve(spec.stops.size());
  for (const auto &stop : spec.stops)
    stops.push_back({stop.ratio,
                     TPixel32(stop.color.red, stop.color.green, stop.color.blue,
                              stop.color.alpha),
                     stop.alpha});
  const auto spread = spec.spread == Flash::GradientSpec::Spread::Reflect
                          ? TFlashGradientStyle::Spread::Reflect
                      : spec.spread == Flash::GradientSpec::Spread::Repeat
                          ? TFlashGradientStyle::Spread::Repeat
                          : TFlashGradientStyle::Spread::Pad;
  const auto interpolation =
      spec.interpolation == Flash::GradientSpec::Interpolation::LinearRgb
          ? TFlashGradientStyle::Interpolation::LinearRgb
          : TFlashGradientStyle::Interpolation::Rgb;
  const bool radial = spec.kind == Flash::Paint::Kind::RadialGradient;
  std::unique_ptr<TFlashGradientStyle> style(new TFlashGradientStyle());
  if (spec.singular || !std::isfinite(stageUnitsPerPixel) ||
      stageUnitsPerPixel <= 0 ||
      !style->setGradient(radial ? TFlashGradientStyle::Kind::Radial
                                 : TFlashGradientStyle::Kind::Linear,
                          gradientFieldToLocal(spec.matrix, stageUnitsPerPixel),
                          stops, spread, interpolation,
                          radial ? spec.focalPointRatio : 0.0))
    return nullptr;
  return style;
}

// The one color of a gradient whose matrix is singular (see
// Flash::GradientSpec::singular): the color at ratio 0, the first stop.
TPixel32 firstStopColor(const Flash::GradientSpec &spec) {
  const auto &color = spec.stops.front().color;
  return TPixel32(color.red, color.green, color.blue, color.alpha);
}

// A gradient style is identified by its complete field: kind, matrix, the
// ordered stops with their exact alpha, spread, interpolation and focus.
// Distinct gradients with equal stop colors therefore stay distinct styles.
QByteArray gradientKey(const TFlashGradientStyle &style) {
  QByteArray key;
  auto append = [&key](const void *data, size_t size) {
    key.append(static_cast<const char *>(data), int(size));
  };
  const int header[] = {int(style.kind()), int(style.spread()),
                        int(style.interpolation()), int(style.stops().size())};
  append(header, sizeof(header));
  const TAffine &field  = style.gradientToLocal();
  const double values[] = {field.a11, field.a12, field.a13,         field.a21,
                           field.a22, field.a23, style.focalRatio()};
  append(values, sizeof(values));
  for (const auto &stop : style.stops()) {
    const double numbers[]     = {stop.ratio, stop.alpha};
    const unsigned char rgbm[] = {stop.color.r, stop.color.g, stop.color.b,
                                  stop.color.m};
    append(numbers, sizeof(numbers));
    append(rgbm, sizeof(rgbm));
  }
  return key;
}

// A BitmapFill that maps to TFlashBitmapStyle: a library bitmap named by
// bitmapPath, repeated or clipped (bitmapIsClipped), placed by one
// invertible paint matrix, without other attributes or children.
bool bitmapFill(const Flash::Paint &paint) {
  if (paint.kind != Flash::Paint::Kind::Bitmap ||
      paint.sourceTag != "BitmapFill" || !paint.extras.isEmpty() ||
      !paint.source.qualifiedAttributes.isEmpty() ||
      paint.properties.value("bitmapPath").isEmpty())
    return false;
  for (auto it = paint.properties.cbegin(); it != paint.properties.cend(); ++it)
    if (it.key() != "bitmapPath" && it.key() != "bitmapIsClipped") return false;
  const QString clipped = paint.properties.value("bitmapIsClipped", "false");
  if (clipped != "true" && clipped != "false") return false;
  if (!paint.matrixXml || paint.matrixSource.sourceId.isEmpty() ||
      paint.matrixXml->nodes.size() != 2 ||
      !paint.matrixXml->nodes[0].attributes.isEmpty() ||
      !paint.matrixSource.source.qualifiedAttributes.isEmpty())
    return false;
  for (auto it = paint.matrixSource.properties.cbegin();
       it != paint.matrixSource.properties.cend(); ++it)
    if (!QStringList{"a", "b", "c", "d", "tx", "ty"}.contains(it.key()))
      return false;
  const auto &m            = paint.matrix;
  const double determinant = m.determinant();
  return std::isfinite(m.a) && std::isfinite(m.b) && std::isfinite(m.c) &&
         std::isfinite(m.d) && std::isfinite(m.tx) && std::isfinite(m.ty) &&
         std::isfinite(determinant) && determinant != 0;
}

// Paints without a native style (gradients whose semantics are not mapped,
// bitmap fills without a decodable bitmap) are drawn with the mean of their
// gradient stops (other paints: their solid color), which keeps the region
// editable; the approximation is reported.
TPixel32 paintColor(const Flash::Paint &paint) {
  if (gradientPaint(paint) && !paint.stops.isEmpty()) {
    double r = 0, g = 0, b = 0, a = 0;
    for (const auto &stop : paint.stops) {
      r += stop.color.red;
      g += stop.color.green;
      b += stop.color.blue;
      a += stop.color.alpha;
    }
    const double n = paint.stops.size();
    return TPixel32(int(std::lround(r / n)), int(std::lround(g / n)),
                    int(std::lround(b / n)), int(std::lround(a / n)));
  }
  const auto &color = paint.color;
  return TPixel32(color.red, color.green, color.blue, color.alpha);
}

// Finds or creates the palette style of a source paint. Solid styles are
// keyed by color, gradient styles by their complete field. During concurrent
// conversion every style already exists (registerVectorPaints), so style()
// only reads the palette.
class Colors {
  TPalette *m_palette;
  double m_stageUnitsPerPixel;
  bool m_gradients;
  const VectorBitmapResolver *m_bitmaps;
  bool m_requireBitmaps;
  QMap<quint32, int> m_styles;
  QHash<QByteArray, int> m_gradientStyles;
  std::vector<std::pair<int, const TFlashBitmapStyle *>> m_bitmapStyles;

  int add(TColorStyle *style) {
    const int index =
        m_palette->addStyle(style);  // owns style, even on failure
    if (index < 0)
      throw Flash::Error("Cannot create an imported vector color.");
    if (m_palette->getPageCount() == 0) m_palette->addPage(L"Imported colors");
    m_palette->getPage(0)->addStyle(index);
    return index;
  }

public:
  // Bitmap fills get their bitmaps from `bitmaps` while paints are
  // registered; later conversions find the registered styles. With
  // `requireBitmaps` a bitmap fill without a registered style is an error.
  Colors(TPalette *palette, double stageUnitsPerPixel, bool gradients,
         const VectorBitmapResolver *bitmaps = nullptr,
         bool requireBitmaps                 = false)
      : m_palette(palette)
      , m_stageUnitsPerPixel(stageUnitsPerPixel)
      , m_gradients(gradients)
      , m_bitmaps(bitmaps)
      , m_requireBitmaps(requireBitmaps) {
    // Style zero means unpainted native geometry. A transparent source paint
    // needs its own editable style even when its RGBA equals that sentinel.
    for (int index = 1; index < palette->getStyleCount(); ++index) {
      const TColorStyle *style = palette->getStyle(index);
      if (style && style->getTagId() == 3)
        m_styles.insert(colorKey(style->getMainColor()), index);
      else if (const auto *gradient =
                   dynamic_cast<const TFlashGradientStyle *>(style))
        m_gradientStyles.insert(gradientKey(*gradient), index);
      else if (const auto *bitmap =
                   dynamic_cast<const TFlashBitmapStyle *>(style))
        m_bitmapStyles.push_back({index, bitmap});
    }
  }

  // The registered style of a bitmap fill (same bitmap, edges and
  // placement), registering it when a resolver can decode the bitmap; 0 when
  // there is none.
  int bitmapStyle(const Flash::Paint &paint) {
    const std::string name =
        paint.properties.value("bitmapPath").toUtf8().toStdString();
    const bool repeat =
        paint.properties.value("bitmapIsClipped", "false") != "true";
    for (const auto &entry : m_bitmapStyles) {
      const TFlashBitmapStyle *style = entry.second;
      if (style->name() == name && style->repeat() == repeat &&
          style->bitmap() &&
          style->bitmapToLocal() == bitmapFillToLocal(paint.matrix,
                                                      style->bitmap()->getLy(),
                                                      m_stageUnitsPerPixel))
        return entry.first;
    }
    if (!m_bitmaps || !*m_bitmaps) return 0;
    const VectorBitmap bitmap = (*m_bitmaps)(paint);
    if (!bitmap.pixels) return 0;
    std::unique_ptr<TFlashBitmapStyle> style(new TFlashBitmapStyle());
    if (!style->setBitmap(
            bitmap.pixels,
            bitmapFillToLocal(paint.matrix, bitmap.pixels->getLy(),
                              m_stageUnitsPerPixel),
            repeat, bitmap.smooth, name))
      return 0;
    const TFlashBitmapStyle *registered = style.get();
    const int index                     = add(style.release());
    m_bitmapStyles.push_back({index, registered});
    return index;
  }

  int style(const Flash::Paint &paint) {
    std::optional<TPixel32> solid;
    if (m_gradients && bitmapFill(paint)) {
      if (const int index = bitmapStyle(paint)) return index;
      if (m_requireBitmaps)
        throw Flash::Error("Bitmap fill has no decoded native bitmap: " +
                           paint.sourceId);
    }
    if (gradientPaint(paint)) {
      const auto spec = Flash::validateGradient(paint, {});
      // A collapsed field draws one color whatever the drawing frame, so
      // primitives map it exactly too.
      if (spec && spec->singular)
        solid = firstStopColor(*spec);
      else if (m_gradients && spec)
        if (auto gradient = gradientStyle(*spec, m_stageUnitsPerPixel)) {
          const QByteArray key = gradientKey(*gradient);
          const auto found     = m_gradientStyles.constFind(key);
          if (found != m_gradientStyles.cend()) return found.value();
          const int index = add(gradient.release());
          m_gradientStyles.insert(key, index);
          return index;
        }
    }
    const TPixel32 pixel = solid ? *solid : paintColor(paint);
    const quint32 key    = colorKey(pixel);
    const auto found     = m_styles.constFind(key);
    if (found != m_styles.cend()) return found.value();
    const int index = add(new TSolidColorStyle(pixel));
    m_styles.insert(key, index);
    return index;
  }
};

TStroke::OutlineOptions outline(const Flash::Stroke &stroke) {
  TStroke::OutlineOptions options;
  const QString cap = stroke.properties.value("caps", "round");
  if (cap == "none")
    options.m_capStyle = TStroke::OutlineOptions::BUTT_CAP;
  else if (cap == "round")
    options.m_capStyle = TStroke::OutlineOptions::ROUND_CAP;
  else if (cap == "square")
    options.m_capStyle = TStroke::OutlineOptions::PROJECTING_CAP;
  else
    throw Flash::Error("Unsupported XFL stroke cap: " + cap);
  const QString join = stroke.properties.value("joints", "round");
  if (join == "miter")
    options.m_joinStyle = TStroke::OutlineOptions::MITER_JOIN;
  else if (join == "round")
    options.m_joinStyle = TStroke::OutlineOptions::ROUND_JOIN;
  else if (join == "bevel")
    options.m_joinStyle = TStroke::OutlineOptions::BEVEL_JOIN;
  else
    throw Flash::Error("Unsupported XFL stroke joint: " + join);
  bool ok            = true;
  const double miter = stroke.properties.contains("miterLimit")
                           ? stroke.properties["miterLimit"].toDouble(&ok)
                           : 3.0;
  if (!ok || !std::isfinite(miter) || miter < 1 || miter > 1000000)
    throw Flash::Error("Invalid XFL stroke miter limit.");
  options.m_miterUpper = miter;
  return options;
}

class FillWork {
  quint64 m_remaining;
  QString m_sourceId;
  const CompatibilityCheck &m_check;

public:
  FillWork(const QString &sourceId, const CompatibilityCheck &check,
           quint64 budget = 256000000)
      : m_remaining(budget), m_sourceId(sourceId), m_check(check) {}
  void spend(quint64 work = 1) {
    if (work > m_remaining)
      throw Flash::Error(
          "Source fill topology exceeds native analysis budget: " + m_sourceId);
    m_remaining -= work;
    if (m_check) m_check();
  }
};

// These crossings only delimit candidate interior intervals; native containment
// and the source fill evaluator still classify every candidate. Preserve nearby
// roots instead of applying the generic drawing scanline's X merge tolerance.
void sourceScanlineIntersections(const TRegion *region, double y,
                                 std::vector<double> &crossings, FillWork &work,
                                 const QString &sourceId) {
  auto invalid = [&] {
    throw Flash::Error(
        "Source fill topology has invalid native scanline data: " + sourceId);
  };
  if (!region || !std::isfinite(y)) invalid();
  for (UINT edgeIndex = 0; edgeIndex < region->getEdgeCount(); ++edgeIndex) {
    work.spend();
    const auto *edge = region->getEdge(edgeIndex);
    if (!edge || !edge->m_s || !std::isfinite(edge->m_w0) ||
        !std::isfinite(edge->m_w1) || edge->m_w0 < 0 || edge->m_w0 > 1 ||
        edge->m_w1 < 0 || edge->m_w1 > 1)
      invalid();
    int firstChunk, lastChunk;
    double firstT, lastT;
    if (edge->m_s->getChunkAndT(edge->m_w0, firstChunk, firstT) ||
        edge->m_s->getChunkAndT(edge->m_w1, lastChunk, lastT) ||
        firstChunk < 0 || lastChunk < 0 ||
        firstChunk >= edge->m_s->getChunkCount() ||
        lastChunk >= edge->m_s->getChunkCount() || !std::isfinite(firstT) ||
        !std::isfinite(lastT) || firstT < 0 || firstT > 1 || lastT < 0 ||
        lastT > 1)
      invalid();
    const bool forward = firstChunk <= lastChunk;
    for (int chunk = firstChunk;; chunk += forward ? 1 : -1) {
      work.spend();
      const auto *curve = edge->m_s->getChunk(chunk);
      if (!curve) invalid();
      const auto p0 = curve->getP0();
      const auto p1 = curve->getP1();
      const auto p2 = curve->getP2();
      if (!std::isfinite(p0.x) || !std::isfinite(p0.y) ||
          !std::isfinite(p1.x) || !std::isfinite(p1.y) ||
          !std::isfinite(p2.x) || !std::isfinite(p2.y))
        invalid();

      const long double x1     = (long double)p1.x - (long double)p0.x;
      const long double x2     = (long double)p2.x - (long double)p0.x;
      const long double y1     = (long double)p1.y - (long double)p0.y;
      const long double y2     = (long double)p2.y - (long double)p0.y;
      const long double target = (long double)y - (long double)p0.y;
      if (!std::isfinite(x1) || !std::isfinite(x2) || !std::isfinite(y1) ||
          !std::isfinite(y2) || !std::isfinite(target))
        invalid();
      // A constant-Y curve cannot delimit an open horizontal interior interval.
      if (y1 != 0 || y2 != 0) {
        const long double from =
            chunk == firstChunk ? firstT : (forward ? 0 : 1);
        const long double to = chunk == lastChunk ? lastT : (forward ? 1 : 0);
        long double cuts[3]  = {std::min(from, to), std::max(from, to), 0};
        int cutCount         = 2;
        const long double denominator = (y2 - y1) - y1;
        if (!std::isfinite(denominator)) invalid();
        if (denominator != 0) {
          const long double extreme = -y1 / denominator;
          if (!std::isfinite(extreme)) invalid();
          if (extreme > cuts[0] && extreme < cuts[1]) {
            cuts[2]  = cuts[1];
            cuts[1]  = extreme;
            cutCount = 3;
          }
        }
        auto value = [&](long double control, long double end, long double t) {
          const long double u = 1 - t;
          const long double result =
              u * (t * control) + t * (u * control + t * end);
          if (!std::isfinite(result)) invalid();
          return result;
        };
        auto appendCrossing = [&](long double t) {
          work.spend();
          const long double actualX = (long double)p0.x + value(x1, x2, t);
          const double x            = (double)actualX;
          if (!std::isfinite(actualX) || !std::isfinite(x)) invalid();
          crossings.push_back(x);
        };
        for (int piece = 1; piece < cutCount; ++piece) {
          long double low = cuts[piece - 1], high = cuts[piece];
          if (low == high) continue;
          const long double lowY  = value(y1, y2, low);
          const long double highY = value(y1, y2, high);
          if (target < std::min(lowY, highY) || target > std::max(lowY, highY))
            continue;
          // Shared endpoints/extrema can appear twice. Keep actual endpoint X
          // values here; the caller removes only exactly equal crossings.
          if (target == lowY) appendCrossing(low);
          if (target == highY) appendCrossing(high);
          if (target == lowY || target == highY) continue;
          const bool increasing = highY > lowY;
          for (int iteration = 0; iteration < 96; ++iteration) {
            work.spend();
            const long double middle = low + (high - low) * 0.5L;
            if (middle == low || middle == high) break;
            const long double middleY = value(y1, y2, middle);
            if (middleY == target) {
              low = high = middle;
              break;
            }
            if ((middleY < target) == increasing)
              low = middle;
            else
              high = middle;
          }
          appendCrossing(low + (high - low) * 0.5L);
        }
      }
      if (chunk == lastChunk) break;
    }
  }
}

// Best-effort fill assignment when the exact topology resolver gives up:
// each native face takes the source paint found at one interior point, using
// the even-odd rule where the rules disagree. Faces whose sample is on a
// boundary or unresolved stay unpainted.
void approximateRegionFills(const TVectorImageP &image,
                            const Flash::Shape &shape,
                            const QMap<int, int> &fillStyles, double scale,
                            const QString &sourceId,
                            const CompatibilityCheck &check) {
  Flash::FillOptions options;
  options.allowImplicitClosure = true;  // best-effort conversion only
  options.cancelled            = [&] {
    if (check) check();
    return false;
  };
  Flash::FillEvaluator evaluator(shape, options, sourceId);
  std::vector<TRegion *> regions;
  for (UINT index = 0; index < image->getRegionCount(); ++index)
    regions.push_back(image->getRegion(index));
  for (size_t index = 0; index < regions.size(); ++index) {
    TRegion *region = regions[index];
    if (!region) continue;
    for (UINT child = 0; child < region->getSubregionCount(); ++child)
      regions.push_back(region->getSubregion(child));
    int style = 0;
    TPointD point;
    if (!fillStyles.isEmpty() && region->getInternalPoint(point)) {
      try {
        const auto sample =
            evaluator.evaluate({point.x / scale, -point.y / scale});
        const QVector<int> &fills =
            !sample.evenOddFillIndices.isEmpty() ? sample.evenOddFillIndices
            : !sample.fillIndices.isEmpty()      ? sample.fillIndices
                                                 : sample.nonZeroFillIndices;
        // Later source fill styles draw on top of earlier ones.
        if (!fills.isEmpty())
          style = std::max(0, fillStyles.value(fills.back(), 0));
      } catch (const Flash::Error &) {
        if (check) check();  // cancellation still propagates
      }
    }
    region->setStyle(style);
  }
}

void resolveRegionFills(const TVectorImageP &image, const Flash::Shape &shape,
                        Flash::FillEvaluator &evaluator,
                        const QMap<int, int> &fillStyles, double scale,
                        const QString &sourceId,
                        const CompatibilityCheck &check,
                        QVector<VectorPaintWitness> *witnesses,
                        QVector<int> *sourceFillByFace,
                        quint64 maxPaintWitnesses, bool bestEffort) {
  // Source paints are compound paths, not independent colors on each native
  // boundary. In particular, ink-only contours do not remove enclosing paint.
  // Evaluate the original quadratic paths at interior points, then assign the
  // complete native face. Neither getStyle() nor PLI's edge-color normalization
  // may choose an arbitrary paint for us.
  // Best-effort imports give large drawings (detailed brush artwork) a
  // larger budget before they fall back to approximated fills.
  FillWork work(sourceId, check, bestEffort ? 1024000000 : 256000000);
  std::vector<TRegion *> regions;
  for (UINT index = 0; index < image->getRegionCount(); ++index)
    regions.push_back(image->getRegion(index));
  QSet<TRegion *> visited;
  QMap<TRegion *, quint64> costs;
  quint64 totalCost = 0;
  for (size_t index = 0; index < regions.size(); ++index) {
    work.spend();
    if (regions.size() > 100000)
      throw Flash::Error("Source fill topology exceeds native region budget: " +
                         sourceId);
    auto *region = regions[index];
    if (!region || visited.contains(region))
      throw Flash::Error("Invalid native region tree: " + sourceId);
    visited.insert(region);
    quint64 cost = 1;
    for (UINT edgeIndex = 0; edgeIndex < region->getEdgeCount(); ++edgeIndex) {
      const auto *edge = region->getEdge(edgeIndex);
      if (!edge || !edge->m_s || !std::isfinite(edge->m_w0) ||
          !std::isfinite(edge->m_w1))
        throw Flash::Error("Unmapped native region boundary: " + sourceId);
      cost += edge->m_s->getChunkCount();
    }
    costs.insert(region, cost);
    totalCost += cost;
    for (UINT child = 0; child < region->getSubregionCount(); ++child)
      regions.push_back(region->getSubregion(child));
  }
  // Saving quantizes every control to the PLI grid, so a witness closer to
  // a native contour than a few grid steps can change sides on reopening
  // although the drawing is unchanged. Such points still check the
  // conversion here, but are not kept as save/reopen witnesses.
  const int precisionScale = PliCodec::precisionScaleForImage(image);
  const double codecGrid =
      1.0 /
      (precisionScale > 0 ? precisionScale : PliCodec::defaultPrecisionScale);
  auto robustWitness = [&](const TPointD &point, double distance) {
    if (distance >= 64 * codecGrid) return true;
    double w = 0, distance2 = 0;
    UINT stroke = 0;
    work.spend(image->getStrokeCount() + 1);
    return !image->getNearestStroke(point, w, stroke, distance2, false) ||
           distance2 >= (8 * codecGrid) * (8 * codecGrid);
  };
  QHash<const TRegion *, int> faceIndexes;
  if (witnesses) {
    for (size_t index = 0; index < regions.size(); ++index)
      faceIndexes.insert(regions[index], int(index));
  }
  auto rememberPaint = [&](const TPointD &point, int sourceFill, int style,
                           const TRegion *face) {
    if (!witnesses) return;
    if (quint64(witnesses->size()) >= maxPaintWitnesses)
      throw Flash::Error("Native source paint witness budget exceeded: " +
                         sourceId);
    witnesses->push_back(
        {point, sourceFill, style, face ? faceIndexes.value(face, -2) : -1});
    if (witnesses->back().nativeFace == -2)
      throw Flash::Error("Native paint witness has an unknown face: " +
                         sourceId);
  };
  // Flash draws the fills of one shape in ascending style order, so where
  // paints overlap an opaque topmost paint is exactly what is visible.
  auto opaque = [&](int fill) {
    const auto found = shape.fills.constFind(fill);
    if (found == shape.fills.cend()) return false;
    const auto &paint = found.value();
    if (paint.kind == Flash::Paint::Kind::Solid)
      return paint.color.alpha == 255;
    if (!gradientPaint(paint) || paint.stops.isEmpty()) return false;
    for (const auto &stop : paint.stops)
      if (stop.color.alpha != 255) return false;
    return true;
  };
  auto sourceStyle = [&](const Flash::Point &point, int &style, int &sourceFill,
                         bool *overlapped = nullptr) {
    work.spend();
    auto sample = evaluator.evaluate(point);
    if (sample.status == Flash::FillStatus::Boundary) return false;
    if (sample.status != Flash::FillStatus::Resolved)
      throw Flash::Error(
          "Source fill topology has an unresolved winding rule: " + sourceId);
    if (sample.fillIndices.size() > 1 && bestEffort &&
        opaque(sample.fillIndices.back())) {
      sample.fillIndices = {sample.fillIndices.back()};
      if (overlapped) *overlapped = true;
    }
    if (sample.fillIndices.size() > 1) {
      QStringList paints;
      constexpr int maxReportedPaints = 8;
      for (int i = 0; i < sample.fillIndices.size() && i < maxReportedPaints;
           ++i)
        paints.append(QString::number(sample.fillIndices[i]));
      const QString remainder = sample.fillIndices.size() > maxReportedPaints
                                    ? QStringLiteral(", ...")
                                    : QString();
      throw Flash::Error(
          QString("Source fill topology requires overlapping native paints "
                  "[%1%2] at source point (%3, %4): %5")
              .arg(paints.join(", "))
              .arg(remainder)
              .arg(point.x, 0, 'g', 17)
              .arg(point.y, 0, 'g', 17)
              .arg(sourceId));
    }
    style = sample.fillIndices.isEmpty()
                ? 0
                : fillStyles.value(sample.fillIndices.front(), -1);
    if (style < 0)
      throw Flash::Error("Source fill topology has an unmapped paint: " +
                         sourceId);
    sourceFill = sample.fillIndices.isEmpty() ? 0 : sample.fillIndices.front();
    return true;
  };
  QMap<TRegion *, int> styles;
  QMap<TRegion *, int> sourceFillsByRegion;
  for (auto *region : regions) {
    if (fillStyles.isEmpty()) {
      styles.insert(region, 0);
      sourceFillsByRegion.insert(region, 0);
      continue;
    }
    const auto box = region->getBBox();
    if (!std::isfinite(box.x0) || !std::isfinite(box.y0) ||
        !std::isfinite(box.x1) || !std::isfinite(box.y1) || box.x0 >= box.x1 ||
        box.y0 >= box.y1)
      throw Flash::Error("Source fill topology has a degenerate native face: " +
                         sourceId);
    std::vector<double> events{box.y0, box.y1};
    quint64 scanCost = costs.value(region);
    auto addEvent    = [&](double y) {
      if (std::isfinite(y) && y > box.y0 && y < box.y1) events.push_back(y);
    };
    auto addEvents = [&](const TRegion *boundary) {
      for (UINT edgeIndex = 0; edgeIndex < boundary->getEdgeCount();
           ++edgeIndex) {
        work.spend();
        const auto *edge = boundary->getEdge(edgeIndex);
        int firstChunk, lastChunk;
        double firstT, lastT;
        if (edge->m_s->getChunkAndT(edge->m_w0, firstChunk, firstT) ||
            edge->m_s->getChunkAndT(edge->m_w1, lastChunk, lastT))
          throw Flash::Error(
              "Source fill topology has invalid native parameters: " +
              sourceId);
        const bool forward = firstChunk <= lastChunk;
        // Native bounds include ink. Actual curve endpoints and extrema also
        // locate thin filled interiors under much thicker ink outlines.
        for (int chunk = firstChunk;; chunk += forward ? 1 : -1) {
          work.spend();
          const auto *curve = edge->m_s->getChunk(chunk);
          const double from = chunk == firstChunk ? firstT : (forward ? 0 : 1);
          const double to   = chunk == lastChunk ? lastT : (forward ? 1 : 0);
          addEvent(curve->getPoint(from).y);
          addEvent(curve->getPoint(to).y);
          const double denominator =
              curve->getP0().y - 2 * curve->getP1().y + curve->getP2().y;
          if (denominator != 0) {
            const double extreme =
                (curve->getP0().y - curve->getP1().y) / denominator;
            if (extreme > std::min(from, to) && extreme < std::max(from, to))
              addEvent(curve->getPoint(extreme).y);
          }
          if (chunk == lastChunk) break;
        }
      }
    };
    addEvents(region);
    for (UINT child = 0; child < region->getSubregionCount(); ++child) {
      auto *subregion = region->getSubregion(child);
      scanCost += costs.value(subregion);
      addEvents(subregion);
    }
    std::sort(events.begin(), events.end());
    events.erase(std::unique(events.begin(), events.end()), events.end());
    std::vector<std::pair<double, double>> gaps;
    for (size_t index = 1; index < events.size(); ++index)
      gaps.emplace_back(
          events[index] - events[index - 1],
          events[index - 1] + (events[index] - events[index - 1]) * 0.5);
    std::sort(gaps.begin(), gaps.end(),
              [](const auto &a, const auto &b) { return a.first > b.first; });
    if (gaps.size() > 64) gaps.resize(64);
    int resolvedStyle = -1, resolvedFill = -1, samples = 0;
    bool globalLookup = true;
    int robustSamples = 0;
    std::optional<std::pair<TPointD, int>> fragileSample;
    for (const auto &gap : gaps) {
      const double y = gap.second;
      if (!(y > box.y0 && y < box.y1)) continue;
      work.spend(scanCost);
      std::vector<double> crossings;
      sourceScanlineIntersections(region, y, crossings, work, sourceId);
      for (UINT child = 0; child < region->getSubregionCount(); ++child) {
        std::vector<double> childCrossings;
        sourceScanlineIntersections(region->getSubregion(child), y,
                                    childCrossings, work, sourceId);
        crossings.insert(crossings.end(), childCrossings.begin(),
                         childCrossings.end());
      }
      std::sort(crossings.begin(), crossings.end());
      crossings.erase(std::unique(crossings.begin(), crossings.end()),
                      crossings.end());
      int attempts = 0;
      for (size_t index = 1; index < crossings.size() && attempts < 128;
           ++index) {
        const double x = crossings[index - 1] +
                         (crossings[index] - crossings[index - 1]) * 0.5;
        if (!(x > crossings[index - 1] && x < crossings[index])) continue;
        ++attempts;
        work.spend(scanCost * 3);
        const TPointD point(x, y);
        if (!region->contains(point) || region->getRegion(point) != region)
          continue;
        int style, sourceFill;
        if (!sourceStyle({x / scale, -y / scale}, style, sourceFill)) continue;
        if (resolvedStyle >= 0 && resolvedStyle != style)
          throw Flash::Error(
              "Source fill topology differs within a native face: " + sourceId);
        if (resolvedFill >= 0 && resolvedFill != sourceFill)
          throw Flash::Error(
              QString("Source fills [%1, %2] collapse into one native face "
                      "at source point (%3, %4): %5")
                  .arg(resolvedFill)
                  .arg(sourceFill)
                  .arg(x / scale, 0, 'g', 17)
                  .arg(-y / scale, 0, 'g', 17)
                  .arg(sourceId));
        resolvedStyle = style;
        resolvedFill  = sourceFill;
        // A witness is checked later through the drawing-wide face lookup.
        // Where that lookup does not reach this face (a face nested under a
        // parent that does not contain the point), the face keeps its
        // resolved paint, but its interior samples cannot witness it.
        if (witnesses && samples == 0) {
          work.spend(image->getRegionCount() + 1);
          globalLookup = image->getRegion(point) == region;
        }
        const double clearance = std::min(
            {x - crossings[index - 1], crossings[index] - x, gap.first * 0.5});
        if (globalLookup && robustWitness(point, clearance)) {
          rememberPaint(point, sourceFill, style, region);
          ++robustSamples;
        } else if (globalLookup && !fragileSample)
          fragileSample = std::make_pair(point, sourceFill);
        if (++samples == 3) break;
      }
      if (samples == 3) break;
    }
    if (resolvedStyle < 0)
      throw Flash::Error(
          "Source fill topology has no stable native interior point: " +
          sourceId);
    // A face too thin for any robust sample keeps its closest-to-robust one.
    if (!robustSamples && fragileSample)
      rememberPaint(fragileSample->first, fragileSample->second, resolvedStyle,
                    region);
    styles.insert(region, resolvedStyle);
    sourceFillsByRegion.insert(region, resolvedFill);
  }
  // Check both sides of every source-painted segment. This also catches a
  // missing native face; sampling only the faces that survived reconstruction
  // could otherwise overlook omitted source paint. These are geometric
  // consistency checks, not a claim of complete rendered visual fidelity.
  for (const auto &edge : shape.edges) {
    if ((!edge.fill0 && !edge.fill1) || edge.fill0 == edge.fill1) continue;
    for (const auto &segment : edge.segments) {
      work.spend();
      if (segment.start == segment.end &&
          (!segment.quadratic || segment.control == segment.start))
        continue;
      bool witnessed = false, hidden = false;
      // Fixed offsets can jump across an entire thin painted component.
      // Shrink until actual source paint is witnessed, or reject explicitly.
      // Other parameters avoid sampling exactly at a crossing or cusp.
      for (double t : {0.5, 0.375, 0.625}) {
        const double u = 1 - t;
        const Flash::Point control =
            segment.quadratic
                ? segment.control
                : Flash::Point{segment.start.x * 0.5 + segment.end.x * 0.5,
                               segment.start.y * 0.5 + segment.end.y * 0.5};
        const Flash::Point middle{
            u * u * segment.start.x + 2 * u * t * control.x +
                t * t * segment.end.x,
            u * u * segment.start.y + 2 * u * t * control.y +
                t * t * segment.end.y};
        const double dx     = 2 * (u * (control.x - segment.start.x) +
                               t * (segment.end.x - control.x));
        const double dy     = 2 * (u * (control.y - segment.start.y) +
                               t * (segment.end.y - control.y));
        const double length = std::hypot(dx, dy);
        if (length == 0 || !std::isfinite(length)) continue;
        double offset = 0.0625;
        for (int refinement = 0; refinement < 10;
             ++refinement, offset *= 0.25) {
          int resolved            = 0;
          bool hasPaint           = false;
          bool overlapped         = false;
          int sourceFills[2]      = {-1, -1};
          TRegion *nativeFaces[2] = {nullptr, nullptr};
          TPointD sidePoints[2];
          int side = 0;
          for (double distance : {-offset, offset}) {
            const Flash::Point point{middle.x - dy * distance / length,
                                     middle.y + dx * distance / length};
            int expected;
            const int sideIndex = side++;
            if (!sourceStyle(point, expected, sourceFills[sideIndex],
                             &overlapped))
              continue;
            ++resolved;
            hasPaint = hasPaint || expected != 0;
            work.spend(totalCost);
            auto *native =
                image->getRegion(TPointD(point.x * scale, -point.y * scale));
            nativeFaces[sideIndex] = native;
            sidePoints[sideIndex]  = TPointD(point.x * scale, -point.y * scale);
            if ((native ? styles.value(native, -1) : 0) != expected)
              throw Flash::Error(
                  QString(
                      "Source fill topology differs from native boundaries at "
                      "(%1, %2), expected style %3, found %4, native face %6: "
                      "%5")
                      .arg(point.x, 0, 'g', 17)
                      .arg(point.y, 0, 'g', 17)
                      .arg(expected)
                      .arg(native ? styles.value(native, -1) : 0)
                      .arg(sourceId)
                      .arg(native ? "present" : "absent"));
            if (native &&
                sourceFillsByRegion.value(native, -1) != sourceFills[sideIndex])
              throw Flash::Error(
                  QString("Source fill %1 loses its native face identity at "
                          "source point (%2, %3): %4")
                      .arg(sourceFills[sideIndex])
                      .arg(point.x, 0, 'g', 17)
                      .arg(point.y, 0, 'g', 17)
                      .arg(sourceId));
          }
          // A boundary of a paint covered on both sides by one opaque paint
          // drawn on top of it is not visible, and has no transition.
          if (resolved == 2 && overlapped && sourceFills[0] == sourceFills[1])
            hidden = true;
          // Both distant samples can lie in the same paint when a thin empty
          // hole was omitted. Witness the actual source paint transition,
          // including different source paints sharing one palette color.
          if (resolved == 2 && hasPaint && sourceFills[0] != sourceFills[1]) {
            if (nativeFaces[0] == nativeFaces[1])
              throw Flash::Error(
                  QString("Source fills [%1, %2] collapse into one native "
                          "face at source point (%3, %4): %5")
                      .arg(sourceFills[0])
                      .arg(sourceFills[1])
                      .arg(middle.x, 0, 'g', 17)
                      .arg(middle.y, 0, 'g', 17)
                      .arg(sourceId));
            if (styles.value(nativeFaces[0], 0) ==
                styles.value(nativeFaces[1], 0))
              throw Flash::Error(
                  QString("Source fills [%1, %2] share native palette style "
                          "%3 at source point (%4, %5): %6")
                      .arg(sourceFills[0])
                      .arg(sourceFills[1])
                      .arg(styles.value(nativeFaces[0], 0))
                      .arg(middle.x, 0, 'g', 17)
                      .arg(middle.y, 0, 'g', 17)
                      .arg(sourceId));
            if (witnesses) {
              for (int side = 0; side < 2; ++side)
                if (robustWitness(sidePoints[side], offset * scale))
                  rememberPaint(sidePoints[side], sourceFills[side],
                                nativeFaces[side]
                                    ? styles.value(nativeFaces[side], -1)
                                    : 0,
                                nativeFaces[side]);
            }
            witnessed = true;
            break;
          }
        }
        if (witnessed) break;
      }
      if (!witnessed && !hidden)
        throw Flash::Error(
            "Source fill topology has an unwitnessed painted boundary: " +
            sourceId);
    }
  }
  // Validate the complete tree before canonicalizing any shared native edge.
  if (sourceFillByFace) {
    sourceFillByFace->reserve(int(regions.size()));
    for (auto *region : regions)
      sourceFillByFace->push_back(sourceFillsByRegion.value(region, -1));
  }
  for (auto *region : regions) region->setStyle(styles.value(region));
}

}  // namespace

namespace Compatibility {
bool mappedBitmapFill(const Flash::Paint &paint) { return bitmapFill(paint); }

void vector(const Flash::Element &element, const CompatibilitySink &sink,
            const CompatibilityCheck &check, quint64 *usedStyleBindings) {
  if (usedStyleBindings) *usedStyleBindings = 0;
  auto report = [&](const QString &code, const QString &sourceId,
                    const QString &message) {
    sink({code, sourceId, message});
  };
  const bool primitive = element.kind == Flash::Element::Kind::Oval ||
                         element.kind == Flash::Element::Kind::Rectangle;
  if ((element.kind != Flash::Element::Kind::Shape && !primitive) ||
      !element.shape) {
    report("invalid-shape-element", element.sourceId,
           "Native vector conversion requires a shape element.");
    return;
  }
  if (primitive) {
    if (element.shape->fills.isEmpty() && element.shape->strokes.isEmpty())
      report("empty-primitive-paint", element.sourceId,
             "Primitive has no mapped fill or stroke.");
    const QSet<QString> allowed =
        element.kind == Flash::Element::Kind::Oval
            ? QSet<QString>{"x",           "y",
                            "objectWidth", "objectHeight",
                            "selected",    "lockFlag",
                            "visible",     "blendMode"}
            : QSet<QString>{"x",
                            "y",
                            "objectWidth",
                            "objectHeight",
                            "selected",
                            "lockFlag",
                            "visible",
                            "blendMode",
                            "topLeftRadius",
                            "topRightRadius",
                            "bottomLeftRadius",
                            "bottomRightRadius"};
    for (auto it = element.properties.cbegin(); it != element.properties.cend();
         ++it)
      if (!allowed.contains(it.key()))
        report(
            "unmapped-primitive-attribute", element.sourceId + "/" + it.key(),
            "Primitive attribute has no verified native mapping: " + it.key());
  }
  // This guard is also used by vectorImage directly, outside a scene build.
  if (!element.extras.isEmpty())
    report("unmapped-shape-data", element.sourceId,
           "Additional shape content requires a native mapping.");
  const auto &shape = *element.shape;
  QSet<int> fills, strokes;
  for (int index = 0; index < shape.edges.size(); ++index) {
    if (check && (index & 1023) == 0) check();
    const auto &edge = shape.edges[index];
    if (edge.segments.isEmpty()) continue;
    if (edge.fill0) fills.insert(edge.fill0);
    if (edge.fill1) fills.insert(edge.fill1);
    if (edge.stroke) strokes.insert(edge.stroke);
  }
  if (usedStyleBindings)
    *usedStyleBindings = quint64(fills.size()) + quint64(strokes.size());
  auto paint = [&](const Flash::Paint &paint, const QString &sourceId) {
    if (gradientPaint(paint)) {
      const auto spec = Flash::validateGradient(
          paint, [&](const Flash::GradientIssue &issue) {
            report(issue.code, issue.sourceId, issue.message);
          });
      // The native style's validity does not depend on the positive
      // coordinate scale, so any scale gives the conversion's answer.
      if (!spec)
        report("unmapped-vector-paint", sourceId,
               "This gradient's source semantics have no verified native "
               "mapping; it is drawn in the mean color of its stops: " +
                   paint.sourceTag);
      else if (spec->singular) {
        // One exact color (firstStopColor).
      } else if (primitive)
        report("unmapped-vector-paint", sourceId,
               "Gradients on primitive objects have no verified native "
               "mapping; drawn in the mean color of the stops: " +
                   paint.sourceTag);
      else if (!gradientStyle(*spec, 1.0))
        report("unmapped-vector-paint", sourceId,
               "This gradient field has no native equivalent; it is drawn in "
               "the mean color of its stops: " +
                   paint.sourceTag);
    } else if (!primitive && bitmapFill(paint)) {
      // TFlashBitmapStyle. A bitmap that cannot be decoded is reported when
      // the paint is registered (registerVectorPaints).
    } else if (paint.kind != Flash::Paint::Kind::Solid ||
               !paint.extras.isEmpty())
      report("unmapped-vector-paint", sourceId,
             "This vector paint requires an additional native style mapping: " +
                 paint.sourceTag);
  };
  for (int index : fills) {
    if (check) check();
    const auto sourceId = element.sourceId + "/fill" + QString::number(index);
    const auto found    = shape.fills.constFind(index);
    if (found == shape.fills.cend())
      report("missing-vector-fill", sourceId, "Missing source vector fill.");
    else
      paint(found.value(), sourceId);
  }
  for (int index : strokes) {
    if (check) check();
    const auto sourceId = element.sourceId + "/stroke" + QString::number(index);
    const auto found    = shape.strokes.constFind(index);
    if (found == shape.strokes.cend()) {
      report("missing-vector-stroke", sourceId,
             "Missing source vector stroke.");
      continue;
    }
    const auto &stroke = found.value();
    // Pixel hinting only snaps the stroke to whole device pixels while a
    // player rasterizes it; the published shape keeps the authored geometry,
    // which the reference renderer draws unhinted, and so does this import.
    const QString hinting = stroke.properties.value("pixelHinting", "false");
    if (stroke.type != "SolidStroke" || !stroke.extras.isEmpty() ||
        stroke.properties.value("scaleMode", "normal") != "normal" ||
        (!stroke.properties.value("solidStyle").isEmpty() &&
         stroke.properties.value("solidStyle") != "normal") ||
        (hinting != "false" && hinting != "true"))
      report("unmapped-vector-stroke", sourceId,
             "Stroke type/style, scale mode or pixel hinting needs a native "
             "mapping: " +
                 stroke.type + " solidStyle=" +
                 stroke.properties.value("solidStyle") + " pixelHinting=" +
                 stroke.properties.value("pixelHinting", "false"));
    if (!std::isfinite(stroke.weight) || stroke.weight <= 0)
      report("unmapped-hairline-stroke", sourceId,
             "Hairline or invalid strokes need a dedicated native mapping.");
    paint(stroke.paint, sourceId + "/fill");
    try {
      outline(stroke);
    } catch (const Flash::Error &error) {
      report("invalid-stroke-outline", sourceId,
             QString::fromUtf8(error.what()));
    }
  }
}
}  // namespace Compatibility

// Every PLI write needs at least the legacy 1/16384 grid. Reject values that
// cannot fit even there before handing a source stroke to the writer; the
// frame-wide scale selector then checks all absolute controls and deltas for
// the finer source grid before any integer cast.
static void validatePliStrokeCodec(const TStroke &stroke,
                                   const QString &sourceId,
                                   const CompatibilityCheck &check) {
  constexpr double grid  = 16384.0;
  constexpr double limit = double(std::numeric_limits<int>::max()) - 1.0;
  auto bounded           = [&](double value) {
    if (!std::isfinite(value) || value < -limit || value > limit)
      throw Flash::Error("Native PLI vector coordinate exceeds codec limits: " +
                                   sourceId);
  };
  double maxThickness = 0;
  for (int control = 0; control < stroke.getControlPointCount(); ++control) {
    if (check && (control & 1023) == 0) check();
    const auto point = stroke.getControlPoint(control);
    if (!std::isfinite(point.x) || !std::isfinite(point.y) ||
        !std::isfinite(point.thick) || point.thick < 0)
      throw Flash::Error("Native PLI source control is not finite: " +
                         sourceId);
    maxThickness = std::max(maxThickness, point.thick);
  }
  if (maxThickness > 255)
    throw Flash::Error("Native PLI stroke thickness exceeds codec limits: " +
                       sourceId);
  if (!stroke.getChunkCount()) return;
  const auto first = stroke.getControlPoint(0);
  bounded(first.x * grid);
  bounded(first.y * grid);
  for (int chunk = 0; chunk < stroke.getChunkCount(); ++chunk) {
    if (check && (chunk & 1023) == 0) check();
    const auto *curve = stroke.getChunk(chunk);
    const TPointD p0 = curve->getP0(), p1 = curve->getP1(), p2 = curve->getP2();
    bounded((p1.x - p0.x) * grid);
    bounded((p1.y - p0.y) * grid);
    bounded((p2.x - p1.x) * grid);
    bounded((p2.y - p1.y) * grid);
  }
}

TAffine bitmapFillToLocal(const Flash::Affine &matrix, int bitmapHeight,
                          double stageUnitsPerPixel) {
  // Bitmap fill matrices scale like SWF's, 20 units per bitmap pixel, with
  // the translation in source pixels: bitmap pixel (u, v), v down from the
  // top row, lies at source ((a u + c v) / 20 + tx, (b u + d v) / 20 + ty).
  // Native bitmap rows run bottom-up (v = height - y) and the drawing maps a
  // source pixel (x, y) to (s x, -s y).
  const double s = stageUnitsPerPixel / 20.0;
  const double h = bitmapHeight;
  return TAffine(s * matrix.a, -s * matrix.c,
                 s * matrix.c * h + stageUnitsPerPixel * matrix.tx,
                 -s * matrix.b, s * matrix.d,
                 -s * matrix.d * h - stageUnitsPerPixel * matrix.ty);
}

TAffine gradientFieldToLocal(const Flash::Affine &matrix,
                             double stageUnitsPerPixel) {
  // Flash gradient space spans [-16384, 16384] twips (+-819.2 px) before the
  // paint matrix, whose translation is in source pixels. The drawing maps a
  // source pixel (x, y) to (s x, -s y). gradientToLocal = C * M * scale(819.2)
  // with C = diag(s, -s), applied to the style's [-1, 1] field.
  constexpr double halfExtent = 819.2;
  const double s              = stageUnitsPerPixel;
  return TAffine(s * matrix.a * halfExtent, s * matrix.c * halfExtent,
                 s * matrix.tx, -s * matrix.b * halfExtent,
                 -s * matrix.d * halfExtent, -s * matrix.ty);
}

void registerVectorPaints(const Flash::Element &element, TPalette *palette,
                          double stageUnitsPerPixel,
                          const VectorBitmapResolver &bitmaps) {
  if (!element.shape || !palette || !std::isfinite(stageUnitsPerPixel) ||
      stageUnitsPerPixel <= 0)
    return;
  // Same first-encounter order as vectorImage(): per edge, fill0, fill1, then
  // the stroke paint. A later vectorImage() call then only reads the palette.
  Colors colors(palette, stageUnitsPerPixel, nativeGradientsAllowed(element),
                &bitmaps);
  const auto &shape = *element.shape;
  QSet<int> fills, strokes;
  for (const auto &edge : shape.edges) {
    if (edge.segments.isEmpty()) continue;
    for (int fill : {edge.fill0, edge.fill1}) {
      if (!fill || fills.contains(fill)) continue;
      fills.insert(fill);
      const auto found = shape.fills.constFind(fill);
      if (found != shape.fills.cend()) colors.style(found.value());
    }
    if (edge.stroke && !strokes.contains(edge.stroke)) {
      strokes.insert(edge.stroke);
      const auto found = shape.strokes.constFind(edge.stroke);
      if (found != shape.strokes.cend()) colors.style(found.value().paint);
    }
  }
}

TVectorImageP vectorImage(const Flash::Element &element, TPalette *palette,
                          double stageUnitsPerPixel,
                          const CompatibilityCheck &check,
                          QVector<VectorStyleBinding> *sourceStyles,
                          QVector<VectorStrokeBinding> *sourceStrokes,
                          VectorFrameAudit *frameAudit,
                          quint64 maxPaintWitnesses, bool approximatePaint) {
  // Best-effort callers have already reported approximated paint and stroke
  // details; only details that cannot be drawn still reject here.
  Compatibility::vector(
      element,
      approximatePaint ? CompatibilitySink([](const CompatibilityIssue &issue) {
        if (Compatibility::handling(issue.code) != IssueHandling::Approximate)
          Compatibility::reject(issue);
      })
                       : CompatibilitySink(Compatibility::reject),
      check);
  if (!palette || !std::isfinite(stageUnitsPerPixel) || stageUnitsPerPixel <= 0)
    throw Flash::Error("Invalid native vector palette or coordinate scale.");
  const auto &shape = *element.shape;
  TVectorImageP image(new TVectorImage());
  image->setRegionFindingMode(TVectorImage::RegionFindingMode::PreserveSource);
  image->setPalette(palette);
  Colors colors(palette, stageUnitsPerPixel, nativeGradientsAllowed(element),
                nullptr, !approximatePaint);
  QMap<int, int> fillStyles;
  QMap<int, int> inkStyles;
  QSet<int> recordedStrokes;
  QVector<VectorStyleBinding> convertedStyles;
  QVector<VectorStrokeBinding> convertedStrokes;
  VectorFrameAudit convertedAudit;
  QVector<const TStroke *> convertedStrokePointers;
  auto fillStyle = [&](int sourceIndex) {
    if (sourceIndex == 0) return 0;
    const auto existing = fillStyles.constFind(sourceIndex);
    if (existing != fillStyles.cend()) return existing.value();
    if (!shape.fills.contains(sourceIndex))
      throw Flash::Error("Missing source vector fill.");
    const auto &paint = shape.fills[sourceIndex];
    const int style   = colors.style(paint);
    fillStyles.insert(sourceIndex, style);
    convertedStyles.push_back(
        {"fill", paint.style.sourceId, paint.sourceId, sourceIndex, style});
    return style;
  };
  // An edge without fills or ink draws nothing and bounds no paint. The
  // best-effort import leaves it out (stray construction lines can lie far
  // outside the PLI coordinate range).
  auto drawsNothing = [&](const Flash::Edge &edge) {
    return approximatePaint && !edge.fill0 && !edge.fill1 && !edge.stroke;
  };
  for (int edgeIndex = 0; edgeIndex < shape.edges.size(); ++edgeIndex) {
    const auto &edge = shape.edges[edgeIndex];
    if (check) check();
    if (edge.segments.isEmpty() || drawsNothing(edge)) continue;
    fillStyle(edge.fill0);
    fillStyle(edge.fill1);
    int inkStyle  = 0;
    double radius = 0;
    TStroke::OutlineOptions options;
    if (edge.stroke) {
      if (!shape.strokes.contains(edge.stroke))
        throw Flash::Error("Missing source vector stroke.");
      const auto &stroke = shape.strokes[edge.stroke];
      const auto known   = inkStyles.constFind(edge.stroke);
      inkStyle           = known != inkStyles.cend()
                               ? known.value()
                               : inkStyles.insert(edge.stroke, colors.style(stroke.paint))
                           .value();
      if (!recordedStrokes.contains(edge.stroke)) {
        convertedStyles.push_back({"stroke", stroke.style.sourceId,
                                   stroke.paint.sourceId, edge.stroke,
                                   inkStyle});
        recordedStrokes.insert(edge.stroke);
      }
      // Hairlines are one device pixel in Flash; approximate with one
      // source pixel.
      const double weight =
          approximatePaint &&
                  !(std::isfinite(stroke.weight) && stroke.weight > 0)
              ? 1.0
              : stroke.weight;
      radius  = weight * stageUnitsPerPixel / 2;
      options = outline(stroke);
    }
    std::vector<TThickPoint> points;
    int firstSegment  = 0;
    int nextSegment   = 0;
    int contourIndex  = 0;
    auto appendStroke = [&] {
      if (points.size() < 3) return;
      std::unique_ptr<TStroke> stroke(new TStroke(points));
      validatePliStrokeCodec(*stroke, element.sourceId, check);
      stroke->setStyle(inkStyle);
      stroke->outlineOptions() = options;
      stroke->setSelfLoop(points.front().x == points.back().x &&
                          points.front().y == points.back().y);
      const int segments = nextSegment - firstSegment;
      if (sourceStrokes && (stroke->getChunkCount() != segments ||
                            stroke->getControlPointCount() != 2 * segments + 1))
        throw Flash::Error("Native stroke no longer matches source segments.");
      const int index = image->addStroke(stroke.get(), false);
      if (index < 0)
        throw Flash::Error("Cannot create an imported vector stroke.");
      // addStroke owns the stroke from this point. Building the source binding
      // can allocate, so do not retain a second owner while doing that work.
      stroke.release();
      if (sourceStrokes) {
        const TStroke *const native = image->getStroke(index);
        convertedStrokePointers.push_back(native);
        const QString edgeId =
            edge.source.sourceId.isEmpty()
                ? element.sourceId + "/edges/Edge" + QString::number(edgeIndex)
                : edge.source.sourceId;
        convertedStrokes.push_back({edgeId, edge.source.source.sourceId,
                                    edgeIndex, contourIndex, firstSegment,
                                    segments, index, 0, native->getChunkCount(),
                                    native->getControlPointCount()});
      }
      points.clear();
      ++contourIndex;
    };
    auto point = [&](Flash::Point value) {
      const double x = value.x * stageUnitsPerPixel;
      const double y = -value.y * stageUnitsPerPixel;
      constexpr double maxPosition =
          (double(std::numeric_limits<int>::max()) - 1.0) / 16384.0;
      if (!std::isfinite(x) || !std::isfinite(y) || !std::isfinite(radius) ||
          std::abs(x) > maxPosition || std::abs(y) > maxPosition ||
          radius < 0 || radius > 255)
        throw Flash::Error(
            "Native PLI vector coordinate or thickness exceeds "
            "codec limits: " +
            element.sourceId);
      return TThickPoint(x, y, radius);
    };
    for (int segmentIndex = 0; segmentIndex < edge.segments.size();
         ++segmentIndex) {
      const auto &segment = edge.segments[segmentIndex];
      if (check && (segmentIndex & 1023) == 0) check();
      if (segment.startsContour && !points.empty()) appendStroke();
      if (points.empty()) firstSegment = segmentIndex;
      if (points.empty()) points.push_back(point(segment.start));
      points.push_back(point(segment.control));
      points.push_back(point(segment.end));
      nextSegment = segmentIndex + 1;
    }
    appendStroke();
  }
  if (check) check();
  // Best-effort conversion closes open source fill outlines as Flash and
  // Ruffle draw them (Flash::FillOptions::allowImplicitClosure). A straight
  // closure that no straight source segment already draws gets an invisible
  // native stroke after all source strokes, so native faces have the same
  // boundary as the source paint and source stroke indexes are unchanged.
  // Where a paint's contours overlap themselves, best-effort conversion uses
  // the even-odd rule: Animate publishes shapes without the SWF nonzero
  // winding flag (DefineShape4), so players fill them even-odd.
  Flash::FillOptions fillOptions;
  fillOptions.allowImplicitClosure = approximatePaint;
  if (approximatePaint) {
    fillOptions.rule = Flash::FillRule::EvenOdd;
    fillOptions.maxWork *= 4;
  }
  fillOptions.cancelled = [&] {
    if (check) check();
    return false;
  };
  std::unique_ptr<Flash::FillEvaluator> evaluator;
  QString fillError;
  try {
    evaluator.reset(
        new Flash::FillEvaluator(shape, fillOptions, element.sourceId));
  } catch (const Flash::Error &error) {
    if (check) check();
    if (!approximatePaint) throw;
    fillError = QString::fromUtf8(error.what());
  }
  if (evaluator && !evaluator->closures().isEmpty()) {
    std::set<std::pair<std::pair<double, double>, std::pair<double, double>>>
        straight;
    for (const auto &edge : shape.edges)
      for (const auto &segment : edge.segments) {
        if (drawsNothing(edge)) break;
        const double cross = (segment.control.x - segment.start.x) *
                                 (segment.end.y - segment.start.y) -
                             (segment.control.y - segment.start.y) *
                                 (segment.end.x - segment.start.x);
        if (segment.quadratic && cross != 0) continue;
        auto a = std::make_pair(segment.start.x, segment.start.y);
        auto b = std::make_pair(segment.end.x, segment.end.y);
        straight.insert(a < b ? std::make_pair(a, b) : std::make_pair(b, a));
      }
    for (const auto &closure : evaluator->closures()) {
      if (check) check();
      if (closure.sourceEdge) continue;
      const auto &segment = closure.segment;
      auto a              = std::make_pair(segment.start.x, segment.start.y);
      auto b              = std::make_pair(segment.end.x, segment.end.y);
      if (straight.count(a < b ? std::make_pair(a, b) : std::make_pair(b, a)))
        continue;
      std::vector<TThickPoint> points;
      for (const auto &value : {segment.start, segment.control, segment.end})
        points.push_back(TThickPoint(value.x * stageUnitsPerPixel,
                                     -value.y * stageUnitsPerPixel, 0));
      std::unique_ptr<TStroke> stroke(new TStroke(points));
      validatePliStrokeCodec(*stroke, element.sourceId, check);
      stroke->setStyle(0);
      if (image->addStroke(stroke.get(), false) < 0)
        throw Flash::Error("Cannot create an imported fill closure.");
      stroke.release();
    }
  }
  image->findRegions();
  if (check) check();
  try {
    if (!evaluator) throw Flash::Error(fillError);
    resolveRegionFills(image, shape, *evaluator, fillStyles, stageUnitsPerPixel,
                       element.sourceId, check,
                       frameAudit ? &convertedAudit.paint : nullptr,
                       frameAudit ? &convertedAudit.sourceFillByFace : nullptr,
                       maxPaintWitnesses, approximatePaint);
  } catch (const Flash::Error &error) {
    if (check) check();  // cancellation is never approximated
    if (!approximatePaint) throw;
    convertedAudit.paint.clear();
    convertedAudit.sourceFillByFace.clear();
    convertedAudit.approximatedFills   = true;
    convertedAudit.approximationReason = QString::fromUtf8(error.what());
    approximateRegionFills(image, shape, fillStyles, stageUnitsPerPixel,
                           element.sourceId, check);
  }
  for (int i = 0; i < convertedStrokes.size(); ++i) {
    const auto &binding = convertedStrokes[i];
    if (binding.nativeStrokeIndex < 0 ||
        binding.nativeStrokeIndex >= int(image->getStrokeCount()))
      throw Flash::Error(
          "Source vector stroke moved during region construction.");
    const TStroke *native = image->getStroke(binding.nativeStrokeIndex);
    if (native != convertedStrokePointers[i] ||
        native->getChunkCount() != binding.nativeChunkCount ||
        native->getControlPointCount() != binding.nativeControlCount)
      throw Flash::Error(
          "Source vector contour changed during region construction.");
  }
  if (sourceStyles) *sourceStyles = std::move(convertedStyles);
  if (sourceStrokes) *sourceStrokes = std::move(convertedStrokes);
  if (frameAudit) {
    convertedAudit.sourceId               = element.sourceId;
    convertedAudit.expectedPaintWitnesses = convertedAudit.paint.size();
    *frameAudit                           = std::move(convertedAudit);
  }
  return image;
}

void verifyVectorFramePersistence(const TVectorImageP &original,
                                  const TVectorImageP &reopened,
                                  const VectorFrameAudit &audit,
                                  const CompatibilityCheck &check) {
  if (!original || !reopened || !original->getPalette() ||
      !reopened->getPalette())
    throw Flash::Error("Native PLI frame or palette is missing.");
  if (original->getRegionFindingMode() != reopened->getRegionFindingMode() ||
      original->getStrokeCount() != reopened->getStrokeCount())
    throw Flash::Error("Native PLI changed the vector policy or stroke count.");
  const auto *beforePalette = original->getPalette();
  const auto *afterPalette  = reopened->getPalette();
  if (beforePalette->getStyleCount() != afterPalette->getStyleCount())
    throw Flash::Error("Native PLI changed the vector palette style count.");
  if (beforePalette->getPageCount() != afterPalette->getPageCount())
    throw Flash::Error("Native PLI changed vector palette pages.");
  // Imported styles are identified by their palette style index; that index
  // must keep the same style kind, name and colors across save/reopen.
  for (int style = 0; style < beforePalette->getStyleCount(); ++style) {
    if (check && (style & 1023) == 0) check();
    const auto *before = beforePalette->getStyle(style);
    const auto *after  = afterPalette->getStyle(style);
    if (!before || !after || before->getMainColor() != after->getMainColor())
      throw Flash::Error("Native PLI changed a vector palette paint.");
    bool sameStyle =
        before->getTagId() == after->getTagId() &&
        before->getName() == after->getName() &&
        before->getColorParamCount() == after->getColorParamCount();
    for (int param = 0; sameStyle && param < before->getColorParamCount();
         ++param)
      sameStyle =
          before->getColorParamValue(param) == after->getColorParamValue(param);
    if (sameStyle && dynamic_cast<const TFlashBitmapStyle *>(before)) {
      const auto *oldFill = dynamic_cast<const TFlashBitmapStyle *>(before);
      const auto *newFill = dynamic_cast<const TFlashBitmapStyle *>(after);
      sameStyle           = newFill && oldFill->name() == newFill->name() &&
                  oldFill->repeat() == newFill->repeat() &&
                  oldFill->smooth() == newFill->smooth() &&
                  oldFill->bitmapToLocal() == newFill->bitmapToLocal() &&
                  oldFill->bitmap() && newFill->bitmap() &&
                  oldFill->bitmap()->getSize() == newFill->bitmap()->getSize();
      for (int y = 0; sameStyle && y < oldFill->bitmap()->getLy(); ++y)
        sameStyle = std::equal(
            oldFill->bitmap()->pixels(y),
            oldFill->bitmap()->pixels(y) + oldFill->bitmap()->getLx(),
            newFill->bitmap()->pixels(y));
    }
    // Gradient fields are stored exactly (matrix, stop ratios and alpha,
    // modes, focus); every parameter must survive unchanged.
    if (sameStyle && dynamic_cast<const TFlashGradientStyle *>(before)) {
      const auto *oldField = dynamic_cast<const TFlashGradientStyle *>(before);
      const auto *newField = dynamic_cast<const TFlashGradientStyle *>(after);
      sameStyle            = newField &&
                  gradientKey(*oldField) == gradientKey(*newField) &&
                  before->getParamCount() == after->getParamCount();
      for (int param = 0; sameStyle && param < before->getParamCount(); ++param)
        sameStyle =
            before->getParamType(param) == after->getParamType(param) &&
            (before->getParamType(param) == TColorStyle::DOUBLE
                 ? before->getParamValue(TColorStyle::double_tag(), param) ==
                       after->getParamValue(TColorStyle::double_tag(), param)
                 : before->getParamValue(TColorStyle::int_tag(), param) ==
                       after->getParamValue(TColorStyle::int_tag(), param));
    }
    if (!sameStyle)
      throw Flash::Error("Native PLI changed a vector palette style identity.");
    const auto *beforePage = beforePalette->getStylePage(style);
    const auto *afterPage  = afterPalette->getStylePage(style);
    if ((beforePage == nullptr) != (afterPage == nullptr) ||
        (beforePage &&
         (beforePage->getIndex() != afterPage->getIndex() ||
          beforePage->search(style) < 0 || afterPage->search(style) < 0)))
      throw Flash::Error("Native PLI changed vector palette page membership.");
  }

  // PLI writes the first control with integer truncation on a bounded grid.
  // Source-preserving frames quantize each absolute control before producing
  // deltas, while ordinary frames retain independent rounded deltas. The
  // reader also separates a zero half-chunk by 0.001 of the other half.
  // Reconstruct the actual codec result, including the source-only anchoring.
  const int precisionScale = PliCodec::precisionScaleForImage(original);
  if (precisionScale == 0)
    throw Flash::Error("Native PLI vector coordinate exceeds codec limits.");
  const double grid   = double(precisionScale);
  const bool anchored = original->getRegionFindingMode() ==
                        TVectorImage::RegionFindingMode::PreserveSource;
  auto integral = [](double value, bool rounded) {
    if (!std::isfinite(value) ||
        value < double(std::numeric_limits<int>::min()) + 1 ||
        value > double(std::numeric_limits<int>::max()) - 1)
      throw Flash::Error("Native PLI vector coordinate exceeds codec limits.");
    return rounded ? tround(value) : int(value);
  };
  auto codecDelta = [&](double from, double to) {
    if (anchored)
      return (double(integral(to * grid, false)) -
              double(integral(from * grid, false))) /
             grid;
    return double(integral((to - from) * grid, true)) / grid;
  };
  for (UINT index = 0; index < original->getStrokeCount(); ++index) {
    if (check && (index & 255) == 0) check();
    const auto *before = original->getStroke(index);
    const auto *after  = reopened->getStroke(index);
    if (!before || !after || before->getStyle() != after->getStyle() ||
        before->isSelfLoop() != after->isSelfLoop() ||
        before->getChunkCount() != after->getChunkCount() ||
        before->getControlPointCount() != after->getControlPointCount() ||
        original->getGroupDepth(index) != reopened->getGroupDepth(index) ||
        (index && original->sameGroup(index - 1, index) !=
                      reopened->sameGroup(index - 1, index)))
      throw Flash::Error("Native PLI changed editable stroke structure.");
    const auto &oldOutline = before->outlineOptions();
    const auto &newOutline = after->outlineOptions();
    if (oldOutline.m_capStyle != newOutline.m_capStyle ||
        oldOutline.m_joinStyle != newOutline.m_joinStyle ||
        std::abs(oldOutline.m_miterLower - newOutline.m_miterLower) > 1e-8 ||
        std::abs(oldOutline.m_miterUpper - newOutline.m_miterUpper) > 1e-8)
      throw Flash::Error("Native PLI changed editable stroke outline options.");
    if (before->getChunkCount() <= 0) continue;
    double maxThickness = 0;
    for (int control = 0; control < before->getControlPointCount(); ++control) {
      if (check && (control & 1023) == 0) check();
      const auto point = before->getControlPoint(control);
      if (!std::isfinite(point.x) || !std::isfinite(point.y) ||
          !std::isfinite(point.thick) || point.thick < 0)
        throw Flash::Error("Native PLI source control is not finite.");
      maxThickness = std::max(maxThickness, point.thick);
    }
    if (maxThickness > 255)
      throw Flash::Error("Native PLI stroke thickness exceeds codec limits.");
    const double thickStep = std::ceil(maxThickness) / 255.0;
    auto verifyControl     = [&](int control, const TPointD &expected) {
      const auto oldPoint = before->getControlPoint(control);
      const auto newPoint = after->getControlPoint(control);
      if (!std::isfinite(newPoint.x) || !std::isfinite(newPoint.y) ||
          !std::isfinite(newPoint.thick) ||
          std::abs(newPoint.x - expected.x) > 1e-8 ||
          std::abs(newPoint.y - expected.y) > 1e-8 ||
          std::abs(newPoint.thick - oldPoint.thick) > (thickStep / 2.0 + 1e-8))
        throw Flash::Error(
                QString("Native PLI changed vector control %1 on stroke %2.")
                    .arg(control)
                    .arg(index));
    };
    const auto start = before->getControlPoint(0);
    TPointD decoded(integral(start.x * grid, false) / grid,
                    integral(start.y * grid, false) / grid);
    const TPointD decodedFirst = decoded;
    for (int chunk = 0; chunk < before->getChunkCount(); ++chunk) {
      if (check && (chunk & 1023) == 0) check();
      const auto *curve = before->getChunk(chunk);
      const TPointD p0 = curve->getP0(), p1 = curve->getP1(),
                    p2 = curve->getP2();
      TPointD delta1(codecDelta(p0.x, p1.x), codecDelta(p0.y, p1.y));
      TPointD delta2(codecDelta(p1.x, p2.x), codecDelta(p1.y, p2.y));
      if (delta1.x == 0 && delta1.y == 0 && (delta2.x != 0 || delta2.y != 0)) {
        delta1 = TPointD(delta2.x * 0.001, delta2.y * 0.001);
        delta2 = TPointD(delta2.x * 0.999, delta2.y * 0.999);
      } else if (delta2.x == 0 && delta2.y == 0 &&
                 (delta1.x != 0 || delta1.y != 0)) {
        delta2 = TPointD(delta1.x * 0.001, delta1.y * 0.001);
        delta1 = TPointD(delta1.x * 0.999, delta1.y * 0.999);
      }
      decoded = TPointD(decoded.x + delta1.x, decoded.y + delta1.y);
      verifyControl(2 * chunk + 1, decoded);
      decoded = TPointD(decoded.x + delta2.x, decoded.y + delta2.y);
      if (!before->isSelfLoop() || chunk + 1 < before->getChunkCount())
        verifyControl(2 * chunk + 2, decoded);
    }
    // TImageReaderPli calls setSelfLoop() after rebuilding a closed stroke.
    // That native operation averages the first and last decoded controls to
    // reconnect rounding drift, so compare against its actual codec result.
    if (before->isSelfLoop()) {
      const TPointD joined((decodedFirst.x + decoded.x) * 0.5,
                           (decodedFirst.y + decoded.y) * 0.5);
      verifyControl(0, joined);
      verifyControl(before->getControlPointCount() - 1, joined);
    } else {
      verifyControl(0, decodedFirst);
    }
  }

  // Intern complete child trees bottom-up. A flat histogram of region sizes
  // and paints would miss a child reparented under a different native face.
  using Face = std::tuple<UINT, int, std::vector<int>>;
  std::map<Face, int> canonicalFaces;
  auto faceStructure = [&](const TVectorImageP &image,
                           QHash<const TRegion *, int> &ordinals) {
    std::vector<const TRegion *> regions;
    for (UINT index = 0; index < image->getRegionCount(); ++index)
      regions.push_back(image->getRegion(index));
    QSet<const TRegion *> visited;
    struct Node {
      UINT edges        = 0;
      int style         = 0;
      size_t firstChild = 0;
      UINT children     = 0;
    };
    std::vector<Node> nodes;
    for (size_t index = 0; index < regions.size(); ++index) {
      if (check && (index & 1023) == 0) check();
      const auto *region = regions[index];
      if (!region || visited.contains(region) || regions.size() > 4000000 ||
          !region->getEdgeCount())
        throw Flash::Error("Native PLI changed the vector region tree.");
      visited.insert(region);
      ordinals.insert(region, int(index));
      const int style = region->getEdge(0)->m_styleId;
      if (style < 0 || style >= image->getPalette()->getStyleCount())
        throw Flash::Error("Native PLI has an invalid face paint.");
      for (UINT edge = 1; edge < region->getEdgeCount(); ++edge) {
        if (check && (edge & 1023) == 0) check();
        if (region->getEdge(edge)->m_styleId != style)
          throw Flash::Error("Native PLI has conflicting face paints.");
      }
      nodes.push_back({region->getEdgeCount(), style, regions.size(),
                       region->getSubregionCount()});
      for (UINT child = 0; child < region->getSubregionCount(); ++child)
        regions.push_back(region->getSubregion(child));
    }
    std::vector<int> canonical(nodes.size());
    for (size_t reverse = nodes.size(); reverse > 0; --reverse) {
      const size_t index = reverse - 1;
      const auto &node   = nodes[index];
      std::vector<int> children;
      children.reserve(node.children);
      for (UINT child = 0; child < node.children; ++child)
        children.push_back(canonical[node.firstChild + child]);
      std::sort(children.begin(), children.end());
      Face key{node.edges, node.style, std::move(children)};
      auto found = canonicalFaces.find(key);
      if (found == canonicalFaces.end())
        found = canonicalFaces
                    .emplace(std::move(key), int(canonicalFaces.size()) + 1)
                    .first;
      canonical[index] = found->second;
    }
    std::vector<int> roots(canonical.begin(),
                           canonical.begin() + image->getRegionCount());
    std::sort(roots.begin(), roots.end());
    return roots;
  };
  QHash<const TRegion *, int> beforeFaces, afterFaces;
  if (faceStructure(original, beforeFaces) !=
      faceStructure(reopened, afterFaces))
    throw Flash::Error("Native PLI changed editable face structure or paint.");

  // Region count, style and child-tree shape alone cannot identify a face:
  // two same-color faces can exchange their authored contours. PLI stores the
  // stroke index and exact parameter interval of every directed region edge.
  // Match those boundaries one-to-one before attributing reopened paint to a
  // source fill. If the source keys are ambiguous, retain the strict gate.
  using BoundaryEdge = std::tuple<int, double, double, int, int>;
  using BoundaryKey  = std::vector<BoundaryEdge>;
  struct BoundedFace {
    const TRegion *region = nullptr;
    BoundaryKey edges;
    int parent = -1;
  };
  auto boundedFaces = [&](const TVectorImageP &image,
                          const QHash<const TRegion *, int> &ordinals) {
    std::vector<std::pair<const TRegion *, int>> pending;
    for (UINT root = 0; root < image->getRegionCount(); ++root)
      pending.emplace_back(image->getRegion(root), -1);
    std::vector<BoundedFace> faces;
    for (size_t index = 0; index < pending.size(); ++index) {
      if (check && (index & 1023) == 0) check();
      const auto *region = pending[index].first;
      if (!region || ordinals.value(region, -1) != int(index))
        throw Flash::Error("Native PLI changed source face enumeration.");
      BoundedFace face;
      face.region = region;
      face.parent = pending[index].second;
      face.edges.reserve(region->getEdgeCount());
      for (UINT edgeIndex = 0; edgeIndex < region->getEdgeCount();
           ++edgeIndex) {
        if (check && (edgeIndex & 1023) == 0) check();
        const auto *edge = region->getEdge(edgeIndex);
        if (!edge || !edge->m_s || !std::isfinite(edge->m_w0) ||
            !std::isfinite(edge->m_w1) || edge->m_w0 < 0 || edge->m_w0 > 1 ||
            edge->m_w1 < 0 || edge->m_w1 > 1 ||
            (edge->m_index >= 0 &&
             (edge->m_index >= int(image->getStrokeCount()) ||
              image->getStroke(edge->m_index) != edge->m_s)))
          throw Flash::Error("Native PLI has an invalid source face boundary.");
        face.edges.emplace_back(edge->m_index, edge->m_w0, edge->m_w1,
                                int(edge->m_type), edge->m_styleId);
      }
      std::sort(face.edges.begin(), face.edges.end());
      faces.push_back(std::move(face));
      for (UINT child = 0; child < region->getSubregionCount(); ++child)
        pending.emplace_back(region->getSubregion(child), int(index));
    }
    return faces;
  };
  const auto oldBoundaries = boundedFaces(original, beforeFaces);
  const auto newBoundaries = boundedFaces(reopened, afterFaces);
  if (oldBoundaries.size() != newBoundaries.size())
    throw Flash::Error("Native PLI changed source face boundaries.");
  std::map<BoundaryKey, int> oldBoundaryIds, newBoundaryIds;
  for (size_t index = 0; index < oldBoundaries.size(); ++index) {
    if (!oldBoundaryIds.emplace(oldBoundaries[index].edges, int(index))
             .second ||
        !newBoundaryIds.emplace(newBoundaries[index].edges, int(index)).second)
      throw Flash::Error("Native PLI has ambiguous source face boundaries.");
  }
  std::vector<int> oldToNew(oldBoundaries.size(), -1);
  std::vector<int> newToOld(newBoundaries.size(), -1);
  for (const auto &entry : oldBoundaryIds) {
    const auto found = newBoundaryIds.find(entry.first);
    if (found == newBoundaryIds.end())
      throw Flash::Error("Native PLI changed an authored face boundary.");
    oldToNew[entry.second]  = found->second;
    newToOld[found->second] = entry.second;
  }
  for (size_t index = 0; index < oldBoundaries.size(); ++index) {
    if (check && (index & 1023) == 0) check();
    const int oldParent = oldBoundaries[index].parent;
    const int newParent = newBoundaries[oldToNew[index]].parent;
    if (newParent != (oldParent < 0 ? -1 : oldToNew[oldParent]))
      throw Flash::Error("Native PLI changed source face nesting.");
  }
  if (audit.sourceFillByFace.size() != beforeFaces.size())
    throw Flash::Error("Native PLI source face accounting is incomplete.");
  if (audit.expectedPaintWitnesses != audit.paint.size())
    throw Flash::Error(
        "Native PLI source paint witness coverage is incomplete.");

  // A PLI-rounded contour can move a sample into its parent, child or the
  // exterior without removing either face. Defer those samples until all
  // other witnesses establish stable interiors on both sides. The face keys
  // above must match first; a nearby unrelated contour is not sufficient.
  struct ChangedWitness {
    TPointD point;
    int oldFace = -1;
    int newFace = -1;  // Original face identity mapped from the reopened face.
    int sourceFill = 0;
  };
  QVector<ChangedWitness> changedWitnesses;
  QSet<int> stableFaces;
  QHash<int, int> sourceFillForOriginalFace;
  QHash<const TRegion *, int> sourceFillForReopenedFace;
  QHash<int, QSet<int>> originalFacesByFill;
  QHash<int, QSet<const TRegion *>> reopenedFacesByFill;
  for (int index = 0; index < audit.paint.size(); ++index) {
    if (check && (index & 1023) == 0) check();
    const auto &sample = audit.paint[index];
    if (!std::isfinite(sample.point.x) || !std::isfinite(sample.point.y) ||
        sample.sourceFillIndex < 0 || sample.nativeFace < -1 ||
        sample.nativeFace >= audit.sourceFillByFace.size() ||
        sample.nativePaletteStyleIndex < 0 ||
        sample.nativePaletteStyleIndex >= afterPalette->getStyleCount() ||
        (sample.sourceFillIndex == 0) != (sample.nativePaletteStyleIndex == 0))
      throw Flash::Error("Native PLI has an invalid source paint witness.");
    const auto *oldFace = original->getRegion(sample.point);
    const auto *newFace = reopened->getRegion(sample.point);
    const int oldFaceId = oldFace ? beforeFaces.value(oldFace, -2) : -1;
    const int newFaceId = newFace ? afterFaces.value(newFace, -2) : -1;
    if (oldFaceId != sample.nativeFace || newFaceId == -2 ||
        (oldFace ? oldFace->getEdge(0)->m_styleId : 0) !=
            sample.nativePaletteStyleIndex)
      throw Flash::Error(
          "Native PLI has an invalid source paint witness face.");
    if (sample.nativeFace >= 0 &&
        sample.sourceFillIndex != audit.sourceFillByFace[sample.nativeFace])
      throw Flash::Error("Native PLI changed an authored face fill identity.");
    const int mappedNewFace = newFaceId < 0 ? -1 : newToOld[newFaceId];
    const int reopenedStyle = newFace ? newFace->getEdge(0)->m_styleId : 0;
    const int reopenedFill =
        mappedNewFace < 0 ? 0 : audit.sourceFillByFace[mappedNewFace];
    if (reopenedStyle != sample.nativePaletteStyleIndex ||
        reopenedFill != sample.sourceFillIndex ||
        (sample.nativeFace < 0 && newFace) ||
        (sample.nativeFace >= 0 && !newFace)) {
      if (changedWitnesses.size() >= 256)
        throw Flash::Error("Native PLI has too many changed paint witnesses.");
      changedWitnesses.push_back({sample.point, sample.nativeFace,
                                  mappedNewFace, sample.sourceFillIndex});
      continue;
    }
    if (sample.nativeFace < 0) {
      stableFaces.insert(-1);
      continue;
    }
    if (sample.nativeFace >= 0) {
      if (!newFace || !afterFaces.contains(newFace) ||
          (sourceFillForOriginalFace.contains(sample.nativeFace) &&
           sourceFillForOriginalFace.value(sample.nativeFace) !=
               sample.sourceFillIndex) ||
          (sourceFillForReopenedFace.contains(newFace) &&
           sourceFillForReopenedFace.value(newFace) != sample.sourceFillIndex))
        throw Flash::Error(
            QString("Native PLI merged or reassigned source fill %1 at "
                    "paint witness (%2, %3).")
                .arg(sample.sourceFillIndex)
                .arg(sample.point.x, 0, 'g', 17)
                .arg(sample.point.y, 0, 'g', 17));
      if (mappedNewFace == sample.nativeFace)
        stableFaces.insert(sample.nativeFace);
      sourceFillForOriginalFace.insert(sample.nativeFace,
                                       sample.sourceFillIndex);
      sourceFillForReopenedFace.insert(newFace, sample.sourceFillIndex);
      originalFacesByFill[sample.sourceFillIndex].insert(sample.nativeFace);
      reopenedFacesByFill[sample.sourceFillIndex].insert(newFace);
    }
  }
  // A quadratic Bezier moves by at most the maximum displacement of its three
  // controls at equal parameter values. Permit a changed sample only across
  // one complete closed contour separating adjacent, independently witnessed
  // faces, and only inside that contour's measured PLI displacement band.
  // Sixteen codec grid units are below 0.003 source pixel at 120 DPI; larger
  // shifts stay explicit failures even when the face topology survives.
  constexpr double maxBoundaryDrift = 16.0 / 16384.0;
  constexpr double numericalSlack   = 1e-8;
  quint64 probedChunks              = 0;
  for (const auto &sample : changedWitnesses) {
    if (check) check();
    int separator = -1;
    if (sample.oldFace >= 0 && sample.newFace >= 0) {
      if (oldBoundaries[sample.oldFace].parent == sample.newFace)
        separator = sample.oldFace;
      else if (oldBoundaries[sample.newFace].parent == sample.oldFace)
        separator = sample.newFace;
    } else if (sample.oldFace < 0 && sample.newFace >= 0 &&
               oldBoundaries[sample.newFace].parent < 0)
      separator = sample.newFace;
    else if (sample.newFace < 0 && sample.oldFace >= 0 &&
             oldBoundaries[sample.oldFace].parent < 0)
      separator = sample.oldFace;
    bool certified = separator >= 0 && stableFaces.contains(sample.oldFace) &&
                     stableFaces.contains(sample.newFace);
    const TRegion *boundary =
        certified ? oldBoundaries[separator].region : nullptr;
    if (certified && boundary && boundary->getEdgeCount() == 1) {
      const auto *edge      = boundary->getEdge(0);
      const int strokeIndex = edge->m_index;
      certified             = strokeIndex >= 0 &&
                  strokeIndex < int(original->getStrokeCount()) &&
                  ((edge->m_w0 == 0 && edge->m_w1 == 1) ||
                   (edge->m_w0 == 1 && edge->m_w1 == 0));
      if (certified) {
        const auto *before = original->getStroke(strokeIndex);
        const auto *after  = reopened->getStroke(strokeIndex);
        certified          = before->isSelfLoop() && after->isSelfLoop() &&
                    before->getChunkCount() > 0;
        if (certified) {
          probedChunks += quint64(before->getChunkCount()) * 2;
          if (probedChunks > 1000000)
            throw Flash::Error(
                "Native PLI paint-boundary proof budget exceeded.");
          double maxDrift = 0;
          for (int control = 0; control < before->getControlPointCount();
               ++control) {
            const auto a = before->getControlPoint(control);
            const auto b = after->getControlPoint(control);
            maxDrift     = std::max(maxDrift, std::hypot(a.x - b.x, a.y - b.y));
          }
          certified = maxDrift > 0 && maxDrift <= maxBoundaryDrift;
          if (certified) {
            double oldT = 0, newT = 0, oldDistance2 = 0, newDistance2 = 0;
            int oldChunk = -1, newChunk = -1;
            certified = before->getNearestChunk(sample.point, oldT, oldChunk,
                                                oldDistance2, false) &&
                        after->getNearestChunk(sample.point, newT, newChunk,
                                               newDistance2, false) &&
                        std::sqrt(oldDistance2) <= maxDrift + numericalSlack &&
                        std::sqrt(newDistance2) <= maxDrift + numericalSlack;
          }
        }
      }
    } else
      certified = false;
    if (!certified)
      throw Flash::Error(
          QString("Native PLI lost source fill %1 at paint witness (%2, %3).")
              .arg(sample.sourceFill)
              .arg(sample.point.x, 0, 'g', 17)
              .arg(sample.point.y, 0, 'g', 17));
  }
  for (auto it = beforeFaces.cbegin(); it != beforeFaces.cend(); ++it) {
    if (check && (it.value() & 1023) == 0) check();
    const int sourceFill = audit.sourceFillByFace[it.value()];
    const int style      = it.key()->getEdge(0)->m_styleId;
    if (sourceFill < 0 || (sourceFill == 0) != (style == 0))
      throw Flash::Error(
          "Native PLI has an invalid source face fill identity.");
    if (style > 0 && !sourceFillForOriginalFace.contains(it.value()))
      throw Flash::Error("Native PLI has an unwitnessed painted source face.");
  }
  for (auto it = originalFacesByFill.cbegin(); it != originalFacesByFill.cend();
       ++it)
    if (it.value().size() != reopenedFacesByFill.value(it.key()).size())
      throw Flash::Error(
          QString("Native PLI changed the witnessed face count of source fill "
                  "%1.")
              .arg(it.key()));
}

}  // namespace FlashNative
