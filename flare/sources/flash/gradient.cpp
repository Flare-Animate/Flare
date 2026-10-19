#include "gradient.h"

#include <QSet>
#include <algorithm>
#include <cmath>

namespace Flash {
namespace {

QString location(const SourceNode &source, const QString &fallback) {
  return source.sourceId.isEmpty() ? fallback : source.sourceId;
}

QString fieldLocation(const SourceNode &source, const QString &fallback,
                      const QString &field) {
  return location(source, fallback) + "/@" + field;
}

bool xflNode(const ExtraNode &node, const QString &localName) {
  return (node.namespaceUri.isEmpty() ||
          node.namespaceUri == "http://ns.adobe.com/xfl/2008/") &&
         node.qualifiedName.section(':', -1) == localName;
}

bool decimal(const QString &lexical, double &result) {
  bool ok = false;
  result  = lexical.toDouble(&ok);
  return ok && std::isfinite(result);
}

}  // namespace

std::optional<GradientSpec> validateGradient(const Paint &paint,
                                             const GradientIssueSink &sink) {
  bool valid = true;
  auto issue = [&](const QString &code, const QString &sourceId,
                   const QString &message) {
    valid = false;
    if (sink) sink({code, sourceId, message});
  };
  GradientSpec spec;
  spec.kind = paint.kind;
  if ((paint.kind != Paint::Kind::LinearGradient ||
       paint.sourceTag != "LinearGradient") &&
      (paint.kind != Paint::Kind::RadialGradient ||
       paint.sourceTag != "RadialGradient")) {
    issue("invalid-gradient-model", paint.sourceId,
          "Paint kind and source gradient tag disagree.");
    return std::nullopt;
  }

  for (auto it = paint.properties.cbegin(); it != paint.properties.cend(); ++it)
    if (it.key() != "spreadMethod" && it.key() != "interpolationMethod" &&
        it.key() != "focalPointRatio")
      issue("unmapped-gradient-attribute",
            fieldLocation(paint.source, paint.sourceId, it.key()),
            "Gradient attribute has no verified semantic mapping: " + it.key());
  for (const auto &attribute : paint.source.qualifiedAttributes)
    issue("unmapped-gradient-attribute", attribute.sourceId,
          "Qualified gradient attribute has no verified semantic mapping: " +
              attribute.qualifiedName);
  for (const auto &extra : paint.extras)
    issue("unmapped-gradient-child", extra.sourceId,
          "Gradient child has no verified semantic mapping: " + extra.tag);

  const QString spread = paint.properties.value("spreadMethod", "pad");
  if (spread == "pad")
    spec.spread = GradientSpec::Spread::Pad;
  else if (spread == "reflect")
    spec.spread = GradientSpec::Spread::Reflect;
  else if (spread == "repeat")
    spec.spread = GradientSpec::Spread::Repeat;
  else
    issue("unmapped-gradient-mode",
          fieldLocation(paint.source, paint.sourceId, "spreadMethod"),
          "Gradient spread mode has no verified mapping: " + spread);

  const QString interpolation =
      paint.properties.value("interpolationMethod", "rgb");
  if (interpolation == "rgb")
    spec.interpolation = GradientSpec::Interpolation::Rgb;
  else if (interpolation == "linearRGB")
    spec.interpolation = GradientSpec::Interpolation::LinearRgb;
  else
    issue("unmapped-gradient-mode",
          fieldLocation(paint.source, paint.sourceId, "interpolationMethod"),
          "Gradient interpolation mode has no verified mapping: " +
              interpolation);

  if (paint.properties.contains("focalPointRatio")) {
    double focal = 0;
    if (!decimal(paint.properties.value("focalPointRatio"), focal) ||
        focal < -1 || focal > 1)
      issue("invalid-gradient-focal",
            fieldLocation(paint.source, paint.sourceId, "focalPointRatio"),
            "Gradient focal ratio must be finite and in [-1, 1].");
    else if (paint.kind == Paint::Kind::LinearGradient)
      issue("unmapped-gradient-mode",
            fieldLocation(paint.source, paint.sourceId, "focalPointRatio"),
            "A focal ratio on a linear gradient has no verified mapping.");
    else
      spec.focalPointRatio = focal;
  }

  if (!paint.style.sourceId.isEmpty()) {
    const bool fillWrapper =
        paint.style.source.qualifiedName.section(':', -1) == "FillStyle";
    for (auto it = paint.style.properties.cbegin();
         it != paint.style.properties.cend(); ++it)
      if (!(fillWrapper && it.key() == "index"))
        issue("unmapped-gradient-style-attribute",
              fieldLocation(paint.style.source, paint.style.sourceId, it.key()),
              "Gradient style wrapper attribute has no verified mapping: " +
                  it.key());
    for (const auto &attribute : paint.style.source.qualifiedAttributes)
      issue("unmapped-gradient-style-attribute", attribute.sourceId,
            "Qualified gradient style wrapper attribute has no verified "
            "mapping: " +
                attribute.qualifiedName);
  }

  if (!paint.matrixXml || paint.matrixSource.sourceId.isEmpty()) {
    issue("unmapped-gradient-matrix", paint.sourceId + "/matrix",
          "Gradient matrix absence has no verified native mapping.");
  } else {
    const auto &nodes = paint.matrixXml->nodes;
    if (nodes.size() != 2 || !xflNode(nodes[0], "matrix") ||
        !xflNode(nodes[1], "Matrix") ||
        nodes[1].sourceId != paint.matrixSource.source.sourceId)
      issue("unmapped-gradient-matrix", paint.sourceId + "/matrix",
            "Gradient matrix must have exactly one recognized Matrix child.");
    if (!nodes.isEmpty())
      for (const auto &attribute : nodes[0].attributes)
        issue("unmapped-gradient-matrix-attribute", attribute.sourceId,
              "Gradient matrix container attribute has no verified mapping: " +
                  attribute.qualifiedName);
    const QSet<QString> matrixFields = {"a", "b", "c", "d", "tx", "ty"};
    for (auto it = paint.matrixSource.properties.cbegin();
         it != paint.matrixSource.properties.cend(); ++it)
      if (!matrixFields.contains(it.key()))
        issue("unmapped-gradient-matrix-attribute",
              fieldLocation(paint.matrixSource.source,
                            paint.matrixSource.sourceId, it.key()),
              "Gradient Matrix attribute has no verified mapping: " + it.key());
    for (const auto &attribute : paint.matrixSource.source.qualifiedAttributes)
      issue("unmapped-gradient-matrix-attribute", attribute.sourceId,
            "Qualified gradient Matrix attribute has no verified mapping: " +
                attribute.qualifiedName);
  }
  spec.matrix = paint.matrix;
  const double scale =
      std::max({std::abs(spec.matrix.a), std::abs(spec.matrix.b),
                std::abs(spec.matrix.c), std::abs(spec.matrix.d)});
  const double determinant = spec.matrix.determinant();
  if (!std::isfinite(scale) || !std::isfinite(determinant) ||
      !std::isfinite(spec.matrix.tx) || !std::isfinite(spec.matrix.ty))
    issue("invalid-gradient-matrix", paint.matrixSource.sourceId,
          "Gradient matrix contains a nonfinite coefficient.");
  else if (determinant == 0)
    spec.singular = true;  // e.g. <Matrix a="0" d="0"/>; see GradientSpec
  else if (std::abs(determinant) <= 1e-12 * scale * scale)
    issue(
        "unmapped-gradient-matrix", paint.matrixSource.sourceId,
        "Singular or ill-conditioned gradient matrix needs verified behavior.");

  if (paint.stops.size() < 2 || paint.stops.size() > 1024)
    issue("unmapped-gradient-stop-count", paint.sourceId,
          "Gradient stop count is outside the bounded native specification.");
  double lastRatio = -1;
  for (const auto &stop : paint.stops) {
    for (auto it = stop.properties.cbegin(); it != stop.properties.cend(); ++it)
      if (it.key() != "ratio" && it.key() != "color" && it.key() != "alpha")
        issue("unmapped-gradient-stop-attribute",
              fieldLocation(stop.source, stop.sourceId, it.key()),
              "Gradient stop attribute has no verified mapping: " + it.key());
    for (const auto &attribute : stop.source.qualifiedAttributes)
      issue("unmapped-gradient-stop-attribute", attribute.sourceId,
            "Qualified gradient stop attribute has no verified mapping: " +
                attribute.qualifiedName);
    for (const auto &extra : stop.extras)
      issue("unmapped-gradient-stop-child", extra.sourceId,
            "Gradient stop child has no verified mapping: " + extra.tag);
    double ratio = 0;
    if (!decimal(stop.properties.value("ratio", "0"), ratio) || ratio < 0 ||
        ratio > 1 || ratio != stop.ratio || ratio < lastRatio)
      issue("invalid-gradient-stop",
            fieldLocation(stop.source, stop.sourceId, "ratio"),
            "Gradient stop ratio is invalid, out of order or differs from "
            "model.");
    else
      lastRatio = ratio;
    double alpha = 1;
    if (!decimal(stop.properties.value("alpha", "1"), alpha) || alpha < 0 ||
        alpha > 1 || std::lround(alpha * 255) != stop.color.alpha)
      issue("invalid-gradient-stop",
            fieldLocation(stop.source, stop.sourceId, "alpha"),
            "Gradient stop alpha is invalid or differs from model.");
    Color parsedColor;
    if (!parseHexRgb(stop.properties.value("color", "#000000"), parsedColor) ||
        parsedColor.red != stop.color.red ||
        parsedColor.green != stop.color.green ||
        parsedColor.blue != stop.color.blue)
      issue("invalid-gradient-stop",
            fieldLocation(stop.source, stop.sourceId, "color"),
            "Gradient stop RGB is invalid or differs from model.");
    spec.stops.push_back({stop.ratio, stop.color, alpha});
  }
  if (!valid) return std::nullopt;
  return spec;
}

}  // namespace Flash
