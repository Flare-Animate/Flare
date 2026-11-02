#include "document.h"
#include "geometry.h"
#include "model.h"

#include <QJsonArray>
#include <QUrl>
#include <QXmlStreamReader>
#include <algorithm>
#include <cmath>
#include <functional>

namespace Flash {
namespace {

QJsonObject counts(const QMap<QString, quint64> &values) {
  QJsonObject json;
  for (auto it = values.cbegin(); it != values.cend(); ++it)
    json.insert(it.key(), double(it.value()));
  return json;
}

void issue(DocumentInfo &info, const QString &severity, const QString &code,
           const QString &member, const QString &detail) {
  info.issues.append(QJsonObject{{"severity", severity},
                                 {"code", code},
                                 {"member", member},
                                 {"detail", detail}});
}

QString attributeValue(const QXmlStreamAttributes &attributes,
                       const QString &name) {
  for (const auto &attribute : attributes)
    if (attribute.namespaceUri().isEmpty() && attribute.name() == name)
      return attribute.value().toString();
  return {};
}

bool hasAttribute(const QXmlStreamAttributes &attributes, const QString &name) {
  for (const auto &attribute : attributes)
    if (attribute.namespaceUri().isEmpty() && attribute.name() == name)
      return true;
  return false;
}

QString elementName(const QXmlStreamReader &xml) {
  const QString uri  = xml.namespaceUri().toString();
  const QString name = xml.name().toString();
  return uri.isEmpty() || uri == "http://ns.adobe.com/xfl/2008/"
             ? name
             : "{" + uri + "}" + name;
}

int integer(const QXmlStreamAttributes &attributes, const QString &name,
            int fallback, int minimum, int maximum) {
  if (!hasAttribute(attributes, name)) return fallback;
  bool ok         = false;
  const int value = attributeValue(attributes, name).toInt(&ok);
  if (!ok || value < minimum || value > maximum)
    throw Error("Invalid XFL integer attribute: " + name);
  return value;
}

QString memberName(const QString &href, Package &package) {
  const QString direct = "LIBRARY/" + href;
  if (package.contains(direct)) return direct;
  // XFL producers may percent-encode filenames in Include records.
  const QString decoded = "LIBRARY/" + QUrl::fromPercentEncoding(href.toUtf8());
  return package.contains(decoded) ? decoded : direct;
}

struct ScanResult {
  SymbolInfo symbol;
  QSet<QString> includes;
  QSet<QString> assets;
};

ScanResult scan(Package &package, const QString &member, DocumentInfo &info,
                bool mainDocument) {
  if (info.xmlBytes >= package.limits().maxDocumentXmlBytes)
    throw Error("Document XML budget exceeded: " + member);
  const QByteArray bytes = package.read(
      member, std::min(package.limits().maxXmlBytes,
                       package.limits().maxDocumentXmlBytes - info.xmlBytes));
  info.xmlBytes += bytes.size();
  ++info.xmlMembers;
  QXmlStreamReader xml(bytes);
  ScanResult result;
  result.symbol.member = member;
  int depth            = 0;
  int timelineDepth = -1, layerDepth = -1;
  int timeline = -1, layer = -1;
  int previousSpanEnd = 0;
  bool rootSeen       = false;
  int foreignDepth    = -1;
  while (!xml.atEnd()) {
    const auto token = xml.readNext();
    package.checkCancelled();
    if (token == QXmlStreamReader::DTD ||
        token == QXmlStreamReader::EntityReference)
      throw Error(
          "XML declarations or unresolved entities are not supported: " +
          member);
    if (token == QXmlStreamReader::EndElement) {
      if (foreignDepth >= 0) {
        if (depth == foreignDepth) foreignDepth = -1;
        --depth;
        continue;
      }
      if (depth == layerDepth) {
        layer      = -1;
        layerDepth = -1;
      }
      if (depth == timelineDepth) {
        timeline      = -1;
        timelineDepth = -1;
      }
      --depth;
      continue;
    }
    if (token != QXmlStreamReader::StartElement) continue;
    ++depth;
    if (depth > package.limits().maxXmlDepth)
      throw Error("XML nesting budget exceeded: " + member);
    const QString tag     = elementName(xml);
    const auto attributes = xml.attributes();
    ++info.elements[tag];
    // Keep a dedicated total so the limit does not depend on known tags.
    const quint64 elementCount = ++info.elements["$total"];
    if (elementCount > package.limits().maxXmlElements)
      throw Error("XML element budget exceeded: " + member);
    if (!rootSeen) {
      rootSeen = true;
      if (tag != (mainDocument ? "DOMDocument" : "DOMSymbolItem"))
        throw Error("Unexpected XFL root element in " + member);
      if (mainDocument) {
        info.creator  = attributeValue(attributes, "creatorInfo");
        info.version  = attributeValue(attributes, "xflVersion");
        info.platform = attributeValue(attributes, "platform");
        info.width    = integer(attributes, "width", 0, 1, 1000000);
        info.height   = integer(attributes, "height", 0, 1, 1000000);
        bool ok       = true;
        info.frameRate =
            hasAttribute(attributes, "frameRate")
                ? attributeValue(attributes, "frameRate").toDouble(&ok)
                : 24.0;
        if (!ok || !std::isfinite(info.frameRate) || info.frameRate <= 0 ||
            info.frameRate > 1000 || !info.width || !info.height)
          throw Error("Invalid XFL document size or frame rate.");
      } else {
        result.symbol.name = attributeValue(attributes, "name");
        result.symbol.type = attributeValue(attributes, "symbolType");
        if (result.symbol.name.isEmpty())
          throw Error("Unnamed library symbol: " + member);
      }
    }
    // A foreign extension can contain familiar local names. Its entire
    // subtree remains source data and cannot drive XFL scene inspection.
    if (foreignDepth >= 0 ||
        (!xml.namespaceUri().isEmpty() &&
         xml.namespaceUri() != "http://ns.adobe.com/xfl/2008/")) {
      if (foreignDepth < 0) foreignDepth = depth;
      continue;
    }
    if (tag == "Include") {
      const QString href = attributeValue(attributes, "href");
      if (href.isEmpty()) throw Error("Empty library Include: " + member);
      result.includes.insert(memberName(href, package));
    } else if (tag == "DOMTimeline") {
      if (timeline >= 0) throw Error("Nested timeline XML in " + member);
      TimelineInfo value;
      value.name = attributeValue(attributes, "name");
      result.symbol.timelines.push_back(value);
      timeline      = result.symbol.timelines.size() - 1;
      timelineDepth = depth;
    } else if (tag == "DOMLayer") {
      if (timeline < 0 || layer >= 0)
        throw Error("Layer outside timeline: " + member);
      LayerInfo value;
      value.name = attributeValue(attributes, "name");
      value.type = attributeValue(attributes, "layerType");
      if (value.type.isEmpty()) value.type = "normal";
      value.parent  = integer(attributes, "parentLayerIndex", -1, -1, 100000);
      value.visible = attributeValue(attributes, "visible") != "false";
      value.locked  = attributeValue(attributes, "locked") == "true";
      ++info.layerTypes[value.type];
      result.symbol.timelines[timeline].layers.push_back(value);
      layer           = result.symbol.timelines[timeline].layers.size() - 1;
      layerDepth      = depth;
      previousSpanEnd = 0;
    } else if (tag == "DOMFrame") {
      if (timeline < 0 || layer < 0)
        throw Error("Frame outside layer: " + member);
      SpanInfo span;
      span.index    = integer(attributes, "index", 0, 0, 10000000);
      span.duration = integer(attributes, "duration", 1, 1, 10000000);
      if (span.index > 10000000 - span.duration || span.index < previousSpanEnd)
        throw Error("Overlapping or excessive frame span: " + member);
      previousSpanEnd = span.index + span.duration;
      span.tween      = attributeValue(attributes, "tweenType");
      if (!span.tween.isEmpty()) ++info.tweenTypes[span.tween];
      auto &value      = result.symbol.timelines[timeline];
      value.frameCount = std::max(value.frameCount, previousSpanEnd);
      value.layers[layer].spans.push_back(span);
    } else if (tag == "DOMSymbolInstance") {
      const QString name = attributeValue(attributes, "libraryItemName");
      if (name.isEmpty())
        throw Error("Symbol instance without library reference: " + member);
      // A camera layer's __Camera__ instance is built into Animate; its
      // library has no such item.
      const bool cameraLayer =
          timeline >= 0 && layer >= 0 &&
          result.symbol.timelines[timeline].layers[layer].type == "camera";
      if (!cameraLayer) result.symbol.references.insert(name);
      const QString mode = attributeValue(attributes, "loop");
      ++info.playbackModes[mode.isEmpty() ? "unspecified" : mode];
    } else if (tag == "Edge" && info.geometryVerified) {
      const QString encoded = attributeValue(attributes, "edges");
      if (!encoded.isEmpty()) {
        const quint64 remaining =
            package.limits().maxGeometrySegments -
            std::min(info.geometrySegments,
                     package.limits().maxGeometrySegments);
        try {
          info.geometrySegments += quint64(
              decodeEdges(encoded, remaining, package.limits().cancelled)
                  .size());
        } catch (const Error &error) {
          throw Error(QString("%1 in %2 at line %3")
                          .arg(error.what())
                          .arg(member)
                          .arg(xml.lineNumber()));
        }
      }
    } else if (tag == "DOMTextAttrs") {
      const QString face = attributeValue(attributes, "face");
      if (!face.isEmpty()) info.fonts.insert(face);
    } else if (tag == "DOMBitmapItem" || tag == "DOMSoundItem" ||
               tag == "DOMVideoItem") {
      QJsonObject asset{{"type", tag}};
      for (const auto &attribute : attributes) {
        const QString uri = attribute.namespaceUri().toString();
        const QString key = uri.isEmpty()
                                ? attribute.name().toString()
                                : "{" + uri + "}" + attribute.name().toString();
        asset.insert(key, attribute.value().toString());
      }
      const QString name = attributeValue(attributes, "name");
      if (!name.isEmpty()) info.assets.insert(name, asset);
      for (const auto &attribute : attributes) {
        if (!attribute.namespaceUri().isEmpty()) continue;
        const QString key = attribute.name().toString();
        if (key.endsWith("DataHRef", Qt::CaseInsensitive)) {
          QString reference = attribute.value().toString();
          if (!package.contains(reference) &&
              package.contains("bin/" + reference))
            reference = "bin/" + reference;
          result.assets.insert(reference);
        }
      }
    }
  }
  if (xml.hasError())
    throw Error(QString("XML error in %1 at line %2: %3")
                    .arg(member)
                    .arg(xml.lineNumber())
                    .arg(xml.errorString()));
  if (!rootSeen) throw Error("Empty XML document: " + member);
  for (auto &value : result.symbol.timelines) {
    QVector<int> parents;
    QVector<QString> types;
    for (const auto &layer : value.layers) {
      parents.push_back(layer.parent);
      types.push_back(layer.type);
    }
    parents = layerParentIndices(parents, types);
    for (int index = 0; index < value.layers.size(); ++index)
      value.layers[index].parent = parents[index];
    switch (layerParentProblem(parents)) {
    case LayerParentProblem::None:
      break;
    case LayerParentProblem::Invalid:
      throw Error("Invalid layer parent in " + member);
    case LayerParentProblem::Cyclic:
      throw Error("Cyclic layer parents in " + member);
    }
  }
  return result;
}

void inspectGraph(DocumentInfo &info, int maxSymbolDepth) {
  QMap<QString, int> state, depths;
  // Iterative DFS also handles intentionally deep malformed graphs safely.
  struct Visit {
    QString name;
    QStringList children;
    int index = 0;
    int depth = 1;
  };
  for (auto symbol = info.symbols.cbegin(); symbol != info.symbols.cend();
       ++symbol) {
    if (state.value(symbol.key()) == 2) continue;
    QVector<Visit> stack;
    stack.push_back({symbol.key(), symbol.value().references.values(), 0, 1});
    state[symbol.key()] = 1;
    while (!stack.isEmpty()) {
      auto &visit = stack.last();
      if (visit.index == visit.children.size()) {
        depths[visit.name]  = visit.depth;
        state[visit.name]   = 2;
        info.maxSymbolDepth = std::max(info.maxSymbolDepth, visit.depth);
        if (visit.depth > maxSymbolDepth)
          throw Error(QString("Symbol nesting depth %1 exceeds the limit of "
                              "%2 at symbol: %3")
                          .arg(visit.depth)
                          .arg(maxSymbolDepth)
                          .arg(visit.name.left(256)));
        const int completedDepth = visit.depth;
        stack.removeLast();
        if (!stack.isEmpty())
          stack.last().depth = std::max(stack.last().depth, completedDepth + 1);
        continue;
      }
      const QString child = visit.children[visit.index++];
      if (!info.symbols.contains(child)) {
        issue(info, "error", "missing-symbol", info.symbols[visit.name].member,
              child);
      } else if (state.value(child) == 1) {
        issue(info, "error", "cyclic-symbol", info.symbols[visit.name].member,
              child);
      } else if (state.value(child) == 2) {
        visit.depth = std::max(visit.depth, depths.value(child) + 1);
      } else {
        if (stack.size() >= maxSymbolDepth)
          throw Error(QString("Symbol nesting depth exceeds the limit of %1 "
                              "at symbol: %2")
                          .arg(maxSymbolDepth)
                          .arg(child.left(256)));
        state[child] = 1;
        stack.push_back({child, info.symbols[child].references.values(), 0, 1});
      }
    }
  }
  QSet<QString> reachable;
  QStringList pending = info.rootReferences.values();
  while (!pending.isEmpty()) {
    const QString name = pending.takeLast();
    if (reachable.contains(name)) continue;
    if (!info.symbols.contains(name)) {
      issue(info, "error", "missing-symbol", "DOMDocument.xml", name);
      continue;
    }
    reachable.insert(name);
    pending.append(info.symbols[name].references.values());
  }
  info.reachableSymbols = reachable.size();
}

QJsonArray timelinesJson(const QVector<TimelineInfo> &timelines) {
  QJsonArray result;
  for (const auto &timeline : timelines) {
    QJsonArray layers;
    for (const auto &layer : timeline.layers) {
      QJsonArray spans;
      for (const auto &span : layer.spans)
        spans.append(QJsonObject{{"index", span.index},
                                 {"duration", span.duration},
                                 {"tween", span.tween}});
      layers.append(QJsonObject{{"name", layer.name},
                                {"type", layer.type},
                                {"parent", layer.parent},
                                {"visible", layer.visible},
                                {"locked", layer.locked},
                                {"spans", spans}});
    }
    result.append(QJsonObject{{"name", timeline.name},
                              {"frameCount", timeline.frameCount},
                              {"layers", layers}});
  }
  return result;
}

}  // namespace

LayerParentProblem layerParentProblem(const QVector<int> &parents) {
  for (int i = 0; i < parents.size(); ++i) {
    const int parent = parents[i];
    if (parent >= parents.size() || parent == i)
      return LayerParentProblem::Invalid;
    QSet<int> seen;
    int ancestor = parent;
    while (ancestor >= 0) {
      if (seen.contains(ancestor)) return LayerParentProblem::Cyclic;
      seen.insert(ancestor);
      ancestor = parents[ancestor];
    }
  }
  return LayerParentProblem::None;
}

QVector<int> layerParentIndices(const QVector<int> &authored,
                                const QVector<QString> &types) {
  QVector<int> counted;  // authored index -> layer index
  for (int index = 0; index < types.size(); ++index)
    if (types[index] != "camera") counted.push_back(index);
  if (counted.size() == types.size()) return authored;
  QVector<int> result;
  for (int parent : authored)
    result.push_back(parent < 0                ? parent
                     : parent < counted.size() ? counted[parent]
                                               : types.size());
  return result;
}

DocumentInfo inspect(Package &package, bool verifyGeometry, bool verifyModel) {
  DocumentInfo info;
  info.geometryVerified       = verifyGeometry || verifyModel;
  info.directoryCompatibility = package.usedDirectoryCompatibility();
  const ScanResult root       = scan(package, "DOMDocument.xml", info, true);
  info.timelines              = root.symbol.timelines;
  info.rootReferences         = root.symbol.references;
  if (info.timelines.isEmpty())
    throw Error("XFL document contains no timelines.");
  if (info.version != "2.1" && info.version != "2.7")
    issue(info, "warning", "untested-xfl-version", "DOMDocument.xml",
          info.version);
  if (info.directoryCompatibility)
    issue(info, "information", "adobe-directory-size", "",
          QString::number(package.declaredDirectorySizeDifference()));
  QSet<QString> includes = root.includes, media = root.assets;
  for (const QString &name : package.names()) {
    if (!name.startsWith("LIBRARY/") ||
        !name.endsWith(".xml", Qt::CaseInsensitive))
      continue;
    const ScanResult result = scan(package, name, info, false);
    if (info.symbols.contains(result.symbol.name))
      throw Error("Duplicate library symbol: " + result.symbol.name);
    info.symbols.insert(result.symbol.name, result.symbol);
    includes.unite(result.includes);
    media.unite(result.assets);
  }
  for (const QString &name : includes)
    if (!package.contains(name))
      issue(info, "error", "missing-library-xml", "DOMDocument.xml", name);
  for (const QString &name : media)
    if (!package.contains(name))
      issue(info, "error", "missing-embedded-media", "DOMDocument.xml", name);
  inspectGraph(info, package.limits().maxSymbolDepth);
  if (verifyModel) {
    info.modelVerified = true;
    auto countElement  = [&](const Element &element, auto &&recurse) -> void {
      ++info.modelElements;
      if (element.shape) ++info.modelShapes;
      for (const auto &child : element.members) recurse(child, recurse);
    };
    auto checkDefinition = [&](const QString &member) {
      const Definition model = readDefinition(package, member);
      ++info.modelDefinitions;
      info.modelGeometrySegments += model.geometrySegments;
      for (const auto &timeline : model.timelines)
        for (const auto &layer : timeline.layers)
          for (const auto &frame : layer.frames)
            for (const auto &element : frame.elements)
              countElement(element, countElement);
    };
    checkDefinition("DOMDocument.xml");
    for (const auto &symbol : info.symbols) checkDefinition(symbol.member);
    if (info.modelGeometrySegments != info.geometrySegments)
      issue(info, "warning", "unmodeled-geometry", "",
            QString("%1 decoded source segments, %2 modeled segments")
                .arg(info.geometrySegments)
                .arg(info.modelGeometrySegments));
  }
  return info;
}

QJsonObject DocumentInfo::toJson() const {
  QJsonArray symbolList;
  for (const auto &symbol : symbols) {
    QStringList references = symbol.references.values();
    references.sort();
    symbolList.append(
        QJsonObject{{"name", symbol.name},
                    {"member", symbol.member},
                    {"type", symbol.type},
                    {"references", QJsonArray::fromStringList(references)},
                    {"timelines", timelinesJson(symbol.timelines)}});
  }
  QStringList fontList = fonts.values();
  fontList.sort();
  return QJsonObject{{"creator", creator},
                     {"xflVersion", version},
                     {"platform", platform},
                     {"width", width},
                     {"height", height},
                     {"frameRate", frameRate},
                     {"directoryCompatibility", directoryCompatibility},
                     {"xmlBytes", double(xmlBytes)},
                     {"geometryVerified", geometryVerified},
                     {"geometrySegments", double(geometrySegments)},
                     {"modelVerified", modelVerified},
                     {"modelDefinitions", double(modelDefinitions)},
                     {"modelShapes", double(modelShapes)},
                     {"modelElements", double(modelElements)},
                     {"modelGeometrySegments", double(modelGeometrySegments)},
                     {"xmlMembers", xmlMembers},
                     {"maxSymbolDepth", maxSymbolDepth},
                     {"reachableSymbols", reachableSymbols},
                     {"fonts", QJsonArray::fromStringList(fontList)},
                     {"elements", counts(elements)},
                     {"tweens", counts(tweenTypes)},
                     {"playbackModes", counts(playbackModes)},
                     {"layerTypes", counts(layerTypes)},
                     {"assets", assets},
                     {"timelines", timelinesJson(timelines)},
                     {"symbols", symbolList},
                     {"issues", issues}};
}

}  // namespace Flash
