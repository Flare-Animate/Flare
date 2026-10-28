#include "flashpreflight.h"
#include "../flash/parttracks.h"
#include "flashbitmap.h"
#include "../flash/effects.h"

#include "../flash/motion.h"
#include "../flash/placement.h"
#include "../flash/shapetween.h"

#include <QColor>
#include <QDir>
#include <QMap>
#include <QSet>
#include <algorithm>
#include <cmath>

namespace FlashNative {
namespace {
void issue(const CompatibilitySink &sink, const QString &code,
           const QString &sourceId, const QString &message) {
  sink({code, sourceId, message});
}

// XFL omits symbolType on movie-clip instances; graphic and button
// instances name their type explicitly.
bool movieClipInstance(const Flash::Element &element) {
  return Flash::symbolInstanceType(element) == "movie clip";
}

bool descriptiveDefinitionExtra(const QString &tag, bool librarySymbol) {
  if (tag == "persistentData" || tag == "metadata") return true;
  return !librarySymbol &&
         (tag == "symbols" || tag == "media" || tag == "folders" ||
          tag == "fonts" || tag == "PrinterSettings" ||
          tag == "PublishSettings" || tag == "publishHistory" ||
          tag == "swatchLists" || tag == "extendedSwatchLists");
}

// Kinds of mask artwork the builder draws as the silhouette Flash clips with
// (Compatibility::maskSilhouette()): the filled area, fully opaque, without
// strokes, colors, gradients or transparency, as Animate's published SWFs
// play (Ruffle writes its mask stencil for every filled pixel and omits
// strokes). Shapes, primitives, groups, text outlines and symbol instances
// (through opaque fill-only copies of their symbols) qualify, also when
// classic or shape tweened. Bitmaps and videos would clip by their alpha.
bool silhouetteMaskElement(const Flash::Element &element) {
  switch (element.kind) {
  case Flash::Element::Kind::Shape:
  case Flash::Element::Kind::Oval:
  case Flash::Element::Kind::Rectangle:
  case Flash::Element::Kind::Text:
  case Flash::Element::Kind::Symbol:
    return true;
  case Flash::Element::Kind::Group:
    return std::all_of(element.members.cbegin(), element.members.cend(),
                       silhouetteMaskElement);
  default:
    return false;
  }
}

// Keep this gate shared by preflight and the builder. One native mask column
// (the union of the mask elements, as their silhouette) clips every masked
// column, also several masked layers and elements. Hidden mask and masked
// layers clip alike when hidden layers are published; the builder reports
// a hidden mask that is not published. A mask or masked layer without
// artwork leaves nothing clipped, in Flash too.
bool supportedMaskPair(const Flash::Timeline &timeline, int maskIndex,
                       int childIndex) {
  if (timeline.layers[childIndex].parent != maskIndex ||
      timeline.layers[childIndex].type != "normal")
    return false;
  for (int index = 0; index < timeline.layers.size(); ++index)
    if (timeline.layers[index].parent == childIndex) return false;
  for (const auto &frame : timeline.layers[maskIndex].frames)
    for (const auto &element : frame.elements)
      if (!silhouetteMaskElement(element)) return false;
  return true;
}

QString maskPairFailure(const Flash::Timeline &timeline, int maskIndex) {
  for (const auto &frame : timeline.layers[maskIndex].frames)
    for (const auto &element : frame.elements)
      if (!silhouetteMaskElement(element))
        return "Linked mask contains a bitmap, video or unmapped element, "
               "which would clip by its pixels' alpha; Flash masks with its "
               "whole frame.";
  return "Linked mask requires normal masked layers directly under the "
         "mask.";
}

// A symbol instance's color effect, filters and blend mode. The builder maps
// them (FlashNative::EffectChain, blendFx()); these are the details it
// approximates or leaves out, whatever the instance's context.
void effects(const Flash::Element &element, const CompatibilitySink &sink) {
  auto located = [&](const QString &code, const QString &detail) {
    const int separator = detail.indexOf(": ");
    issue(sink, code, separator > 0 ? detail.left(separator) : element.sourceId,
          separator > 0 ? detail.mid(separator + 2) : detail);
  };
  for (const auto &detail : Flash::colorEffect(element).unmapped)
    located("unmapped-color-effect", detail);
  const auto filters = Flash::elementFilters(element);
  for (const auto &detail : filters.unmapped)
    located("unmapped-filter", detail);
  const bool graphic = Flash::symbolInstanceType(element) == "graphic";
  const QString mode = Flash::blendMode(element);
  if (graphic) {
    // Flash flattens graphic symbols into their parent when it publishes:
    // their instances have no filters or blend mode.
    bool filtered = false;
    for (const auto &filter : filters.filters) filtered |= filter.effective();
    if (filtered)
      issue(sink, "unmapped-filter", filters.sourceId,
            "Flash does not publish filters on graphic symbol instances; "
            "they are left out.");
    if (mode != "normal" && mode != "layer")
      issue(sink, "unmapped-blend-mode", element.sourceId,
            "Flash does not publish blend modes on graphic symbol "
            "instances; drawn in normal mode.");
    return;
  }
  // A layer blend groups the instance, as every native sub-xsheet is.
  if (mode != "normal" && mode != "layer" && Flash::blendFxType(mode).isEmpty())
    issue(sink, "unmapped-blend-mode", element.sourceId,
          "Blend mode " + mode.left(64) +
              " has no native blending FX; drawn in normal mode.");
  using Kind = Flash::Filter::Kind;
  for (const auto &filter : filters.filters) {
    if (!filter.effective()) continue;
    if (filter.kind == Kind::Blur) {
      // Two passes of an odd whole-pixel box are the Blur FX's triangle.
      const bool exact = filter.quality == 2 && filter.blurX == filter.blurY &&
                         filter.blurX == std::floor(filter.blurX) &&
                         int(filter.blurX) % 2 == 1;
      if (!exact)
        issue(sink, "unmapped-filter", filter.sourceId,
              QString("Flash's box blur (%1 x %2 px, %3 passes) is drawn by "
                      "the Blur FX's triangle filter with the same spread "
                      "(radius %4 px)%5.")
                  .arg(filter.blurX)
                  .arg(filter.blurY)
                  .arg(filter.quality)
                  .arg(Flash::blurRadius(filter.blurX, filter.blurY,
                                         filter.quality),
                       0, 'f', 2)
                  .arg(filter.blurX != filter.blurY
                           ? ", the same in both directions"
                           : ""));
    } else if (filter.kind == Kind::GradientGlow ||
               filter.kind == Kind::GradientBevel)
      issue(sink, "unmapped-filter", filter.sourceId,
            filter.kind == Kind::GradientGlow
                ? QString("The gradient glow is drawn as a glow in the "
                          "gradient's last color%1.")
                      .arg(filter.type == "full" ? ", outside the object" : "")
                : QString("The gradient bevel is drawn as a bevel lit with "
                          "the gradient's last color and shaded with its "
                          "first."));
  }
}
}  // namespace

namespace Compatibility {
IssueHandling handling(const QString &code) {
  // Details the converter can drop or approximate while keeping the element:
  // the native scene shows the artwork and its keyed placement, without the
  // listed source behavior.
  static const QSet<QString> approximate{
      "invalid-stage-background",  // default white stage
      // Scripts are never executed: playhead commands become timing, other
      // statements are dropped (see Flash::parseFrameScript).
      "scripted-playback", "unmapped-script-property", "unmapped-script-event",
      "unmapped-script-load", "unmapped-script-sound", "unmapped-frame-script",
      "unmapped-definition-data", "unmapped-timeline-data",
      "unmapped-layer-data", "unmapped-frame-data", "unmapped-element-data",
      "unmapped-shape-data", "unmapped-primitive-attribute",
      "unmapped-motion-guide",      // positions keyed per frame, or straight
      "unmapped-mask-compositing",  // FX-graph matte from the mask artwork
      "unmapped-tween",             // tween details evaluated without
      "unmapped-layer-parenting",   // parenting keyed frame by frame
      "unmapped-camera",            // camera effects, layer depth
      "unmapped-3d-transform",      // 3D placement drawn with its 2D matrix
      "shape-tween-approximated",   // in-betweens without Animate's morph
      "unmapped-frame-label", "unmapped-key-mode",
      "unmapped-sound-attribute",   // default sync/loop/range used instead
      "unmapped-sound-tail",        // ends at the last frame of its timeline
      "sound-decode-approximated",  // decoded length padded or cut
      "unmapped-blend-mode", "unmapped-cubic-controls",
      // Filters and color effects drawn approximately or left out (see
      // effects() and FlashNative::EffectChain).
      "unmapped-filter", "unmapped-color-effect", "unmapped-movie-clip-clock",
      "unmapped-symbol-playback", "invalid-symbol-pose",
      "unmapped-vector-paint", "unmapped-vector-stroke",
      "unmapped-hairline-stroke", "unpreserved-library-media",
      "empty-primitive-paint",
      "unmapped-text-attribute",  // text drawn without that attribute
      "unmapped-video-audio",     // video imported without its sound
      "vector-fill-approximated"};
  // Elements or layers with no native representation yet. Hidden and
  // degenerate placements are omitted because Flash does not draw them either.
  static const QSet<QString> omit{
      "unmapped-element-type", "unmapped-element-visibility",
      "invalid-native-placement", "unmapped-layer-type", "missing-symbol",
      "missing-bitmap", "unmapped-bitmap-media", "missing-vector-fill",
      "missing-vector-stroke", "invalid-stroke-outline",
      "invalid-shape-element", "vector-conversion-failed",
      // Sounds without decodable embedded samples (or without FFmpeg for
      // MP3), sounds inside graphic symbols (Flash never plays those) and
      // buttons, and movie clip sounds no clip on stage plays.
      "missing-sound", "missing-embedded-media", "unmapped-sound-format",
      "sound-decoder-unavailable", "sound-decode-failed",
      "graphic-symbol-sound", "unmapped-nested-sound", "sound-budget-exceeded",
      // Videos without decodable embedded frames or without FFmpeg.
      "missing-video", "unmapped-video-media", "video-decoder-unavailable",
      "video-decode-failed", "video-budget-exceeded",
      // Sound tracks of videos on layers Flash does not publish (guides,
      // hidden layers without "Include hidden layers"), silent there too.
      "unpublished-video-audio"};
  // Gradient validation details accompany an unmapped-vector-paint issue for
  // the same paint, which is approximated by a solid color.
  if (approximate.contains(code) || code.contains("gradient"))
    return IssueHandling::Approximate;
  if (omit.contains(code)) return IssueHandling::Omit;
  // Structural errors, resource budgets and unknown codes stay fatal.
  return IssueHandling::Fatal;
}

QString handlingName(IssueHandling handling) {
  switch (handling) {
  case IssueHandling::Approximate:
    return "approximated";
  case IssueHandling::Omit:
    return "omitted";
  default:
    return "blocking";
  }
}

bool supportedPassiveFrameLabel(const Flash::Frame &frame) {
  if (!frame.properties.contains("name") ||
      frame.properties.value("name").isEmpty())
    return false;
  const QString type = frame.properties.value("labelType", "name");
  return type == "name" || type == "comment";
}

void options(const Flash::DocumentInfo &info, const BuildOptions &options,
             const CompatibilitySink &sink) {
  if (!QDir::isAbsolutePath(options.assetDirectory) ||
      !std::isfinite(options.dpi) || options.dpi <= 0 ||
      options.maxLevels <= 0 || options.maxColumns <= 0 ||
      options.maxProvenanceEntries <= 0 || options.maxStrokeBindings == 0 ||
      options.maxCells == 0 || info.width <= 0 || info.height <= 0 ||
      !std::isfinite(info.frameRate) || info.frameRate <= 0)
    issue(sink, "invalid-import-options", "DOMDocument.xml",
          "Invalid native import options or stage metadata.");
  for (const auto &value : info.issues) {
    const auto source = value.toObject();
    if (source.value("severity") == "error")
      issue(sink, source.value("code").toString("source-inspection-error"),
            source.value("sourceId")
                .toString(source.value("member").toString("DOMDocument.xml")),
            source.value("message").toString(source.value("detail").toString(
                "Document inspection reported a relationship error.")));
  }
}

void definition(const Flash::Definition &definition, bool librarySymbol,
                const CompatibilitySink &sink) {
  if (librarySymbol && definition.timelines.size() != 1)
    issue(sink, "symbol-timeline-count", definition.member,
          "A library symbol must have one timeline.");
  if (!librarySymbol) {
    if (definition.timelines.isEmpty())
      issue(sink, "missing-timeline", definition.member,
            "Document has no timelines.");
    const QColor background(
        definition.properties.value("backgroundColor", "#FFFFFF"));
    if (!background.isValid())
      issue(sink, "invalid-stage-background", definition.member,
            "Invalid source stage background.");
  }
  for (const auto &extra : definition.extras)
    if (!descriptiveDefinitionExtra(extra.tag, librarySymbol))
      issue(sink,
            extra.tag.contains("script", Qt::CaseInsensitive)
                ? "scripted-playback"
                : "unmapped-definition-data",
            definition.member + "/" + extra.tag,
            "Unmapped definition data: " + extra.tag);
}

void timeline(const Flash::Timeline &timeline, const QString &sourceId,
              const CompatibilitySink &sink) {
  for (const auto &extra : timeline.extras)
    issue(sink, "unmapped-timeline-data", sourceId + "/" + extra.tag,
          "Unmapped timeline data: " + extra.tag);
  for (int index = 0; index < timeline.layers.size(); ++index) {
    int ancestor = timeline.layers[index].parent;
    int steps    = 0;
    while (ancestor >= 0 && ++steps <= timeline.layers.size()) {
      if (ancestor >= timeline.layers.size()) break;
      // A motion guide moves only classic tweens of its layers; the builder
      // maps them to native motion paths and reports what it cannot follow
      // (FlashNative followGuide()). Untweened guided artwork stays put.
      if (timeline.layers[ancestor].type == "guide") break;
      if (timeline.layers[ancestor].type == "mask") {
        if (ancestor != timeline.layers[index].parent ||
            !supportedMaskPair(timeline, ancestor, index))
          issue(sink, "unmapped-mask-compositing",
                sourceId + "/layer" + QString::number(index),
                maskPairFailure(timeline, ancestor));
        break;
      }
      ancestor = timeline.layers[ancestor].parent;
    }
  }
}

void layer(const Flash::Layer &layer, const QString &sourceId,
           const CompatibilitySink &sink) {
  // Camera layers drive the scene camera (FlashNative cameraLayer()).
  if (layer.type != "normal" && layer.type != "folder" &&
      layer.type != "guide" && layer.type != "mask" && layer.type != "camera")
    issue(sink, "unmapped-layer-type", sourceId,
          "Unmapped layer type: " + layer.type);
  for (const auto &extra : layer.extras)
    issue(sink, "unmapped-layer-data", sourceId + "/" + extra.tag,
          "Unmapped layer data: " + extra.tag);
}

void frame(const Flash::Frame &frame, const QString &sourceId,
           const CompatibilitySink &sink, const QString &container) {
  const QString tweenType = frame.properties.value("tweenType", "none");
  const bool tweened      = tweenType != "none";
  // Classic motion tweens become interpolated native stage keys: linear or
  // the quadratic Flash ease, around the source transformation point, with
  // auto/clockwise/counter-clockwise rotation.
  if (tweenType == "motion") {
    // Custom ease curves (<tweens><CustomEase>) are mapped by the builder,
    // and so is orient to path, which acts only on a motion guide.
    if (frame.properties.value("motionTweenScale", "true") != "true")
      issue(sink, "unmapped-tween", sourceId,
            "The tween scale option is not mapped.");
  } else if (tweenType == "shape") {
    // Shape tweens become in-between drawings from Animate's saved morph
    // (MorphShape, shape hints) with the ease; the builder reports any
    // approximation of it as shape-tween-approximated.
  } else if (tweenType == "motion object") {
    // Motion-object tweens (CS4+) become native stage keys from their
    // property curves (FlashNative motionSpan()); curves without a native
    // mapping are listed here.
    const auto motion = Flash::motionObject(frame);
    if (!motion)
      issue(sink, "unmapped-tween", sourceId + "/motionObjectXML",
            "Motion tween without readable motion data (held at its first "
            "frame).");
    else {
      auto report = [&](const QString &code, const QString &detail) {
        const int separator = detail.indexOf(": ");
        issue(sink, code,
              sourceId + "/motionObjectXML/" +
                  (separator > 0 ? detail.left(separator) : QString()),
              separator > 0 ? detail.mid(separator + 2) : detail);
      };
      for (const auto &detail : motion->unmapped3D)
        report("unmapped-3d-transform", detail);
      for (const auto &detail : motion->unmapped)
        report("unmapped-tween", detail);
      // Color curves (alpha, tint, brightness, advanced) become the
      // column's color effect keys, frame by frame (FlashNative
      // applyEffects()).
    }
  } else if (tweened)
    issue(sink, "unmapped-tween", sourceId,
          "Unmapped tween (held at keyframes): " + tweenType);
  // Layer depth (Animate 2019+, a parallax depth seen through the camera).
  bool depthValue = false;
  const double depth =
      frame.properties.value("frameZDepth").toDouble(&depthValue);
  if (depthValue && depth != 0)
    issue(sink, "unmapped-camera", sourceId + "/frameZDepth",
          "Layer depth " + frame.properties.value("frameZDepth") +
              " is not mapped; the layer is drawn at depth 0.");
  // Passive name/comment labels retain their source keyframe and layer IDs in
  // native Xsheet records. Anchor navigation and malformed labels still need
  // behavior beyond an editable badge.
  if ((frame.properties.contains("name") ||
       frame.properties.contains("labelType")) &&
      !supportedPassiveFrameLabel(frame))
    issue(sink, "unmapped-frame-label", sourceId + "/name",
          "Frame label needs a nonempty name and passive name/comment type; "
          "anchor navigation is not mapped.");
  // keyMode is a bit set Animate keeps for the timeline editor: 0x2600 on
  // untweened keys (9728), 0x4600 plus 1 (classic) or 2 (shape) on tweens.
  // In Animate documents 0x0800 is set exactly on the classic tweens
  // whose motionTweenSync is on, and 0x1000 on every classic tween (whose
  // Scale option is on). Untweened keys that keep those two option bits of
  // a removed classic tween (15872, 13824) have no tween for them to act on,
  // and their published SWF frames show the keys held as stored. Other
  // untweened modes are unknown and reported.
  const int keyMode = frame.properties.value("keyMode", "9728").toInt();
  if (!tweened && frame.properties.contains("keyMode") &&
      (keyMode & ~0x1800) != 0x2600)
    issue(sink, "unmapped-key-mode", sourceId + "/keyMode",
          "Frame key mode has no verified native timeline mapping: " +
              frame.properties.value("keyMode"));
  // Sounds are classified by sound(); their envelope belongs to them.
  const bool hasSound = frame.properties.contains("soundName");
  for (const auto &extra : frame.extras)
    // A classic or shape tween's <tweens> holds its custom ease, and a shape
    // tween's MorphShape and morphHintsList its morph: all mapped. Animate
    // also keeps that morph data on keyframes whose shape tween was removed,
    // where it has no effect. Frame scripts are classified below.
    if ((!hasSound || !Flash::soundEnvelopeExtra(extra)) &&
        !(extra.tag == "tweens" &&
          (tweenType == "motion" || tweenType == "shape")) &&
        !Flash::shapeMorphExtra(extra) && !Flash::frameScriptExtra(extra) &&
        // Classified above; a removed motion tween leaves inert curves.
        !Flash::motionObjectExtra(extra))
      issue(sink,
            extra.tag.contains("script", Qt::CaseInsensitive)
                ? "scripted-playback"
                : "unmapped-frame-data",
            sourceId + "/" + extra.tag, "Unmapped frame data: " + extra.tag);
  // Frame scripts: playhead commands (stop, play, gotoAnd*, next/prevFrame)
  // become the timeline's native timing (see Flash::timelinePlayhead); other
  // statements are reported by kind. Flash ignores the scripts of graphic
  // symbols (and buttons have none), so they change nothing there.
  const QString script = Flash::frameScript(frame);
  if (script.trimmed().isEmpty() ||
      (container != "document" && container != "movie clip"))
    return;
  const auto parsed   = Flash::parseFrameScript(script);
  const QString where = sourceId + "/Actionscript";
  for (const auto &problem : parsed.problems)
    issue(sink, problem.code, where, problem.detail);
  if (container != "document")
    for (const auto &command : parsed.commands)
      if (command.root)
        issue(sink, "scripted-playback", where,
              "Frame script of a movie clip controls the main timeline, "
              "which is not translated: \"" +
                  command.text + "\"");
}

std::optional<Flash::FrameSound> sound(const Flash::Frame &frame,
                                       const QString &sourceId,
                                       const QString &container,
                                       const CompatibilitySink &sink) {
  auto result = Flash::frameSound(frame);
  if (!result) return std::nullopt;
  if (container == "graphic") {
    issue(sink, "graphic-symbol-sound", sourceId,
          "Flash does not play sounds inside graphic symbols; sound \"" +
              result->name + "\" is not placed.");
    return std::nullopt;
  }
  // Movie clip sounds play where clips on stage enter their frames; the
  // builder places them on the top-level timeline (and reports those no
  // clip plays). Button sounds follow the mouse.
  if (container != "document" && container != "movie clip") {
    issue(sink, "unmapped-nested-sound", sourceId,
          "Sound \"" + result->name + "\" inside a " + container +
              " is not placed: Flash plays it on mouse events, which native "
              "sound columns cannot follow.");
    return std::nullopt;
  }
  for (const auto &detail : result->unmapped) {
    const int separator = detail.indexOf(": ");
    issue(sink, "unmapped-sound-attribute",
          separator > 0 ? detail.left(separator) : sourceId,
          separator > 0 ? detail.mid(separator + 2) : detail);
  }
  // A constant envelope is the column volume; fades and panning are baked
  // into the sound's WAV file (exact).
  return result;
}

std::optional<Flash::SoundPayload> soundMedia(Flash::Package &package,
                                              const Flash::DocumentInfo &info,
                                              const QString &name,
                                              const QString &sourceId,
                                              bool decoderAvailable,
                                              const CompatibilitySink &sink) {
  const QJsonObject item = info.assets.value(name).toObject();
  if (name.isEmpty() || item.value("type") != "DOMSoundItem") {
    issue(sink, "missing-sound", sourceId,
          "Missing sound library item: " + name);
    return std::nullopt;
  }
  QMap<QString, QString> attributes;
  for (auto it = item.constBegin(); it != item.constEnd(); ++it)
    if (it.key() != "type") attributes.insert(it.key(), it.value().toString());
  try {
    auto sound = Flash::readEmbeddedSound(package, attributes);
    if (!sound.decoded() && !decoderAvailable) {
      issue(sink, "sound-decoder-unavailable", sourceId,
            "Sound \"" + name +
                "\" is not placed: it is MP3-compressed and FFmpeg, which "
                "decodes it, was not found (set its folder in Preferences > "
                "Import/Export).");
      return std::nullopt;
    }
    return sound;
  } catch (const Flash::SoundMediaError &error) {
    issue(sink,
          error.kind() == Flash::SoundMediaError::Kind::Missing
              ? "missing-embedded-media"
              : "unmapped-sound-format",
          sourceId,
          "Sound \"" + name +
              "\" is not placed: " + QString::fromUtf8(error.what()));
  } catch (const Flash::Error &error) {
    // Cancellation escapes; an unreadable member omits only this sound.
    package.checkCancelled();
    issue(sink, "unmapped-sound-format", sourceId,
          "Sound \"" + name +
              "\" is not placed: " + QString::fromUtf8(error.what()));
  }
  return std::nullopt;
}

std::optional<Flash::VideoPayload> videoMedia(Flash::Package &package,
                                              const Flash::DocumentInfo &info,
                                              const QString &name,
                                              const QString &sourceId,
                                              bool decoderAvailable,
                                              const CompatibilitySink &sink) {
  const QJsonObject item = info.assets.value(name).toObject();
  if (name.isEmpty() || item.value("type") != "DOMVideoItem") {
    issue(sink, "missing-video", sourceId,
          "Missing video library item: " + name);
    return std::nullopt;
  }
  QMap<QString, QString> attributes;
  for (auto it = item.constBegin(); it != item.constEnd(); ++it)
    if (it.key() != "type") attributes.insert(it.key(), it.value().toString());
  try {
    auto video = Flash::readEmbeddedVideo(package, attributes);
    if (!decoderAvailable) {
      issue(sink, "video-decoder-unavailable", sourceId,
            "Video \"" + name +
                "\" is not placed: FFmpeg, which decodes Flash video, was "
                "not found (set its folder in Preferences > Import/Export).");
      return std::nullopt;
    }
    // An audible track is placed with the video's instances (the builder
    // reports the instances whose track does not play natively).
    if (video.sound == Flash::VideoPayload::Sound::Unreadable)
      issue(sink, "unmapped-video-audio", sourceId,
            "Video \"" + name +
                "\" is imported without its own sound track, which is not "
                "raw PCM covering the video (" +
                video.soundProblem + ").");
    return video;
  } catch (const Flash::VideoMediaError &error) {
    issue(sink,
          error.kind() == Flash::VideoMediaError::Kind::Missing
              ? "missing-embedded-media"
              : "unmapped-video-media",
          sourceId,
          "Video \"" + name +
              "\" is not placed: " + QString::fromUtf8(error.what()));
  } catch (const Flash::Error &error) {
    // Cancellation escapes; an unreadable member omits only this video.
    package.checkCancelled();
    issue(sink, "unmapped-video-media", sourceId,
          "Video \"" + name +
              "\" is not placed: " + QString::fromUtf8(error.what()));
  }
  return std::nullopt;
}

void element(const Flash::Element &element, const Flash::Frame &frame,
             double dpi, const CompatibilitySink &sink,
             const CompatibilityCheck &check, quint64 *usedStyleBindings) {
  if (usedStyleBindings) *usedStyleBindings = 0;
  // Structured text retains its whole subtree in extras; textLayout() lists
  // the parts that have no layout mapping.
  const bool text = element.kind == Flash::Element::Kind::Text && element.text;
  // Symbol instances map their color effect (column opacity and color FX),
  // filters and blend mode to native FX (see FlashNative::EffectChain).
  const bool symbol = element.kind == Flash::Element::Kind::Symbol;
  for (const auto &extra : element.extras)
    if (!text && !(symbol && (extra.tag == "color" || extra.tag == "filters")))
      issue(sink, "unmapped-element-data", element.sourceId + "/" + extra.tag,
            "Unmapped element rendering data: " + extra.tag);
  if (symbol) effects(element, sink);
  if (text)
    for (const auto &detail : Flash::textLayout(element).unmapped) {
      const int separator = detail.indexOf(": ");
      issue(sink, "unmapped-text-attribute",
            separator > 0 ? detail.left(separator) : element.sourceId,
            "Unmapped text attribute: " +
                (separator > 0 ? detail.mid(separator + 2) : detail));
    }
  if (!symbol && element.properties.value("blendMode", "normal") != "normal")
    issue(sink, "unmapped-blend-mode", element.sourceId,
          "Unmapped blend mode: " + element.properties.value("blendMode"));
  if (element.properties.value("visible", "true") != "true")
    issue(sink, "unmapped-element-visibility", element.sourceId,
          "Element visibility does not yet have a native import mapping.");
  // 3D placement (CS4+ 3D rotation and translation tools): the stage keys
  // carry the instance's 2D matrix.
  QStringList depth;
  for (const char *name : {"rotationX", "rotationY", "matrix3D"})
    if (element.properties.contains(name)) depth.push_back(name);
  if (element.properties.value("centerPoint3DZ", "0").toDouble() != 0)
    depth.push_back("centerPoint3DZ");
  if (!depth.isEmpty())
    issue(sink, "unmapped-3d-transform", element.sourceId,
          "3D placement (" + depth.join(", ") +
              ") is not mapped; the instance is drawn with its 2D matrix.");
  if (Flash::motionObjectFrame(frame))
    if (const auto motion = Flash::motionObject(frame)) {
      bool ambiguous = false;
      Flash::motionRelative(*motion, element.matrix, &ambiguous);
      if (ambiguous)
        issue(sink, "unmapped-tween", element.sourceId,
              "The motion tween's rotation, skew and scale at its first "
              "frame match neither the instance's transform nor the "
              "identity; they are applied as absolute values.");
    }
  try {
    Flash::stagePlacement(element.matrix, dpi);
  } catch (const Flash::Error &error) {
    issue(sink, "invalid-native-placement", element.sourceId,
          QString::fromUtf8(error.what()));
  }
  switch (element.kind) {
  case Flash::Element::Kind::Shape:
  case Flash::Element::Kind::Oval:
  case Flash::Element::Kind::Rectangle:
    vector(element, sink, check, usedStyleBindings);
    if (element.shape)
      for (int index = 0; index < element.shape->edges.size(); ++index)
        if (!element.shape->edges[index].cubicControlData.isEmpty()) {
          // Drawable quadratic edges can be converted independently for
          // geometry diagnostics. A complete editable scene cannot discard
          // these additional authoring controls without disclosure.
          issue(sink, "unmapped-cubic-controls",
                element.sourceId + "/edge" + QString::number(index) + "/cubics",
                "Cubic shape authoring data has no editable native mapping.");
          break;
        }
    break;
  case Flash::Element::Kind::Symbol:
    if (movieClipInstance(element)) {
      // Nested clips keep their own clocks: the builder checks every
      // placement of the symbol holding them and replays it where the
      // native local clock would differ (unmapped-movie-clip-clock reports
      // what stays different).
      try {
        Flash::movieClipTiming(element, frame);
      } catch (const Flash::Error &error) {
        issue(sink, "unmapped-symbol-playback", element.sourceId,
              QString::fromUtf8(error.what()));
      }
    } else
      try {
        Flash::instanceTiming(element, frame);
      } catch (const Flash::Error &error) {
        issue(sink, "unmapped-symbol-playback", element.sourceId,
              QString::fromUtf8(error.what()));
      }
    break;
  case Flash::Element::Kind::Group:
  case Flash::Element::Kind::Bitmap:
  case Flash::Element::Kind::Video:  // media checked by videoMedia()
    break;
  case Flash::Element::Kind::Text:
    if (text) break;
    [[fallthrough]];
  default:
    issue(sink, "unmapped-element-type", element.sourceId,
          "Unmapped element type: " + element.tag);
  }
}

void reject(const CompatibilityIssue &issue) {
  throw Flash::Error(issue.sourceId + ": " + issue.message);
}

Flash::Shape maskSilhouette(const Flash::Shape &shape) {
  Flash::Shape silhouette;
  for (auto it = shape.fills.cbegin(); it != shape.fills.cend(); ++it) {
    Flash::Paint paint = it.value();
    if (!paint.stops.isEmpty() &&
        (paint.kind == Flash::Paint::Kind::LinearGradient ||
         paint.kind == Flash::Paint::Kind::RadialGradient)) {
      double sum[3] = {0, 0, 0};
      for (const auto &stop : paint.stops) {
        sum[0] += stop.color.red;
        sum[1] += stop.color.green;
        sum[2] += stop.color.blue;
      }
      const auto mean = [&](double value) {
        return (unsigned char)std::lround(value / paint.stops.size());
      };
      paint.color.red   = mean(sum[0]);
      paint.color.green = mean(sum[1]);
      paint.color.blue  = mean(sum[2]);
    }
    paint.kind        = Flash::Paint::Kind::Solid;
    paint.color.alpha = 255;
    paint.stops.clear();
    paint.matrix = {};
    paint.matrixXml.reset();
    paint.extras.clear();
    silhouette.fills.insert(it.key(), paint);
  }
  for (const auto &edge : shape.edges) {
    if (!edge.fill0 && !edge.fill1) continue;
    Flash::Edge filled = edge;
    filled.stroke      = 0;
    silhouette.edges.push_back(std::move(filled));
  }
  return silhouette;
}
}  // namespace Compatibility

namespace {
class Preflight {
  Flash::Package &m_package;
  const Flash::DocumentInfo &m_info;
  const BuildOptions &m_options;
  QJsonArray m_issues;
  quint64 m_issueCount = 0, m_errorCount = 0;
  quint64 m_approximatedCount = 0, m_omittedCount = 0;
  QMap<QString, quint64> m_issueCodeCounts, m_errorCodeCounts;
  quint64 m_cells = 0, m_columns = 0, m_levels = 0, m_geometry = 0;
  quint64 m_provenanceEntries = 0;
  quint64 m_styleRecords      = 0;
  quint64 m_strokeRecords     = 0;
  quint64 m_bitmapPixels      = 0;
  quint64 m_displaySlotLayers = 0, m_displaySlotKeyframes = 0;
  quint64 m_displaySlotPlacements = 0;
  QJsonArray m_displaySlotLayerSamples;
  QMap<QString, bool> m_pending;
  QSet<QString> m_visited, m_budgetIssues, m_bitmaps;
  QSet<QString> m_soundReferences, m_videoReferences;
  // Type of the definition being walked: "document" or its symbol type.
  QString m_container = "document";
  // Sound items checked for a placed cue, with the reason one is unusable.
  QMap<QString, std::optional<CompatibilityIssue>> m_soundMedia;
  bool m_blank = false, m_stopReading = false;
  int m_maskArtwork = 0;  // walking the artwork of a linked mask layer
  bool m_mediaReferencesComplete                  = true;
  static constexpr int maxIssues                  = 1000;
  static constexpr int maxDisplaySlotLayerSamples = 12;

