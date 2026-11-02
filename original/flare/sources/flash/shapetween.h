#pragma once

#include "model.h"

#include <optional>

namespace Flash {

// Animate's own shape-tween correspondence, saved on the first keyframe of a
// shape tween (DOMFrame/MorphShape). Each segment is one contour of the start
// (A) and end (B) artwork, split into the same number of quadratic curves.
// Points are in the layer's space (model pixels, downward Y), after each
// shape element's matrix; XFL stores them in units of 256 twips. A filled
// segment has its fill on the right of its direction of travel, so holes run
// the other way round; the other segments are strokes. The style indices of
// the XML refer to a table that is not saved, so the builder finds each
// segment's paints on the keyframe shapes instead.
struct MorphCurve {
  Point controlA, anchorA, controlB, anchorB;
  bool line = false;
};

struct MorphSegment {
  Point startA, startB;
  bool filled = false;
  QVector<MorphCurve> curves;
};

// A shape hint: matching points on the start and end artwork.
struct MorphHint {
  Point start, end;
};

struct ShapeMorph {
  QVector<MorphSegment> segments;
  QVector<MorphHint> hints;
};

// The frame's MorphShape and shape hints (frame-level morphHintsList), or
// nullopt without a MorphShape. Malformed data also gives nullopt, with
// `problem` set. Never throws.
std::optional<ShapeMorph> shapeMorph(const Frame &frame, QString &problem);
// True for the frame extras of a shape tween that shapeMorph() evaluates.
bool shapeMorphExtra(const Extra &extra);

// How a shape tween's in-betweens correspond to its keyframes.
enum class ShapeTweenMethod {
  Morph,      // Animate's saved correspondence (MorphShape): exact
  Structure,  // no usable MorphShape; identical edge structure, point by point
  Resampled   // contours matched by style and position, resampled by arc length
};

// The tween of one shape element of a shape-tween keyframe toward the next
// keyframe of the same layer.
class ShapeTween {
public:
  // A style of one keyframe shape: shape is the ordinal among the shape
  // elements of the start keyframe (or of the end keyframe when `end`),
  // index the XFL style index (0 = none).
  struct StyleRef {
    int shape = -1;
    int index = 0;
    bool end  = false;
    bool valid() const { return shape >= 0 && index > 0; }
    bool operator<(const StyleRef &other) const {
      if (end != other.end) return end < other.end;
      return shape != other.shape ? shape < other.shape : index < other.index;
    }
  };
  // One drawable path with equal start and end segment counts, in layer
  // space. Fills follow XFL: fill0 on the left, fill1 on the right.
  struct Path {
    QVector<Segment> a, b;
    StyleRef fill0A, fill0B, fill1A, fill1B, strokeA, strokeB;
  };

  ShapeTweenMethod method = ShapeTweenMethod::Morph;
  // Differences from Animate's in-betweens, for the import report.
  QStringList approximations;

  // The in-between at eased progress in (0, 1), in the start element's local
  // coordinates (its matrix stays on the element's placement).
  Shape at(double progress) const;

private:
  friend class ShapeTweenPlanner;
  QVector<Shape> m_shapesA, m_shapesB;
  QVector<Affine> m_matricesA, m_matricesB;
  Affine m_toLocal;
  QVector<Path> m_paths;
};

// Plans the tweens of the shape elements of `start` (a frame with
// tweenType="shape") toward `end`, the next keyframe of its layer. Returns
// one entry per element of start.elements: nullopt for elements that are not
// shapes, and for every element when `end` has no shape (Animate's broken
// tween, which holds the keyframe too). `notes` receives the start shapes
// that hold although Animate would tween them (a singular matrix, no
// counterpart without a MorphShape).
QVector<std::optional<ShapeTween>> planShapeTweens(const Frame &start,
                                                   const Frame &end,
                                                   QStringList &notes);

}  // namespace Flash
