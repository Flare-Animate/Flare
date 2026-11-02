#pragma once

#include "archive.h"
#include <QJsonObject>
#include <QMap>
#include <QStringList>
#include <QVector>
#include <optional>

namespace Flash {

// XFL/JSFL graphic loop names: "loop", "play once", "single frame",
// "loop reverse" and "play once reverse". New values are appended so existing
// persisted integer values (UI combo data) keep their meaning.
enum class PlaybackMode {
  Loop,
  PlayOnce,
  SingleFrame,
  LoopReverse,
  PlayOnceReverse
};

PlaybackMode playbackMode(const QString &name);
QString playbackModeName(PlaybackMode mode);
bool isReversePlayback(PlaybackMode mode);
// The frame Animate plays for a first frame at or past the end of a symbol
// of symbolFrameCount frames: loops wrap around the symbol, the other modes
// rest on its last frame.
int wrappedFirstFrame(int firstFrame, PlaybackMode mode, int symbolFrameCount);

// Only the mapped graphic timing fields and known descriptive instance fields
// are accepted; unfamiliar fields might alter playback and are rejected.
void validateGraphicInstanceProperties(
    const QMap<QString, QString> &properties);

// Parent and source frames are zero-based. Native TFrameId conversion belongs
// at the scene boundary. This object is owned by a placement, not a symbol.
//
// Source frame for elapsed = parentFrame - start, n = symbol frame count,
// f = firstFrame, L = lastFrame (inclusive, zero-based, Animate 2019+):
//   single frame       : f (L is irrelevant)
//   without L:
//     loop             : (f + elapsed) mod n            (legacy whole-symbol)
//     play once        : min(f + elapsed, n - 1)
//     loop reverse     : (f - elapsed) mod n, wrapping n-1 after 0
//     play once reverse: max(f - elapsed, 0)
//   with L (forward modes require L >= f, reverse modes require L <= f):
//     loop             : f + elapsed mod (L - f + 1)
//     play once        : min(f + elapsed, L)
//     loop reverse     : f - elapsed mod (f - L + 1)
//     play once reverse: max(f - elapsed, L)
// A range pointing the other way, or f/L outside the symbol, is an error.
struct GraphicTiming {
  int start         = 0;
  int duration      = 1;
  int firstFrame    = 0;
  PlaybackMode mode = PlaybackMode::Loop;
  std::optional<int> lastFrame;

  std::optional<int> sourceFrameAt(int parentFrame, int symbolFrameCount) const;
  // Schema 1 holds legacy fields only; schema 2 is written only when a
  // reverse mode or lastFrame is used, so legacy readers never misread it.
  QJsonObject toJson() const;
  static GraphicTiming fromJson(const QJsonObject &json);
};

// The frames a movie clip (or the main timeline) shows on its consecutive
// ticks, counted from the tick it appears: frames[t] for t < frames.size(),
// then frames[loopStart..] over and over. Without scripts that is 0..n-1
// looping; frame scripts (see simulatePlayhead) stop, hold or jump it.
struct Playhead {
  QVector<int> frames{0};
  int loopStart = 0;

  static Playhead linear(int frameCount);
  bool isLinear(int frameCount) const;
  int cycle() const { return frames.size() - loopStart; }
  int frameAt(qint64 tick) const;
};

// A movie clip owns its own clock from the tick it appears. The clock begins
// at clockStart (default: start), the first frame of a run of consecutive
// keyframes that keep the same clip instance (see movieClipClockStart in
// model.h); otherwise each keyframe occurrence restarts the clip. Elapsed
// ticks map to source frames through the clip's playhead (its frame scripts),
// by default 0..n-1 looping. The adapter bakes this into ordinary child-frame
// exposures of the clip's native Xsheet.
struct MovieClipTiming {
  int start    = 0;
  int duration = 1;
  std::optional<int> clockStart;
  std::optional<Playhead> playhead;

  int clockOrigin() const { return clockStart.value_or(start); }
  std::optional<int> sourceFrameAt(int sceneFrame, int symbolFrameCount) const;
};

// Frame scripts --------------------------------------------------------------
//
// Animate keeps a frame's ActionScript in <Actionscript><script>. Scripts are
// never executed. The subset that only moves the timeline's own playhead is
// translated into timing: stop(), play(), gotoAndStop(f), gotoAndPlay(f),
// nextFrame() and prevFrame(), optionally prefixed by "this." (AS2 and AS3),
// where f is a 1-based frame number or a frame label. "_root." / "root." /
// "MovieClip(root)." commands address the main timeline. trace() and AS3
// import declarations change nothing visible and are accepted.
struct ScriptCommand {
  enum class Kind {
    Stop,
    Play,
    GotoAndStop,
    GotoAndPlay,
    NextFrame,
    PrevFrame
  };
  Kind kind = Kind::Stop;
  int frame = -1;  // Zero-based target frame, or -1 when `label` names it
  QString label;
  bool root = false;  // Addressed to the main timeline
  QString text;       // The statement as written
};

// A statement outside the subset, with the import issue code that reports it:
//   scripted-playback         timeline control that is not translated (other
//                             clips, scenes, computed frames, conditions)
//   unmapped-script-property  property, variable and state changes
//   unmapped-script-event     event handlers, listeners, timers, functions
//   unmapped-script-load      loading, URLs, external calls, dynamic children
//   unmapped-script-sound     scripted sound control
//   unmapped-frame-script     anything else
struct ScriptProblem {
  QString code;
  QString detail;
};

struct FrameScript {
  QVector<ScriptCommand> commands;  // In execution order
  QVector<ScriptProblem> problems;
};

FrameScript parseFrameScript(const QString &source);

// Simulates a timeline's playhead tick by tick: it starts on frame 0, playing,
// and each tick advances (looping after the last frame) unless stopped. The
// scripts of a frame run when the playhead enters it (also by a goto, after
// the current script); a goto to the frame already shown runs nothing.
// `commands` maps zero-based frames to their scripts' commands in execution
// order, `labels` frame label names to frames. Unknown labels and endless
// goto chains are skipped and listed in `problems`.
Playhead simulatePlayhead(int frameCount,
                          const QMap<int, QVector<ScriptCommand>> &commands,
                          const QMap<QString, int> &labels,
                          QStringList *problems = nullptr);

}  // namespace Flash
