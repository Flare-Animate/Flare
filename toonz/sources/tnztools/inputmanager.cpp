

#include <tools/inputmanager.h>

// TnzCore includes
#include <tgl.h>


//*****************************************************************************************
//    static members
//*****************************************************************************************

static const bool debugInputManager = false;
TInputState::TouchId TInputManager::m_lastTouchId = 0;


//*****************************************************************************************
//    TInputModifier implementation
//*****************************************************************************************


void
TInputModifier::setManager(TInputManager *manager) {
  if (m_manager != manager)
    { m_manager = manager; onSetManager(); }
}


void
TInputModifier::modifyTrack(
  const TTrack &track,
  const TInputSavePoint::Holder &savePoint,
  TTrackList &outTracks )
{
  if (!track.handler) {
      track.handler = new TTrackHandler(track);
      track.handler->tracks.push_back(
        new TTrack(
          new TTrackModifier(*track.handler) ));
  }

  outTracks.insert(
    outTracks.end(),
    track.handler->tracks.begin(),
    track.handler->tracks.end() );
  if (!track.changed())
    return;

  int start = std::max(0, track.size() - track.pointsAdded);
  for(TTrackList::const_iterator ti = track.handler->tracks.begin(); ti != track.handler->tracks.end(); ++ti) {
    TTrack &subTrack = **ti;
    subTrack.truncate(start);
    for(int i = start; i < track.size(); ++i)
      subTrack.push_back( subTrack.modifier->calcPoint(i) );
  }
  track.resetChanges();
}


void
TInputModifier::modifyTracks(
    const TTrackList &tracks,
    const TInputSavePoint::Holder &savePoint,
    TTrackList &outTracks )
{
  for(TTrackList::const_iterator i = tracks.begin(); i != tracks.end(); ++i)
    modifyTrack(**i, savePoint, outTracks);
}


void
TInputModifier::modifyHover(
  const TPointD &hover,
  THoverList &outHovers )
{
  outHovers.push_back(hover);
}


void
TInputModifier::modifyHovers(
  const THoverList &hovers,
  THoverList &outHovers )
{
  for(THoverList::const_iterator i = hovers.begin(); i != hovers.end(); ++i)
    modifyHover(*i, outHovers);
}


TRectD
TInputModifier::calcDrawBounds(const TTrackList &tracks, const THoverList &hovers) {
  TRectD bounds;
  for(TTrackList::const_iterator i = tracks.begin(); i != tracks.end(); ++i)
    bounds += calcDrawBoundsTrack(**i);
  for(std::vector<TPointD>::const_iterator i = hovers.begin(); i != hovers.end(); ++i)
    bounds += calcDrawBoundsHover(*i);
  return bounds;
}


void
TInputModifier::drawTracks(const TTrackList &tracks) {
  for(TTrackList::const_iterator i = tracks.begin(); i != tracks.end(); ++i)
    drawTrack(**i);
}


void
TInputModifier::drawHovers(const std::vector<TPointD> &hovers) {
  for(std::vector<TPointD>::const_iterator i = hovers.begin(); i != hovers.end(); ++i)
    drawHover(*i);
}


void
TInputModifier::draw(const TTrackList &tracks, const std::vector<TPointD> &hovers) {
  drawTracks(tracks);
  drawHovers(hovers);
}


//*****************************************************************************************
//    TInputHandler implementation
//*****************************************************************************************


bool
TInputHandler::inputKeyEvent(
  bool press,
  TInputState::Key key,
  QKeyEvent *event,
  const TInputManager &manager )
{
  return press && event && inputKeyDown(event);
}


void
TInputHandler::inputButtonEvent(
  bool press,
  TInputState::DeviceId device,
  TInputState::Button button,
  const TInputManager &manager )
{
  if (press && button == Qt::RightButton && !manager.getOutputHovers().empty())
    inputRightButtonDown(manager.getOutputHovers().front(), manager.state);
}


void
TInputHandler::inputHoverEvent(const TInputManager &manager) {
  if (!manager.getOutputHovers().empty())
    inputMouseMove(manager.getOutputHovers().front(), manager.state);
}


void
TInputHandler::inputPaintTrackPoint(const TTrackPoint &point, const TTrack &track, bool firstTrack) {
  if (firstTrack) {
    if (track.pointsAdded == track.size())
      inputLeftButtonDown(point, track);
    else
    if (point.final)
      inputLeftButtonUp(point, track);
    else
      inputLeftButtonDrag(point, track);
   }
}


