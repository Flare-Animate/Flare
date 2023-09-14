
#include <tools/modifiers/modifierjitter.h>



//*****************************************************************************************
//    static functions
//*****************************************************************************************


namespace {

class Jitter {
public:
  static inline double randomNext(unsigned int &seed) {
    static const unsigned int max = 32767;
    static const double k = 1.0/max;
    seed = ((1103515245*seed + 12345) >> 16) & max;
    return seed*k;
  }

  static TPointD getPoint(unsigned int seed, int i, double *prevX = nullptr) {
    unsigned int pseed = seed^i;
    double dx = randomNext(pseed);
    double dy = randomNext(pseed)*2 - 1;
    if (dx < 0.5) {
      double px = prevX ? *prevX : getPoint(seed, i-1).x;
      px += 0.5 - i;
      if (dx < px)
        dx = randomNext(pseed)*(1 - px) + px;
    }
    return TPointD(dx + i, dy);
  }

  static inline double spline(double x, TPointD *points) {
    double x0 = points[1].x;
    double y0 = points[1].y;
    double x1 = points[2].x;
    double y1 = points[2].y;
    double t0 = (y1 - points[0].y)/(x1 - points[0].x)*(x1 - x0);
    double t1 = (points[3].y - y0)/(points[3].x - x0)*(x1 - x0);

    double l = (x - x0)/(x1 - x0);
    double ll = l*l;
    double lll = ll*l;

    return y0*( 2*lll - 3*ll + 1)
         + y1*(-2*lll + 3*ll    )
         + t0*(   lll - 2*ll + l)
         + t1*(   lll -   ll    );
  }

  static inline double func(unsigned int seed, double x) {
    int i = (int)floor(x);
    TPointD points[5];
    points[0] = getPoint(seed, i-2);
    for(int j = 1; j < 5; ++j)
      points[j] = getPoint(seed, i-2+j, &points[j-1].x);
    return spline(x, &points[ x < points[2].x ? 0 : 1 ]);
  }
};

static inline unsigned int trackSeedX(const TTrack &track) {
  unsigned int seed = track.id;
  Jitter::randomNext(seed);
  return seed;
}

static inline unsigned int trackSeedY(const TTrack &track) {
  unsigned int seed = track.id^32143;
  Jitter::randomNext(seed);
  return seed;
}

} // namespace



//*****************************************************************************************
//    TModifierJitter::Interpolator implementation
//*****************************************************************************************


TModifierJitter::Interpolator::Interpolator(TTrack &track, double period, double amplitude):
  TTrackInterpolator(track),
  seedX(trackSeedX(track)),
  seedY(trackSeedY(track)),
  frequency(fabs(period) > TConsts::epsilon ? 1/period : 0),
  amplitude(fabs(period) > TConsts::epsilon ? amplitude : 0)
  { }


TTrackPoint TModifierJitter::Interpolator::interpolateFromOriginal(double originalIndex) {
  TTrackPoint p = track.calcPointFromOriginal(originalIndex);
  double l = p.length*frequency;
  p.position.x += Jitter::func(seedX, l)*amplitude;
  p.position.y += Jitter::func(seedY, l)*amplitude;
  return p;
}


TTrackPoint TModifierJitter::Interpolator::interpolate(double index)
  { return interpolateFromOriginal(track.originalIndexByIndex(index)); }


 
//*****************************************************************************************
//    TModifierJitter implementation
//*****************************************************************************************


TModifierJitter::TModifierJitter(double period, double amplitude, int skipFirst):
  period(period), amplitude(amplitude), skipFirst(skipFirst) { }


void TModifierJitter::modifyTrack(const TTrack &track,
                                  TTrackList &outTracks)
{
  if (!track.handler && fabs(period) > TConsts::epsilon) {
    Handler *handler = new Handler();
    track.handler = handler;
    handler->track = new TTrack(track);
    new Interpolator(*handler->track, period, amplitude);
  }
  
  Handler *handler = dynamic_cast<Handler*>(track.handler.getPointer());
  if (!handler)
    return TInputModifier::modifyTrack(track, outTracks);
  
  outTracks.push_back(handler->track);
  TTrack &subTrack = *handler->track;
  
  if (!track.changed())
    return;

  Interpolator *intr = dynamic_cast<Interpolator*>(subTrack.getInterpolator().getPointer());
  if (!intr)
    return;

  int start = track.size() - track.pointsAdded;
  if (start < 0) start = 0;

  // process sub-track
  subTrack.truncate(start);
  for (int i = start; i < track.size(); ++i)
    subTrack.push_back(intr->interpolateFromOriginal(i), false);
  
  // fit points
  subTrack.fix_to(track.fixedSize());
  
  track.resetChanges();
}


void
TModifierJitter::modifyTracks(
    const TTrackList &tracks,
    TTrackList &outTracks )
{
  int cnt = std::min( std::max(0, skipFirst), (int)tracks.size() );
  TTrackList::const_iterator split = tracks.begin() + cnt;
  for(TTrackList::const_iterator i = tracks.begin(); i != split; ++i)
    TInputModifier::modifyTrack(**i, outTracks);
  for(TTrackList::const_iterator i = split; i != tracks.end(); ++i)
    modifyTrack(**i, outTracks);
}


double TModifierJitter::func(unsigned int seed, double x)
  { return Jitter::func(seed, x); }

  
