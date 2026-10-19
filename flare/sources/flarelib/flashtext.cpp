#include "flashtext.h"

#include "../flash/model.h"
// The Text FX layout (header only), so imported text is placed exactly as
// the FX lays it out.
#include "../stdfx/iwa_textfxlayout.h"
#include "tpalette.h"
#include "tregion.h"
#include "tstroke.h"
#include "tcolorstyles.h"

#include <QFontDatabase>
#include <QGlyphRun>
#include <QHash>
#include <QLineF>
#include <QMutex>
#include <QPainterPath>
#include <QRawFont>
#include <QTextLayout>
#include <QTransform>
#include <algorithm>
#include <cstring>
#include <cmath>
#include <limits>
#include <vector>

namespace FlashNative {
namespace {

// Text is laid out at 20 times its source size (twips) so that Qt's integer
// pixel font sizes keep Flash's fractional sizes and positions.
constexpr double layoutScale = 20.0;

struct ResolvedFont {
  QString family;
  int weight       = -1;  // -1 keeps the bold flag
  bool italic      = false;
  bool substituted = false;
};

QString splitCamelCase(const QString &name) {
  QString result;
  for (int i = 0; i < name.size(); ++i) {
    if (i > 0 && name[i].isUpper() && name[i - 1].isLower()) result += ' ';
    result += name[i];
  }
  return result;
}

// Flash stores PostScript names ("Arial-Black", "TimesNewRomanPSMT",
// "ArialMT", "MyriadPro-Regular") and sometimes Windows family names with a
// script suffix ("Arial Baltic"). Map them to an installed family.
ResolvedFont resolveFont(const QString &face) {
  static QMutex mutex;
  static QHash<QString, ResolvedFont> cache;
  QMutexLocker lock(&mutex);
  const auto cached = cache.constFind(face);
  if (cached != cache.cend()) return cached.value();

  const QStringList families = QFontDatabase().families();
  auto installed             = [&](const QString &name) {
    for (const auto &family : families)
      if (family.compare(name, Qt::CaseInsensitive) == 0) return family;
    return QString();
  };
  ResolvedFont result;
  QString name   = face.trimmed(), style;
  const int dash = name.indexOf('-');
  if (dash > 0) {
    style = name.mid(dash + 1);
    name  = name.left(dash);
  }
  for (const char *suffix : {"PSMT", "PS", "MT"}) {
    if (name.endsWith(suffix) && name.size() > int(strlen(suffix)) + 1)
      name.chop(int(strlen(suffix)));
    if (style.endsWith(suffix)) style.chop(int(strlen(suffix)));
  }
  const QString spacedStyle = splitCamelCase(style);
  if (spacedStyle.contains("Italic") || spacedStyle.contains("Oblique"))
    result.italic = true;
  const QStringList weights{"Thin",     "Light", "Regular", "Medium",
                            "Semibold", "Bold",  "Black"};
  const int qtWeights[] = {QFont::Thin,   QFont::Light,    QFont::Normal,
                           QFont::Medium, QFont::DemiBold, QFont::Bold,
                           QFont::Black};
  for (int i = 0; i < weights.size(); ++i)
    if (spacedStyle.contains(weights[i])) result.weight = qtWeights[i];

  QStringList candidates{face.trimmed()};
  QStringList words = splitCamelCase(name).split(' ', QString::SkipEmptyParts);
  if (!spacedStyle.isEmpty()) candidates << words.join(' ') + " " + spacedStyle;
  for (; !words.isEmpty(); words.removeLast()) candidates << words.join(' ');
  for (const auto &candidate : candidates) {
    result.family = installed(candidate);
    if (result.family.isEmpty()) continue;
    // A family that names the style already ("Arial Black") needs no weight.
    if (candidate.endsWith(spacedStyle) && !spacedStyle.isEmpty() &&
        candidate != face.trimmed())
      result.weight = -1;
    break;
  }
  if (result.family.isEmpty()) {
    result.substituted = true;
    for (const auto &fallback :
         {QFontDatabase::systemFont(QFontDatabase::GeneralFont).family(),
          QString("Arial"), QString("Segoe UI"), QString("Helvetica"),
          QString("DejaVu Sans"), QString("Liberation Sans")}) {
      result.family = installed(fallback);
      if (!result.family.isEmpty()) break;
    }
    if (result.family.isEmpty() && !families.isEmpty())
      result.family = families.front();
  }
  cache.insert(face, result);
  return result;
}

class Colors {
  TPalette *m_palette;
  QHash<quint32, int> m_styles;

