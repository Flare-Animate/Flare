#pragma once

#include "archive.h"
#include <QJsonArray>
#include <QJsonObject>
#include <QSet>
#include <QVector>

namespace Flash {

struct SpanInfo {
  int index    = 0;
  int duration = 1;
  QString tween;
};

struct LayerInfo {
  QString name;
  QString type;
  int parent   = -1;
  bool visible = true;
  bool locked  = false;
  QVector<SpanInfo> spans;
};

struct TimelineInfo {
  QString name;
  int frameCount = 0;
  QVector<LayerInfo> layers;
};

struct SymbolInfo {
  QString name;
  QString member;
  QString type;
  QVector<TimelineInfo> timelines;
  QSet<QString> references;
};

struct DocumentInfo {
  QString creator;
  QString version;
  QString platform;
  int width        = 0;
  int height       = 0;
  double frameRate = 0;
  QVector<TimelineInfo> timelines;
  QMap<QString, SymbolInfo> symbols;
  QSet<QString> rootReferences;
  QSet<QString> fonts;
  QMap<QString, quint64> elements;
  QMap<QString, quint64> tweenTypes;
  QMap<QString, quint64> playbackModes;
  QMap<QString, quint64> layerTypes;
  QJsonObject assets;
  QJsonArray issues;
  quint64 xmlBytes              = 0;
  quint64 geometrySegments      = 0;
  bool geometryVerified         = false;
  bool modelVerified            = false;
  quint64 modelDefinitions      = 0;
  quint64 modelShapes           = 0;
  quint64 modelElements         = 0;
  quint64 modelGeometrySegments = 0;
  int xmlMembers                = 0;
  int maxSymbolDepth            = 0;
  int reachableSymbols          = 0;
  bool directoryCompatibility   = false;
  QJsonObject toJson() const;
};

enum class LayerParentProblem { None, Invalid, Cyclic };
// Checks each layer's parent index (-1 for none) in layer order and reports
// the first out-of-range/self parent or ancestor cycle.
LayerParentProblem layerParentProblem(const QVector<int> &parents);
// Layer indices of authored parentLayerIndex values. Animate (2017+) does
// not count a timeline's camera layer (layerType="camera", saved first) in
// those indices: Animate 20 saves a folder nested in a folder below the
// camera layer as parentLayerIndex 1, the second layer after the camera.
// Unchanged without a camera layer; out-of-range values stay out of range.
QVector<int> layerParentIndices(const QVector<int> &authored,
                                const QVector<QString> &types);

// Structural inspection does not imply rendering or editing compatibility.
// Library XML is streamed and released, keeping the inspection bounded.
DocumentInfo inspect(Package &package, bool verifyGeometry = false,
                     bool verifyModel = false);

}  // namespace Flash
