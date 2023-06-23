#pragma once

#ifndef MODIFIERSMOOTH_INCLUDED
#define MODIFIERSMOOTH_INCLUDED

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
//    TModifierSmooth definition
//*****************************************************************************************

class DVAPI TModifierSmooth: public TInputModifier {
public:
  class DVAPI Modifier: public TTrackModifier {
  public:
    const int radius;
    TTrack *smoothedTrack;

    Modifier(TTrackHandler &handler, int radius);
    TTrackPoint calcPoint(double originalIndex) override;
  };

private:
  int m_radius;
  
public:
  TModifierSmooth(int radius = 10);

  void setRadius(int radius);
  int getRadius() const { return m_radius; }
  
  void modifyTrack(
    const TTrack &track,
    TTrackList &outTracks ) override;
};


#endif

