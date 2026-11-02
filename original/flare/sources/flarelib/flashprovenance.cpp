#include "flashprovenance.h"

#include "flare/levelset.h"
#include "flare/toonzscene.h"
#include "flare/txshcell.h"
#include "flare/txshchildlevel.h"
#include "flare/txshlevel.h"
#include "flare/txshleveltypes.h"
#include "flare/txshlevelcolumn.h"
#include "flare/txshsimplelevel.h"
#include "flare/txsheet.h"
#include "flare/tstageobject.h"
#include "tdoubleparam.h"
#include "tpalette.h"
#include "timage.h"
#include "trasterimage.h"
#include "traster.h"
#include "tregion.h"
#include "tstroke.h"
#include "tvectorimage.h"

#include <QCryptographicHash>
#include <QHash>
#include <QJsonObject>
#include <QSet>
#include <algorithm>
#include <cmath>
#include <limits>
#include <set>
#include <tuple>

namespace FlashNative {
namespace {

bool integer(const QJsonObject &object, const char *name, int &out) {
  const auto value = object.value(QLatin1String(name));
  if (!value.isDouble()) return false;
  const double number = value.toDouble();
  if (!std::isfinite(number) || std::floor(number) != number || number < 0 ||
      number > std::numeric_limits<int>::max())
    return false;
  out = int(number);
  return true;
}

QHash<QString, TXshLevel *> indexLevels(const ToonzScene &scene) {
  QHash<QString, TXshLevel *> indexed;
  auto *levels = scene.getLevelSet();
  for (int index = 0; index < levels->getLevelCount(); ++index) {
    auto *level        = levels->getLevel(index);
    const QString name = QString::fromStdWString(level->getName());
    if (indexed.contains(name))
      indexed[name] = nullptr;  // Duplicate name cannot resolve a binding.
    else
      indexed.insert(name, level);
  }
  return indexed;
}

struct NativeLevelIndex {
  QHash<QString, TXshLevel *> levels;
  QSet<QString> ambiguous;
};

NativeLevelIndex indexNativeLevels(const ToonzScene &scene) {
  NativeLevelIndex result;
  auto *levels = scene.getLevelSet();
  for (int index = 0; index < levels->getLevelCount(); ++index) {
    auto *level = levels->getLevel(index);
    if (!level) continue;
    const QString &id = level->flashNativeId();
    if (id.isEmpty() || result.ambiguous.contains(id)) continue;
    if (result.levels.contains(id)) {
      result.levels.remove(id);
      result.ambiguous.insert(id);
    } else {
      result.levels.insert(id, level);
    }
  }
  return result;
}

TXshLevel *bindingLevel(const QJsonObject &binding,
                        const QHash<QString, TXshLevel *> &names,
                        const NativeLevelIndex &ids) {
  if (binding.contains("nativeLevelId"))
    return ids.levels.value(binding.value("nativeLevelId").toString(), nullptr);
  return names.value(binding.value("nativeLevelName").toString(), nullptr);
}

bool kindMatches(const QString &kind, TXshLevel *level) {
  if (kind == "library-symbol" || kind == "document-scene" || kind == "group" ||
      kind == "symbol")
    return level->getChildLevel() != nullptr;
  if (kind == "shape" || kind == "oval" || kind == "rectangle" ||
      kind == "text")
    return level->getType() == PLI_XSHLEVEL;
  if (kind == "library-bitmap" || kind == "bitmap" || kind == "video")
    return level->getType() == OVL_XSHLEVEL;
  return false;
}

void stale(SourceBindingCheck &result, int index, const QString &sourceId,
           const QString &reason, int limit) {
  ++result.stale;
  if (result.diagnostics.size() >= limit) return;
  result.diagnostics.append(QJsonObject{
      {"bindingIndex", index}, {"sourceId", sourceId}, {"reason", reason}});
}

void styleDiagnostic(SourceBindingCheck &result, int bindingIndex,
                     int styleIndex, const QString &sourceStyleId,
                     const QString &reason, int limit) {
  if (result.styleDiagnostics.size() >= limit) return;
  result.styleDiagnostics.append(QJsonObject{{"bindingIndex", bindingIndex},
                                             {"styleIndex", styleIndex},
                                             {"sourceStyleId", sourceStyleId},
                                             {"reason", reason}});
}

void contentDiagnostic(SourceBindingCheck &result, int bindingIndex,
                       const QString &sourceId, const QString &reason,
                       int limit) {
  if (result.contentDiagnostics.size() >= limit) return;
  result.contentDiagnostics.append(QJsonObject{{"bindingIndex", bindingIndex},
                                               {"sourceId", sourceId},
                                               {"reason", reason}});
}

struct DigestWriter {
  QCryptographicHash hash{QCryptographicHash::Sha256};
  quint64 remaining;
  bool valid = true;

