#include "timing.h"

#include <QHash>
#include <QRegularExpression>
#include <QSet>
#include <algorithm>
#include <cmath>

namespace Flash {
namespace {
constexpr int maxFrame = 10000000;

void rejectUnknownProperties(const QJsonObject &json, int schema) {
  static const QSet<QString> fields{"schemaVersion", "start", "duration",
                                    "firstFrame", "mode"};
  for (auto property = json.constBegin(); property != json.constEnd();
       ++property)
    if (!fields.contains(property.key()) &&
        !(schema >= 2 && property.key() == "lastFrame"))
      throw Error("Unsupported symbol timing property: " +
                  property.key().left(256));
}

int requiredInteger(const QJsonObject &json, const QString &name, int minimum) {
  const QJsonValue value = json.value(name);
  const double number    = value.toDouble(-1);
  if (!value.isDouble() || !std::isfinite(number) ||
      number != std::floor(number) || number < minimum || number > maxFrame)
    throw Error("Invalid symbol timing property: " + name);
  return int(number);
}

void validate(const GraphicTiming &timing) {
  if (timing.start < 0 || timing.duration <= 0 || timing.firstFrame < 0 ||
      timing.start > maxFrame - timing.duration ||
      timing.firstFrame > maxFrame ||
      (timing.lastFrame &&
       (*timing.lastFrame < 0 || *timing.lastFrame > maxFrame)))
    throw Error("Invalid graphic symbol timing range.");
  if (timing.lastFrame && timing.mode != PlaybackMode::SingleFrame &&
      (isReversePlayback(timing.mode) ? *timing.lastFrame > timing.firstFrame
                                      : *timing.lastFrame < timing.firstFrame))
    throw Error("Graphic lastFrame points against its playback direction.");
}

int positiveModulo(qint64 value, qint64 modulus) {
  return int(((value % modulus) + modulus) % modulus);
}
}  // namespace

PlaybackMode playbackMode(const QString &name) {
  if (name.isEmpty() || name == "loop") return PlaybackMode::Loop;
  if (name == "play once") return PlaybackMode::PlayOnce;
  if (name == "single frame") return PlaybackMode::SingleFrame;
  if (name == "loop reverse") return PlaybackMode::LoopReverse;
  if (name == "play once reverse") return PlaybackMode::PlayOnceReverse;
  throw Error("Unsupported graphic playback mode: " + name.left(256));
}

void validateGraphicInstanceProperties(
    const QMap<QString, QString> &properties) {
  static const QSet<QString> fields{
      "libraryItemName", "symbolType", "firstFrame", "lastFrame", "loop",
      // Identity and authoring-only metadata do not change a 2D graphic clock.
      "name", "itemID", "referenceID", "lastModified", "selected", "isSelected",
      "centerPoint3DX", "centerPoint3DY",
      // A 3D instance's rotation and matrix do not change its clock either;
      // preflight reports them (unmapped-3d-transform).
      "centerPoint3DZ", "rotationX", "rotationY", "rotationZ", "matrix3D",
      // Only the mapped/default rendering values may pass native preflight.
      "blendMode", "visible", "isVisible", "cacheAsBitmap"};
  for (auto property = properties.constBegin();
       property != properties.constEnd(); ++property)
    if (!fields.contains(property.key()))
      throw Error("Unsupported graphic instance property: " +
                  property.key().left(256));

  // Presence matters: an explicit empty loop must not receive an absent-field
  // default, and an unimplemented visibility/cache mode must not be ignored.
  if (properties.contains("loop")) {
    if (properties.value("loop").isEmpty())
      throw Error("Invalid graphic playback property: loop");
    playbackMode(properties.value("loop"));
  }
  for (const QString &visibility : {QString("visible"), QString("isVisible")})
    if (properties.contains(visibility) &&
        properties.value(visibility) != "true")
      throw Error("Unsupported graphic instance property: " + visibility);
  // Bitmap caching draws the same image; it does not change the clock.
  if (properties.contains("cacheAsBitmap") &&
      properties.value("cacheAsBitmap") != "false" &&
      properties.value("cacheAsBitmap") != "true")
    throw Error("Unsupported graphic instance property: cacheAsBitmap");
}

QString playbackModeName(PlaybackMode mode) {
  switch (mode) {
  case PlaybackMode::Loop:
    return "loop";
  case PlaybackMode::PlayOnce:
    return "play once";
  case PlaybackMode::SingleFrame:
    return "single frame";
  case PlaybackMode::LoopReverse:
    return "loop reverse";
  case PlaybackMode::PlayOnceReverse:
    return "play once reverse";
  }
  throw Error("Invalid graphic playback mode.");
}

bool isReversePlayback(PlaybackMode mode) {
  return mode == PlaybackMode::LoopReverse ||
         mode == PlaybackMode::PlayOnceReverse;
}

int wrappedFirstFrame(int firstFrame, PlaybackMode mode, int symbolFrameCount) {
  const int count = std::max(1, symbolFrameCount);
  return mode == PlaybackMode::Loop || mode == PlaybackMode::LoopReverse
             ? firstFrame % count
             : count - 1;
}

std::optional<int> GraphicTiming::sourceFrameAt(int parentFrame,
                                                int symbolFrameCount) const {
  validate(*this);
  if (symbolFrameCount <= 0 || firstFrame >= symbolFrameCount)
    throw Error("Graphic first frame is outside its symbol timeline.");
  if (lastFrame && *lastFrame >= symbolFrameCount)
    throw Error("Graphic lastFrame is outside its symbol timeline.");
  if (parentFrame < start || parentFrame - start >= duration)
    return std::nullopt;
  const qint64 elapsed = parentFrame - start;
  const qint64 first   = firstFrame;
  switch (mode) {
  case PlaybackMode::SingleFrame:
    return firstFrame;
  case PlaybackMode::Loop:
    if (lastFrame) return int(first + elapsed % (*lastFrame - first + 1));
    return int((first + elapsed) % symbolFrameCount);
  case PlaybackMode::PlayOnce:
    return int(std::min<qint64>(first + elapsed,
                                lastFrame ? *lastFrame : symbolFrameCount - 1));
  case PlaybackMode::LoopReverse:
    if (lastFrame) return int(first - elapsed % (first - *lastFrame + 1));
    return positiveModulo(first - elapsed, symbolFrameCount);
  case PlaybackMode::PlayOnceReverse:
    return int(std::max<qint64>(first - elapsed, lastFrame ? *lastFrame : 0));
  }
  throw Error("Invalid graphic playback mode.");
}

std::optional<int> MovieClipTiming::sourceFrameAt(int sceneFrame,
                                                  int symbolFrameCount) const {
  const int origin = clockOrigin();
  if (start < 0 || duration <= 0 || start > maxFrame - duration || origin < 0 ||
      origin > start || symbolFrameCount <= 0 || symbolFrameCount > maxFrame)
    throw Error("Invalid movie clip occurrence or symbol timeline.");
  if (sceneFrame < start || sceneFrame - start >= duration) return std::nullopt;
  if (!playhead) return (sceneFrame - origin) % symbolFrameCount;
  const int frame = playhead->frameAt(sceneFrame - origin);
  if (frame < 0 || frame >= symbolFrameCount)
    throw Error("Movie clip playhead leaves its symbol timeline.");
  return frame;
}

Playhead Playhead::linear(int frameCount) {
  Playhead result;
  result.frames.resize(std::max(1, frameCount));
  for (int frame = 0; frame < result.frames.size(); ++frame)
    result.frames[frame] = frame;
  return result;
}

bool Playhead::isLinear(int frameCount) const {
  if (loopStart != 0 || frames.size() != std::max(1, frameCount)) return false;
  for (int frame = 0; frame < frames.size(); ++frame)
    if (frames[frame] != frame) return false;
  return true;
}

int Playhead::frameAt(qint64 tick) const {
  if (frames.isEmpty() || loopStart < 0 || loopStart >= frames.size())
    throw Error("Invalid movie clip playhead.");
  if (tick < 0) throw Error("Movie clip playhead before its first tick.");
  if (tick < frames.size()) return frames[int(tick)];
  return frames[loopStart + int((tick - loopStart) % cycle())];
}

namespace {
// Removes // and /* */ comments outside string literals.
QString withoutComments(const QString &source) {
  QString result;
  result.reserve(source.size());
  QChar quote;
  bool escaped = false;
  for (int i = 0; i < source.size(); ++i) {
    const QChar c = source[i];
    if (!quote.isNull()) {
      result += c;
      if (escaped)
        escaped = false;
      else if (c == '\\')
        escaped = true;
      else if (c == quote)
        quote = QChar();
      continue;
    }
    if (c == '"' || c == '\'') {
      quote = c;
      result += c;
    } else if (c == '/' && i + 1 < source.size() && source[i + 1] == '/') {
      while (i + 1 < source.size() && source[i + 1] != '\n' &&
             source[i + 1] != '\r')
        ++i;
    } else if (c == '/' && i + 1 < source.size() && source[i + 1] == '*') {
      const int end = source.indexOf("*/", i + 2);
      i             = end < 0 ? source.size() : end + 1;
      result += ' ';
    } else
      result += c;
  }
  return result;
}

// Top-level statements, split at semicolons and line breaks outside
// brackets; a braced block ends its statement. A control header on a line
// of its own (if (...), else, while (...)) takes the next statement as body.
QStringList statements(const QString &code) {
  QStringList result;
  QString current;
  int brackets = 0, braces = 0;
  QChar quote;
  bool escaped     = false;
  const auto flush = [&] {
    const QString statement = current.simplified();
    if (!statement.isEmpty()) result << statement;
    current.clear();
  };
  for (const QChar c : code) {
    if (!quote.isNull()) {
      current += c;
      if (escaped)
        escaped = false;
      else if (c == '\\')
        escaped = true;
      else if (c == quote)
        quote = QChar();
      continue;
    }
    if (c == '"' || c == '\'') quote = c;
    if (c == '(' || c == '[') ++brackets;
    if ((c == ')' || c == ']') && brackets > 0) --brackets;
    if (c == '{') ++braces;
    if (c == '}') {
      current += c;
      if (braces > 0) --braces;
      if (!braces && !brackets) flush();
      continue;
    }
    if (!brackets && !braces && (c == ';' || c == '\n' || c == '\r')) {
      flush();
      continue;
    }
    current += c;
  }
  flush();
  static const QRegularExpression header(
      R"(^(?:(?:else\s+)?if|while|for|with)\s*\(.*\)$|^(?:else|do)$)");
  for (int index = 0; index + 1 < result.size();)
    if (header.match(result[index]).hasMatch()) {
      result[index] += " " + result[index + 1];
      result.removeAt(index + 1);
    } else
      ++index;
  return result;
}

QString quoted(const QString &statement) {
  return "\"" +
         (statement.size() > 160 ? statement.left(157) + "..." : statement) +
         "\"";
}
}  // namespace

FrameScript parseFrameScript(const QString &source) {
  FrameScript result;
  // Optional target: this, or the main timeline.
  static const QString target =
      R"((?:(this|_root|_level0|root|MovieClip\s*\(\s*root\s*\)|\(\s*root\s+as\s+MovieClip\s*\)|this\s*\.\s*root)\s*\.\s*)?)";
  static const QRegularExpression simple(
      "^" + target + R"((stop|play|nextFrame|prevFrame)\s*\(\s*\)$)");
  static const QRegularExpression go(
      "^" + target +
      R"((gotoAndStop|gotoAndPlay)\s*\(\s*(\d+|"[^"\\]*"|'[^'\\]*')\s*\)$)");
  static const QRegularExpression harmless(
      R"(^(?:trace\s*\(.*\)|import\s+[\w.*]+)$)");
  static const QRegularExpression control(
      R"(^(?:if|else|for|while|do|switch|with|try|catch|finally)\b)");
  static const QRegularExpression timelineCall(
      R"(\b(?:stop|play|gotoAndStop|gotoAndPlay|nextFrame|prevFrame|nextScene|prevScene)\s*\()");
  static const QRegularExpression event(
      R"(^(?:function\b|on\s*\(|onClipEvent\b)|\b(?:addEventListener|removeEventListener|onEnterFrame|onLoad|onUnload|onPress|onRelease|onReleaseOutside|onRollOver|onRollOut|onDragOver|onDragOut|onMouse\w*|onKey\w*|setInterval|clearInterval|setTimeout|clearTimeout|Timer|Event\.)\b)");
  static const QRegularExpression load(
      R"(\b(?:loadMovie|loadMovieNum|unloadMovie|unloadMovieNum|loadVariables|loadVariablesNum|getURL|navigateToURL|sendToURL|URLRequest|URLLoader|Loader|LoadVars|XML|fscommand|ExternalInterface|attachMovie|duplicateMovieClip|createEmptyMovieClip|removeMovieClip|addChild|addChildAt|removeChild|removeChildAt|createTextField)\b)");
  static const QRegularExpression sound(
      R"(\b(?:stopAllSounds|Sound|SoundMixer|SoundChannel|SoundTransform)\b)");
  static const QRegularExpression property(
      R"((?:^|[^=!<>])=(?!=)|\+\+|--|^(?:var|const|delete)\b|\bsetProperty\s*\()");
  for (const QString &statement : statements(withoutComments(source))) {
    QRegularExpressionMatch match = simple.match(statement);
    if (match.hasMatch()) {
      ScriptCommand command;
      const QString name = match.captured(2);
      command.kind       = name == "stop"        ? ScriptCommand::Kind::Stop
                           : name == "play"      ? ScriptCommand::Kind::Play
                           : name == "nextFrame" ? ScriptCommand::Kind::NextFrame
                                                 : ScriptCommand::Kind::PrevFrame;
      command.root =
          !match.captured(1).isEmpty() && match.captured(1) != "this";
      command.text = statement;
      result.commands.push_back(command);
      continue;
    }
    match = go.match(statement);
    if (match.hasMatch()) {
      ScriptCommand command;
      command.kind = match.captured(2) == "gotoAndStop"
                         ? ScriptCommand::Kind::GotoAndStop
                         : ScriptCommand::Kind::GotoAndPlay;
      command.root =
          !match.captured(1).isEmpty() && match.captured(1) != "this";
      command.text           = statement;
      const QString argument = match.captured(3);
      if (argument.startsWith('"') || argument.startsWith('\''))
        command.label = argument.mid(1, argument.size() - 2);
      else {
        bool ok         = false;
        const int value = argument.toInt(&ok);
        if (!ok || value < 1) {
          result.problems.push_back(
              {"scripted-playback",
               "Frame script goes to a frame that does not exist: " +
                   quoted(statement)});
          continue;
        }
        command.frame = value - 1;
      }
      result.commands.push_back(command);
      continue;
    }
    if (harmless.match(statement).hasMatch()) continue;
    const bool controlsTimeline = timelineCall.match(statement).hasMatch();
    QString code, detail;
    if (control.match(statement).hasMatch()) {
      code   = controlsTimeline ? "scripted-playback" : "unmapped-frame-script";
      detail = controlsTimeline
                   ? "Conditional or repeated timeline control is not "
                     "evaluated: "
                   : "Frame script condition or loop is not evaluated: ";
    } else if (event.match(statement).hasMatch()) {
      code   = "unmapped-script-event";
      detail = "Frame script event handler, listener or timer is not run: ";
    } else if (load.match(statement).hasMatch()) {
      code   = "unmapped-script-load";
      detail = "Frame script loading, link or dynamic content is not run: ";
    } else if (sound.match(statement).hasMatch()) {
      code   = "unmapped-script-sound";
      detail = "Frame script sound control is not run: ";
    } else if (controlsTimeline) {
      code = "scripted-playback";
      detail =
          "Frame script controls another timeline, a scene or a "
          "computed frame, which is not translated: ";
    } else if (property.match(statement).hasMatch()) {
      code   = "unmapped-script-property";
      detail = "Frame script property or variable change is not applied: ";
    } else {
      code   = "unmapped-frame-script";
      detail = "Frame script statement is not run: ";
    }
    result.problems.push_back({code, detail + quoted(statement)});
  }
  return result;
}

Playhead simulatePlayhead(int frameCount,
                          const QMap<int, QVector<ScriptCommand>> &commands,
                          const QMap<QString, int> &labels,
                          QStringList *problems) {
  const int count = std::max(1, frameCount);
  if (commands.isEmpty()) return Playhead::linear(count);
  QSet<QString> noted;
  const auto note = [&](const QString &problem) {
    if (problems && !noted.contains(problem)) {
      noted.insert(problem);
      *problems << problem;
    }
  };
  int frame    = 0;
  bool playing = true;
  // Runs the scripts of `entered`, then of each frame a goto moved to.
  const auto enter = [&](int entered) {
    for (int runs = 0;; ++runs) {
      for (const auto &command : commands.value(entered)) {
        int target = -1;
        switch (command.kind) {
        case ScriptCommand::Kind::Stop:
          playing = false;
          break;
        case ScriptCommand::Kind::Play:
          playing = true;
          break;
        case ScriptCommand::Kind::NextFrame:
          playing = false;
          frame   = std::min(frame + 1, count - 1);
          break;
        case ScriptCommand::Kind::PrevFrame:
          playing = false;
          frame   = std::max(frame - 1, 0);
          break;
        case ScriptCommand::Kind::GotoAndStop:
        case ScriptCommand::Kind::GotoAndPlay:
          if (command.label.isEmpty())
            // Flash shows the last frame for a frame number past the end.
            target = std::min(command.frame, count - 1);
          else if (labels.contains(command.label))
            target = labels.value(command.label);
          else {
            note("Frame script goes to an unknown frame label: " +
                 quoted(command.text));
            break;
          }
          playing = command.kind == ScriptCommand::Kind::GotoAndPlay;
          frame   = target;
          break;
        }
      }
      if (frame == entered) return;
      if (runs > count) {
        note(
            "Frame scripts go to each other endlessly; the playhead stays "
            "on frame " +
            QString::number(frame + 1) + ".");
        return;
      }
      entered = frame;
    }
  };
  // The state after a tick (frame, playing) decides every later tick, so the
  // first repeated state closes the cycle.
  Playhead result;
  result.frames.clear();
  QHash<int, int> seen;  // state -> tick
  enter(0);
  for (int tick = 0;; ++tick) {
    if (tick > 0 && playing && count > 1) {
      frame = (frame + 1) % count;
      enter(frame);
    }
    const int state  = frame * 2 + (playing ? 1 : 0);
    const auto first = seen.constFind(state);
    if (first != seen.cend()) {
      result.loopStart = first.value();
      break;
    }
    seen.insert(state, tick);
    result.frames.push_back(frame);
  }
  // A cycle that ends on the frame just before it can start one tick earlier.
  while (result.loopStart > 0 &&
         result.frames[result.loopStart - 1] == result.frames.back()) {
    result.frames.removeLast();
    --result.loopStart;
  }
  return result;
}

QJsonObject GraphicTiming::toJson() const {
  validate(*this);
  const bool extended = lastFrame || isReversePlayback(mode);
  QJsonObject json{{"schemaVersion", extended ? 2 : 1},
                   {"start", start},
                   {"duration", duration},
                   {"firstFrame", firstFrame},
                   {"mode", playbackModeName(mode)}};
  if (lastFrame) json.insert("lastFrame", *lastFrame);
  return json;
}

GraphicTiming GraphicTiming::fromJson(const QJsonObject &json) {
  const int schema = requiredInteger(json, "schemaVersion", 1);
  if (schema != 1 && schema != 2)
    throw Error("Unsupported graphic timing schema version.");
  rejectUnknownProperties(json, schema);
  if (!json.value("mode").isString())
    throw Error("Missing graphic playback mode.");
  if (json.value("mode").toString().isEmpty())
    throw Error("Invalid persisted graphic playback mode.");
  GraphicTiming timing;
  timing.start      = requiredInteger(json, "start", 0);
  timing.duration   = requiredInteger(json, "duration", 1);
  timing.firstFrame = requiredInteger(json, "firstFrame", 0);
  timing.mode       = playbackMode(json.value("mode").toString());
  if (json.contains("lastFrame"))
    timing.lastFrame = requiredInteger(json, "lastFrame", 0);
  if (schema == 1 && isReversePlayback(timing.mode))
    throw Error("Unsupported graphic playback mode for timing schema 1: " +
                playbackModeName(timing.mode));
  if (schema == 2 && !timing.lastFrame && !isReversePlayback(timing.mode))
    throw Error("Graphic timing schema 2 requires lastFrame or reverse mode.");
  validate(timing);
  return timing;
}

}  // namespace Flash
