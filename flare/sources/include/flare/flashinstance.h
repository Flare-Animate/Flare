#pragma once

#include "flare/txshlevel.h"
#include "../../flash/geometry.h"
#include "../../flash/timing.h"

#undef DVAPI
#ifdef FLARELIB_EXPORTS
#define DVAPI DV_EXPORT_API
#else
#define DVAPI DV_IMPORT_API
#endif

// Source rules retained alongside ordinary native exposures. Native cells
// remain authoritative; TXshLevelColumn verifies that they still match before
// presenting these rules as linked editing controls.
struct DVAPI FlashInstanceData {
  QString sourceId;
  // Identifies this native placement, independently of its Flash source and
  // current column/row. A copied placement receives a new identifier.
  QString nativeInstanceId;
  QString libraryName;
  TXshLevelP level;
  Flash::GraphicTiming timing;
  int symbolFrameCount = 0;
  Flash::Point sourcePivot;
  bool hasSourcePivot = false;

  QJsonObject toJson() const;
  static FlashInstanceData fromJson(const QJsonObject &json, TXshLevel *level);
};

// A source occurrence compiled to ordinary native cells without Graphic
// playback controls. The digest records only its numeric frame exposure
// sequence; artwork and stage transforms are deliberately outside this check.
struct DVAPI FlashPlacementData {
  QString sourceId;
  QString kind;
  QString nativePlacementId;
  TXshLevelP level;
  int start    = 0;
  int duration = 0;
  QString framesSha256;
  // An intersecting native timeline edit can change or remove the exposure.
  // Keep its source lineage and native ID without claiming that cells still
  // represent the imported occurrence. Schema v2 is used for detached records
  // so older readers that only accept v1 fail closed. Undo restores v1 state.
  bool detached = false;

  QJsonObject toJson() const;
  static FlashPlacementData fromJson(const QJsonObject &json, TXshLevel *level);
};
