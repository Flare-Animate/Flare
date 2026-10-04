// MohoReader.h - Moho (Lost Marble) 2D animation project reader
// Copyright (c) 2026 Flare Project
//
// Moho project structure, as reverse-engineered and documented by:
//
//   * Smith Micro / Lost Marble, "Anime Studio Document File Format" (the
//     original 20-page spec PDF, still the only first-party documentation of
//     the top-level sections, styles and the layer `type` enum).
//   * vinhio/moho2svg, which validates JSON Schemas against 46 real project
//     files spanning format versions 1021-1045. Its schemas are the most
//     complete public description of the format; the code is not open source
//     and nothing is copied from it here.
//   * jpdurigan/godot_moho_importer (MIT) and mohokit, for the container
//     layout and the rigging model.
//
// Container: a Moho project is a ZIP holding a single minified JSON document
// (`Project.mohoproj`, or `Project.animeproj` from Anime Studio 11) plus an
// optional `preview.jpg`. `.mohoproj` / `.animeproj` are the same document
// without the ZIP wrapper, and Moho 13+ opens those directly.
//
// Deliberately not implemented: rendering. Reproducing a Moho rig means
// solving the skeleton, then per-layer region-weight deformation (weights are
// computed at run time from bone length and strength - there is no per-point
// weight table), then bezier reconstruction, masking, layer effects and bone
// dynamics. moho2svg, the most advanced implementation in existence, needed
// months of pixel-diffing against Moho's own SVG export. What is extracted
// here is the structure: layers, bones, binding, keyframe tracks and the
// image paths the rig refers to.
//
// Note on assets: bitmaps are NOT inside the container. Moho references them
// by relative path from a sibling folder (normally `images/`), which
// `File -> Gather Media` populates. A `.moho` file handed over on its own is
// therefore incomplete, and this reader reports how many referenced images it
// could actually find.

#ifndef MOHOREADER_H_
#define MOHOREADER_H_

#include "tcommon.h"
#include "tfilepath.h"
#include <QJsonObject>
#include <QJsonValue>
#include <QMap>
#include <QString>
#include <QStringList>
#include <QVector>

#undef DVAPI
#undef DVVAR
#ifdef TFLASH_EXPORTS
#define DVAPI DV_EXPORT_API
#define DVVAR DV_EXPORT_VAR
#else
#define DVAPI DV_IMPORT_API
#define DVVAR DV_IMPORT_VAR
#endif

namespace Moho {

// Container flavour, decided from the content rather than the extension.
enum class Container {
    Unknown,
    Zip,      // .moho / .anime : ZIP around Project.mohoproj
    RawJson,  // .mohoproj / .animeproj : the bare document
    Legacy    // .anme : pre-11 plain-text format, read-only in Moho itself
};

// One animated value track. Moho stores every animatable value as six
// parallel arrays; `frames` and `values` are the only two needed to describe
// the data, and are index-aligned.
struct Channel {
    QString type;                 // Val | Vec2 | Vec3 | Color | Bool | String
    QVector<int> frames;
    QVector<QJsonValue> values;
    bool muted = false;
    int size() const { return qMin(frames.size(), values.size()); }
};

struct Layer {
    QString name;
    QString type;       // MeshLayer | ImageLayer | GroupLayer | BoneLayer | ...
    QString uuid;
    bool visible = true;
    int parentBone = -2;         // -2 = none, -1 = flexible/region binding
    QString flexibleBoneSubset;  // "|"-joined bone indices, when region bound
    QString imagePath;           // ImageLayer.image_path, relative to the project
    QString texturePath;         // MeshLayer fill/line texture
    int depth = 0;               // nesting depth, for reporting
    QVector<Layer> children;     // GroupLayer / BoneLayer / SwitchLayer
};

struct Bone {
    QString name;
    int parent = -1;             // index into Document::bones
    double length = 0.0;
    double strength = 0.0;
    bool dynamic = false;        // bone_dynamics: spring/damping at playback
};

struct Switch {
    QString name;
    QVector<QString> alternatives;   // child layer names
    Channel keys;                    // String channel naming the active child
    // The child named at the first key. In a Moho document frame 0 is the rest
    // pose rather than frame one, so the first key *is* the rest pose -- but it
    // is not necessarily the child showing later in the timeline, which is what
    // "active" suggests. Both are reported rather than picking one silently.
    QString childAtRest;
    QString childAtEnd;              // empty when the channel has no keys
};

// A parsed Moho document.
struct Document {
    bool valid = false;
    Container container = Container::Unknown;
    QString mimeType;
    // Which member of the container held the document ("Project.mohoproj", or
    // the file's own name for a bare .mohoproj). Worth reporting: it tells a
    // user immediately whether they handed over a .moho or a stripped project.
    QString containerEntry;
    int version = 0;              // format revision: 1021 / 1038 / 1045
    int majorVersion = 0;

    int width = 0;
    int height = 0;
    double fps = 0.0;
    int startFrame = 0;
    int endFrame = 0;

    QVector<Layer> layers;        // top level, in draw order
    QVector<Bone> bones;
    QVector<Switch> switches;

    // Census, so the import can say what the rig contains.
    int totalLayers = 0;
    QMap<QString, int> layerTypes;
    int keyframeTracks = 0;
    int keyframes = 0;
    int actions = 0;             // Smart Bone / timeline action names
    int shapes = 0;
    int meshPoints = 0;
    int styles = 0;

    // Image paths the rig refers to, in document order, deduplicated.
    QStringList referencedImages;
    // Of those, how many were actually found next to the project.
    int imagesFound = 0;
    int imagesMissing = 0;

    QString error;
};

// Identify the container. Reads only the leading bytes where possible.
DVAPI Container detectContainer(const TFilePath &path);

// Parse a Moho project. Accepts a .moho/.anime ZIP or a bare
// .mohoproj/.animeproj document. Returns false and fills doc.error on failure;
// a .anme is recognised and reported as legacy rather than mis-parsed.
DVAPI bool read(const TFilePath &path, Document &doc);

// Write the extracted rig structure to `<outDir>/moho_manifest.json`, and copy
// the referenced images that exist next to the project into `<outDir>/images/`.
// Returns the number of files written.
DVAPI int writeManifest(const TFilePath &projectPath, const Document &doc,
                        const TFilePath &outDir);

}  // namespace Moho

#endif  // MOHOREADER_H_
