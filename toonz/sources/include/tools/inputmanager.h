#pragma once

#ifndef INPUTMANAGER_INCLUDED
#define INPUTMANAGER_INCLUDED

// TnzTools includes
#include <tools/tooltimer.h>
#include <tools/inputstate.h>
#include <tools/track.h>

// TnzCore includes
#include <tcommon.h>
#include <tgeometry.h>
#include <tsmartpointer.h>

// Qt includes
#include <QObject>
#include <QKeyEvent>

// std includes
#include <vector>
#include <algorithm>


#undef DVAPI
#undef DVVAR
#ifdef TNZTOOLS_EXPORTS
#define DVAPI DV_EXPORT_API
#define DVVAR DV_EXPORT_VAR
#else
#define DVAPI DV_IMPORT_API
#define DVVAR DV_IMPORT_VAR
#endif


//====================================================

//  Forward declarations

class TTool;
class TInputModifier;
class TInputManager;

typedef TSmartPointerT<TInputModifier> TInputModifierP;

//===================================================================


//*****************************************************************************************
//    TInputSavePoint definition
//*****************************************************************************************

class TInputSavePoint {
public:
  class Holder {
  private:
    TInputSavePoint *m_savePoint;
    bool m_lock = false;

  public:
    inline explicit Holder(TInputSavePoint *savePoint = NULL, bool lock = true):
      m_savePoint(), m_lock()
      { set(savePoint, lock); }
    inline Holder(const Holder &other):
      m_savePoint(), m_lock()
      { *this = other; }
    inline ~Holder()
      { reset(); }

    inline Holder& operator= (const Holder &other)
      { set(other.m_savePoint, other.m_lock); return *this; }

    inline operator bool () const
      { return assigned(); }

    inline void set(TInputSavePoint *savePoint, bool lock) {
      if (m_savePoint != savePoint) {
        if (m_savePoint) {
          if (m_lock) m_savePoint->unlock();
          m_savePoint->release();
        }
        m_savePoint = savePoint;
        m_lock = lock;
        if (m_savePoint) {
          m_savePoint->hold();
           if (m_lock) savePoint->lock();
        }
      } else
      if (m_lock != lock) {
        if (m_savePoint) {
          if (lock) m_savePoint->lock();
               else m_savePoint->unlock();
        }
        m_lock = lock;
      }
    }

    inline void reset()
      { set(NULL, false); }
    inline void lock()
      { set(m_savePoint, true); }
    inline void unlock()
      { set(m_savePoint, false); }

    inline TInputSavePoint* savePoint() const
      { return m_savePoint; }
    inline bool assigned() const
      { return savePoint(); }
    inline bool locked() const
      { return m_savePoint && m_lock; }
    inline bool available() const
      { return m_savePoint && m_savePoint->available; }
    inline bool isFree() const
      { return !m_savePoint || m_savePoint->isFree(); }
  };

  typedef std::vector<Holder> List;

private:
  int m_refCount;
  int m_lockCount;

  inline void hold()
    { ++m_refCount; }
  inline void release()
    { if ((--m_refCount) <= 0) delete this; }
  inline void lock()
    { ++m_lockCount; }
  inline void unlock()
    { --m_lockCount; }

public:
  bool available;

  inline explicit TInputSavePoint(bool available = false):
    m_refCount(), m_lockCount(), available(available) { }
  inline bool isFree() const
    { return m_lockCount <= 0; }

  static inline Holder create(bool available = false)
    { return Holder(new TInputSavePoint(available)); }
};


//*****************************************************************************************
//    TInputModifier definition
//*****************************************************************************************

class TInputModifier: public TSmartObject {
private:
  TInputManager *m_manager;

public:
  typedef std::vector<TInputModifierP> List;

  TInputManager* getManager() const
    { return m_manager; }
  void setManager(TInputManager *manager);
  virtual void onSetManager() { }

  virtual void activate() { }

  virtual void modifyTrack(
    const TTrack &track,
    const TInputSavePoint::Holder &savePoint,
    TTrackList &outTracks );
  virtual void modifyTracks(
    const TTrackList &tracks,
    const TInputSavePoint::Holder &savePoint,
    TTrackList &outTracks );

  virtual void modifyHover(
    const TPointD &hover,
    THoverList &outHovers );
  virtual void modifyHovers(
    const THoverList &hovers,
    THoverList &outHovers );

  virtual TRectD calcDrawBoundsHover(const TPointD &hover) { return TRectD(); }
  virtual TRectD calcDrawBoundsTrack(const TTrack &track) { return TRectD(); }
  virtual TRectD calcDrawBounds(const TTrackList &tracks, const THoverList &hovers);

  virtual void drawTrack(const TTrack &track) { }
  virtual void drawHover(const TPointD &hover) { }
  virtual void drawTracks(const TTrackList &tracks);
  virtual void drawHovers(const THoverList &hovers);
  virtual void draw(const TTrackList &tracks, const THoverList &hovers);

  virtual void deactivate() { }
};


//*****************************************************************************************
//    TInputHandler definition
//*****************************************************************************************

class TInputHandler {
public:
  virtual void inputLeftButtonDown(const TTrackPoint&, const TTrack&) { }
  virtual void inputLeftButtonDrag(const TTrackPoint&, const TTrack&) { }
  virtual void inputLeftButtonUp(const TTrackPoint&, const TTrack&) { }
  virtual void inputMouseMove(const TPointD&, const TInputState&) { }
  virtual void inputRightButtonDown(const TPointD&, const TInputState&) { }
  virtual bool inputKeyDown(QKeyEvent *) { return false; }