void
TInputHandler::inputPaintTracks(const TTrackList &tracks) {
  // paint track points in chronological order
  while(true) {
    TTrackP track;
    TTimerTicks minTicks = 0;
    double minTimeOffset = 0.0;
    for(TTrackList::const_iterator i = tracks.begin(); i != tracks.end(); ++i) {
      const TTrack &t = **i;
      if (t.pointsAdded > 0) {
        TTimerTicks ticks = t.ticks();
        double timeOffset = t.timeOffset() + t.current().time;
        if (!track || (ticks - minTicks)*TToolTimer::frequency + timeOffset - minTimeOffset < 0.0) {
          track = *i;
          minTicks = ticks;
          minTimeOffset = timeOffset;
        }
      }
    }
    if (!track) break;
    inputPaintTrackPoint(track->current(), *track, track == tracks.front());
    --track->pointsAdded;
  }
}


//*****************************************************************************************
//    TInputManager implementation
//*****************************************************************************************


TInputManager::TInputManager():
  m_lastTicks(TToolTimer::ticks()),
  m_handler(),
  m_tracks(1),
  m_hovers(1),
  m_started(),
  m_savePointsSent()
{ }


void
TInputManager::paintRollbackTo(int saveIndex, TTrackList &subTracks) {
  if (saveIndex >= (int)m_savePoints.size())
    return;

  int level = saveIndex + 1;
  if (level <= m_savePointsSent) {
    if (m_handler) {
      if (level < m_savePointsSent)
        m_handler->inputPaintPop(m_savePointsSent - level);
      m_handler->inputPaintCancel();
    }
    m_savePointsSent = level;
  }

  for(TTrackList::const_iterator i = subTracks.begin(); i != subTracks.end(); ++i) {
    TTrack &track = **i;
    if (TrackHandler *handler = dynamic_cast<TrackHandler*>(track.handler.getPointer())) {
      handler->saves.resize(level);
      int cnt = handler->saves[saveIndex];
      track.resetRemoved();
      track.pointsAdded = track.size() - cnt;
    }
  }
  for(int i = level; i < (int)m_savePoints.size(); ++i)
    m_savePoints[i].savePoint()->available = false;
  m_savePoints.resize(level);
}


void
TInputManager::paintApply(int count, TTrackList &subTracks) {
  if (count <= 0)
    return;

  int level = (int)m_savePoints.size() - count;
  bool resend = true;

  if (level < m_savePointsSent) {
    // apply
    int applied = m_handler ? m_handler->inputPaintApply(m_savePointsSent - level) : false;
    applied = std::max(0, std::min(m_savePointsSent - level, applied));
    m_savePointsSent -= applied;
    if (m_savePointsSent == level) resend = false;
  }

  if (level < m_savePointsSent) {
    // rollback
    if (m_handler) m_handler->inputPaintPop(m_savePointsSent - level);
    m_savePointsSent = level;
  }

  // remove keypoints
  for(TTrackList::const_iterator i = subTracks.begin(); i != subTracks.end(); ++i) {
    TTrack &track = **i;
    if (TrackHandler *handler = dynamic_cast<TrackHandler*>(track.handler.getPointer())) {
      if (resend) {
        track.resetRemoved();
        track.pointsAdded = track.size() - handler->saves[m_savePointsSent];
      }
      handler->saves.resize(level);
    }
  }
  for(int i = level; i < (int)m_savePoints.size(); ++i)
    m_savePoints[i].savePoint()->available = false;
  m_savePoints.resize(level);
}


