#include "flashscene.h"
#include "../flash/parttracks.h"
#include "flashbitmap.h"
#include "flashpreflight.h"
#include "flashprovenance.h"
#include "flashtext.h"
#include "flasheffects.h"
#include "../flash/effects.h"
#include "../flash/model.h"
#include "../flash/motion.h"
#include "../flash/placement.h"
#include "../flash/shapetween.h"

#include "flare/levelproperties.h"
#include "flare/levelset.h"
#include "flare/fxdag.h"
#include "flare/sceneproperties.h"
#include "flare/stage.h"
#include "flare/tcamera.h"
#include "flare/tstageobject.h"
#include "flare/tstageobjectspline.h"
#include "flare/tstageobjecttree.h"
#include "tstroke.h"
#include "flare/txshcell.h"
#include "flare/txshchildlevel.h"
#include "flare/txshcolumn.h"
#include "flare/txsheet.h"
#include "flare/txshnoteset.h"
#include "flare/txshleveltypes.h"
#include "flare/txshsimplelevel.h"
#include "flare/txshlevelcolumn.h"
#include "flare/txshsoundcolumn.h"
#include "flare/txshsoundlevel.h"
#include "flare/tcolumnfxset.h"
#include "flare/tcolumnfx.h"
#include "flare/txshzeraryfxcolumn.h"
#include "tnotanimatableparam.h"
#include "tparamset.h"
#include "tdoublekeyframe.h"
#include "tdoubleparam.h"
#include "tfx.h"
#include "tparamcontainer.h"
#include "toutputproperties.h"
#include "tsound.h"
#include "tcolorstyles.h"
#include "tpalette.h"
#include "trop.h"

#include <QColor>
#include <QXmlStreamReader>
#include <QThread>
#include <thread>
#include <chrono>
#include <atomic>
#include <QElapsedTimer>
#include <QDir>
#include <QFileInfo>
#include <QRegularExpression>
#include <algorithm>
#include <array>
#include <cmath>
#include <cstring>
#include <numeric>
#include <optional>

namespace FlashNative {
namespace {

void heldKey(TStageObject *object, TStageObject::Channel channel, int frame,
             double value) {
  TDoubleKeyframe key(frame, value);
  key.m_type     = TDoubleKeyframe::Constant;
  key.m_prevType = TDoubleKeyframe::Constant;
  object->getParam(channel)->setKeyframe(key);
}

// Classic (CS3-style) motion tweens interpolate the instance transform to the
// next keyframe. Shape tweens become in-between drawings (queueShapeTween());
// motion-object tweens key their property curves (motionSpan()).
bool classicMotionTween(const Flash::Frame &frame) {
  return frame.properties.value("tweenType") == "motion";
}

using Compatibility::maskSilhouette;

// Best-effort timing when the authored graphic controls are not mapped:
// keep the first frame and a known loop mode, otherwise loop.
Flash::GraphicTiming fallbackGraphicTiming(const Flash::Element &element,
                                           const Flash::Frame &frame) {
  Flash::GraphicTiming timing{frame.index, frame.duration, 0,
                              Flash::PlaybackMode::Loop};
  // A button rests on its Up state, the first frame of its timeline.
  if (Flash::symbolInstanceType(element) == "button") {
    timing.mode = Flash::PlaybackMode::SingleFrame;
    return timing;
  }
  timing.firstFrame =
      std::max(0, element.properties.value("firstFrame", "0").toInt());
  try {
    timing.mode = Flash::playbackMode(element.properties.value("loop", "loop"));
  } catch (const Flash::Error &) {
  }
  return timing;
}

const TStageObject::Channel placementChannels[] = {
    TStageObject::T_X,      TStageObject::T_Y,      TStageObject::T_Angle,
    TStageObject::T_ScaleX, TStageObject::T_ScaleY, TStageObject::T_ShearX,
    TStageObject::T_ShearY};

// A classic motion tween from the previous keyframe of the same native track.
struct IncomingTween {
  int frame   = 0;
  double ease = 0;  // Flash acceleration / 100, in [-1, 1]
  // Custom ease: cubic Bezier control points (x = time, y = progress) in
  // [0, 1], 1 + 3n points for n segments. Empty for the standard ease.
  QVector<TPointD> curve;
  QString rotate  = "auto";
  int rotateTimes = 0;
  double angle    = 0;
};

// Reads a classic tween's <tweens><CustomEase> curve. The "all" target
// drives every property; otherwise the first curve is used (reported).
QVector<TPointD> customEase(const Flash::Frame &frame, bool *partial) {
  QVector<TPointD> chosen;
  *partial = false;
  for (const auto &extra : frame.extras) {
    if (extra.tag != "tweens") continue;
    QXmlStreamReader xml(extra.xml);
    QVector<TPointD> points;
    QString target;
    bool inEase = false;
    while (!xml.atEnd()) {
      xml.readNext();
      if (xml.isStartElement() && xml.name() == QLatin1String("CustomEase")) {
        inEase = true;
        points.clear();
        target = xml.attributes().value("target").toString();
      } else if (inEase && xml.isStartElement() &&
                 xml.name() == QLatin1String("Point")) {
        points.push_back(TPointD(xml.attributes().value("x").toDouble(),
                                 xml.attributes().value("y").toDouble()));
      } else if (inEase && xml.isEndElement() &&
                 xml.name() == QLatin1String("CustomEase")) {
        inEase           = false;
        const bool valid = points.size() >= 4 && (points.size() - 1) % 3 == 0;
        if (!valid) continue;
        if (target == "all" || target.isEmpty()) return points;
        if (chosen.isEmpty()) chosen = points;
        *partial = true;
      }
    }
  }
  return chosen;
}

// Progress at time t in [0, 1] along a piecewise cubic ease curve.
double easeAt(const QVector<TPointD> &curve, double t) {
  for (int first = 0; first + 3 < curve.size(); first += 3) {
    const TPointD p0 = curve[first], p1 = curve[first + 1],
                  p2 = curve[first + 2], p3 = curve[first + 3];
    if (t > p3.x && first + 4 < curve.size()) continue;
    auto at = [&](double u, double a, double b, double c, double d) {
      const double v = 1 - u;
      return v * v * v * a + 3 * v * v * u * b + 3 * v * u * u * c +
             u * u * u * d;
    };
    double low = 0, high = 1;
    for (int step = 0; step < 60; ++step) {
      const double mid                                   = (low + high) / 2;
      (at(mid, p0.x, p1.x, p2.x, p3.x) < t ? low : high) = mid;
    }
    return at((low + high) / 2, p0.y, p1.y, p2.y, p3.y);
  }
  return t;
}

// Interpolates a key pair with a classic tween's ease when one native
// segment represents it exactly; false for a custom ease of several
// segments. Flash's classic ease is t - e*t*(1-t), like a shape tween's:
// acceleration 100 starts slowly (Animate's published SWFs, and the
// CustomEase Animate saves for an acceleration: (1/3, (1-e)/3),
// (2/3, 1-(1+e)/3)). That cubic Bezier has its time handles at thirds, so
// native SpeedInOut reproduces it exactly.
bool easeKeys(TDoubleKeyframe &start, TDoubleKeyframe &end,
              const IncomingTween &tween) {
  const double span  = end.m_frame - start.m_frame;
  const double delta = end.m_value - start.m_value;
  if (tween.curve.size() == 4) {
    // One cubic segment: exactly a SpeedInOut segment, whose Bezier is
    // solved for time just as Flash's ease curve is.
    const TPointD c1 = tween.curve[1], c2 = tween.curve[2];
    start.m_type          = TDoubleKeyframe::SpeedInOut;
    start.m_linkedHandles = false;
    start.m_speedOut      = TPointD(c1.x * span, c1.y * delta);
    end.m_linkedHandles   = false;
    end.m_speedIn         = TPointD((c2.x - 1) * span, (c2.y - 1) * delta);
  } else if (!tween.curve.isEmpty())
    return false;
  else if (tween.ease == 0)
    start.m_type = TDoubleKeyframe::Linear;
  else {
    start.m_type          = TDoubleKeyframe::SpeedInOut;
    start.m_linkedHandles = false;
    start.m_speedOut      = TPointD(span / 3, delta * (1 - tween.ease) / 3);
    end.m_linkedHandles   = false;
    end.m_speedIn         = TPointD(-span / 3, -delta * (1 + tween.ease) / 3);
  }
  return true;
}

// Classic tweens interpolate the decomposed transform around the source
// transformation point: the pivot's position moves linearly while rotation,
// scale and skew are interpolated. Native stage objects compose
// T(position) * R * shear * S * T(-center), so a center at that pivot gives
// the same in-between placements. Untweened tracks keep a zero center.
double place(TStageObject *object, int frame, const Flash::Affine &matrix,
             double dpi, Flash::Point pivot, const IncomingTween *tween) {
  const auto placement = Flash::stagePlacement(matrix, dpi, pivot);
  object->setCenterAndOffset({placement.center.x, placement.center.y}, {});
  double angle = placement.angle;
  if (tween) {
    // Pick the equivalent angle Flash reaches from the previous key. Source
    // Y points down, so Flash's clockwise is a decreasing native angle.
    const double delta = std::remainder(angle - tween->angle, 360.0);
    if (tween->rotate == "clockwise")
      angle = tween->angle -
              std::fmod(360.0 - std::fmod(delta + 360.0, 360.0), 360.0) -
              360.0 * tween->rotateTimes;
    else if (tween->rotate == "counter-clockwise")
      angle = tween->angle + std::fmod(delta + 360.0, 360.0) +
              360.0 * tween->rotateTimes;
    else
      angle = tween->angle + delta;
  }
  const double values[] = {
      placement.position.x, placement.position.y, angle,
      placement.scaleX,     placement.scaleY,     placement.shearX,
      placement.shearY};
  for (int index = 0; index < 7; ++index) {
    TDoubleParam *param = object->getParam(placementChannels[index]);
    TDoubleKeyframe key(frame, values[index]);
    key.m_type     = TDoubleKeyframe::Constant;
    key.m_prevType = TDoubleKeyframe::Constant;
    if (tween && param->isKeyframe(tween->frame)) {
      TDoubleKeyframe start = param->getKeyframeAt(tween->frame);
      if (!easeKeys(start, key, *tween)) {
        // Several custom ease segments: a linear key on every in-between.
        const double span  = frame - tween->frame;
        const double delta = values[index] - start.m_value;
        start.m_type       = TDoubleKeyframe::Linear;
        for (int between = tween->frame + 1; between < frame; ++between) {
          TDoubleKeyframe sample(
              between,
              start.m_value + delta * easeAt(tween->curve,
                                             (between - tween->frame) / span));
          sample.m_type = TDoubleKeyframe::Linear;
          param->setKeyframe(sample);
        }
      }
      param->setKeyframe(start);
    }
    param->setKeyframe(key);
  }
  return angle;
}

double place(TXsheet *xsheet, int column, int frame,
             const Flash::Affine &matrix, double dpi, Flash::Point pivot,
             const IncomingTween *tween) {
  return place(xsheet->getStageObject(TStageObjectId::ColumnId(column)), frame,
               matrix, dpi, pivot, tween);
}

Flash::Affine inverse(const Flash::Affine &m) {
  const double determinant = m.determinant();
  Flash::Affine result;
  result.a  = m.d / determinant;
  result.b  = -m.b / determinant;
  result.c  = -m.c / determinant;
  result.d  = m.a / determinant;
  result.tx = -(result.a * m.tx + result.c * m.ty);
  result.ty = -(result.b * m.tx + result.d * m.ty);
  return result;
}

// The source (pixels, Y down) affine of a native placement. Native
// placements are D * A * D^-1 with D = diag(inch / dpi, -inch / dpi), the
// mapping place() keys.
Flash::Affine sourceAffine(const TAffine &native, double dpi) {
  const double unit = Stage::inch / dpi;
  Flash::Affine result;
  result.a  = native.a11;
  result.c  = -native.a12;
  result.tx = native.a13 / unit;
  result.b  = -native.a21;
  result.d  = native.a22;
  result.ty = -native.a23 / unit;
  return result;
}

// Motion-object tweens (Flash::motionPose()). Frame f of the span shows
// pre * pose(f) * post: `pre` maps the element's parent space to the
// column's (the stage origin, or a native parent's inverse), `post` is the
// artwork's own placement inside the element (a bitmap's centering, a Text
// FX column's scale). The channels pivot on `center`, as place() keys them.
struct MotionSpan {
  std::shared_ptr<const Flash::MotionObject> motion;
  Flash::Affine base;     // the element's matrix
  Flash::Point pivot;     // its transformation point
  bool relative = false;  // Flash::motionRelative()
  int start = 0, duration = 1;
  double frameRate = 24;
};

// The span's channel values frame by frame, in placementChannels order. The
// angle continues from the span's first key by the pose's own turning, so
// several turns in a few frames stay continuous.
std::vector<std::array<double, 7>> motionChannels(
    const MotionSpan &span, const Flash::Affine &pre, const Flash::Affine &post,
    Flash::Point center, double dpi, double startAngle) {
  std::vector<std::array<double, 7>> values;
  const double sense = pre.determinant() * post.determinant() < 0 ? -1.0 : 1.0;
  double angle = startAngle, turn = 0;
  for (int offset = 0; offset < span.duration; ++offset) {
    const auto pose = Flash::motionPose(*span.motion, span.base, span.pivot,
                                        offset, span.frameRate, span.relative);
    const auto placement =
        Flash::stagePlacement(pre * pose.matrix * post, dpi, center);
    // Source Y points down: Flash's skew angles turn the other way natively.
    const double turned =
        offset == 0 ? startAngle : angle + sense * (-pose.skewY - turn);
    angle = placement.angle +
            360.0 * std::round((turned - placement.angle) / 360.0);
    turn = -pose.skewY;
    values.push_back({placement.position.x, placement.position.y, angle,
                      placement.scaleX, placement.scaleY, placement.shearX,
                      placement.shearY});
  }
  return values;
}

// Keys channel `index` of a motion span. Where the channel is an affine
// function of one property curve whose keys lie on frames, the curve's own
// segments become native keys (Linear, or SpeedInOut with its Bezier
// handles; a Quadratic ease over one straight segment is a SpeedInOut
// segment too), kept only if they reproduce every frame of the span;
// otherwise every frame gets a key.
void keyMotionChannel(TDoubleParam *param, const MotionSpan &span,
                      const std::vector<std::array<double, 7>> &values,
                      int index) {
  const int count    = span.duration;
  const auto &motion = *span.motion;
  const double ticks = motion.timeScale / span.frameRate;  // per frame
  auto value         = [&](int offset) { return values[offset][index]; };
  auto close         = [](double a, double b) {
    return std::abs(a - b) <= 1e-6 * std::max(1.0, std::abs(b));
  };
  auto reproduces = [&](const std::vector<TDoubleKeyframe> &keys) {
    TDoubleParamP probe(new TDoubleParam(0.0));
    for (const auto &key : keys) probe->setKeyframe(key);
    for (int offset = 0; offset < count; ++offset)
      if (!close(probe->getValue(span.start + offset), value(offset)))
        return false;
    return true;
  };
  std::vector<TDoubleKeyframe> keys;
  bool constant = true;
  for (int offset = 1; offset < count; ++offset)
    constant = constant && close(value(offset), value(0));
  if (constant) {
    TDoubleKeyframe key(span.start, value(0));
    key.m_type = TDoubleKeyframe::Constant;
    keys.push_back(key);
  } else
    for (const char *id : {"Motion_X", "Motion_Y", "Rotation_Z", "Skew_X",
                           "Skew_Y", "Scale_X", "Scale_Y"}) {
      const auto found = motion.properties.constFind(id);
      if (found == motion.properties.cend() || found->keys.size() < 2) continue;
      const auto &curve = *found;
      std::vector<double> q(count);
      int low = 0, high = 0;
      for (int offset = 0; offset < count; ++offset) {
        q[offset] = Flash::motionValue(
            curve, Flash::motionTicks(motion, offset, span.frameRate));
        if (q[offset] < q[low]) low = offset;
        if (q[offset] > q[high]) high = offset;
      }
      if (q[high] - q[low] <= 1e-12) continue;
      const double beta  = (value(high) - value(low)) / (q[high] - q[low]);
      const double alpha = value(low) - beta * q[low];
      bool fits          = true;
      for (int offset = 0; offset < count && fits; ++offset)
        fits = close(alpha + beta * q[offset], value(offset));
      if (!fits) continue;
      // A Quadratic ease over one straight segment is a SpeedInOut segment
      // with its handles at thirds (as easeKeys()); other eases are not.
      double ease = 0;
      if (curve.ease && curve.ease->strength != 0) {
        const bool flat = curve.keys.size() == 2 &&
                          curve.keys[0].next.dt == 0 &&
                          curve.keys[1].previous.dt == 0;
        if (curve.ease->type != "Quadratic" || !flat) continue;
        ease = (curve.ease->strength > 0 ? 1 : -1) *
               std::min(std::abs(curve.ease->strength) / 100, 1.0);
      }
      std::vector<TDoubleKeyframe> candidate;
      bool onFrames = true;
      for (const auto &source : curve.keys) {
        const double frame = source.time / ticks;
        if (std::abs(frame - std::round(frame)) > 1e-6 || frame < -1e-6 ||
            frame > count - 1 + 1e-6) {
          onFrames = false;
          break;
        }
        TDoubleKeyframe key(span.start + std::lround(frame),
                            alpha + beta * source.value);
        key.m_linkedHandles = false;
        key.m_type          = TDoubleKeyframe::Constant;
        candidate.push_back(key);
      }
      if (!onFrames) continue;
      for (int segment = 0; segment + 1 < int(curve.keys.size()); ++segment) {
        const auto &from = curve.keys[segment], &to = curve.keys[segment + 1];
        auto &first = candidate[segment], &last = candidate[segment + 1];
        const double frames = last.m_frame - first.m_frame;
        const double delta  = last.m_value - first.m_value;
        first.m_type        = TDoubleKeyframe::Linear;
        if (ease != 0) {
          first.m_type     = TDoubleKeyframe::SpeedInOut;
          first.m_speedOut = TPointD(frames / 3, delta * (1 + ease) / 3);
          last.m_speedIn   = TPointD(-frames / 3, -delta * (1 - ease) / 3);
        } else if (from.next.dt != 0 || to.previous.dt != 0) {
          first.m_type = TDoubleKeyframe::SpeedInOut;
          first.m_speedOut =
              from.next.dt == 0
                  ? TPointD()
                  : TPointD(from.next.dt / ticks,
                            beta * (from.next.value - from.value));
          last.m_speedIn = to.previous.dt == 0
                               ? TPointD()
                               : TPointD(to.previous.dt / ticks,
                                         beta * (to.previous.value - to.value));
        }
      }
      if (candidate.front().m_frame > span.start) {
        TDoubleKeyframe hold(span.start, candidate.front().m_value);
        hold.m_type = TDoubleKeyframe::Constant;
        candidate.insert(candidate.begin(), hold);
      }
      if (reproduces(candidate)) {
        keys = candidate;
        break;
      }
    }
  if (keys.empty())
    for (int offset = 0; offset < count; ++offset) {
      TDoubleKeyframe key(span.start + offset, value(offset));
      key.m_type = offset + 1 < count ? TDoubleKeyframe::Linear
                                      : TDoubleKeyframe::Constant;
      keys.push_back(key);
    }
  // The span's first key keeps its incoming side (place()).
  for (const auto &key : keys)
    if (key.m_frame == span.start && param->isKeyframe(span.start)) {
      TDoubleKeyframe first = param->getKeyframeAt(span.start);
      first.m_value         = key.m_value;
      first.m_type          = key.m_type;
      first.m_speedOut      = key.m_speedOut;
      first.m_linkedHandles = false;
      param->setKeyframe(first);
    } else
      param->setKeyframe(key);
}

// Keys the frames of a motion-object span after its first key (place());
// false when its poses are not finite, so the span holds its first pose.
bool motionSpan(TStageObject *object, const MotionSpan &span,
                const Flash::Affine &pre, const Flash::Affine &post,
                Flash::Point center, double dpi, double startAngle) {
  std::vector<std::array<double, 7>> values;
  try {
    values = motionChannels(span, pre, post, center, dpi, startAngle);
  } catch (const Flash::Error &) {
    return false;
  }
  for (int index = 0; index < 7; ++index)
    keyMotionChannel(object->getParam(placementChannels[index]), span, values,
                     index);
  return true;
}

// Motion guides. A classic tween on a layer guided by a motion guide moves
// the transformation point of its instance along the guide's stroke, from
// the point of the path nearest the start key's pivot to the one nearest the
// end key's, by arc length and with the tween's ease; rotation, scale and
// skew tween as usual and the keys keep their own placement. (Animate's
// published SWFs: every in-between pivot lies on the path within a twip, at
// arc s0 + (s1 - s0) * ease(t).) Other layers parented to a guide, and
// tweens whose guide frame has no stroke, move as if unguided.

// The tween's ease starting at a classic tween keyframe (frame and ease
// only; rotation is chosen by place()). `partial` is set when a custom ease
// has per-property curves (customEase()).
IncomingTween tweenEase(const Flash::Frame &frame, bool *partial = nullptr) {
  IncomingTween tween;
  tween.frame       = frame.index;
  const bool custom = frame.properties.value("hasCustomEase") == "true";
  bool ignored      = false;
  if (custom) tween.curve = customEase(frame, partial ? partial : &ignored);
  if (!custom)
    tween.ease = std::max(
        -1.0,
        std::min(1.0, frame.properties.value("acceleration", "0").toDouble() /
                          100.0));
  return tween;
}

// Tween progress at time t in [0, 1].
double easeProgress(const IncomingTween &tween, double t) {
  return tween.curve.isEmpty() ? t - tween.ease * t * (1 - t)
                               : easeAt(tween.curve, t);
}

// A classic motion tween's ease with its rotation (motionTweenRotate).
IncomingTween withRotation(IncomingTween tween, const Flash::Frame &frame,
                           double angle) {
  tween.angle       = angle;
  tween.rotate      = frame.properties.value("motionTweenRotate", "auto");
  tween.rotateTimes = std::max(
      0, frame.properties.value("motionTweenRotateTimes", "0").toInt());
  return tween;
}

// The guide frame's strokes as one path of quadratic pieces in source
// pixels (lines have their control point halfway), in either direction.
// Empty when the frame has no stroke; nullopt when its strokes do not form
// one path, as Flash then has no single route to follow.
std::optional<QVector<Flash::Segment>> guideRoute(const Flash::Frame &frame,
                                                  const Flash::Affine &origin) {
  QVector<Flash::Segment> pieces;
  auto same = [](Flash::Point a, Flash::Point b) {
    return std::abs(a.x - b.x) < 1e-3 && std::abs(a.y - b.y) < 1e-3;
  };
  for (const auto &element : frame.elements) {
    if (element.kind != Flash::Element::Kind::Shape || !element.shape) continue;
    const Flash::Affine matrix = origin * element.matrix;
    for (const auto &edge : element.shape->edges) {
      if (!edge.stroke) continue;
      for (Flash::Segment piece : edge.segments) {
        piece.start   = matrix.apply(piece.start);
        piece.control = matrix.apply(piece.control);
        piece.end     = matrix.apply(piece.end);
        if (same(piece.start, piece.end) && same(piece.start, piece.control))
          continue;
        // Strokes drawn twice (both sides of a region) count once.
        const bool repeated = std::any_of(
            pieces.cbegin(), pieces.cend(), [&](const Flash::Segment &other) {
              return same(other.control, piece.control) &&
                     ((same(other.start, piece.start) &&
                       same(other.end, piece.end)) ||
                      (same(other.start, piece.end) &&
                       same(other.end, piece.start)));
            });
        if (!repeated) pieces.push_back(piece);
      }
    }
  }
  if (pieces.isEmpty()) return QVector<Flash::Segment>();
  auto reversed = [](Flash::Segment piece) {
    std::swap(piece.start, piece.end);
    return piece;
  };
  QVector<Flash::Segment> route{pieces.takeFirst()};
  for (bool grew = true; grew && !pieces.isEmpty();) {
    grew = false;
    for (int index = 0; index < pieces.size(); ++index) {
      const Flash::Segment piece = pieces[index];
      if (same(piece.start, route.back().end))
        route.push_back(piece);
      else if (same(piece.end, route.back().end))
        route.push_back(reversed(piece));
      else if (same(piece.end, route.front().start))
        route.push_front(piece);
      else if (same(piece.start, route.front().start))
        route.push_front(reversed(piece));
      else
        continue;
      pieces.removeAt(index);
      grew = true;
      break;
    }
  }
  if (!pieces.isEmpty()) return std::nullopt;
  return route;
}

// Native stroke control points (stage units) of a route in source pixels.
std::vector<TThickPoint> routePoints(const QVector<Flash::Segment> &route,
                                     double dpi) {
  const double unit = Stage::inch / dpi;
  auto native       = [unit](Flash::Point point) {
    return TThickPoint(point.x * unit, -point.y * unit, 0);
  };
  std::vector<TThickPoint> points{native(route.front().start)};
  for (const auto &piece : route) {
    points.push_back(native(piece.control));
    points.push_back(native(piece.end));
  }
  return points;
}

// On a guided layer, each chain of keyframes linked by classic tweens on one
// track gets a track (native column) of its own, so that column can follow
// the guide as a native motion path while the layer's other artwork keeps
// its own placements.
Flash::PartTrackPlan splitGuidedChains(Flash::PartTrackPlan plan,
                                       const Flash::Layer &layer) {
  const QVector<QVector<int>> original = plan.frameTracks;
  const int tracks                     = plan.trackCount;
  for (int track = 0; track < tracks; ++track) {
    int chain = -1;
    for (int ordinal = 0; ordinal < layer.frames.size(); ++ordinal) {
      const int slot = original[ordinal].indexOf(track);
      if (slot < 0) {
        chain = -1;
        continue;
      }
      const bool onward = classicMotionTween(layer.frames[ordinal]) &&
                          ordinal + 1 < layer.frames.size() &&
                          original[ordinal + 1].contains(track);
      if (chain < 0 && onward) {
        chain = plan.trackCount++;
        plan.trackNames.push_back(plan.trackNames.value(track));
      }
      if (chain >= 0) plan.frameTracks[ordinal][slot] = chain;
      if (!onward) chain = -1;
    }
  }
  return plan;
}

class Builder {
  Flash::Package &m_package;
  const Flash::DocumentInfo &m_info;
  const BuildOptions &m_options;
  BuildResult m_result;
  QMap<QString, TXshChildLevel *> m_symbols;
  // Mask artwork: while m_maskArtwork > 0 shapes are built as their mask
  // silhouette (maskSilhouette()), symbols as opaque fill-only copies kept in
  // m_maskSymbols, and alpha and Text FX are left out. Copies (the innermost
  // last in m_maskCopyNames while built) have no source bindings, which the
  // painted symbol holds; their issues wait in m_maskCopyIssues and are
  // reported only for symbols never built painted (see degrade()).
  QMap<QString, TXshChildLevel *> m_maskSymbols;
  int m_maskArtwork = 0;
  QStringList m_maskCopyNames;
  QMap<QString, QVector<QPair<CompatibilityIssue, IssueHandling>>>
      m_maskCopyIssues;
  bool m_maskOwnIssue = false;  // an issue of the mask artwork itself
  struct DepthScope {
    int &depth;
    const bool active;
    DepthScope(int &value, bool enter) : depth(value), active(enter) {
      if (active) ++depth;
    }
    ~DepthScope() {
      if (active) --depth;
    }
  };
  struct MaskCopyScope {
    QStringList &names;
    const bool active;
    MaskCopyScope(QStringList &list, const QString &name)
        : names(list), active(!name.isEmpty()) {
      if (active) names.push_back(name);
    }
    ~MaskCopyScope() {
      if (active) names.pop_back();
    }
  };
  QMap<QString, TXshSimpleLevel *> m_bitmaps;
  QSet<QString> m_building;
  TXshSimpleLevel *m_blank   = nullptr;
  int m_levelSerial          = 0;
  quint64 m_geometrySegments = 0;
  quint64 m_bitmapPixels     = 0;
  quint64 m_styleBindings    = 0;
  quint64 m_strokeBindings   = 0;
  quint64 m_paintWitnesses   = 0;
  struct StageBindingSite {
    int bindingIndex = -1;
    TXsheet *sheet   = nullptr;
    int column = -1, first = -1, count = 0;
  };
  QVector<StageBindingSite> m_stageBindings;
  // A shape queued for concurrent conversion (see convertQueuedVectors()).
  struct VectorJob {
    Flash::Element element;
    TXshSimpleLevel *level = nullptr;
    int frame              = 0;  // native frame number
    int bindingIndex       = -1;
    QString maskCopyOf;  // the symbol of a mask copy drawing (see symbol())
    // Results
    TVectorImageP image;
    QVector<VectorStyleBinding> styles;
    QVector<VectorStrokeBinding> strokes;
    VectorFrameAudit audit;
    QString error;
  };
  std::vector<VectorJob> m_vectorJobs;