  virtual void inputSetBusy(bool) { }
  
  virtual bool inputKeyEvent(
    bool press,
    TInputState::Key key,
    QKeyEvent *event,
    const TInputManager &manager );
  
  virtual void inputButtonEvent(
    bool press,
    TInputState::DeviceId device,
    TInputState::Button button,
    const TInputManager &manager );
  
  virtual void inputHoverEvent(const TInputManager &manager);

  /*! paint single track-point at the top painting level */
  virtual void inputPaintTrackPoint(const TTrackPoint &point, const TTrack &track, bool firstTrack);

  /*! create new painting level and return true, or do nothing and return false
      was:            ------O-------O------
      become:         ------O-------O------O */
  virtual bool inputPaintPush() { return false; }
  /*! paint several track-points at the top painting level
      was:            ------O-------O------
      become:         ------O-------O------------ */
  virtual void inputPaintTracks(const TTrackList &tracks);
  /*! try to merge N top painting levels and return count of levels that actually merged
      was:            ------O-------O------O------
      become (N = 2): ------O--------------------- */
  virtual int inputPaintApply(int count) { return 0; }
  /*! reset top level to initial state
      was:            ------O-------O------O------
      become:         ------O-------O------O */
  virtual void inputPaintCancel() { }
  /*! cancel and pop N painting levels
      was:            ------O-------O------O------
      become (N = 2): ------O------- */
  virtual void inputPaintPop(int count) { }
  
  virtual void inputInvalidateRect(const TRectD &bounds) { }
  
  virtual TAffine toWorld() { return TAffine(); };
  virtual TTool* getTool() { return nullptr; };
};


//*****************************************************************************************
//    TInputManager definition
//*****************************************************************************************

class TInputManager {
public:
  class TrackHandler: public TTrackHandler {
  public:
    std::vector<int> saves;
    TrackHandler(TTrack &original, int keysCount = 0):
      TTrackHandler(original), saves(keysCount, 0)
      { }
  };

private:
  TTimerTicks m_lastTicks;
  TInputHandler *m_handler;
  TInputModifier::List m_modifiers;
  std::vector<TTrackList> m_tracks;
  std::vector<THoverList> m_hovers;
  TInputSavePoint::List m_savePoints;
  int m_savePointsSent;

  static TInputState::TouchId m_lastTouchId;


public:
  TInputState state;


public:
  TInputManager();

private:
  inline TTimerTicks fixTicks(TTimerTicks ticks) {
    if (ticks <= m_lastTicks) ticks = m_lastTicks + 1;
    return m_lastTicks = ticks;
  }
  
  void paintRollbackTo(int saveIndex, TTrackList &subTracks);
  void paintApply(int count, TTrackList &subTracks);
  void paintTracks();

  int trackCompare(
    const TTrack &track,
    TInputState::DeviceId deviceId,
    TInputState::TouchId touchId ) const;
  const TTrackP& createTrack(
    int index,
    TInputState::DeviceId deviceId,
    TInputState::TouchId touchId,
    TTimerTicks ticks,
    bool hasPressure,
    bool hasTilt );
  const TTrackP& getTrack(
    TInputState::DeviceId deviceId,
    TInputState::TouchId touchId,
    TTimerTicks ticks,
    bool hasPressure,
    bool hasTilt );
  void addTrackPoint(
    const TTrackP& track,
    const TPointD &position,
    double pressure,
    const TPointD &tilt,
    double time,
    bool final );
  void touchTracks(bool finish = false);

  void modifierActivate(const TInputModifierP &modifier);
  void modifierDeactivate(const TInputModifierP &modifier);

public:
  inline const TTrackList& getInputTracks() const
    { return m_tracks.front(); }
  inline const TTrackList& getOutputTracks() const
    { return m_tracks.back(); }

  inline const THoverList& getInputHovers() const
    { return m_hovers.front(); }
  inline const THoverList& getOutputHovers() const
    { return m_hovers.back(); }

  void processTracks();
  void finishTracks();
  void reset();

  TInputHandler* getHandler() const
    { return m_handler; }
  void setHandler(TInputHandler *handler);

  int getModifiersCount() const
    { return (int)m_modifiers.size(); }
  const TInputModifierP& getModifier(int index) const
    { return m_modifiers[index]; }
  int findModifier(const TInputModifierP &modifier) const;
  void insertModifier(int index, const TInputModifierP &modifier);
  void addModifier(const TInputModifierP &modifier)
    { insertModifier(getModifiersCount(), modifier); }
  void removeModifier(int index);
  void removeModifier(const TInputModifierP &modifier)
    { removeModifier(findModifier(modifier)); }
  void clearModifiers();

  void trackEvent(
    TInputState::DeviceId deviceId,
    TInputState::TouchId touchId,
    const TPointD &position,
    const double *pressure,
    const TPointD *tilt,
    bool final,
    TTimerTicks ticks );
  bool keyEvent(
    bool press,
    TInputState::Key key,
    TTimerTicks ticks,
    QKeyEvent *event );
  void buttonEvent(
    bool press,
    TInputState::DeviceId deviceId,
    TInputState::Button button,
    TTimerTicks ticks);
  void hoverEvent(const THoverList &hovers);

  TRectD calcDrawBounds();
  void draw();

  static TInputState::TouchId genTouchId();
};


#endif
