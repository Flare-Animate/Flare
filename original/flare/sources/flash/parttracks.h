#pragma once

#include "model.h"

#include <QHash>
#include <QSet>

#include <algorithm>

namespace Flash {

struct PartTrackPlan {
  // Indexed by Layer::frames, then Frame::elements. A track is a native
  // column destination, not an assertion that anonymous artwork has a stable
  // authored identity across keyframes.
  QVector<QVector<int>> frameTracks;
  // Nonempty only for tracks matched by a unique authored instance name.
  QVector<QString> trackNames;
  int trackCount = 0;
};

inline bool hasNamedPartIdentity(const Element &element) {
  return (element.kind == Element::Kind::Symbol ||
          element.kind == Element::Kind::Bitmap) &&
         !element.properties.value("name").isEmpty();
}

// Flash instance names can identify an editable part within one source layer.
// A repeated name on the same keyframe is ambiguous, so all occurrences of
// that name keep the element-slot destination. Other unidentifiable
// elements also retain slot tracks until their source continuity is known.
// Mask inputs keep slot tracks because their current native FX graph has a
// fixed internal composition order.
inline PartTrackPlan planLayerTracks(const Layer &layer,
                                     bool preserveSlotOrder) {
  PartTrackPlan plan;
  plan.frameTracks.reserve(layer.frames.size());
  int maxSlots = 0;
  for (const auto &frame : layer.frames)
    maxSlots = std::max(maxSlots, int(frame.elements.size()));

  if (preserveSlotOrder) {
    plan.trackCount = maxSlots;
    plan.trackNames.resize(maxSlots);
    for (const auto &frame : layer.frames) {
      QVector<int> tracks;
      tracks.reserve(frame.elements.size());
      for (int slot = 0; slot < frame.elements.size(); ++slot)
        tracks.push_back(slot);
      plan.frameTracks.push_back(std::move(tracks));
    }
    return plan;
  }

  QSet<QString> ambiguousNames;
  for (const auto &frame : layer.frames) {
    QSet<QString> seen;
    for (const auto &element : frame.elements) {
      if (!hasNamedPartIdentity(element)) continue;
      const QString name = element.properties.value("name");
      if (seen.contains(name)) ambiguousNames.insert(name);
      seen.insert(name);
    }
  }

  // Reserve slot destinations only where an unnamed or ambiguous occurrence
  // uses them. Named tracks then occupy distinct column destinations.
  QSet<int> fallbackSlots;
  for (const auto &frame : layer.frames)
    for (int slot = 0; slot < frame.elements.size(); ++slot) {
      const auto &element = frame.elements[slot];
      if (!hasNamedPartIdentity(element) ||
          ambiguousNames.contains(element.properties.value("name")))
        fallbackSlots.insert(slot);
    }
  QHash<int, int> slotTracks;
  for (int slot = 0; slot < maxSlots; ++slot)
    if (fallbackSlots.contains(slot)) {
      slotTracks.insert(slot, plan.trackCount++);
      plan.trackNames.push_back(QString());
    }

  QHash<QString, int> namedTracks;
  for (const auto &frame : layer.frames) {
    QVector<int> tracks;
    tracks.reserve(frame.elements.size());
    for (int slot = 0; slot < frame.elements.size(); ++slot) {
      const auto &element = frame.elements[slot];
      const QString name  = element.properties.value("name");
      if (!hasNamedPartIdentity(element) || ambiguousNames.contains(name)) {
        tracks.push_back(slotTracks.value(slot));
      } else {
        if (!namedTracks.contains(name)) {
          namedTracks.insert(name, plan.trackCount++);
          plan.trackNames.push_back(name);
        }
        tracks.push_back(namedTracks.value(name));
      }
    }
    plan.frameTracks.push_back(std::move(tracks));
  }
  return plan;
}

}  // namespace Flash
