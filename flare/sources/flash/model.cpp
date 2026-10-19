#include "model.h"

#include <QSet>
#include <QXmlStreamReader>
#include <QXmlStreamWriter>
#include <algorithm>
#include <cmath>
#include <utility>

namespace Flash {
namespace {

double number(const Attributes &properties, const QString &name,
              double fallback, double minimum = -1000000000,
              double maximum = 1000000000) {
  if (!properties.contains(name)) return fallback;
  bool ok            = false;
  const double value = properties[name].toDouble(&ok);
  if (!ok || !std::isfinite(value) || value < minimum || value > maximum)
    throw Error("Invalid XFL number: " + name);
  return value;
}

int integer(const Attributes &properties, const QString &name, int fallback,
            int minimum = 0, int maximum = 10000000) {
  const double value = number(properties, name, fallback, minimum, maximum);
  if (value != std::floor(value) || value < minimum || value > maximum)
    throw Error("Invalid XFL integer: " + name);
  return int(value);
}

Color color(const Attributes &properties) {
  const QString encoded = properties.value("color", "#000000");
  Color result;
  if (!parseHexRgb(encoded, result))
    throw Error("Invalid XFL color: " + encoded);
  const double alpha = number(properties, "alpha", 1, 0, 1);
  result.alpha       = static_cast<unsigned char>(std::lround(alpha * 255));
  return result;
}

class Reader {
  Package &m_package;
  QXmlStreamReader m_xml;
  QString m_member;
  int m_depth                   = 0;
  quint64 m_elements            = 0;
  QXmlStreamWriter *m_recording = nullptr;
  QVector<int> m_sourceIndices;
  QVector<int> m_nextSibling;
  mutable QString m_cachedXmlId;
  mutable bool m_xmlIdDirty = true;
  struct ExtraCapture {
    Extra *extra = nullptr;
  };
  QVector<ExtraCapture *> m_extraCaptures;

  QString currentXmlId() const {
    if (!m_xmlIdDirty) return m_cachedXmlId;
    m_cachedXmlId = m_member;
    for (int index = 0; index < m_sourceIndices.size(); ++index)
      m_cachedXmlId += index == 0
                           ? "/node" + QString::number(m_sourceIndices[index])
                           : "/child" + QString::number(m_sourceIndices[index]);
    m_xmlIdDirty = false;
    return m_cachedXmlId;
  }

  void recordExtraToken(ExtraCapture &capture) const {
    if (m_xml.isStartElement()) {
      const QString path = currentXmlId();
      ExtraNode node;
      node.sourceId      = path;
      node.namespaceUri  = m_xml.namespaceUri().toString();
      node.qualifiedName = m_xml.qualifiedName().toString();
      for (const auto &attribute : m_xml.attributes()) {
        const QString uri = attribute.namespaceUri().toString();
        const QString expanded =
            uri.isEmpty() ? attribute.name().toString()
                          : "{" + uri + "}" + attribute.name().toString();
        node.attributes.push_back({uri, attribute.name().toString(),
                                   attribute.qualifiedName().toString(),
                                   attribute.value().toString(),
                                   path + "/@" + expanded});
      }
      capture.extra->nodes.push_back(std::move(node));
    }
  }

  void writeToken(QXmlStreamWriter &writer) const {
    if (m_xml.isStartElement()) {
      writer.writeStartElement(m_xml.namespaceUri().toString(),
                               m_xml.name().toString());
      for (const auto &attribute : m_xml.attributes())
        if (attribute.namespaceUri().isEmpty())
          writer.writeAttribute(attribute.name().toString(),
                                attribute.value().toString());
        else
          writer.writeAttribute(attribute.namespaceUri().toString(),
                                attribute.name().toString(),
                                attribute.value().toString());
    } else if (m_xml.isEndElement())
      writer.writeEndElement();
    else if (m_xml.isCharacters())
      writer.writeCharacters(m_xml.text().toString());
  }

public:
  quint64 segments = 0;
  explicit Reader(Package &package, const QByteArray &bytes,
                  const QString &member)
      : m_package(package), m_xml(bytes), m_member(member) {}

