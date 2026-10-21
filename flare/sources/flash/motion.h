#pragma once

#include "effects.h"
#include "model.h"

#include <optional>

namespace Flash {

// Motion-object tweens (CS4+ "Motion Tween", Animate's default tween type).
// Animate saves one <DOMFrame tweenType="motion object"> per tween span: the
// frame's element is the state at the span's first frame, and
// <motionObjectXML><AnimationCore> holds one curve per animated property.
// Format, as Flash CS5 and Animate save it:
//
//   <AnimationCore TimeScale="24000" duration="30000">
//     <TimeMap strength="0" type="Quadratic"/>       (eases, by TimeMapIndex)
//     <metadata><Settings orientToPath="0" .../></metadata>
//     <PropertyContainer id="headContainer">
//       <PropertyContainer id="Basic_Motion">        (Motion_X/Y, Rotation_Z)
//         <Property id="Motion_X" ignoreTimeMap="0" TimeMapIndex="0">
//           <Keyframe anchor="0,0" next="dt,v" previous="dt,v" timevalue="0"/>
//       <PropertyContainer id="Transformation">      (Skew_X/Y, Scale_X/Y)
//       <PropertyContainer id="Colors"/> <PropertyContainer id="Filters"/>
//
// - timevalue is in TimeScale ticks (TimeScale = 1000 x frame rate, so a
//   frame is 1000 ticks); every property has a key at time 0.
// - anchor "0,v" is the key's value; next/previous are its Bezier handles as
//   "dt,v": a time offset from the key in ticks and an absolute value. A
//   segment is a cubic Bezier in (time, value), solved for time; a handle on
//   its key (dt = 0) makes the segment linear there. Spatial paths store the
//   path's handles in Motion_X/Y with their time handles at segment thirds.
// - A TimeMap eases a property over its first..last key, unless the property
//   says ignoreTimeMap="1". Simple (Quadratic..Quintic) and Stop and Start
//   (DualQuadratic..DualQuintic) eases are evaluated; strength -100..100,
//   positive eases out (Quadratic is fl.motion's SimpleEase t + s*t*(1-t)).
// - Motion_X/Y (pixels) move the transformation point from where the frame's
//   element puts it. Rotation_Z and Skew_X/Y (degrees, added to Flash's skew
//   angles) and Scale_X/Y (percent) rebuild the matrix (motionRelative()).
// - Orient to path is baked into Rotation_Z by Animate (per-frame keys).
struct MotionHandle {
  double dt    = 0;  // ticks from the key
  double value = 0;  // absolute
};

struct MotionKey {
  double time  = 0;  // ticks from the start of the span
  double value = 0;
  MotionHandle next, previous;
};

struct MotionEase {
  QString type    = "Quadratic";
  double strength = 0;
};

struct MotionProperty {
  QString id;
  QVector<MotionKey> keys;  // by time
  std::optional<MotionEase> ease;
};

struct MotionObject {
  double timeScale = 24000;                  // ticks per second
  QMap<QString, MotionProperty> properties;  // enabled curves by id
  // Curves the evaluation leaves out, as "<id>: <detail>": 3D (Motion_Z,
  // Rotation_X/Y), filters, unknown properties, eases without a mapping.
  QStringList unmapped3D, unmapped;
  // Colors curves (Alpha_Amount, Tint_*, Brightness_Amount, AdvClr_*): see
  // motionColor().
  bool hasColor = false;
};

bool motionObjectFrame(const Frame &frame);  // tweenType="motion object"
bool motionObjectExtra(const Extra &extra);  // <motionObjectXML>
// The frame's motion curves; nullopt without a readable <motionObjectXML>.
// Never throws for unusual values: they keep their defaults.
std::optional<MotionObject> motionObject(const Frame &frame);

// The span's time in ticks `frameOffset` frames after its first frame.
double motionTicks(const MotionObject &motion, double frameOffset,
                   double frameRate);
// The property's value at `ticks` (its first value before its first key, its
// last one after its last key).
double motionValue(const MotionProperty &property, double ticks);

// Rotation_Z, Skew_X/Y and Scale_X/Y either restate the element's own values
// (Animate's saves: the curves at time 0 equal the element's transform) or
// are relative to it (fl.motion's exported motions: identity at time 0).
// Both give the element's own matrix at the span's first frame; this picks
// the convention the curves follow. `ambiguous` is set when their time-0
// values match neither (they are then read as absolute values).
bool motionRelative(const MotionObject &motion, const Affine &base,
                    bool *ambiguous = nullptr);

struct MotionPose {
  Affine matrix;
  double skewY = 0;  // degrees, continuous (multi-turn) rotation of the pose
};
// The element's pose `frameOffset` frames into the span. `base` is the
// element's matrix, `pivot` its transformation point (local coordinates).
MotionPose motionPose(const MotionObject &motion, const Affine &base,
                      Point pivot, double frameOffset, double frameRate,
                      bool relative);
// The color effect the Colors curves give at that frame; nullopt when the
// tween has no Colors curves.
std::optional<ColorTransform> motionColor(const MotionObject &motion,
                                          double frameOffset, double frameRate);

}  // namespace Flash