  static quint32 key(const TPixel32 &color) {
    return (quint32(color.r) << 24) | (quint32(color.g) << 16) |
           (quint32(color.b) << 8) | color.m;
  }

public:
  explicit Colors(TPalette *palette) : m_palette(palette) {
    for (int index = 1; index < palette->getStyleCount(); ++index) {
      const TColorStyle *style = palette->getStyle(index);
      // Plain solid colors only (TSolidColorStyle tag).
      if (style && style->getTagId() == 3)
        m_styles.insert(key(style->getMainColor()), index);
    }
  }
  int style(const Flash::Color &color) {
    const TPixel32 pixel(color.red, color.green, color.blue, color.alpha);
    const auto found = m_styles.constFind(key(pixel));
    if (found != m_styles.cend()) return found.value();
    const int index = m_palette->addStyle(pixel);
    if (index < 0) throw Flash::Error("Cannot create an imported text color.");
    if (m_palette->getPageCount() == 0) m_palette->addPage(L"Imported colors");
    m_palette->getPage(0)->addStyle(index);
    m_styles.insert(key(pixel), index);
    return index;
  }
};

// One laid-out run of glyph outlines in source pixels (y down).
struct Outline {
  int run = 0;
  QPainterPath path;
};

// Converts closed glyph contours (source pixels) into native strokes.
class StrokeBuilder {
  TVectorImage &m_image;
  double m_scale;
  int m_style;
  std::vector<TThickPoint> m_points;
  QPointF m_last;

  TThickPoint native(const QPointF &point) const {
    return TThickPoint(point.x() * m_scale, -point.y() * m_scale, 0);
  }
  void quadratic(const QPointF &control, const QPointF &end) {
    if (QLineF(m_last, end).length() < 1e-4 &&
        QLineF(m_last, control).length() < 1e-4)
      return;
    m_points.push_back(native(control));
    m_points.push_back(native(end));
    m_last = end;
  }
  void cubic(const QPointF &p0, const QPointF &c1, const QPointF &c2,
             const QPointF &p3, int depth) {
    // Distance between the cubic and its best single quadratic.
    const QPointF error = p3 - 3 * c2 + 3 * c1 - p0;
    const double size = std::hypot(error.x(), error.y()) * std::sqrt(3.0) / 36;
    if (size <= 0.005 || depth >= 6) {
      quadratic((3 * (c1 + c2) - (p0 + p3)) / 4, p3);
      return;
    }
    const QPointF a = (p0 + c1) / 2, b = (c1 + c2) / 2, c = (c2 + p3) / 2;
    const QPointF d = (a + b) / 2, e = (b + c) / 2, m = (d + e) / 2;
    cubic(p0, a, d, m, depth + 1);
    cubic(m, e, c, p3, depth + 1);
  }

public:
  StrokeBuilder(TVectorImage &image, double scale, int style)
      : m_image(image), m_scale(scale), m_style(style) {}

