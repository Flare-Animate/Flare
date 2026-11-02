#pragma once

#include "document.h"
#include "geometry.h"
#include "timing.h"
#include <functional>
#include <memory>
#include <optional>

namespace Flash {

using Attributes = QMap<QString, QString>;

struct XmlAttribute {
  QString namespaceUri;
  QString name;
  QString qualifiedName;
  QString value;
  // XML-tree locator for retained attributes. Exact quoting and entity
  // spelling remain authoritative in the immutable source package snapshot.
  QString sourceId;
};

// Unqualified authored values live in each model node's properties map.
// A missing key means the reader supplied a default. Store extension fields
// separately so an extension cannot impersonate an XFL attribute.
struct SourceNode {
  // Canonical XML-tree path; parent SourceRecord IDs remain semantic paths.
  QString sourceId;
  QString namespaceUri;
  QString qualifiedName;
  QVector<XmlAttribute> qualifiedAttributes;
};

// A lightweight reference to an authored XML leaf. The package snapshot owns
// the exact bytes; this record keeps the evaluated object traceable to them.
struct SourceRecord {
  QString sourceId;
  Attributes properties;
  SourceNode source;
};

struct ExtraNode {
  QString sourceId;
  QString namespaceUri;
  QString qualifiedName;
  QVector<XmlAttribute> attributes;
};

struct Extra {
  // Canonical XML-tree path within the original package member. Unlike a
  // semantic sourceId, this counts all element siblings, including modeled
  // ones and unknown extension nodes.
  QString sourceId;
  // Existing modeled-parent identity, if the subtree belongs to one.
  QString ownerSourceId;
  QString tag;
  QString namespaceUri;
  QString qualifiedName;
  QByteArray xml;
  // Flat preorder index of every element under this retained subtree. Child
  // numbers count element siblings, so duplicate names remain distinct.
  QVector<ExtraNode> nodes;
};

struct Color {
  unsigned char red = 0, green = 0, blue = 0, alpha = 255;
};

struct GradientStop {
  double ratio = 0;
  Color color;
  Attributes properties;
  SourceNode source;
  QString sourceId;
  QVector<Extra> extras;
};

struct Paint {
  enum class Kind {
    Solid,
    LinearGradient,
    RadialGradient,
    Bitmap,
    Unsupported
  };
  Kind kind = Kind::Solid;
  Color color;
  Affine matrix;
  QVector<GradientStop> stops;
  Attributes properties;
  SourceNode source;
  QString sourceId;
  SourceRecord style;
  SourceRecord matrixSource;
  std::optional<Extra> matrixXml;
  QString sourceTag;
  QVector<Extra> extras;
};

struct Stroke {
  QString type;
  double weight = 1;
  Paint paint;
  Attributes properties;
  SourceNode source;
  QString sourceId;
  SourceRecord style;
  QVector<Extra> extras;
};

struct Edge {
  int fill0 = 0, fill1 = 0, stroke = 0;
  QVector<Segment> segments;
  // Editor cubic handles supplement the quadratic display geometry.
  QString cubicControlData;
  // The encoded edges string is decoded into segments; this record keeps the
  // remaining attributes and location.
  SourceRecord source;
  QVector<Extra> extras;
};

struct Shape {
  QMap<int, Paint> fills;
  QMap<int, Stroke> strokes;
  QVector<Edge> edges;
};

// Text nodes keep all raw attributes, including namespaced ones, for
// structured text evaluation.
struct TextNode {
  // Canonical XML-tree path for this structured text node.
  QString sourceId;
  Attributes properties;
  QVector<XmlAttribute> rawAttributes;
  QVector<Extra> extras;
};

struct TextStyle {
  QString sourceId;
  TextNode source;
};

struct TextRun {
  QString sourceId;
  TextNode source;
  bool hasCharacters = false;
  QString characters;
  TextNode charactersSource;
  bool hasTextAttrs = false;
  TextNode textAttrsSource;
  std::optional<TextStyle> style;
};

struct TextBlock {
  QString sourceId;
  TextNode source;
  std::optional<Extra> matrixXml;
  std::optional<Extra> transformationPointXml;
  bool hasRuns = false;
  TextNode runsSource;
  QVector<TextRun> runs;
};

// Evaluated presentation of one text run (DOMTextAttrs), with the XFL
// defaults applied. Lengths are in element-local source pixels.
struct TextRunFormat {
  QString sourceId;
  QString characters;
  QString face;  // As authored: usually a PostScript name, e.g. "Arial-Black"
  double size = 12;
  Color fill;  // fillColor with alpha
  bool bold = false, italic = false, autoKern = true;
  double letterSpacing = 0;
  QString alignment    = "left";  // left, center, right or justify
  // Extra space between lines when authored (hasLineSpacing; Animate omits
  // its default of 2 px). lineHeight is Animate's measure of the line height
  // without the spacing (ascent plus descent; 0 when absent).
  double lineSpacing = 0, lineHeight = 0;
  bool hasLineSpacing = false;
  double indent = 0, leftMargin = 0, rightMargin = 0;
};

// What laying out a text element needs, evaluated from TextBlock. The text
// box origin is (left, top) in element-local pixels; the first baseline lies
// one line ascent below it. Attributes that affect rendering but have no
// layout mapping are listed in `unmapped` as "<sourceId>: <detail>".
struct TextLayoutSource {
  double left = 0, top = 0, width = 0, height = 0;
  bool autoExpand = false;
  bool wrap       = false;  // Wrap words at the box width
  QVector<TextRunFormat> runs;
  QStringList unmapped;
};

struct Element {
  enum class Kind {
    Shape,
    Symbol,
    Group,
    Bitmap,
    Text,
    Oval,
    Rectangle,
    Video,
    Unsupported
  };
  Kind kind = Kind::Unsupported;
  QString tag;
  QString sourceId;
  Attributes properties;
  SourceNode source;
  Affine matrix;
  SourceRecord matrixSource;
  std::optional<Extra> matrixXml;
  Point transformationPoint;
  bool hasTransformationPoint = false;
  SourceRecord transformationPointSource;
  std::optional<Extra> transformationPointXml;
  std::shared_ptr<Shape> shape;
  // Static, dynamic and input text are structured (see textLayout()).
  // Placement/pivot remain in the common element fields.
  std::shared_ptr<TextBlock> text;
  QVector<Element> members;
  QVector<Extra> extras;
};

struct Frame {
  QString sourceId;
  int index = 0, duration = 1;
  Attributes properties;
  SourceNode source;
  QVector<Element> elements;
  QVector<Extra> extras;
};

struct Layer {
  QString sourceId;
  Attributes properties;
  SourceNode source;
  QString name;
  QString type = "normal";
  int parent   = -1;
  bool visible = true, locked = false;
  QVector<Frame> frames;
  QVector<Extra> extras;
};

struct Timeline {
  QString sourceId;
  QString name;
  Attributes properties;
  SourceNode source;
  QVector<Layer> layers;
  QVector<Extra> extras;
  int frameCount = 0;
};

struct Definition {
  QString member;
  QString name;
  QString type;
  Attributes properties;
  SourceNode source;
  QVector<Timeline> timelines;
  QVector<Extra> extras;
  quint64 geometrySegments = 0;
};

// Definitions are read on demand, allowing a builder to release large source
// XML/models after creating their shared native library item. Unsupported
// subtrees are retained as Extra records; they are not treated as evaluated.
Definition readDefinition(Package &package, const QString &member);
// Strict "#rrggbb" parser for XFL color attributes.
bool parseHexRgb(const QString &lexical, Color &result);

// Effective DOMSymbolInstance type. Adobe omits symbolType for movie clips
// (the XFL default); "graphic" and "button" are always explicit.
QString symbolInstanceType(const Element &element);

// Requires element.text (static, dynamic or input text). Never throws for
// unusual authored values: they keep their defaults and are listed.
TextLayoutSource textLayout(const Element &element);

GraphicTiming instanceTiming(const Element &element, const Frame &frame);
// The single-frame overload restarts the clip at frame.index. The layer
// overload continues the clock across consecutive keyframes that keep the
// same clip instance (see movieClipClockStart).
MovieClipTiming movieClipTiming(const Element &element, const Frame &frame);
MovieClipTiming movieClipTiming(const Layer &layer, int frameOrdinal,
                                int elementSlot);
// First frame index of the run of directly adjacent keyframes on this layer
// whose element at elementSlot is the same movie clip instance (same
// libraryItemName and same instance name, absent names matching only absent
// names) as layer.frames[frameOrdinal].elements[elementSlot]. Returns the
// frame's own index when the clip does not continue from the previous one.
int movieClipClockStart(const Layer &layer, int frameOrdinal, int elementSlot);

// Frame scripts (see parseFrameScript in timing.h). frameScript() is the
// text of a frame's <Actionscript><script> blocks, empty without scripts.
bool frameScriptExtra(const Extra &extra);
QString frameScript(const Frame &frame);
// The translated commands of each scripted frame, in the order Flash runs
// them (bottom layer first). Commands a clip addresses to the main timeline
// are left out unless `documentTimeline`.
QMap<int, QVector<ScriptCommand>> timelineScriptCommands(
    const Timeline &timeline, bool documentTimeline);
// The playhead of a movie clip or document timeline (linear without scripts).
Playhead timelinePlayhead(const Timeline &timeline, bool documentTimeline,
                          QStringList *problems = nullptr);
// A timeline whose frame t shows frame frames[t] of `source`, for building a
// native Xsheet in which nested movie clips keep their own clocks: runs of
// consecutive frames copy their keyframes, a held frame stretches the
// keyframe it shows. Graphic instances entered or held part way show the
// frame they would (symbolFrames gives a symbol's frame count). Labels and
// scripts are left out. A tween held or cut part way shows its keyframe pose,
// listed in `approximations`.
Timeline replayTimeline(const Timeline &source, const QVector<int> &frames,
                        const std::function<int(const QString &)> &symbolFrames,
                        QStringList *approximations = nullptr);

// One point of a keyframe's sound volume envelope (SoundEnvelopePoint).
struct SoundEnvelopePoint {
  qint64 mark44 = 0;                   // Position in 44.1 kHz samples
  int level0 = 32768, level1 = 32768;  // Left/right level; 32768 is 100%
};

// The sound a keyframe starts, stops or streams, evaluated from the DOMFrame
// sound attributes and its SoundEnvelope with the XFL defaults applied.
struct FrameSound {
  enum class Sync { Event, Start, Stop, Stream };
  QString name;  // DOMSoundItem library name
  Sync sync        = Sync::Event;
  bool loopForever = false;  // soundLoopMode="loop"
  int repeat       = 1;      // soundLoop play count, at least 1
  // Played source range in 44.1 kHz samples, whatever the sound's own rate.
  qint64 inPoint44 = 0;
  std::optional<qint64> outPoint44;
  QVector<SoundEnvelopePoint> envelope;
  QString envelopeSourceId;
  // Authored values without a known meaning, as "<sourceId>: <detail>".
  QStringList unmapped;
};

// The frame's sound, or nullopt when it has no soundName. Never throws:
// malformed values keep their defaults and are listed in `unmapped`.
std::optional<FrameSound> frameSound(const Frame &frame);
// True for an extra that frameSound() evaluates (the SoundEnvelope).
bool soundEnvelopeExtra(const Extra &extra);
// The envelope as one constant volume in [0, 1] (1 without an envelope).
// `exact` is false when the level changes over time or differs between the
// channels (fades, panning); the loudest level is returned then.
double soundVolume(const FrameSound &sound, bool *exact = nullptr);

}  // namespace Flash
