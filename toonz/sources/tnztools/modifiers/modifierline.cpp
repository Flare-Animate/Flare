

#include <tools/modifiers/modifierline.h>

//*****************************************************************************************
//    TModifierLine implementation
//*****************************************************************************************

static inline void calcFixedAngle(const TTrackPoint &p0, TTrackPoint &p1) {
  TPointD p = p1.position - p0.position;
  double l  = sqrt(p.x * p.x + p.y * p.y);
  if (l < TConsts::epsilon) {
    p = TPointD();
  } else {
    double a = atan2(p.y, p.x);
    a        = round(a * 4 / M_PI) * M_PI / 4;
    p.x      = cos(a) * l;
    p.y      = sin(a) * l;
  }
  p1.position = p0.position + p;
}

TTrackPoint TModifierLine::Modifier::calcPoint(double originalIndex) {
  if (original.empty()) return TTrackPoint();
  if (original.size() < 2) return original.front();
  TTrackPoint p0 = original.front();
  TTrackPoint p1 = original.back();
  if (fixAngle) calcFixedAngle(p0, p1);
  return TTrack::interpolationLinear(p0, p1,
                                     originalIndex / (original.size() - 1));
}

void TModifierLine::modifyTrack(const TTrack &track,
                                const TInputSavePoint::Holder &savePoint,
                                TTrackList &outTracks) {
  if (!track.handler) {
    track.handler       = new TTrackHandler(track);
    Modifier *modifier  = new Modifier(*track.handler);
    modifier->savePoint = savePoint;
    track.handler->tracks.push_back(new TTrack(modifier));
  }

  if (!track.changed()) return;
  if (track.handler->tracks.empty()) return;

  TTrack &subTrack   = *track.handler->tracks.front();
  Modifier *modifier = dynamic_cast<Modifier *>(subTrack.modifier.getPointer());
  if (!modifier) {
    track.resetChanges();
    return;
  }

  bool fixAngle = track.getKeyState(track.back()).isPressed(TKey::control);
  outTracks.push_back(track.handler->tracks.front());

  int i1             = track.size();
  int i0             = i1 - track.pointsAdded;
  double maxPressure = modifier->maxPressure;
  if (track.pointsRemoved) {
    maxPressure = 0;
    i0          = 0;
  }
  for (int i = i0; i < i1; ++i) {
    double p = track[i].pressure;
    if (maxPressure < p) maxPressure = p;
  }
  modifier->maxPressure = maxPressure;
  modifier->fixAngle    = fixAngle;
  if (track.finished()) modifier->savePoint.reset();

  subTrack.truncate(0);

  if (track.size() > 0) {
    TTrackPoint p   = track.front();
    p.originalIndex = 0;
    p.pressure      = maxPressure;
    p.tilt          = TPointD();
    subTrack.push_back(p);
  }

  if (track.size() > 1) {
    TTrackPoint p   = track.back();
    p.originalIndex = track.size() - 1;
    p.pressure      = maxPressure;
    p.tilt          = TPointD();
    if (fixAngle) calcFixedAngle(subTrack.front(), p);
    subTrack.push_back(p);
  }

  track.resetChanges();
}