  void convertJob(VectorJob &job, const std::atomic<bool> &cancelled) {
    try {
      job.image = vectorImage(
          job.element, job.level->getPalette(), Stage::inch / m_options.dpi,
          [&cancelled] {
            if (cancelled) throw Flash::Error("Document reading cancelled.");
          },
          &job.styles, &job.strokes, &job.audit, m_options.maxPaintWitnesses,
          m_options.bestEffort);
    } catch (const std::exception &error) {
      job.error = QString::fromUtf8(error.what());
    } catch (...) {
      job.error = "Unexpected vector conversion failure.";
    }
  }

  // Converts every queued shape on all cores. Conversions only read their
  // level palettes (see registerVectorPaints), and the vector core used here
  // keeps no shared mutable state. The calling thread keeps polling
  // cancellation and reporting progress, so UI callbacks stay on it.
  void convertQueuedVectors() {
    const int total = int(m_vectorJobs.size());
    if (!total) return;
    std::atomic<int> next{0}, done{0};
    std::atomic<bool> cancelled{false};
    const int workers = std::max(
        1,
        std::min<int>(std::min<int>(QThread::idealThreadCount(), 16), total));
    std::vector<std::thread> threads;
    for (int index = 0; index < workers; ++index)
      threads.emplace_back([&] {
        for (int job; (job = next++) < total && !cancelled;) {
          convertJob(m_vectorJobs[job], cancelled);
          ++done;
        }
      });
    try {
      while (done < total) {
        if (m_options.progress) m_options.progress(int(done), total);
        check();
        std::this_thread::sleep_for(std::chrono::milliseconds(30));
      }
    } catch (...) {
      cancelled = true;
      for (auto &thread : threads) thread.join();
      throw;
    }
    for (auto &thread : threads) thread.join();
    if (m_options.progress) m_options.progress(total, total);
  }

  // Applies the conversions in their original order, as a sequential build
  // would have, including omissions of shapes that could not be converted.
  void applyQueuedVectors() {
    for (auto &job : m_vectorJobs) {
      check();
      MaskCopyScope copy(m_maskCopyNames, job.maskCopyOf);
      if (!job.error.isEmpty() || !job.image) {
        const Flash::Error error(job.error.isEmpty()
                                     ? QString("No native vector image.")
                                     : job.error);
        if (!m_options.bestEffort)
          throw Flash::Error(job.element.sourceId + ": " + job.error);
        omitAfterFailure("vector-conversion-failed", job.element.sourceId,
                         error);
        // Keep the exposure and timing; the drawing frame is simply empty.
        TVectorImageP empty(new TVectorImage());
        empty->setRegionFindingMode(
            TVectorImage::RegionFindingMode::PreserveSource);
        empty->setPalette(job.level->getPalette());
        job.level->setFrame(TFrameId(job.frame), TImageP(empty.getPointer()));
        if (job.bindingIndex >= 0) {
          auto binding =
              m_result.nativeSourceMap.at(job.bindingIndex).toObject();
          binding.remove("nativeDrawingFrame");
          binding["nativeOmitted"] = true;
          m_result.nativeSourceMap.replace(job.bindingIndex, binding);
        }
        continue;
      }
      if (quint64(job.audit.paint.size()) >
          m_options.maxPaintWitnesses -
              std::min(m_paintWitnesses, m_options.maxPaintWitnesses))
        throw Flash::Error("Native source paint witness budget exceeded: " +
                           job.element.sourceId);
      m_paintWitnesses += job.audit.paint.size();
      if (job.audit.approximatedFills)
        degrade({"vector-fill-approximated", job.element.sourceId,
                 "Fill regions were matched by sampling, without an exact "
                 "topology certificate: " +
                     job.audit.approximationReason},
                IssueHandling::Approximate);
      job.level->setFrame(TFrameId(job.frame), TImageP(job.image.getPointer()));
      m_result.vectorFrameAudits[job.level].insert(job.frame,
                                                   std::move(job.audit));
      if (job.bindingIndex < 0) continue;
      auto binding = m_result.nativeSourceMap.at(job.bindingIndex).toObject();
      binding["nativeContentSha256"] = timed("contentDigest", [&] {
        return drawingContentDigest(TImageP(job.image.getPointer()));
      });
      if (quint64(job.styles.size()) >
          quint64(m_options.maxProvenanceEntries) -
              std::min(m_styleBindings,
                       quint64(m_options.maxProvenanceEntries)))
        throw Flash::Error("Native source style provenance budget exceeded.");
      m_styleBindings += job.styles.size();
      QJsonArray nativeStyles;
      for (const auto &style : job.styles)
        nativeStyles.append(QJsonObject{
            {"role", style.role},
            {"sourceStyleId", style.sourceStyleId},
            {"sourcePaintId", style.sourcePaintId},
            {"sourceStyleIndex", style.sourceStyleIndex},
            {"nativePaletteStyleIndex", style.nativePaletteStyleIndex}});
      binding["nativeStyles"] = nativeStyles;
      if (quint64(job.strokes.size()) >
          m_options.maxStrokeBindings -
              std::min(m_strokeBindings, m_options.maxStrokeBindings))
        throw Flash::Error("Native source stroke provenance budget exceeded.");
      m_strokeBindings += job.strokes.size();
      QJsonArray nativeStrokes;
      for (const auto &stroke : job.strokes)
        nativeStrokes.append(
            QJsonObject{{"sourceEdgeId", stroke.sourceEdgeId},
                        {"sourceEdgeXmlId", stroke.sourceEdgeXmlId},
                        {"sourceEdgeIndex", stroke.sourceEdgeIndex},
                        {"sourceContourIndex", stroke.sourceContourIndex},
                        {"sourceFirstSegment", stroke.sourceFirstSegment},
                        {"sourceSegmentCount", stroke.sourceSegmentCount},
                        {"nativeStrokeIndex", stroke.nativeStrokeIndex},
                        {"nativeFirstChunk", stroke.nativeFirstChunk},
                        {"nativeChunkCount", stroke.nativeChunkCount},
                        {"nativeControlCount", stroke.nativeControlCount}});
      binding["nativeStrokes"] = nativeStrokes;
      m_result.nativeSourceMap.replace(job.bindingIndex, binding);
    }
    m_vectorJobs.clear();
  }

  // Shape tweens (DOMFrame tweenType="shape") morph a keyframe's shapes into
  // those of the next keyframe of the layer. Every in-between row gets a
  // drawing of its own in the element's drawing level, interpolated as
  // Animate does (see flash/shapetween.h) with the frame's ease, and queued
  // for conversion like any other shape. The plans of one keyframe serve all
  // of its shape elements.
  const Flash::Frame *m_shapeTweenFrame = nullptr;
  QString m_shapeTweenFrameId;
  QVector<std::optional<Flash::ShapeTween>> m_shapeTweens;
  QMap<QString, int> m_shapeTweenCounts;  // tweens per method
  int m_shapeTweenDrawings = 0;

  // Queues the in-between drawings of element `slot` of a shape-tween
  // keyframe and returns how many rows they cover (0: the keyframe holds).
  // Call before queuing the keyframe's own job, which must stay last.
  int queueShapeTween(const Flash::Layer &layer, int ordinal, int slot,
                      TXshSimpleLevel *level) {
    const auto &frame = layer.frames[ordinal];
    if (frame.properties.value("tweenType") != "shape" || frame.duration < 2 ||
        ordinal + 1 >= layer.frames.size() ||
        layer.frames[ordinal + 1].index != frame.index + frame.duration)
      return 0;
    const auto &element = frame.elements[slot];
    if (m_shapeTweenFrame != &frame || m_shapeTweenFrameId != frame.sourceId) {
      m_shapeTweenFrame   = &frame;
      m_shapeTweenFrameId = frame.sourceId;
      QStringList notes;
      m_shapeTweens = timed("shapeTween", [&] {
        return Flash::planShapeTweens(frame, layer.frames[ordinal + 1], notes);
      });
      for (const auto &note : notes)
        sink()({"shape-tween-approximated", frame.sourceId, note});
    }
    if (slot >= m_shapeTweens.size() || !m_shapeTweens[slot]) return 0;
    const Flash::ShapeTween &tween = *m_shapeTweens[slot];
    for (const auto &note : tween.approximations)
      sink()({"shape-tween-approximated", element.sourceId, note});
    // Flash's acceleration or a custom curve, as in classic tweens:
    // published SWFs place in-between t at morph ratio t - a * t * (1 - t),
    // a = acceleration / 100 (-100 gives 0.75 halfway, 100 gives 0.25).
    const IncomingTween ease = reportedEase(frame);
    for (int step = 1; step < frame.duration; ++step) {
      check();
      const int row          = frame.index + step;
      const double t         = double(step) / frame.duration;
      Flash::Element between = element;
      between.sourceId =
          element.sourceId + "/shapeTween" + QString::number(row);
      between.shape =
          std::make_shared<Flash::Shape>(tween.at(easeProgress(ease, t)));
      // A tweened mask clips with each in-between's silhouette.
      if (m_maskArtwork)
        between.shape =
            std::make_shared<Flash::Shape>(maskSilhouette(*between.shape));
      registerVectorPaints(between, level->getPalette(),
                           Stage::inch / m_options.dpi, fillBitmaps());
      VectorJob job;
      job.element = std::move(between);
      job.level   = level;
      job.frame   = row + 1;
      if (!m_maskCopyNames.isEmpty()) job.maskCopyOf = m_maskCopyNames.back();
      m_vectorJobs.push_back(std::move(job));
      ++m_shapeTweenDrawings;
    }
    static const char *const methods[] = {"morph", "structure", "resampled"};
    ++m_shapeTweenCounts[methods[int(tween.method)]];
    return frame.duration - 1;
  }

  // Set by the best-effort sink when the current element or layer is omitted.
  bool m_omit                                  = false;
  static constexpr int maxReportedDegradations = 4000;
  static constexpr int maxSamplesPerCode       = 100;

  void check() { m_package.checkCancelled(); }

  // Accumulates wall time per construction step for the import report.
  QMap<QString, qint64> m_profileNs;
  template <class F>
  auto timed(const char *step, F &&work) -> decltype(work()) {
    QElapsedTimer clock;
    clock.start();
    struct Add {
      QMap<QString, qint64> &map;
      const char *step;
      QElapsedTimer &clock;
      ~Add() { map[step] += clock.nsecsElapsed(); }
    } add{m_profileNs, step, clock};
    return work();
  }

  void degrade(const CompatibilityIssue &issue, IssueHandling handling) {
    // A movie clip clock replay repeats its symbol's issues (clockReplay()).
    if (m_replaying) return;
    // A mask copy repeats the painted symbol's issues: they wait until the
    // build ends and count only if that symbol was never built painted
    // (reportMaskCopyIssues()). Issues of the mask artwork itself count now.
    if (!m_maskCopyNames.isEmpty() && !m_maskOwnIssue) {
      m_maskCopyIssues[m_maskCopyNames.back()].push_back({issue, handling});
      return;
    }
    ++(handling == IssueHandling::Omit ? m_result.omitted
                                       : m_result.approximated);
    const double count =
        m_result.degradationCounts.value(issue.code).toDouble() + 1;
    m_result.degradationCounts[issue.code] = count;
    // Sample every kind of issue: frequent benign codes must not crowd out
    // rare ones. Counts stay complete.
    if (count <= maxSamplesPerCode &&
        m_result.degradations.size() < maxReportedDegradations)
      m_result.degradations.append(
          QJsonObject{{"code", issue.code},
                      {"sourceId", issue.sourceId},
                      {"message", issue.message},
                      {"handling", Compatibility::handlingName(handling)}});
  }

  void reportMaskCopyIssues() {
    const auto pending = std::move(m_maskCopyIssues);
    m_maskCopyIssues.clear();
    for (auto it = pending.cbegin(); it != pending.cend(); ++it)
      if (!m_symbols.contains(it.key()))
        for (const auto &entry : it.value()) degrade(entry.first, entry.second);
  }

  // A detail of mask artwork that the mask silhouette cannot express.
  void degradeMask(const QString &sourceId, const QString &message) {
    if (!m_options.bestEffort) throw Flash::Error(sourceId + ": " + message);
    const bool own = m_maskOwnIssue;
    m_maskOwnIssue = true;
    degrade({"unmapped-mask-compositing", sourceId, message},
            IssueHandling::Approximate);
    m_maskOwnIssue = own;
  }

  // Strict builds reject on the first issue. Best-effort builds reject only
  // blocking issues and record the rest; Omit marks the current element.
  // tweenEase(), reporting per-property custom ease curves.
  IncomingTween reportedEase(const Flash::Frame &frame) {
    bool partial              = false;
    const IncomingTween tween = tweenEase(frame, &partial);
    if (partial)
      degrade({"unmapped-tween", frame.sourceId + "/CustomEase",
               "Per-property ease curves are approximated with one curve for "
               "all properties."},
              IssueHandling::Approximate);
    return tween;
  }

  CompatibilitySink sink() {
    return [this](const CompatibilityIssue &issue) {
      const IssueHandling handling = m_options.bestEffort
                                         ? Compatibility::handling(issue.code)
                                         : IssueHandling::Fatal;
      if (handling == IssueHandling::Fatal) Compatibility::reject(issue);
      degrade(issue, handling);
      if (handling == IssueHandling::Omit) m_omit = true;
    };
  }

  // Converts a failure of one element into an omission in best-effort builds.
  // Cancellation and strict builds rethrow.
  void omitAfterFailure(const QString &code, const QString &sourceId,
                        const Flash::Error &error) {
    check();
    if (!m_options.bestEffort) throw error;
    degrade({code, sourceId, QString::fromUtf8(error.what())},
            IssueHandling::Omit);
  }

  // Binds the rows [start, start + count) of a document scene's sub-xsheet
  // column to its source timeline.
  void sceneBinding(TXsheet *root, int index, TXshChildLevel *level,
                    const QString &sourceId, int start, int count) {
    FlashPlacementData placement;
    placement.sourceId = sourceId;
    placement.kind     = "document-scene";
    placement.level    = level;
    placement.start    = start;
    placement.duration = count;
    const QString nativePlacementId =
        root->getColumn(index)->getLevelColumn()->addFlashPlacement(placement);
    sourceBinding(QJsonObject{
        {"kind", "document-scene"},
        {"sourceId", sourceId},
        {"nativeColumn", index},
        {"nativeFirstRow", start},
        {"nativeRowCount", count},
        {"nativeLevelName", QString::fromStdWString(level->getName())},
        {"nativeLevelId", level->flashNativeId()},
        {"nativePlacementId", nativePlacementId}});
  }

  void sourceBinding(const QJsonObject &binding) {
    check();
    if (m_result.nativeSourceMap.size() >= m_options.maxProvenanceEntries)
      throw Flash::Error("Native source provenance budget exceeded.");
    m_result.nativeSourceMap.append(binding);
  }

  QString levelName(const QString &label) {
    check();
    if (m_levelSerial >= m_options.maxLevels)
      throw Flash::Error("Native level budget exceeded.");
    return label + " [Flash " + QString::number(++m_levelSerial) + "]";
  }

  TXshChildLevel *child(const QString &label) {
    const auto name = levelName(label);
    auto level      = std::make_unique<TXshChildLevel>(name.toStdWString());
    level->setScene(m_result.scene.get());
    level->ensureFlashNativeId();
    m_result.scene->getLevelSet()->insertLevel(level.get());
    ++m_result.childLevels;
    return level.release();
  }

  TXshSimpleLevel *drawing(const QString &label) {
    const auto name = levelName(label);
    auto level      = std::make_unique<TXshSimpleLevel>(name.toStdWString());
    level->setScene(m_result.scene.get());
    level->ensureFlashNativeId();
    level->setType(PLI_XSHLEVEL);
    const QString filename =
        QDir(m_options.assetDirectory)
            .filePath(
                QString("flash_%1.pli").arg(m_levelSerial, 6, 10, QChar('0')));
    if (QFileInfo::exists(filename))
      throw Flash::Error("Import asset path already exists: " + filename);
    level->setPath(TFilePath(filename.toStdWString()));
    TPaletteP palette(new TPalette());
    level->setPalette(palette.getPointer());
    level->getProperties()->setDpi(m_options.dpi);
    level->getProperties()->setImageRes({m_info.width, m_info.height});
    level->setDirtyFlag(true);
    m_result.scene->getLevelSet()->insertLevel(level.get());
    ++m_result.drawingLevels;
    return level.release();
  }

  TXshSimpleLevel *bitmap(const QString &libraryName) {
    check();
    if (auto *existing = m_bitmaps.value(libraryName, nullptr)) return existing;
    TRasterImageP image =
        bitmapImage(m_package, m_info, libraryName, m_options.dpi);
    if (!image || !image->getRaster())
      throw Flash::Error("Embedded bitmap has no native raster: " +
                         libraryName);
    const quint64 pixels = quint64(image->getRaster()->getLx()) *
                           quint64(image->getRaster()->getLy());
    if (pixels > m_options.maxBitmapPixels -
                     std::min(m_bitmapPixels, m_options.maxBitmapPixels))
      throw Flash::Error("Native bitmap pixel budget exceeded: " + libraryName);
    m_bitmapPixels += pixels;
    const auto name = levelName(libraryName);
    auto level      = std::make_unique<TXshSimpleLevel>(name.toStdWString());
    level->setScene(m_result.scene.get());
    level->ensureFlashNativeId();
    level->setType(OVL_XSHLEVEL);
    const QString filename =
        QDir(m_options.assetDirectory)
            .filePath(
                QString("flash_%1..png").arg(m_levelSerial, 6, 10, QChar('0')));
    if (QFileInfo::exists(filename))
      throw Flash::Error("Import asset path already exists: " + filename);
    level->setPath(TFilePath(filename.toStdWString()));
    level->getProperties()->setDpi(m_options.dpi);
    level->getProperties()->setImageRes(
        {image->getRaster()->getLx(), image->getRaster()->getLy()});
    level->setFrame(TFrameId(1), image);
    level->setDirtyFlag(true);
    m_result.scene->getLevelSet()->insertLevel(level.get());
    ++m_result.drawingLevels;
    auto *result = level.release();
    m_bitmaps.insert(libraryName, result);
    const QString contentDigest = drawingContentDigest(image);
    sourceBinding(QJsonObject{
        {"kind", "library-bitmap"},
        {"sourceId", "DOMDocument.xml/media/DOMBitmapItem/" + libraryName},
        {"nativeLevelName", QString::fromStdWString(result->getName())},
        {"nativeLevelId", result->flashNativeId()},
        {"nativeContentSha256", contentDigest}});
    return result;
  }

  // Bitmaps of BitmapFill paints (TFlashBitmapStyle), decoded once per
  // library item; the problem is kept when one cannot be decoded.
  QHash<QString, QPair<VectorBitmap, QString>> m_fillBitmaps;

  // Resolves the bitmaps of bitmap fills while their paints are registered.
  // A fill whose bitmap cannot be decoded keeps a solid fallback, reported
  // for every drawing that uses it (strict builds reject it).
  VectorBitmapResolver fillBitmaps() {
    return [this](const Flash::Paint &paint) {
      const QString name = paint.properties.value("bitmapPath");
      auto found         = m_fillBitmaps.find(name);
      if (found == m_fillBitmaps.end()) {
        VectorBitmap bitmap;
        QString problem;
        try {
          check();
          const TRasterImageP image =
              bitmapImage(m_package, m_info, name, m_options.dpi);
          const TRaster32P raster =
              image ? TRaster32P(image->getRaster()) : TRaster32P();
          if (!raster) throw Flash::Error("Bitmap decoder returned no raster.");
          const quint64 pixels =
              quint64(raster->getLx()) * quint64(raster->getLy());
          if (pixels > m_options.maxBitmapPixels -
                           std::min(m_bitmapPixels, m_options.maxBitmapPixels))
            throw Flash::Error("Native bitmap pixel budget exceeded: " + name);
          m_bitmapPixels += pixels;
          // Native rasters are premultiplied; the fill keeps straight RGBA.
          TRaster32P straight(raster->getLx(), raster->getLy());
          straight->copy(raster);
          TRop::depremultiply(straight);
          bitmap.pixels = straight;
          bitmap.smooth = m_info.assets.value(name)
                              .toObject()
                              .value("allowSmoothing")
                              .toString() == "true";
        } catch (const Flash::Error &error) {
          check();  // cancellation is never approximated
          problem = QString::fromUtf8(error.what());
        }
        found = m_fillBitmaps.insert(name, {bitmap, problem});
      }
      if (!found.value().second.isEmpty()) {
        const CompatibilityIssue issue{
            "unmapped-vector-paint", paint.sourceId,
            "The bitmap of this bitmap fill cannot be decoded; it is drawn in "
            "a solid color: " +
                found.value().second};
        if (!m_options.bestEffort) Compatibility::reject(issue);
        degrade(issue, IssueHandling::Approximate);
      }
      return found.value().first;
    };
  }

  // Embedded videos: one raster level of PNG frames per library item,
  // decoded once on first use (BuildOptions::decodeVideo) into the asset
  // directory. An item's issues are repeated for each of its instances.
  struct VideoItem {
    TXshSimpleLevel *level = nullptr;
    int frames = 0, width = 0, height = 0;
    QVector<CompatibilityIssue> issues;
    // An audible sound track, played with the instances (queueVideoSound()).
    std::shared_ptr<const Flash::SoundPayload> sound;
  };
  QMap<QString, VideoItem> m_videos;
  quint64 m_videoFrames = 0;
  // Decoded frames are files, not memory: about two hours at 25 fps.
  static constexpr quint64 maxVideoFrames = 200000;

  const VideoItem &video(const QString &libraryName, const QString &sourceId) {
    check();
    auto found = m_videos.constFind(libraryName);
    if (found != m_videos.cend()) return found.value();
    VideoItem item;
    const auto collect = [&item](const CompatibilityIssue &issue) {
      item.issues.push_back(issue);
    };
    const auto payload =
        Compatibility::videoMedia(m_package, m_info, libraryName, sourceId,
                                  bool(m_options.decodeVideo), collect);
    const auto refuse = [&](const QString &code, const QString &message) {
      item.issues.push_back(
          {code, sourceId,
           "Video \"" + libraryName + "\" is not placed: " + message});
    };
    if (!payload) {
    } else if (quint64(payload->frames.size()) >
               maxVideoFrames - std::min(m_videoFrames, maxVideoFrames)) {
      refuse("video-budget-exceeded",
             "the native video frame budget is exceeded.");
    } else {
      try {
        if (payload->sound == Flash::VideoPayload::Sound::Audible)
          item.sound = std::make_shared<const Flash::SoundPayload>(
              Flash::videoSoundTrack(*payload));
        item.level  = timed("videoDecode",
                            [&] { return videoLevel(libraryName, *payload); });
        item.frames = payload->frames.size();
        item.width  = payload->width;
        item.height = payload->height;
        m_videoFrames += quint64(item.frames);
      } catch (const Flash::Error &error) {
        item.sound.reset();
        check();  // cancellation escapes
        refuse("video-decode-failed", QString::fromUtf8(error.what()));
      }
    }
    return m_videos.insert(libraryName, item).value();
  }

  TXshSimpleLevel *videoLevel(const QString &libraryName,
                              const Flash::VideoPayload &payload) {
    const QString name = levelName(libraryName);
    const QString base =
        QString("flash_%1").arg(m_levelSerial, 6, 10, QChar('0'));
    const QString filename =
        QDir(m_options.assetDirectory).filePath(base + "..png");
    if (QFileInfo::exists(filename))
      throw Flash::Error("Import asset path already exists: " + filename);
    auto level = std::make_unique<TXshSimpleLevel>(name.toStdWString());
    level->setScene(m_result.scene.get());
    level->ensureFlashNativeId();
    level->setType(OVL_XSHLEVEL);
    // Set up like an embedded bitmap level while its files do not exist yet.
    level->setPath(TFilePath(filename.toStdWString()));
    const int frames = payload.frames.size();
    m_options.decodeVideo(payload, m_options.assetDirectory, base,
                          [&](int done) {
                            check();
                            if (m_options.videoProgress)
                              m_options.videoProgress(done, frames);
                          });
    // FFmpeg's PNG files carry no resolution: give the level the import DPI,
    // so one video pixel is one stage pixel, as for embedded bitmaps.
    level->getProperties()->setDpiPolicy(LevelProperties::DP_CustomDpi);
    level->getProperties()->setDpi(m_options.dpi);
    level->getProperties()->setImageDpi(TPointD(m_options.dpi, m_options.dpi));
    level->getProperties()->setImageRes({payload.width, payload.height});
    for (int frame = 1; frame <= frames; ++frame) {
      const TFrameId id(frame);
      if (!QFileInfo(QString::fromStdWString(
                         level->getPath().withFrame(id).getWideString()))
               .isFile()) {
        for (int written = 1; written <= frames; ++written)
          QFile::remove(QString::fromStdWString(
              level->getPath().withFrame(TFrameId(written)).getWideString()));
        throw Flash::Error("Decoded video frame is missing: " + libraryName);
      }
      level->setFrame(id, TImageP());  // loaded from its file on demand
    }
    m_result.scene->getLevelSet()->insertLevel(level.get());
    ++m_result.drawingLevels;
    m_result.videoLevels.insert(level.get());
    return level.release();
  }

  // Flash plays an embedded video in step with the timeline from the
  // keyframe where it appears. Following keyframes of the layer that keep
  // the same video continue it, as one placed object.
  static bool showsVideo(const Flash::Frame &frame,
                         const QString &libraryName) {
    return std::any_of(frame.elements.cbegin(), frame.elements.cend(),
                       [&](const Flash::Element &element) {
                         return element.kind == Flash::Element::Kind::Video &&
                                element.properties.value("libraryItemName") ==
                                    libraryName;
                       });
  }
  static int videoClockStart(const Flash::Layer &layer, int ordinal,
                             const QString &libraryName) {
    int start = layer.frames[ordinal].index;
    for (int previous = ordinal - 1; previous >= 0; --previous) {
      const auto &frame = layer.frames[previous];
      if (!showsVideo(frame, libraryName) ||
          frame.index + frame.duration != start)
        break;
      start = frame.index;
    }
    return start;
  }

