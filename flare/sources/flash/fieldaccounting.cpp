#include "fieldaccounting.h"

#include <QCryptographicHash>
#include <QJsonArray>
#include <QJsonDocument>
#include <QMap>
#include <QSet>
#include <QVector>
#include <QXmlStreamReader>

namespace Flash {
namespace {

struct Context {
  QString kind, parentName, expandedName, semanticRole, mappingStatus;
  QString firstSourceId, lastSourceId;
  quint64 count = 0;
};

struct MemberScan {
  QJsonObject report;
  QMap<QString, FieldLookup> found;
  quint64 elements = 0, attributes = 0, textCharacters = 0;
  quint64 characterRuns = 0, cdataRuns = 0, comments = 0;
  quint64 processingInstructions = 0, namespaceDeclarations = 0;
  int contextCount = 0;
};

QString expanded(const QString &uri, const QString &local) {
  return uri.isEmpty() ? local : "{" + uri + "}" + local;
}

QString coreName(const QString &expandedName) {
  static const QString xfl = "{http://ns.adobe.com/xfl/2008/}";
  if (expandedName.startsWith(xfl)) return expandedName.mid(xfl.size());
  if (expandedName.isEmpty()) return QStringLiteral("");
  return expandedName.startsWith('{') ? QString() : expandedName;
}

QString roleFor(const QString &kind, const QString &parent,
                const QString &name) {
  if (kind != "element" && kind != "attribute") return "unknown";
  const QString coreParent = coreName(parent);
  const QString coreField  = coreName(name);
  if (coreParent.isNull() || coreField.isNull()) return "unknown";
  if (coreField == "PublishSettings" || coreField == "PublishProfile" ||
      coreField == "publishSettings" || coreParent == "PublishSettings")
    return "publishing-metadata";
  // An XFL local name is only meaningful in the unqualified or official XFL
  // namespace. The scanner has no ancestry or value-dependent proof of native
  // conversion, so these roles describe source semantics only.
  if (kind == "element") {
    static const QSet<QString> behavior{"DOMDocument",
                                        "DOMSymbolItem",
                                        "timelines",
                                        "DOMTimeline",
                                        "layers",
                                        "DOMLayer",
                                        "frames",
                                        "DOMFrame",
                                        "elements",
                                        "DOMShape",
                                        "DOMSymbolInstance",
                                        "DOMGroup",
                                        "DOMBitmapInstance",
                                        "DOMStaticText",
                                        "DOMDynamicText",
                                        "DOMInputText",
                                        "DOMOvalObject",
                                        "DOMRectangleObject",
                                        "DOMVideoInstance",
                                        "members",
                                        "fills",
                                        "FillStyle",
                                        "strokes",
                                        "StrokeStyle",
                                        "edges",
                                        "Edge",
                                        "fill",
                                        "stroke",
                                        "SolidColor",
                                        "SolidStroke",
                                        "LinearGradient",
                                        "RadialGradient",
                                        "BitmapFill",
                                        "GradientEntry",
                                        "matrix",
                                        "Matrix",
                                        "transformationPoint",
                                        "Point",
                                        "color",
                                        "Color",
                                        "textRuns",
                                        "DOMTextRun",
                                        "characters",
                                        "textAttrs",
                                        "DOMTextAttrs",
                                        "media",
                                        "DOMBitmapItem",
                                        "DOMSoundItem",
                                        "DOMVideoItem",
                                        "symbols",
                                        "Include"};
    if (behavior.contains(coreField)) return "animation-behavior";
    if (coreField == "DOMFolderItem" || coreField == "folders")
      return "authoring-metadata";
    return "unknown";
  }
  if (coreParent == "DOMDocument") {
    if (coreField == "width" || coreField == "height" ||
        coreField == "frameRate" || coreField == "backgroundColor")
      return "animation-behavior";
    if (coreField == "creatorInfo" || coreField == "xflVersion" ||
        coreField == "platform")
      return "authoring-metadata";
  } else if (coreParent == "DOMFrame") {
    if (coreField == "duration" || coreField == "index" ||
        coreField == "tweenType" || coreField == "acceleration" ||
        coreField == "hasCustomEase" || coreField == "soundName" ||
        coreField == "soundSync")
      return "animation-behavior";
    if (coreField == "name" || coreField == "labelType")
      return "authoring-metadata";
    // keyMode is an incompletely decoded bitfield; leave it unknown.
  } else if (coreParent == "DOMLayer") {
    if (coreField == "layerType" || coreField == "parentLayerIndex" ||
        coreField == "visible")
      return "animation-behavior";
    if (coreField == "name") return "authoring-metadata";
    if (coreField == "color" || coreField == "autoNamed" ||
        coreField == "locked" || coreField == "current" ||
        coreField == "isSelected" || coreField == "outline" ||
        coreField == "useOutlineView")
      return "editor-state";
  } else if (coreParent == "DOMTimeline") {
    if (coreField == "name") return "authoring-metadata";
    if (coreField == "currentFrame") return "editor-state";
  } else if (coreParent == "DOMSymbolItem") {
    if (coreField == "name" || coreField == "symbolType")
      return "animation-behavior";
    if (coreField == "itemID" || coreField == "lastModified")
      return "authoring-metadata";
  } else if (coreParent == "Include") {
    if (coreField == "href") return "animation-behavior";
    if (coreField == "itemID" || coreField == "lastModified" ||
        coreField == "itemIcon" || coreField == "loadImmediate")
      return "authoring-metadata";
  } else if (coreParent == "DOMSymbolInstance" ||
             coreParent == "DOMBitmapInstance") {
    if (coreField == "libraryItemName" || coreField == "symbolType" ||
        coreField == "loop" || coreField == "firstFrame")
      return "animation-behavior";
    if (coreField == "selected") return "editor-state";
  } else if (coreParent == "DOMShape" || coreParent == "DOMGroup" ||
             coreParent == "DOMStaticText") {
    if (coreField == "selected") return "editor-state";
    if (coreParent == "DOMStaticText" &&
        (coreField == "width" || coreField == "height"))
      return "animation-behavior";
  } else if (coreParent == "Edge") {
    if (coreField == "edges" || coreField == "fillStyle0" ||
        coreField == "fillStyle1" || coreField == "strokeStyle")
      return "animation-behavior";
    // cubics are retained geometry-authoring data, but their relation to
    // drawable edges is not established by this scanner.
  } else if (coreParent == "FillStyle" || coreParent == "StrokeStyle") {
    if (coreField == "index") return "animation-behavior";
  } else if (coreParent == "SolidColor" || coreParent == "GradientEntry") {
    if (coreField == "color" || coreField == "alpha" ||
        (coreParent == "GradientEntry" && coreField == "ratio"))
      return "animation-behavior";
  } else if (coreParent == "SolidStroke") {
    if (coreField == "weight" || coreField == "scaleMode" ||
        coreField == "caps" || coreField == "joints")
      return "animation-behavior";
  } else if (coreParent == "DOMTextAttrs") {
    if (coreField == "face" || coreField == "size" ||
        coreField == "fillColor" || coreField == "autoKern" ||
        coreField == "bold" || coreField == "italic" ||
        coreField == "lineHeight")
      return "animation-behavior";
  } else if (coreParent == "Matrix") {
    if (coreField == "tx" || coreField == "ty" || coreField == "a" ||
        coreField == "b" || coreField == "c" || coreField == "d")
      return "animation-behavior";
  } else if (coreParent == "Point") {
    if (coreField == "x" || coreField == "y") return "animation-behavior";
  }
  return "unknown";
}

QString statusFor(const QString &role) {
  // Only importer bindings can assert that a field reached native Flare.
  return role == "unknown" ? "retained-only" : "unclassified";
}

void validate(const FieldAccountingLimits &l) {
  if (!l.maxXmlMemberBytes || !l.maxXmlTotalBytes || !l.maxElements ||
      !l.maxAttributes || !l.maxContentRecords || !l.maxNamespaceDeclarations ||
      l.maxDepth < 1 || l.maxContexts < 1 || l.maxReportBytes < 1 ||
      l.maxXmlTotalBytes > (1ull << 53))
    throw Error("Invalid XML field-accounting budget");
}

MemberScan scan(const QString &member, const QByteArray &bytes,
                const FieldAccountingLimits &limits,
                const QSet<QString> &targets, Package &package) {
  MemberScan result;
  QMap<QString, Context> contexts;
  QVector<QString> path, elementName;
  QVector<bool> foreignTree;
  QVector<quint64> nextChild, nextContent;
  quint64 documentContent = 0;
  const QString hash      = QString::fromLatin1(
           QCryptographicHash::hash(bytes, QCryptographicHash::Sha256).toHex());
  result.report["member"] = member;
  result.report["sha256"] = hash;
  result.report["bytes"]  = double(bytes.size());

  if (bytes.isEmpty()) {
    result.report["status"] = "empty-xml-member";
  } else {
    QXmlStreamReader xml(bytes);
    quint64 tokens = 0;
    auto record    = [&](const QString &kind, const QString &parent,
                      const QString &name, const QString &sourceId,
                      const QString &qualifiedName, const QString &value,
                      const QString &valueRepresentation,
                      bool insideForeignTree) {
      const QString role =
          insideForeignTree ? "unknown" : roleFor(kind, parent, name);
      const QString status = statusFor(role);
      // The same expanded name may occur both in real XFL and under a
      // foreign extension. Keep their semantic contexts separate.
      const QString key =
          kind + QChar(0x1f) + parent + QChar(0x1f) + name + QChar(0x1f) + role;
      auto it = contexts.find(key);
      if (it == contexts.end()) {
        if (contexts.size() >= limits.maxContexts)
          throw Error("XML field context budget exceeded: " + member);
        Context context;
        context.kind          = kind;
        context.parentName    = parent;
        context.expandedName  = name;
        context.semanticRole  = role;
        context.mappingStatus = status;
        context.firstSourceId = sourceId;
        it                    = contexts.insert(key, context);
      }
      ++it->count;
      it->lastSourceId = sourceId;
      if (targets.contains(sourceId)) {
        if (result.found.contains(sourceId))
          throw Error("Duplicate XML source locator: " + sourceId);
        result.found.insert(
               sourceId, FieldLookup{sourceId, kind, name, qualifiedName, value,
                                  valueRepresentation, role, status});
      }
    };
    auto contentId = [&] {
      return path.isEmpty()
                 ? member + "/content" + QString::number(documentContent++)
                 : path.last() + "/content" +
                       QString::number(nextContent.last()++);
    };
    auto contentBudget = [&] {
      if (result.characterRuns + result.cdataRuns + result.comments +
              result.processingInstructions >=
          limits.maxContentRecords)
        throw Error("XML content-record budget exceeded: " + member);
    };

    while (!xml.atEnd()) {
      xml.readNext();
      if (++tokens % 4096 == 0) package.checkCancelled();
      if (xml.isDTD() || xml.tokenType() == QXmlStreamReader::EntityReference)
        throw Error("XML DTD/entity is unsupported: " + member);
      if (xml.isStartElement()) {
        if (path.size() >= limits.maxDepth)
          throw Error("XML field depth budget exceeded: " + member);
        if (++result.elements > limits.maxElements)
          throw Error("XML element budget exceeded: " + member);
        const QString name =
            expanded(xml.namespaceUri().toString(), xml.name().toString());
        const QString parent =
            elementName.isEmpty() ? QString() : elementName.last();
        const bool insideForeignTree =
            (!foreignTree.isEmpty() && foreignTree.last()) ||
            coreName(name).isNull();
        const QString sourceId =
            path.isEmpty()
                ? member + "/node0"
                : path.last() + "/child" + QString::number(nextChild.last()++);
        record("element", parent, name, sourceId,
               xml.qualifiedName().toString(), {}, "none", insideForeignTree);
        path.append(sourceId);
        elementName.append(name);
        foreignTree.append(insideForeignTree);
        nextChild.append(0);
        nextContent.append(0);
        QSet<QString> seenDeclarations;
        for (const auto &declaration : xml.namespaceDeclarations()) {
          if (++result.namespaceDeclarations > limits.maxNamespaceDeclarations)
            throw Error("XML namespace-declaration budget exceeded: " + member);
          const QString prefix = declaration.prefix().toString();
          if (seenDeclarations.contains(prefix))
            throw Error("Duplicate XML namespace prefix: " + sourceId);
          seenDeclarations.insert(prefix);
          const QString qualified =
              prefix.isEmpty() ? "xmlns" : "xmlns:" + prefix;
          const QString expandedName = "{http://www.w3.org/2000/xmlns/}" +
                                       (prefix.isEmpty() ? "xmlns" : prefix);
          record("namespace-declaration", name, expandedName,
                 sourceId + "/@" + qualified, qualified,
                 declaration.namespaceUri().toString(), "xml-parser-normalized",
                 insideForeignTree);
        }
        const auto attributes = xml.attributes();
        QSet<QString> seenAttributes;
        for (const auto &attribute : attributes) {
          if (++result.attributes > limits.maxAttributes)
            throw Error("XML attribute budget exceeded: " + member);
          const QString attributeName = expanded(
              attribute.namespaceUri().toString(), attribute.name().toString());
          if (seenAttributes.contains(attributeName))
            throw Error("Duplicate expanded XML attribute: " + sourceId);
          seenAttributes.insert(attributeName);
          record("attribute", name, attributeName,
                 sourceId + "/@" + attributeName,
                 attribute.qualifiedName().toString(),
                 attribute.value().toString(), "xml-parser-normalized",
                 insideForeignTree);
        }
      } else if (xml.isEndElement()) {
        if (path.isEmpty()) throw Error("Unexpected XML end: " + member);
        path.removeLast();
        elementName.removeLast();
        foreignTree.removeLast();
        nextChild.removeLast();
        nextContent.removeLast();
      } else if (xml.isCharacters()) {
        contentBudget();
        const bool cdata = xml.isCDATA();
        if (cdata)
          ++result.cdataRuns;
        else
          ++result.characterRuns;
        const QString value = xml.text().toString();
        record(cdata ? "cdata" : "characters",
               elementName.isEmpty() ? QString() : elementName.last(),
               cdata ? "#cdata" : "#text", contentId(),
               cdata ? "#cdata" : "#text", value, "xml-parser-normalized",
               !foreignTree.isEmpty() && foreignTree.last());
        result.textCharacters += quint64(xml.text().size());
      } else if (xml.isComment()) {
        contentBudget();
        ++result.comments;
        record("comment",
               elementName.isEmpty() ? QString() : elementName.last(),
               "#comment", contentId(), "#comment", xml.text().toString(),
               "xml-parser-normalized",
               !foreignTree.isEmpty() && foreignTree.last());
      } else if (xml.isProcessingInstruction()) {
        contentBudget();
        ++result.processingInstructions;
        const QString targetName = xml.processingInstructionTarget().toString();
        record("processing-instruction",
               elementName.isEmpty() ? QString() : elementName.last(),
               targetName, contentId(), targetName,
               xml.processingInstructionData().toString(),
               "xml-parser-normalized",
               !foreignTree.isEmpty() && foreignTree.last());
      }
    }
    if (xml.hasError() || !path.isEmpty())
      throw Error("Malformed XML field member " + member + ": " +
                  xml.errorString());
    result.report["status"] = "parsed";
  }

  QJsonArray contextRows;
  for (const Context &c : contexts) {
    QJsonObject row;
    row["kind"]                = c.kind;
    row["parentExpandedName"]  = c.parentName;
    row["expandedName"]        = c.expandedName;
    row["count"]               = double(c.count);
    row["firstSourceId"]       = c.firstSourceId;
    row["lastSourceId"]        = c.lastSourceId;
    row["semanticRole"]        = c.semanticRole;
    row["nativeMappingStatus"] = c.mappingStatus;
    contextRows.append(row);
  }
  result.contextCount                = contextRows.size();
  result.report["elementCount"]      = double(result.elements);
  result.report["attributeCount"]    = double(result.attributes);
  result.report["textCharacters"]    = double(result.textCharacters);
  result.report["characterRunCount"] = double(result.characterRuns);
  result.report["cdataRunCount"]     = double(result.cdataRuns);
  result.report["commentCount"]      = double(result.comments);
  result.report["processingInstructionCount"] =
      double(result.processingInstructions);
  result.report["namespaceDeclarationCount"] =
      double(result.namespaceDeclarations);
  result.report["contexts"] = contextRows;
  if (QJsonDocument(result.report).toJson(QJsonDocument::Compact).size() >
      limits.maxReportBytes)
    throw Error("XML field member report-byte budget exceeded: " + member);
  return result;
}

}  // namespace

QJsonObject accountXmlFields(Package &package,
                             const FieldAccountingLimits &limits) {
  validate(limits);
  QJsonArray members;
  quint64 totalBytes = 0, elements = 0, attributes = 0;
  quint64 characterRuns = 0, cdataRuns = 0, comments = 0;
  quint64 processingInstructions = 0, namespaceDeclarations = 0;
  int contexts = 0, ignoredMembers = 0;
  quint64 memberJsonBytes = 2;  // JSON array brackets.
  const QStringList names = package.names();
  for (const QString &name : names) {
    package.checkCancelled();
    if (!name.endsWith(".xml", Qt::CaseInsensitive)) {
      ++ignoredMembers;
      continue;
    }
    const QByteArray bytes = package.read(name, limits.maxXmlMemberBytes);
    if (quint64(bytes.size()) > limits.maxXmlTotalBytes - totalBytes)
      throw Error("XML field total-byte budget exceeded");
    totalBytes += quint64(bytes.size());
    MemberScan scanned = scan(name, bytes, limits, {}, package);
    if (scanned.elements > limits.maxElements - elements ||
        scanned.attributes > limits.maxAttributes - attributes ||
        scanned.characterRuns + scanned.cdataRuns + scanned.comments +
                scanned.processingInstructions >
            limits.maxContentRecords - (characterRuns + cdataRuns + comments +
                                        processingInstructions) ||
        scanned.namespaceDeclarations >
            limits.maxNamespaceDeclarations - namespaceDeclarations ||
        scanned.contextCount > limits.maxContexts - contexts)
      throw Error("XML field cumulative budget exceeded");
    elements += scanned.elements;
    attributes += scanned.attributes;
    characterRuns += scanned.characterRuns;
    cdataRuns += scanned.cdataRuns;
    comments += scanned.comments;
    processingInstructions += scanned.processingInstructions;
    namespaceDeclarations += scanned.namespaceDeclarations;
    contexts += scanned.contextCount;
    const quint64 serializedBytes = quint64(
        QJsonDocument(scanned.report).toJson(QJsonDocument::Compact).size());
    const quint64 additional = serializedBytes + (members.isEmpty() ? 0 : 1);
    if (memberJsonBytes > quint64(limits.maxReportBytes) ||
        additional > quint64(limits.maxReportBytes) - memberJsonBytes)
      throw Error("XML field cumulative report-byte budget exceeded");
    memberJsonBytes += additional;
    members.append(scanned.report);
  }
  QJsonObject report;
  report["schemaVersion"]              = 2;
  report["classifierVersion"]          = 2;
  report["scope"]                      = "all-package-xml-parser-records";
  report["memberSelection"]            = "case-insensitive-.xml-suffix";
  report["unlocatedSyntax"]            = QJsonArray{"xml-declaration"};
  report["contentTokenization"]        = "QXmlStreamReader-token-boundaries";
  report["lookupValueRepresentation"]  = "xml-parser-normalized";
  report["exactLexicalAuthority"]      = "retained-source-member-bytes";
  report["nativeFidelityVerified"]     = false;
  report["members"]                    = members;
  report["xmlMemberCount"]             = members.size();
  report["ignoredNonXmlMemberCount"]   = ignoredMembers;
  report["xmlBytes"]                   = double(totalBytes);
  report["elementCount"]               = double(elements);
  report["attributeCount"]             = double(attributes);
  report["characterRunCount"]          = double(characterRuns);
  report["cdataRunCount"]              = double(cdataRuns);
  report["commentCount"]               = double(comments);
  report["processingInstructionCount"] = double(processingInstructions);
  report["namespaceDeclarationCount"]  = double(namespaceDeclarations);
  report["contextCount"]               = contexts;
  if (QJsonDocument(report).toJson(QJsonDocument::Compact).size() >
      limits.maxReportBytes)
    throw Error("XML field report-byte budget exceeded");
  return report;
}

QMap<QString, FieldLookup> lookupXmlFields(
    Package &package, const QJsonObject &accounting, const QString &member,
    const QSet<QString> &sourceIds, const FieldAccountingLimits &limits) {
  validate(limits);
  const int version = accounting.value("schemaVersion").toInt();
  if (version != 2 ||
      accounting.value("classifierVersion").toInt() != version ||
      sourceIds.isEmpty() || sourceIds.contains(QString()) ||
      quint64(sourceIds.size()) > limits.maxAttributes)
    throw Error("Invalid XML field accounting report or locator");
  QString expectedHash;
  for (const auto &value : accounting.value("members").toArray()) {
    const QJsonObject row = value.toObject();
    if (row.value("member").toString() == member) {
      if (!expectedHash.isEmpty())
        throw Error("Duplicate XML member in report");
      expectedHash = row.value("sha256").toString();
    }
  }
  if (expectedHash.size() != 64 ||
      !member.endsWith(".xml", Qt::CaseInsensitive))
    throw Error("XML member absent from field accounting report");
  const QByteArray bytes   = package.read(member, limits.maxXmlMemberBytes);
  const QString actualHash = QString::fromLatin1(
      QCryptographicHash::hash(bytes, QCryptographicHash::Sha256).toHex());
  if (actualHash != expectedHash)
    throw Error("XML member changed since field accounting: " + member);
  return scan(member, bytes, limits, sourceIds, package).found;
}

}  // namespace Flash
