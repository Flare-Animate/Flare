#pragma once

#ifndef MODIFIERJITTER_INCLUDED
#define MODIFIERJITTER_INCLUDED

// TnzTools includes
#include <tools/inputmanager.h>

#undef DVAPI
#undef DVVAR
#ifdef TNZTOOLS_EXPORTS
#define DVAPI DV_EXPORT_API
#define DVVAR DV_EXPORT_VAR
#else
#define DVAPI DV_IMPORT_API
#define DVVAR DV_IMPORT_VAR
#endif

//===================================================================

//*****************************************************************************************
//    TModifierJitter definition
//*****************************************************************************************

class DVAPI TModifierJitter : public TInputModifier {
public:
  typedef TSubTrackHandler Handler;

  class DVAPI Interpolator : public TTrackInterpolator {
  public:
    const unsigned int seedX;
    const unsigned int seedY;
    const double frequency;
    const double amplitude;
    Interpolator(TTrack &track, double period, double amplitude);
    TTrackPoint interpolateFromOriginal(double originalIndex);
    TTrackPoint interpolate(double index) override;
  };

public:
  double period;
  double amplitude;
  int skipFirst;
  
  TModifierJitter(
    double period = 30,
    double amplitude = 10,
    int skipFirst = 0 );

  void modifyTrack(
    const TTrack &track,
    TTrackList &outTracks ) override;
  
  void modifyTracks(
    const TTrackList &tracks,
    TTrackList &outTracks ) override;
  
  static double func(unsigned int seed, double x);
};

#endif
