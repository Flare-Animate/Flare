#include "flashfieldbindings.h"
#include "flashbundle.h"
#include "../flash/fieldaccounting.h"

#include "flare/flashinstance.h"
#include "flare/levelset.h"
#include "flare/toonzscene.h"
#include "flare/txshchildlevel.h"
#include "flare/txshlevelcolumn.h"
#include "flare/txsheet.h"

#include <QHash>
#include <QJsonDocument>
#include <QMap>
#include <QSet>
#include <algorithm>
#include <cmath>

namespace FlashNative {
namespace {

constexpr int maxVerifiedFields    = 100000;
constexpr int maxExaminedInstances = 20000;
constexpr int maxExaminedLabels    = 20000;
constexpr int maxSidecarBytes      = 16 * 1024 * 1024;

struct InstanceSite {
  TXshLevelColumn *column = nullptr;
  int index               = -1;
};

struct InstanceIndex {
  QHash<QString, InstanceSite> sites;
  QSet<QString> ambiguous;
  bool budgetExceeded = false;
};

struct LabelSite {
  const FlashFrameLabelData *record = nullptr;
};

struct LabelIndex {
  QMap<QString, LabelSite> sites;
  QSet<QString> ambiguous;
  bool budgetExceeded = false;
};

LabelIndex indexLabels(const ToonzScene &scene) {
  LabelIndex result;
  QSet<TXsheet *> sheets;
  sheets.insert(scene.getTopXsheet());
  auto *levels = scene.getLevelSet();
  for (int i = 0; i < levels->getLevelCount(); ++i)
    if (auto *child = levels->getLevel(i)->getChildLevel())
      sheets.insert(child->getXsheet());
  int visited = 0;
  for (auto *sheet : sheets) {
    if (!sheet) continue;
    for (const auto &label : sheet->flashFrameLabels()) {
      if (++visited > 1000000) {
        result.budgetExceeded = true;
        return result;
      }
      const QString id = label.nativeLabelId;
      if (id.isEmpty() || result.ambiguous.contains(id)) continue;
      if (result.sites.contains(id)) {
        result.sites.remove(id);
        result.ambiguous.insert(id);
      } else {
        result.sites.insert(id, {&label});
      }
    }
  }
  return result;
}

InstanceIndex indexInstances(const ToonzScene &scene) {
  InstanceIndex result;
  QSet<TXsheet *> sheets;
  sheets.insert(scene.getTopXsheet());
  auto *levels = scene.getLevelSet();
  for (int i = 0; i < levels->getLevelCount(); ++i)
    if (auto *child = levels->getLevel(i)->getChildLevel())
      sheets.insert(child->getXsheet());
  int visited = 0;
  for (auto *sheet : sheets) {
    if (!sheet) continue;
    for (int column = 0; column < sheet->getColumnCount(); ++column) {
      auto *levelColumn = sheet->getColumn(column)
                              ? sheet->getColumn(column)->getLevelColumn()
                              : nullptr;
      if (!levelColumn) continue;
      const auto &instances = levelColumn->flashInstances();
      for (int index = 0; index < int(instances.size()); ++index) {
        if (++visited > 1000000) {
          result.budgetExceeded = true;
          return result;
        }
        const QString id = instances[index].nativeInstanceId;
        if (id.isEmpty() || result.ambiguous.contains(id)) continue;
        if (result.sites.contains(id)) {
          result.sites.remove(id);
          result.ambiguous.insert(id);
        } else {
          result.sites.insert(id, {levelColumn, index});
        }
      }
    }
  }
  return result;
}

QString memberFor(const QString &locator, const QStringList &members) {
  QString found;
  for (const QString &member : members)
    if (locator.startsWith(member + '/') && member.size() > found.size())
      found = member;
  return found;
}

bool exactInteger(const QString &text, int value) {
  bool ok             = false;
  const double parsed = text.toDouble(&ok);
  return ok && std::isfinite(parsed) && parsed == double(value);
}

}  // namespace

QJsonObject verifiedGraphicFieldBindings(Flash::Package &package,
                                         const QJsonObject &accounting,
                                         const QString &accountingSha256,
                                         const ToonzScene &reopened,
                                         const QJsonArray &nativeSourceMap) {
  const InstanceIndex native = indexInstances(reopened);
  if (native.budgetExceeded)
    throw Flash::Error("Graphic field-binding instance budget exceeded");
  const LabelIndex labels = indexLabels(reopened);
  if (labels.budgetExceeded)
    throw Flash::Error("Frame-label field-binding budget exceeded");

  QStringList members;
  for (const auto &entry : accounting.value("members").toArray()) {
    const QString name = entry.toObject().value("member").toString();
    if (!name.isEmpty()) members.push_back(name);
  }
  struct Candidate {
    QJsonObject sourceBinding;
    QString member, frameXmlId, elementXmlId;
    InstanceSite site;
  };
  QVector<Candidate> candidates;
  struct LabelCandidate {
    const FlashFrameLabelData *label = nullptr;
    QString member;
  };
  QVector<LabelCandidate> labelCandidates;
  QMap<QString, QSet<QString>> wanted;
  int graphicInstances = 0;
  bool truncated       = false;
  for (const auto &item : nativeSourceMap) {
    const auto binding = item.toObject();
    if (binding.value("kind") != "symbol" ||
        binding.value("nativeInstanceId").toString().isEmpty())
      continue;  // Baked movie clips do not use the graphic timing record.
    // Best-effort imports report approximated playback separately; those
    // source fields intentionally differ from the native timing.
    if (binding.value("nativeTimingApproximated").toBool()) continue;
    ++graphicInstances;
    if (candidates.size() >= maxExaminedInstances) {
      truncated = true;
      continue;
    }
    const QString id      = binding.value("nativeInstanceId").toString();
    const QString frame   = binding.value("sourceFrameXmlId").toString();
    const QString element = binding.value("sourceElementXmlId").toString();
    if (frame.isEmpty()) continue;  // Synthetic group frames have no XML node.
    const QString member = memberFor(frame, members);
    if (element.isEmpty() || member.isEmpty() ||
        memberFor(element, members) != member || !native.sites.contains(id))
      throw Flash::Error("Graphic field source or native instance is missing");
    const InstanceSite site = native.sites.value(id);
    const auto &record      = site.column->flashInstances()[site.index];
    if (record.sourceId != binding.value("sourceId").toString() ||
        !record.level || !record.level->getChildLevel() ||
        record.level->flashNativeId() !=
            binding.value("nativeLevelId").toString() ||
        record.timing.start != binding.value("nativeFirstRow").toInt(-1) ||
        record.timing.duration != binding.value("nativeRowCount").toInt(-1) ||
        !site.column->isFlashInstanceLinked(site.index))
      throw Flash::Error(
          "Reopened graphic instance does not match its source "
          "binding");
    candidates.push_back({binding, member, frame, element, site});
    auto &ids = wanted[member];
    ids.insert(frame);
    ids.insert(element);
    for (const QString &name : {"index", "duration"})
      ids.insert(frame + "/@" + name);
    for (const QString &name : {"firstFrame", "loop", "libraryItemName"})
      ids.insert(element + "/@" + name);
  }
  int frameLabels = 0;
  for (auto it = labels.sites.cbegin(); it != labels.sites.cend(); ++it) {
    const auto &label = *it.value().record;
    if (label.sourceFrameId.isEmpty()) continue;
    ++frameLabels;
    if (labelCandidates.size() >= maxExaminedLabels) {
      truncated = true;
      continue;
    }
    const QString member = memberFor(label.sourceFrameXmlId, members);
    if (label.detached || label.sourceFrameXmlId.isEmpty() ||
        member.isEmpty() || !label.sourceFrameId.startsWith(member + "/"))
      throw Flash::Error("Reopened frame label lacks a unique source locator");
    labelCandidates.push_back({&label, member});
    auto &ids = wanted[member];
    ids.insert(label.sourceFrameXmlId);
    for (const QString &name : {"index", "name", "labelType"})
      ids.insert(label.sourceFrameXmlId + "/@" + name);
  }
  if (!labels.ambiguous.isEmpty())
    throw Flash::Error("Reopened frame label has a duplicate native ID");

  QMap<QString, QMap<QString, Flash::FieldLookup>> found;
  for (auto it = wanted.cbegin(); it != wanted.cend(); ++it) {
    package.checkCancelled();
    found.insert(it.key(), Flash::lookupXmlFields(package, accounting, it.key(),
                                                  it.value()));
  }

  QJsonArray rows;
  quint64 rowBytes = 2;
  auto add         = [&](const Candidate &candidate, const QString &node,
                 const QString &attribute, const QString &destination,
                 const QJsonValue &nativeValue, bool matches) {
    const QString locator = node + "/@" + attribute;
    const auto &fields    = found.value(candidate.member);
    if (!fields.contains(locator)) return;  // Omitted source default.
    const auto &source = fields.value(locator);
    if (source.kind != "attribute" || source.expandedName != attribute ||
        source.semanticRole != "animation-behavior" || !matches)
      throw Flash::Error(
                  "Graphic source field did not reach its native "
                          "destination: " +
                  locator);
    if (rows.size() >= maxVerifiedFields) {
      truncated = true;
      return;
    }
    const QJsonObject row{
        {"sourceMember", candidate.member},
        {"sourceId", locator},
        {"sourceNodeId", node},
        {"expandedName", source.expandedName},
        {"sourceValue", source.value},
        {"valueRepresentation", source.valueRepresentation},
        {"sourcePresence", "explicit"},
        {"sourceObjectId", candidate.sourceBinding.value("sourceId")},
        {"nativeInstanceId", candidate.sourceBinding.value("nativeInstanceId")},
        {"nativeLevelId", candidate.sourceBinding.value("nativeLevelId")},
        {"nativeProperty", destination},
        {"nativeValue", nativeValue},
        {"mappingStatus", "verified-at-import"}};
    const quint64 encoded =
        quint64(QJsonDocument(row).toJson(QJsonDocument::Compact).size()) + 1;
    if (encoded > 15ull * 1024 * 1024 - rowBytes) {
      truncated = true;
      return;
    }
    rowBytes += encoded;
    rows.append(row);
  };
  for (const auto &candidate : candidates) {
    package.checkCancelled();
    const auto &fields = found.value(candidate.member);
    if (!fields.contains(candidate.frameXmlId) ||
        !fields.contains(candidate.elementXmlId) ||
        (fields.value(candidate.frameXmlId).expandedName != "DOMFrame" &&
         fields.value(candidate.frameXmlId).expandedName !=
             "{http://ns.adobe.com/xfl/2008/}DOMFrame") ||
        (fields.value(candidate.elementXmlId).expandedName !=
             "DOMSymbolInstance" &&
         fields.value(candidate.elementXmlId).expandedName !=
             "{http://ns.adobe.com/xfl/2008/}DOMSymbolInstance"))
      throw Flash::Error("Graphic source XML node changed after parsing");
    const auto &record =
        candidate.site.column->flashInstances()[candidate.site.index];
    auto value = [&](const QString &node, const QString &attribute) {
      return fields.value(node + "/@" + attribute).value;
    };
    add(candidate, candidate.frameXmlId, "index", "timing.start",
        record.timing.start,
        exactInteger(value(candidate.frameXmlId, "index"),
                     record.timing.start));
    add(candidate, candidate.frameXmlId, "duration", "timing.duration",
        record.timing.duration,
        exactInteger(value(candidate.frameXmlId, "duration"),
                     record.timing.duration));
    // A first frame past the symbol's end is kept as the frame Animate plays
    // (FlashNative builder): wrapped for loops, the last frame otherwise.
    const QString first = value(candidate.elementXmlId, "firstFrame");
    bool firstMatches   = exactInteger(first, record.timing.firstFrame);
    bool number         = false;
    const int authored  = first.toInt(&number);
    const int count     = record.symbolFrameCount;
    if (!firstMatches && number && count > 0 && authored >= count &&
        !record.timing.lastFrame)
      firstMatches =
          record.timing.firstFrame ==
          Flash::wrappedFirstFrame(authored, record.timing.mode, count);
    add(candidate, candidate.elementXmlId, "firstFrame", "timing.firstFrame",
        record.timing.firstFrame, firstMatches);
    const QString loop = value(candidate.elementXmlId, "loop");
    bool loopMatches   = false;
    if (fields.contains(candidate.elementXmlId + "/@loop"))
      loopMatches = Flash::playbackMode(loop) == record.timing.mode;
    add(candidate, candidate.elementXmlId, "loop", "timing.mode",
        Flash::playbackModeName(record.timing.mode), loopMatches);
    const QString library = value(candidate.elementXmlId, "libraryItemName");
    add(candidate, candidate.elementXmlId, "libraryItemName", "libraryName",
        record.libraryName, library == record.libraryName);
  }
  for (const auto &candidate : labelCandidates) {
    package.checkCancelled();
    const auto &label  = *candidate.label;
    const auto &fields = found.value(candidate.member);
    const QString node = label.sourceFrameXmlId;
    if (!fields.contains(node) ||
        (fields.value(node).expandedName != "DOMFrame" &&
         fields.value(node).expandedName !=
             "{http://ns.adobe.com/xfl/2008/}DOMFrame"))
      throw Flash::Error("Frame-label source XML node changed after parsing");
    const auto addLabel = [&](const QString &attribute, const QString &property,
                              const QString &role,
                              const QJsonValue &nativeValue, bool matches) {
      const QString locator = node + "/@" + attribute;
      if (!fields.contains(locator)) return;  // Omitted source default.
      const auto &source = fields.value(locator);
      if (source.kind != "attribute" || source.expandedName != attribute ||
          source.semanticRole != role || !matches)
        throw Flash::Error(
            "Frame-label source field did not reach its native "
            "destination: " +
            locator);
      if (rows.size() >= maxVerifiedFields) {
        truncated = true;
        return;
      }
      const QJsonObject row{{"sourceMember", candidate.member},
                            {"sourceId", locator},
                            {"sourceNodeId", node},
                            {"expandedName", source.expandedName},
                            {"sourceValue", source.value},
                            {"valueRepresentation", source.valueRepresentation},
                            {"sourcePresence", "explicit"},
                            {"sourceObjectId", label.sourceFrameId},
                            {"nativeLabelId", label.nativeLabelId},
                            {"nativeProperty", property},
                            {"nativeValue", nativeValue},
                            {"mappingStatus", "verified-at-import"}};
      const quint64 encoded =
          quint64(QJsonDocument(row).toJson(QJsonDocument::Compact).size()) + 1;
      if (encoded > 15ull * 1024 * 1024 - rowBytes) {
        truncated = true;
        return;
      }
      rowBytes += encoded;
      rows.append(row);
    };
    const QString sourceName = fields.value(node + "/@name").value;
    if (!fields.contains(node + "/@name"))
      throw Flash::Error("Mapped frame label lost its authored name");
    addLabel("name", "label.name", "authoring-metadata", label.name,
             sourceName == label.name);
    const QString sourceType = fields.value(node + "/@labelType").value;
    addLabel("labelType", "label.type", "authoring-metadata", label.type,
             sourceType == label.type);
    const QString sourceIndex = fields.value(node + "/@index").value;
    addLabel("index", "label.row", "animation-behavior", label.row,
             label.row == label.sourceFrameIndex &&
                 exactInteger(sourceIndex, label.sourceFrameIndex));
  }
  QJsonObject result{
      {"schemaVersion", 2},
      {"scope", "explicit-native-fields-at-import"},
      {"sourceAccountingSha256", accountingSha256},
      {"verification", "source-member-hash-and-reopened-linked-native-object"},
      {"currentNativeEditFidelityVerified", false},
      {"omittedDefaultsAreSourceFields", false},
      {"graphicInstanceCount", graphicInstances},
      {"examinedGraphicInstanceCount", candidates.size()},
      {"frameLabelCount", frameLabels},
      {"examinedFrameLabelCount", labelCandidates.size()},
      {"bindings", rows}};
  if (QJsonDocument(result).toJson(QJsonDocument::Compact).size() >
      maxSidecarBytes)
    throw Flash::Error("Graphic field-binding report budget exceeded");
  result["bindingCount"] = rows.size();
  result["truncated"]    = truncated;
  return result;
}

QJsonObject inspectCurrentGraphicFieldBindings(const ToonzScene &scene,
                                               int maxDiagnostics) {
  const auto bundle        = resolveSourceBundle(scene);
  const QJsonObject stored = bundle.fieldBindings;
  QJsonObject result{
      {"schemaVersion", 1},
      {"scope", "current-native-bound-field-values"},
      {"available", !stored.isEmpty()},
      {"editHistoryVerified", false},
      {"unsupportedFlashBehaviorVerified", false},
      {"importBindingInventoryTruncated", stored.value("truncated")}};
  if (stored.isEmpty()) {
    result["reason"] = "import-has-no-field-binding-sidecar";
    return result;
  }
  const QJsonArray bindings = stored.value("bindings").toArray();
  result["fieldCount"]      = bindings.size();
  if (bindings.size() > maxVerifiedFields) {
    result["matchesImportValue"]   = 0;
    result["changed"]              = 0;
    result["detachedOrMissing"]    = 0;
    result["ambiguous"]            = 0;
    result["unverified"]           = bindings.size();
    result["budgetExceeded"]       = true;
    result["diagnostics"]          = QJsonArray();
    result["diagnosticsTruncated"] = true;
    return result;
  }
  const InstanceIndex native = indexInstances(scene);
  const LabelIndex labels    = indexLabels(scene);
  int matches = 0, changed = 0, detached = 0, ambiguous = 0, unverified = 0;
  QJsonArray diagnostics;
  quint64 diagnosticBytes  = 2;
  bool diagnosticTruncated = false;
  maxDiagnostics           = std::max(0, std::min(maxDiagnostics, 10000));
  for (const auto &item : bindings) {
    const QJsonObject row  = item.toObject();
    const QString id       = row.value("nativeInstanceId").toString();
    const QString labelId  = row.value("nativeLabelId").toString();
    const QString property = row.value("nativeProperty").toString();
    QString status;
    QJsonValue current(QJsonValue::Undefined);
    if (native.budgetExceeded || labels.budgetExceeded) {
      status = "unverified";
    } else if ((id.isEmpty() == labelId.isEmpty()) || property.isEmpty() ||
               row.value("mappingStatus") != "verified-at-import" ||
               row.value("sourcePresence") != "explicit" ||
               row.value("sourceId").toString().isEmpty()) {
      status = "unverified";
    } else if (!labelId.isEmpty()) {
      if (labels.ambiguous.contains(labelId)) {
        status = "ambiguous";
      } else if (!labels.sites.contains(labelId)) {
        status = "detached-or-missing";
      } else {
        const auto &label = *labels.sites.value(labelId).record;
        if (label.detached ||
            label.sourceFrameId != row.value("sourceObjectId").toString() ||
            label.sourceFrameXmlId != row.value("sourceNodeId").toString()) {
          status = "detached-or-missing";
        } else {
          if (property == "label.name")
            current = label.name;
          else if (property == "label.type")
            current = label.type;
          else if (property == "label.row")
            current = label.row;
          if (current.isUndefined() || row.value("nativeValue").isUndefined() ||
              current.type() != row.value("nativeValue").type())
            status = "unverified";
          else
            status = current == row.value("nativeValue")
                         ? "matches-import-value"
                         : "changed";
        }
      }
    } else if (native.ambiguous.contains(id)) {
      status = "ambiguous";
    } else if (!native.sites.contains(id)) {
      status = "detached-or-missing";
    } else {
      const InstanceSite site = native.sites.value(id);
      const auto &record      = site.column->flashInstances()[site.index];
      if (record.sourceId != row.value("sourceObjectId").toString() ||
          !record.level || !record.level->getChildLevel() ||
          record.level->flashNativeId() !=
              row.value("nativeLevelId").toString() ||
          !site.column->isFlashInstanceLinked(site.index)) {
        status = "detached-or-missing";
      } else {
        if (property == "timing.start")
          current = record.timing.start;
        else if (property == "timing.duration")
          current = record.timing.duration;
        else if (property == "timing.firstFrame")
          current = record.timing.firstFrame;
        else if (property == "timing.mode")
          current = Flash::playbackModeName(record.timing.mode);
        else if (property == "libraryName")
          current = record.libraryName;
        if (current.isUndefined() || row.value("nativeValue").isUndefined() ||
            current.type() != row.value("nativeValue").type())
          status = "unverified";
        else
          status = current == row.value("nativeValue") ? "matches-import-value"
                                                       : "changed";
      }
    }
    if (status == "matches-import-value")
      ++matches;
    else if (status == "changed")
      ++changed;
    else if (status == "detached-or-missing")
      ++detached;
    else if (status == "ambiguous")
      ++ambiguous;
    else
      ++unverified;
    if (diagnostics.size() >= maxDiagnostics) {
      diagnosticTruncated = true;
      continue;
    }
    QJsonObject diagnostic{{"sourceId", row.value("sourceId")},
                           {"nativeInstanceId", id},
                           {"nativeProperty", property},
                           {"status", status}};
    if (!labelId.isEmpty()) diagnostic["nativeLabelId"] = labelId;
    if (!current.isUndefined()) diagnostic["currentNativeValue"] = current;
    const quint64 encoded =
        quint64(
            QJsonDocument(diagnostic).toJson(QJsonDocument::Compact).size()) +
        1;
    if (encoded <= 1024ull * 1024 - diagnosticBytes) {
      diagnostics.append(diagnostic);
      diagnosticBytes += encoded;
    } else {
      diagnosticTruncated = true;
    }
  }
  result["matchesImportValue"] = matches;
  result["changed"]            = changed;
  result["detachedOrMissing"]  = detached;
  result["ambiguous"]          = ambiguous;
  result["unverified"]         = unverified;
  result["budgetExceeded"]     = native.budgetExceeded || labels.budgetExceeded;
  result["diagnostics"]        = diagnostics;
  result["diagnosticsTruncated"] = diagnosticTruncated;
  return result;
}

}  // namespace FlashNative