  void add(QString severity, const QString &code, const QString &sourceId,
           const QString &message) {
    m_package.checkCancelled();
    ++m_issueCount;
    ++m_issueCodeCounts[code];
    // A best-effort import keeps going past details it can approximate or
    // omit; only blocking issues remain errors.
    IssueHandling handling = IssueHandling::Fatal;
    if (severity == "error" && m_options.bestEffort) {
      handling = Compatibility::handling(code);
      if (handling != IssueHandling::Fatal) {
        severity = "warning";
        ++(handling == IssueHandling::Omit ? m_omittedCount
                                           : m_approximatedCount);
      }
    }
    if (severity == "error") {
      ++m_errorCount;
      ++m_errorCodeCounts[code];
    }
    if (m_issues.size() < maxIssues) {
      QJsonObject issue{{"severity", severity},
                        {"code", code},
                        {"sourceId", sourceId},
                        {"message", message}};
      if (m_options.bestEffort && severity == "warning" &&
          handling != IssueHandling::Fatal)
        issue["handling"] = Compatibility::handlingName(handling);
      m_issues.append(issue);
    }
  }

  CompatibilitySink sink() {
    return [this](const CompatibilityIssue &issue) {
      add("error", issue.code, issue.sourceId, issue.message);
    };
  }

  void budget(const QString &code, const QString &sourceId,
              const QString &message) {
    if (m_budgetIssues.contains(code)) return;
    m_budgetIssues.insert(code);
    add("error", code, sourceId, message);
  }

