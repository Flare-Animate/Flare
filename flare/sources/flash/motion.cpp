#include "motion.h"

#include <QXmlStreamReader>

#include <algorithm>
#include <cmath>

namespace Flash {

// An ease's progress at r in [0, 1]; linear (and *mapped false) for preset
// types without a mapping.
double motionEase(const MotionEase &ease, double r, bool *mapped = nullptr);

// Flash's decomposition of a matrix (fl.motion.MatrixTransformer): column
// lengths and skew angles in degrees; rotation is skewY when both are equal.
struct FlashTransform {
  double scaleX = 1, scaleY = 1, skewX = 0, skewY = 0;
};

namespace {

constexpr double degree = 3.14159265358979323846 / 180;

// "a,b" as two finite numbers.
bool numberPair(const QString &text, double &first, double &second) {
  const int comma = text.indexOf(',');
  if (comma < 0) return false;
  bool okFirst = false, okSecond = false;
  first  = text.left(comma).trimmed().toDouble(&okFirst);
  second = text.mid(comma + 1).trimmed().toDouble(&okSecond);
  return okFirst && okSecond && std::isfinite(first) && std::isfinite(second);
}

bool sameAngle(double a, double b, double tolerance) {
  return std::abs(std::remainder(a - b, 360.0)) <= tolerance;
}

const QStringList &colorIds() {
  static const QStringList ids{
      "Alpha_Amount",    "Brightness_Amount", "Tint_Amount",
      "Tint_Color.r",    "Tint_Color.g",      "Tint_Color.b",
      "Tint_Color.a",    "AdvClr_R_Pct",      "AdvClr_G_Pct",
      "AdvClr_B_Pct",    "AdvClr_A_Pct",      "AdvClr_R_Offset",
      "AdvClr_G_Offset", "AdvClr_B_Offset",   "AdvClr_A_Offset"};
  return ids;
}

int easePower(const QString &type) {
  if (type == "Quadratic") return 2;
  if (type == "Cubic") return 3;
  if (type == "Quartic") return 4;
  if (type == "Quintic") return 5;
  return 0;
}

double cubic(double p0, double p1, double p2, double p3, double u) {
  const double v = 1 - u;
  return v * v * v * p0 + 3 * v * v * u * p1 + 3 * v * u * u * p2 +
         u * u * u * p3;
}

// One Bezier segment in (time, value) at time t, t0 <= t <= t1. A handle
// with no time offset lies on its key.
double segmentValue(const MotionKey &k0, const MotionKey &k1, double t) {
  const double t0 = k0.time, t1 = k1.time;
  if (t1 <= t0) return k1.value;
  const bool flatOut = k0.next.dt == 0, flatIn = k1.previous.dt == 0;
  if (flatOut && flatIn)
    return k0.value + (k1.value - k0.value) * (t - t0) / (t1 - t0);
  // Time handles stay inside the segment, so time is monotonic in u.
  const double x1 = std::min(std::max(t0 + k0.next.dt, t0), t1);
  const double x2 = std::min(std::max(t1 + k1.previous.dt, t0), t1);
  const double y1 = flatOut ? k0.value : k0.next.value;
  const double y2 = flatIn ? k1.value : k1.previous.value;
  double low = 0, high = 1;
  for (int step = 0; step < 80; ++step) {
    const double mid                              = (low + high) / 2;
    (cubic(t0, x1, x2, t1, mid) < t ? low : high) = mid;
  }
  return cubic(k0.value, y1, y2, k1.value, (low + high) / 2);
}

void finishProperty(MotionObject &result, MotionProperty property,
                    const QStringList &containers, bool ignoreTimeMap,
                    int easeIndex, const QVector<MotionEase> &eases) {
  std::stable_sort(
      property.keys.begin(), property.keys.end(),
      [](const MotionKey &a, const MotionKey &b) { return a.time < b.time; });
  if (!ignoreTimeMap && easeIndex >= 0 && easeIndex < eases.size())
    property.ease = eases[easeIndex];
  const QString &id = property.id;
  if (containers.contains("Filters")) {
    const QString detail = id + ": filter curve (filters are not mapped)";
    if (!result.unmapped.contains(detail)) result.unmapped.push_back(detail);
    return;
  }
  if (property.keys.isEmpty()) return;
  static const QStringList planar{"Motion_X", "Motion_Y", "Rotation_Z",
                                  "Skew_X",   "Skew_Y",   "Scale_X",
                                  "Scale_Y"};
  static const QStringList depth{"Motion_Z", "Rotation_X", "Rotation_Y"};
  if (depth.contains(id)) {
    // Animate keeps zero 3D curves on 2D tweens.
    const bool moves = std::any_of(
        property.keys.cbegin(), property.keys.cend(), [](const MotionKey &k) {
          return k.value != 0 || (k.next.dt != 0 && k.next.value != 0) ||
                 (k.previous.dt != 0 && k.previous.value != 0);
        });
    if (moves)
      result.unmapped3D.push_back(
          id + ": 3D motion is not mapped (the tween stays in the plane)");
    return;
  }
  if (!planar.contains(id) && !colorIds().contains(id)) {
    result.unmapped.push_back(id + ": unknown motion property (not applied)");
    return;
  }
  if (colorIds().contains(id)) result.hasColor = true;
  if (property.ease && property.keys.size() > 1) {
    bool mapped = true;
    motionEase(*property.ease, 0.5, &mapped);
    if (!mapped)
      result.unmapped.push_back(
          id + ": " + property.ease->type + " ease (strength " +
          QString::number(property.ease->strength) +
          ") is not mapped; the property is eased linearly");
  }
  result.properties.insert(id, property);
}

}  // namespace

bool motionObjectFrame(const Frame &frame) {
  return frame.properties.value("tweenType") == "motion object";
}

bool motionObjectExtra(const Extra &extra) {
  return extra.tag == "motionObjectXML";
}

std::optional<MotionObject> motionObject(const Frame &frame) {
  const Extra *source = nullptr;
  for (const auto &extra : frame.extras)
    if (motionObjectExtra(extra)) {
      source = &extra;
      break;
    }
  if (!source) return std::nullopt;
  MotionObject result;
  QXmlStreamReader xml(source->xml);
  QVector<MotionEase> eases;
  QStringList containers;
  bool inCore = false, inProperty = false;
  bool ignoreTimeMap = false;
  int easeIndex      = 0;
  MotionProperty property;
  // Tint_Color keys are colors (value="0xRRGGBBAA"): one curve per channel.
  QVector<MotionKey> channels[4];
  while (!xml.atEnd()) {
    xml.readNext();
    const auto attributes = xml.attributes();
    auto text             = [&](const char *name) {
      return attributes.value(QLatin1String(name)).toString();
    };
    auto number = [&](const char *name, double fallback) {
      bool ok            = false;
      const double value = text(name).toDouble(&ok);
      return ok && std::isfinite(value) ? value : fallback;
    };
    if (xml.isStartElement()) {
      const QString name = xml.name().toString();
      if (name == "AnimationCore") {
        inCore           = true;
        result.timeScale = number("TimeScale", 24000);
        if (result.timeScale <= 0) result.timeScale = 24000;
      } else if (!inCore) {
        continue;
      } else if (name == "TimeMap" && containers.isEmpty() && !inProperty) {
        MotionEase ease;
        ease.type     = text("type");
        ease.strength = number("strength", 0);
        eases.push_back(ease);
      } else if (name == "PropertyContainer") {
        containers.push_back(text("id"));
      } else if (name == "Property") {
        inProperty    = true;
        property      = MotionProperty();
        property.id   = text("id");
        ignoreTimeMap = text("ignoreTimeMap") == "1";
        easeIndex     = int(number("TimeMapIndex", 0));
        for (auto &channel : channels) channel.clear();
        if (text("enabled") == "0") property.id.clear();
        // A keyless property is a constant (filter options).
        if (attributes.hasAttribute(QLatin1String("value"))) {
          MotionKey key;
          bool ok   = false;
          key.value = text("value").toDouble(&ok);
          if (ok) property.keys.push_back(key);
        }
      } else if (name == "Keyframe" && inProperty) {
        MotionKey key;
        key.time     = number("timevalue", 0);
        double first = 0, second = 0;
        if (numberPair(text("anchor"), first, second)) {
          key.value = second;
          if (numberPair(text("next"), first, second))
            key.next = {first, second};
          if (numberPair(text("previous"), first, second))
            key.previous = {first, second};
          property.keys.push_back(key);
        } else {
          const QString value = text("value");
          bool ok             = false;
          if (value.startsWith("0x", Qt::CaseInsensitive) &&
              value.size() == 10) {
            const quint32 rgba = value.mid(2).toUInt(&ok, 16);
            if (ok)
              for (int channel = 0; channel < 4; ++channel) {
                MotionKey part = key;
                part.value     = (rgba >> (24 - 8 * channel)) & 0xff;
                channels[channel].push_back(part);
              }
          } else {
            key.value = value.toDouble(&ok);
            if (ok) property.keys.push_back(key);
          }
        }
      }
    } else if (xml.isEndElement()) {
      const QString name = xml.name().toString();
      if (name == "PropertyContainer" && !containers.isEmpty())
        containers.pop_back();
      else if (name == "Property" && inProperty) {
        inProperty = false;
        if (property.id.isEmpty()) continue;
        if (!channels[0].isEmpty()) {
          static const char *suffix[] = {".r", ".g", ".b", ".a"};
          for (int channel = 0; channel < 4; ++channel) {
            MotionProperty part = property;
            part.id += suffix[channel];
            part.keys = channels[channel];
            finishProperty(result, part, containers, ignoreTimeMap, easeIndex,
                           eases);
          }
        } else
          finishProperty(result, property, containers, ignoreTimeMap, easeIndex,
                         eases);
      }
    }
  }
  if (xml.hasError() || !inCore) return std::nullopt;
  return result;
}

double motionTicks(const MotionObject &motion, double frameOffset,
                   double frameRate) {
  return frameOffset * motion.timeScale / frameRate;
}

double motionEase(const MotionEase &ease, double r, bool *mapped) {
  if (mapped) *mapped = true;
  const double blend = std::min(std::abs(ease.strength) / 100, 1.0);
  if (const int power = easePower(ease.type)) {
    const double curve =
        ease.strength > 0 ? 1 - std::pow(1 - r, power) : std::pow(r, power);
    return blend * curve + (1 - blend) * r;
  }
  if (ease.type.startsWith("Dual")) {
    if (const int power = easePower(ease.type.mid(4))) {
      auto p = [&](double x) { return std::pow(std::abs(x), power); };
      // Positive: fast, slow in the middle, fast; negative: ease in and out.
      const double curve =
          ease.strength > 0
              ? (r < 0.5 ? 0.5 - p(2 * r - 1) / 2 : 0.5 + p(2 * r - 1) / 2)
              : (r < 0.5 ? p(2 * r) / 2 : 1 - p(2 * r - 2) / 2);
      return blend * curve + (1 - blend) * r;
    }
  }
  if (mapped) *mapped = ease.type.isEmpty() || ease.type == "Linear";
  return r;
}

double motionValue(const MotionProperty &property, double ticks) {
  const auto &keys = property.keys;
  if (keys.isEmpty()) return 0;
  const double first = keys.front().time, last = keys.back().time;
  if (keys.size() == 1 || ticks <= first) return keys.front().value;
  if (last <= first) return keys.back().value;
  double r = std::min((ticks - first) / (last - first), 1.0);
  if (property.ease) r = motionEase(*property.ease, r);
  if (r <= 0) return keys.front().value;
  if (r >= 1) return keys.back().value;
  const double time = first + (last - first) * r;
  for (int index = 0; index + 1 < keys.size(); ++index)
    if (time <= keys[index + 1].time)
      return segmentValue(keys[index], keys[index + 1], time);
  return keys.back().value;
}

FlashTransform flashTransform(const Affine &matrix) {
  FlashTransform result;
  result.scaleX = std::hypot(matrix.a, matrix.b);
  result.scaleY = std::hypot(matrix.c, matrix.d);
  result.skewY  = std::atan2(matrix.b, matrix.a) / degree;
  result.skewX  = std::atan2(-matrix.c, matrix.d) / degree;
  return result;
}

Affine flashMatrix(const FlashTransform &transform) {
  Affine result;
  result.a = transform.scaleX * std::cos(transform.skewY * degree);
  result.b = transform.scaleX * std::sin(transform.skewY * degree);
  result.c = -transform.scaleY * std::sin(transform.skewX * degree);
  result.d = transform.scaleY * std::cos(transform.skewX * degree);
  return result;
}

bool motionRelative(const MotionObject &motion, const Affine &base,
                    bool *ambiguous) {
  if (ambiguous) *ambiguous = false;
  const FlashTransform own        = flashTransform(base);
  constexpr double angleTolerance = 0.02;  // degrees
  constexpr double scaleTolerance = 2e-4;  // as a factor
  auto at0                        = [&](const QString &id, double fallback) {
    const auto found = motion.properties.constFind(id);
    return found == motion.properties.cend() ? fallback
                                                                    : motionValue(*found, 0);
  };
  auto near = [&](double a, double b) {
    return std::abs(a - b) <= scaleTolerance * std::max(1.0, std::abs(b));
  };
  // Absolute curves restate the element's transform at time 0.
  {
    const double rotation = at0("Rotation_Z", own.skewY);
    const double skewX    = at0("Skew_X", own.skewX - own.skewY);
    const double skewY    = at0("Skew_Y", 0);
    if (sameAngle(rotation + skewX, own.skewX, angleTolerance) &&
        sameAngle(rotation + skewY, own.skewY, angleTolerance) &&
        near(at0("Scale_X", 100 * own.scaleX) / 100, own.scaleX) &&
        near(at0("Scale_Y", 100 * own.scaleY) / 100, own.scaleY))
      return false;
  }
  // Relative curves start at the identity.
  const double rotation = at0("Rotation_Z", 0);
  if (sameAngle(rotation + at0("Skew_X", 0), 0, angleTolerance) &&
      sameAngle(rotation + at0("Skew_Y", 0), 0, angleTolerance) &&
      near(at0("Scale_X", 100) / 100, 1) && near(at0("Scale_Y", 100) / 100, 1))
    return true;
  if (ambiguous) *ambiguous = true;
  return false;
}

MotionPose motionPose(const MotionObject &motion, const Affine &base,
                      Point pivot, double frameOffset, double frameRate,
                      bool relative) {
  const double ticks = motionTicks(motion, frameOffset, frameRate);
  auto value         = [&](const QString &id, double fallback) {
    const auto found = motion.properties.constFind(id);
    return found == motion.properties.cend() ? fallback
                                                     : motionValue(*found, ticks);
  };
  const FlashTransform own = flashTransform(base);
  FlashTransform pose;
  if (relative) {
    const double rotation = value("Rotation_Z", 0);
    pose.scaleX           = own.scaleX * value("Scale_X", 100) / 100;
    pose.scaleY           = own.scaleY * value("Scale_Y", 100) / 100;
    pose.skewX            = own.skewX + rotation + value("Skew_X", 0);
    pose.skewY            = own.skewY + rotation + value("Skew_Y", 0);
  } else {
    const double rotation = value("Rotation_Z", own.skewY);
    pose.scaleX           = value("Scale_X", 100 * own.scaleX) / 100;
    pose.scaleY           = value("Scale_Y", 100 * own.scaleY) / 100;
    pose.skewX            = rotation + value("Skew_X", own.skewX - own.skewY);
    pose.skewY            = rotation + value("Skew_Y", 0);
  }
  MotionPose result;
  result.matrix = flashMatrix(pose);
  result.skewY  = pose.skewY;
  // The transformation point moves by (Motion_X, Motion_Y) from where the
  // element puts it; rotation, skew and scale pivot around it.
  const Point start = base.apply(pivot);
  const Point at{start.x + value("Motion_X", 0),
                 start.y + value("Motion_Y", 0)};
  result.matrix.tx =
      at.x - (result.matrix.a * pivot.x + result.matrix.c * pivot.y);
  result.matrix.ty =
      at.y - (result.matrix.b * pivot.x + result.matrix.d * pivot.y);
  return result;
}

std::optional<ColorTransform> motionColor(const MotionObject &motion,
                                          double frameOffset,
                                          double frameRate) {
  if (!motion.hasColor) return std::nullopt;
  const double ticks = motionTicks(motion, frameOffset, frameRate);
  auto has = [&](const QString &id) { return motion.properties.contains(id); };
  auto value = [&](const QString &id, double fallback) {
    const auto found = motion.properties.constFind(id);
    return found == motion.properties.cend() ? fallback
                                             : motionValue(*found, ticks);
  };
  ColorTransform result;
  // Animate's color effect styles; the advanced fields take precedence.
  static const char *channel[] = {"R", "G", "B", "A"};
  bool advanced                = false;
  for (const char *name : channel)
    advanced |= has(QString("AdvClr_%1_Pct").arg(name)) ||
                has(QString("AdvClr_%1_Offset").arg(name));
  if (advanced) {
    for (int index = 0; index < 4; ++index) {
      result.multiplier[index] =
          value(QString("AdvClr_%1_Pct").arg(channel[index]), 100) / 100;
      result.offset[index] =
          value(QString("AdvClr_%1_Offset").arg(channel[index]), 0);
    }
  } else if (has("Alpha_Amount")) {
    result.multiplier[3] = value("Alpha_Amount", 100) / 100;
  } else if (has("Brightness_Amount")) {
    const double amount =
        std::max(-1.0, std::min(1.0, value("Brightness_Amount", 0) / 100));
    for (int index = 0; index < 3; ++index) {
      result.multiplier[index] = 1 - std::abs(amount);
      result.offset[index]     = amount > 0 ? 255 * amount : 0;
    }
  } else if (has("Tint_Amount") || has("Tint_Color.r")) {
    const double amount =
        std::max(0.0, std::min(1.0, value("Tint_Amount", 0) / 100));
    static const char *tint[] = {"Tint_Color.r", "Tint_Color.g",
                                 "Tint_Color.b"};
    for (int index = 0; index < 3; ++index) {
      result.multiplier[index] = 1 - amount;
      result.offset[index]     = value(tint[index], 0) * amount;
    }
  }
  return result;
}

}  // namespace Flash