  explicit DigestWriter(quint64 budget) : remaining(budget) {}
  void bytes(const void *data, quint64 size) {
    if (!valid || size > remaining ||
        size > quint64(std::numeric_limits<int>::max())) {
      valid = false;
      return;
    }
    remaining -= size;
    hash.addData(static_cast<const char *>(data), int(size));
  }
  template <typename T>
  void scalar(const T &value) {
    bytes(&value, sizeof(T));
  }
};

bool digestNumber(DigestWriter &writer, double value) {
  if (!std::isfinite(value) || std::abs(value) > 1.0e12) return false;
  const qint64 quantized = qRound64(value * 1000000.0);
  writer.scalar(quantized);
  return writer.valid;
}

void digestString(DigestWriter &writer, const std::string &value) {
  const quint64 length = value.size();
  writer.scalar(length);
  writer.bytes(value.data(), length);
}

void digestRegion(DigestWriter &writer, const TRegion *region, int depth) {
  if (!writer.valid || !region || depth > 128) {
    writer.valid = false;
    return;
  }
  const int style = region->getStyle();
  writer.scalar(style);
  const UINT edges    = region->getEdgeCount();
  const UINT children = region->getSubregionCount();
  writer.scalar(edges);
  writer.scalar(children);
  // PLI reconstructs intersection intervals during load; their floating
  // parameters are not stable across a native save/reopen. Keep the stable
  // edge linkage, count, and fill assignment in the content baseline.
  for (UINT index = 0; index < edges && writer.valid; ++index) {
    const TEdge *edge = region->getEdge(index);
    if (!edge) {
      writer.valid = false;
      break;
    }
    writer.scalar(edge->m_type);
    writer.scalar(edge->m_index);
    writer.scalar(edge->m_styleId);
  }
  for (UINT index = 0; index < children && writer.valid; ++index)
    digestRegion(writer, region->getSubregion(index), depth + 1);
}

struct NativeInstanceSite {
  TXsheet *sheet          = nullptr;
  TXshLevelColumn *column = nullptr;
  int columnIndex         = -1;
  int instanceIndex       = -1;
};

struct NativeInstanceIndex {
  QHash<QString, NativeInstanceSite> sites;
  QSet<QString> ambiguous;
  bool budgetExceeded = false;
};

struct NativePlacementSite {
  TXsheet *sheet          = nullptr;
  TXshLevelColumn *column = nullptr;
  int columnIndex         = -1;
  int placementIndex      = -1;
};

struct NativePlacementIndex {
  QHash<QString, NativePlacementSite> sites;
  QSet<QString> ambiguous;
  bool budgetExceeded = false;
};

NativePlacementIndex indexPlacements(const ToonzScene &scene) {
  NativePlacementIndex result;
  QSet<TXsheet *> sheets;
  sheets.insert(scene.getTopXsheet());
  auto *levels = scene.getLevelSet();
  for (int index = 0; index < levels->getLevelCount(); ++index)
    if (auto *child = levels->getLevel(index)->getChildLevel())
      sheets.insert(child->getXsheet());
  constexpr int maxPlacementsToIndex = 1000000;
  int visitedPlacements              = 0;
  for (auto *sheet : sheets) {
    if (!sheet) continue;
    for (int columnIndex = 0; columnIndex < sheet->getColumnCount();
         ++columnIndex) {
      auto *column = sheet->getColumn(columnIndex);
      auto *native = column ? column->getLevelColumn() : nullptr;
      if (!native) continue;
      const auto &placements = native->flashPlacements();
      for (int placementIndex = 0; placementIndex < int(placements.size());
           ++placementIndex) {
        if (++visitedPlacements > maxPlacementsToIndex) {
          result.budgetExceeded = true;
          return result;
        }
        const QString &id = placements[placementIndex].nativePlacementId;
        if (id.isEmpty() || result.ambiguous.contains(id)) continue;
        if (result.sites.contains(id)) {
          result.sites.remove(id);
          result.ambiguous.insert(id);
        } else {
          result.sites.insert(id, {sheet, native, columnIndex, placementIndex});
        }
      }
    }
  }
  return result;
}

NativeInstanceIndex indexInstances(const ToonzScene &scene) {
  NativeInstanceIndex result;
  QSet<TXsheet *> sheets;
  sheets.insert(scene.getTopXsheet());
  auto *levels = scene.getLevelSet();
  for (int index = 0; index < levels->getLevelCount(); ++index)
    if (auto *child = levels->getLevel(index)->getChildLevel())
      sheets.insert(child->getXsheet());
  constexpr int maxInstancesToIndex = 1000000;
  int visitedInstances              = 0;
  for (auto *sheet : sheets) {
    if (!sheet) continue;
    for (int columnIndex = 0; columnIndex < sheet->getColumnCount();
         ++columnIndex) {
      auto *column = sheet->getColumn(columnIndex);
      auto *native = column ? column->getLevelColumn() : nullptr;
      if (!native) continue;
      const auto &instances = native->flashInstances();
      for (int instanceIndex = 0; instanceIndex < int(instances.size());
           ++instanceIndex) {
        if (++visitedInstances > maxInstancesToIndex) {
          result.budgetExceeded = true;
          return result;
        }
        const QString &id = instances[instanceIndex].nativeInstanceId;
        if (id.isEmpty() || result.ambiguous.contains(id)) continue;
        if (result.sites.contains(id)) {
          result.sites.remove(id);
          result.ambiguous.insert(id);
        } else {
          result.sites.insert(id, {sheet, native, columnIndex, instanceIndex});
        }
      }
    }
  }
  return result;
}

}  // namespace

StageTransformBaseline stageTransformBaseline(TXsheet *sheet, int column,
                                              int firstRow, int rowCount) {
  StageTransformBaseline result;
  // The samples and key scan are intentionally bounded. A large or unusual
  // stage track must be reported unverified, never accepted from a prefix.
  constexpr int maxRows = 4096;
  constexpr int maxKeys = 65536;
  if (!sheet || column < 0 || column >= sheet->getColumnCount() ||
      firstRow < 0 || rowCount < 1 || rowCount > maxRows ||
      rowCount > std::numeric_limits<int>::max() - firstRow)
    return result;
  auto *object = sheet->getStageObject(TStageObjectId::ColumnId(column));
  if (!object) return result;

  DigestWriter keys(16ull * 1024 * 1024);
  const quint32 version = 1;
  keys.scalar(version);
  QSet<unsigned int> visited;
  int keyCount = 0;
  for (int depth = 0; object && depth < 32; ++depth) {
    const auto id = object->getId();
    if (visited.contains(id.getCode())) return result;
    visited.insert(id.getCode());
    digestString(keys, id.toString());
    digestString(keys, object->getParent().toString());
    digestString(keys, object->getHandle());
    digestString(keys, object->getParentHandle());
    TPointD center, offset;
    object->getCenterAndOffset(center, offset);
    if (!digestNumber(keys, center.x) || !digestNumber(keys, center.y) ||
        !digestNumber(keys, offset.x) || !digestNumber(keys, offset.y))
      return result;
    for (int channel = 0; channel < TStageObject::T_ChannelCount; ++channel) {
      auto *param = object->getParam(TStageObject::Channel(channel));
      if (!param) return result;
      const int count = param->getKeyframeCount();
      if (count < 0 || count > maxKeys - keyCount || param->isCycleEnabled())
        return result;
      keyCount += count;
      keys.scalar(count);
      for (int index = 0; index < count; ++index) {
        const auto &key = param->getKeyframe(index);
        // Expression/path/interpolated native tracks are outside this
        // conservative imported held-key baseline.
        if (key.m_type != TDoubleKeyframe::Constant) return result;
        if (!digestNumber(keys, key.m_frame) ||
            !digestNumber(keys, key.m_value))
          return result;
        keys.scalar(key.m_step);
      }
    }
    const auto parent = object->getParent();
    if (parent == TStageObjectId::NoneId) {
      object = nullptr;
      break;
    }
    object = sheet->getStageObject(parent);
  }
  if (object || !keys.valid) return result;
  result.stageKeysSha256 = QString::fromLatin1(keys.hash.result().toHex());

  auto *placed = sheet->getStageObject(TStageObjectId::ColumnId(column));
  DigestWriter samples(2ull * 1024 * 1024);
  samples.scalar(version);
  samples.scalar(rowCount);
  for (int offset = 0; offset < rowCount; ++offset) {
    for (int half = 0; half < 2; ++half) {
      const TAffine affine =
          placed->getPlacement(firstRow + offset + half * 0.5);
      if (!digestNumber(samples, affine.a11) ||
          !digestNumber(samples, affine.a12) ||
          !digestNumber(samples, affine.a13) ||
          !digestNumber(samples, affine.a21) ||
          !digestNumber(samples, affine.a22) ||
          !digestNumber(samples, affine.a23))
        return {};
    }
  }
  if (samples.valid)
    result.sampledPlacementSha256 =
        QString::fromLatin1(samples.hash.result().toHex());
  return result;
}

QString drawingContentDigest(const TImageP &image, quint64 maxBytes,
                             quint64 *bytesHashed) {
  if (bytesHashed) *bytesHashed = 0;
  if (!image) return {};
  DigestWriter writer(maxBytes);
  const quint32 version = 1;
  writer.scalar(version);
  if (TVectorImageP vector = image) {
    const quint32 kind = 1;
    writer.scalar(kind);
    const UINT strokes = vector->getStrokeCount();
    const UINT regions = vector->getRegionCount();
    writer.scalar(strokes);
    writer.scalar(regions);
    for (UINT index = 0; index < strokes && writer.valid; ++index) {
      const auto *stroke = vector->getStroke(index);
      if (!stroke) return {};
      const int style      = stroke->getStyle();
      const int groupDepth = vector->getGroupDepth(index);
      const int controls   = stroke->getControlPointCount();
      writer.scalar(style);
      writer.scalar(groupDepth);
      writer.scalar(controls);
      for (int point = 0; point < controls && writer.valid; ++point) {
        const auto control = stroke->getControlPoint(point);
        // PLI stores positions on a 1/16384 grid and thickness as a byte.
        // Quantize more coarsely so a native write/read does not register as
        // an artwork edit; this remains an edit indicator, not lossless proof.
        const qint64 x     = qRound64(control.x * 16.0);
        const qint64 y     = qRound64(control.y * 16.0);
        const qint64 thick = qRound64(control.thick * 16.0);
        writer.scalar(x);
        writer.scalar(y);
        writer.scalar(thick);
      }
    }
    for (UINT index = 0; index < regions && writer.valid; ++index)
      digestRegion(writer, vector->getRegion(index), 0);
  } else if (TRasterImageP rasterImage = image) {
    const quint32 kind = 2;
    writer.scalar(kind);
    const TRasterP raster = rasterImage->getRaster();
    if (!raster) return {};
    const int width = raster->getLx(), height = raster->getLy();
    const int pixelSize = raster->getPixelSize();
    if (width <= 0 || height <= 0 || pixelSize <= 0) return {};
    writer.scalar(width);
    writer.scalar(height);
    writer.scalar(pixelSize);
    const quint64 rowBytes = quint64(width) * quint64(pixelSize);
    if (rowBytes > quint64(std::numeric_limits<int>::max()) ||
        quint64(height) > writer.remaining / rowBytes)
      return {};
    raster->lock();
    for (int y = 0; y < height && writer.valid; ++y)
      writer.bytes(raster->getRawData(0, y), rowBytes);
    raster->unlock();
  } else {
    return {};
  }
  if (!writer.valid) return {};
  if (bytesHashed) *bytesHashed = maxBytes - writer.remaining;
  return QString::fromLatin1(writer.hash.result().toHex());
}

SourceBindingCheck inspectSourceBindings(const ToonzScene &scene,
                                         const QJsonArray &bindings) {
  const int maxDiagnostics      = 1000;
  const quint64 maxCellsToCheck = 2000000;
  SourceBindingCheck result;
  const auto levels       = indexLevels(scene);
  const auto nativeLevels = indexNativeLevels(scene);
  const auto instances    = indexInstances(scene);
  const auto placements   = indexPlacements(scene);
  for (int bindingIndex = 0; bindingIndex < bindings.size(); ++bindingIndex) {
    const auto binding = bindings.at(bindingIndex).toObject();
    if (!binding.contains("sourceTransform")) continue;
    const QString sourceId = binding.value("sourceId").toString();
    TXsheet *sheet         = nullptr;
    int column = -1, first = -1, count = 0;
    bool linked = false;
    if (binding.contains("nativePlacementId") && !placements.budgetExceeded) {
      const auto id = binding.value("nativePlacementId").toString();
      if (!placements.ambiguous.contains(id)) {
        const auto site = placements.sites.value(id);
        if (site.column && site.placementIndex >= 0) {
          const auto &record =
              site.column->flashPlacements()[site.placementIndex];
          sheet  = site.sheet;
          column = site.columnIndex;
          first  = record.start;
          count  = record.duration;
          linked = record.sourceId == sourceId &&
                   site.column->isFlashPlacementLinked(site.placementIndex,
                                                       record.duration);
        }
      }
    } else if (binding.contains("nativeInstanceId") &&
               !instances.budgetExceeded) {
      const auto id = binding.value("nativeInstanceId").toString();
      if (!instances.ambiguous.contains(id)) {
        const auto site = instances.sites.value(id);
        if (site.column && site.instanceIndex >= 0) {
          const auto &record =
              site.column->flashInstances()[site.instanceIndex];
          sheet  = site.sheet;
          column = site.columnIndex;
          first  = record.timing.start;
          count  = record.timing.duration;
          linked = record.sourceId == sourceId &&
                   site.column->isFlashInstanceLinked(site.instanceIndex);
        }
      }
    }
    const QString expectedSamples =
        binding.value("nativeStageSamplesSha256").toString();
    const QString expectedKeys =
        binding.value("nativeStageKeysSha256").toString();
    if (!linked || expectedSamples.size() != 64 || expectedKeys.size() != 64) {
      ++result.transformUnverified;
      continue;
    }
    const auto actual = stageTransformBaseline(sheet, column, first, count);
    if (actual.sampledPlacementSha256.isEmpty() ||
        actual.stageKeysSha256.isEmpty()) {
      ++result.transformUnverified;
    } else if (actual.sampledPlacementSha256 != expectedSamples) {
      ++result.transformChanged;
    } else if (actual.stageKeysSha256 != expectedKeys) {
      ++result.transformUnverified;
    } else {
      ++result.transformMatches;
    }
  }
  quint64 remainingContentBytes = 512ull * 1024 * 1024;
  QHash<const TXshSimpleLevel *, QHash<int, QString>> contentCache;
  for (int bindingIndex = 0; bindingIndex < bindings.size(); ++bindingIndex) {
    const auto binding = bindings.at(bindingIndex).toObject();
    const auto kind    = binding.value("kind").toString();
    if (kind != "shape" && kind != "oval" && kind != "rectangle" &&
        kind != "text" && kind != "bitmap" && kind != "library-bitmap")
      continue;
    const auto sourceId = binding.value("sourceId").toString();
    const auto expected = binding.value("nativeContentSha256").toString();
    int frame           = kind == "library-bitmap" ? 1 : 0;
    if (expected.size() != 64 ||
        (kind != "library-bitmap" &&
         !integer(binding, "nativeDrawingFrame", frame))) {
      ++result.contentUnverified;
      contentDiagnostic(result, bindingIndex, sourceId,
                        "drawing-content-baseline-missing", maxDiagnostics);
      continue;
    }
    auto *level   = bindingLevel(binding, levels, nativeLevels);
    auto *drawing = level ? level->getSimpleLevel() : nullptr;
    if (!drawing || frame < 1 || !drawing->isFid(TFrameId(frame))) {
      ++result.contentUnverified;
      contentDiagnostic(result, bindingIndex, sourceId,
                        "drawing-content-frame-missing", maxDiagnostics);
      continue;
    }
    QString actual;
    if (contentCache.value(drawing).contains(frame)) {
      actual = contentCache.value(drawing).value(frame);
    } else {
      const auto image = drawing->getFrame(TFrameId(frame), false);
      // Each image may use only what remains of the shared budget; one that
      // does not fit is reported unverified, never partially hashed.
      quint64 bytesHashed = 0;
      actual = drawingContentDigest(image, remainingContentBytes, &bytesHashed);
      if (actual.isEmpty()) {
        ++result.contentUnverified;
        contentDiagnostic(result, bindingIndex, sourceId,
                          "drawing-content-budget-or-format-unverified",
                          maxDiagnostics);
        continue;
      }
      // Shared bitmap and drawing levels are hashed once in one inspection.
      contentCache[drawing].insert(frame, actual);
      remainingContentBytes -= bytesHashed;
    }
    if (actual == expected) {
      ++result.contentMatches;
    } else {
      ++result.contentChanged;
      contentDiagnostic(result, bindingIndex, sourceId,
                        "native-drawing-content-changed", maxDiagnostics);
    }
  }
  for (int bindingIndex = 0; bindingIndex < bindings.size(); ++bindingIndex) {
    const auto binding = bindings.at(bindingIndex).toObject();
    if (!binding.contains("nativeStyles")) continue;
    const auto styles = binding.value("nativeStyles").toArray();
    for (int styleIndex = 0; styleIndex < styles.size(); ++styleIndex) {
      ++result.styleBindings;
      const auto style         = styles.at(styleIndex).toObject();
      const auto sourceStyleId = style.value("sourceStyleId").toString();
      int nativeIndex          = -1;
      auto *level              = bindingLevel(binding, levels, nativeLevels);
      auto *drawing            = level && level->getType() == PLI_XSHLEVEL
                                     ? level->getSimpleLevel()
                                     : nullptr;
      auto *palette            = drawing ? drawing->getPalette() : nullptr;
      const bool validSlot =
          palette && integer(style, "nativePaletteStyleIndex", nativeIndex) &&
          nativeIndex >= 1 && nativeIndex < palette->getStyleCount();
      if (!validSlot) {
        ++result.styleSlotsMissing;
        styleDiagnostic(result, bindingIndex, styleIndex, sourceStyleId,
                        "palette-style-slot-missing", maxDiagnostics);
      } else {
        ++result.styleSlotsPresent;
      }
      if (validSlot && !palette->getStylePage(nativeIndex)) {
        ++result.styleSlotsUnpaged;
        styleDiagnostic(result, bindingIndex, styleIndex, sourceStyleId,
                        "palette-style-slot-unpaged-or-reusable",
                        maxDiagnostics);
      }
      // Imported palette styles are identified by their palette style index.
      if (!style.contains("nativePaletteStyleIndex")) {
        ++result.styleIdentityUnverified;
        continue;
      }
      if (!palette) {
        ++result.styleIdentityStale;
        styleDiagnostic(result, bindingIndex, styleIndex, sourceStyleId,
                        "palette-style-level-missing", maxDiagnostics);
      } else if (validSlot && palette->getStylePage(nativeIndex)) {
        ++result.styleIdentityMatches;
      } else {
        ++result.styleIdentityStale;
        styleDiagnostic(result, bindingIndex, styleIndex, sourceStyleId,
                        "native-palette-style-index-missing", maxDiagnostics);
      }
    }
  }
  QHash<QString, TXsheet *> timelines;
  QSet<QString> ambiguousTimelines;
  bool hasDocumentScenes = false;
  for (const auto &value : bindings)
    if (value.toObject().value("kind") == "document-scene")
      hasDocumentScenes = true;
  if (!hasDocumentScenes)
    timelines.insert("DOMDocument.xml/timeline0", scene.getTopXsheet());

  // Resolve child sheets only through their reported source and native level.
  // A repeated timeline identifier is ambiguous, even when both entries point
  // to the same level, and must not silently choose one.
  for (const auto &value : bindings) {
    const auto binding = value.toObject();
    const QString kind = binding.value("kind").toString();
    const QString id   = binding.value("sourceId").toString();
    QString timelineId;
    if (kind == "library-symbol") timelineId = id + "/timeline0";
    if (kind == "group" || kind == "document-scene") timelineId = id;
    if (timelineId.isEmpty()) continue;
    auto *level = bindingLevel(binding, levels, nativeLevels);
    auto *child = level ? level->getChildLevel() : nullptr;
    if (!child || timelines.contains(timelineId)) {
      ambiguousTimelines.insert(timelineId);
      timelines.remove(timelineId);
      continue;
    }
    if (!ambiguousTimelines.contains(timelineId))
      timelines.insert(timelineId, child->getXsheet());
  }

  using Site = std::tuple<TXsheet *, TXshLevel *, int, int, int>;
  std::set<Site> reportedSites;
  for (const auto &value : bindings) {
    const auto binding = value.toObject();
    auto *level        = bindingLevel(binding, levels, nativeLevels);
    auto *sheet =
        binding.value("kind") == "document-scene"
            ? scene.getTopXsheet()
            : timelines.value(binding.value("sourceTimelineId").toString(),
                              nullptr);
    int column = 0, first = 0, count = 0;
    if (level && sheet && integer(binding, "nativeColumn", column) &&
        integer(binding, "nativeFirstRow", first) &&
        integer(binding, "nativeRowCount", count) && count > 0)
      reportedSites.emplace(sheet, level, column, first, count);
  }

  quint64 checkedCells = 0;
  for (int index = 0; index < bindings.size(); ++index) {
    const auto binding = bindings.at(index).toObject();
    const QString id   = binding.value("sourceId").toString();
    const QString kind = binding.value("kind").toString();
    if (binding.contains("nativePlacementId")) {
      const QString nativeId = binding.value("nativePlacementId").toString();
      if (nativeId.isEmpty() || placements.budgetExceeded) {
        stale(result, index, id,
              placements.budgetExceeded ? "verification-budget-exceeded"
                                        : "invalid-native-placement-id",
              maxDiagnostics);
        continue;
      }
      if (placements.ambiguous.contains(nativeId)) {
        stale(result, index, id, "ambiguous-native-placement-id",
              maxDiagnostics);
        continue;
      }
      const auto site = placements.sites.value(nativeId);
      if (!site.column || site.placementIndex < 0) {
        stale(result, index, id, "native-placement-missing", maxDiagnostics);
        continue;
      }
      const auto &record = site.column->flashPlacements()[site.placementIndex];
      if (record.sourceId != id || record.kind != kind ||
          record.level.getPointer() !=
              bindingLevel(binding, levels, nativeLevels)) {
        stale(result, index, id, "native-placement-source-changed",
              maxDiagnostics);
        continue;
      }
      if (record.detached) {
        stale(result, index, id, "source-placement-detached", maxDiagnostics);
        continue;
      }
      if (quint64(record.duration) >
          maxCellsToCheck - std::min(checkedCells, maxCellsToCheck)) {
        stale(result, index, id, "verification-budget-exceeded",
              maxDiagnostics);
        continue;
      }
      checkedCells += record.duration;
      if (!site.column->isFlashPlacementLinked(site.placementIndex,
                                               record.duration)) {
        stale(result, index, id, "native-placement-cells-changed",
              maxDiagnostics);
        continue;
      }
      if (binding.contains("nativeDrawingFrame")) {
        int frame     = 0;
        auto *drawing = record.level->getSimpleLevel();
        if (!integer(binding, "nativeDrawingFrame", frame) || frame < 1 ||
            !drawing || !drawing->isFid(TFrameId(frame))) {
          stale(result, index, id, "drawing-frame-missing", maxDiagnostics);
          continue;
        }
      }
      ++result.identityMatches;
      int originalColumn = 0, originalRow = 0, originalCount = 0;
      const bool originalSite =
          integer(binding, "nativeColumn", originalColumn) &&
          integer(binding, "nativeFirstRow", originalRow) &&
          integer(binding, "nativeRowCount", originalCount) &&
          site.sheet == (kind == "document-scene"
                             ? scene.getTopXsheet()
                             : timelines.value(
                                   binding.value("sourceTimelineId").toString(),
                                   nullptr)) &&
          site.columnIndex == originalColumn && record.start == originalRow &&
          record.duration == originalCount &&
          QString::fromStdWString(record.level->getName()) ==
              binding.value("nativeLevelName").toString();
      if (originalSite)
        ++result.layoutMatches;
      else
        ++result.relocated;
      continue;
    }
    if (binding.contains("nativeInstanceId")) {
      const QString nativeId = binding.value("nativeInstanceId").toString();
      if (kind != "symbol" || nativeId.isEmpty() || instances.budgetExceeded) {
        stale(result, index, id,
              instances.budgetExceeded ? "verification-budget-exceeded"
                                       : "invalid-native-instance-id",
              maxDiagnostics);
        continue;
      }
      if (instances.ambiguous.contains(nativeId)) {
        stale(result, index, id, "ambiguous-native-instance-id",
              maxDiagnostics);
        continue;
      }
      const auto site = instances.sites.value(nativeId);
      if (!site.column || site.instanceIndex < 0) {
        stale(result, index, id, "native-instance-missing", maxDiagnostics);
        continue;
      }
      const auto &record = site.column->flashInstances()[site.instanceIndex];
      if (record.sourceId != id || !record.level ||
          !record.level->getChildLevel()) {
        stale(result, index, id, "native-instance-source-changed",
              maxDiagnostics);
        continue;
      }
      if (binding.contains("nativeLevelId") &&
          record.level.getPointer() !=
              bindingLevel(binding, levels, nativeLevels)) {
        stale(result, index, id, "native-level-missing-or-duplicate",
              maxDiagnostics);
        continue;
      }
      if (quint64(record.timing.duration) >
          maxCellsToCheck - std::min(checkedCells, maxCellsToCheck)) {
        stale(result, index, id, "verification-budget-exceeded",
              maxDiagnostics);
        continue;
      }
      checkedCells += record.timing.duration;
      if (!site.column->isFlashInstanceLinked(site.instanceIndex)) {
        stale(result, index, id, "native-instance-cells-changed",
              maxDiagnostics);
        continue;
      }
      ++result.identityMatches;
      int originalColumn = 0, originalRow = 0, originalCount = 0;
      const bool originalSite =
          integer(binding, "nativeColumn", originalColumn) &&
          integer(binding, "nativeFirstRow", originalRow) &&
          integer(binding, "nativeRowCount", originalCount) &&
          site.sheet ==
              timelines.value(binding.value("sourceTimelineId").toString(),
                              nullptr) &&
          site.columnIndex == originalColumn &&
          record.timing.start == originalRow &&
          record.timing.duration == originalCount &&
          QString::fromStdWString(record.level->getName()) ==
              binding.value("nativeLevelName").toString();
      if (originalSite)
        ++result.layoutMatches;
      else
        ++result.relocated;
      continue;
    }
    auto *level = bindingLevel(binding, levels, nativeLevels);
    if (id.isEmpty() || !level || !kindMatches(kind, level)) {
      stale(result, index, id,
            binding.contains("nativeLevelId")
                ? "native-level-missing-or-duplicate"
                : "level-missing-or-replaced",
            maxDiagnostics);
      continue;
    }
    if (binding.contains("nativeDrawingFrame")) {
      int frame     = 0;
      auto *drawing = level->getSimpleLevel();
      if (!integer(binding, "nativeDrawingFrame", frame) || frame < 1 ||
          !drawing || !drawing->isFid(TFrameId(frame))) {
        stale(result, index, id, "drawing-frame-missing", maxDiagnostics);
        continue;
      }
    }
    if (kind == "library-symbol" &&
        ambiguousTimelines.contains(id + "/timeline0")) {
      stale(result, index, id, "ambiguous-native-timeline", maxDiagnostics);
      continue;
    }
    if (kind == "group" || kind == "document-scene") {
      if (ambiguousTimelines.contains(id)) {
        stale(result, index, id, "ambiguous-native-timeline", maxDiagnostics);
        continue;
      }
    }
    if (binding.contains("nativeColumn")) {
      TXsheet *sheet =
          kind == "document-scene"
              ? scene.getTopXsheet()
              : timelines.value(binding.value("sourceTimelineId").toString(),
                                nullptr);
      int column = 0, first = 0, count = 0;
      if (!sheet || !integer(binding, "nativeColumn", column) ||
          !integer(binding, "nativeFirstRow", first) ||
          !integer(binding, "nativeRowCount", count) || count < 1 ||
          column >= sheet->getColumnCount() ||
          count > std::numeric_limits<int>::max() - first) {
        stale(result, index, id, "timeline-or-span-missing", maxDiagnostics);
        continue;
      }
      if (quint64(count) >
          maxCellsToCheck - std::min(checkedCells, maxCellsToCheck)) {
        stale(result, index, id, "verification-budget-exceeded",
              maxDiagnostics);
        continue;
      }
      bool matches = true;
      for (int row = first; row < first + count; ++row) {
        ++checkedCells;
        if (sheet->getCell(row, column).m_level.getPointer() != level) {
          matches = false;
          break;
        }
      }
      if (!matches) {
        stale(result, index, id, "native-cells-changed", maxDiagnostics);
        continue;
      }
      bool ambiguous = false, exhausted = false;
      for (int other = 0; other < sheet->getColumnCount(); ++other) {
        if (other == column) continue;
        if (checkedCells >= maxCellsToCheck) {
          exhausted = true;
          break;
        }
        ++checkedCells;
        if (sheet->getCell(first, other).m_level.getPointer() != level)
          continue;
        bool duplicate = true;
        for (int row = first + 1; row < first + count; ++row) {
          if (checkedCells >= maxCellsToCheck) {
            exhausted = true;
            break;
          }
          ++checkedCells;
          if (sheet->getCell(row, other).m_level.getPointer() != level) {
            duplicate = false;
            break;
          }
        }
        if (exhausted) break;
        if (duplicate &&
            !reportedSites.count(Site(sheet, level, other, first, count))) {
          ambiguous = true;
          break;
        }
      }
      if (ambiguous || exhausted) {
        stale(result, index, id,
              ambiguous ? "unreported-duplicate-span"
                        : "verification-budget-exceeded",
              maxDiagnostics);
        continue;
      }
    }
    if (binding.contains("nativeLevelId")) {
      ++result.identityMatches;
      if (QString::fromStdWString(level->getName()) ==
          binding.value("nativeLevelName").toString())
        ++result.layoutMatches;
      else
        ++result.relocated;
    } else {
      ++result.layoutMatches;
    }
  }
  return result;
}

}  // namespace FlashNative