  void resources(const QString &sourceId) {
    if (m_cells > m_options.maxCells)
      budget("native-exposure-budget", sourceId,
             "Native exposure budget exceeded.");
    if (m_columns > quint64(std::max(0, m_options.maxColumns)))
      budget("native-column-budget", sourceId,
             "Native column budget exceeded.");
    if (m_levels > quint64(std::max(0, m_options.maxLevels)))
      budget("native-level-budget", sourceId, "Native level budget exceeded.");
    if (m_provenanceEntries >
        quint64(std::max(0, m_options.maxProvenanceEntries)))
      budget("native-provenance-budget", sourceId,
             "Native source provenance budget exceeded.");
    if (m_styleRecords > quint64(std::max(0, m_options.maxProvenanceEntries)))
      budget("native-style-provenance-budget", sourceId,
             "Native source style provenance budget exceeded.");
    if (m_strokeRecords > m_options.maxStrokeBindings)
      budget("native-stroke-provenance-budget", sourceId,
             "Native source stroke provenance budget exceeded.");
  }

  void symbolReference(const Flash::Element &element, const Flash::Frame &frame,
                       bool linearRootClock) {
    const auto name   = element.properties.value("libraryItemName");
    const auto source = m_info.symbols.constFind(name);
    if (source == m_info.symbols.cend()) {
      add("error", "missing-symbol", element.sourceId,
          "Missing source symbol: " + name);
      return;
    }
    if (!m_visited.contains(name)) m_pending.insert(name, true);
    if (source->timelines.size() != 1) return;
    try {
      const int count = std::max(1, source->timelines.front().frameCount);
      if (!movieClipInstance(element)) {
        auto timing = Flash::instanceTiming(element, frame);
        // A first frame past the end plays as in Animate (the builder wraps
        // loops and rests other modes on the last frame).
        if (!timing.lastFrame && timing.firstFrame >= count)
          timing.firstFrame =
              Flash::wrappedFirstFrame(timing.firstFrame, timing.mode, count);
        timing.sourceFrameAt(frame.index, count);
      } else if (linearRootClock)
        Flash::movieClipTiming(element, frame)
            .sourceFrameAt(frame.index, count);
    } catch (const Flash::Error &error) {
      // Non-graphic mode was already reported by the shared element guard.
      if (!movieClipInstance(element))
        add("error", "invalid-symbol-pose", element.sourceId,
            QString::fromUtf8(error.what()));
    }
  }