  void start(const QPointF &point) {
    finish();
    m_points.push_back(native(point));
    m_last = point;
  }
  void line(const QPointF &end) { quadratic((m_last + end) / 2, end); }
  void curve(const QPointF &c1, const QPointF &c2, const QPointF &end) {
    cubic(m_last, c1, c2, end, 0);
  }
  void finish() {
    if (m_points.size() >= 5) {
      // Glyph contours are closed; join a residual gap exactly.
      const TThickPoint first = m_points.front();
      if (tdistance(TPointD(first.x, first.y),
                    TPointD(m_points.back().x, m_points.back().y)) >
          1e-6 * std::max(1.0, m_scale)) {
        const TThickPoint back = m_points.back();
        m_points.push_back(
            TThickPoint((back.x + first.x) / 2, (back.y + first.y) / 2, 0));
      }
      m_points.back() = first;
      auto *stroke    = new TStroke(m_points);
      stroke->setStyle(m_style);
      stroke->setSelfLoop(true);
      if (m_image.addStroke(stroke, false) < 0) {
        delete stroke;
        throw Flash::Error("Cannot create an imported text stroke.");
      }
    }
    m_points.clear();
  }
};

void appendStrokes(TVectorImage &image, const QPainterPath &path, double scale,
                   int style) {
  StrokeBuilder builder(image, scale, style);
  for (int i = 0; i < path.elementCount(); ++i) {
    const auto element = path.elementAt(i);
    switch (element.type) {
    case QPainterPath::MoveToElement:
      builder.start(element);
      break;
    case QPainterPath::LineToElement:
      builder.line(element);
      break;
    case QPainterPath::CurveToElement:
      if (i + 2 >= path.elementCount())
        throw Flash::Error("Incomplete glyph outline curve.");
      builder.curve(element, path.elementAt(i + 1), path.elementAt(i + 2));
      i += 2;
      break;
    default:
      break;
    }
  }
  builder.finish();
}

// A point inside `region` but outside its subregions, if one is found.
bool interiorPoint(const TRegion *region, TPointD &point) {
  const TRectD box = region->getBBox();
  if (!(box.y1 > box.y0)) return false;
  for (const double fraction :
       {0.5, 0.25, 0.75, 0.375, 0.625, 0.125, 0.875, 0.0625, 0.9375, 0.3125,
        0.6875, 0.1875, 0.8125, 0.4375, 0.5625}) {
    const double y = box.y0 + (box.y1 - box.y0) * fraction;
    std::vector<double> crossings;
    region->computeScanlineIntersections(y, crossings);
    for (UINT child = 0; child < region->getSubregionCount(); ++child) {
      // The native scanline helper requires an empty output vector.
      std::vector<double> inner;
      region->getSubregion(child)->computeScanlineIntersections(y, inner);
      crossings.insert(crossings.end(), inner.begin(), inner.end());
    }
    std::sort(crossings.begin(), crossings.end());
    // Prefer the widest interval for a robust sample.
    double bestWidth = 0;
    for (size_t i = 1; i < crossings.size(); ++i) {
      const double width = crossings[i] - crossings[i - 1];
      if (width <= bestWidth) continue;
      const TPointD candidate((crossings[i] + crossings[i - 1]) / 2, y);
      if (!region->contains(candidate)) continue;
      bool inChild = false;
      for (UINT child = 0; child < region->getSubregionCount() && !inChild;
           ++child)
        inChild = region->getSubregion(child)->contains(candidate);
      if (inChild) continue;
      bestWidth = width;
      point     = candidate;
    }
    if (bestWidth > 0) return true;
  }
  return false;
}

// Paints every face, in the breadth-first face order that the PLI
// persistence check enumerates, and records a witness for each painted face.
void fillRegions(const TVectorImageP &image,
                 const std::vector<Outline> &outlines,
                 const std::vector<int> &styles, double scale,
                 VectorFrameAudit *audit) {
  std::vector<TRegion *> regions;
  for (UINT index = 0; index < image->getRegionCount(); ++index)
    regions.push_back(image->getRegion(index));
  for (size_t index = 0; index < regions.size(); ++index) {
    TRegion *region = regions[index];
    TPointD point;
    int style = 0, fill = 0;
    if (interiorPoint(region, point)) {
      const QPointF source(point.x / scale, -point.y / scale);
      // Later runs are drawn over earlier ones.
      for (size_t i = outlines.size(); i-- > 0;)
        if (outlines[i].path.contains(source)) {
          style = styles[outlines[i].run];
          fill  = outlines[i].run + 1;
          break;
        }
    }
    region->setStyle(style);
    if (audit) {
      audit->sourceFillByFace.push_back(style ? fill : 0);
      if (style) audit->paint.push_back({point, fill, style, int(index)});
    }
    for (UINT child = 0; child < region->getSubregionCount(); ++child)
      regions.push_back(region->getSubregion(child));
  }
}

struct Paragraph {
  QString text;
  struct Piece {
    int start = 0, length = 0, run = 0;
  };
  QVector<Piece> pieces;
  int formatRun = 0;  // run whose paragraph format applies
};

QFont runFont(const Flash::TextRunFormat &format,
              const ResolvedFont &resolved) {
  QFont font(resolved.family);
  font.setPixelSize(std::max(1, int(std::lround(format.size * layoutScale))));
  int weight = resolved.weight >= 0 ? resolved.weight : int(QFont::Normal);
  if (format.bold) weight = std::max<int>(weight, QFont::Bold);
  font.setWeight(weight);
  font.setItalic(format.italic || resolved.italic);
  font.setKerning(format.autoKern);
  font.setHintingPreference(QFont::PreferNoHinting);
  font.setStyleStrategy(
      QFont::StyleStrategy(QFont::ForceOutline | QFont::PreferQuality));
  if (format.letterSpacing != 0)
    font.setLetterSpacing(QFont::AbsoluteSpacing,
                          format.letterSpacing * layoutScale);
  return font;
}

Qt::Alignment alignment(const QString &value) {
  if (value == "center") return Qt::AlignHCenter;
  if (value == "right") return Qt::AlignRight;
  if (value == "justify") return Qt::AlignJustify;
  return Qt::AlignLeft;
}

// Installed fonts per run, with one note per substituted face.
QVector<ResolvedFont> resolveFonts(const Flash::TextLayoutSource &source,
                                   QStringList *approximations) {
  QVector<ResolvedFont> fonts;
  QStringList substituted;
  for (const auto &run : source.runs) {
    fonts.push_back(resolveFont(run.face));
    if (fonts.back().family.isEmpty())
      throw Flash::Error("No font is available to draw text.");
    if (fonts.back().substituted && !substituted.contains(run.face)) {
      substituted << run.face;
      if (approximations)
        *approximations << QString("Font \"%1\" substituted by \"%2\".")
                               .arg(run.face, fonts.back().family);
    }
  }
  return fonts;
}

// Flash separates paragraphs with CR (LF and CRLF are accepted too).
QVector<Paragraph> splitParagraphs(const Flash::TextLayoutSource &source) {
  QVector<Paragraph> paragraphs(1);
  for (int run = 0; run < source.runs.size(); ++run) {
    const QString &characters = source.runs[run].characters;
    if (paragraphs.back().text.isEmpty() && paragraphs.back().pieces.isEmpty())
      paragraphs.back().formatRun = run;
    for (int i = 0; i < characters.size(); ++i) {
      const QChar c = characters[i];
      if (c == '\r' || c == '\n') {
        if (c == '\r' && i + 1 < characters.size() && characters[i + 1] == '\n')
          ++i;
        paragraphs.push_back({});
        paragraphs.back().formatRun = run;
        continue;
      }
      auto &paragraph = paragraphs.back();
      if (paragraph.pieces.isEmpty() || paragraph.pieces.back().run != run)
        paragraph.pieces.push_back({paragraph.text.size(), 0, run});
      paragraph.text += c == '\t' ? QChar(' ') : c;
      ++paragraph.pieces.back().length;
    }
  }
  return paragraphs;
}

// The end of text[start, end) without its trailing whitespace.
int trimmedEnd(const QString &text, int start, int end) {
  while (end > start && text[end - 1].isSpace()) --end;
  return end;
}

// Line spacing of text whose attributes omit it: Animate leaves out its
// default of 2 px. Published SWFs (lines of static text 2 px further apart
// than the font's ascent plus descent) and the box heights Animate saves for
// multi-line text (n line heights plus 2 px between lines) agree.
constexpr double defaultLineSpacing = 2;

// Lays the paragraphs out as Flash does and calls
// visit(paragraph, line, top, snap) for every line, `top` being the line top
// in source pixels. Line coordinates are source pixels times layoutScale.
// With `snap` (static text), every baseline and line start lies on a whole
// pixel of the text box, as in the DefineText records Animate publishes;
// `snap` passed to visit() is that rounding (source pixels), already
// applied to the line and to `top`.
template <class Visit>
void layOutLines(const Flash::TextLayoutSource &source,
                 const QVector<ResolvedFont> &fonts,
                 const QVector<Paragraph> &paragraphs, bool snap,
                 Visit &&visit) {
  double offset = 0;  // Line top below the box top, before rounding
  for (const auto &paragraph : paragraphs) {
    const auto &format = source.runs[paragraph.formatRun];
    QTextLayout layout(paragraph.text,
                       runFont(format, fonts[paragraph.formatRun]));
    QVector<QTextLayout::FormatRange> ranges;
    for (const auto &piece : paragraph.pieces) {
      QTextLayout::FormatRange range;
      range.start  = piece.start;
      range.length = piece.length;
      range.format.setFont(runFont(source.runs[piece.run], fonts[piece.run]));
      ranges.push_back(range);
    }
    layout.setFormats(ranges);
    QTextOption option;
    const bool boxed = source.width > 0;
    option.setAlignment(boxed ? alignment(format.alignment) : Qt::AlignLeft);
    option.setWrapMode(source.wrap && boxed ? QTextOption::WordWrap
                                            : QTextOption::NoWrap);
    option.setUseDesignMetrics(true);
    layout.setTextOption(option);
    layout.setCacheEnabled(true);
    layout.beginLayout();
    for (bool first = true;; first = false) {
      QTextLine line = layout.createLine();
      if (!line.isValid()) break;
      const double indent = first ? format.indent : 0;
      const double available =
          source.width - format.leftMargin - format.rightMargin - indent;
      line.setLineWidth(boxed && available > 0 ? available * layoutScale : 1e7);
      line.setPosition(
          QPointF((source.left + format.leftMargin + indent) * layoutScale, 0));
    }
    layout.endLayout();
    const Qt::Alignment aligned = option.alignment();
    for (int index = 0; index < layout.lineCount(); ++index) {
      QTextLine line       = layout.lineAt(index);
      const double ascent  = line.ascent() / layoutScale;
      const double natural = (line.ascent() + line.descent()) / layoutScale;
      // Flash centers and right-aligns a line by its width without trailing
      // spaces (published DefineText records); Qt counts the spaces that end
      // a paragraph when they fit the line.
      if ((aligned & (Qt::AlignHCenter | Qt::AlignRight)) &&
          line.width() < 1e6) {
        const int end      = line.textStart() + line.textLength();
        const int inked    = trimmedEnd(paragraph.text, line.textStart(), end);
        const double start = line.cursorToX(line.textStart());
        const double width = line.cursorToX(inked) - start;
        const double wanted =
            line.x() + (aligned & Qt::AlignRight ? line.width() - width
                                                 : (line.width() - width) / 2);
        line.setPosition(line.position() + QPointF(wanted - start, 0));
      }
      QPointF rounding;
      if (snap) {
        const double start =
            line.cursorToX(line.textStart()) / layoutScale - source.left;
        rounding = QPointF(std::floor(start + 0.5) - start,
                           std::floor(offset + ascent + 0.5) - offset - ascent);
        line.setPosition(line.position() +
                         QPointF(rounding.x() * layoutScale, 0));
      }
      visit(paragraph, line, source.top + offset + rounding.y(), rounding);
      // Line advance: ascent plus descent plus the line spacing. (Animate's
      // lineHeight attribute is its own measure of ascent plus descent.)
      offset += natural + (format.hasLineSpacing ? format.lineSpacing
                                                 : defaultLineSpacing);
    }
  }
}

}  // namespace

TVectorImageP textImage(const Flash::Element &element, TPalette *palette,
                        double stageUnitsPerPixel, QStringList *approximations,
                        VectorFrameAudit *audit) {
  if (!element.text)
    throw Flash::Error("Text element has no structured text runs.");
  if (!palette || !std::isfinite(stageUnitsPerPixel) || stageUnitsPerPixel <= 0)
    throw Flash::Error("Invalid native text palette or coordinate scale.");
  const Flash::TextLayoutSource source = Flash::textLayout(element);
  TVectorImageP image(new TVectorImage());
  image->setPalette(palette);
  if (source.runs.isEmpty()) return image;

  const QVector<ResolvedFont> fonts   = resolveFonts(source, approximations);
  const QVector<Paragraph> paragraphs = splitParagraphs(source);

  std::vector<Outline> outlines;
  layOutLines(
      source, fonts, paragraphs, element.tag == "DOMStaticText",
      [&](const Paragraph &paragraph, const QTextLine &line, double top,
          const QPointF &) {
        const double ascent = line.ascent() / layoutScale;
        QTransform toSource;
        toSource.translate(0, top + ascent);
        toSource.scale(1 / layoutScale, 1 / layoutScale);
        toSource.translate(0, -line.ascent());
        for (const auto &piece : paragraph.pieces) {
          const int from = std::max(piece.start, line.textStart());
          const int to   = std::min(piece.start + piece.length,
                                    line.textStart() + line.textLength());
          if (from >= to) continue;
          Outline outline;
          outline.run = piece.run;
          outline.path.setFillRule(Qt::WindingFill);
          const auto &runFormat = source.runs[piece.run];
          for (const QGlyphRun &glyphs : line.glyphRuns(from, to - from)) {
            const QRawFont font  = glyphs.rawFont();
            const auto indexes   = glyphs.glyphIndexes();
            const auto positions = glyphs.positions();
            // Oblique when the chosen face has no italic design.
            const bool synthesizeItalic =
                (runFormat.italic || fonts[piece.run].italic) &&
                font.style() == QFont::StyleNormal;
            for (int g = 0; g < indexes.size() && g < positions.size(); ++g) {
              QPainterPath glyph = font.pathForGlyph(indexes[g]);
              if (glyph.isEmpty()) continue;
              if (synthesizeItalic)
                glyph = QTransform(1, 0, -0.2, 1, 0, 0).map(glyph);
              glyph.translate(positions[g]);
              outline.path.addPath(toSource.map(glyph));
            }
          }
          if (!outline.path.isEmpty()) outlines.push_back(std::move(outline));
        }
      });
  if (outlines.empty()) return image;

  Colors colors(palette);
  std::vector<int> styles;
  for (const auto &run : source.runs) styles.push_back(colors.style(run.fill));
  for (const auto &outline : outlines) {
    const double limit = (double(std::numeric_limits<int>::max()) - 1.0) /
                         16384.0 / stageUnitsPerPixel;
    const QRectF bounds = outline.path.boundingRect();
    if (!std::isfinite(bounds.left()) || !std::isfinite(bounds.right()) ||
        std::abs(bounds.left()) > limit || std::abs(bounds.right()) > limit ||
        std::abs(bounds.top()) > limit || std::abs(bounds.bottom()) > limit)
      throw Flash::Error("Text outline exceeds native coordinate limits.");
    appendStrokes(*image, outline.path, stageUnitsPerPixel,
                  styles[outline.run]);
  }
  image->findRegions();
  VectorFrameAudit converted;
  fillRegions(image, outlines, styles, stageUnitsPerPixel,
              audit ? &converted : nullptr);
  if (audit) {
    converted.sourceId               = element.sourceId;
    converted.expectedPaintWitnesses = converted.paint.size();
    *audit                           = std::move(converted);
  }
  return image;
}

namespace {

bool hasInk(const QString &text) {
  for (const QChar c : text)
    if (!c.isSpace()) return true;
  return false;
}

// The Text FX font of a run. Its pixel size is the run size in FX length
// units, rounded; `columnScale` restores the exact size. Kerning and letter
// spacing are layout options of the FX, not part of its font parameter.
QFont fxFont(const Flash::TextRunFormat &format, const ResolvedFont &resolved,
             double stageUnitsPerPixel, double &columnScale) {
  const double size = format.size * stageUnitsPerPixel;
  const int pixels  = std::max(1, int(std::lround(size)));
  columnScale       = size / pixels;
  QFont font(resolved.family);
  font.setPixelSize(pixels);
  int weight = resolved.weight >= 0 ? resolved.weight : int(QFont::Normal);
  if (format.bold) weight = std::max<int>(weight, QFont::Bold);
  font.setWeight(weight);
  font.setItalic(format.italic || resolved.italic);
  return font;
}

TPixel32 pixel(const Flash::Color &color) {
  return TPixel32(color.red, color.green, color.blue, color.alpha);
}

// Room around the laid-out text in a Text FX box (layout pixels), so that
// slightly different font metrics elsewhere never make the FX wrap it.
double slack(double size) {
  return std::max<double>(Iwa_TextFxLayout::transformedScale, 0.05 * size);
}

// Glyph origins of laid-out text.
struct Glyphs {
  QVector<quint32> indexes;
  QVector<QPointF> positions;
};

// The glyphs of `line` (of its characters [from, from + length) when given),
// their positions mapped by `toSource`.
Glyphs glyphs(const QTextLine &line, const QTransform &toSource, int from = -1,
              int length = -1) {
  Glyphs result;
  for (const QGlyphRun &run : line.glyphRuns(from, length)) {
    result.indexes += run.glyphIndexes();
    for (const QPointF &position : run.positions())
      result.positions << toSource.map(position);
  }
  return result;
}

// The largest distance between matching glyphs; -1 when the glyphs differ.
double deviation(const Glyphs &a, const Glyphs &b) {
  if (a.indexes != b.indexes || a.positions.size() != b.positions.size())
    return -1;
  double result = 0;
  for (int i = 0; i < a.positions.size(); ++i)
    result = std::max(result, QLineF(a.positions[i], b.positions[i]).length());
  return result;
}

// Text FX glyphs farther than this from Flash's (source pixels) are noted:
// half a pixel, the rounding of Flash's static text lines, which the
// continuous FX layout splits between its lines.
constexpr double glyphTolerance = 0.5 + 1e-6;

struct FxLine {
  int paragraph = 0;
  QTextLine line;
};

// The lines of a Text FX layout in order.
QVector<FxLine> fxLines(const Iwa_TextFxLayout::Block &block) {
  QVector<FxLine> lines;
  for (int paragraph = 0; paragraph < block.paragraphCount(); ++paragraph)
    for (int index = 0; index < block.paragraph(paragraph).lineCount(); ++index)
      lines.push_back({paragraph, block.paragraph(paragraph).lineAt(index)});
  return lines;
}

QString excerpt(QString text) {
  text.replace('\n', ' ');
  return text.size() > 32 ? text.left(29) + "..." : text;
}

}  // namespace

QVector<TextFxItem> textFxItems(const Flash::Element &element,
                                double stageUnitsPerPixel,
                                QStringList *approximations) {
  if (!element.text)
    throw Flash::Error("Text element has no structured text runs.");
  if (!std::isfinite(stageUnitsPerPixel) || stageUnitsPerPixel <= 0)
    throw Flash::Error("Invalid native text coordinate scale.");
  const Flash::TextLayoutSource source = Flash::textLayout(element);
  QVector<TextFxItem> items;
  if (source.runs.isEmpty()) return items;
  const double unit = stageUnitsPerPixel;
  // Text FX layout pixels per FX length unit ("Transform with Column").
  const double scale                  = Iwa_TextFxLayout::transformedScale;
  const QVector<ResolvedFont> fonts   = resolveFonts(source, nullptr);
  const QVector<Paragraph> paragraphs = splitParagraphs(source);
  QStringList notes;
  auto note = [&](const QString &text) {
    if (!notes.contains(text)) notes << text;
  };
  // Flash glyphs (source pixels) against the FX glyphs.
  auto check = [&](const QString &text, double distance) {
    if (distance < 0)
      note(QString("The Text FX draws \"%1\" with other glyphs than Flash.")
               .arg(excerpt(text)));
    else if (distance > glyphTolerance)
      note(QString("The Text FX draws \"%1\" up to %2 px from its Flash "
                   "glyph positions.")
               .arg(excerpt(text))
               .arg(distance, 0, 'f', 2));
  };
  // Flash's spacing as FX layout options, in the FX's column space.
  auto spacing = [&](TextFxItem &item, const Flash::TextRunFormat &format) {
    item.kerning       = format.autoKern;
    item.letterSpacing = format.letterSpacing * unit / item.columnScale;
    item.shrinkToFit   = false;  // Flash text never shrinks
  };
  auto options = [&](const TextFxItem &item) {
    Iwa_TextFxLayout::Options result;
    result.hAlign        = item.alignment;
    result.kerning       = item.kerning;
    result.letterSpacing = item.letterSpacing * scale;
    result.lineSpacing   = item.lineSpacing * scale;
    result.indent        = item.indent * scale;
    return result;
  };
  // Width of a Text FX box that holds every line unwrapped.
  auto unwrapped = [&](const TextFxItem &item, const QFont &font) {
    Iwa_TextFxLayout::Options natural = options(item);
    natural.hAlign                    = Qt::AlignLeft;
    const double right =
        Iwa_TextFxLayout::Block(item.text, font, 1e6, natural).rect().right();
    return right + slack(right);
  };

  // Text FX fonts of every run; single style when all runs that draw
  // something share one font, color and spacing.
  QVector<QString> fontKeys(source.runs.size());
  int styleRun     = -1;
  bool singleStyle = true;
  for (int run = 0; run < source.runs.size(); ++run) {
    const auto &format = source.runs[run];
    double columnScale = 1;
    fontKeys[run] = fxFont(format, fonts[run], unit, columnScale).toString();
    if (!hasInk(format.characters)) continue;
    if (styleRun < 0) {
      styleRun = run;
      continue;
    }
    const auto &style = source.runs[styleRun];
    singleStyle &= fontKeys[run] == fontKeys[styleRun] &&
                   pixel(format.fill) == pixel(style.fill) &&
                   format.letterSpacing == style.letterSpacing &&
                   format.autoKern == style.autoKern;
  }
  if (styleRun < 0) return items;  // Nothing visible
  for (const auto &paragraph : paragraphs) {
    const auto &a = source.runs[paragraph.formatRun];
    const auto &b = source.runs[paragraphs.front().formatRun];
    singleStyle &= a.alignment == b.alignment && a.leftMargin == b.leftMargin &&
                   a.rightMargin == b.rightMargin && a.indent == b.indent &&
                   a.hasLineSpacing == b.hasLineSpacing &&
                   a.lineSpacing == b.lineSpacing &&
                   a.lineHeight == b.lineHeight;
  }

  // The Flash layout, source pixels.
  struct Line {
    int paragraph = 0, start = 0;
    int end    = 0;  // Line end without trailing spaces
    double top = 0, ascent = 0;
    double x = 0;      // Pen position of the line start
    QPointF rounding;  // Whole-pixel rounding of the line (included above)
    Glyphs glyphs;     // Single-style text only
  };
  QVector<Line> lines;
  struct Segment {
    int from = 0, to = 0, run = -1;
    bool ink = false;
  };
  layOutLines(
      source, fonts, paragraphs, element.tag == "DOMStaticText",
      [&](const Paragraph &paragraph, const QTextLine &line, double top,
          const QPointF &rounding) {
        const QTransform toSource(1 / layoutScale, 0, 0, 1 / layoutScale, 0,
                                  top);
        Line record;
        record.paragraph = int(&paragraph - paragraphs.constData());
        record.start     = line.textStart();
        record.end       = trimmedEnd(paragraph.text, line.textStart(),
                                      line.textStart() + line.textLength());
        record.top       = top;
        record.ascent    = line.ascent() / layoutScale;
        record.x         = line.cursorToX(line.textStart()) / layoutScale;
        record.rounding  = rounding;
        if (singleStyle)
          record.glyphs =
              glyphs(line, toSource, record.start, record.end - record.start);
        lines.push_back(record);
        if (singleStyle) return;
        // Adjacent pieces one FX can draw: one font, spacing and color, plus
        // spaces in that font.
        QVector<Segment> segments;
        for (const auto &piece : paragraph.pieces) {
          const int from = std::max(piece.start, line.textStart());
          const int to   = std::min(piece.start + piece.length,
                                    line.textStart() + line.textLength());
          if (from >= to) continue;
          const bool ink = hasInk(paragraph.text.mid(from, to - from));
          if (!segments.isEmpty()) {
            Segment &last        = segments.back();
            const auto &format   = source.runs[piece.run];
            const auto &previous = source.runs[last.run];
            const bool sameFont =
                fontKeys[piece.run] == fontKeys[last.run] &&
                format.autoKern == previous.autoKern &&
                format.letterSpacing == previous.letterSpacing;
            const bool sameColor = pixel(format.fill) == pixel(previous.fill);
            if (last.to == from && sameFont &&
                (!ink || !last.ink || sameColor)) {
              last.to = to;
              if (ink && !last.ink) last.run = piece.run;
              last.ink |= ink;
              continue;
            }
          }
          segments.push_back({from, to, piece.run, ink});
        }
        const double baseline = top + line.ascent() / layoutScale;
        for (const auto &segment : segments) {
          if (!segment.ink) continue;
          int from = segment.from, to = segment.to;
          while (from < to && paragraph.text[from].isSpace()) ++from;
          while (to > from && paragraph.text[to - 1].isSpace()) --to;
          const auto &format = source.runs[segment.run];
          TextFxItem item;
          item.text = paragraph.text.mid(from, to - from);
          item.font = fxFont(format, fonts[segment.run], unit, item.columnScale)
                          .toString();
          item.color     = pixel(format.fill);
          item.alignment = Qt::AlignLeft;
          spacing(item, format);
          const QFont layoutFont = Iwa_TextFxLayout::transformedFont(item.font);
          // Layout pixels per source pixel in this FX's column.
          const double toLayout = unit / item.columnScale * scale;
          const double width    = unwrapped(item, layoutFont);
          const Iwa_TextFxLayout::Block block(item.text, layoutFont, width,
                                              options(item));
          const QTextLine fx = block.paragraph(0).lineAt(0);
          // The first glyph starts at its Flash pen position, on Flash's
          // baseline.
          const QPointF origin(
              line.cursorToX(from) / layoutScale - fx.cursorToX(0) / toLayout,
              baseline - (fx.y() + fx.ascent()) / toLayout);
          const QPointF center = origin + block.rect().center() / toLayout;
          item.center          = TPointD(center.x() * unit / item.columnScale,
                                         -center.y() * unit / item.columnScale);
          item.width           = width / scale;
          item.height =
              (block.rect().height() + slack(block.rect().height())) / scale;
          if (block.paragraph(0).lineCount() != 1)
            note(QString("The Text FX wraps \"%1\" where Flash does not.")
                     .arg(excerpt(item.text)));
          else
            check(item.text,
                  deviation(
                      glyphs(line, toSource, from, to - from),
                      glyphs(fx, QTransform(1 / toLayout, 0, 0, 1 / toLayout,
                                            origin.x(), origin.y()))));
          item.firstRun = item.lastRun = segment.run;
          for (const auto &piece : paragraph.pieces)
            if (piece.start < to && piece.start + piece.length > from) {
              item.firstRun = std::min(item.firstRun, piece.run);
              item.lastRun  = std::max(item.lastRun, piece.run);
            }
          items.push_back(item);
        }
      });

  if (singleStyle && !lines.isEmpty()) {
    const auto &format    = source.runs[styleRun];
    const auto &paragraph = source.runs[paragraphs.front().formatRun];
    TextFxItem item;
    // Without the spaces that end paragraphs: they draw nothing, and Flash
    // leaves them out when it centers or right-aligns a line.
    QStringList texts;
    for (const auto &each : paragraphs)
      texts << each.text.left(trimmedEnd(each.text, 0, each.text.size()));
    item.text = texts.join('\n');
    item.font =
        fxFont(format, fonts[styleRun], unit, item.columnScale).toString();
    item.color       = pixel(format.fill);
    const bool boxed = source.width > 0;
    const double available =
        source.width - paragraph.leftMargin - paragraph.rightMargin;
    const bool wraps = source.wrap && boxed && available > 0;
    item.alignment =
        boxed ? int(alignment(paragraph.alignment)) : int(Qt::AlignLeft);
    // Lines that never wrap end their paragraph: nothing to justify.
    if (!wraps && item.alignment == Qt::AlignJustify)
      item.alignment = Qt::AlignLeft;
    spacing(item, format);
    item.indent            = paragraph.indent * unit / item.columnScale;
    const QFont layoutFont = Iwa_TextFxLayout::transformedFont(item.font);
    const double toLayout  = unit / item.columnScale * scale;
    // Flash wraps at the box width less the margins (the FX box), with the
    // same alignment; text that does not wrap gets a box that holds it.
    const double width =
        wraps ? available * toLayout : unwrapped(item, layoutFont);
    if (lines.size() > 1) {
      // Line spacing: the distance between the first two FX lines made
      // Flash's before its whole-pixel rounding.
      const Iwa_TextFxLayout::Block probe(item.text, layoutFont, width,
                                          options(item));
      const QVector<FxLine> probed = fxLines(probe);
      const double step            = (lines[1].top - lines[1].rounding.y()) -
                          (lines[0].top - lines[0].rounding.y());
      if (probed.size() > 1)
        item.lineSpacing =
            (step * toLayout - (probed[1].line.y() - probed[0].line.y())) /
            scale;
    }
    const Iwa_TextFxLayout::Block block(item.text, layoutFont, width,
                                        options(item));
    const QVector<FxLine> fx = fxLines(block);
    const QTextLine &first   = fx.front().line;
    // The first line starts at its Flash pen position, on Flash's baseline,
    // before rounding; then the FX shares the rounding of the lines (at most
    // half a pixel each way).
    QPointF low = lines[0].rounding, high = lines[0].rounding;
    for (const auto &line : lines) {
      low  = QPointF(std::min(low.x(), line.rounding.x()),
                     std::min(low.y(), line.rounding.y()));
      high = QPointF(std::max(high.x(), line.rounding.x()),
                     std::max(high.y(), line.rounding.y()));
    }
    const QPointF origin =
        QPointF(lines[0].x - first.cursorToX(first.textStart()) / toLayout,
                lines[0].top + lines[0].ascent -
                    (first.y() + first.ascent()) / toLayout) -
        lines[0].rounding + (low + high) / 2;
    const QPointF center = origin + block.rect().center() / toLayout;
    item.center          = TPointD(center.x() * unit / item.columnScale,
                                   -center.y() * unit / item.columnScale);
    item.width           = width / scale;
    item.height =
        (block.rect().height() + slack(block.rect().height())) / scale;
    // Lines and glyphs are compared without trailing spaces.
    auto fxEnd = [&](const FxLine &line) {
      return trimmedEnd(block.paragraph(line.paragraph).text(),
                        line.line.textStart(),
                        line.line.textStart() + line.line.textLength());
    };
    bool sameLines = fx.size() == lines.size();
    for (int i = 0; sameLines && i < fx.size(); ++i)
      sameLines = fx[i].paragraph == lines[i].paragraph &&
                  fx[i].line.textStart() == lines[i].start &&
                  fxEnd(fx[i]) == lines[i].end;
    if (fx.size() != lines.size())
      note(QString("The Text FX wraps \"%1\" into %2 lines; Flash into %3.")
               .arg(excerpt(item.text))
               .arg(fx.size())
               .arg(lines.size()));
    else if (!sameLines)
      note(QString("The Text FX breaks the lines of \"%1\" at other words "
                   "than Flash.")
               .arg(excerpt(item.text)));
    else {
      const QTransform toSource(1 / toLayout, 0, 0, 1 / toLayout, origin.x(),
                                origin.y());
      double worst = 0;
      for (int i = 0; i < fx.size() && worst >= 0; ++i) {
        const double distance = deviation(
            lines[i].glyphs, glyphs(fx[i].line, toSource, lines[i].start,
                                    lines[i].end - lines[i].start));
        worst = distance < 0 ? distance : std::max(worst, distance);
      }
      check(item.text, worst);
    }
    item.firstRun = 0;
    item.lastRun  = source.runs.size() - 1;
    items.push_back(item);
  }
  if (approximations) *approximations << notes;
  return items;
}

}  // namespace FlashNative