  QString tag() const {
    const QString name = m_xml.name().toString();
    const QString uri  = m_xml.namespaceUri().toString();
    return uri.isEmpty() || uri == "http://ns.adobe.com/xfl/2008/"
               ? name
               : "{" + uri + "}" + name;
  }
  bool xflTag(const QString &name) const {
    return tag() == name &&
           (m_xml.namespaceUri().isEmpty() ||
            m_xml.namespaceUri() == "http://ns.adobe.com/xfl/2008/");
  }
  Attributes attributes() const {
    Attributes result;
    for (const auto &attribute : m_xml.attributes())
      if (attribute.namespaceUri().isEmpty())
        result.insert(attribute.name().toString(),
                      attribute.value().toString());
    return result;
  }
  struct EdgeAttributes {
    Attributes properties;
    QString encodedEdges;
    QString cubics;
    SourceNode source;
  };
  EdgeAttributes edgeAttributes() const {
    EdgeAttributes result;
    result.source.sourceId      = currentXmlId();
    result.source.namespaceUri  = m_xml.namespaceUri().toString();
    result.source.qualifiedName = m_xml.qualifiedName().toString();
    for (const auto &attribute : m_xml.attributes()) {
      if (!attribute.namespaceUri().isEmpty()) {
        const QString uri  = attribute.namespaceUri().toString();
        const QString name = attribute.name().toString();
        result.source.qualifiedAttributes.push_back(
            {uri, name, attribute.qualifiedName().toString(),
             attribute.value().toString(),
             result.source.sourceId + "/@{" + uri + "}" + name});
        continue;
      }
      const QString name = attribute.name().toString();
      if (name == "edges")
        result.encodedEdges = attribute.value().toString();
      else if (name == "cubics")
        result.cubics = attribute.value().toString();
      else
        result.properties.insert(name, attribute.value().toString());
    }
    return result;
  }
  SourceNode source() const {
    SourceNode result;
    result.sourceId      = currentXmlId();
    result.namespaceUri  = m_xml.namespaceUri().toString();
    result.qualifiedName = m_xml.qualifiedName().toString();
    for (const auto &attribute : m_xml.attributes()) {
      if (!attribute.namespaceUri().isEmpty()) {
        const QString uri = attribute.namespaceUri().toString();
        result.qualifiedAttributes.push_back(
            {uri, attribute.name().toString(),
             attribute.qualifiedName().toString(), attribute.value().toString(),
             result.sourceId + "/@{" + uri + "}" +
                 attribute.name().toString()});
      }
    }
    return result;
  }
  TextNode textNode() const {
    TextNode result;
    result.sourceId = currentXmlId();
    for (const auto &attribute : m_xml.attributes()) {
      const QString uri = attribute.namespaceUri().toString();
      const QString expanded =
          uri.isEmpty() ? attribute.name().toString()
                        : "{" + uri + "}" + attribute.name().toString();
      result.rawAttributes.push_back({uri, attribute.name().toString(),
                                      attribute.qualifiedName().toString(),
                                      attribute.value().toString(),
                                      result.sourceId + "/@" + expanded});
      if (attribute.namespaceUri().isEmpty())
        result.properties.insert(attribute.name().toString(),
                                 attribute.value().toString());
    }
    return result;
  }
  QXmlStreamReader::TokenType next() {
    m_package.checkCancelled();
    const auto token = m_xml.readNext();
    if (token == QXmlStreamReader::DTD ||
        token == QXmlStreamReader::EntityReference)
      throw Error("XML declarations/entities are not supported.");
    if (token == QXmlStreamReader::StartElement) {
      if (++m_depth > m_package.limits().maxXmlDepth ||
          ++m_elements > m_package.limits().maxXmlElements)
        throw Error("XFL model XML budget exceeded.");
      const int ordinal = m_nextSibling.isEmpty() ? 0 : m_nextSibling.back()++;
      m_sourceIndices.push_back(ordinal);
      m_nextSibling.push_back(0);
      m_xmlIdDirty = true;
    } else if (token == QXmlStreamReader::EndElement)
      --m_depth;
    if (m_xml.hasError())
      throw Error(QString("XML error at line %1: %2")
                      .arg(m_xml.lineNumber())
                      .arg(m_xml.errorString()));
    for (auto *capture : m_extraCaptures) recordExtraToken(*capture);
    if (m_recording) writeToken(*m_recording);
    if (token == QXmlStreamReader::EndElement) {
      m_sourceIndices.pop_back();
      m_nextSibling.pop_back();
      m_xmlIdDirty = true;
    }
    return token;
  }
  bool child() {
    while (!m_xml.atEnd()) {
      const auto token = next();
      if (token == QXmlStreamReader::StartElement) return true;
      if (token == QXmlStreamReader::EndElement) return false;
    }
    return false;
  }
  void finish() {
    while (!m_xml.atEnd()) next();
  }
  QString characters(const QString &sourceId) {
    QString result;
    while (!m_xml.atEnd()) {
      const auto token = next();
      if (token == QXmlStreamReader::StartElement)
        throw Error(sourceId + ": Text characters contain child elements.");
      if (token == QXmlStreamReader::EndElement) return result;
      if (token == QXmlStreamReader::Characters)
        result += m_xml.text().toString();
    }
    throw Error(sourceId + ": Unterminated text characters.");
  }
  Extra capture(const QString &sourceId) {
    Extra result;
    result.sourceId      = currentXmlId();
    result.ownerSourceId = sourceId;
    result.tag           = tag();
    result.namespaceUri  = m_xml.namespaceUri().toString();
    result.qualifiedName = m_xml.qualifiedName().toString();
    QXmlStreamWriter writer(&result.xml);
    const int startingDepth = m_depth;
    ExtraCapture capture{&result};
    m_extraCaptures.push_back(&capture);
    recordExtraToken(capture);
    try {
      while (true) {
        writeToken(writer);
        if (m_depth < startingDepth) break;
        next();
      }
    } catch (...) {
      m_extraCaptures.pop_back();
      throw;
    }
    m_extraCaptures.pop_back();
    return result;
  }
  Extra captureParsed(const QString &sourceId,
                      const std::function<void()> &parse) {
    Extra result;
    result.sourceId      = currentXmlId();
    result.ownerSourceId = sourceId;
    result.tag           = tag();
    result.namespaceUri  = m_xml.namespaceUri().toString();
    result.qualifiedName = m_xml.qualifiedName().toString();
    QXmlStreamWriter writer(&result.xml);
    const int startingDepth = m_depth;
    if (m_recording) throw Error("Nested parsed XML capture.");
    ExtraCapture capture{&result};
    m_extraCaptures.push_back(&capture);
    recordExtraToken(capture);
    m_recording = &writer;
    writeToken(writer);
    try {
      parse();
      m_recording = nullptr;
      m_extraCaptures.pop_back();
    } catch (...) {
      m_recording = nullptr;
      m_extraCaptures.pop_back();
      throw;
    }
    if (m_depth != startingDepth - 1 || !m_xml.isEndElement())
      throw Error("Incomplete parsed XML capture.");
    return result;
  }
  void skip() {
    const int startingDepth = m_depth;
    while (m_depth >= startingDepth && !m_xml.atEnd()) next();
  }
  QVector<Segment> edges(const QString &encoded) {
    const quint64 limit     = m_package.limits().maxGeometrySegments;
    const quint64 remaining = limit - std::min(limit, segments);
    const QVector<Segment> decoded =
        decodeEdges(encoded, remaining, m_package.limits().cancelled);
    segments += decoded.size();
    return decoded;
  }
  void reserveSegments(quint64 count) {
    const quint64 limit = m_package.limits().maxGeometrySegments;
    if (count > limit - std::min(limit, segments))
      throw Error("XFL model geometry budget exceeded.");
    segments += count;
  }
};

void appendExtra(Reader &reader, QVector<Extra> &extras,
                 const QString &parentId) {
  extras.push_back(reader.capture(parentId));
}

TextRun textRun(Reader &reader, const QString &sourceId) {
  TextRun result;
  result.sourceId = sourceId;
  result.source   = reader.textNode();
  while (reader.child()) {
    if (reader.xflTag("characters")) {
      if (result.hasCharacters)
        throw Error(sourceId + ": Duplicate text characters.");
      result.hasCharacters    = true;
      result.charactersSource = reader.textNode();
      result.characters       = reader.characters(sourceId + "/characters");
    } else if (reader.xflTag("textAttrs")) {
      if (result.hasTextAttrs)
        throw Error(sourceId + ": Duplicate textAttrs container.");
      result.hasTextAttrs    = true;
      result.textAttrsSource = reader.textNode();
      while (reader.child()) {
        if (reader.xflTag("DOMTextAttrs")) {
          if (result.style)
            throw Error(sourceId + ": Duplicate DOMTextAttrs style.");
          TextStyle style;
          style.sourceId = sourceId + "/textAttrs/DOMTextAttrs";
          style.source   = reader.textNode();
          while (reader.child())
            appendExtra(reader, style.source.extras, style.sourceId);
          result.style = std::move(style);
        } else
          appendExtra(reader, result.textAttrsSource.extras,
                      sourceId + "/textAttrs");
      }
    } else
      appendExtra(reader, result.source.extras, sourceId);
  }
  return result;
}

void textRuns(Reader &reader, TextBlock &block) {
  block.runsSource = reader.textNode();
  while (reader.child()) {
    if (reader.xflTag("DOMTextRun"))
      block.runs.push_back(
          textRun(reader, block.sourceId + "/textRuns/run" +
                              QString::number(block.runs.size())));
    else
      appendExtra(reader, block.runsSource.extras,
                  block.sourceId + "/textRuns");
  }
}

Affine matrix(Reader &reader, const QString &sourceId, SourceRecord &source) {
  Affine result;
  bool seen = false;
  while (reader.child()) {
    if (reader.xflTag("Matrix")) {
      if (seen) throw Error(sourceId + ": Duplicate Matrix record.");
      seen                        = true;
      const Attributes properties = reader.attributes();
      source = {sourceId + "/Matrix", properties, reader.source()};
      result = {number(properties, "a", 1),  number(properties, "b", 0),
                number(properties, "c", 0),  number(properties, "d", 1),
                number(properties, "tx", 0), number(properties, "ty", 0)};
      result.validate();
    }
    reader.skip();
  }
  return result;
}

Paint readPaint(Reader &reader, const QString &sourceId) {
  Paint result;
  result.sourceId   = sourceId;
  result.sourceTag  = reader.tag();
  result.properties = reader.attributes();
  result.source     = reader.source();
  if (result.sourceTag == "SolidColor") {
    result.kind  = Paint::Kind::Solid;
    result.color = color(result.properties);
  } else if (result.sourceTag == "LinearGradient")
    result.kind = Paint::Kind::LinearGradient;
  else if (result.sourceTag == "RadialGradient")
    result.kind = Paint::Kind::RadialGradient;
  else if (result.sourceTag == "BitmapFill")
    result.kind = Paint::Kind::Bitmap;
  else
    result.kind = Paint::Kind::Unsupported;
  while (reader.child()) {
    if (reader.xflTag("matrix")) {
      if (result.matrixXml)
        throw Error(sourceId + ": Duplicate matrix container.");
      result.matrixXml = reader.captureParsed(sourceId + "/matrix", [&] {
        result.matrix =
            matrix(reader, sourceId + "/matrix", result.matrixSource);
      });
    } else if (reader.tag() == "GradientEntry") {
      const Attributes properties = reader.attributes();
      GradientStop stop;
      stop.ratio      = number(properties, "ratio", 0, 0, 1);
      stop.color      = color(properties);
      stop.properties = properties;
      stop.source     = reader.source();
      stop.sourceId =
          sourceId + "/GradientEntry" + QString::number(result.stops.size());
      while (reader.child()) appendExtra(reader, stop.extras, stop.sourceId);
      result.stops.push_back(std::move(stop));
    } else
      appendExtra(reader, result.extras, sourceId);
  }
  if (result.kind == Paint::Kind::LinearGradient ||
      result.kind == Paint::Kind::RadialGradient) {
    if (result.stops.size() < 2)
      throw Error("Gradient requires at least two stops.");
    double last = -1;
    for (const auto &stop : result.stops) {
      if (stop.ratio < last)
        throw Error("Gradient stop ratios are not ordered.");
      last = stop.ratio;
    }
  }
  return result;
}

Paint paintContainer(Reader &reader, const QString &sourceId) {
  Paint result;
  bool seen = false;
  const SourceRecord style{sourceId, reader.attributes(), reader.source()};
  while (reader.child()) {
    if (seen) throw Error("Multiple paints in one fill style.");
    result       = readPaint(reader, sourceId + "/paint");
    result.style = style;
    seen         = true;
  }
  if (!seen) throw Error("Empty XFL fill style.");
  return result;
}

Stroke strokeContainer(Reader &reader, const QString &sourceId) {
  Stroke result;
  bool seen    = false;
  result.style = {sourceId, reader.attributes(), reader.source()};
  while (reader.child()) {
    if (seen) throw Error("Multiple paints in one stroke style.");
    seen              = true;
    result.sourceId   = sourceId + "/stroke";
    result.type       = reader.tag();
    result.properties = reader.attributes();
    result.source     = reader.source();
    result.weight     = number(result.properties, "weight", 1, 0, 1000000);
    bool fillSeen     = false;
    while (reader.child()) {
      if (reader.tag() == "fill") {
        if (fillSeen) throw Error("Multiple fills in one stroke.");
        result.paint = paintContainer(reader, result.sourceId + "/fill");
        fillSeen     = true;
      } else
        appendExtra(reader, result.extras, result.sourceId);
    }
  }
  if (!seen) throw Error("Empty XFL stroke style.");
  return result;
}

void primitiveGeometry(Element &element, Reader &reader) {
  const auto &properties = element.properties;
  const double x         = number(properties, "x", 0);
  const double y         = number(properties, "y", 0);
  const double width     = number(properties, "objectWidth", 0, 0);
  const double height    = number(properties, "objectHeight", 0, 0);
  if (width <= 0 || height <= 0 || !std::isfinite(x + width) ||
      !std::isfinite(y + height))
    throw Error(element.sourceId + ": Invalid primitive dimensions.");
  const bool oval = element.kind == Element::Kind::Oval;
  const double tl = oval ? 0 : number(properties, "topLeftRadius", 0, 0);
  const double tr = oval ? 0 : number(properties, "topRightRadius", 0, 0);
  const double br = oval ? 0 : number(properties, "bottomRightRadius", 0, 0);
  const double bl = oval ? 0 : number(properties, "bottomLeftRadius", 0, 0);
  if (!oval && (tl + tr > width || bl + br > width || tl + bl > height ||
                tr + br > height))
    throw Error(element.sourceId + ": Overlapping rectangle corner radii.");

  // Charge before generating any curve points. The model reader's segment
  // total then flows through both the preflight and direct scene-builder
  // document budgets, just like encoded DOMShape edges.
  const quint64 count =
      oval ? 32
           : quint64(width > tl + tr) + quint64(height > tr + br) +
                 quint64(width > bl + br) + quint64(height > tl + bl) +
                 8 * (quint64(tl > 0) + quint64(tr > 0) + quint64(br > 0) +
                      quint64(bl > 0));
  reader.reserveSegments(count);

  Edge edge;
  edge.fill1  = element.shape->fills.contains(1) ? 1 : 0;
  edge.stroke = element.shape->strokes.contains(1) ? 1 : 0;
  edge.source = {element.sourceId + "/primitiveGeometry", properties,
                 element.source};
  auto line   = [&](Point a, Point b) {
    if (!edge.segments.isEmpty()) a = edge.segments.back().end;
    if (a == b) return;
    Segment segment;
    segment.start         = a;
    segment.control       = {(a.x + b.x) / 2, (a.y + b.y) / 2};
    segment.end           = b;
    segment.startsContour = edge.segments.isEmpty();
    edge.segments.push_back(segment);
  };
  auto arc = [&](double cx, double cy, double rx, double ry, double from,
                 double to, Point begin, Point finish) {
    // Native geometry uses ordinary quadratics. Eight subdivisions per
    // quarter keep the analytic Flash primitive within a small fraction of a
    // source pixel, even on large stage objects.
    const int count = oval ? 32 : 8;
    for (int index = 0; index < count; ++index) {
      const double a      = from + (to - from) * index / count;
      const double b      = from + (to - from) * (index + 1) / count;
      const double middle = (a + b) / 2;
      const double scale  = 1 / std::cos((b - a) / 2);
      Segment segment;
      segment.start =
          edge.segments.isEmpty() ? begin : edge.segments.back().end;
      segment.control       = {cx + rx * std::cos(middle) * scale,
                               cy + ry * std::sin(middle) * scale};
      segment.end           = index + 1 == count
                                  ? finish
                                  : Point{cx + rx * std::cos(b), cy + ry * std::sin(b)};
      segment.quadratic     = true;
      segment.startsContour = edge.segments.isEmpty();
      edge.segments.push_back(segment);
    }
  };
  constexpr double pi = 3.1415926535897932384626433832795;
  if (oval) {
    // Start at the rightmost point and proceed clockwise in source Y-down.
    const Point right{x + width, y + height / 2};
    arc(x + width / 2, y + height / 2, width / 2, height / 2, 0, 2 * pi, right,
        right);
  } else {
    const Point start{x + tl, y};
    line(start, {x + width - tr, y});
    if (tr)
      arc(x + width - tr, y + tr, tr, tr, -pi / 2, 0, {x + width - tr, y},
          {x + width, y + tr});
    line({x + width, y + tr}, {x + width, y + height - br});
    if (br)
      arc(x + width - br, y + height - br, br, br, 0, pi / 2,
          {x + width, y + height - br}, {x + width - br, y + height});
    line({x + width - br, y + height}, {x + bl, y + height});
    if (bl)
      arc(x + bl, y + height - bl, bl, bl, pi / 2, pi, {x + bl, y + height},
          {x, y + height - bl});
    line({x, y + height - bl}, {x, y + tl});
    if (tl)
      arc(x + tl, y + tl, tl, tl, pi, 3 * pi / 2, {x, y + tl}, {x + tl, y});
  }
  if (edge.segments.isEmpty())
    throw Error(element.sourceId + ": Empty primitive geometry.");
  edge.segments.back().end = edge.segments.front().start;
  element.shape->edges.push_back(std::move(edge));
}

Element readElement(Reader &reader, const QString &sourceId);

QVector<Element> elements(Reader &reader, const QString &sourceId) {
  QVector<Element> result;
  while (reader.child())
    result.push_back(
        readElement(reader, sourceId + '/' + QString::number(result.size())));
  return result;
}

Element readElement(Reader &reader, const QString &sourceId) {
  Element result;
  result.tag        = reader.tag();
  result.sourceId   = sourceId;
  result.properties = reader.attributes();
  result.source     = reader.source();
  if (result.tag == "DOMShape") {
    result.kind  = Element::Kind::Shape;
    result.shape = std::make_shared<Shape>();
  } else if (result.tag == "DOMSymbolInstance")
    result.kind = Element::Kind::Symbol;
  else if (result.tag == "DOMGroup")
    result.kind = Element::Kind::Group;
  else if (result.tag == "DOMBitmapInstance")
    result.kind = Element::Kind::Bitmap;
  else if (result.tag == "DOMStaticText" || result.tag == "DOMDynamicText" ||
           result.tag == "DOMInputText")
    result.kind = Element::Kind::Text;
  else if (result.tag == "DOMOvalObject")
    result.kind = Element::Kind::Oval;
  else if (result.tag == "DOMRectangleObject")
    result.kind = Element::Kind::Rectangle;
  else if (result.tag == "DOMVideoInstance")
    result.kind = Element::Kind::Video;
  if (result.kind == Element::Kind::Oval ||
      result.kind == Element::Kind::Rectangle)
    result.shape = std::make_shared<Shape>();
  if (reader.xflTag("DOMStaticText") || reader.xflTag("DOMDynamicText") ||
      reader.xflTag("DOMInputText")) {
    result.text           = std::make_shared<TextBlock>();
    result.text->sourceId = sourceId;
    result.text->source   = reader.textNode();
  }
  bool membersSeen = false;
  while (reader.child()) {
    const QString tag = reader.tag();
    if (tag == "matrix") {
      if (result.matrixXml)
        throw Error(sourceId + ": Duplicate matrix container.");
      const auto preserved = reader.captureParsed(sourceId + "/matrix", [&] {
        result.matrix =
            matrix(reader, sourceId + "/matrix", result.matrixSource);
      });
      result.matrixXml     = preserved;
      if (result.text) result.text->matrixXml = preserved;
    } else if (tag == "transformationPoint") {
      if (result.transformationPointXml)
        throw Error(sourceId + ": Duplicate transformationPoint container.");
      const auto preserved =
          reader.captureParsed(sourceId + "/transformationPoint", [&] {
            while (reader.child()) {
              if (reader.xflTag("Point")) {
                if (result.hasTransformationPoint)
                  throw Error(sourceId +
                              "/transformationPoint: Duplicate Point record.");
                const Attributes properties      = reader.attributes();
                result.transformationPointSource = {
                    sourceId + "/transformationPoint/Point", properties,
                    reader.source()};
                result.transformationPoint    = {number(properties, "x", 0),
                                                 number(properties, "y", 0)};
                result.hasTransformationPoint = true;
              }
              reader.skip();
            }
          });
      result.transformationPointXml = preserved;
      if (result.text) result.text->transformationPointXml = preserved;
    } else if (reader.xflTag("textRuns") && result.text) {
      if (result.text->hasRuns)
        throw Error(sourceId + ": Duplicate textRuns container.");
      result.text->hasRuns = true;
      // Retain the original subtree as well as the structured data.
      const Extra preserved = reader.captureParsed(
          sourceId + "/textRuns", [&] { textRuns(reader, *result.text); });
      result.extras.push_back(preserved);
    } else if (tag == "members" && result.kind == Element::Kind::Group) {
      if (membersSeen) throw Error(sourceId + ": Duplicate members container.");
      membersSeen    = true;
      result.members = elements(reader, sourceId + "/members");
    } else if (reader.xflTag("fill") && result.shape &&
               result.kind != Element::Kind::Shape) {
      if (result.shape->fills.contains(1))
        throw Error(sourceId + ": Duplicate primitive fill.");
      result.shape->fills.insert(1, paintContainer(reader, sourceId + "/fill"));
    } else if (reader.xflTag("stroke") && result.shape &&
               result.kind != Element::Kind::Shape) {
      if (result.shape->strokes.contains(1))
        throw Error(sourceId + ": Duplicate primitive stroke.");
      result.shape->strokes.insert(
          1, strokeContainer(reader, sourceId + "/stroke"));
    } else if (tag == "fills" && result.kind == Element::Kind::Shape) {
      while (reader.child()) {
        if (reader.tag() != "FillStyle")
          throw Error("Unexpected shape fill record.");
        const int index = integer(reader.attributes(), "index", 0, 1, 65535);
        if (result.shape->fills.contains(index))
          throw Error("Duplicate shape fill index.");
        result.shape->fills.insert(
            index, paintContainer(reader, sourceId + "/fills/FillStyle" +
                                              QString::number(index)));
      }
    } else if (tag == "strokes" && result.kind == Element::Kind::Shape) {
      while (reader.child()) {
        if (reader.tag() != "StrokeStyle")
          throw Error("Unexpected shape stroke record.");
        const int index = integer(reader.attributes(), "index", 0, 1, 65535);
        if (result.shape->strokes.contains(index))
          throw Error("Duplicate shape stroke index.");
        result.shape->strokes.insert(
            index, strokeContainer(reader, sourceId + "/strokes/StrokeStyle" +
                                               QString::number(index)));
      }
    } else if (tag == "edges" && result.kind == Element::Kind::Shape) {
      while (reader.child()) {
        if (reader.tag() != "Edge")
          throw Error("Unexpected shape edge record.");
        Reader::EdgeAttributes attributes = reader.edgeAttributes();
        Edge edge;
        edge.source.sourceId = sourceId + "/edges/Edge" +
                               QString::number(result.shape->edges.size());
        edge.source.properties = std::move(attributes.properties);
        edge.source.source     = std::move(attributes.source);
        edge.fill0 = integer(edge.source.properties, "fillStyle0", 0, 0, 65535);
        edge.fill1 = integer(edge.source.properties, "fillStyle1", 0, 0, 65535);
        edge.stroke =
            integer(edge.source.properties, "strokeStyle", 0, 0, 65535);
        edge.cubicControlData = std::move(attributes.cubics);
        edge.segments         = reader.edges(attributes.encodedEdges);
        while (reader.child())
          appendExtra(reader, edge.extras, edge.source.sourceId);
        result.shape->edges.push_back(std::move(edge));
      }
    } else {
      const Extra extra = reader.capture(sourceId);
      if (result.text) result.text->source.extras.push_back(extra);
      result.extras.push_back(extra);
    }
  }
  if (result.shape)
    for (const auto &edge : result.shape->edges) {
      if ((edge.fill0 && !result.shape->fills.contains(edge.fill0)) ||
          (edge.fill1 && !result.shape->fills.contains(edge.fill1)) ||
          (edge.stroke && !result.shape->strokes.contains(edge.stroke)))
        throw Error("Shape edge refers to a missing fill/stroke style.");
    }
  if (result.kind == Element::Kind::Oval ||
      result.kind == Element::Kind::Rectangle)
    primitiveGeometry(result, reader);
  return result;
}

Frame readFrame(Reader &reader, const QString &sourceId) {
  Frame result;
  result.sourceId   = sourceId;
  result.properties = reader.attributes();
  result.source     = reader.source();
  result.index      = integer(result.properties, "index", 0);
  result.duration   = integer(result.properties, "duration", 1, 1);
  if (result.index > 10000000 - result.duration)
    throw Error("Excessive XFL frame span.");
  bool elementsSeen = false;
  while (reader.child()) {
    if (reader.tag() == "elements") {
      if (elementsSeen)
        throw Error(sourceId + ": Duplicate elements container.");
      elementsSeen    = true;
      result.elements = elements(reader, sourceId);
    } else
      appendExtra(reader, result.extras, sourceId);
  }
  return result;
}

Layer readLayer(Reader &reader, const QString &sourceId) {
  Layer result;
  result.sourceId   = sourceId;
  result.properties = reader.attributes();
  result.source     = reader.source();
  result.name       = result.properties.value("name");
  result.type       = result.properties.value("layerType", "normal");
  result.parent     = integer(result.properties, "parentLayerIndex", -1, -1);
  result.visible    = result.properties.value("visible", "true") != "false";
  result.locked     = result.properties.value("locked") == "true";
  int previousEnd   = 0;
  while (reader.child()) {
    if (reader.tag() == "frames") {
      while (reader.child()) {
        if (reader.tag() != "DOMFrame")
          throw Error("Unexpected layer frame record.");
        Frame frame =
            readFrame(reader, sourceId + "/frame" +
                                  reader.attributes().value("index", "0"));
        if (frame.index < previousEnd)
          throw Error("Overlapping XFL frame spans.");
        previousEnd = frame.index + frame.duration;
        result.frames.push_back(std::move(frame));
      }
    } else
      appendExtra(reader, result.extras, sourceId);
  }
  return result;
}

Timeline readTimeline(Reader &reader, const QString &sourceId) {
  Timeline result;
  result.sourceId   = sourceId;
  result.properties = reader.attributes();
  result.source     = reader.source();
  result.name       = result.properties.value("name");
  while (reader.child()) {
    if (reader.tag() == "layers") {
      while (reader.child()) {
        if (reader.tag() != "DOMLayer")
          throw Error("Unexpected timeline layer record.");
        Layer layer =
            readLayer(reader, sourceId + "/layer" +
                                  QString::number(result.layers.size()));
        for (const auto &frame : layer.frames)
          result.frameCount =
              std::max(result.frameCount, frame.index + frame.duration);
        result.layers.push_back(std::move(layer));
      }
    } else
      appendExtra(reader, result.extras, sourceId);
  }
  QVector<int> parents;
  QVector<QString> types;
  for (const auto &layer : result.layers) {
    parents.push_back(layer.parent);
    types.push_back(layer.type);
  }
  // A camera layer is not counted in parentLayerIndex (layerParentIndices).
  parents = layerParentIndices(parents, types);
  for (int index = 0; index < result.layers.size(); ++index)
    result.layers[index].parent = parents[index];
  switch (layerParentProblem(parents)) {
  case LayerParentProblem::None:
    break;
  case LayerParentProblem::Invalid:
    throw Error("Invalid XFL layer parent.");
  case LayerParentProblem::Cyclic:
    throw Error("Cyclic XFL layer parent.");
  }
  return result;
}

void validateMovieClipInstance(const Element &element) {
  if (element.kind != Element::Kind::Symbol ||
      symbolInstanceType(element) != "movie clip")
    throw Error("Movie clip timing requires a movie clip instance.");
  // Movie clips do not use the graphic instance's First/Last/loop controls.
  // An unfamiliar field might alter playback, so keep it visible to preflight.
  static const QSet<QString> fields{
      "libraryItemName", "symbolType", "name", "itemID", "referenceID",
      "lastModified", "selected", "isSelected", "centerPoint3DX",
      "centerPoint3DY", "blendMode", "visible", "isVisible", "cacheAsBitmap",
      // 3D placement (reported by preflight as unmapped-3d-transform).
      "centerPoint3DZ", "rotationX", "rotationY", "rotationZ", "matrix3D"};
  for (auto property = element.properties.constBegin();
       property != element.properties.constEnd(); ++property)
    if (!fields.contains(property.key()))
      throw Error("Unsupported movie clip instance property: " +
                  property.key().left(256));
  for (const QString &visibility : {QString("visible"), QString("isVisible")})
    if (element.properties.contains(visibility) &&
        element.properties.value(visibility) != "true")
      throw Error("Unsupported movie clip instance property: " + visibility);
  // Bitmap caching draws the same image (Flash caches filtered instances
  // anyway); it does not change the clock either.
  if (element.properties.contains("cacheAsBitmap") &&
      element.properties.value("cacheAsBitmap") != "false" &&
      element.properties.value("cacheAsBitmap") != "true")
    throw Error("Unsupported movie clip instance property: cacheAsBitmap");
}

const Element *movieClipAt(const Frame &frame, int slot) {
  if (slot < 0 || slot >= frame.elements.size()) return nullptr;
  const Element &element = frame.elements[slot];
  return element.kind == Element::Kind::Symbol &&
                 symbolInstanceType(element) == "movie clip"
             ? &element
             : nullptr;
}

bool sameMovieClipInstance(const Element &a, const Element &b) {
  return a.properties.value("libraryItemName") ==
             b.properties.value("libraryItemName") &&
         a.properties.contains("name") == b.properties.contains("name") &&
         a.properties.value("name") == b.properties.value("name");
}

}  // namespace

bool parseHexRgb(const QString &lexical, Color &result) {
  if (lexical.size() != 7 || lexical[0] != '#') return false;
  for (int i = 1; i < lexical.size(); ++i) {
    const QChar digit = lexical[i].toLower();
    if (!((digit >= '0' && digit <= '9') || (digit >= 'a' && digit <= 'f')))
      return false;
  }
  bool ok             = false;
  const quint32 value = lexical.mid(1).toUInt(&ok, 16);
  if (!ok) return false;
  result.red   = static_cast<unsigned char>((value >> 16) & 255);
  result.green = static_cast<unsigned char>((value >> 8) & 255);
  result.blue  = static_cast<unsigned char>(value & 255);
  return true;
}

namespace {
// Lenient text attribute evaluation: an unusual value keeps its default and
// is reported, since text layout is an approximation anyway.
struct TextAttributeReader {
  const TextNode &node;
  const QString &sourceId;
  QStringList &unmapped;
  double number(const QString &name, double fallback, double minimum,
                double maximum) const {
    if (!node.properties.contains(name)) return fallback;
    bool ok            = false;
    const double value = node.properties.value(name).toDouble(&ok);
    if (ok && std::isfinite(value) && value >= minimum && value <= maximum)
      return value;
    unmapped << sourceId + ": invalid " + name + " \"" +
                    node.properties.value(name) + "\"";
    return fallback;
  }
  bool flag(const QString &name, bool fallback) const {
    if (!node.properties.contains(name)) return fallback;
    const QString value = node.properties.value(name);
    if (value == "true") return true;
    if (value == "false") return false;
    unmapped << sourceId + ": invalid " + name + " \"" + value + "\"";
    return fallback;
  }
  void unknown(const QSet<QString> &known) const {
    for (const auto &attribute : node.rawAttributes)
      if (!attribute.namespaceUri.isEmpty() || !known.contains(attribute.name))
        unmapped << sourceId + ": attribute " + attribute.qualifiedName;
    for (const auto &extra : node.extras)
      unmapped << sourceId + ": child " + extra.qualifiedName;
  }
};
}  // namespace

TextLayoutSource textLayout(const Element &element) {
  TextLayoutSource result;
  if (!element.text) return result;
  const TextBlock &block = *element.text;
  const QString blockId  = block.sourceId;
  const TextAttributeReader box{block.source, blockId, result.unmapped};
  // Editor-only, accessibility, scripting and font-embedding settings do not
  // change how the authored text looks.
  box.unknown({"width",
               "height",
               "left",
               "top",
               "autoExpand",
               "isSelectable",
               "selected",
               "name",
               "orientation",
               "lineType",
               "renderAsHTML",
               "border",
               "fontRenderingMode",
               "antiAliasSharpness",
               "antiAliasThickness",
               "maxCharacters",
               "variableName",
               "scrollable",
               "embedRanges",
               "embeddedCharacters",
               "includeOutlines",
               "useDeviceFonts",
               "accName",
               "description",
               "shortcut",
               "silent",
               "forceSimple",
               "tabIndex",
               "autoLabeling",
               "instanceName"});
  result.left       = box.number("left", 0, -1e7, 1e7);
  result.top        = box.number("top", 0, -1e7, 1e7);
  result.width      = box.number("width", 0, 0, 1e7);
  result.height     = box.number("height", 0, 0, 1e7);
  result.autoExpand = box.flag("autoExpand", false);
  const QString orientation =
      block.source.properties.value("orientation", "horizontal");
  if (orientation != "horizontal")
    result.unmapped << blockId + ": vertical text orientation \"" +
                           orientation + "\" is laid out horizontally";
  if (box.flag("renderAsHTML", false))
    result.unmapped << blockId + ": HTML text is shown as plain text";
  if (box.flag("border", false))
    result.unmapped << blockId + ": text field border is not drawn";
  if (element.tag == "DOMStaticText")
    result.wrap = !result.autoExpand;
  else {
    const QString lineType =
        block.source.properties.value("lineType", "single line");
    result.wrap = lineType == "multiline";
    if (lineType == "password")
      result.unmapped << blockId + ": password text is shown unmasked";
  }
  for (const auto &extra : block.source.extras)
    result.unmapped << blockId + ": child " + extra.qualifiedName;
  for (const auto &extra : block.runsSource.extras)
    result.unmapped << blockId + "/textRuns: child " + extra.qualifiedName;
  TextRunFormat previous;
  for (const auto &run : block.runs) {
    // A run without attributes continues the previous formatting.
    TextRunFormat format = previous;
    format.sourceId      = run.sourceId;
    format.characters    = run.characters;
    for (const auto &extra : run.source.extras)
      result.unmapped << run.sourceId + ": child " + extra.qualifiedName;
    for (const auto &extra : run.textAttrsSource.extras)
      result.unmapped << run.sourceId + "/textAttrs: child " +
                             extra.qualifiedName;
    if (run.style) {
      const QString styleId = run.style->sourceId;
      const TextAttributeReader attrs{run.style->source, styleId,
                                      result.unmapped};
      attrs.unknown({"aliasText",
                     "autoKern",
                     "bold",
                     "italic",
                     "size",
                     "bitmapSize",
                     "face",
                     "fillColor",
                     "alpha",
                     "alignment",
                     "letterSpacing",
                     "lineSpacing",
                     "lineHeight",
                     "indent",
                     "leftMargin",
                     "rightMargin",
                     "characterPosition",
                     "rotation",
                     "url",
                     "target"});
      const auto &p = run.style->source.properties;
      format.face   = p.value("face", format.face);
      format.size   = attrs.number("size", format.size, 0.01, 10000);
      if (p.contains("fillColor")) {
        Color color;
        if (parseHexRgb(p.value("fillColor"), color)) {
          color.alpha = format.fill.alpha;
          format.fill = color;
        } else
          result.unmapped << styleId + ": invalid fillColor \"" +
                                 p.value("fillColor") + "\"";
      }
      if (p.contains("alpha"))
        format.fill.alpha = static_cast<unsigned char>(
            std::lround(attrs.number("alpha", 1, 0, 1) * 255));
      format.bold     = attrs.flag("bold", format.bold);
      format.italic   = attrs.flag("italic", format.italic);
      format.autoKern = attrs.flag("autoKern", format.autoKern);
      format.letterSpacing =
          attrs.number("letterSpacing", format.letterSpacing, -1000, 1000);
      if (p.contains("lineSpacing")) {
        format.lineSpacing    = attrs.number("lineSpacing", 0, -1000, 10000);
        format.hasLineSpacing = true;
      }
      format.lineHeight = attrs.number("lineHeight", format.lineHeight, 0, 1e5);
      format.indent     = attrs.number("indent", format.indent, -1e5, 1e5);
      format.leftMargin = attrs.number("leftMargin", format.leftMargin, 0, 1e5);
      format.rightMargin =
          attrs.number("rightMargin", format.rightMargin, 0, 1e5);
      const QString alignment = p.value("alignment", format.alignment);
      if (alignment == "left" || alignment == "center" ||
          alignment == "right" || alignment == "justify")
        format.alignment = alignment;
      else
        result.unmapped << styleId + ": alignment \"" + alignment + "\"";
      const QString position = p.value("characterPosition", "normal");
      if (position != "normal")
        result.unmapped << styleId + ": " + position +
                               " characters are drawn on the baseline";
      if (attrs.flag("rotation", false))
        result.unmapped << styleId + ": rotated characters are drawn upright";
      if (!p.value("url").isEmpty())
        result.unmapped << styleId + ": hyperlink is not kept";
    }
    previous = format;
    result.runs.push_back(std::move(format));
  }
  return result;
}

QString symbolInstanceType(const Element &element) {
  return element.properties.value("symbolType", "movie clip");
}

Definition readDefinition(Package &package, const QString &member) {
  const QByteArray bytes = package.read(member, package.limits().maxXmlBytes);
  Reader reader(package, bytes, member);
  if (!reader.child()) throw Error("Empty XFL definition: " + member);
  const QString rootTag = reader.tag();
  if (rootTag != "DOMDocument" && rootTag != "DOMSymbolItem")
    throw Error("Unexpected XFL definition root: " + member);
  Definition result;
  result.member     = member;
  result.properties = reader.attributes();
  result.source     = reader.source();
  result.name       = result.properties.value("name");
  result.type       = result.properties.value("symbolType");
  while (reader.child()) {
    if (reader.tag() == "timeline" || reader.tag() == "timelines") {
      while (reader.child()) {
        if (reader.tag() != "DOMTimeline")
          throw Error("Unexpected XFL timeline record.");
        result.timelines.push_back(readTimeline(
            reader,
            member + "/timeline" + QString::number(result.timelines.size())));
      }
    } else
      appendExtra(reader, result.extras, member);
  }
  reader.finish();
  result.geometrySegments = reader.segments;
  return result;
}

GraphicTiming instanceTiming(const Element &element, const Frame &frame) {
  if (element.kind != Element::Kind::Symbol ||
      symbolInstanceType(element) != "graphic")
    throw Error("Graphic timing requires a graphic symbol instance.");
  validateGraphicInstanceProperties(element.properties);
  GraphicTiming timing;
  timing.start      = frame.index;
  timing.duration   = frame.duration;
  timing.firstFrame = integer(element.properties, "firstFrame", 0);
  timing.mode       = playbackMode(element.properties.value("loop", "loop"));
  if (element.properties.contains("lastFrame"))
    timing.lastFrame = integer(element.properties, "lastFrame", 0);
  // Validates the range and its direction; the symbol frame count is checked
  // later by sourceFrameAt.
  timing.toJson();
  return timing;
}

MovieClipTiming movieClipTiming(const Element &element, const Frame &frame) {
  validateMovieClipInstance(element);
  MovieClipTiming timing{frame.index, frame.duration};
  timing.sourceFrameAt(timing.start, 1);
  return timing;
}

MovieClipTiming movieClipTiming(const Layer &layer, int frameOrdinal,
                                int elementSlot) {
  if (frameOrdinal < 0 || frameOrdinal >= layer.frames.size() ||
      elementSlot < 0 ||
      elementSlot >= layer.frames[frameOrdinal].elements.size())
    throw Error("Movie clip timing location is outside its layer.");
  const Frame &frame     = layer.frames[frameOrdinal];
  MovieClipTiming timing = movieClipTiming(frame.elements[elementSlot], frame);
  const int clockStart = movieClipClockStart(layer, frameOrdinal, elementSlot);
  if (clockStart != frame.index) timing.clockStart = clockStart;
  timing.sourceFrameAt(timing.start, 1);
  return timing;
}

int movieClipClockStart(const Layer &layer, int frameOrdinal, int elementSlot) {
  if (frameOrdinal < 0 || frameOrdinal >= layer.frames.size())
    throw Error("Movie clip timing location is outside its layer.");
  const Element *current = movieClipAt(layer.frames[frameOrdinal], elementSlot);
  int ordinal            = frameOrdinal;
  while (current && ordinal > 0) {
    const Frame &previous = layer.frames[ordinal - 1];
    const Frame &frame    = layer.frames[ordinal];
    const Element *before = movieClipAt(previous, elementSlot);
    if (previous.index + previous.duration != frame.index || !before ||
        !sameMovieClipInstance(*before, *current))
      break;
    current = before;
    --ordinal;
  }
  return layer.frames[ordinal].index;
}

bool frameScriptExtra(const Extra &extra) {
  return extra.tag.compare("Actionscript", Qt::CaseInsensitive) == 0;
}

QString frameScript(const Frame &frame) {
  QStringList scripts;
  for (const auto &extra : frame.extras) {
    if (!frameScriptExtra(extra)) continue;
    QXmlStreamReader xml(extra.xml);
    xml.setNamespaceProcessing(false);
    QString text;
    int depth = 0;  // inside <script>
    while (!xml.atEnd()) {
      xml.readNext();
      if (xml.isStartElement() &&
          (depth || xml.name().compare(QLatin1String("script"),
                                       Qt::CaseInsensitive) == 0))
        ++depth;
      else if (xml.isEndElement() && depth) {
        if (!--depth) {
          scripts << text;
          text.clear();
        }
      } else if (depth && xml.isCharacters())
        text += xml.text();
    }
  }
  return scripts.join('\n');
}

QMap<QString, int> frameLabelTargets(const Timeline &timeline) {
  QMap<QString, int> result;
  for (const auto &layer : timeline.layers)
    for (const auto &frame : layer.frames) {
      const QString name = frame.properties.value("name");
      const QString type = frame.properties.value("labelType", "name");
      if (!name.isEmpty() && (type == "name" || type == "anchor") &&
          !result.contains(name))
        result.insert(name, frame.index);
    }
  return result;
}

QMap<int, QVector<ScriptCommand>> timelineScriptCommands(
    const Timeline &timeline, bool documentTimeline) {
  QMap<int, QVector<ScriptCommand>> result;
  // Flash runs a frame's scripts from the bottom layer up; XFL lists the
  // top layer first.
  for (int layer = timeline.layers.size() - 1; layer >= 0; --layer)
    for (const auto &frame : timeline.layers[layer].frames) {
      const QString source = frameScript(frame);
      if (source.trimmed().isEmpty()) continue;
      for (const auto &command : parseFrameScript(source).commands)
        // A clip's commands to the main timeline are reported, not applied.
        if (documentTimeline || !command.root)
          result[frame.index].push_back(command);
    }
  return result;
}

Playhead timelinePlayhead(const Timeline &timeline, bool documentTimeline,
                          QStringList *problems) {
  return simulatePlayhead(std::max(1, timeline.frameCount),
                          timelineScriptCommands(timeline, documentTimeline),
                          frameLabelTargets(timeline), problems);
}

namespace {
// A keyframe copied into a replay: entered `offset` frames after its start,
// held on that frame (`hold`), or cut before its end (`clipped`).
Frame replayedKeyframe(const Frame &source, int offset, bool hold, bool clipped,
                       const std::function<int(const QString &)> &symbolFrames,
                       QStringList *approximations) {
  Frame result = source;
  // Labels and scripts stay with the source timeline; the replay is timing.
  result.properties.remove("name");
  result.properties.remove("labelType");
  result.extras.erase(std::remove_if(result.extras.begin(), result.extras.end(),
                                     frameScriptExtra),
                      result.extras.end());
  const QString tweenType = source.properties.value("tweenType", "none");
  if (tweenType != "none" && (hold || offset > 0 || clipped)) {
    // A held tween start is its keyframe pose; any other cut drops the
    // in-betweens a tween would interpolate.
    result.properties["tweenType"] = "none";
    if (offset > 0 || clipped)
      if (approximations)
        *approximations << source.sourceId +
                               ": the playhead holds or jumps inside this "
                               "tween; its keyframe pose is shown instead of "
                               "the in-between.";
  }
  if (!hold && offset == 0) return result;
  const int frame = source.index + offset;
  for (auto &element : result.elements) {
    if (element.kind != Element::Kind::Symbol ||
        symbolInstanceType(element) != "graphic")
      continue;
    // Graphics follow their parent's frame: show the one at the entry frame,
    // held or playing on from there.
    try {
      const GraphicTiming timing = instanceTiming(element, source);
      const int count            = std::max(
                     1, symbolFrames(element.properties.value("libraryItemName")));
      const auto shown = timing.sourceFrameAt(frame, count);
      if (!shown) continue;
      if (hold) {
        element.properties["loop"]       = "single frame";
        element.properties["firstFrame"] = QString::number(*shown);
        element.properties.remove("lastFrame");
      } else if (timing.lastFrame && *shown != timing.firstFrame &&
                 (timing.mode == PlaybackMode::Loop ||
                  timing.mode == PlaybackMode::LoopReverse)) {
        element.properties["firstFrame"] = QString::number(*shown);
        if (approximations)
          *approximations << element.sourceId +
                                 ": a looping frame range entered part way "
                                 "now loops from the entry frame.";
      } else if (timing.mode != PlaybackMode::SingleFrame)
        element.properties["firstFrame"] = QString::number(*shown);
    } catch (const Error &) {
      // Invalid poses are reported (and approximated) where they are built.
    }
  }
  return result;
}
}  // namespace

Timeline replayTimeline(const Timeline &source, const QVector<int> &frames,
                        const std::function<int(const QString &)> &symbolFrames,
                        QStringList *approximations) {
  // Runs of ticks that advance one frame per tick or hold one frame.
  struct Run {
    int tick = 0, length = 1, first = 0;
    bool hold = false;
  };
  QVector<Run> runs;
  for (int tick = 0; tick < frames.size(); ++tick) {
    const int frame = frames[tick];
    if (frame < 0 || frame >= std::max(1, source.frameCount))
      throw Error("Replayed frame outside its timeline.");
    if (!runs.isEmpty()) {
      Run &run       = runs.last();
      const int last = run.hold ? run.first : run.first + run.length - 1;
      if (frame == last + 1 && (!run.hold || run.length == 1)) {
        run.hold = false;
        ++run.length;
        continue;
      }
      if (frame == last && (run.hold || run.length == 1)) {
        run.hold = true;
        ++run.length;
        continue;
      }
    }
    runs.push_back({tick, 1, frame, false});
  }
  Timeline result;
  result.sourceId   = source.sourceId;
  result.name       = source.name;
  result.properties = source.properties;
  result.source     = source.source;
  result.frameCount = std::max(1, int(frames.size()));
  for (const auto &layer : source.layers) {
    Layer copy  = layer;
    copy.frames = {};
    for (const auto &run : runs) {
      const int end = run.hold ? run.first + 1 : run.first + run.length;
      for (const auto &frame : layer.frames) {
        const int frameEnd = frame.index + frame.duration;
        if (frameEnd <= run.first || frame.index >= end) continue;
        const int from = std::max(frame.index, run.first);
        const int to   = std::min(frameEnd, end);
        Frame replayed = replayedKeyframe(frame, from - frame.index, run.hold,
                                          !run.hold && to < frameEnd,
                                          symbolFrames, approximations);
        replayed.index = run.tick + (from - run.first);
        replayed.duration = run.hold ? run.length : to - from;
        copy.frames.push_back(std::move(replayed));
      }
    }
    result.layers.push_back(std::move(copy));
  }
  return result;
}

bool soundEnvelopeExtra(const Extra &extra) {
  return extra.tag == "SoundEnvelope";
}

std::optional<FrameSound> frameSound(const Frame &frame) {
  const Attributes &properties = frame.properties;
  if (!properties.contains("soundName")) return std::nullopt;
  FrameSound result;
  result.name     = properties.value("soundName");
  const auto note = [&](const QString &where, const QString &detail) {
    result.unmapped << where + ": " + detail;
  };
  const auto count = [&](const QString &name, qint64 &value) {
    if (!properties.contains(name)) return false;
    bool ok             = false;
    const qint64 parsed = properties.value(name).toLongLong(&ok);
    if (!ok || parsed < 0) {
      note(frame.sourceId + "/" + name,
           "Invalid sound value \"" + properties.value(name) + "\"");
      return false;
    }
    value = parsed;
    return true;
  };
  static const QMap<QString, FrameSound::Sync> syncs{
      {"event", FrameSound::Sync::Event},
      {"start", FrameSound::Sync::Start},
      {"stop", FrameSound::Sync::Stop},
      {"stream", FrameSound::Sync::Stream}};
  const QString sync = properties.value("soundSync", "event");
  if (syncs.contains(sync))
    result.sync = syncs.value(sync);
  else
    note(frame.sourceId + "/soundSync",
         "Unknown sound sync \"" + sync + "\", played as an event sound");
  const QString loopMode = properties.value("soundLoopMode", "repeat");
  if (loopMode == "loop")
    result.loopForever = true;
  else if (loopMode != "repeat")
    note(frame.sourceId + "/soundLoopMode",
         "Unknown sound loop mode \"" + loopMode + "\", played once");
  qint64 value = 0;
  // Flash writes soundLoop="0" for a sound played once.
  if (count("soundLoop", value))
    result.repeat = int(std::min<qint64>(std::max<qint64>(value, 1), 1000000));
  if (count("inPoint44", value)) result.inPoint44 = value;
  if (count("outPoint44", value)) {
    if (value > result.inPoint44)
      result.outPoint44 = value;
    else
      note(frame.sourceId + "/outPoint44",
           "Sound out point does not follow its in point");
  }
  for (const auto &extra : frame.extras) {
    if (!soundEnvelopeExtra(extra)) continue;
    if (!result.envelopeSourceId.isEmpty()) {
      note(extra.sourceId, "Duplicate sound envelope");
      continue;
    }
    result.envelopeSourceId = extra.sourceId;
    for (int index = 1; index < extra.nodes.size(); ++index) {
      const auto &node    = extra.nodes[index];
      const QString local = node.qualifiedName.section(':', -1);
      if ((!node.namespaceUri.isEmpty() &&
           node.namespaceUri != "http://ns.adobe.com/xfl/2008/") ||
          local != "SoundEnvelopePoint") {
        note(node.sourceId, "Unknown sound envelope element " + local);
        continue;
      }
      SoundEnvelopePoint point;
      for (const auto &attribute : node.attributes) {
        bool ok             = false;
        const qint64 number = attribute.value.toLongLong(&ok);
        const bool level =
            attribute.name == "level0" || attribute.name == "level1";
        if (!attribute.namespaceUri.isEmpty() ||
            (!level && attribute.name != "mark44")) {
          note(attribute.sourceId,
               "Unknown sound envelope attribute " + attribute.name);
        } else if (!ok || number < 0 || (level && number > 32768)) {
          note(attribute.sourceId,
               "Invalid sound envelope value \"" + attribute.value + "\"");
        } else if (attribute.name == "mark44")
          point.mark44 = number;
        else if (attribute.name == "level0")
          point.level0 = int(number);
        else
          point.level1 = int(number);
      }
      result.envelope.push_back(point);
    }
  }
  const QString effect = properties.value("soundEffect", "none");
  if (effect != "none" && result.envelope.isEmpty())
    note(frame.sourceId + "/soundEffect",
         "Sound effect \"" + effect + "\" has no volume envelope");
  return result;
}

double soundVolume(const FrameSound &sound, bool *exact) {
  if (exact) *exact = true;
  if (sound.envelope.isEmpty()) return 1.0;
  const int first = sound.envelope.front().level0;
  int loudest     = 0;
  bool constant   = true;
  for (const auto &point : sound.envelope) {
    constant &= point.level0 == first && point.level1 == first;
    loudest = std::max({loudest, point.level0, point.level1});
  }
  if (exact) *exact = constant;
  return double(constant ? first : loudest) / 32768.0;
}

}  // namespace Flash