  // The frame after the last keyframe continuing that placed video.
  static int videoClockEnd(const Flash::Layer &layer, int ordinal,
                           const QString &libraryName) {
    int end = layer.frames[ordinal].index + layer.frames[ordinal].duration;
    for (int next = ordinal + 1; next < layer.frames.size(); ++next) {
      const auto &frame = layer.frames[next];
      if (!showsVideo(frame, libraryName) || frame.index != end) break;
      end = frame.index + frame.duration;
    }
    return end;
  }

  int column(TXsheet *xsheet, const QString &label, bool visible,
             bool editorOnly = false) {
    check();
    if (m_result.columns >= m_options.maxColumns)
      throw Flash::Error("Native column budget exceeded.");
    const int index = xsheet->getColumnCount();
    xsheet->insertColumn(index, TXshColumn::eLevelType);
    xsheet->getStageObject(TStageObjectId::ColumnId(index))
        ->setName(label.toUtf8().toStdString());
    auto *native = xsheet->getColumn(index);
    native->setPreviewVisible(visible && !editorOnly);
    native->setCamstandVisible(visible);
    ++m_result.columns;
    return index;
  }

  void cell(TXsheet *xsheet, int row, int column, TXshLevel *level, int frame) {
    if (m_result.cells >= m_options.maxCells)
      throw Flash::Error("Native exposure budget exceeded.");
    if ((m_result.cells & 1023) == 0) check();
    if (!xsheet->setCell(row, column, TXshCell(level, TFrameId(frame + 1))))
      throw Flash::Error("Could not insert native exposure.");
    ++m_result.cells;
  }

  // Flash text is drawn by Flare's Text FX (stdfx Iwa_TextFx with
  // "Transform with Column" on), in zerary FX columns placed like any other
  // element (see textFxItems()); its glyph outlines stay in a hidden
  // reference column. The FX comes from the stdfx plugin library, which the
  // application and tcomposer load. Without it the outlines show the text.
  static constexpr const char *textFxType = "STD_iwa_TextFx";
  int m_textFxState = 0;  // 1 available, -1 not available, 0 not checked yet

  bool textFxAvailable() {
    if (!m_textFxState) {
      TFxP probe(TFx::create(textFxType));
      // A Text FX with the column transform and the layout options.
      const TParamContainer *params = probe && probe->getFxType() == textFxType
                                          ? probe->getParams()
                                          : nullptr;
      m_textFxState = params && params->getParam("transformWithColumn") &&
                              params->getParam("shrinkToFit")
                          ? 1
                          : -1;
    }
    return m_textFxState > 0;
  }

  // A Text FX column of a text track. A later keyframe drawing the same FX
  // content continues the column, so held text keeps one editable FX.
  struct TextFxColumn {
    QString key;
    int column  = -1;
    int lastRow = -1;
  };

  static QString textFxKey(const TextFxItem &item) {
    return QStringList{item.text,
                       item.font,
                       QString::number(item.color.r),
                       QString::number(item.color.g),
                       QString::number(item.color.b),
                       QString::number(item.color.m),
                       QString::number(item.alignment),
                       QString::number(item.center.x, 'g', 17),
                       QString::number(item.center.y, 'g', 17),
                       QString::number(item.width, 'g', 17),
                       QString::number(item.height, 'g', 17),
                       QString::number(item.columnScale, 'g', 17),
                       QString::number(int(item.kerning)),
                       QString::number(int(item.shrinkToFit)),
                       QString::number(item.letterSpacing, 'g', 17),
                       QString::number(item.lineSpacing, 'g', 17),
                       QString::number(item.indent, 'g', 17)}
        .join(QChar(0x1f));
  }

  static TFx *textFxOf(TXsheet *xsheet, int column) {
    auto *native = xsheet->getColumn(column)
                       ? xsheet->getColumn(column)->getZeraryFxColumn()
                       : nullptr;
    return native ? native->getZeraryColumnFx()->getZeraryFx() : nullptr;
  }

  // Creates a zerary column whose Text FX draws `item`, exposed on
  // [first, first + count).
  int textFxColumn(TXsheet *xsheet, const QString &label, bool visible,
                   bool editorOnly, const TextFxItem &item, int first,
                   int count) {
    check();
    if (m_result.columns >= m_options.maxColumns)
      throw Flash::Error("Native column budget exceeded.");
    TFxP fx(TFx::create(textFxType));
    TParamContainer *params = fx ? fx->getParams() : nullptr;
    auto param              = [&](const char *name) {
      return params ? params->getParam(name) : nullptr;
    };
    auto *text   = dynamic_cast<TStringParam *>(param("text"));
    auto *font   = dynamic_cast<TFontParam *>(param("font"));
    auto *align  = dynamic_cast<TIntEnumParam *>(param("hAlign"));
    auto *center = dynamic_cast<TPointParam *>(param("center"));
    auto *width  = dynamic_cast<TDoubleParam *>(param("width"));
    auto *height = dynamic_cast<TDoubleParam *>(param("height"));
    auto *color  = dynamic_cast<TPixelParam *>(param("textColor"));
    auto *follow = dynamic_cast<TBoolParam *>(param("transformWithColumn"));
    // Layout options (see FlashNative::textFxItems()).
    auto *kerning       = dynamic_cast<TBoolParam *>(param("kerning"));
    auto *shrinkToFit   = dynamic_cast<TBoolParam *>(param("shrinkToFit"));
    auto *letterSpacing = dynamic_cast<TDoubleParam *>(param("letterSpacing"));
    auto *lineSpacing   = dynamic_cast<TDoubleParam *>(param("lineSpacing"));
    auto *indent        = dynamic_cast<TDoubleParam *>(param("indent"));
    if (!text || !font || !align || !center || !width || !height || !color ||
        !follow || !kerning || !shrinkToFit || !letterSpacing || !lineSpacing ||
        !indent)
      throw Flash::Error("Cannot create a native Text FX.");
    kerning->setValue(item.kerning);
    shrinkToFit->setValue(item.shrinkToFit);
    letterSpacing->setDefaultValue(item.letterSpacing);
    lineSpacing->setDefaultValue(item.lineSpacing);
    indent->setDefaultValue(item.indent);
    text->setValue(item.text.toStdWString());
    font->setValue(item.font.toStdWString());
    align->setValue(item.alignment);
    center->setDefaultValue(item.center);
    width->setDefaultValue(item.width);
    height->setDefaultValue(item.height);
    if (item.color.m < 255) color->enableMatte(true);
    color->setDefaultValue(item.color);
    follow->setValue(true);
    // A zerary column's cells reference its own FX level: keep the one the
    // constructor exposes, then clear its row.
    auto *native            = new TXshZeraryFxColumn(1);
    const TXshCell exposure = native->getCell(0);
    native->clearCells(0, 1);
    native->getZeraryColumnFx()->setZeraryFx(fx.getPointer());
    const int index = xsheet->getColumnCount();
    xsheet->insertColumn(index, native);
    xsheet->getFxDag()->assignUniqueId(fx.getPointer());
    xsheet->getStageObject(TStageObjectId::ColumnId(index))
        ->setName(label.toUtf8().toStdString());
    native->setPreviewVisible(visible && !editorOnly);
    native->setCamstandVisible(visible);
    ++m_result.columns;
    textFxCells(xsheet, index, first, count, exposure);
    return index;
  }

  void textFxCells(TXsheet *xsheet, int column, int first, int count,
                   TXshCell exposure = TXshCell()) {
    auto *native = xsheet->getColumn(column)->getZeraryFxColumn();
    int r0 = 0, r1 = -1;
    if (exposure.isEmpty() && native->getRange(r0, r1) > 0)
      exposure = native->getCell(r0);
    if (exposure.isEmpty())
      throw Flash::Error("Could not expose a native Text FX.");
    for (int row = first; row < first + count; ++row) {
      if (m_result.cells >= m_options.maxCells)
        throw Flash::Error("Native exposure budget exceeded.");
      if ((m_result.cells & 1023) == 0) check();
      if (!xsheet->setCell(row, column, exposure))
        throw Flash::Error("Could not insert native exposure.");
      ++m_result.cells;
    }
  }

  void preserveDuration(TXsheet *xsheet, int frameCount) {
    xsheet->updateFrameCount();
    if (xsheet->getFrameCount() >= frameCount) return;
    // Native xsheets derive duration from their last nonempty cell. A hidden
    // transparent drawing keeps an intentionally blank trailing source frame.
    if (!m_blank) {
      m_blank = drawing("Flash empty frame");
      TVectorImageP image(new TVectorImage(true));
      image->setRegionFindingMode(
          TVectorImage::RegionFindingMode::PreserveSource);
      image->setPalette(m_blank->getPalette());
      m_blank->setFrame(TFrameId(1), image);
    }
    const int index = column(xsheet, "Flash duration", false);
    cell(xsheet, frameCount - 1, index, m_blank, 0);
    xsheet->getColumn(index)->lock(true);
    xsheet->updateFrameCount();
  }

  TXshChildLevel *symbol(const QString &name) {
    check();
    // Mask artwork places an opaque, fill-only copy of the symbol (its mask
    // silhouette), built and cached apart from the painted symbol.
    const bool maskCopy = m_maskArtwork > 0;
    const QString key   = maskCopy ? QString(QChar(0x1f)) + name : name;
    auto &symbols       = maskCopy ? m_maskSymbols : m_symbols;
    if (m_building.contains(key))
      throw Flash::Error("Cyclic source symbol: " + name);
    if (symbols.contains(name)) return symbols.value(name);
    const auto source = m_info.symbols.constFind(name);
    if (source == m_info.symbols.cend())
      throw Flash::Error("Missing source symbol: " + name);
    // Nested symbols recurse through timeline(); bound the native stack.
    if (m_building.size() >= m_package.limits().maxSymbolDepth)
      throw Flash::Error("Source symbol nesting exceeds the depth limit: " +
                         name);
    m_building.insert(key);
    struct Building {
      QSet<QString> &set;
      const QString &name;
      ~Building() { set.remove(name); }
    } building{m_building, key};
    // A copy's issues repeat the painted symbol's (see degrade()).
    MaskCopyScope copy(m_maskCopyNames, maskCopy ? name : QString());
    const auto definition = timed("readDefinition", [&] {
      return Flash::readDefinition(m_package, source->member);
    });
    Compatibility::definition(definition, true, sink());
    // Mask copies repeat geometry the preflight budget already counted.
    if (!maskCopy) {
      if (definition.geometrySegments >
          m_package.limits().maxGeometrySegments - m_geometrySegments)
        throw Flash::Error("Native geometry budget exceeded.");
      m_geometrySegments += definition.geometrySegments;
    }
    auto *level = child(maskCopy ? name + " (Flash mask)" : name);
    {
      ContainerScope container(m_container, definition.type.isEmpty()
                                                ? QString("movie clip")
                                                : definition.type);
      timeline(level->getXsheet(), definition.timelines.front(), {},
               source->member + "/timeline0", false, false);
    }
    // How the symbol's movie clip instances play: its frame scripts (Flash
    // ignores those of graphic symbols) give the playhead.
    {
      QStringList problems;
      const auto &symbolTimeline = definition.timelines.front();
      m_playheads.insert(
          level, definition.type.isEmpty() || definition.type == "movie clip"
                     ? Flash::timelinePlayhead(symbolTimeline, false,
                                               maskCopy ? nullptr : &problems)
                     : Flash::Playhead::linear(symbolTimeline.frameCount));
      for (const auto &problem : problems)
        sink()({"scripted-playback", source->member + "/timeline0", problem});
    }
    // What its placements need to play sounds (flattenClipSounds()).
    if (!maskCopy && !m_clipSoundTimelines.contains(name))
      m_clipSoundTimelines.insert(
          name, clipSoundTimeline(name, definition.timelines.front(),
                                  source->member + "/timeline0"));
    if (!maskCopy)
      sourceBinding(QJsonObject{
          {"kind", "library-symbol"},
          {"sourceId", source->member},
          {"nativeLevelName", QString::fromStdWString(level->getName())},
          {"nativeLevelId", level->flashNativeId()}});
    symbols.insert(name, level);
    return level;
  }

  // Movie clip clocks ---------------------------------------------------------
  // Flash plays a movie clip on its own clock from the tick it appears and
  // keeps it while its placement stays on stage, whatever the timeline that
  // holds it does (graphics are flattened into their parent timeline; a loop
  // or a goto keeps a clip placed on the frame it returns to). Native cells
  // show a clip nested in a symbol Xsheet on a local clock: rows [first, end)
  // of the Xsheet show clip ticks row - first. Clips whose display changes
  // over time record these runs per Xsheet, so every placement of the Xsheet
  // is checked (clockMismatch()): where the placement's rows do not advance
  // one frame per tick through a run, it plays a replay of the symbol whose
  // rows are ticks (clockReplay()), and nested clips get Flash's clocks.
  struct ClockRun {
    int first = 0, end = 0;  // Rows of the holding Xsheet
    int prefix = 0,
        period = 1;  // The clip repeats every period ticks after prefix
    int lane   = 0;  // One clip placement; it continues across a loop
    QString sourceId;
  };
  QMap<const TXsheet *, QVector<ClockRun>> m_clockRuns;
  QMap<QString, int> m_clockLanes;
  // How the movie clip instances of a symbol play: the native level (shared
  // Xsheet or clock replay), ticks to its rows, and when the display repeats.
  struct ClipClock {
    TXshChildLevel *level = nullptr;
    std::optional<Flash::Playhead> playhead;  // None: linear
    int prefix = 0, period = 1;
    bool animated = false;
  };
  QMap<const TXshChildLevel *, Flash::Playhead> m_playheads;  // see symbol()
  QMap<const TXshChildLevel *, ClipClock> m_clipClocks;
  QMap<QString, TXshChildLevel *> m_graphicReplays;
  int m_replaying = 0;  // Building a replay, whose issues are already reported
  static constexpr int maxReplayRows = 2400;

  static qint64 clockPhase(qint64 elapsed, int prefix, int period) {
    return elapsed < prefix ? elapsed
                            : prefix + (elapsed - prefix) % std::max(1, period);
  }

  int clockLane(const QString &key) {
    const auto found = m_clockLanes.constFind(key);
    if (found != m_clockLanes.cend()) return found.value();
    const int lane = m_clockLanes.size();
    m_clockLanes.insert(key, lane);
    return lane;
  }

  void recordClockRun(const TXsheet *xsheet, const ClockRun &run) {
    for (auto &known : m_clockRuns[xsheet])
      if (known.lane == run.lane && known.first == run.first) {
        known.end = std::max(known.end, run.end);
        return;
      }
    m_clockRuns[xsheet].push_back(run);
  }

  // Why Flash shows the nested clips of `runs` differently from their local
  // clocks while rows 0..rows-1 of a placement show frames frameAt(row) of
  // the Xsheet holding them (frameCount frames); empty when it does not.
  // `projected` receives the runs as rows of the placement when they match.
  QString clockMismatch(const QVector<ClockRun> &runs, int frameCount, int rows,
                        const std::function<int(int)> &frameAt,
                        QVector<ClockRun> *projected = nullptr) {
    struct Shown {
      qint64 elapsed;
      int run, projected;
    };
    QMap<int, Shown> shown;  // lane -> clip on stage
    int previous = -1;
    for (int row = 0; row < rows; ++row) {
      if ((row & 1023) == 0) check();
      const int frame = frameAt(row);
      QMap<int, Shown> next;
      for (int index = 0; index < runs.size(); ++index) {
        const auto &run = runs[index];
        if (frame < run.first || frame >= run.end || next.contains(run.lane))
          continue;
        const auto before = shown.constFind(run.lane);
        // Flash keeps the clip on its placement, and across a loop onto the
        // placement that continues it.
        const bool kept =
            before != shown.cend() &&
            (runs[before->run].first == run.first ||
             (runs[before->run].end >= frameCount && run.first == 0));
        const qint64 elapsed = kept ? before->elapsed + 1 : 0;
        const qint64 local   = frame - run.first;
        if (clockPhase(elapsed, run.prefix, run.period) !=
            clockPhase(local, run.prefix, run.period)) {
          const QString clip = "movie clip " + run.sourceId;
          if (!kept)
            return QString(
                       "frame %1 is entered part way through the span of "
                       "%2, which Flash starts there")
                .arg(frame + 1)
                .arg(clip);
          if (frame == previous)
            return QString("frame %1 is held while %2 plays on")
                .arg(frame + 1)
                .arg(clip);
          if (frame < previous && run.first == 0 &&
              runs[before->run].end >= frameCount)
            return QString(
                       "the timeline loops while %1 keeps playing across "
                       "the loop")
                .arg(clip);
          return QString(
                     "the playhead jumps from frame %1 to %2 while %3 "
                     "keeps playing")
              .arg(previous + 1)
              .arg(frame + 1)
              .arg(clip);
        }
        int at = kept ? before->projected : -1;
        if (projected) {
          if (at < 0) {
            projected->push_back(
                {row, row + 1, run.prefix, run.period, run.lane, run.sourceId});
            at = projected->size() - 1;
          } else
            (*projected)[at].end = row + 1;
        }
        next.insert(run.lane, {elapsed, index, at});
      }
      shown    = std::move(next);
      previous = frame;
    }
    return {};
  }

  // Records the clip runs a placement shows at rows [first, first + rows) of
  // `xsheet`, in the lanes of the placement's track.
  void projectClockRuns(const TXsheet *xsheet, const QString &laneKey,
                        int first, const QVector<ClockRun> &runs) {
    for (auto run : runs) {
      run.lane = clockLane(laneKey + QChar(0x1f) + QString::number(run.lane));
      run.first += first;
      run.end += first;
      recordClockRun(xsheet, run);
    }
  }

  // A native Xsheet that plays frames[t] of a symbol timeline on its row t,
  // so its nested movie clips get the clocks Flash gives them. Its issues are
  // the symbol's, already reported, apart from tweens it cuts.
  TXshChildLevel *clockReplay(const QString &name,
                              const Flash::Timeline &source,
                              const QVector<int> &frames,
                              const QString &timelineId,
                              const QString &container) {
    QStringList approximations;
    Flash::Timeline replay;
    try {
      replay = Flash::replayTimeline(
          source, frames,
          [this](const QString &symbol) {
            const auto info = m_info.symbols.constFind(symbol);
            return info == m_info.symbols.cend() || info->timelines.isEmpty()
                       ? 1
                       : info->timelines.front().frameCount;
          },
          &approximations);
    } catch (const Flash::Error &) {
      check();
      return nullptr;
    }
    auto *level = child(name + " [clock]");
    {
      ContainerScope scope(m_container, container);
      DepthScope replaying(m_replaying, true);
      timeline(level->getXsheet(), replay, {}, timelineId, false, false);
    }
    approximations.removeDuplicates();
    for (const auto &note : approximations) {
      const int separator = note.indexOf(": ");
      sink()({"unmapped-movie-clip-clock",
              separator > 0 ? note.left(separator) : timelineId,
              "Movie clip clock replay: " +
                  (separator > 0 ? note.mid(separator + 2) : note)});
    }
    return level;
  }

  // How movie clip instances of `level` (a symbol's shared Xsheet) play;
  // computed on the first one. A replay covers the ticks until the display
  // repeats: the playhead's cycle and those of the nested clips.
  ClipClock clipClock(TXshChildLevel *level, const QString &name) {
    const auto known = m_clipClocks.constFind(level);
    if (known != m_clipClocks.cend()) return known.value();
    ClipClock clock;
    clock.level     = level;
    const int count = std::max(1, level->getXsheet()->getFrameCount());
    const Flash::Playhead playhead =
        m_playheads.value(level, Flash::Playhead::linear(count));
    if (!playhead.isLinear(count)) clock.playhead = playhead;
    const QSet<int> visited(playhead.frames.cbegin(), playhead.frames.cend());
    const auto runs = m_clockRuns.value(level->getXsheet());
    clock.animated  = visited.size() > 1 || !runs.isEmpty();
    clock.prefix    = playhead.loopStart;
    clock.period    = visited.size() > 1 ? playhead.cycle() : 1;
    if (!runs.isEmpty()) {
      qint64 period    = playhead.cycle();
      int nestedPrefix = 0;
      for (const auto &run : runs) {
        period = period / std::gcd<qint64>(period, run.period) * run.period;
        nestedPrefix = std::max(nestedPrefix, run.prefix);
        if (period > maxReplayRows) break;
      }
      const qint64 loopStart = playhead.loopStart + nestedPrefix + period;
      const qint64 rows      = loopStart + period;
      QString reason =
          period > maxReplayRows
              ? QString("the nested clips repeat only after %1 ticks")
                    .arg(period)
              : clockMismatch(runs, count, int(rows + 1),
                              [&](int tick) { return playhead.frameAt(tick); });
      if (reason.isEmpty())
        clock.period = playhead.cycle();
      else if (rows <= maxReplayRows && !m_maskArtwork) {
        const auto source      = m_info.symbols.constFind(name);
        TXshChildLevel *replay = nullptr;
        if (source != m_info.symbols.cend()) {
          const auto definition =
              Flash::readDefinition(m_package, source->member);
          QVector<int> frames(static_cast<int>(rows));
          for (int tick = 0; tick < frames.size(); ++tick)
            frames[tick] = playhead.frameAt(tick);
          replay = clockReplay(name, definition.timelines.front(), frames,
                               source->member + "/timeline0", "movie clip");
        }
        if (replay) {
          Flash::Playhead ticks = Flash::Playhead::linear(int(rows));
          ticks.loopStart       = int(loopStart);
          clock.level           = replay;
          clock.playhead        = ticks;
          clock.prefix          = int(loopStart);
          clock.period          = int(period);
          reason.clear();
        }
      }
      if (!reason.isEmpty()) {
        sink()({"unmapped-movie-clip-clock", runs.front().sourceId,
                "Nested movie clips play on local clocks of " + name + ": " +
                    reason + "."});
        clock.period = int(std::min<qint64>(period, maxReplayRows));
      }
    }
    m_clipClocks.insert(level, clock);
    return clock;
  }

  // A graphic (or group) placement shows rows [0, rows) as frames frameAt(row)
  // of `level`. When nested clips there would lose their Flash clocks it
  // returns a replay to show instead, row by row; otherwise records their
  // runs in the placing Xsheet. Returns nullptr when the shared level fits.
  TXshChildLevel *placeClockRuns(TXsheet *xsheet, TXshChildLevel *level,
                                 const QString &name, const QString &laneKey,
                                 int first, int rows,
                                 const std::function<int(int)> &frameAt,
                                 const QString &sourceId) {
    const auto runs = m_clockRuns.value(level->getXsheet());
    if (runs.isEmpty()) return nullptr;
    const int count = std::max(1, level->getXsheet()->getFrameCount());
    QVector<ClockRun> projected;
    QString reason = clockMismatch(runs, count, rows, frameAt, &projected);
    if (reason.isEmpty()) {
      projectClockRuns(xsheet, laneKey, first, projected);
      return nullptr;
    }
    TXshChildLevel *replay = nullptr;
    const auto source      = m_info.symbols.constFind(name);
    if (rows <= maxReplayRows && !m_maskArtwork &&
        source != m_info.symbols.cend()) {
      QVector<int> frames(rows);
      QString key = name;
      for (int row = 0; row < rows; ++row) {
        frames[row] = frameAt(row);
        key += QChar(0x1f) + QString::number(frames[row]);
      }
      replay = m_graphicReplays.value(key);
      if (!replay) {
        const auto definition =
            Flash::readDefinition(m_package, source->member);
        replay = clockReplay(name, definition.timelines.front(), frames,
                             source->member + "/timeline0", "graphic");
        if (replay) m_graphicReplays.insert(key, replay);
      }
    }
    if (!replay) {
      sink()({"unmapped-movie-clip-clock", sourceId,
              "Nested movie clips of " + name +
                  " play on local clocks: " + reason + "."});
      return nullptr;
    }
    projectClockRuns(xsheet, laneKey, first,
                     m_clockRuns.value(replay->getXsheet()));
    return replay;
  }

  // Keeps a frame script readable in the Xsheet: a note on its frame's row
  // with the script as written and what the import made of it.
  void scriptNote(TXsheet *xsheet, const Flash::Layer &layer,
                  const Flash::Frame &frame) {
    if (m_replaying || m_maskArtwork || !m_maskCopyNames.isEmpty()) return;
    const QString script = Flash::frameScript(frame);
    if (script.trimmed().isEmpty()) return;
    QString outcome;
    if (m_container != "document" && m_container != "movie clip")
      outcome = "Flash ignores scripts in graphic symbols.";
    else {
      const auto parsed = Flash::parseFrameScript(script);
      QStringList applied, dropped;
      for (const auto &command : parsed.commands)
        if (!command.root || m_container == "document") applied << command.text;
      for (const auto &problem : parsed.problems)
        if (!dropped.contains(problem.code)) dropped << problem.code;
      if (!applied.isEmpty())
        outcome =
            "Imported as this timeline's timing: " + applied.join("; ") + ".";
      if (!dropped.isEmpty())
        outcome += QString(outcome.isEmpty() ? "" : " ") +
                   "Not run (see the import report): " + dropped.join(", ") +
                   ".";
    }
    TXshNoteSet::Note note;
    note.m_row  = frame.index;
    note.m_col  = 0;
    note.m_text = QString(
                      "<b>Flash frame script</b> (layer \"%1\", frame %2)"
                      "<pre>%3</pre>%4")
                      .arg(layer.name.toHtmlEscaped())
                      .arg(frame.index + 1)
                      .arg(script.trimmed().toHtmlEscaped())
                      .arg(outcome.toHtmlEscaped());
    xsheet->getNotes()->addNote(note);
  }

  // A group becomes a sub-xsheet whose members are placed exactly as if they
  // were on the parent timeline (Animate stores them in the parent's space),
  // so the group column itself is untransformed.
  TXshChildLevel *group(const Flash::Element &element, int duration,
                        const Flash::Affine &origin) {
    auto *level = child("Flash group");
    Flash::Timeline contents;
    contents.frameCount = duration;
    Flash::Layer layer;
    layer.name = "Group members";
    Flash::Frame frame;
    frame.duration = duration;
    frame.elements = element.members;
    layer.frames.push_back(std::move(frame));
    contents.layers.push_back(std::move(layer));
    ContainerScope container(m_container, "group");
    timeline(level->getXsheet(), contents, origin, element.sourceId, false,
             true);
    return level;
  }

  // Flash's layer eye is an authoring-view toggle. The published movie also
  // shows hidden layers when "Include hidden layers" is on (the default), so
  // the native columns follow the document's publish setting.
  bool visible(const Flash::Timeline &timeline, int layerIndex,
               bool editorOnly = false) {
    int steps   = 0;
    bool hidden = false;
    while (layerIndex >= 0) {
      if (layerIndex >= timeline.layers.size() ||
          ++steps > timeline.layers.size())
        throw Flash::Error("Invalid layer parent hierarchy.");
      const auto &layer = timeline.layers[layerIndex];
      hidden |= !layer.visible;
      layerIndex = layer.parent;
    }
    // Editor-only columns (guides, unlinked masks) are never published, so
    // they keep the authored eye state.
    return (m_publishHiddenLayers && !editorOnly) || !hidden;
  }

  bool m_publishHiddenLayers = true;

  // Reads "Include hidden layers" from the document's publish profile.
  static bool publishesHiddenLayers(Flash::Package &package) {
    if (!package.contains("PublishSettings.xml")) return true;
    const QString settings =
        QString::fromUtf8(package.read("PublishSettings.xml", 4 * 1024 * 1024));
    const int tag = settings.indexOf("<InvisibleLayer>");
    if (tag < 0) return true;
    return settings.mid(tag + 16, 8).trimmed().left(1) != "0";
  }