void
TInputManager::paintTracks() {
  bool allFinished = true;
  for(TTrackList::const_iterator i = m_tracks.front().begin(); i != m_tracks.front().end(); ++i)
    if (!(*i)->finished())
      { allFinished = false; break; }

  while(true) {
    // run modifiers
    TInputSavePoint::Holder newSavePoint = TInputSavePoint::create(true);
    for(int i = 0; i < (int)m_modifiers.size(); ++i) {
      m_tracks[i+1].clear();
      m_modifiers[i]->modifyTracks(m_tracks[i], newSavePoint, m_tracks[i+1]);
    }
    TTrackList &subTracks = m_tracks.back();

    // is paint started?
    if (!m_started && !subTracks.empty()) {
      m_started = true;
      if (m_handler) m_handler->inputSetBusy(true);
    }

    // create handlers
    for(TTrackList::const_iterator i = subTracks.begin(); i != subTracks.end(); ++i)
      if (!(*i)->handler)
        (*i)->handler = new TrackHandler(**i, (int)m_savePoints.size());

    if (!m_savePoints.empty()) {
      // rollback
      int rollbackIndex = (int)m_savePoints.size();
      for(TTrackList::const_iterator i = subTracks.begin(); i != subTracks.end(); ++i) {
        TTrack &track = **i;
        if (track.pointsRemoved > 0) {
          int count = track.size() - track.pointsAdded;
          if (TrackHandler *handler = dynamic_cast<TrackHandler*>(track.handler.getPointer()))
            while(rollbackIndex > 0 && (rollbackIndex >= (int)m_savePoints.size() || handler->saves[rollbackIndex] > count))
              --rollbackIndex;
        }
      }
      paintRollbackTo(rollbackIndex, subTracks);

      // apply
      int applyCount = 0;
      while(applyCount < (int)m_savePoints.size() && m_savePoints[(int)m_savePoints.size() - applyCount - 1].isFree())
        ++applyCount;
      paintApply(applyCount, subTracks);
    }

    // send to handler
    if (m_savePointsSent == (int)m_savePoints.size() && !subTracks.empty() && m_handler)
      m_handler->inputPaintTracks(subTracks);
    for(TTrackList::const_iterator i = subTracks.begin(); i != subTracks.end(); ++i)
      (*i)->resetChanges();

    // is paint finished?
    newSavePoint.unlock();
    if (newSavePoint.isFree()) {
      newSavePoint.savePoint()->available = false;
      if (allFinished) {
        paintApply((int)m_savePoints.size(), subTracks);
        // send to tool final
        if (!subTracks.empty()) {
          if (m_handler) m_handler->inputPaintTracks(subTracks);
          for(TTrackList::const_iterator i = subTracks.begin(); i != subTracks.end(); ++i)
            (*i)->resetChanges();
        }
        for(std::vector<TTrackList>::iterator i = m_tracks.begin(); i != m_tracks.end(); ++i)
          i->clear();
        if (m_started) {
          if (m_handler) m_handler->inputSetBusy(false);
          m_started = false;
        }
      }
      break;
    }

    // create save point
    if (m_handler && m_handler->inputPaintPush()) ++m_savePointsSent;
    m_savePoints.push_back(newSavePoint);
    for(TTrackList::const_iterator i = subTracks.begin(); i != subTracks.end(); ++i)
      if (TrackHandler *handler = dynamic_cast<TrackHandler*>((*i)->handler.getPointer()))
        handler->saves.push_back((*i)->size());
  }
}


int
TInputManager::trackCompare(
  const TTrack &track,
  TInputState::DeviceId deviceId,
  TInputState::TouchId touchId ) const
{
  if (track.deviceId < deviceId) return -1;
  if (deviceId < track.deviceId) return  1;
  if (track.touchId < touchId) return -1;
  if (touchId < track.touchId) return 1;
  return 0;
}


const TTrackP&
TInputManager::createTrack(
  int index,
  TInputState::DeviceId deviceId,
  TInputState::TouchId touchId,
  TTimerTicks ticks,
  bool hasPressure,
  bool hasTilt )
{
  TTrackP track = new TTrack(
    deviceId,
    touchId,
    state.keyHistoryHolder(ticks),
    state.buttonHistoryHolder(deviceId, ticks),
    hasPressure,
    hasTilt );
  return *m_tracks.front().insert(m_tracks[0].begin() + index, track);
}


const TTrackP&
TInputManager::getTrack(
  TInputState::DeviceId deviceId,
  TInputState::TouchId touchId,
  TTimerTicks ticks,
  bool hasPressure,
  bool hasTilt )
{
  TTrackList &origTracks = m_tracks.front();
  if (origTracks.empty())
    return createTrack(0, deviceId, touchId, ticks, hasPressure, hasTilt);
  int cmp;

  int a = 0;
  cmp = trackCompare(*origTracks[a], deviceId, touchId);
  if (cmp == 0) return origTracks[a];
  if (cmp < 0) return createTrack(a, deviceId, touchId, ticks, hasPressure, hasTilt);

  int b = (int)origTracks.size() - 1;
  cmp = trackCompare(*origTracks[b], deviceId, touchId);
  if (cmp == 0) return origTracks[b];
  if (cmp > 0) return createTrack(b+1, deviceId, touchId, ticks, hasPressure, hasTilt);

  // binary search: tracks[a] < tracks[c] < tracks[b]
  while(true) {
    int c = (a + b)/2;
    if (a == c) break;
    cmp = trackCompare(*origTracks[c], deviceId, touchId);
    if (cmp < 0) b = c; else
      if (cmp > 0) a = c; else
        return origTracks[c];
  }
  return createTrack(b, deviceId, touchId, ticks, hasPressure, hasTilt);
}


