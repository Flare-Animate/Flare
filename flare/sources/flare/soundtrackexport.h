#pragma once

#ifndef SOUNDTRACKEXPORT_H
#define SOUNDTRACKEXPORT_H

#include "flareqt/dvdialog.h"
#include "flare/sceneproperties.h"
#include "tsound.h"

// forward declaration
class flareScene;

//=============================================================================
// SoundtrackExport
//-----------------------------------------------------------------------------

class SoundtrackExport : public QWidget {
  Q_OBJECT

public:
  SoundtrackExport();
  bool hasSoundTrack();
  void makeSoundtrack(int r0, int r1, double fps);
  void saveSoundtrack();

protected:
  bool m_hasSoundtrack;
  TSoundTrackP m_st;
  TSoundTrack *m_sound;
  flareScene *m_scene;
  TFilePath m_fp;
};

#endif  // SOUNDTRACKEXPORT_H