  // Layers a mask layer clips: its descendants (a folder inside the mask
  // passes it on to its own layers), which follow it in the layer list.
  static QVector<int> maskedLayers(const Flash::Timeline &source, int mask) {
    QVector<int> masked;
    for (int index = mask + 1; index < source.layers.size(); ++index) {
      int ancestor = source.layers[index].parent, steps = 0;
      while (ancestor > mask && ancestor < source.layers.size() &&
             ++steps <= source.layers.size())
        ancestor = source.layers[ancestor].parent;
      if (ancestor == mask) masked.push_back(index);
    }
    return masked;
  }

  // A drawing that covers far more than any stage: a mask cell with it lets
  // everything through. Flash draws masked layers unclipped where the mask
  // layer has no artwork; the mask column holds this drawing on those rows,
  // so the In FX matte (render) and the native mask (viewer) both pass.
  TXshSimpleLevel *m_unmasked = nullptr;
  TXshSimpleLevel *unmaskedDrawing() {
    if (m_unmasked) return m_unmasked;
    const QString reach = "2000000";  // twips: 100000 pixels each way
    auto shape          = std::make_shared<Flash::Shape>();
    Flash::Paint fill;  // opaque black solid
    shape->fills.insert(1, fill);
    Flash::Edge edge;
    edge.fill1    = 1;
    edge.segments = Flash::decodeEdges(
        "!-" + reach + " -" + reach + "|" + reach + " -" + reach + "|" + reach +
        " " + reach + "|-" + reach + " " + reach + "|-" + reach + " -" + reach);
    shape->edges.push_back(edge);
    Flash::Element element;
    element.kind     = Flash::Element::Kind::Shape;
    element.tag      = "DOMShape";
    element.sourceId = "import/unmasked-rows";
    element.shape    = shape;
    auto *level      = drawing("Flash unmasked rows");
    registerVectorPaints(element, level->getPalette(),
                         Stage::inch / m_options.dpi);
    VectorFrameAudit audit;
    TVectorImageP image = vectorImage(
        element, level->getPalette(), Stage::inch / m_options.dpi,
        [this] { check(); }, nullptr, nullptr, &audit);
    if (!image) throw Flash::Error("Cannot build the unmasked-rows drawing.");
    level->setFrame(TFrameId(1), TImageP(image.getPointer()));
    m_result.vectorFrameAudits[level].insert(1, std::move(audit));
    m_unmasked = level;
    return level;
  }

  // Builds a mask layer that needs several native columns (several elements
  // at once): its artwork becomes one sub-xsheet shown in one column named
  // like the layer, so the native mask is the union of the elements.
  void groupMask(TXsheet *xsheet, const Flash::Timeline &source, int mask,
                 Flash::Affine origin, const QString &sourceMember,
                 bool linearRootClock,
                 const QVector<QVector<int>> &sourceColumns) {
    const auto &maskLayer = source.layers[mask];
    // Keep layer indices (source IDs) and the mask's parents; every other
    // layer becomes an empty folder. The sub-xsheet lasts as long as the
    // artwork.
    Flash::Timeline part;
    part.name     = source.name;
    part.sourceId = source.sourceId;
    for (const auto &frame : maskLayer.frames)
      if (!frame.elements.isEmpty())
        part.frameCount =
            std::max(part.frameCount, frame.index + frame.duration);
    part.frameCount = std::min(part.frameCount, std::max(1, source.frameCount));
    for (int index = 0; index < source.layers.size(); ++index) {
      const auto &original = source.layers[index];
      if (index == mask) {
        Flash::Layer artwork = original;
        artwork.type         = "normal";  // drawn, as matte
        part.layers.push_back(std::move(artwork));
        continue;
      }
      Flash::Layer stub;
      stub.sourceId = original.sourceId;
      stub.name     = original.name;
      stub.type     = "folder";
      stub.parent   = original.parent;
      stub.visible  = original.visible;
      stub.locked   = original.locked;
      part.layers.push_back(std::move(stub));
    }
    auto *maskLevel = child(maskLayer.name + " (Flash mask)");
    {
      DepthScope artwork(m_maskArtwork, true);  // drawn as mask silhouettes
      timeline(maskLevel->getXsheet(), part, origin, sourceMember,
               linearRootClock, false);
    }
    const int maskColumn =
        column(xsheet, maskLayer.name, visible(source, mask), false);
    const int frames = std::max(1, source.frameCount);
    for (const auto &frame : maskLayer.frames) {
      if (frame.elements.isEmpty()) continue;  // blank keyframe: no cells
      for (int row = std::max(0, frame.index);
           row < std::min(frames, frame.index + frame.duration); ++row)
        cell(xsheet, row, maskColumn, maskLevel, row);
    }
    xsheet->getColumn(maskColumn)->lock(locked(source, mask));
    linkMask(xsheet, source, mask, {maskColumn}, sourceColumns,
             sourceMember + "/layer" + QString::number(mask));
  }

  // Connects a mask layer's native column(s) to the columns of the layers it
  // masks, as Animate structures it: the mask column is followed in stacking
  // order by the masked columns and clips all of them (native mask scope) in
  // the editor viewer and playback; the render clips them with an In FX
  // whose Source composes the masked columns and whose Matte is the mask.
  void linkMask(TXsheet *xsheet, const Flash::Timeline &source, int mask,
                const QVector<int> &columns,
                const QVector<QVector<int>> &sourceColumns,
                const QString &layerId) {
    const auto &layer         = source.layers[mask];
    const QVector<int> masked = maskedLayers(source, mask);
    QVector<int> childColumns;  // back to front
    for (int index = masked.size() - 1; index >= 0; --index)
      childColumns += sourceColumns[masked[index]];
    if (masked.isEmpty()) return;
    if (columns.isEmpty() || childColumns.isEmpty()) {
      // Nothing to clip, or a mask layer without artwork, which leaves its
      // children unclipped in Flash too. Only mask artwork that was left out
      // (reported where it was omitted) changes the result.
      bool omittedArtwork = false;
      if (columns.isEmpty() && !childColumns.isEmpty())
        for (const auto &frame : layer.frames)
          omittedArtwork |= !frame.elements.isEmpty();
      if (omittedArtwork) {
        if (!m_options.bestEffort)
          throw Flash::Error(layerId + ": Incomplete linked mask FX inputs.");
        degrade({"unmapped-mask-compositing", layerId,
                 "Mask layer has no native mask artwork; its children are "
                 "shown unmasked."},
                IssueHandling::Approximate);
      }
      return;
    }
    // A hidden mask layer clips like a visible one when hidden layers are
    // published (Animate's default). Otherwise the render has no matte.
    bool hiddenMask = true, shownChild = false;
    for (int index : columns)
      hiddenMask &= !xsheet->getColumn(index)->isPreviewVisible();
    for (int index : childColumns)
      shownChild |= xsheet->getColumn(index)->isPreviewVisible();
    if (hiddenMask && shownChild) {
      const QString message =
          "The mask layer is hidden and hidden layers are not published; the "
          "render has no matte for its visible masked layers.";
      if (!m_options.bestEffort) throw Flash::Error(layerId + ": " + message);
      degrade({"unmapped-mask-compositing", layerId, message},
              IssueHandling::Approximate);
    }
    // Rows where the mask has no cell but something is masked: Flash shows
    // the masked layers unclipped there.
    const int maskColumn = columns.size() == 1 ? columns.front() : -1;
    if (maskColumn >= 0) {
      auto *object =
          xsheet->getStageObject(TStageObjectId::ColumnId(maskColumn));
      // Locked columns refuse cells: lock again afterwards.
      const bool wasLocked = xsheet->getColumn(maskColumn)->isLocked();
      xsheet->getColumn(maskColumn)->lock(false);
      const int frames = std::max(1, source.frameCount);
      bool previous    = false;
      for (int row = 0; row < frames; ++row) {
        bool unmasked = xsheet->getCell(row, maskColumn).isEmpty();
        if (unmasked) {
          unmasked = false;
          for (int child : childColumns)
            if (!xsheet->getCell(row, child).isEmpty()) unmasked = true;
        }
        if (unmasked) {
          cell(xsheet, row, maskColumn, unmaskedDrawing(), 0);
          // Untransformed from where the blank span starts, so the drawing
          // keeps covering the stage whatever the mask artwork was doing.
          if (!previous)
            for (const auto channel : placementChannels)
              heldKey(object, channel, row,
                      object->getParam(channel)->getDefaultValue());
        }
        previous = unmasked;
      }
      xsheet->getColumn(maskColumn)->lock(wasLocked);
    }
    auto *dag          = xsheet->getFxDag();
    const auto compose = [&](const QVector<int> &inputs,
                             const QString &label) -> TFx * {
      // A column's filters and color effect come before the mask.
      TFx *composite = columnOutput(xsheet, inputs.front());
      for (int slot = 1; slot < inputs.size(); ++slot) {
        TFx *front = columnOutput(xsheet, inputs[slot]);
        TFxP over(TFx::create("overFx"));
        if (!front || !composite || !over || !over->connect("Source1", front) ||
            !over->connect("Source2", composite))
          throw Flash::Error(layerId + ": Cannot compose linked mask artwork.");
        dag->assignUniqueId(over.getPointer());
        over->setName(label.toStdWString());
        dag->getInternalFxs()->addFx(over.getPointer());
        composite = over.getPointer();
      }
      return composite;
    };
    TFx *sourceFx =
        compose(childColumns, layer.name + " [Flash masked artwork]");
    TFx *matteFx = compose(columns, layer.name + " [Flash mask artwork]");
    TFxP clip(TFx::create("inFx"));
    if (!clip || !sourceFx || !matteFx || !clip->connect("Source", sourceFx) ||
        !clip->connect("Matte", matteFx))
      throw Flash::Error(layerId + ": Cannot connect native filled-mask FX.");
    dag->assignUniqueId(clip.getPointer());
    clip->setName((layer.name + " [Flash mask]").toStdWString());
    dag->getInternalFxs()->addFx(clip.getPointer());
    for (int nativeColumn : childColumns)
      dag->removeFromXsheet(columnOutput(xsheet, nativeColumn));
    for (int nativeColumn : columns)
      dag->removeFromXsheet(columnOutput(xsheet, nativeColumn));
    dag->addToXsheet(clip.getPointer());
    // The editor viewer traverses columns in stacking order, not this FX
    // graph. One mask column right below the masked columns, clipping exactly
    // that many columns, matches the In FX. Masked layers follow the mask
    // layer, so their bands are contiguous: the mask column takes the lower
    // quarter of the lowest band and the masked columns move up half a band,
    // above unordered columns (stacking order 0) and below the next layer.
    // Quarter positions survive the TNZ round trip exactly.
    if (maskColumn < 0 || masked.back() - mask != masked.size()) {
      if (!m_options.bestEffort)
        throw Flash::Error(layerId + ": Masked layers do not follow the mask.");
      degrade({"unmapped-mask-compositing", layerId,
               "The render clips the masked layers; the editor viewer shows "
               "them unclipped."},
              IssueHandling::Approximate);
      return;
    }
    const double maskOrder =
        double(source.layers.size() - 1 - masked.back()) + 0.25;
    auto *maskObject =
        xsheet->getStageObject(TStageObjectId::ColumnId(maskColumn));
    maskObject->getParam(TStageObject::T_SO)->clearKeyframes();
    heldKey(maskObject, TStageObject::T_SO, 0, maskOrder);
    for (int child : childColumns) {
      TDoubleParam *order =
          xsheet->getStageObject(TStageObjectId::ColumnId(child))
              ->getParam(TStageObject::T_SO);
      for (int key = 0; key < order->getKeyframeCount(); ++key) {
        TDoubleKeyframe keyframe = order->getKeyframe(key);
        keyframe.m_value += 0.5;
        order->setKeyframe(key, keyframe);
      }
      if (!order->hasKeyframes())
        order->setDefaultValue(order->getDefaultValue() + 0.5);
    }
    auto *native = xsheet->getColumn(maskColumn);
    native->setIsMask(true);
    native->setMaskScope(childColumns.size());
    for (const auto &site : m_stageBindings) {
      if (site.sheet != xsheet) continue;
      const bool isMask  = site.column == maskColumn;
      const bool isChild = childColumns.contains(site.column);
      if (!isMask && !isChild) continue;
      auto binding = m_result.nativeSourceMap.at(site.bindingIndex).toObject();
      binding["nativeStackingOrder"] =
          isMask ? maskOrder
                 : binding.value("nativeStackingOrder").toDouble() + 0.5;
      m_result.nativeSourceMap.replace(site.bindingIndex, binding);
    }
  }

  bool m_columnOpacity = false;

  // Flash symbol alpha becomes native column opacity, which the viewer,
  // playback and (with the scene's column-filter render option) the final
  // render apply to the whole composited instance, as Flash does. Alpha that
  // changes between keyframes is keyed on the column's opacity curve (an
  // ordinary curve in the Function Editor): linear where the keyframe tweens
  // to the next one, held otherwise.
  template <class Keys>
  void applyAlpha(TXsheet *xsheet, int column, const Keys &keys) {
    bool varies = false, translucent = false;
    for (const auto &key : keys) {
      translucent |= key.alpha < 1.0;
      varies |= key.alpha != keys.front().alpha;
    }
    if (!translucent) return;
    const auto unit = [](double alpha) {
      return std::max(0.0, std::min(1.0, alpha));
    };
    auto *native    = xsheet->getColumn(column);
    m_columnOpacity = true;
    if (!varies) {
      native->setOpacity(UCHAR(std::lround(unit(keys.front().alpha) * 255)));
      return;
    }
    TDoubleParam *curve = native->getOpacityCurve();
    for (const auto &key : keys) {
      TDoubleKeyframe keyframe(key.frame, unit(key.alpha));
      keyframe.m_type =
          key.tweens ? TDoubleKeyframe::Linear : TDoubleKeyframe::Constant;
      curve->setKeyframe(keyframe);
    }
  }

  // Color effects, filters and blend modes ------------------------------------
  // A symbol instance's filters and color effect become FX after its column
  // (FlashNative::EffectChain): the filters in Flash's order, then the color
  // transform, which Flash applies to the filtered image. Their parameters
  // are keyed like the stage keys: held from each keyframe and, across a
  // classic tween, interpolated with its ease (one native segment when the
  // values follow the ease, a key on every in-between otherwise, e.g. a
  // filter length on a column whose scale tweens). A blend mode becomes an
  // Flare layer blending FX whose Xsheet port composes everything below
  // the column in its Xsheet.
  struct EffectKey {
    int frame   = 0;
    bool tweens = false;  // a classic tween to the next key of the column
    IncomingTween ease;
    QString sourceId;
    bool graphic = false;  // Flash publishes no filters or blend here
    Flash::ColorTransform color;
    QVector<Flash::Filter> filters;  // the effective ones
    QString blendMode = "normal";
  };
  // The FX whose output stands for a column in its Xsheet (its last effect
  // FX, or the column FX).
  QMap<std::pair<const TXsheet *, int>, TFx *> m_columnOutputs;
  // Document Xsheets with a blended column: their stage background becomes
  // a column, the backdrop Flash blends with.
  QSet<const TXsheet *> m_blendedSheets;

  TFx *columnOutput(TXsheet *xsheet, int column) {
    return m_columnOutputs.value({xsheet, column},
                                 xsheet->getColumn(column)->getFx());
  }

  static EffectKey effectKey(const Flash::Element &element, int frame,
                             bool tweens, const IncomingTween &ease) {
    EffectKey key;
    key.frame    = frame;
    key.tweens   = tweens;
    key.ease     = ease;
    key.sourceId = element.sourceId;
    if (element.kind != Flash::Element::Kind::Symbol) return key;
    key.graphic   = Flash::symbolInstanceType(element) == "graphic";
    key.color     = Flash::colorEffect(element).transform;
    key.blendMode = Flash::blendMode(element);
    for (const auto &filter : Flash::elementFilters(element).filters)
      if (filter.effective()) key.filters.push_back(filter);
    return key;
  }

  // Keys `param` with valueAt(frame) at the column's keys and across their
  // tweens (see above).
  void keyEffectParam(TDoubleParam *param, const QVector<EffectKey> &keys,
                      const std::function<double(int)> &valueAt) {
    const int count = keys.size();
    QVector<double> values;
    for (const auto &key : keys) values.push_back(valueAt(key.frame));
    auto spans = [&](int k) {
      return keys[k].tweens && k + 1 < count &&
             keys[k + 1].frame > keys[k].frame + 1;
    };
    auto same = [](double a, double b) {
      return std::abs(a - b) <= 1e-9 * (1 + std::abs(a));
    };
    bool varies = false;
    for (int k = 0; k < count && !varies; ++k) {
      varies |= !same(values[k], values.front());
      if (spans(k))
        for (int frame = keys[k].frame + 1; frame < keys[k + 1].frame; ++frame)
          varies |= !same(valueAt(frame), values[k]);
    }
    param->setDefaultValue(values.front());
    if (!varies) return;
    QVector<TDoubleKeyframe> frames;
    for (int k = 0; k < count; ++k) {
      TDoubleKeyframe key(keys[k].frame, values[k]);
      key.m_type = keys[k].tweens && k + 1 < count ? TDoubleKeyframe::Linear
                                                   : TDoubleKeyframe::Constant;
      frames.push_back(key);
    }
    QVector<TDoubleKeyframe> between;
    for (int k = 0; k + 1 < count; ++k) {
      if (!spans(k)) continue;
      const int first = keys[k].frame, last = keys[k + 1].frame;
      QVector<double> samples;
      bool eased = true;
      for (int frame = first + 1; frame < last; ++frame) {
        const double value = valueAt(frame);
        const double expected =
            values[k] + (values[k + 1] - values[k]) *
                            easeProgress(keys[k].ease, double(frame - first) /
                                                           (last - first));
        eased &= std::abs(value - expected) <= 1e-9 * (1 + std::abs(value));
        samples.push_back(value);
      }
      if (eased && easeKeys(frames[k], frames[k + 1], keys[k].ease)) continue;
      frames[k].m_type = TDoubleKeyframe::Linear;
      for (int frame = first + 1; frame < last; ++frame) {
        TDoubleKeyframe key(frame, samples[frame - first - 1]);
        key.m_type = TDoubleKeyframe::Linear;
        between.push_back(key);
      }
    }
    // The stock color channel unit has no extension, which debug builds
    // assert on when a key gets it: key color channels without the measure.
    const std::string measure = param->getMeasureName();
    if (measure == "colorChannel") param->setMeasureName("");
    for (const auto &key : frames + between) param->setKeyframe(key);
    param->setMeasureName(measure);
  }

  // Builds the effect FX of a column. Returns true when the color FX carries
  // the column's alpha (with filters, Flash fades the filtered image; an
  // alpha offset needs the color FX too), so it needs no column opacity.
  bool applyEffects(TXsheet *xsheet, int column, const QVector<EffectKey> &keys,
                    const Flash::Timeline &source, int layerIndex,
                    const QString &label) {
    if (keys.isEmpty() || m_maskArtwork) return false;
    // The kinds of filters the column's FX chain has: those of its first key
    // with filters. Keys without filters tween from or to them unfiltered.
    QVector<Flash::Filter> kinds;
    for (const auto &key : keys)
      if (!key.graphic && !key.filters.isEmpty()) {
        kinds = key.filters;
        break;
      }
    if (!kinds.isEmpty() && m_container != "document")
      sink()({"unmapped-filter", keys.front().sourceId,
              "Filter lengths follow this instance inside its symbol; where "
              "the symbol itself is placed scaled, they scale with it (Flash "
              "keeps them in stage pixels)."});
    auto *object = xsheet->getStageObject(TStageObjectId::ColumnId(column));
    auto scaleAt = [object](int frame) {
      const double scale =
          std::sqrt(std::abs(object->getPlacement(frame).det()));
      return scale > 1e-9 ? scale : 1.0;
    };
    QVector<EffectState> states;
    bool filtered = false, translucent = false, alphaOffset = false;
    for (const auto &key : keys) {
      EffectState state;
      state.color        = key.color;
      state.filters      = neutralFilters(kinds);
      const auto filters = key.graphic ? QVector<Flash::Filter>() : key.filters;
      if (!filters.isEmpty() && Flash::sameFilterKinds(filters, kinds))
        state.filters = filters;
      else if (!filters.isEmpty()) {
        for (int slot = 0; slot < kinds.size() && slot < filters.size(); ++slot)
          if (filters[slot].kind == kinds[slot].kind &&
              filters[slot].gradient.size() == kinds[slot].gradient.size())
            state.filters[slot] = filters[slot];
        sink()({"unmapped-filter", key.sourceId,
                "The filters differ in kind from the column's first filtered "
                "keyframe; only matching filters are kept."});
      }
      filtered |= !filters.isEmpty();
      translucent |= key.color.multiplier[3] != 1;
      // An offset, or a multiplier above 1, changes the alpha of translucent
      // pixels in ways column opacity cannot.
      alphaOffset |= key.color.offset[3] != 0 || key.color.multiplier[3] > 1;
      states.push_back(state);
    }
    const bool alphaInColor = (filtered && translucent) || alphaOffset;
    // What the column shows at a frame: a tween interpolates its keys.
    auto stateAt = [&](int frame) {
      int k = 0;
      while (k + 1 < keys.size() && keys[k + 1].frame <= frame) ++k;
      EffectState state = states[k];
      if (keys[k].tweens && k + 1 < keys.size() && frame > keys[k].frame) {
        const double t =
            double(frame - keys[k].frame) / (keys[k + 1].frame - keys[k].frame);
        const double s = easeProgress(keys[k].ease, t);
        state.color =
            Flash::interpolate(states[k].color, states[k + 1].color, s);
        for (int slot = 0; slot < state.filters.size(); ++slot)
          state.filters[slot] = Flash::interpolate(
              states[k].filters[slot], states[k + 1].filters[slot], s);
      }
      state.scale = scaleAt(frame);
      return state;
    };

    QVector<EffectIssue> issues;
    EffectChain chain(states, Stage::inch / m_options.dpi, alphaInColor);
    TFx *input  = xsheet->getColumn(column)->getFx();
    TFx *output = chain.build(xsheet, input, label, issues);
    for (const auto &issue : issues)
      sink()({issue.code, issue.sourceId, issue.message});
    for (const auto &param : chain.params())
      keyEffectParam(param.param, keys,
                     [&](int frame) { return param.value(stateAt(frame)); });
    bool alphaHandled = false;
    for (const auto &issue : issues)
      alphaHandled |= issue.code == "unmapped-color-effect";
    alphaHandled = alphaInColor && !alphaHandled;
    if (alphaOffset)
      for (const auto &key : keys)
        if (key.color.offset[3] != 0 || key.color.multiplier[3] > 1) {
          sink()({"unmapped-color-effect", key.sourceId,
                  "The alpha offset or gain acts on the composited instance "
                  "(as when Flash caches it as a bitmap), not on each of its "
                  "shapes."});
          break;
        }

    // The blend mode of the column's keys (Flash publishes none on graphic
    // instances; the preflight reports modes without a native FX).
    QString mode;
    for (const auto &key : keys)
      if (!key.graphic && !Flash::blendFxType(key.blendMode).isEmpty()) {
        mode = key.blendMode;
        break;
      }
    if (!mode.isEmpty()) {
      for (const auto &key : keys)
        if (!key.graphic && key.blendMode != mode) {
          sink()({"unmapped-blend-mode", key.sourceId,
                  "The column blends in " + mode +
                      " mode on all its frames; "
                      "this keyframe's " +
                      key.blendMode + " mode is not kept."});
          break;
        }
      bool masked = false;
      for (int ancestor = source.layers[layerIndex].parent, steps = 0;
           ancestor >= 0 && ancestor < source.layers.size() &&
           ++steps <= source.layers.size();
           ancestor = source.layers[ancestor].parent)
        masked |= source.layers[ancestor].type == "mask";
      if (masked)
        sink()({"unmapped-blend-mode", keys.front().sourceId,
                "Masked artwork is drawn in normal mode: the mask composes "
                "it apart from what lies below."});
      else if (xsheet->getColumn(column)->isPreviewVisible()) {
        if (m_container == "graphic" || m_container == "group")
          sink()({"unmapped-blend-mode", keys.front().sourceId,
                  "Inside a graphic symbol or group the instance blends "
                  "with the symbol's own artwork below it; Flash flattens "
                  "the symbol and blends with everything below it."});
        output = blendFx(xsheet, output, mode, label);
        if (m_container == "document") m_blendedSheets.insert(xsheet);
      }
    }
    if (output != input) m_columnOutputs[{xsheet, column}] = output;
    return alphaHandled;
  }

  // The stage background as the bottom column of a document Xsheet with
  // blended columns: Flash blends with the stage color where nothing else
  // lies below. A color card, below every other column.
  void stageBackground(TXsheet *xsheet, int frameCount) {
    TFxP fx(TFx::create("colorCardFx"));
    auto *color =
        fx ? dynamic_cast<TPixelParam *>(fx->getParams()->getParam("color"))
           : nullptr;
    if (!color) throw Flash::Error("Cannot create the stage background.");
    color->setDefaultValue(m_result.scene->getProperties()->getBgColor());
    auto *native            = new TXshZeraryFxColumn(1);
    const TXshCell exposure = native->getCell(0);
    native->clearCells(0, 1);
    native->getZeraryColumnFx()->setZeraryFx(fx.getPointer());
    const int index = xsheet->getColumnCount();
    xsheet->insertColumn(index, native);
    xsheet->getFxDag()->assignUniqueId(fx.getPointer());
    auto *object = xsheet->getStageObject(TStageObjectId::ColumnId(index));
    object->setName("Flash stage background");
    heldKey(object, TStageObject::T_SO, 0, -1.0);
    ++m_result.columns;
    textFxCells(xsheet, index, 0, std::max(1, frameCount), exposure);
    xsheet->getColumn(index)->lock(true);
  }

  // The motion guide of a layer: its nearest guide ancestor, or -1.
  static int motionGuide(const Flash::Timeline &timeline, int layerIndex) {
    int steps = 0;
    for (int ancestor = timeline.layers[layerIndex].parent;
         ancestor >= 0 && ancestor < timeline.layers.size() &&
         ++steps <= timeline.layers.size();
         ancestor = timeline.layers[ancestor].parent) {
      if (timeline.layers[ancestor].type == "guide") return ancestor;
      if (timeline.layers[ancestor].type == "mask") break;
    }
    return -1;
  }