  // Bitmap fills draw their library bitmap (TFlashBitmapStyle). Each item is
  // decoded once; a fill whose bitmap cannot be decoded is drawn solid, and
  // reported for every shape that uses it, as the builder does.
  QMap<QString, QString> m_fillBitmapProblems;
  void bitmapFills(const Flash::Element &element) {
    const auto &shape = *element.shape;
    QSet<int> fills, strokes;
    for (const auto &edge : shape.edges) {
      if (edge.segments.isEmpty()) continue;
      if (edge.fill0) fills.insert(edge.fill0);
      if (edge.fill1) fills.insert(edge.fill1);
      if (edge.stroke) strokes.insert(edge.stroke);
    }
    QVector<const Flash::Paint *> paints;
    for (int index : fills) {
      const auto found = shape.fills.constFind(index);
      if (found != shape.fills.cend()) paints.push_back(&found.value());
    }
    for (int index : strokes) {
      const auto found = shape.strokes.constFind(index);
      if (found != shape.strokes.cend()) paints.push_back(&found.value().paint);
    }
    for (const Flash::Paint *paint : paints) {
      if (!Compatibility::mappedBitmapFill(*paint)) continue;
      const QString name = paint->properties.value("bitmapPath");
      if (!m_fillBitmapProblems.contains(name)) {
        QString problem;
        try {
          const TRasterImageP image =
              bitmapImage(m_package, m_info, name, m_options.dpi);
          if (!image || !image->getRaster())
            throw Flash::Error("Bitmap decoder returned no raster.");
          m_bitmaps.insert(name);
        } catch (const Flash::Error &error) {
          m_package.checkCancelled();
          problem = QString::fromUtf8(error.what());
        }
        m_fillBitmapProblems.insert(name, problem);
      }
      const QString problem = m_fillBitmapProblems.value(name);
      if (!problem.isEmpty())
        sink()({"unmapped-vector-paint", paint->sourceId,
                "The bitmap of this bitmap fill cannot be decoded; it is "
                "drawn in a solid color: " +
                    problem});
    }
  }