void
TInputManager::addTrackPoint(
  const TTrackP& track,
  const TPointD &position,
  double pressure,
  const TPointD &tilt,
  double time,
  bool final )
{
  track->push_back( TTrackPoint(
    position,
    pressure,
    tilt,
    (double)track->size(),
    time,
    0.0, // length will calculated inside of TTrack::push_back
    final ));
}


void
TInputManager::touchTracks(bool finish) {
  for(TTrackList::const_iterator i = m_tracks.front().begin(); i != m_tracks.front().end(); ++i) {
    if (!(*i)->finished() && (*i)->size() > 0) {
      const TTrackPoint &p = (*i)->back();
      addTrackPoint(*i, p.position, p.pressure, p.tilt, fixTicks(m_lastTicks)*TToolTimer::step, finish);
    }
  }
}


void
TInputManager::modifierActivate(const TInputModifierP &modifier) {
  modifier->setManager(this);
  modifier->activate();
}


void
TInputManager::modifierDeactivate(const TInputModifierP &modifier) {
  modifier->deactivate();
  modifier->setManager(NULL);
}


void
TInputManager::processTracks() {
  paintTracks();
  if (m_handler) {
    TRectD bounds = calcDrawBounds();
    if (!bounds.isEmpty()) {
      m_handler->inputInvalidateRect(m_prevBounds + bounds);
      m_nextBounds += bounds;
    }
  }
}


void
TInputManager::finishTracks() {
  touchTracks(true);
  processTracks();
}


void
TInputManager::reset() {
  // forget about handler paint stack
  // assuime it was already reset by outside
  m_started = false;
  m_savePointsSent = 0;

  // reset save point
  for(int i = 0; i < (int)m_savePoints.size(); ++i)
    m_savePoints[i].savePoint()->available = false;
  m_savePoints.clear();

  // reset tracks
  for(int i = 0; i < (int)m_tracks.size(); ++i)
    m_tracks[i].clear();
}


void
TInputManager::setHandler(TInputHandler *handler) {
  if (m_handler == handler) return;
  finishTracks();
  m_handler = handler;
}


int
TInputManager::findModifier(const TInputModifierP &modifier) const {
  for(int i = 0; i < getModifiersCount(); ++i)
    if (getModifier(i) == modifier)
      return i;
  return -1;
}


void
TInputManager::insertModifier(int index, const TInputModifierP &modifier) {
  if (findModifier(modifier) >= 0) return;
  finishTracks();
  m_modifiers.insert(m_modifiers.begin() + index, modifier);
  m_tracks.insert(m_tracks.begin() + index + 1, TTrackList());
  m_hovers.insert(m_hovers.begin() + index + 1, THoverList());
  modifierActivate(modifier);
}


void
TInputManager::removeModifier(int index) {
  if (index >= 0 && index < getModifiersCount()) {
    finishTracks();
    modifierDeactivate(m_modifiers[index]);
    m_modifiers.erase(m_modifiers.begin() + index);
    m_tracks.erase(m_tracks.begin() + index + 1);
    m_hovers.erase(m_hovers.begin() + index + 1);
  }
}


void
TInputManager::clearModifiers() {
  while(getModifiersCount() > 0)
    removeModifier(getModifiersCount() - 1);
}


void
TInputManager::trackEvent(
  TInputState::DeviceId deviceId,
  TInputState::TouchId touchId,
  const TPointD &position,
  const double *pressure,
  const TPointD *tilt,
  bool final,
  TTimerTicks ticks )
{
  ticks = fixTicks(ticks);
  TTrackP track = getTrack(deviceId, touchId, ticks, (bool)pressure, (bool)tilt);
  if (!track->finished()) {
    ticks = fixTicks(ticks);
    double time = (double)(ticks - track->ticks())*TToolTimer::step - track->timeOffset();
    addTrackPoint(
      track,
      position,
      pressure ? *pressure : 0.5,
      tilt ? *tilt : TPointD(),
      time,
      final );
  }
}


bool
TInputManager::keyEvent(
  bool press,
  TInputState::Key key,
  TTimerTicks ticks,
  QKeyEvent *event )
{
  bool wasPressed = state.isKeyPressed(key);
  ticks = fixTicks(ticks);
  state.keyEvent(press, key, ticks);
  processTracks();
  bool result = m_handler && m_handler->inputKeyEvent(press, key, event, *this);
  if (wasPressed != press) {
    touchTracks();
    processTracks();
    //hoverEvent(getInputHovers());
  }
  return result;
}