  // Guided classic tweens follow the guide's stroke (see guideRoute()). Each
  // guided chain has a column of its own (splitGuidedChains()), which becomes
  // a native motion path: the guide stroke is the column's spline and T_Path
  // is keyed with the tween's ease, so path and timing stay editable. A key
  // whose pivot lies beyond an end of the path, as Flash allows, joins it by
  // a straight piece (T_Path keyed frame by frame on that tween, which leaves
  // the key for the path's end at once). A key off the path elsewhere cannot
  // share one native path with the in-betweens: that column keys Flash's
  // positions frame by frame instead (reported).
  void followGuide(TXsheet *xsheet, const Flash::Timeline &source,
                   int layerIndex, int guideIndex,
                   const Flash::PartTrackPlan &plan,
                   const QVector<int> &trackColumns,
                   const Flash::Affine &origin, const QString &layerId) {
    const auto &layer = source.layers[layerIndex];
    const double unit = Stage::inch / m_options.dpi;  // stage units / pixel
    constexpr double tolerance = 0.25;  // source pixels, a few twips
    for (int track = 0; track < plan.trackCount; ++track) {
      const int column = trackColumns.value(track, -1);
      if (column < 0) continue;
      QVector<int> ordinals;  // the chain's keyframes
      for (int ordinal = 0; ordinal < layer.frames.size(); ++ordinal)
        if (plan.frameTracks[ordinal].contains(track))
          ordinals.push_back(ordinal);
      if (ordinals.size() < 2 ||
          !classicMotionTween(layer.frames[ordinals.front()]) ||
          ordinals[1] != ordinals[0] + 1)
        continue;
      bool symbols = true;
      for (int ordinal : ordinals)
        symbols &= layer.frames[ordinal]
                       .elements[plan.frameTracks[ordinal].indexOf(track)]
                       .kind == Flash::Element::Kind::Symbol;
      if (!symbols) continue;
      const int first        = layer.frames[ordinals.front()].index;
      const int last         = layer.frames[ordinals.back()].index;
      const QString sourceId = layerId + "/frame" + QString::number(first);
      const Flash::Frame *guideFrame = nullptr;
      for (const auto &frame : source.layers[guideIndex].frames)
        if (frame.index <= first && first < frame.index + frame.duration)
          guideFrame = &frame;
      if (!guideFrame) continue;
      const auto route = guideRoute(*guideFrame, origin);
      if (!route) {
        sink()({"unmapped-motion-guide", sourceId,
                "The guide's strokes do not form one path; the tween moves "
                "in a straight line."});
        continue;
      }
      if (route->isEmpty()) continue;  // nothing to follow
      if (last >= guideFrame->index + guideFrame->duration)
        sink()({"unmapped-motion-guide", sourceId,
                "The guide changes during the tween; its first path is "
                "followed throughout."});
      auto *object = xsheet->getStageObject(TStageObjectId::ColumnId(column));
      TDoubleParam *xs = object->getParam(TStageObject::T_X);
      TDoubleParam *ys = object->getParam(TStageObject::T_Y);
      struct Key {
        int frame;
        TPointD pivot;                 // stage units
        double arc = 0, distance = 0;  // on the guide; source pixels
      };
      QVector<Key> keys;
      QVector<IncomingTween> eases;
      bool orient = false;
      for (int ordinal : ordinals) {
        const auto &frame = layer.frames[ordinal];
        keys.push_back({frame.index, TPointD(xs->getValue(frame.index),
                                             ys->getValue(frame.index)) *
                                         Stage::inch});
        if (ordinal != ordinals.back()) {
          eases.push_back(tweenEase(frame));
          orient |= frame.properties.value("motionTweenOrientToPath",
                                           "false") == "true";
        }
      }
      std::vector<TThickPoint> points = routePoints(*route, m_options.dpi);
      std::unique_ptr<TStroke> guide;
      auto project = [&] {
        guide = std::make_unique<TStroke>(points);
        for (auto &key : keys) {
          double w = 0, distance2 = 0;
          guide->getNearestW(key.pivot, w, distance2, false);
          key.arc      = guide->getLength(0.0, w);
          key.distance = std::sqrt(distance2) / unit;
        }
      };
      project();
      // Run the path the way the tween travels.
      if (keys.front().arc > keys.back().arc) {
        std::reverse(points.begin(), points.end());
        project();
      }
      const double length = guide->getLength();
      if (!(length > 0)) continue;
      const bool joinStart = keys.front().distance > tolerance &&
                             keys.front().arc <= 1e-9 * length;
      const bool joinEnd = keys.back().distance > tolerance &&
                           keys.back().arc >= (1 - 1e-9) * length;
      double offPath = 0;
      for (int index = 0; index < keys.size(); ++index)
        if (!(index == 0 && joinStart) &&
            !(index + 1 == keys.size() && joinEnd))
          offPath = std::max(offPath, keys[index].distance);
      // Flash's guide arc of the pivot at a frame of tween `index`.
      auto arcAt = [&](int index, int frame) {
        const double t = double(frame - keys[index].frame) /
                         (keys[index + 1].frame - keys[index].frame);
        return keys[index].arc + (keys[index + 1].arc - keys[index].arc) *
                                     easeProgress(eases[index], t);
      };
      auto linear = [](TDoubleParam *param, int frame, double value) {
        TDoubleKeyframe key = param->isKeyframe(frame)
                                  ? param->getKeyframeAt(frame)
                                  : TDoubleKeyframe(frame, value);
        key.m_value         = value;
        key.m_type          = TDoubleKeyframe::Linear;
        param->setKeyframe(key);
      };
      if (offPath > tolerance) {
        // Exact positions without a native path: key every in-between.
        for (int index = 0; index + 1 < keys.size(); ++index) {
          linear(xs, keys[index].frame, xs->getValue(keys[index].frame));
          linear(ys, keys[index].frame, ys->getValue(keys[index].frame));
          for (int frame = keys[index].frame + 1; frame < keys[index + 1].frame;
               ++frame) {
            const TPointD at = guide->getPointAtLength(arcAt(index, frame));
            linear(xs, frame, at.x / Stage::inch);
            linear(ys, frame, at.y / Stage::inch);
          }
        }
        sink()({"unmapped-motion-guide", sourceId,
                QString("A key's transformation point is %1 px off the guide "
                        "path: Flash's positions along the path are keyed "
                        "frame by frame instead of a native motion path.")
                    .arg(offPath, 0, 'f', 2)});
        continue;
      }
      // The native path: the guide, joined to keys beyond its ends.
      std::vector<TThickPoint> native = points;
      if (joinStart) {
        const TThickPoint pivot(keys.front().pivot, 0);
        native.insert(native.begin(), {pivot, 0.5 * (pivot + native.front())});
      }
      if (joinEnd) {
        const TThickPoint pivot(keys.back().pivot, 0);
        native.push_back(0.5 * (native.back() + pivot));
        native.push_back(pivot);
      }
      auto stroke        = std::make_unique<TStroke>(native);
      const double lead  = joinStart ? stroke->getLengthAtControlPoint(2) : 0;
      const double total = stroke->getLength();
      auto percent       = [&](double guideArc) {
        return 100 * (lead + guideArc) / total;
      };
      TDoubleParam *path = object->getParam(TStageObject::T_Path);
      for (int index = 0; index < keys.size(); ++index) {
        TDoubleKeyframe key(keys[index].frame,
                            index == 0 && joinStart ? 0
                            : index + 1 == keys.size() && joinEnd
                                ? 100
                                : percent(keys[index].arc));
        key.m_type     = TDoubleKeyframe::Constant;
        key.m_prevType = TDoubleKeyframe::Constant;
        path->setKeyframe(key);
      }
      for (int index = 0; index + 1 < keys.size(); ++index) {
        TDoubleKeyframe start = path->getKeyframeAt(keys[index].frame);
        TDoubleKeyframe end   = path->getKeyframeAt(keys[index + 1].frame);
        const bool joined =
            (index == 0 && joinStart) || (index + 2 == keys.size() && joinEnd);
        if (!joined && easeKeys(start, end, eases[index])) {
          path->setKeyframe(start);
          path->setKeyframe(end);
          continue;
        }
        linear(path, keys[index].frame, start.m_value);
        for (int frame = keys[index].frame + 1; frame < keys[index + 1].frame;
             ++frame)
          linear(path, frame, percent(arcAt(index, frame)));
      }
      if (orient) {
        // Orient to path: the native path adds the path direction to the
        // rotation. The keys keep their own angle; in between, the angle
        // turns with the path relative to the eased key directions.
        auto direction = [&](int frame) {
          return atan(stroke->getSpeedAtLength(path->getValue(frame) * 0.01 *
                                               total)) *
                 180 / std::acos(-1.0);
        };
        TDoubleParam *angle = object->getParam(TStageObject::T_Angle);
        std::vector<std::pair<int, double>> values;
        for (int index = 0; index + 1 < keys.size(); ++index) {
          const double from = direction(keys[index].frame);
          const double turn =
              std::remainder(direction(keys[index + 1].frame) - from, 360.0);
          for (int frame = keys[index].frame; frame < keys[index + 1].frame;
               ++frame) {
            const double t = double(frame - keys[index].frame) /
                             (keys[index + 1].frame - keys[index].frame);
            values.push_back(
                {frame, angle->getValue(frame) -
                            (from + turn * easeProgress(eases[index], t))});
          }
        }
        values.push_back(
            {keys.back().frame, angle->getValue(keys.back().frame) -
                                    direction(keys.back().frame)});
        for (const auto &value : values)
          linear(angle, value.first, value.second);
        sink()({"unmapped-motion-guide", sourceId,
                "Orient to path is approximated: between keys the instance "
                "turns with the path's direction."});
      }
      object->setStatus(orient ? TStageObject::PATH_AIM : TStageObject::PATH);
      object->getSpline()->setStroke(stroke.release());
    }
  }

  // A motion-object tween's curves, or null without readable motion data.
  static std::shared_ptr<const Flash::MotionObject> motionCurves(
      const Flash::Frame &frame) {
    if (!Flash::motionObjectFrame(frame)) return nullptr;
    auto motion = Flash::motionObject(frame);
    return motion ? std::make_shared<const Flash::MotionObject>(*motion)
                  : nullptr;
  }

  // The span a motion-object tween keys on its element's columns. Groups
  // (whose members carry their matrix) and degenerate matrices hold.
  std::optional<MotionSpan> motionSpanFor(
      const std::shared_ptr<const Flash::MotionObject> &motion,
      const Flash::Element &element, const Flash::Frame &frame) const {
    if (!motion || element.kind == Flash::Element::Kind::Group ||
        std::abs(element.matrix.determinant()) < 1e-12)
      return std::nullopt;
    MotionSpan span;
    span.motion   = motion;
    span.base     = element.matrix;
    span.pivot    = element.hasTransformationPoint ? element.transformationPoint
                                                   : Flash::Point{};
    span.relative = Flash::motionRelative(*motion, element.matrix);
    span.start    = frame.index;
    span.duration = frame.duration;
    span.frameRate = m_info.frameRate;
    return span;
  }

  // Layer parenting (Animate 2019+). A keyframe's parentLayerIndex names its
  // layer's parent from that keyframe on: the layer whose layerRiggingIndex
  // it equals (Animate 20 saves). Child keyframes store their artwork as it
  // stands on the stage; between them the child follows the parent layer's
  // symbol instance P: world(t) = P(t) * inverse(P(k)) * C(k) while the
  // child holds keyframe k, interpolated in the parent's space across a
  // classic tween (and a motion tween's curves apply in that space too).
  // A native column parent keys exactly that: the child keeps local keys
  // inverse(P(k)) * C(k) (applyRig()).
  struct RigKeying {
    int column = -1, layer = -1, ordinal = 0, frame = 0, duration = 1;
    Flash::Affine matrix;  // the placement given to place()
    Flash::Point pivot;
    bool tweened = false;  // the key ends a classic tween (`tween`)
    IncomingTween tween;
    std::optional<MotionSpan> motion;
    Flash::Affine post;         // see MotionSpan
    bool parentObject = false;  // the keyframe's only element, a symbol
  };

  static QMap<int, int> rigLayers(const Flash::Timeline &source) {
    QMap<int, int> result;  // layerRiggingIndex -> layer
    for (int index = 0; index < source.layers.size(); ++index) {
      bool ok = false;
      const int rig =
          source.layers[index].properties.value("layerRiggingIndex").toInt(&ok);
      if (ok && source.layers[index].type == "normal")
        result.insert(rig, index);
    }
    return result;
  }

  static int rigParent(const Flash::Timeline &source,
                       const QMap<int, int> &rigs, int layer,
                       const Flash::Frame &frame) {
    bool ok       = false;
    const int rig = frame.properties.value("parentLayerIndex").toInt(&ok);
    if (!ok || source.layers[layer].type != "normal") return -1;
    const int parent = rigs.value(rig, -1);
    return parent == layer ? -1 : parent;
  }

  void applyRig(TXsheet *xsheet, const Flash::Timeline &source,
                const QMap<int, int> &rigs, const QVector<RigKeying> &keyings,
                const Flash::Affine &origin, const QString &sourceMember) {
    const double dpi = m_options.dpi;
    QMap<int, QVector<int>> byLayer, byColumn;
    QVector<int> columnOrder;
    for (int index = 0; index < keyings.size(); ++index) {
      byLayer[keyings[index].layer].push_back(index);
      if (!byColumn.contains(keyings[index].column))
        columnOrder.push_back(keyings[index].column);
      byColumn[keyings[index].column].push_back(index);
    }
    auto parentOf = [&](const RigKeying &keying) {
      return rigParent(source, rigs, keying.layer,
                       source.layers[keying.layer].frames[keying.ordinal]);
    };
    // The native column of a layer's parent instance at a frame, or -1.
    auto objectColumn = [&](int layer, int frame) {
      for (int index : byLayer.value(layer)) {
        const auto &keying = keyings[index];
        if (keying.parentObject && frame >= keying.frame &&
            frame < keying.frame + keying.duration)
          return keying.column;
      }
      return -1;
    };
    auto stage = [&](int column) {
      return xsheet->getStageObject(TStageObjectId::ColumnId(column));
    };
    // Parents first, so every parent is final when its children follow it.
    QMap<int, int> depths;
    std::function<int(int, int)> depth = [&](int layer, int guard) {
      if (depths.contains(layer)) return depths.value(layer);
      int result = 0;
      if (guard <= source.layers.size())
        for (int index : byLayer.value(layer)) {
          const int parent = parentOf(keyings[index]);
          if (parent >= 0)
            result = std::max(result, depth(parent, guard + 1) + 1);
        }
      depths.insert(layer, result);
      return result;
    };
    QMap<int, int> columnDepth;
    for (int column : columnOrder)
      columnDepth.insert(column,
                         depth(keyings[byColumn[column].front()].layer, 0));
    std::stable_sort(columnOrder.begin(), columnOrder.end(), [&](int a, int b) {
      return columnDepth.value(a) < columnDepth.value(b);
    });
    const int frames = std::max(1, source.frameCount);
    for (int column : columnOrder) {
      const auto &records = byColumn[column];
      const int layer     = keyings[records.front()].layer;
      QVector<int> parents;
      bool parented = false;
      for (int index : records) {
        parents.push_back(parentOf(keyings[index]));
        parented |= parents.back() >= 0;
      }
      if (!parented) continue;
      const QString sourceId = sourceMember + "/layer" + QString::number(layer);
      if (motionGuide(source, layer) >= 0) {
        degrade({"unmapped-layer-parenting", sourceId,
                 "A layer on a motion guide follows its guide; its layer "
                 "parent is not applied."},
                IssueHandling::Approximate);
        continue;
      }
      TStageObject *child = stage(column);
      // One parent instance column throughout: a native column parent.
      int parentColumn = -1;
      bool fixed       = true;
      for (int r = 0; r < records.size() && fixed; ++r) {
        const auto &keying = keyings[records[r]];
        fixed              = parents[r] >= 0 && parents[r] == parents.front();
        for (int frame = keying.frame;
             fixed && frame < keying.frame + keying.duration; ++frame) {
          const int at = objectColumn(parents[r], frame);
          fixed        = at >= 0 && at != column &&
                  (parentColumn < 0 || at == parentColumn);
          parentColumn = at;
        }
      }
      if (fixed)
        for (TStageObjectId up = stage(parentColumn)->getParent();
             fixed && up.isColumn();
             up = xsheet->getStageObject(up)->getParent())
          fixed = up != TStageObjectId::ColumnId(column);
      if (fixed)
        for (int index : records)
          fixed =
              fixed &&
              std::abs(
                  sourceAffine(
                      stage(parentColumn)->getPlacement(keyings[index].frame),
                      dpi)
                      .determinant()) > 1e-12;
      if (fixed) {
        TStageObject *parent = stage(parentColumn);
        // Stacking orders add up along native parents: keep the column's.
        QVector<double> order;
        for (int frame = 0; frame < frames; ++frame)
          order.push_back(child->getSO(frame));
        child->setParent(TStageObjectId::ColumnId(parentColumn));
        child->getParam(TStageObject::T_SO)->clearKeyframes();
        double last = 0;
        for (int frame = 0; frame < frames; ++frame) {
          const double value = order[frame] - parent->getSO(frame);
          if (frame == 0 || value != last)
            heldKey(child, TStageObject::T_SO, frame, value);
          last = value;
        }
        for (const auto channel : placementChannels)
          child->getParam(channel)->clearKeyframes();
        double angle = 0;
        int previous = -1;
        for (int index : records) {
          const auto &keying = keyings[index];
          const Flash::Affine back =
              inverse(sourceAffine(parent->getPlacement(keying.frame), dpi));
          IncomingTween tween = keying.tween;
          tween.frame         = previous;
          tween.angle         = angle;
          angle = place(child, keying.frame, back * keying.matrix, dpi,
                        keying.pivot,
                        keying.tweened && previous >= 0 ? &tween : nullptr);
          if (keying.motion)
            motionSpan(child, *keying.motion, back * origin, keying.post,
                       keying.pivot, dpi, angle);
          previous = keying.frame;
        }
        continue;
      }
      // Otherwise the column stays unparented, keyed on every frame from its
      // parent's motion (each keyframe's parent, where it has one instance).
      QVector<std::pair<int, Flash::Affine>> shown;
      QVector<Flash::Point> pivots;
      for (int r = 0; r < records.size(); ++r) {
        const auto &keying = keyings[records[r]];
        const int start =
            parents[r] >= 0 ? objectColumn(parents[r], keying.frame) : -1;
        Flash::Affine back;
        bool follows = false;
        if (start >= 0) {
          const auto at =
              sourceAffine(stage(start)->getPlacement(keying.frame), dpi);
          follows = std::abs(at.determinant()) > 1e-12;
          if (follows) back = inverse(at);
        }
        for (int frame = keying.frame; frame < keying.frame + keying.duration;
             ++frame) {
          Flash::Affine placed = sourceAffine(child->getPlacement(frame), dpi);
          const int at         = follows ? objectColumn(parents[r], frame) : -1;
          if (at >= 0)
            placed = sourceAffine(stage(at)->getPlacement(frame), dpi) * back *
                     placed;
          shown.push_back({frame, placed});
          pivots.push_back(keying.pivot);
        }
      }
      for (const auto channel : placementChannels)
        child->getParam(channel)->clearKeyframes();
      for (int index = 0; index < shown.size(); ++index)
        place(child, shown[index].first, shown[index].second, dpi,
              pivots[index], nullptr);
      degrade({"unmapped-layer-parenting", sourceId,
               "The layer's parent changes between its keyframes (or has no "
               "single symbol instance on some of them), which a native "
               "column parent cannot follow: the column is keyed on every "
               "frame, following its parent; its own tweens move in stage "
               "space between their keys."},
              IssueHandling::Approximate);
    }
  }

  // Animate's camera (2017+): a layerType="camera" layer holding one
  // __Camera__ instance per keyframe, whose matrix places the stage-sized
  // camera frame (centered on its origin) on the stage. The view is
  // T(stage center) * inverse(camera), and the camera tweens like any
  // instance. Flare's camera is a stage object seen through the inverse
  // of its placement, so keying it as a column with the same matrix gives
  // the same view: pan, zoom (camera scale 0.5 is 200%), rotation, and their
  // classic and motion tweens, editable as camera keys.
  bool cameraLayer(TXsheet *xsheet, const Flash::Timeline &source,
                   int layerIndex, const Flash::Affine &origin,
                   const QString &layerId, bool mapped) {
    const auto &layer = source.layers[layerIndex];
    // Animate ignores a camera the timeline disables.
    if (source.properties.value("cameraLayerEnabled") == "false") return false;
    if (mapped) {
      degrade({"unmapped-camera", layerId,
               "Only the first camera layer drives the camera."},
              IssueHandling::Approximate);
      return false;
    }
    if (xsheet != m_result.scene->getTopXsheet()) {
      degrade({"unmapped-camera", layerId,
               m_container == "document"
                   ? "The camera of a scene played from a sub-xsheet (a "
                     "document with several scenes or frame scripts) is not "
                     "mapped; the scene is shown without it."
                   : "A camera layer inside a symbol is not mapped."},
              IssueHandling::Approximate);
      return false;
    }
    TStageObject *camera = xsheet->getStageObject(
        xsheet->getStageObjectTree()->getCurrentCameraId());
    auto cameraOf = [](const Flash::Frame &frame) {
      const Flash::Element *found = nullptr;
      for (const auto &element : frame.elements)
        if (element.kind == Flash::Element::Kind::Symbol) {
          if (element.properties.value("libraryItemName") == "__Camera__")
            return &element;
          if (!found) found = &element;
        }
      return found;
    };
    Flash::Point pivot;
    for (const auto &frame : layer.frames)
      if (const auto *element = cameraOf(frame)) {
        if (element->hasTransformationPoint)
          pivot = element->transformationPoint;
        break;
      }
    Flash::Affine home;  // the default camera frames the stage
    home.tx = m_info.width / 2.0;
    home.ty = m_info.height / 2.0;
    IncomingTween incoming;
    bool hasIncoming = false;
    for (int ordinal = 0; ordinal < layer.frames.size(); ++ordinal) {
      const auto &frame     = layer.frames[ordinal];
      const QString frameId = layerId + "/frame" + QString::number(frame.index);
      const auto *element   = cameraOf(frame);
      if (!element) {
        place(camera, frame.index, origin * home, m_options.dpi, pivot,
              nullptr);
        hasIncoming = false;
        continue;
      }
      for (const auto &extra : element->extras)
        if (extra.tag == "color" || extra.tag == "filters")
          degrade({"unmapped-camera", element->sourceId + "/" + extra.tag,
                   "Camera color effects and filters are not mapped."},
                  IssueHandling::Approximate);
      const double angle =
          place(camera, frame.index, origin * element->matrix, m_options.dpi,
                pivot, hasIncoming ? &incoming : nullptr);
      if (const auto span = motionSpanFor(motionCurves(frame), *element, frame))
        if (!motionSpan(camera, *span, origin, Flash::Affine{}, pivot,
                        m_options.dpi, angle))
          degrade({"unmapped-tween", frameId,
                   "The camera's motion tween has no finite pose; it holds "
                   "its first frame."},
                  IssueHandling::Approximate);
      hasIncoming = classicMotionTween(frame) &&
                    ordinal + 1 < layer.frames.size() &&
                    cameraOf(layer.frames[ordinal + 1]);
      if (hasIncoming) incoming = withRotation(tweenEase(frame), frame, angle);
    }
    return true;
  }

  // Layers attached to the camera (attachedToCamera, on the layer or on a
  // folder holding it) stay put in the view: native children of the camera.
  void attachToCamera(TXsheet *xsheet, const Flash::Timeline &source,
                      const QVector<QVector<int>> &sourceColumns) {
    const TStageObjectId camera =
        xsheet->getStageObjectTree()->getCurrentCameraId();
    for (int layerIndex = 0; layerIndex < source.layers.size(); ++layerIndex) {
      bool attached = false;
      int steps     = 0;
      for (int index = layerIndex; index >= 0 && index < source.layers.size() &&
                                   ++steps <= source.layers.size();
           index = source.layers[index].parent)
        attached |=
            source.layers[index].properties.value("attachedToCamera") == "true";
      if (!attached) continue;
      for (int column : sourceColumns[layerIndex]) {
        auto *object = xsheet->getStageObject(TStageObjectId::ColumnId(column));
        // A layer parent keeps the column (applyRig()).
        if (!object->getParent().isColumn()) object->setParent(camera);
      }
    }
  }

  bool locked(const Flash::Timeline &timeline, int layerIndex) {
    int steps = 0;
    while (layerIndex >= 0) {
      if (layerIndex >= timeline.layers.size() ||
          ++steps > timeline.layers.size())
        throw Flash::Error("Invalid layer parent hierarchy.");
      const auto &layer = timeline.layers[layerIndex];
      if (layer.locked) return true;
      layerIndex = layer.parent;
    }
    return false;
  }

