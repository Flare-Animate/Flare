

#include <tools/modifiers/modifierassistants.h>

// TnzTools includes
#include <tools/tool.h>

// TnzLib includes
#include <toonz/tapplication.h>
#include <toonz/txshlevelhandle.h>
#include <toonz/txsheethandle.h>
#include <toonz/txsheet.h>
#include <toonz/tframehandle.h>
#include <toonz/tobjecthandle.h>
#include <toonz/dpiscale.h>

// TnzCore includes
#include <tgl.h>
#include <tmetaimage.h>


//*****************************************************************************************
//    TModifierAssistants::Modifier implementation
//*****************************************************************************************


TModifierAssistants::Modifier::Modifier(TTrackHandler &handler):
  TTrackModifier(handler),
  initialized()
{ }


TTrackPoint
TModifierAssistants::Modifier::calcPoint(double originalIndex) {
  TTrackPoint p = TTrackModifier::calcPoint(originalIndex);
  return guidelines.empty() ? p : guidelines.front()->smoothTransformPoint(p);
}


//*****************************************************************************************
//    TModifierAssistants implementation
//*****************************************************************************************


TModifierAssistants::TModifierAssistants(bool drawOnly):
  drawOnly(drawOnly),
  sensitiveLength(50.0) { }


bool
TModifierAssistants::scanAssistants(
  const TPointD *positions,
  int positionsCount,
  TGuidelineList *outGuidelines,
  bool draw,
  bool enabledOnly ) const
{
  bool found = false;
  if (TInputManager *manager = getManager())
  if (TInputHandler *handler = manager->getHandler())
  if (TTool *tool = handler->inputGetTool())
  if (TToolViewer *viewer = tool->getViewer())
  if (TApplication *application = tool->getApplication())
  if (TXshLevelHandle *levelHandle = application->getCurrentLevel())
  if (TXshLevel *level = levelHandle->getLevel())
  if (TXshSimpleLevel *simpleLevel = level->getSimpleLevel())
  if (TFrameHandle *frameHandle = application->getCurrentFrame())
  if (TXsheetHandle *XsheetHandle = application->getCurrentXsheet())
  if (TXsheet *Xsheet = XsheetHandle->getXsheet())
  {
    TPointD dpiScale = getCurrentDpiScale(simpleLevel, tool->getCurrentFid());
    bool findGuidelines = (positions && positionsCount > 0 && outGuidelines);
    bool doSomething = findGuidelines || draw;

    int frame = frameHandle->getFrame();
    int count = Xsheet->getColumnCount();
    TAffine worldToTrack;
    worldToTrack.a11 /= dpiScale.x;
    worldToTrack.a22 /= dpiScale.y;

    for(int i = 0; i < count; ++i)
      if (TImageP image = Xsheet->getCell(frame, i).getImage(false))
      if (image->getType() == TImage::META)
      if (TMetaImage *metaImage = dynamic_cast<TMetaImage*>(image.getPointer()))
      {
        TAffine imageToTrack = worldToTrack * tool->getColumnMatrix(i);
        if (draw) { glPushMatrix(); tglMultMatrix(imageToTrack); }

        TMetaImage::Reader reader(*metaImage);
        for(TMetaObjectListCW::iterator i = reader->begin(); i != reader->end(); ++i)
          if (*i)
          if (const TAssistant *assistant = (*i)->getHandler<TAssistant>())
          if (!enabledOnly || assistant->getEnabled())
          {
            found = true;
            if (findGuidelines)
              for(int i = 0; i < positionsCount; ++i)
                assistant->getGuidelines(positions[i], imageToTrack, *outGuidelines);
            if (draw) assistant->draw(viewer);
            if (!doSomething) return true;
          }

        if (draw) glPopMatrix();
      }
  }
  return found;
}


