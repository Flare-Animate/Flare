

#include <tools/modifiers/modifiersmooth.h>
#include <algorithm>


//*****************************************************************************************
//    TModifierSmooth::Modifier implementation
//*****************************************************************************************


TModifierSmooth::Modifier::Modifier(TTrackHandler &handler, int radius):
  TTrackModifier(handler),
  radius(std::max(radius, 0)),
  smoothedTrack()
{ }


TTrackPoint
TModifierSmooth::Modifier::calcPoint(double originalIndex) {
  return smoothedTrack
       ? smoothedTrack->interpolateLinear(originalIndex)
       : TTrackModifier::calcPoint(originalIndex);
}


//*****************************************************************************************
//    TModifierSmooth implementation
//*****************************************************************************************


TModifierSmooth::TModifierSmooth(int radius): m_radius()
  { setRadius(radius); }


void
TModifierSmooth::setRadius(int radius)
  { m_radius = std::max(0, radius); }


void
TModifierSmooth::modifyTrack(
  const TTrack &track,
  TTrackList &outTracks )
{
  if (!m_radius) {
    TInputModifier::modifyTrack(track, outTracks);
    return;
  }
  
  if (!track.handler) {
    track.handler = new TTrackHandler(track);
    Modifier *modifier = new Modifier(*track.handler, m_radius);
    modifier->smoothedTrack = new TTrack(modifier);
    track.handler->tracks.push_back(modifier->smoothedTrack);
  }

  if (track.handler->tracks.empty())
    return;

  TTrack &subTrack = *track.handler->tracks.front();
  outTracks.push_back(track.handler->tracks.front());

  if (!track.changed())
    return;
  
  Modifier *modifier = dynamic_cast<Modifier*>(subTrack.modifier.getPointer());
  if (!modifier)
    return;
  int radius = modifier->radius;

  // remove points
  int start = std::max(0, track.size() - track.pointsAdded);
  subTrack.truncate(start);

  // add points
  // extra points will be added at the beginning and at the end
  int size = track.size() + 2*radius;
  double k = 1.0/(2*radius + 1);
  TTrackTangent accum;
  for(int i = start - 2*radius; i < size; ++i) {
    
    const TTrackPoint &p1 = track[i];
    accum.position += p1.position;
    accum.pressure += p1.pressure;
    accum.tilt     += p1.tilt;
    if (i < start)
      continue;

    int oi = track.clampIndex(i - radius);
    const TTrackPoint &p = track[oi];
    subTrack.push_back(
      TTrackPoint(
        accum.position*k,
        accum.pressure*k,
        accum.tilt*k,
        oi,
        p.time,
        0,
        p.final ),
      false );
    
    const TTrackPoint &p0 = track[i - 2*radius];
    accum.position -= p0.position;
    accum.pressure -= p0.pressure;
    accum.tilt     -= p0.tilt;
  }
  
  // fix points
  if (track.fixedFinished()) {
    subTrack.fix_all();
  } else
  if (track.fixedSize()) {
    subTrack.fix_to(track.fixedSize());
  }
  
  track.resetChanges();
}