  void timeline(TXsheet *xsheet, const Flash::Timeline &source,
                Flash::Affine origin, const QString &sourceMember,
                bool linearRootClock, bool syntheticGroup) {
    Compatibility::timeline(source, sourceMember, sink());
    QVector<QVector<int>> sourceColumns(source.layers.size());
    // A native mask column clips the columns that follow it in stacking
    // order (its mask scope), as a Flash mask clips all its masked layers;
    // those keep their own columns (see linkMask()). A mask layer that needs
    // several native columns (several elements at once) is built in one
    // sub-xsheet column, so the mask is their union (see groupMask()).
    QSet<int> groupedMasks;
    for (int mask = 0; mask < source.layers.size(); ++mask)
      if (source.layers[mask].type == "mask" &&
          !maskedLayers(source, mask).isEmpty() &&
          Flash::planLayerTracks(source.layers[mask], true).trackCount > 1)
        groupedMasks.insert(mask);
    // Layer parenting and the camera (applyRig(), cameraLayer()).
    const QMap<int, int> rigs = rigLayers(source);
    QVector<RigKeying> rigKeyings;
    bool cameraMapped = false;
    // XFL lists front layers first; native column stacking is back to front.
    for (int layerIndex = source.layers.size() - 1; layerIndex >= 0;
         --layerIndex) {
      // Masked layers come after their mask, so they are already built.
      if (groupedMasks.contains(layerIndex)) {
        groupMask(xsheet, source, layerIndex, origin, sourceMember,
                  linearRootClock, sourceColumns);
        continue;
      }
      const auto &layer = source.layers[layerIndex];
      const auto layerId =
          sourceMember + "/layer" + QString::number(layerIndex);
      m_omit = false;
      Compatibility::layer(layer, layerId, sink());
      if (m_omit) continue;
      if (layer.type == "camera") {
        cameraMapped |= cameraLayer(xsheet, source, layerIndex, origin, layerId,
                                    cameraMapped);
        continue;
      }
      // A mask layer that clips other layers draws its artwork as the
      // opaque, fill-only silhouette Flash clips with (maskSilhouette()).
      DepthScope maskArtwork(
          m_maskArtwork,
          layer.type == "mask" && !maskedLayers(source, layerIndex).isEmpty());
      // Direct children of a mask layer, back to front.
      QVector<int> linkedChildren;
      if (layer.type == "mask")
        for (int index = source.layers.size() - 1; index > layerIndex; --index)
          if (source.layers[index].parent == layerIndex)
            linkedChildren.push_back(index);
      // A named instance can keep its editable column when its position in
      // the source display list changes. Mask FX inputs currently compose in
      // fixed column order, so those layers retain slot-based destinations.
      const bool preserveSlotOrder =
          layer.type == "mask" ||
          (layer.parent >= 0 && layer.parent < source.layers.size() &&
           source.layers[layer.parent].type == "mask");
      // Guided tween chains get columns of their own (followGuide()).
      const int guideIndex = motionGuide(source, layerIndex);
      const Flash::PartTrackPlan partPlan =
          guideIndex >= 0
              ? splitGuidedChains(
                    Flash::planLayerTracks(layer, preserveSlotOrder), layer)
              : Flash::planLayerTracks(layer, preserveSlotOrder);
      // A source layer is an authored timeline track. When it has only one
      // element at any keyframe, its native column can retain that layer's
      // continuity without claiming that successive anonymous elements are
      // the same placed object. A slot in a multi-element layer is only a
      // display-list destination unless a unique instance name identifies it.
      // Group-member sheets are generated by this adapter, not source layers.
      const bool singleElementLayer = std::all_of(
          layer.frames.cbegin(), layer.frames.cend(),
          [](const Flash::Frame &frame) { return frame.elements.size() <= 1; });
      QVector<int> trackColumns(partPlan.trackCount, -1);
      // A classic motion tween interpolates one placed object to the next
      // keyframe of the same track. Those tracks pivot on the source
      // transformation point (see place()).
      QVector<bool> tweenTracks(partPlan.trackCount, false);
      QVector<Flash::Point> trackPivots(partPlan.trackCount);
      auto tweensTo = [&](int frameOrdinal, int track) {
        if (!classicMotionTween(layer.frames[frameOrdinal]) ||
            frameOrdinal + 1 >= layer.frames.size())
          return false;
        const auto &next = partPlan.frameTracks[frameOrdinal + 1];
        return std::find(next.cbegin(), next.cend(), track) != next.cend();
      };
      for (int ordinal = 0; ordinal < layer.frames.size(); ++ordinal)
        for (int slot = 0; slot < layer.frames[ordinal].elements.size(); ++slot)
          if (tweensTo(ordinal, partPlan.frameTracks[ordinal][slot]) ||
              Flash::motionObjectFrame(layer.frames[ordinal]))
            tweenTracks[partPlan.frameTracks[ordinal][slot]] = true;
      QVector<bool> hasPivot(partPlan.trackCount, false);
      for (int ordinal = 0; ordinal < layer.frames.size(); ++ordinal)
        for (int slot = 0; slot < layer.frames[ordinal].elements.size();
             ++slot) {
          const auto &element = layer.frames[ordinal].elements[slot];
          const int track     = partPlan.frameTracks[ordinal][slot];
          if (tweenTracks[track] && !hasPivot[track] &&
              element.kind == Flash::Element::Kind::Symbol &&
              element.hasTransformationPoint) {
            trackPivots[track] = element.transformationPoint;
            hasPivot[track]    = true;
          }
        }
      QVector<IncomingTween> incoming(partPlan.trackCount);
      // Symbol alpha (color alphaMultiplier) per native column and keyframe,
      // with whether that keyframe tweens to the next one.
      struct AlphaKey {
        int frame;
        double alpha;
        bool tweens;
      };
      QMap<int, QVector<AlphaKey>> columnAlpha;
      // Filters, color effects and blend modes per native column and key.
      QMap<int, QVector<EffectKey>> columnEffects;
      QVector<bool> hasIncoming(partPlan.trackCount, false);
      QVector<int> columns;
      QMap<int, TXshSimpleLevel *> drawings;
      // Text drawn by Text FX columns keeps its outlines in hidden columns.
      QMap<int, TXshSimpleLevel *> textDrawings;
      QVector<int> textOutlineColumns(partPlan.trackCount, -1);
      QMap<int, QVector<TextFxColumn>> textFxColumns;
      std::vector<FlashFrameLabelData> frameLabels;
      for (int frameOrdinal = 0; frameOrdinal < layer.frames.size();
           ++frameOrdinal) {
        const auto &frame = layer.frames[frameOrdinal];
        check();
        Compatibility::frame(frame,
                             layerId + "/frame" + QString::number(frame.index),
                             sink(), m_container);
        // A motion-object tween's curves key the rest of its span.
        const auto motionCurve = motionCurves(frame);
        scriptNote(xsheet, layer, frame);
        {
          const QString frameId =
              layerId + "/frame" + QString::number(frame.index);
          // Movie clip sounds are placed where clips play them
          // (flattenClipSounds()).
          if (const auto sound =
                  Compatibility::sound(frame, frameId, m_container, sink()))
            if (m_container == "document")
              queueSound(*sound, source, layerIndex, layerId, frame, frameId);
        }
        if (Compatibility::supportedPassiveFrameLabel(frame)) {
          FlashFrameLabelData label;
          label.sourceTimelineId = source.sourceId;
          label.sourceLayerId    = layer.sourceId;
          label.sourceFrameId    = frame.sourceId;
          label.sourceFrameXmlId = frame.source.sourceId;
          label.sourceLayerName  = layer.name;
          label.name             = frame.properties.value("name");
          label.type             = frame.properties.value("labelType", "name");
          label.sourceFrameIndex = frame.index;
          label.row              = frame.index;
          frameLabels.push_back(std::move(label));
        }
        for (int slot = 0; slot < frame.elements.size(); ++slot) {
          // Mask artwork shapes are drawn as their mask silhouette.
          Flash::Element silhouette;
          const bool maskShape =
              m_maskArtwork && frame.elements[slot].shape &&
              (frame.elements[slot].kind == Flash::Element::Kind::Shape ||
               frame.elements[slot].kind == Flash::Element::Kind::Oval ||
               frame.elements[slot].kind == Flash::Element::Kind::Rectangle);
          if (maskShape) {
            silhouette       = frame.elements[slot];
            silhouette.shape = std::make_shared<Flash::Shape>(
                maskSilhouette(*silhouette.shape));
          }
          const auto &element = maskShape ? silhouette : frame.elements[slot];
          const int track     = partPlan.frameTracks[frameOrdinal][slot];
          try {
            m_omit = false;
            Compatibility::element(element, frame, m_options.dpi, sink(),
                                   [this] { check(); });
            if (m_omit) {
              hasIncoming[track] = false;
              continue;
            }
            TXshLevel *level = nullptr;
            Flash::GraphicTiming timing{frame.index, frame.duration, 0,
                                        Flash::PlaybackMode::SingleFrame};
            std::optional<Flash::MovieClipTiming> movieClipClock;
            int frameCount = 1;
            // Animate stores DOMGroup members in the group's parent space;
            // the group matrix only repeats their transforms (see group()).
            Flash::Affine artworkMatrix =
                element.kind == Flash::Element::Kind::Group
                    ? Flash::Affine{}
                    : origin * element.matrix;
            Flash::Point pivot =
                tweenTracks[track] ? trackPivots[track] : Flash::Point{};
            QString contentDigest;
            bool graphicInstance = false;
            bool clockReplayed =
                false;  // a graphic placement shows a clock replay
            bool queuedVector = false;
            bool textOutline  = false;  // hidden outlines of Text FX text
            QJsonArray textFx;
            // Set when the authored playback could not be kept exactly.
            bool timingApproximated = false;
            if (element.kind == Flash::Element::Kind::Shape ||
                element.kind == Flash::Element::Kind::Oval ||
                element.kind == Flash::Element::Kind::Rectangle) {
              auto *drawingLevel  = drawings.value(track, nullptr);
              const bool newLevel = !drawingLevel;
              if (newLevel) {
                drawingLevel = drawing(layer.name);
                drawings.insert(track, drawingLevel);
              }
              // Converted later, concurrently with the other queued shapes.
              // Registering the paints now keeps palette order identical to
              // a sequential conversion and leaves the palette read-only.
              registerVectorPaints(element, drawingLevel->getPalette(),
                                   Stage::inch / m_options.dpi, fillBitmaps());
              // Shape tween in-betweens are queued first: the keyframe's job
              // must stay last for its source binding below.
              const int inBetweens =
                  queueShapeTween(layer, frameOrdinal, slot, drawingLevel);
              VectorJob job;
              job.element = element;
              job.level   = drawingLevel;
              job.frame   = frame.index + 1;
              if (!m_maskCopyNames.isEmpty())
                job.maskCopyOf = m_maskCopyNames.back();
              m_vectorJobs.push_back(std::move(job));
              queuedVector      = true;
              level             = drawingLevel;
              timing.firstFrame = frame.index;
              frameCount        = frame.index + 1;
              if (inBetweens) {
                // Each in-between row exposes its own drawing (frame row + 1).
                timing.mode = Flash::PlaybackMode::PlayOnce;
                frameCount  = frame.index + 1 + inBetweens;
              }
            } else if (element.kind == Flash::Element::Kind::Text) {
              // Editable Text FX columns draw the text; its glyph outlines
              // stay in a hidden reference column of their own (see
              // textFxAvailable()). Without the Text FX the outlines are the
              // visible text, in the track column. Mask artwork keeps the
              // outlines, opaque, as its silhouette.
              textOutline         = textFxAvailable() && !m_maskArtwork;
              auto &outlineLevels = textOutline ? textDrawings : drawings;
              // Glyph outlines are laid out here, on the building thread:
              // fonts are not used from the concurrent vector workers.
              auto *drawingLevel = outlineLevels.value(track, nullptr);
              TPaletteP palette =
                  drawingLevel ? drawingLevel->getPalette() : new TPalette();
              TVectorImageP image;
              QStringList approximations;
              VectorFrameAudit audit;
              QVector<TextFxItem> fxItems;
              try {
                image = textImage(element, palette.getPointer(),
                                  Stage::inch / m_options.dpi, &approximations,
                                  &audit);
                if (textOutline)
                  fxItems = textFxItems(element, Stage::inch / m_options.dpi,
                                        &approximations);
                if (m_maskArtwork)
                  for (int index = 1; index < palette->getStyleCount();
                       ++index) {
                    TColorStyle *style = palette->getStyle(index);
                    TPixel32 color     = style->getMainColor();
                    if (color.m == 255) continue;
                    color.m = 255;
                    style->setMainColor(color);
                  }
              } catch (const Flash::Error &error) {
                omitAfterFailure("vector-conversion-failed", element.sourceId,
                                 error);
                hasIncoming[track] = false;
                continue;
              }
              approximations.removeDuplicates();
              for (const auto &note : approximations)
                degrade({"unmapped-text-attribute", element.sourceId, note},
                        IssueHandling::Approximate);
              if (!drawingLevel) {
                drawingLevel = drawing(layer.name);
                drawingLevel->setPalette(palette.getPointer());
                outlineLevels.insert(track, drawingLevel);
              }
              if (textOutline) {
                // One column per FX, placed like the element (scaled to
                // the exact font size) and stacked with its layer.
                const bool editorOnlyLayer =
                    layer.type == "guide" ||
                    (layer.type == "mask" && linkedChildren.isEmpty());
                const bool shown =
                    visible(source, layerIndex, editorOnlyLayer) &&
                    !(editorOnlyLayer && !linearRootClock &&
                      !m_options.showSymbolGuides);
                const QString role     = layer.type == "guide" ? "[guide] "
                                         : editorOnlyLayer     ? "[mask] "
                                                               : "";
                const QString partName = partPlan.trackNames[track];
                const double stackingOrder =
                    double(source.layers.size() - 1 - layerIndex) +
                    double(slot + 1) / (double(m_options.maxColumns) + 1.0);
                auto &reusable = textFxColumns[track];
                const int last = frame.index + frame.duration - 1;
                for (int index = 0; index < fxItems.size(); ++index) {
                  const auto &item  = fxItems[index];
                  const QString key = textFxKey(item);
                  int fxColumn      = -1;
                  for (auto &candidate : reusable)
                    if (candidate.key == key &&
                        candidate.lastRow < frame.index) {
                      fxColumn          = candidate.column;
                      candidate.lastRow = last;
                      textFxCells(xsheet, fxColumn, frame.index,
                                  frame.duration);
                      break;
                    }
                  if (fxColumn < 0) {
                    fxColumn = textFxColumn(
                        xsheet,
                        role + layer.name + " / " +
                            (partName.isEmpty()
                                 ? QString::number(columns.size() + 1)
                             : fxItems.size() > 1
                                 ? partName + "." + QString::number(index + 1)
                                 : partName),
                        shown, editorOnlyLayer, item, frame.index,
                        frame.duration);
                    reusable.push_back({key, fxColumn, last});
                    columns.push_back(fxColumn);
                  }
                  Flash::Affine size;
                  size.a = size.d = item.columnScale;
                  const Flash::Point fxPivot{pivot.x / item.columnScale,
                                             pivot.y / item.columnScale};
                  const double fxAngle =
                      place(xsheet, fxColumn, frame.index, artworkMatrix * size,
                            m_options.dpi, fxPivot,
                            hasIncoming[track] ? &incoming[track] : nullptr);
                  // A motion tween moves each Text FX column with the text.
                  const auto fxSpan =
                      motionSpanFor(motionCurve, element, frame);
                  const Flash::Affine fxPost =
                      fxSpan ? inverse(origin * element.matrix) *
                                   (artworkMatrix * size)
                             : Flash::Affine{};
                  if (fxSpan)
                    motionSpan(xsheet->getStageObject(
                                   TStageObjectId::ColumnId(fxColumn)),
                               *fxSpan, origin, fxPost, fxPivot, m_options.dpi,
                               fxAngle);
                  if (!rigs.isEmpty()) {
                    RigKeying keying;
                    keying.column   = fxColumn;
                    keying.layer    = layerIndex;
                    keying.ordinal  = frameOrdinal;
                    keying.frame    = frame.index;
                    keying.duration = frame.duration;
                    keying.matrix   = artworkMatrix * size;
                    keying.pivot    = fxPivot;
                    keying.tweened  = hasIncoming[track];
                    keying.tween    = incoming[track];
                    keying.motion   = fxSpan;
                    keying.post     = fxPost;
                    rigKeyings.push_back(keying);
                  }
                  heldKey(xsheet->getStageObject(
                              TStageObjectId::ColumnId(fxColumn)),
                          TStageObject::T_SO, frame.index, stackingOrder);
                  textFx.append(
                      QJsonObject{{"nativeColumn", fxColumn},
                                  {"nativeFxId",
                                   QString::fromStdWString(
                                       textFxOf(xsheet, fxColumn)->getFxId())},
                                  {"nativeColumnScale", item.columnScale},
                                  {"sourceFirstRun", item.firstRun},
                                  {"sourceLastRun", item.lastRun},
                                  {"text", item.text}});
                }
              }
              image->setPalette(drawingLevel->getPalette());
              drawingLevel->setFrame(TFrameId(frame.index + 1),
                                     TImageP(image.getPointer()));
              m_paintWitnesses += audit.paint.size();
              m_result.vectorFrameAudits[drawingLevel].insert(frame.index + 1,
                                                              audit);
              contentDigest = drawingContentDigest(TImageP(image.getPointer()));
              level         = drawingLevel;
              timing.firstFrame = frame.index;
              frameCount        = frame.index + 1;
            } else if (element.kind == Flash::Element::Kind::Symbol) {
              const auto name = element.properties.value("libraryItemName");
              if (!m_info.symbols.contains(name)) {
                sink()({"missing-symbol", element.sourceId,
                        "Missing source symbol: " + name});
                hasIncoming[track] = false;
                continue;
              }
              // XFL omits symbolType on movie-clip instances.
              const bool movieClip =
                  Flash::symbolInstanceType(element) == "movie clip";
              try {
                if (movieClip)
                  movieClipClock =
                      Flash::movieClipTiming(layer, frameOrdinal, slot);
                else
                  timing = Flash::instanceTiming(element, frame);
              } catch (const Flash::Error &) {
                check();
                if (!m_options.bestEffort) throw;
                // Already reported as approximated by Compatibility::element.
                if (movieClip)
                  movieClipClock =
                      Flash::MovieClipTiming{frame.index, frame.duration};
                else
                  timing = fallbackGraphicTiming(element, frame);
                timingApproximated = true;
              }
              auto *childLevel = symbol(name);
              level            = childLevel;
              frameCount       = childLevel->getFrameCount();
              graphicInstance  = !movieClip;
              // A first frame past the symbol's end plays as in Animate:
              // loops wrap around the symbol (Animate's Sync option itself
              // saves such unwrapped first frames), the other modes rest on
              // its last frame (Animate's published SWFs).
              if (graphicInstance && !timing.lastFrame &&
                  timing.firstFrame >= std::max(1, frameCount))
                timing.firstFrame = Flash::wrappedFirstFrame(
                    timing.firstFrame, timing.mode, frameCount);
              if (graphicInstance && m_options.bestEffort) {
                try {
                  timing.sourceFrameAt(frame.index, frameCount);
                } catch (const Flash::Error &error) {
                  check();
                  // A first/last frame past the symbol's end: loops wrap
                  // around the symbol, other modes rest on its last frame.
                  const int count = std::max(1, frameCount);
                  timing.lastFrame.reset();
                  timing.firstFrame =
                      timing.mode == Flash::PlaybackMode::Loop ||
                              timing.mode == Flash::PlaybackMode::LoopReverse
                          ? timing.firstFrame % count
                          : std::min(timing.firstFrame, count - 1);
                  degrade({"invalid-symbol-pose", element.sourceId,
                           QString::fromUtf8(error.what())},
                          IssueHandling::Approximate);
                  timingApproximated = true;
                }
              }
              // Movie clips play on their own clocks (clipClock()); nested
              // clips seen through a graphic keep theirs (placeClockRuns()).
              const QString laneKey = QString::number(quintptr(xsheet), 16) +
                                      "/" + QString::number(track);
              if (movieClip) {
                const ClipClock clock = clipClock(childLevel, name);
                if (!m_maskArtwork) {
                  level      = clock.level;
                  frameCount = clock.level->getFrameCount();
                }
                if (clock.playhead) movieClipClock->playhead = clock.playhead;
                if (clock.animated)
                  recordClockRun(
                      xsheet,
                      {movieClipClock->clockOrigin(),
                       frame.index + frame.duration, clock.prefix, clock.period,
                       clockLane(laneKey + "/" + name + QChar(0x1f) +
                                 element.properties.value("name")),
                       element.sourceId});
              } else if (auto *replay = placeClockRuns(
                             xsheet, childLevel, name, laneKey, frame.index,
                             frame.duration,
                             [&](int row) {
                               return *timing.sourceFrameAt(frame.index + row,
                                                            frameCount);
                             },
                             element.sourceId)) {
                // The replay plays the graphic's frames tick by tick.
                level           = replay;
                frameCount      = replay->getFrameCount();
                timing          = {frame.index, frame.duration, 0,
                                   Flash::PlaybackMode::PlayOnce};
                graphicInstance = false;
                clockReplayed   = true;
              }
            } else if (element.kind == Flash::Element::Kind::Group) {
              level       = group(element, frame.duration, origin);
              timing.mode = Flash::PlaybackMode::PlayOnce;
              frameCount  = frame.duration;
              // Group rows follow the parent's: nested clip runs carry over.
              projectClockRuns(
                  xsheet,
                  QString::number(quintptr(xsheet), 16) + "/" +
                      QString::number(track),
                  frame.index,
                  m_clockRuns.value(level->getChildLevel()->getXsheet()));
            } else if (element.kind == Flash::Element::Kind::Bitmap) {
              const auto name = element.properties.value("libraryItemName");
              TXshSimpleLevel *rasterLevel = nullptr;
              try {
                rasterLevel = bitmap(name);
              } catch (const Flash::Error &error) {
                omitAfterFailure("unmapped-bitmap-media", element.sourceId,
                                 error);
                hasIncoming[track] = false;
                continue;
              }
              level = rasterLevel;
              if (m_maskArtwork)
                degradeMask(element.sourceId,
                            "A bitmap in mask artwork clips by its pixels' "
                            "alpha; Flash masks with its whole rectangle.");
              contentDigest = drawingContentDigest(
                  rasterLevel->getFrame(TFrameId(1), false));
              const TDimension size =
                  rasterLevel->getProperties()->getImageRes();
              Flash::Affine center;
              center.tx     = size.lx / 2.0;
              center.ty     = size.ly / 2.0;
              artworkMatrix = artworkMatrix * center;
              pivot         = {pivot.x - center.tx, pivot.y - center.ty};
            } else if (element.kind == Flash::Element::Kind::Video) {
              const auto name = element.properties.value("libraryItemName");
              const VideoItem &item = video(name, element.sourceId);
              // An omission, or the approximation of an unplayed sound track.
              for (CompatibilityIssue issue : item.issues) {
                issue.sourceId = element.sourceId;
                sink()(issue);
              }
              if (!item.level) {
                hasIncoming[track] = false;
                continue;
              }
              if (item.sound)
                queueVideoSound(item.sound, name, source, layerIndex, layerId,
                                frameOrdinal, element.sourceId);
              level      = item.level;
              frameCount = item.frames;
              if (m_maskArtwork)
                degradeMask(element.sourceId,
                            "A video in mask artwork clips by its frames' "
                            "alpha; Flash masks with its whole frame.");
              // Video frames follow the timeline; the last one is held.
              const int start = videoClockStart(layer, frameOrdinal, name);
              timing          = {frame.index, frame.duration,
                                 std::min(frame.index - start, item.frames - 1),
                                 Flash::PlaybackMode::PlayOnce};
              // The instance box (frameRight/frameBottom, in twips) shows the
              // whole video; the raster level is centered on its image.
              bool wide = false, tall = false;
              const double boxWidth =
                  element.properties.value("frameRight").toDouble(&wide) / 20;
              const double boxHeight =
                  element.properties.value("frameBottom").toDouble(&tall) / 20;
              Flash::Affine box, center;
              if (wide && boxWidth > 0) box.a = boxWidth / item.width;
              if (tall && boxHeight > 0) box.d = boxHeight / item.height;
              center.tx     = item.width / 2.0;
              center.ty     = item.height / 2.0;
              artworkMatrix = artworkMatrix * box * center;
              pivot         = {pivot.x / box.a - center.tx,
                               pivot.y / box.d - center.ty};
            } else
              throw Flash::Error("Unmapped element type: " + element.tag);
            // Columns are created once the element has native content, so an
            // omitted element never leaves an empty track column behind.
            // Outlines of Text FX text have a hidden column of their own.
            QVector<int> &trackSlots =
                textOutline ? textOutlineColumns : trackColumns;
            if (trackSlots[track] < 0) {
              const bool editorOnlyLayer =
                  layer.type == "guide" ||
                  (layer.type == "mask" && linkedChildren.isEmpty());
              const QString partName = partPlan.trackNames[track];
              // Name guides so they are easy to find and show again.
              const QString role     = textOutline ? "[text outlines] "
                                       : layer.type == "guide" ? "[guide] "
                                       : editorOnlyLayer       ? "[mask] "
                                                               : "";
              const int nativeColumn = column(
                  xsheet,
                  role + layer.name + " / " +
                      (partName.isEmpty() ? QString::number(columns.size() + 1)
                                          : partName),
                  // Flash shows a symbol's guides only while editing that
                  // symbol, never in its placed instances: keep them, hidden.
                  visible(source, layerIndex, editorOnlyLayer) &&
                      !(editorOnlyLayer && !linearRootClock &&
                        !m_options.showSymbolGuides),
                  // The camstand viewer never draws Text FX, so the outlines
                  // show the text there; renders use the Text FX only.
                  editorOnlyLayer || textOutline);
              trackSlots[track] = nativeColumn;
              columns.push_back(nativeColumn);
            }
            const int nativeColumn = trackSlots[track];
            const bool tweensOn    = tweensTo(frameOrdinal, track);
            const EffectKey effects =
                effectKey(element, frame.index, tweensOn,
                          tweensOn ? tweenEase(frame) : IncomingTween());
            columnEffects[nativeColumn].push_back(effects);
            columnAlpha[nativeColumn].push_back(
                {frame.index, effects.color.multiplier[3], tweensOn});
            // A motion tween's color curves, frame by frame (motionColor()).
            if (motionCurve && motionCurve->hasColor &&
                element.kind == Flash::Element::Kind::Symbol)
              for (int offset = 0; offset < frame.duration; ++offset) {
                EffectKey key = effects;
                key.frame     = frame.index + offset;
                key.tweens    = offset + 1 < frame.duration;
                key.ease      = IncomingTween();
                key.color =
                    *Flash::motionColor(*motionCurve, offset, m_info.frameRate);
                auto &effectKeys = columnEffects[nativeColumn];
                auto &alphaKeys  = columnAlpha[nativeColumn];
                if (offset == 0) {
                  effectKeys.back()       = key;
                  alphaKeys.back().alpha  = key.color.multiplier[3];
                  alphaKeys.back().tweens = key.tweens;
                } else {
                  effectKeys.push_back(key);
                  alphaKeys.push_back(
                      {key.frame, key.color.multiplier[3], key.tweens});
                }
              }
            const double angle = place(
                xsheet, nativeColumn, frame.index, artworkMatrix, m_options.dpi,
                pivot, hasIncoming[track] ? &incoming[track] : nullptr);
            {
              // Motion-object tweens key their span (motionSpan()); the
              // artwork's own placement in the element follows it (`post`).
              const auto span = motionSpanFor(motionCurve, element, frame);
              const Flash::Affine post =
                  span ? inverse(origin * element.matrix) * artworkMatrix
                       : Flash::Affine{};
              if (span &&
                  !motionSpan(xsheet->getStageObject(
                                  TStageObjectId::ColumnId(nativeColumn)),
                              *span, origin, post, pivot, m_options.dpi, angle))
                degrade({"unmapped-tween", element.sourceId,
                         "The motion tween has no finite pose; the instance "
                         "holds its first frame."},
                        IssueHandling::Approximate);
              if (!rigs.isEmpty()) {
                RigKeying keying;
                keying.column   = nativeColumn;
                keying.layer    = layerIndex;
                keying.ordinal  = frameOrdinal;
                keying.frame    = frame.index;
                keying.duration = frame.duration;
                keying.matrix   = artworkMatrix;
                keying.pivot    = pivot;
                keying.tweened  = hasIncoming[track];
                keying.tween    = incoming[track];
                keying.motion   = span;
                keying.post     = post;
                keying.parentObject =
                    frame.elements.size() == 1 &&
                    element.kind == Flash::Element::Kind::Symbol;
                rigKeyings.push_back(keying);
              }
            }
            hasIncoming[track] = tweensOn;
            if (hasIncoming[track])
              incoming[track] = withRotation(reportedEase(frame), frame, angle);
            // Native stacking order is animatable without changing geometric
            // depth or the part's column. Keep every source layer in its own
            // unit-wide band, then order its elements by XML display slot.
            const double stackingOrder =
                double(source.layers.size() - 1 - layerIndex) +
                double(slot + 1) / (double(m_options.maxColumns) + 1.0);
            heldKey(
                xsheet->getStageObject(TStageObjectId::ColumnId(nativeColumn)),
                TStageObject::T_SO, frame.index, stackingOrder);
            for (int row = frame.index; row < frame.index + frame.duration;
                 ++row)
              cell(xsheet, row, nativeColumn, level,
                   movieClipClock
                       ? *movieClipClock->sourceFrameAt(row, frameCount)
                       : *timing.sourceFrameAt(row, frameCount));
            QString nativeInstanceId;
            if (graphicInstance) {
              FlashInstanceData instance;
              instance.sourceId = element.sourceId;
              instance.libraryName =
                  element.properties.value("libraryItemName");
              instance.level            = level;
              instance.timing           = timing;
              instance.symbolFrameCount = frameCount;
              instance.sourcePivot      = element.transformationPoint;
              instance.hasSourcePivot   = element.hasTransformationPoint;
              xsheet->getColumn(nativeColumn)
                  ->getLevelColumn()
                  ->addFlashInstance(instance);
              nativeInstanceId = xsheet->getColumn(nativeColumn)
                                     ->getLevelColumn()
                                     ->flashInstances()
                                     .back()
                                     .nativeInstanceId;
            }
            const QString kind =
                element.kind == Flash::Element::Kind::Shape       ? "shape"
                : element.kind == Flash::Element::Kind::Oval      ? "oval"
                : element.kind == Flash::Element::Kind::Rectangle ? "rectangle"
                : element.kind == Flash::Element::Kind::Symbol    ? "symbol"
                : element.kind == Flash::Element::Kind::Group     ? "group"
                : element.kind == Flash::Element::Kind::Bitmap    ? "bitmap"
                : element.kind == Flash::Element::Kind::Text      ? "text"
                                                                  : "video";
            QString nativePlacementId;
            if (element.kind != Flash::Element::Kind::Symbol ||
                movieClipClock || clockReplayed) {
              FlashPlacementData placement;
              placement.sourceId = element.sourceId;
              placement.kind     = kind;
              placement.level    = level;
              placement.start    = frame.index;
              placement.duration = frame.duration;
              nativePlacementId  = xsheet->getColumn(nativeColumn)
                                      ->getLevelColumn()
                                      ->addFlashPlacement(placement);
            }
            QJsonObject binding{
                {"kind", kind},
                {"sourceId", element.sourceId},
                {"sourceFrameXmlId", frame.source.sourceId},
                {"sourceElementXmlId", element.source.sourceId},
                {"sourceTimelineId", sourceMember},
                {"sourceLayerId",
                 layer.sourceId.isEmpty() ? layerId : layer.sourceId},
                {"sourceLayerIndex", layerIndex},
                {"sourceFrameIndex", frame.index},
                {"sourceElementSlot", slot},
                {"nativePartTrack", track},
                {"nativeStackingOrder", stackingOrder},
                {"nativeColumn", nativeColumn},
                {"nativeFirstRow", frame.index},
                {"nativeRowCount", frame.duration},
                {"nativeLevelName", QString::fromStdWString(level->getName())},
                {"nativeLevelId", level->flashNativeId()}};
            binding["sourceTrackRole"] = syntheticGroup ? "group-member"
                                         : !partPlan.trackNames[track].isEmpty()
                                             ? "named-instance"
                                         : singleElementLayer ? "source-layer"
                                                              : "display-slot";
            if (!partPlan.trackNames[track].isEmpty())
              binding["sourcePartName"] = partPlan.trackNames[track];
            if (element.kind == Flash::Element::Kind::Group) {
              // The group column is untransformed: its members carry the
              // group matrix (see artworkMatrix above).
              binding["sourceTransform"]   = QJsonArray{1, 0, 0, 1, 0, 0};
              binding["sourceGroupMatrix"] = QJsonArray{
                  element.matrix.a, element.matrix.b,  element.matrix.c,
                  element.matrix.d, element.matrix.tx, element.matrix.ty};
            } else
              binding["sourceTransform"] = QJsonArray{
                  element.matrix.a, element.matrix.b,  element.matrix.c,
                  element.matrix.d, element.matrix.tx, element.matrix.ty};
            if (element.kind == Flash::Element::Kind::Shape ||
                element.kind == Flash::Element::Kind::Oval ||
                element.kind == Flash::Element::Kind::Rectangle) {
              binding["nativeDrawingFrame"] = frame.index + 1;
            } else if (element.kind == Flash::Element::Kind::Bitmap) {
              binding["nativeDrawingFrame"]  = 1;
              binding["nativeContentSha256"] = contentDigest;
            } else if (element.kind == Flash::Element::Kind::Text) {
              binding["nativeDrawingFrame"]  = frame.index + 1;
              binding["nativeContentSha256"] = contentDigest;
              // nativeColumn holds the glyph outlines; with Text FX columns
              // it is a hidden reference.
              if (textOutline) {
                binding["nativeOutlineHidden"] = true;
                binding["nativeTextFx"]        = textFx;
              }
            }
            if (element.kind == Flash::Element::Kind::Video)
              binding["nativeVideoFirstFrame"] = timing.firstFrame + 1;
            if (!nativeInstanceId.isEmpty())
              binding["nativeInstanceId"] = nativeInstanceId;
            if (timingApproximated) binding["nativeTimingApproximated"] = true;
            if (!nativePlacementId.isEmpty())
              binding["nativePlacementId"] = nativePlacementId;
            // A mask copy repeats artwork the painted symbol binds.
            if (!m_maskCopyNames.isEmpty()) continue;
            sourceBinding(binding);
            if (queuedVector)
              m_vectorJobs.back().bindingIndex =
                  m_result.nativeSourceMap.size() - 1;
            m_stageBindings.push_back({m_result.nativeSourceMap.size() - 1,
                                       xsheet, nativeColumn, frame.index,
                                       frame.duration});
          } catch (const Flash::Error &error) {
            const auto message = QString::fromUtf8(error.what());
            if (message.startsWith(element.sourceId + ":") ||
                message.startsWith(element.sourceId + "/"))
              throw;
            throw Flash::Error(element.sourceId + ": " + message);
          }
        }
      }
      if (guideIndex >= 0)
        followGuide(xsheet, source, layerIndex, guideIndex, partPlan,
                    trackColumns, origin, layerId);
      // A Flash label is attached to a layer keyframe. Anchor it to that
      // layer's first native artwork column, including a visible empty column
      // for an authored layer containing only labels. The painted symbol
      // keeps the labels of a mask copy.
      if (!m_maskCopyNames.isEmpty()) frameLabels.clear();
      if (!frameLabels.empty() && columns.isEmpty())
        columns.push_back(
            column(xsheet, layer.name, visible(source, layerIndex), false));
      for (auto &label : frameLabels) {
        label.nativeColumn = columns.front();
        xsheet->addFlashFrameLabel(std::move(label));
      }
      for (int index : columns)
        xsheet->getColumn(index)->lock(locked(source, layerIndex));
      // Masks ignore the alpha of their artwork (and its effects). The color
      // FX of a filtered column carries its alpha (see applyEffects()).
      if (!m_maskArtwork)
        for (auto it = columnAlpha.cbegin(); it != columnAlpha.cend(); ++it)
          if (!applyEffects(
                  xsheet, it.key(), columnEffects.value(it.key()), source,
                  layerIndex,
                  QString::fromStdString(
                      xsheet->getStageObject(TStageObjectId::ColumnId(it.key()))
                          ->getName())))
            applyAlpha(xsheet, it.key(), it.value());
      sourceColumns[layerIndex] = columns;
      // A mask layer clips the columns of all its masked layers: one mask
      // column (native mask scope) for the viewer and playback, an In FX for
      // render. Inputs stay preview-visible for FxBuilder, but their original
      // terminal outputs must not render independently.
      if (layer.type == "mask")
        linkMask(xsheet, source, layerIndex, columns, sourceColumns, layerId);
    }
    if (!rigKeyings.isEmpty())
      applyRig(xsheet, source, rigs, rigKeyings, origin, sourceMember);
    if (cameraMapped) attachToCamera(xsheet, source, sourceColumns);
    if (m_blendedSheets.contains(xsheet) && m_container == "document")
      stageBackground(xsheet, std::max(1, source.frameCount));
    preserveDuration(xsheet, std::max(1, source.frameCount));
  }