  void bitmapReference(const Flash::Element &element) {
    const QString name = element.properties.value("libraryItemName");
    if (name.isEmpty()) {
      add("error", "missing-bitmap", element.sourceId,
          "Bitmap instance has no library item name.");
      return;
    }
    if (m_bitmaps.contains(name)) return;
    m_bitmaps.insert(name);
    ++m_provenanceEntries;
    try {
      const TRasterImageP image =
          bitmapImage(m_package, m_info, name, m_options.dpi);
      if (!image || !image->getRaster())
        throw Flash::Error("Bitmap decoder returned no raster.");
      const quint64 pixels = quint64(image->getRaster()->getLx()) *
                             quint64(image->getRaster()->getLy());
      if (pixels > m_options.maxBitmapPixels -
                       std::min(m_bitmapPixels, m_options.maxBitmapPixels)) {
        budget("native-bitmap-budget", element.sourceId,
               "Native bitmap pixel budget exceeded.");
        return;
      }
      m_bitmapPixels += pixels;
      ++m_levels;
    } catch (const Flash::Error &error) {
      m_package.checkCancelled();
      add("error", "unmapped-bitmap-media", element.sourceId,
          QString::fromUtf8(error.what()));
    }
  }

  void unpreservedLibraryMedia() {
    // Selected-content import intentionally omits unselected library data.
    // For full-library import, the builder currently materializes media only
    // through placements/cues; it has no unused-media preservation pass.
    if (!m_options.includeUnusedSymbols || m_stopReading ||
        !m_mediaReferencesComplete)
      return;
    for (auto item = m_info.assets.constBegin();
         item != m_info.assets.constEnd(); ++item) {
      m_package.checkCancelled();
      const auto type = item.value().toObject().value("type").toString();
      bool referenced = true;
      if (type == "DOMBitmapItem")
        referenced = m_bitmaps.contains(item.key());
      else if (type == "DOMSoundItem")
        referenced = m_soundReferences.contains(item.key());
      else if (type == "DOMVideoItem")
        referenced = m_videoReferences.contains(item.key());
      if (!referenced)
        add("error", "unpreserved-library-media",
            "DOMDocument.xml/media/" + type + "/" + item.key(),
            "Unreferenced library media has no preserved native library "
            "item. Full-library import cannot omit it: " +
                type);
    }
  }

