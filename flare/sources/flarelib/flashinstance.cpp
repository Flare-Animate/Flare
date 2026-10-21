#include "flare/flashinstance.h"
#include "flare/txshleveltypes.h"

#include <QUuid>
#include <cmath>
#include <limits>

namespace {
QString text(const QJsonObject &json, const QString &name) {
  const auto value = json.value(name);
  if (!value.isString() || value.toString().isEmpty() ||
      value.toString().size() > 65536)
    throw Flash::Error("Invalid Flash instance property: " + name);
  return value.toString();
}

double number(const QJsonObject &json, const QString &name) {
  const auto value = json.value(name);
  if (!value.isDouble() || !std::isfinite(value.toDouble()))
    throw Flash::Error("Invalid Flash instance property: " + name);
  return value.toDouble();
}

bool validNativeInstanceId(const QString &id) {
  if (id.size() != 36) return false;
  const QUuid uuid(id);
  return !uuid.isNull() && uuid.toString(QUuid::WithoutBraces) == id;
}
}  // namespace

QJsonObject FlashInstanceData::toJson() const {
  if (!level || !level->getChildLevel() || sourceId.isEmpty() ||
      sourceId.size() > 65536 || libraryName.isEmpty() ||
      libraryName.size() > 65536 || symbolFrameCount <= 0 ||
      symbolFrameCount > 10000000)
    throw Flash::Error("Invalid Flash instance identity or source timeline.");
  timing.sourceFrameAt(timing.start, symbolFrameCount);
  if (!nativeInstanceId.isEmpty() && !validNativeInstanceId(nativeInstanceId))
    throw Flash::Error("Invalid native Flash instance identifier.");
  QJsonObject result{{"schemaVersion", 1},
                     {"sourceId", sourceId},
                     {"libraryName", libraryName},
                     {"timing", timing.toJson()},
                     {"symbolFrameCount", symbolFrameCount}};
  if (!nativeInstanceId.isEmpty())
    result["nativeInstanceId"] = nativeInstanceId;
  if (hasSourcePivot) {
    if (!std::isfinite(sourcePivot.x) || !std::isfinite(sourcePivot.y))
      throw Flash::Error("Invalid Flash instance source pivot.");
    result["sourcePivot"] =
        QJsonObject{{"x", sourcePivot.x}, {"y", sourcePivot.y}};
  }
  return result;
}

FlashInstanceData FlashInstanceData::fromJson(const QJsonObject &json,
                                              TXshLevel *level) {
  if (number(json, "schemaVersion") != 1)
    throw Flash::Error("Unsupported Flash instance schema version.");
  const double count = number(json, "symbolFrameCount");
  if (count < 1 || count > 10000000 || count != std::floor(count))
    throw Flash::Error("Invalid Flash instance timeline length.");
  if (!json.value("timing").isObject())
    throw Flash::Error("Missing Flash instance timing.");
  FlashInstanceData result;
  result.sourceId = text(json, "sourceId");
  if (json.contains("nativeInstanceId"))
    result.nativeInstanceId = text(json, "nativeInstanceId");
  result.libraryName = text(json, "libraryName");
  result.level       = level;
  result.timing =
      Flash::GraphicTiming::fromJson(json.value("timing").toObject());
  result.symbolFrameCount = int(count);
  if (json.contains("sourcePivot")) {
    if (!json.value("sourcePivot").isObject())
      throw Flash::Error("Invalid Flash instance pivot record.");
    const auto pivot      = json.value("sourcePivot").toObject();
    result.sourcePivot    = {number(pivot, "x"), number(pivot, "y")};
    result.hasSourcePivot = true;
  }
  result.toJson();
  return result;
}

QJsonObject FlashPlacementData::toJson() const {
  if (!level || sourceId.isEmpty() || sourceId.size() > 65536 ||
      (kind != "shape" && kind != "oval" && kind != "rectangle" &&
       kind != "text" && kind != "group" && kind != "bitmap" &&
       kind != "video" && kind != "symbol" && kind != "document-scene") ||
      !validNativeInstanceId(nativePlacementId) || start < 0 || duration < 1 ||
      duration > 20000000 ||
      qint64(start) + duration > std::numeric_limits<int>::max() ||
      framesSha256.size() != 64)
    throw Flash::Error("Invalid native Flash placement record.");
  if (((kind == "group" || kind == "symbol" || kind == "document-scene") &&
       !level->getChildLevel()) ||
      ((kind == "shape" || kind == "oval" || kind == "rectangle" ||
        kind == "text") &&
       level->getType() != PLI_XSHLEVEL) ||
      ((kind == "bitmap" || kind == "video") &&
       level->getType() != OVL_XSHLEVEL))
    throw Flash::Error("Native Flash placement level type does not match.");
  for (const QChar digit : framesSha256)
    if ((digit < QChar('0') || digit > QChar('9')) &&
        (digit < QChar('a') || digit > QChar('f')))
      throw Flash::Error("Invalid native Flash placement frame digest.");
  QJsonObject result{{"schemaVersion", detached ? 2 : 1},
                     {"sourceId", sourceId},
                     {"kind", kind},
                     {"nativePlacementId", nativePlacementId},
                     {"start", start},
                     {"duration", duration},
                     {"framesSha256", framesSha256}};
  if (detached) result["detached"] = true;
  return result;
}

FlashPlacementData FlashPlacementData::fromJson(const QJsonObject &json,
                                                TXshLevel *level) {
  const double schemaVersion = number(json, "schemaVersion");
  if (schemaVersion != 1 && schemaVersion != 2)
    throw Flash::Error("Unsupported native Flash placement schema version.");
  FlashPlacementData result;
  result.sourceId          = text(json, "sourceId");
  result.kind              = text(json, "kind");
  result.nativePlacementId = text(json, "nativePlacementId");
  const double start       = number(json, "start");
  const double duration    = number(json, "duration");
  if (start < 0 || start > std::numeric_limits<int>::max() ||
      start != std::floor(start) || duration < 1 || duration > 20000000 ||
      duration != std::floor(duration))
    throw Flash::Error("Invalid native Flash placement span.");
  result.start        = int(start);
  result.duration     = int(duration);
  result.framesSha256 = text(json, "framesSha256");
  if ((schemaVersion == 1 && json.contains("detached")) ||
      (schemaVersion == 2 &&
       (!json.value("detached").isBool() || !json.value("detached").toBool())))
    throw Flash::Error("Invalid native Flash placement detached state.");
  result.detached = schemaVersion == 2;
  result.level    = level;
  result.toJson();
  return result;
}