  // Native sound columns play only on the top-level xsheet. Sounds of the
  // document timelines are collected while those are built, sounds of movie
  // clips where clips on stage play them (flattenClipSounds()), and all are
  // placed afterwards in source time order, since start and stop cues depend
  // on what is already playing. Compatibility::sound() reports sounds in
  // graphic symbols and buttons.
  struct SoundLayer {
    QString name, sourceLayerId;
    bool visible = true, editorOnly = false, locked = false;
  };
  struct SoundCue {
    Flash::FrameSound sound;
    QString sourceId, frameXmlId;
    int layer       = 0;  // Index into m_soundLayers
    int row         = 0;  // Root row of the keyframe
    int spanEnd     = 0;  // Root row after the keyframe span
    int timelineEnd = 0;  // Root row after its timeline
    // A stream entered part way through its keyframe span starts this many
    // frames into the sound.
    int offsetFrames = 0;
    QString symbol;  // The movie clip whose timeline holds the sound
    // A video's sound track (sound.name is the video item); otherwise the
    // sound item's samples are read when the cue is placed.
    std::shared_ptr<const Flash::SoundPayload> payload;
  };
  struct SoundPlacement {
    int cue               = -1;
    TXshSoundLevel *level = nullptr;
    int start = 0, end = 0;  // Root rows [start, end)
    double volume        = 1;
    const char *envelope = "none";  // "volume" (column) or "baked" (samples)
    const char *encoding = "pcm";   // Of the embedded samples
  };
  std::vector<SoundCue> m_soundCues;
  QVector<SoundLayer> m_soundLayers;
  QMap<QString, int> m_soundLayerIndex;  // Source layer ID to m_soundLayers
  QSet<QString> m_soundFileNames;        // Lower case, without extension
  // Definition whose timeline is being built: "document" or its symbol type.
  QString m_container  = "document";
  int m_rootRowOffset  = 0;  // Root row of the document timeline being built
  quint64 m_soundBytes = 0;
  int m_soundLevels = 0, m_soundColumns = 0;
  int m_bakedEnvelopes = 0, m_clipSoundPlacements = 0;
  QJsonArray m_nativeSounds;
  // Decoded and repeated samples kept in memory until the bundle is written.
  static constexpr quint64 maxSoundBytes = 1ull << 30;

  struct ContainerScope {
    QString &container;
    const QString saved;
    ContainerScope(QString &current, const QString &type)
        : container(current), saved(current) {
      container = type;
    }
    ~ContainerScope() { container = saved; }
  };

  void queueSound(const Flash::FrameSound &sound, const Flash::Timeline &source,
                  int layerIndex, const QString &layerId,
                  const Flash::Frame &frame, const QString &frameId) {
    const auto &layer = source.layers[layerIndex];
    int index         = m_soundLayerIndex.value(layerId, -1);
    if (index < 0) {
      SoundLayer native;
      native.name = layer.name;
      native.sourceLayerId =
          layer.sourceId.isEmpty() ? layerId : layer.sourceId;
      // Guide layers are not published; their sounds stay editor-only.
      native.editorOnly = layer.type == "guide";
      native.visible    = visible(source, layerIndex, native.editorOnly);
      native.locked     = locked(source, layerIndex);
      index             = m_soundLayers.size();
      m_soundLayers.push_back(native);
      m_soundLayerIndex.insert(layerId, index);
    }
    SoundCue cue;
    cue.sound       = sound;
    cue.sourceId    = frameId;
    cue.frameXmlId  = frame.source.sourceId;
    cue.layer       = index;
    cue.row         = m_rootRowOffset + frame.index;
    cue.spanEnd     = cue.row + frame.duration;
    cue.timelineEnd = m_rootRowOffset + std::max(1, source.frameCount);
    m_soundCues.push_back(std::move(cue));
  }

  // A placed video plays its sound track in step with its frames: published
  // SWFs mix it into the main timeline's sound stream with its first sample
  // on the video's first frame, as a stream sound of the keyframes that
  // continue the video (videoClockStart()). Its column is named after the
  // video item. Like the frames, it is published only from layers Flash
  // publishes: never from guides, from hidden layers only with "Include
  // hidden layers" (the published SWFs carry no frames and no sound of those).
  void queueVideoSound(const std::shared_ptr<const Flash::SoundPayload> &track,
                       const QString &libraryName,
                       const Flash::Timeline &source, int layerIndex,
                       const QString &layerId, int frameOrdinal,
                       const QString &sourceId) {
    const auto &layer = source.layers[layerIndex];
    const auto &frame = layer.frames[frameOrdinal];
    // Following keyframes continue the sound placed by the first one.
    if (videoClockStart(layer, frameOrdinal, libraryName) != frame.index)
      return;
    const bool guide = layer.type == "guide";
    if (guide || !visible(source, layerIndex)) {
      sink()({"unpublished-video-audio", sourceId,
              "Video \"" + libraryName + "\" is on a " +
                  (guide ? "guide layer" : "hidden layer") +
                  ", which Flash does not publish; its sound track is not "
                  "placed."});
      return;
    }
    if (m_container != "document") {
      sink()({"unmapped-video-audio", sourceId,
              "Video \"" + libraryName + "\" inside a " + m_container +
                  " is imported without its sound track: native sound "
                  "columns play only on the top-level timeline."});
      return;
    }
    const QString key = layerId + "/video/" + libraryName;
    int index         = m_soundLayerIndex.value(key, -1);
    if (index < 0) {
      SoundLayer native;
      native.name = QFileInfo(libraryName).fileName();
      native.sourceLayerId =
          layer.sourceId.isEmpty() ? layerId : layer.sourceId;
      native.visible = true;
      native.locked  = locked(source, layerIndex);
      index          = m_soundLayers.size();
      m_soundLayers.push_back(native);
      m_soundLayerIndex.insert(key, index);
    }
    SoundCue cue;
    cue.sound.name = libraryName;
    cue.sound.sync = Flash::FrameSound::Sync::Stream;
    cue.payload    = track;
    cue.sourceId   = sourceId;
    cue.frameXmlId = frame.source.sourceId;
    cue.layer      = index;
    cue.row        = m_rootRowOffset + frame.index;
    cue.spanEnd =
        m_rootRowOffset + videoClockEnd(layer, frameOrdinal, libraryName);
    cue.timelineEnd = m_rootRowOffset + std::max(1, source.frameCount);
    m_soundCues.push_back(std::move(cue));
  }

  // Movie clip sounds ---------------------------------------------------------
  // Flash plays the sounds of a movie clip (never those of a graphic symbol)
  // when the clip's playhead enters their keyframes, wherever the clip is on
  // stage, and only top-level native sound columns play. So every placement
  // is followed tick by tick from the document timelines with the clocks
  // Flash gives clips: a clip starts on frame 0 the tick it appears and keeps
  // its clock while its placement stays on stage (consecutive keyframes
  // keeping the same instance, a goto within them, or a loop onto the
  // placement that continues it, as in clockMismatch()); a graphic shows the
  // frames its parent's frame gives it (a button its Up frame). Event, start
  // and stop sounds cue where a clip enters their keyframe; a stream plays
  // while the clip advances frame by frame through its keyframe span (from
  // the frame where it enters it), as the clip's stream blocks would.
  struct ClipSoundChild {
    enum class Kind { MovieClip, Graphic, Button };
    QString symbol;
    Kind kind = Kind::Graphic;
    QString lane;        // The placement within its layer: slot and symbol
    int clockStart = 0;  // Movie clips: the frame their clock starts on
    Flash::GraphicTiming timing;  // Graphics
  };
  struct ClipSoundKey {
    int index = 0, duration = 1;
    std::optional<Flash::FrameSound> sound;
    QString frameId, frameXmlId;
    QVector<ClipSoundChild> children;
  };
  struct ClipSoundLayer {
    int index = 0;  // In the source timeline
    QString name, sourceLayerId;
    bool visible = true, editorOnly = false;
    QVector<ClipSoundKey> keys;  // In frame order: keys with sounds or symbols
  };
  struct ClipSoundTimeline {
    QString symbol;  // Empty for a document timeline
    int frameCount = 1;
    Flash::Playhead playhead;
    QVector<ClipSoundLayer> layers;
    bool ownSounds = false;
  };
  // Per symbol, from its definition when it is built (null: nothing sounds).
  QMap<QString, std::shared_ptr<const ClipSoundTimeline>> m_clipSoundTimelines;
  QMap<QString, bool> m_clipSoundsBelow, m_symbolMayHoldSounds;
  QSet<QString> m_clipSoundsVisiting;
  QSet<QString> m_clipSoundKeysPlayed;  // frameId of every placed clip sound
  int m_clipSoundCues                   = 0;
  static constexpr int maxClipSoundCues = 100000;

  std::shared_ptr<const ClipSoundTimeline> clipSoundTimeline(
      const QString &symbol, const Flash::Timeline &source,
      const QString &timelineId) {
    auto result        = std::make_shared<ClipSoundTimeline>();
    result->symbol     = symbol;
    result->frameCount = std::max(1, source.frameCount);
    for (int layerIndex = 0; layerIndex < source.layers.size(); ++layerIndex) {
      const auto &layer = source.layers[layerIndex];
      const QString layerId =
          timelineId + "/layer" + QString::number(layerIndex);
      ClipSoundLayer summary;
      summary.index = layerIndex;
      summary.name  = layer.name;
      summary.sourceLayerId =
          layer.sourceId.isEmpty() ? layerId : layer.sourceId;
      // Guides are not published; their sounds stay editor-only.
      summary.editorOnly = layer.type == "guide";
      try {
        summary.visible = visible(source, layerIndex, summary.editorOnly);
      } catch (const Flash::Error &) {
        check();
        continue;  // Rejected or reported when the timeline is built
      }
      for (int ordinal = 0; ordinal < layer.frames.size(); ++ordinal) {
        const auto &frame = layer.frames[ordinal];
        ClipSoundKey key;
        key.index    = frame.index;
        key.duration = frame.duration;
        key.sound    = Flash::frameSound(frame);
        if (key.sound) {
          key.frameId       = layerId + "/frame" + QString::number(frame.index);
          key.frameXmlId    = frame.source.sourceId;
          result->ownSounds = true;
        }
        for (int slot = 0; slot < frame.elements.size(); ++slot)
          clipSoundChildren(frame.elements[slot], layer, ordinal, slot,
                            QString::number(slot), key.children);
        if (key.sound || !key.children.isEmpty())
          summary.keys.push_back(std::move(key));
      }
      if (!summary.keys.isEmpty()) result->layers.push_back(std::move(summary));
    }
    // A clip's frame scripts move its playhead (Flash ignores them in
    // graphics, which never use it).
    if (!result->layers.isEmpty())
      result->playhead = Flash::timelinePlayhead(source, false);
    return result;
  }

  void clipSoundChildren(const Flash::Element &element,
                         const Flash::Layer &layer, int ordinal, int slot,
                         const QString &lane,
                         QVector<ClipSoundChild> &children) {
    const auto &frame = layer.frames[ordinal];
    if (element.kind == Flash::Element::Kind::Group) {
      // Group members: their clips start with each keyframe.
      for (int member = 0; member < element.members.size(); ++member)
        clipSoundChildren(element.members[member], layer, ordinal, -1,
                          lane + "." + QString::number(member), children);
      return;
    }
    if (element.kind != Flash::Element::Kind::Symbol) return;
    ClipSoundChild child;
    child.symbol = element.properties.value("libraryItemName");
    if (!m_info.symbols.contains(child.symbol)) return;
    child.lane         = lane + "/" + child.symbol;
    const QString type = Flash::symbolInstanceType(element);
    if (type == "movie clip") {
      child.kind       = ClipSoundChild::Kind::MovieClip;
      child.clockStart = frame.index;
      if (slot >= 0) try {
          child.clockStart = Flash::movieClipClockStart(layer, ordinal, slot);
        } catch (const Flash::Error &) {
          check();
        }
    } else if (type == "button") {
      child.kind = ClipSoundChild::Kind::Button;
    } else {
      try {
        child.timing = Flash::instanceTiming(element, frame);
      } catch (const Flash::Error &) {
        check();
        child.timing = fallbackGraphicTiming(element, frame);
      }
    }
    children.push_back(std::move(child));
  }

  // The frame of a graphic shown on frame `parentFrame` of its parent, as
  // the builder places it (first frames past the end wrap or rest on the
  // last frame); -1 when it shows none.
  static int graphicFrameAt(Flash::GraphicTiming timing, int parentFrame,
                            int frameCount) {
    const int count  = std::max(1, frameCount);
    const bool loops = timing.mode == Flash::PlaybackMode::Loop ||
                       timing.mode == Flash::PlaybackMode::LoopReverse;
    if (!timing.lastFrame && timing.firstFrame >= count)
      timing.firstFrame =
          Flash::wrappedFirstFrame(timing.firstFrame, timing.mode, count);
    for (int attempt = 0; attempt < 2; ++attempt) {
      try {
        const auto frame = timing.sourceFrameAt(parentFrame, count);
        return frame ? *frame : -1;
      } catch (const Flash::Error &) {
        timing.lastFrame.reset();
        timing.firstFrame = loops ? timing.firstFrame % count
                                  : std::min(timing.firstFrame, count - 1);
      }
    }
    return -1;
  }

  // Whether a symbol or one it holds has a frame with a sound, from its XML
  // bytes and the document's symbol references (no parsing).
  bool symbolMayHoldSounds(const QString &symbol, int depth = 0) {
    const auto known = m_symbolMayHoldSounds.constFind(symbol);
    if (known != m_symbolMayHoldSounds.cend()) return known.value();
    const auto info = m_info.symbols.constFind(symbol);
    if (info == m_info.symbols.cend() ||
        depth > m_package.limits().maxSymbolDepth)
      return false;
    bool result = false;
    try {
      result = m_package.read(info->member).contains("soundName");
    } catch (const Flash::Error &) {
      check();
    }
    for (auto it = info->references.cbegin();
         !result && it != info->references.cend(); ++it)
      result = symbolMayHoldSounds(*it, depth + 1);
    m_symbolMayHoldSounds.insert(symbol, result);
    return result;
  }

  // A symbol's summary: recorded when the builder built it, otherwise read
  // now if anything in it may sound (instances the builder omitted, such as
  // hidden ones, still play in Flash).
  std::shared_ptr<const ClipSoundTimeline> clipSoundTimelineOf(
      const QString &symbol) {
    const auto known = m_clipSoundTimelines.constFind(symbol);
    if (known != m_clipSoundTimelines.cend()) return known.value();
    std::shared_ptr<const ClipSoundTimeline> result;
    const auto info = m_info.symbols.constFind(symbol);
    if (info != m_info.symbols.cend() && symbolMayHoldSounds(symbol)) {
      try {
        const auto definition = Flash::readDefinition(m_package, info->member);
        if (!definition.timelines.isEmpty())
          result = clipSoundTimeline(symbol, definition.timelines.front(),
                                     info->member + "/timeline0");
      } catch (const Flash::Error &) {
        check();
      }
    }
    m_clipSoundTimelines.insert(symbol, result);
    return result;
  }

  // Whether a placement can play a sound: its own as a movie clip, or one of
  // a clip placed inside it.
  bool clipSoundsPlay(const ClipSoundChild &child) {
    const auto timeline = clipSoundTimelineOf(child.symbol);
    if (!timeline) return false;
    return (child.kind == ClipSoundChild::Kind::MovieClip &&
            timeline->ownSounds) ||
           clipSoundsBelow(child.symbol);
  }

  bool clipSoundsBelow(const QString &symbol) {
    const auto known = m_clipSoundsBelow.constFind(symbol);
    if (known != m_clipSoundsBelow.cend()) return known.value();
    if (m_clipSoundsVisiting.contains(symbol)) return false;  // a cycle
    m_clipSoundsVisiting.insert(symbol);
    bool result = false;
    if (const auto timeline = clipSoundTimelineOf(symbol))
      for (const auto &layer : timeline->layers)
        for (const auto &key : layer.keys)
          for (const auto &child : key.children)
            if (!result && clipSoundsPlay(child)) result = true;
    m_clipSoundsVisiting.remove(symbol);
    m_clipSoundsBelow.insert(symbol, result);
    return result;
  }

  struct ClipLane {  // A movie clip on stage, as of its last tick
    int tick = -2, clockStart = 0, frame = 0, keyEnd = 0;
    qint64 elapsed = 0;
  };
  struct ClipStream {  // A stream sound playing
    const ClipSoundTimeline *timeline = nullptr;
    int layer = 0, key = 0;
    int first = 0, last = 0, frame = 0, offset = 0;  // Ticks; source frame
    bool visible = true, editorOnly = false;
  };
  struct ClipSoundWalk {
    int rowOffset = 0, timelineEnd = 0;
    QMap<QString, ClipLane> lanes;
    QMap<QString, ClipStream> streams;
  };

  void clipSoundCue(const ClipSoundWalk &walk,
                    const ClipSoundTimeline &timeline, int layerIndex,
                    const ClipSoundKey &key, int firstTick, int endTick,
                    int offsetFrames, bool visible, bool editorOnly) {
    m_clipSoundKeysPlayed.insert(key.frameId);
    if (++m_clipSoundCues > maxClipSoundCues) {
      if (m_clipSoundCues == maxClipSoundCues + 1)
        sink()({"sound-budget-exceeded", key.frameId,
                QString("Movie clips play more than %1 sounds; the rest are "
                        "not placed.")
                    .arg(maxClipSoundCues)});
      return;
    }
    const auto &layer      = timeline.layers[layerIndex];
    const QString layerKey = QString("clip%1%2%1%3%1%4%5")
                                 .arg(QChar(0x1f))
                                 .arg(timeline.symbol, layer.sourceLayerId)
                                 .arg(int(visible))
                                 .arg(int(editorOnly));
    int index = m_soundLayerIndex.value(layerKey, -1);
    if (index < 0) {
      SoundLayer native;
      native.name =
          timeline.symbol.section('/', -1) + " / " + layer.name + " sound";
      native.sourceLayerId = layer.sourceLayerId;
      native.visible       = visible;
      native.editorOnly    = editorOnly;
      index                = m_soundLayers.size();
      m_soundLayers.push_back(native);
      m_soundLayerIndex.insert(layerKey, index);
    }
    SoundCue cue;
    cue.sound        = *key.sound;
    cue.sourceId     = key.frameId;
    cue.frameXmlId   = key.frameXmlId;
    cue.layer        = index;
    cue.row          = walk.rowOffset + firstTick;
    cue.spanEnd      = walk.rowOffset + endTick;
    cue.timelineEnd  = walk.timelineEnd;
    cue.offsetFrames = offsetFrames;
    cue.symbol       = timeline.symbol;
    m_soundCues.push_back(std::move(cue));
  }

  void finishClipStream(const ClipSoundWalk &walk, const ClipStream &stream) {
    const auto &key = stream.timeline->layers[stream.layer].keys[stream.key];
    clipSoundCue(walk, *stream.timeline, stream.layer, key, stream.first,
                 stream.last + 1, stream.offset, stream.visible,
                 stream.editorOnly);
  }

  // Tick `tick` of a walk: `timeline` shows `frame`; `playing` for a movie
  // clip (whose sounds play), `entered` when its playhead entered the frame.
  void walkClipSounds(ClipSoundWalk &walk, const ClipSoundTimeline &timeline,
                      int frame, bool playing, bool entered,
                      const QString &laneKey, bool visible, bool editorOnly,
                      int tick, int depth) {
    if (depth > m_package.limits().maxSymbolDepth) return;
    for (int layerIndex = 0; layerIndex < timeline.layers.size();
         ++layerIndex) {
      const auto &layer = timeline.layers[layerIndex];
      auto found        = std::upper_bound(
                 layer.keys.cbegin(), layer.keys.cend(), frame,
                 [](int at, const ClipSoundKey &key) { return at < key.index; });
      if (found == layer.keys.cbegin()) continue;
      --found;
      if (frame >= found->index + found->duration) continue;
      const ClipSoundKey &key = *found;
      const int keyIndex      = int(found - layer.keys.cbegin());
      const bool shown        = visible && layer.visible;
      const bool editor       = editorOnly || layer.editorOnly;
      if (playing && key.sound) {
        if (key.sound->sync == Flash::FrameSound::Sync::Stream) {
          const QString streamKey =
              laneKey + QChar(0x1f) + QString::number(layer.index);
          auto stream = walk.streams.find(streamKey);
          const bool continues =
              stream != walk.streams.end() && stream->timeline == &timeline &&
              stream->key == keyIndex && stream->last == tick - 1 &&
              frame == stream->frame + 1;
          if (continues) {
            stream->last  = tick;
            stream->frame = frame;
          } else if (entered) {
            if (stream != walk.streams.end()) finishClipStream(walk, *stream);
            walk.streams.insert(
                streamKey, {&timeline, layerIndex, keyIndex, tick, tick, frame,
                            frame - key.index, shown, editor});
          }
        } else if (entered && frame == key.index)
          clipSoundCue(walk, timeline, layerIndex, key, tick,
                       tick + key.duration, 0, shown, editor);
      }
      for (const auto &child : key.children) {
        if (!clipSoundsPlay(child)) continue;
        const auto inner = clipSoundTimelineOf(child.symbol);
        const QString childKey =
            laneKey + "/" + QString::number(layer.index) + ":" + child.lane;
        if (child.kind == ClipSoundChild::Kind::MovieClip) {
          ClipLane &lane = walk.lanes[childKey];
          const bool kept =
              lane.tick == tick - 1 &&
              (lane.clockStart == child.clockStart ||
               (lane.keyEnd >= timeline.frameCount && child.clockStart == 0));
          const qint64 elapsed = kept ? lane.elapsed + 1 : 0;
          const int shownFrame = inner->playhead.frameAt(elapsed);
          const bool enters    = !kept || shownFrame != lane.frame;
          lane.tick            = tick;
          lane.clockStart      = child.clockStart;
          lane.frame           = shownFrame;
          lane.keyEnd          = key.index + key.duration;
          lane.elapsed         = elapsed;
          walkClipSounds(walk, *inner, shownFrame, true, enters, childKey,
                         shown, editor, tick, depth + 1);
        } else {
          const int shownFrame =
              child.kind == ClipSoundChild::Kind::Button
                  ? 0
                  : graphicFrameAt(child.timing, frame, inner->frameCount);
          if (shownFrame >= 0)
            walkClipSounds(walk, *inner, shownFrame, false, false, childKey,
                           shown, editor, tick, depth + 1);
        }
      }
    }
  }