  void elements(const QVector<Flash::Element> &elements,
                const Flash::Frame &frame, bool linearRootClock) {
    for (const auto &element : elements) {
      m_package.checkCancelled();
      ++m_provenanceEntries;
      quint64 usedStyleBindings = 0;
      // Linked mask artwork is drawn as its silhouette: check that drawing.
      Flash::Element silhouette;
      const bool maskShape = m_maskArtwork > 0 && element.shape &&
                             (element.kind == Flash::Element::Kind::Shape ||
                              element.kind == Flash::Element::Kind::Oval ||
                              element.kind == Flash::Element::Kind::Rectangle);
      if (maskShape) {
        silhouette       = element;
        silhouette.shape = std::make_shared<Flash::Shape>(
            Compatibility::maskSilhouette(*element.shape));
      }
      Compatibility::element(
          maskShape ? silhouette : element, frame, m_options.dpi, sink(),
          [this] { m_package.checkCancelled(); }, &usedStyleBindings);
      m_styleRecords += usedStyleBindings;
      if (!maskShape && element.shape &&
          element.kind == Flash::Element::Kind::Shape)
        bitmapFills(element);
      if (element.shape && (element.kind == Flash::Element::Kind::Shape ||
                            element.kind == Flash::Element::Kind::Oval ||
                            element.kind == Flash::Element::Kind::Rectangle))
        for (const auto &edge : element.shape->edges) {
          if (edge.segments.isEmpty()) continue;
          ++m_strokeRecords;
          for (int segment = 1; segment < edge.segments.size(); ++segment)
            if (edge.segments[segment].startsContour) ++m_strokeRecords;
        }
      if (element.kind == Flash::Element::Kind::Symbol)
        symbolReference(element, frame, linearRootClock);
      else if (element.kind == Flash::Element::Kind::Bitmap)
        bitmapReference(element);
      else if (element.kind == Flash::Element::Kind::Video)
        videoReference(element);
      else if (element.kind == Flash::Element::Kind::Group) {
        ++m_levels;
        Flash::Timeline group;
        group.frameCount = frame.duration;
        Flash::Layer layer;
        layer.name = "Group members";
        Flash::Frame members;
        members.duration = frame.duration;
        members.elements = element.members;
        layer.frames.push_back(std::move(members));
        group.layers.push_back(std::move(layer));
        const QString container = m_container;
        m_container             = "group";
        timeline(group, element.sourceId, false, false);
        m_container = container;
      }
      resources(element.sourceId);
    }
  }

