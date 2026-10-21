#pragma once

#include "flashscene.h"
#include "../flash/media.h"
#include "../flash/model.h"
#include "../flash/video.h"

#include <functional>

// This header may follow raster headers, which redefine DVAPI for a different
// library. These declarations are implemented by flarelib.
#undef DVAPI
#ifdef FLARELIB_EXPORTS
#define DVAPI DV_EXPORT_API
#else
#define DVAPI DV_IMPORT_API
#endif

namespace FlashNative {

// The converter and the read-only preflight use these same feature guards.
// A passing guard establishes eligibility for the implemented mapping; it is
// not a visual comparison against Adobe Animate.
struct CompatibilityIssue {
  QString code;
  QString sourceId;
  QString message;
};

using CompatibilitySink  = std::function<void(const CompatibilityIssue &)>;
using CompatibilityCheck = std::function<void()>;

// How a best-effort import treats an issue. Strict imports treat every issue
// as Fatal. Approximate keeps the element with the unmapped detail dropped or
// approximated; Omit leaves that element (or layer) out of the native scene.
// Every non-fatal issue is listed in the import report.
enum class IssueHandling { Fatal, Approximate, Omit };

namespace Compatibility {
DVAPI IssueHandling handling(const QString &code);
DVAPI QString handlingName(IssueHandling handling);
// Name/comment labels are passive authored timeline annotations. Anchor
// navigation semantics and malformed/incomplete labels remain unsupported.
DVAPI bool supportedPassiveFrameLabel(const Flash::Frame &frame);
DVAPI void options(const Flash::DocumentInfo &info, const BuildOptions &options,
                   const CompatibilitySink &sink);
DVAPI void definition(const Flash::Definition &definition, bool librarySymbol,
                      const CompatibilitySink &sink);
DVAPI void timeline(const Flash::Timeline &timeline, const QString &sourceId,
                    const CompatibilitySink &sink);
DVAPI void layer(const Flash::Layer &layer, const QString &sourceId,
                 const CompatibilitySink &sink);
// `container` is as for sound(): frame scripts are classified on document
// and movie clip timelines; Flash ignores them in graphic symbols.
DVAPI void frame(const Flash::Frame &frame, const QString &sourceId,
                 const CompatibilitySink &sink,
                 const QString &container = "document");
// Native sound columns play only on the top-level timeline. `container` is
// "document" for a document (scene) timeline, otherwise the type of the
// enclosing definition ("graphic", "movie clip", "button" or "group").
// Returns the sound to place, or nullopt when the frame has none or it is
// not placed (reported: graphic and button sounds). Movie clip sounds are
// returned: the builder places them where clips on stage play them. Stop
// cues need no media.
DVAPI std::optional<Flash::FrameSound> sound(const Flash::Frame &frame,
                                             const QString &sourceId,
                                             const QString &container,
                                             const CompatibilitySink &sink);
// Reads a sound library item's embedded samples, or reports why the cue at
// sourceId cannot be placed (missing item or data, unsupported format, MP3
// without a decoder). MP3 samples are returned undecoded (see
// BuildOptions::decodeSound).
DVAPI std::optional<Flash::SoundPayload> soundMedia(
    Flash::Package &package, const Flash::DocumentInfo &info,
    const QString &name, const QString &sourceId, bool decoderAvailable,
    const CompatibilitySink &sink);
// Reads a video library item's embedded frames for the instance at sourceId,
// or reports why it cannot be placed (missing item or data, unsupported
// data, no decoder). A sound track that cannot be read (not raw PCM covering
// the video) is reported as an approximation of a placed video; an audible
// one is returned for the builder to place.
DVAPI std::optional<Flash::VideoPayload> videoMedia(
    Flash::Package &package, const Flash::DocumentInfo &info,
    const QString &name, const QString &sourceId, bool decoderAvailable,
    const CompatibilitySink &sink);
DVAPI void element(const Flash::Element &element, const Flash::Frame &frame,
                   double dpi, const CompatibilitySink &sink,
                   const CompatibilityCheck &check = {},
                   quint64 *usedStyleBindings      = nullptr);
DVAPI void vector(const Flash::Element &element, const CompatibilitySink &sink,
                  const CompatibilityCheck &check = {},
                  quint64 *usedStyleBindings      = nullptr);
// Whether vector() maps this bitmap fill to a TFlashBitmapStyle (its bitmap
// is decoded when the paint is registered).
DVAPI bool mappedBitmapFill(const Flash::Paint &paint);

// The silhouette a mask layer clips with. Flash masks use the filled area of
// their artwork, fully opaque, and ignore strokes, colors, gradients and
// transparency: Animate publishes mask shapes with their strokes, and the
// player (Ruffle too) writes its mask stencil for every filled pixel and
// omits strokes. Every fill becomes an opaque solid paint (a gradient its
// mean color), strokes are dropped, and so are edges that bound no fill; the
// drawing stays an editable picture of the mask artwork.
DVAPI Flash::Shape maskSilhouette(const Flash::Shape &shape);
DVAPI void reject(const CompatibilityIssue &issue);
}  // namespace Compatibility

// No scene, level, palette or asset file is created. The package is read on the
// caller's thread, observing its cancellation callback and read/geometry
// limits. The report bounds displayed issues while retaining the total/error
// counts. Native region construction and actual conversion can still fail
// afterwards.
DVAPI QJsonObject preflightScene(Flash::Package &package,
                                 const Flash::DocumentInfo &info,
                                 const BuildOptions &options);

}  // namespace FlashNative