void
TInputManager::buttonEvent(
  bool press,
  TInputState::DeviceId deviceId,
  TInputState::Button button,
  TTimerTicks ticks )
{
  bool wasPressed = state.isButtonPressed(deviceId, button);
  ticks = fixTicks(ticks);
  state.buttonEvent(press, deviceId, button, ticks);
  processTracks();
  if (m_handler) m_handler->inputButtonEvent(press, deviceId, button, *this);
  if (wasPressed != press) {
    touchTracks();
    processTracks();
    //hoverEvent(getInputHovers());
  }
}


void
TInputManager::hoverEvent(const THoverList &hovers) {
  if (&m_hovers[0] != &hovers)
    m_hovers[0] = hovers;
  for(int i = 0; i < (int)m_modifiers.size(); ++i) {
    m_hovers[i+1].clear();
    m_modifiers[i]->modifyHovers(m_hovers[i], m_hovers[i+1]);
  }
  if (m_handler) {
    TRectD bounds = calcDrawBounds();
    if (!bounds.isEmpty()) {
      m_handler->inputInvalidateRect(m_prevBounds + bounds);
      m_nextBounds += bounds;
    }
    m_handler->inputHoverEvent(*this);
  }
}


TRectD
TInputManager::calcDrawBounds() {
  if (debugInputManager)
    return TConsts::infiniteRectD;

  TRectD bounds;
  for(int i = 0; i < (int)m_modifiers.size(); ++i)
    bounds += m_modifiers[i]->calcDrawBounds(m_tracks[i], m_hovers[i]);

  if (m_savePointsSent < (int)m_savePoints.size()) {
    for(TTrackList::const_iterator ti = getOutputTracks().begin(); ti != getOutputTracks().end(); ++ti) {
      TTrack &track = **ti;
      if (TrackHandler *handler = dynamic_cast<TrackHandler*>(track.handler.getPointer())) {
        int start = handler->saves[m_savePointsSent] - 1;
        if (start < 0) start = 0;
        if (start + 1 < track.size())
          for(int i = start + 1; i < track.size(); ++i)
            bounds += boundingBox(track[i-1].position, track[i].position);
      }
    }
  }

  if (!bounds.isEmpty())
    bounds.enlarge(4.0);
  
  return bounds;
}


void
TInputManager::draw() {
  m_prevBounds = m_nextBounds;
  m_nextBounds = TRectD();
  
  // paint not sent sub-tracks
  if (debugInputManager /* || m_savePointsSent < (int)m_savePoints.size() */) {
    glPushAttrib(GL_ALL_ATTRIB_BITS);
    tglEnableBlending();
    tglEnableLineSmooth(true, 0.5);
    double pixelSize = sqrt(tglGetPixelSize2());
    double colorBlack[4] = { 0.0, 0.0, 0.0, 1.0 };
    double colorWhite[4] = { 1.0, 1.0, 1.0, 1.0 };
    for(TTrackList::const_iterator ti = getOutputTracks().begin(); ti != getOutputTracks().end(); ++ti) {
      TTrack &track = **ti;
      if (TrackHandler *handler = dynamic_cast<TrackHandler*>(track.handler.getPointer())) {
        int start = debugInputManager ? 0 : handler->saves[m_savePointsSent] - 1;
        if (start < 0) start = 0;
        if (start + 1 < track.size()) {
          int level = m_savePointsSent;
          colorBlack[3] = (colorWhite[3] = 0.8);
          double radius = 2.0;
          for(int i = start + 1; i < track.size(); ++i) {
            while(level < (int)handler->saves.size() && handler->saves[level] <= i)
              colorBlack[3] = (colorWhite[3] *= 0.8), ++level;

            const TPointD &a = track[i-1].position;
            const TPointD &b = track[i].position;
            TPointD d = b - a;

            double k = norm2(d);
            if (k > TConsts::epsilon*TConsts::epsilon) {
              k = 0.5*pixelSize/sqrt(k);
              d = TPointD(-k*d.y, k*d.x);
              glColor4dv(colorWhite);
              tglDrawSegment(a - d, b - d);
              glColor4dv(colorBlack);
              tglDrawSegment(a + d, b + d);
              radius = 2.0;
            } else {
              radius += 2.0;
            }

            if (debugInputManager) {
              glColor4d(0.0, 0.0, 0.0, 0.25);
              tglDrawCircle(b, radius*pixelSize);
            }
          }
        }
      }
    }
    glPopAttrib();
  }

  // paint modifiers
  for(int i = 0; i < (int)m_modifiers.size(); ++i)
    m_modifiers[i]->draw(m_tracks[i], m_hovers[i]);
}

TInputState::TouchId
TInputManager::genTouchId()
  { return ++m_lastTouchId; }