void
TModifierAssistants::modifyTrack(
  const TTrack &track,
  const TInputSavePoint::Holder &savePoint,
  TTrackList &outTracks )
{
  if (!track.handler) {
    track.handler = new TTrackHandler(track);
    Modifier *modifier = new Modifier(*track.handler);
    if (!drawOnly)
      scanAssistants(&track[0].position, 1, &modifier->guidelines, false, true);

    track.handler->tracks.push_back(new TTrack(modifier));

    if ((int)modifier->guidelines.size() > 1) {
      modifier->savePoint = savePoint;
      outTracks.push_back(track.handler->tracks.front());
      return;
    }
  }

  outTracks.push_back(track.handler->tracks.front());
  TTrack &subTrack = *track.handler->tracks.front();
  if (!track.changed()) return;
  if (Modifier *modifier = dynamic_cast<Modifier*>(subTrack.modifier.getPointer())) {
    // remove points
    int start = track.size() - track.pointsAdded;
    if (start < 0) start = 0;

    if ((int)modifier->guidelines.size() > 1 && modifier->savePoint.available()) {
      // select guideline
      bool longEnough = false;
      if (TInputManager *manager = getManager())
      if (TInputHandler *handler = manager->getHandler())
      if (TTool *tool = handler->inputGetTool())
      if (TToolViewer *viewer = tool->getViewer()) {
        TAffine trackToScreen = tool->getMatrix();
        if (tool->getToolType() & TTool::LevelTool)
          if (TObjectHandle *objHandle = TTool::getApplication()->getCurrentObject())
            if (!objHandle->isSpline())
              trackToScreen *= TScale(viewer->getDpiScale().x, viewer->getDpiScale().y);
        trackToScreen *= viewer->get3dViewMatrix().get2d().inv();
        TGuidelineP guideline = TGuideline::findBest(modifier->guidelines, track, trackToScreen, longEnough);
        if (guideline != modifier->guidelines.front())
          for(int i = 1; i < (int)modifier->guidelines.size(); ++i)
            if (modifier->guidelines[i] == guideline) {
              std::swap(modifier->guidelines[i], modifier->guidelines.front());
              start = 0;
              break;
            }
      }
      modifier->savePoint.setLock(!longEnough);
    } else {
      modifier->savePoint.reset();
    }

    // add points
    subTrack.truncate(start);
    for(int i = start; i < track.size(); ++i)
      subTrack.push_back( modifier->calcPoint(i) );
  }
  track.resetChanges();
}


TRectD
TModifierAssistants::calcDrawBounds(const TTrackList&, const THoverList&) {
  if (scanAssistants(NULL, 0, NULL, false, false))
    return TConsts::infiniteRectD;
  return TRectD();
}


void
TModifierAssistants::drawTrack(const TTrack &track) {
  if (!track.handler) return;
  TTrack &subTrack = *track.handler->tracks.front();
  if (Modifier *modifier = dynamic_cast<Modifier*>(subTrack.modifier.getPointer())) {
    const TGuidelineList &guidelines = modifier->guidelines;
    if (!guidelines.empty()) {
      guidelines.front()->draw(true);
      for(TGuidelineList::const_iterator i = guidelines.begin() + 1; i != guidelines.end(); ++i)
        (*i)->draw();
    }
  }
}


void
TModifierAssistants::draw(const TTrackList &tracks, const THoverList &hovers) {
  THoverList allHovers;
  allHovers.reserve(hovers.size() + tracks.size());
  allHovers.insert(allHovers.end(), hovers.begin(), hovers.end());
  for(TTrackList::const_iterator i = tracks.begin(); i != tracks.end(); ++i)
    if ((*i)->handler && !(*i)->handler->tracks.empty() && !(*i)->handler->tracks.front()->empty())
      allHovers.push_back( (*i)->handler->tracks.front()->back().position );
  
  // draw assistants
  TGuidelineList guidelines;
  scanAssistants(
    allHovers.empty() ? NULL : &allHovers.front(),
    (int)allHovers.size(),
    &guidelines,
    true,
    false );

  // draw guidelines
  for(TGuidelineList::const_iterator i = guidelines.begin(); i != guidelines.end(); ++i)
    (*i)->draw();

  // draw tracks
  TInputModifier::drawTracks(tracks);
}