  // Queues the sounds movie clips on stage play, as top-level cues, and
  // reports the movie clip sounds no clip on stage plays.
  void flattenClipSounds(const Flash::Definition &document,
                         const Flash::Playhead &scenePlayhead,
                         bool scriptedScene) {
    const bool sounds =
        std::any_of(m_info.assets.constBegin(), m_info.assets.constEnd(),
                    [](const QJsonValue &item) {
                      return item.toObject().value("type") == "DOMSoundItem";
                    });
    if (!sounds) return;
    int offset = 0;
    for (int index = 0; index < document.timelines.size(); ++index) {
      const auto &source = document.timelines[index];
      const int count    = std::max(1, source.frameCount);
      const QString timelineId =
          document.member + "/timeline" + QString::number(index);
      const auto scene = clipSoundTimeline(QString(), source, timelineId);
      bool plays       = false;
      for (const auto &layer : scene->layers)
        for (const auto &key : layer.keys)
          for (const auto &child : key.children)
            if (!plays && clipSoundsPlay(child)) plays = true;
      if (plays) {
        ClipSoundWalk walk;
        walk.rowOffset   = offset;
        walk.timelineEnd = offset + count;
        // A scripted main timeline (one scene) shows its playhead's frames.
        const bool followsPlayhead =
            scriptedScene && document.timelines.size() == 1;
        const QString laneKey = "scene" + QString::number(index);
        for (int tick = 0; tick < count; ++tick) {
          if ((tick & 63) == 0) check();
          walkClipSounds(walk, *scene,
                         followsPlayhead ? scenePlayhead.frameAt(tick) : tick,
                         false, false, laneKey, true, false, tick, 0);
        }
        for (const auto &stream : walk.streams) finishClipStream(walk, stream);
      }
      offset += count;
    }
    // Sounds of movie clips that no clip on stage plays: Flash would play
    // them only through scripts (attached clips, goto from elsewhere).
    for (auto it = m_clipSoundTimelines.cbegin();
         it != m_clipSoundTimelines.cend(); ++it) {
      const auto info = m_info.symbols.constFind(it.key());
      if (!it.value() || info == m_info.symbols.cend() ||
          (!info->type.isEmpty() && info->type != "movie clip"))
        continue;
      for (const auto &layer : it.value()->layers)
        for (const auto &key : layer.keys)
          if (key.sound && key.sound->sync != Flash::FrameSound::Sync::Stop &&
              !m_clipSoundKeysPlayed.contains(key.frameId))
            sink()({"unmapped-nested-sound", key.frameId,
                    "Sound \"" + key.sound->name + "\" in movie clip \"" +
                        it.key() +
                        "\" is not placed: no placement of the clip on stage "
                        "reaches its frame (scripts that attach or control "
                        "clips are not run)."});
    }
  }

  // Flare shows a sound level for int(duration * fps) whole frames. The
  // returned sample count, padded with silence, makes those frames cover
  // every source sample; `frames` receives their number.
  static qint64 frameAlignedSamples(qint64 samples, int rate, double fps,
                                    int &frames) {
    frames = std::max(
        1, int(std::ceil(double(samples) * fps / double(rate) - 1e-6)));
    qint64 padded = std::max<qint64>(
        samples, qint64(std::ceil(double(frames) * double(rate) / fps)));
    while (int(double(padded) / double(rate) * fps) < frames) ++padded;
    return padded;
  }

  // A portable file and level name from the library item name: Flare
  // names a loaded sound level after its file.
  QString soundName(const QString &libraryName) {
    QString base;
    for (const QChar c : QFileInfo(libraryName).completeBaseName())
      base += (c.unicode() < 128 && c.isLetterOrNumber()) || c == '-'
                  ? c
                  : QChar('_');
    const auto trim = [](QString &name) {
      while (name.startsWith('_') || name.startsWith('-')) name.remove(0, 1);
      while (name.endsWith('_') || name.endsWith('-')) name.chop(1);
    };
    trim(base);
    base = base.left(60);
    trim(base);
    static const QRegularExpression reserved(
        "^(con|prn|aux|nul|com[0-9]|lpt[0-9])$",
        QRegularExpression::CaseInsensitiveOption);
    if (base.isEmpty() || reserved.match(base).hasMatch())
      base = base.isEmpty() ? QString("sound") : "sound-" + base;
    QString name = base;
    for (int suffix = 2;
         m_soundFileNames.contains(name.toLower()) ||
         m_result.scene->getLevelSet()->hasLevel(name.toStdWString());
         ++suffix)
      name = base + "-" + QString::number(suffix);
    m_soundFileNames.insert(name.toLower());
    return name;
  }

  TXshSoundLevel *soundLevel(const QString &libraryName,
                             const Flash::SoundFormat &format, QByteArray pcm,
                             int &frames) {
    check();
    if (m_levelSerial >= m_options.maxLevels)
      throw Flash::Error("Native level budget exceeded.");
    ++m_levelSerial;
    const int frameBytes = format.frameBytes();
    const qint64 samples = pcm.size() / frameBytes;
    const qint64 padded  = frameAlignedSamples(samples, format.sampleRate,
                                               m_info.frameRate, frames);
    // Silence is 0 for signed 16-bit and 0x80 for unsigned 8-bit samples.
    pcm.append(QByteArray(int((padded - samples) * frameBytes),
                          format.bits == 8 ? char(0x80) : char(0)));
    const QString name = soundName(libraryName);
    const QString filename =
        QDir(m_options.assetDirectory).filePath(name + ".wav");
    if (QFileInfo::exists(filename))
      throw Flash::Error("Import asset path already exists: " + filename);
    TSoundTrackP track = TSoundTrack::create(
        TUINT32(format.sampleRate), format.bits, format.channels,
        TINT32(padded), format.bits == 8 ? TSound::UINT : TSound::INT);
    if (!track || track->getSampleCount() != padded)
      throw Flash::Error("Cannot create a native sound track: " + libraryName);
    std::memcpy(const_cast<UCHAR *>(track->getRawData()), pcm.constData(),
                size_t(pcm.size()));
    auto level = std::make_unique<TXshSoundLevel>(name.toStdWString());
    level->setType(SND_XSHLEVEL);
    level->setScene(m_result.scene.get());
    level->setPath(TFilePath(filename.toStdWString()));
    level->setSoundTrack(track);
    if (level->getFrameCount() != frames)
      throw Flash::Error("Native sound level duration differs: " + libraryName);
    m_result.scene->getLevelSet()->insertLevel(level.get());
    ++m_soundLevels;
    return level.release();
  }

  int soundColumn(TXsheet *xsheet, const SoundLayer &layer, double volume) {
    check();
    if (m_result.columns >= m_options.maxColumns)
      throw Flash::Error("Native column budget exceeded.");
    const int index = xsheet->getColumnCount();
    xsheet->insertColumn(index, TXshColumn::eSoundType);
    auto *column = xsheet->getColumn(index)
                       ? xsheet->getColumn(index)->getSoundColumn()
                       : nullptr;
    if (!column) throw Flash::Error("Cannot create a native sound column.");
    column->setVolume(volume);
    column->setPreviewVisible(layer.visible && !layer.editorOnly);
    column->setCamstandVisible(layer.visible);
    ++m_result.columns;
    ++m_soundColumns;
    return index;
  }

  void placeSounds(TXsheet *root) {
    using Sync = Flash::FrameSound::Sync;
    if (m_soundCues.empty()) return;
    const double fps = m_info.frameRate;
    std::vector<int> order(m_soundCues.size());
    std::iota(order.begin(), order.end(), 0);
    std::stable_sort(order.begin(), order.end(), [&](int a, int b) {
      return m_soundCues[a].row < m_soundCues[b].row;
    });
    QMap<QString, std::shared_ptr<const Flash::SoundPayload>> payloads;
    QMap<QString, CompatibilityIssue> unusable;
    struct Variant {
      TXshSoundLevel *level = nullptr;
      int frames            = 0;
    };
    QMap<QString, Variant> variants;
    std::vector<SoundPlacement> placements;
    const auto playing = [&](const QString &name, int row) {
      return std::any_of(placements.cbegin(), placements.cend(),
                         [&](const SoundPlacement &placement) {
                           return m_soundCues[placement.cue].sound.name ==
                                      name &&
                                  placement.start <= row && row < placement.end;
                         });
    };
    for (const int index : order) {
      check();
      const SoundCue &cue            = m_soundCues[index];
      const Flash::FrameSound &sound = cue.sound;
      if (sound.sync == Sync::Stop) {
        // Silences event sounds of that name which are still playing.
        for (auto &placement : placements) {
          const auto &other = m_soundCues[placement.cue].sound;
          if (other.name == sound.name && other.sync != Sync::Stream &&
              placement.start < cue.row && cue.row < placement.end)
            placement.end = cue.row;
        }
        continue;
      }
      // A start cue does not add an instance of a sound already playing.
      if (sound.sync == Sync::Start && playing(sound.name, cue.row)) continue;
      auto payload = cue.payload ? cue.payload : payloads.value(sound.name);
      if (!payload) {
        if (unusable.contains(sound.name)) {
          CompatibilityIssue issue = unusable.value(sound.name);
          issue.sourceId           = cue.sourceId;
          sink()(issue);
          continue;
        }
        std::optional<CompatibilityIssue> failure;
        auto decoded = Compatibility::soundMedia(
            m_package, m_info, sound.name, cue.sourceId,
            bool(m_options.decodeSound),
            [&failure](const CompatibilityIssue &issue) { failure = issue; });
        // MP3 samples are decoded once, on first use.
        if (decoded && !decoded->decoded()) {
          try {
            m_options.decodeSound(*decoded);
            if (decoded->pcm.size() !=
                decoded->sampleCount * decoded->format.frameBytes())
              throw Flash::Error("The decoder returned " +
                                 QString::number(decoded->pcm.size()) +
                                 " bytes, not the sound's samples.");
            if (!decoded->decodeNote.isEmpty())
              sink()({"sound-decode-approximated", cue.sourceId,
                      "Sound \"" + sound.name + "\": " + decoded->decodeNote +
                          "."});
          } catch (const Flash::Error &error) {
            check();
            failure = CompatibilityIssue{
                "sound-decode-failed", cue.sourceId,
                "Sound \"" + sound.name +
                    "\" is not placed: " + QString::fromUtf8(error.what())};
            decoded.reset();
          }
        }
        if (!decoded) {
          if (!failure) continue;
          unusable.insert(sound.name, *failure);
          sink()(*failure);
          continue;
        }
        payload =
            std::make_shared<const Flash::SoundPayload>(std::move(*decoded));
        payloads.insert(sound.name, payload);
      }
      const int rate        = payload->format.sampleRate;
      const int sourceBytes = payload->format.frameBytes();
      // A volume envelope that changes (fades, panning) is baked into the
      // samples; a constant one is the column volume.
      bool constantVolume       = true;
      const double volume       = Flash::soundVolume(sound, &constantVolume);
      Flash::SoundFormat format = payload->format;
      QVector<Flash::SoundLevelPoint> envelope;
      if (!constantVolume) {
        for (const auto &point : sound.envelope)
          envelope.push_back({qint64(double(point.mark44) * rate / 44100.0),
                              point.level0 / 32768.0, point.level1 / 32768.0});
        bool stereo = format.channels == 2;
        for (const auto &point : envelope) stereo |= point.left != point.right;
        format = {rate, 16, stereo ? 2 : 1};
      }
      const int frameBytes = format.frameBytes();
      // In and out points count 44.1 kHz samples whatever the sound's rate.
      const auto sample = [&](qint64 point44) {
        return std::min(payload->sampleCount,
                        qint64(std::llround(double(point44) * rate / 44100.0)));
      };
      const qint64 in = sample(sound.inPoint44);
      const qint64 out =
          sound.outPoint44 ? sample(*sound.outPoint44) : payload->sampleCount;
      if (out <= in) {
        sink()({"unmapped-sound-attribute", cue.sourceId,
                "Sound \"" + sound.name +
                    "\" has no samples between its in and out points."});
        continue;
      }
      // A stream plays only within its keyframe span; event and start sounds
      // play on, but the native scene ends with the timeline.
      const int limit     = sound.sync == Sync::Stream
                                ? std::min(cue.spanEnd, cue.timelineEnd)
                                : cue.timelineEnd;
      const int available = limit - cue.row;
      if (available <= 0) continue;
      const qint64 unit  = out - in;
      const bool looping = sound.loopForever || sound.repeat > 1;
      // A stream entered part way through its span starts there.
      const qint64 skip =
          qint64(std::llround(double(cue.offsetFrames) * rate / fps));
      const qint64 visibleSamples =
          skip + qint64(std::ceil(double(available) * rate / fps));
      // Repetitions are rendered only as far as they can be heard.
      const qint64 rendered =
          !looping ? unit
          : sound.loopForever
              ? visibleSamples
              : std::min<qint64>(unit * sound.repeat, visibleSamples);
      if (skip >= rendered) continue;
      QString key = QString("%1|%2|%3|%4|%5")
                        .arg(sound.name)
                        .arg(in)
                        .arg(out)
                        .arg(rendered)
                        .arg(skip);
      for (const auto &point : envelope)
        key += QString("|%1:%2:%3")
                   .arg(point.sample)
                   .arg(point.left)
                   .arg(point.right);
      Variant variant = variants.value(key);
      if (!variant.level) {
        const quint64 bytes = quint64(rendered) * quint64(frameBytes);
        if (rendered > std::numeric_limits<int>::max() / (frameBytes * 2) ||
            bytes > maxSoundBytes - std::min(m_soundBytes, maxSoundBytes)) {
          sink()({"sound-budget-exceeded", cue.sourceId,
                  "Sound \"" + sound.name +
                      "\" exceeds the native sound memory budget."});
          continue;
        }
        m_soundBytes += bytes;
        QByteArray pcm;
        pcm.reserve(int(quint64(rendered) * quint64(sourceBytes)));
        for (qint64 done = 0; done < rendered;) {
          check();
          const qint64 take = std::min(unit, rendered - done);
          pcm.append(payload->pcm.constData() + in * sourceBytes,
                     int(take * sourceBytes));
          done += take;
        }
        // The envelope runs from the in point on through the repetitions.
        if (!constantVolume) {
          Flash::SoundFormat baked;
          pcm = Flash::envelopedPcm(payload->format, pcm, envelope, baked);
        }
        if (skip) pcm.remove(0, int(skip * frameBytes));
        variant.level =
            soundLevel(sound.name, format, std::move(pcm), variant.frames);
        variants.insert(key, variant);
      }
      if (sound.sync != Sync::Stream &&
          (sound.loopForever ||
           double(unit) * sound.repeat * fps / rate > available + 1e-6))
        sink()({"unmapped-sound-tail", cue.sourceId,
                QString("Sound \"%1\" plays on past the end of its timeline "
                        "in Flash%2; it ends at the last frame.")
                    .arg(sound.name)
                    .arg(sound.loopForever ? " (it loops forever)" : "")});
      SoundPlacement placement;
      placement.cue      = index;
      placement.level    = variant.level;
      placement.start    = cue.row;
      placement.end      = cue.row + std::min(variant.frames, available);
      placement.volume   = constantVolume ? volume : 1.0;
      placement.envelope = !constantVolume            ? "baked"
                           : sound.envelope.isEmpty() ? "none"
                                                      : "volume";
      static const char *const encodings[] = {"pcm", "mp3", "adpcm"};
      placement.encoding                   = encodings[int(payload->encoding)];
      placements.push_back(placement);
    }

    // One column per source layer; overlapping sounds, or sounds at another
    // volume, continue in further columns of that layer.
    struct Column {
      int index     = -1;
      double volume = 1;
      int end       = 0;
    };
    QVector<QVector<Column>> columns(m_soundLayers.size());
    static const char *const syncNames[] = {"event", "start", "stop", "stream"};
    for (const auto &placement : placements) {
      check();
      const int rows = placement.end - placement.start;
      if (rows <= 0) continue;
      const SoundCue &cue     = m_soundCues[placement.cue];
      const SoundLayer &layer = m_soundLayers[cue.layer];
      auto &layerColumns      = columns[cue.layer];
      Column *target          = nullptr;
      for (auto &column : layerColumns)
        if (column.end <= placement.start &&
            column.volume == placement.volume) {
          target = &column;
          break;
        }
      if (!target) {
        layerColumns.push_back(
            {soundColumn(root, layer, placement.volume), placement.volume, 0});
        target = &layerColumns.back();
      }
      std::vector<TXshCell> cells;
      cells.reserve(size_t(rows));
      for (int row = 0; row < rows; ++row)
        cells.emplace_back(placement.level, TFrameId(row));
      if (!root->setCells(placement.start, target->index, rows, cells.data()))
        throw Flash::Error(cue.sourceId + ": Could not place a native sound.");
      target->end = placement.end;
      QJsonObject entry{
          {"sourceId", cue.sourceId},
          {"sourceFrameXmlId", cue.frameXmlId},
          {"sourceLayerId", layer.sourceLayerId},
          {"kind", cue.payload ? "video" : "sound"},
          {"sound", cue.sound.name},
          {"sync", syncNames[int(cue.sound.sync)]},
          {"encoding", placement.encoding},
          {"envelope", placement.envelope},
          {"nativeColumn", target->index},
          {"nativeFirstRow", placement.start},
          {"nativeRowCount", rows},
          {"nativeLevelName",
           QString::fromStdWString(placement.level->getName())},
          {"nativeFile",
           QString::fromStdWString(
               placement.level->getPath().withoutParentDir().getWideString())},
          {"nativeVolume", placement.volume}};
      if (!cue.symbol.isEmpty()) {
        // Played by a movie clip on stage, flattened onto the main timeline.
        entry["movieClip"] = cue.symbol;
        if (cue.offsetFrames)
          entry["sourceStartFrameOffset"] = cue.offsetFrames;
        ++m_clipSoundPlacements;
      }
      if (QLatin1String(placement.envelope) == QLatin1String("baked"))
        ++m_bakedEnvelopes;
      m_nativeSounds.append(entry);
    }
    // Names are applied last: placing the first cell can rename a column
    // after its level when the application links the two.
    for (int index = 0; index < columns.size(); ++index)
      for (int ordinal = 0; ordinal < columns[index].size(); ++ordinal) {
        const int column   = columns[index][ordinal].index;
        const QString name = ordinal == 0 ? m_soundLayers[index].name
                                          : m_soundLayers[index].name + " / " +
                                                QString::number(ordinal + 1);
        root->getStageObject(TStageObjectId::ColumnId(column))
            ->setName(name.toUtf8().toStdString());
        root->getColumn(column)->lock(m_soundLayers[index].locked);
      }
    root->updateFrameCount();
  }

public:
  Builder(Flash::Package &package, const Flash::DocumentInfo &info,
          const BuildOptions &options)
      : m_package(package), m_info(info), m_options(options) {
    Compatibility::options(info, options, sink());
    m_publishHiddenLayers = publishesHiddenLayers(package);
    m_result.scene        = std::make_unique<ToonzScene>();
    if (options.project) m_result.scene->setProject(options.project);
    auto *camera = m_result.scene->getCurrentCamera();
    camera->setRes({info.width, info.height});
    camera->setSize({info.width / options.dpi, info.height / options.dpi});
    m_result.scene->getProperties()->getOutputProperties()->setFrameRate(
        info.frameRate);
    m_result.scene->getProperties()->getPreviewProperties()->setFrameRate(
        info.frameRate);
  }

  BuildResult run() {
    const auto document = Flash::readDefinition(m_package, "DOMDocument.xml");
    m_geometrySegments  = document.geometrySegments;
    Compatibility::definition(document, false, sink());
    const QColor background(
        document.properties.value("backgroundColor", "#FFFFFF"));
    m_result.scene->getProperties()->setBgColor(
        TPixel32(background.red(), background.green(), background.blue()));
    Flash::Affine origin;
    origin.tx    = -m_info.width / 2.0;
    origin.ty    = -m_info.height / 2.0;
    auto *root   = m_result.scene->getTopXsheet();
    int duration = 0;
    // Frame scripts that stop or move the main timeline (see
    // Flash::timelinePlayhead): its rows play the frames the playhead shows.
    QStringList sceneProblems;
    const auto scenePlayhead =
        document.timelines.size() == 1
            ? Flash::timelinePlayhead(document.timelines.front(), true,
                                      &sceneProblems)
            : Flash::Playhead::linear(1);
    for (const auto &problem : sceneProblems)
      sink()({"scripted-playback", document.member + "/timeline0", problem});
    bool scriptedScene = false;
    if (document.timelines.size() == 1)
      for (int tick = 0;
           tick < std::max(1, document.timelines.front().frameCount); ++tick)
        scriptedScene |= scenePlayhead.frameAt(tick) != tick;
    else
      // The playhead runs through all scenes; they play back to back here.
      for (int index = 0; index < document.timelines.size(); ++index)
        if (!Flash::timelineScriptCommands(document.timelines[index], true)
                 .isEmpty())
          sink()({"scripted-playback",
                  document.member + "/timeline" + QString::number(index),
                  "Frame scripts that move the playhead are not translated "
                  "in documents with several scenes."});
    if (document.timelines.size() == 1 && !scriptedScene) {
      timeline(root, document.timelines.front(), origin,
               document.member + "/timeline0", true, false);
      duration = std::max(1, document.timelines.front().frameCount);
    } else if (scriptedScene) {
      // The scene's own timeline stays intact in a sub-xsheet; its column
      // shows the frame the playhead is on at every tick.
      const auto &source          = document.timelines.front();
      const QString sceneSourceId = document.member + "/timeline0";
      const int count             = std::max(1, source.frameCount);
      const int index = column(root, "Flash scene (scripted)", true);
      auto *level     = child(source.name);
      timeline(level->getXsheet(), source, origin, sceneSourceId, true, false);
      for (int tick = 0; tick < count; ++tick)
        cell(root, tick, index, level, scenePlayhead.frameAt(tick));
      sceneBinding(root, index, level, sceneSourceId, 0, count);
      // Movie clips on the main timeline keep playing while it holds.
      const QString reason =
          clockMismatch(m_clockRuns.value(level->getXsheet()), count, count,
                        [&](int tick) { return scenePlayhead.frameAt(tick); });
      if (!reason.isEmpty())
        sink()({"unmapped-movie-clip-clock", sceneSourceId,
                "Movie clips on the scripted main timeline play on its "
                "frames: " +
                    reason + "."});
      duration = count;
    } else {
      const int index = column(root, "Flash scenes", true);
      for (int sourceIndex = 0; sourceIndex < document.timelines.size();
           ++sourceIndex) {
        const auto &source = document.timelines[sourceIndex];
        auto *level        = child(source.name);
        m_rootRowOffset    = duration;
        timeline(level->getXsheet(), source, origin,
                 document.member + "/timeline" + QString::number(sourceIndex),
                 true, false);
        const int count = std::max(1, source.frameCount);
        if (count > std::numeric_limits<int>::max() - duration)
          throw Flash::Error(
              "Source scene duration exceeds native frame limits.");
        for (int frame = 0; frame < count; ++frame)
          cell(root, duration + frame, index, level, frame);
        sceneBinding(
            root, index, level,
            document.member + "/timeline" + QString::number(sourceIndex),
            duration, count);
        duration += count;
      }
    }
    flattenClipSounds(document, scenePlayhead, scriptedScene);
    placeSounds(root);
    if (m_options.includeUnusedSymbols)
      for (auto it = m_info.symbols.cbegin(); it != m_info.symbols.cend(); ++it)
        symbol(it.key());
    timed("vectorImage", [&] { convertQueuedVectors(); });
    applyQueuedVectors();
    reportMaskCopyIssues();
    // Fingerprint after every timeline has been built: later held keys in a
    // reused column are part of the native stage track saved with the scene.
    for (const auto &site : m_stageBindings) {
      check();
      const auto baseline = timed("stageBaseline", [&] {
        return stageTransformBaseline(site.sheet, site.column, site.first,
                                      site.count);
      });
      if (baseline.sampledPlacementSha256.isEmpty() ||
          baseline.stageKeysSha256.isEmpty())
        continue;
      auto binding = m_result.nativeSourceMap.at(site.bindingIndex).toObject();
      binding["nativeStageSamplesSha256"] = baseline.sampledPlacementSha256;
      binding["nativeStageKeysSha256"]    = baseline.stageKeysSha256;
      m_result.nativeSourceMap.replace(site.bindingIndex, binding);
    }
    m_result.scene->getProperties()->getOutputProperties()->setRange(
        0, duration - 1, 1);
    m_result.scene->getProperties()->getPreviewProperties()->setRange(
        0, duration - 1, 1);
    // Column opacity carries symbol alpha; apply it in final renders too.
    if (m_columnOpacity)
      m_result.scene->getProperties()->enableColumnColorFilterOnRender(true);
    m_result.sourceReport["nativeSoundLevels"]  = m_soundLevels;
    m_result.sourceReport["nativeSoundColumns"] = m_soundColumns;
    m_result.sourceReport["nativeVideoLevels"]  = m_result.videoLevels.size();
    m_result.sourceReport["nativeVideoFrames"]  = double(m_videoFrames);
    m_result.sourceReport["nativeSounds"]       = m_nativeSounds;
    // Sounds movie clips play (flattened onto the main timeline), and
    // volume envelopes baked into their WAV files.
    m_result.sourceReport["nativeMovieClipSounds"]     = m_clipSoundPlacements;
    m_result.sourceReport["nativeBakedSoundEnvelopes"] = m_bakedEnvelopes;
    {
      // Shape tweens built from in-between drawings, per correspondence.
      QJsonObject tweens;
      for (auto it = m_shapeTweenCounts.cbegin();
           it != m_shapeTweenCounts.cend(); ++it)
        tweens[it.key()] = it.value();
      tweens["drawings"]                         = m_shapeTweenDrawings;
      m_result.sourceReport["nativeShapeTweens"] = tweens;
    }
    m_result.paintWitnesses = m_paintWitnesses;
    for (auto it = m_profileNs.cbegin(); it != m_profileNs.cend(); ++it)
      m_result.profileMs[it.key()] = double(it.value() / 1000000);
    return std::move(m_result);
  }
};

}  // namespace

BuildResult buildScene(Flash::Package &package, const Flash::DocumentInfo &info,
                       const BuildOptions &options) {
  return Builder(package, info, options).run();
}

}  // namespace FlashNative