  // Checks a placed video's library item once; reports its issues (an
  // omission or the sound-track approximation) for every instance.
  QMap<QString, QVector<CompatibilityIssue>> m_videoMedia;
  void videoReference(const Flash::Element &element) {
    const QString name = element.properties.value("libraryItemName");
    m_videoReferences.insert(name);
    auto status = m_videoMedia.constFind(name);
    if (status == m_videoMedia.cend()) {
      QVector<CompatibilityIssue> issues;
      const bool placed =
          Compatibility::videoMedia(m_package, m_info, name, element.sourceId,
                                    bool(m_options.decodeVideo),
                                    [&issues](const CompatibilityIssue &issue) {
                                      issues.push_back(issue);
                                    })
              .has_value();
      if (placed) ++m_levels;
      status = m_videoMedia.insert(name, issues);
    }
    for (CompatibilityIssue issue : status.value()) {
      issue.sourceId = element.sourceId;
      add("error", issue.code, issue.sourceId, issue.message);
    }
  }

  // Checks the sound a placed cue plays, once per library item; reports the
  // same omission for every cue of an unusable item.
  bool soundCue(const QString &name, const QString &sourceId) {
    auto status = m_soundMedia.constFind(name);
    if (status == m_soundMedia.cend()) {
      std::optional<CompatibilityIssue> failure;
      Compatibility::soundMedia(
          m_package, m_info, name, sourceId, bool(m_options.decodeSound),
          [&failure](const CompatibilityIssue &issue) { failure = issue; });
      if (!failure) ++m_levels;
      status = m_soundMedia.insert(name, failure);
    }
    if (!status.value()) return true;
    add("error", status.value()->code, sourceId, status.value()->message);
    return false;
  }

  void timeline(const Flash::Timeline &timeline, const QString &sourceId,
                bool linearRootClock, bool authoredLayers) {
    Compatibility::timeline(timeline, sourceId, sink());
    int lastExposure = 0;
    for (int layerIndex = 0; layerIndex < timeline.layers.size();
         ++layerIndex) {
      m_package.checkCancelled();
      const auto &layer  = timeline.layers[layerIndex];
      const auto layerId = sourceId + "/layer" + QString::number(layerIndex);
      Compatibility::layer(layer, layerId, sink());
      const bool fixedMaskOrder =
          layer.type == "mask" ||
          (layer.parent >= 0 && layer.parent < timeline.layers.size() &&
           timeline.layers[layer.parent].type == "mask");
      const auto tracks = Flash::planLayerTracks(layer, fixedMaskOrder);
      const bool singleElementLayer = std::all_of(
          layer.frames.cbegin(), layer.frames.cend(),
          [](const Flash::Frame &frame) { return frame.elements.size() <= 1; });
      quint64 displaySlotKeyframesInLayer = 0;
      QSet<int> drawings;
      bool soundLayer = false;
      // A mask layer that clips other layers is drawn as the silhouette of
      // its artwork (see elements()).
      bool linkedMask = false;
      for (int index = layerIndex + 1; layer.type == "mask" && !linkedMask &&
                                       index < timeline.layers.size();
           ++index) {
        int ancestor = timeline.layers[index].parent, steps = 0;
        while (ancestor > layerIndex && ancestor < timeline.layers.size() &&
               ++steps <= timeline.layers.size())
          ancestor = timeline.layers[ancestor].parent;
        linkedMask = ancestor == layerIndex;
      }
      if (linkedMask) ++m_maskArtwork;
      struct MaskArtwork {
        int &depth;
        const bool active;
        ~MaskArtwork() {
          if (active) --depth;
        }
      } maskArtwork{m_maskArtwork, linkedMask};
      for (int frameIndex = 0; frameIndex < layer.frames.size(); ++frameIndex) {
        const auto &frame = layer.frames[frameIndex];
        m_package.checkCancelled();
        const auto frameId = layerId + "/frame" + QString::number(frame.index);
        Compatibility::frame(frame, frameId, sink(), m_container);
        if (frame.properties.contains("soundName"))
          m_soundReferences.insert(frame.properties.value("soundName"));
        if (const auto sound =
                Compatibility::sound(frame, frameId, m_container, sink()))
          if (sound->sync != Flash::FrameSound::Sync::Stop &&
              soundCue(sound->name, frameId))
            soundLayer = true;
        m_cells += quint64(frame.duration) * quint64(frame.elements.size());
        if (!frame.elements.isEmpty())
          lastExposure = std::max(lastExposure, frame.index + frame.duration);
        bool hasDisplaySlot = false;
        for (int slot = 0; slot < frame.elements.size(); ++slot) {
          const int track = tracks.frameTracks[frameIndex][slot];
          if (authoredLayers && !singleElementLayer &&
              tracks.trackNames[track].isEmpty()) {
            ++m_displaySlotPlacements;
            hasDisplaySlot = true;
          }
          if (frame.elements[slot].kind == Flash::Element::Kind::Shape ||
              frame.elements[slot].kind == Flash::Element::Kind::Oval ||
              frame.elements[slot].kind == Flash::Element::Kind::Rectangle ||
              frame.elements[slot].kind == Flash::Element::Kind::Text)
            drawings.insert(track);
          // Text FX columns: at most one per run that draws something on
          // each line (FlashNative::textFxItems), exposed on the keyframe.
          if (frame.elements[slot].kind == Flash::Element::Kind::Text &&
              frame.elements[slot].text) {
            quint64 runs = 0;
            for (const auto &run : frame.elements[slot].text->runs)
              if (!run.characters.trimmed().isEmpty()) ++runs;
            m_columns += runs;
            m_cells += runs * quint64(frame.duration);
          }
        }
        if (hasDisplaySlot) {
          ++m_displaySlotKeyframes;
          ++displaySlotKeyframesInLayer;
        }
        elements(frame.elements, frame, linearRootClock);
      }
      if (displaySlotKeyframesInLayer) {
        ++m_displaySlotLayers;
        if (m_displaySlotLayerSamples.size() < maxDisplaySlotLayerSamples)
          m_displaySlotLayerSamples.append(
              QJsonObject{{"sourceLayerId", layerId},
                          {"sourceLayerName", layer.name},
                          {"keyframes", double(displaySlotKeyframesInLayer)}});
      }
      m_columns += tracks.trackCount + (soundLayer ? 1 : 0);
      m_levels += drawings.size();
      resources(layerId);
    }
    if (lastExposure < std::max(1, timeline.frameCount)) {
      ++m_columns;
      ++m_cells;
      if (!m_blank) {
        ++m_levels;
        m_blank = true;
      }
      resources(sourceId);
    }
  }

  void read(const QString &member, bool librarySymbol) {
    m_package.checkCancelled();
    Flash::Definition definition;
    try {
      definition = Flash::readDefinition(m_package, member);
    } catch (const Flash::Error &error) {
      // Cancellation must escape, rather than becoming an ordinary blocker.
      m_package.checkCancelled();
      m_mediaReferencesComplete = false;
      add("error", "definition-read-error", member,
          QString::fromUtf8(error.what()));
      return;
    }
    Compatibility::definition(definition, librarySymbol, sink());
    const auto remaining =
        m_package.limits().maxGeometrySegments -
        std::min(m_geometry, m_package.limits().maxGeometrySegments);
    if (definition.geometrySegments > remaining) {
      budget("native-geometry-budget", member,
             "Native geometry budget exceeded.");
      m_stopReading = true;
      return;
    }
    m_geometry += definition.geometrySegments;
    if (!librarySymbol && definition.timelines.size() > 1) {
      m_provenanceEntries += definition.timelines.size();
      ++m_columns;
      m_levels += definition.timelines.size();
      quint64 duration = 0;
      for (const auto &source : definition.timelines)
        duration += std::max(1, source.frameCount);
      m_cells += duration;
      if (duration > quint64(std::numeric_limits<int>::max()))
        budget("native-frame-budget", member,
               "Source scene duration exceeds native frame limits.");
      resources(member);
    }
    m_container = !librarySymbol              ? QString("document")
                  : definition.type.isEmpty() ? QString("movie clip")
                                              : definition.type;
    for (int index = 0; index < definition.timelines.size(); ++index)
      timeline(definition.timelines[index],
               member + "/timeline" + QString::number(index), !librarySymbol,
               true);
  }

public:
  Preflight(Flash::Package &package, const Flash::DocumentInfo &info,
            const BuildOptions &options)
      : m_package(package), m_info(info), m_options(options) {}

  QJsonObject run() {
    m_package.checkCancelled();
    Compatibility::options(m_info, m_options, sink());
    for (const auto &value : m_info.issues) {
      const auto source = value.toObject();
      if (source.value("severity") == "warning")
        add("warning", source.value("code").toString("source-warning"),
            source.value("sourceId")
                .toString(source.value("member").toString("DOMDocument.xml")),
            source.value("message").toString(
                source.value("detail").toString()));
    }
    read("DOMDocument.xml", false);
    if (m_options.includeUnusedSymbols)
      for (auto it = m_info.symbols.cbegin(); it != m_info.symbols.cend(); ++it)
        m_pending.insert(it.key(), true);
    while (!m_pending.isEmpty() && !m_stopReading) {
      m_package.checkCancelled();
      const auto name = m_pending.cbegin().key();
      m_pending.remove(name);
      if (m_visited.contains(name)) continue;
      m_visited.insert(name);
      ++m_levels;
      ++m_provenanceEntries;
      const auto found = m_info.symbols.constFind(name);
      if (found == m_info.symbols.cend()) continue;
      resources(found->member);
      read(found->member, true);
    }
    unpreservedLibraryMedia();
    auto source = m_info.toJson();
    QJsonArray sourceIssues;
    for (int index = 0; index < std::min(maxIssues, m_info.issues.size());
         ++index)
      sourceIssues.append(m_info.issues.at(index));
    source["issues"] = sourceIssues;
    QJsonObject issueCodeCounts, errorCodeCounts;
    for (auto it = m_issueCodeCounts.cbegin(); it != m_issueCodeCounts.cend();
         ++it)
      issueCodeCounts.insert(it.key(), double(it.value()));
    for (auto it = m_errorCodeCounts.cbegin(); it != m_errorCodeCounts.cend();
         ++it)
      errorCodeCounts.insert(it.key(), double(it.value()));
    QJsonObject report{
        {"schemaVersion", 1},
        {"canImport", m_errorCount == 0},
        {"scope", "implemented-native-feature-mapping"},
        {"visualFidelityVerified", false},
        {"issues", m_issues},
        {"issueCodeCounts", issueCodeCounts},
        {"errorCodeCounts", errorCodeCounts},
        {"totalIssueCount", double(m_issueCount)},
        {"errorCount", double(m_errorCount)},
        {"bestEffort", m_options.bestEffort},
        {"approximatedCount", double(m_approximatedCount)},
        {"omittedCount", double(m_omittedCount)},
        {"truncated", m_issueCount > quint64(m_issues.size())},
        {"inspectionStoppedForResourceLimit", m_stopReading},
        {"source", source},
        {"sourceIssueCount", m_info.issues.size()},
        {"sourceIssuesTruncated", m_info.issues.size() > maxIssues},
        {"estimatedNativeCells", double(m_cells)},
        {"estimatedNativeColumns", double(m_columns)},
        {"estimatedNativeLevels", double(m_levels)},
        {"estimatedNativeBitmapPixels", double(m_bitmapPixels)},
        {"estimatedNativeSourceBindings", double(m_provenanceEntries)},
        {"estimatedNativeStyleBindings", double(m_styleRecords)},
        {"estimatedNativeStrokeBindings", double(m_strokeRecords)},
        {"displaySlotLayerCount", double(m_displaySlotLayers)},
        {"displaySlotKeyframeCount", double(m_displaySlotKeyframes)},
        {"displaySlotPlacementCount", double(m_displaySlotPlacements)},
        {"displaySlotLayerSamples", m_displaySlotLayerSamples},
        {"displaySlotLayerSamplesTruncated",
         m_displaySlotLayers > quint64(m_displaySlotLayerSamples.size())},
        {"selectedLibrarySymbols", m_visited.size()},
        {"includeUnusedSymbols", m_options.includeUnusedSymbols}};
    return report;
  }
};
}  // namespace

QJsonObject preflightScene(Flash::Package &package,
                           const Flash::DocumentInfo &info,
                           const BuildOptions &options) {
  return Preflight(package, info, options).run();
}

}  // namespace FlashNative
