// MohoReader.cpp - Moho (Lost Marble) 2D animation project reader
// Copyright (c) 2026 Flare Project
//
// See MohoReader.h for the format background and for what is deliberately out
// of scope. Parsing here is read-only and total: every field is read with a
// default, because Moho documents drop fields between format generations
// (the 1021 generation omits doc_uuid, action_refs, the bezier handle weights
// and modified_date entirely) and a reader that assumes presence will simply
// crash on older files.

#include "MohoReader.h"
#include "ZipArchive.h"
#include "tsystem.h"

#include <cctype>

#include <QAtomicInteger>
#include <QDateTime>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonParseError>
#include <functional>

namespace Moho {

namespace {

// Removes a directory when it goes out of scope.
//
// A Moho project is a ZIP that has to be unpacked before it can be read, and
// unpacking leaves a copy. TSystem has no scoped helper for that, and getting
// the removal wrong on any of the several early returns out of read() leaks a
// full copy of the project into the temp directory on every import.
//
// The destructor is deliberately silent about failure. TSystem::rmDirTree
// throws, and a throw from a destructor is std::terminate -- so an unreadable
// file in the extraction, which is ordinary when something has the directory
// open, would abort the application mid-import. A leftover temp directory is a
// far smaller problem than a crash, and the caller has no way to act on the
// exception anyway.
class ScopedTempDir {
public:
    explicit ScopedTempDir(TFilePath dir) : m_dir(std::move(dir)) {}
    ~ScopedTempDir() noexcept {
        try {
            TSystem::rmDirTree(m_dir);
        } catch (...) {
            // Nothing useful to do here. See the note above.
        }
    }

    ScopedTempDir(const ScopedTempDir &) = delete;
    ScopedTempDir &operator=(const ScopedTempDir &) = delete;

private:
    TFilePath m_dir;
};

// A Moho animated value. All six flavours share the same key names.
Channel readChannel(const QJsonValue &v) {
    Channel c;
    if (!v.isObject()) return c;
    const QJsonObject o = v.toObject();
    c.type = o.value("type").toString();
    c.muted = o.value("mute").toBool(false);
    for (const QJsonValue &f : o.value("when").toArray())
        c.frames.append(f.toInt());
    for (const QJsonValue &x : o.value("val").toArray())
        c.values.append(x);
    return c;
}

// A Moho animated value is structurally identified, not named: any object
// carrying parallel `when` (frame numbers) and `val` (values) arrays is a
// channel, whatever it is called and wherever it is nested.
//
// Detecting them this way rather than from a list of key names is deliberate.
// The real key set is large (40+ distinct names in an ordinary 23-bone rig)
// and Moho nests channels several levels deep -- under `transforms`, inside
// `mesh.points[]` and `mesh.curves[].points[]`, on `skeleton.bones[]`, and
// inside an action's `pose` -- so any hand-maintained list is wrong on the
// first real document. Walking the tree also means a format generation that
// adds a channel is counted without a code change.
void censusChannels(const QJsonValue &node, int &tracks, int &keyframes) {
    if (node.isObject()) {
        const QJsonObject o = node.toObject();
        if (o.contains("when") && o.contains("val")) {
            const Channel c = readChannel(o);
            if (c.size() > 0) {
                ++tracks;
                keyframes += c.size();
            }
            // `interp` is a parallel array of per-segment descriptors, not
            // channels in its own right, so do not descend past the keys that
            // make up the channel itself.
            return;
        }
        for (auto it = o.constBegin(); it != o.constEnd(); ++it)
            censusChannels(it.value(), tracks, keyframes);
    } else if (node.isArray()) {
        for (const QJsonValue &v : node.toArray())
            censusChannels(v, tracks, keyframes);
    }
}

// Bitmaps live outside the container, referenced by path from a sibling folder.
// Which keys carry those paths varies by layer type, so scan the whole tree
// rather than a fixed list.
void collectImages(const QJsonValue &node, QStringList &out) {
    static const QStringList kPathKeys = {
        QStringLiteral("image_path"),        QStringLiteral("fill_texture_path"),
        QStringLiteral("line_texture_path"), QStringLiteral("fill_texture_fileref"),
        QStringLiteral("line_texture_fileref"), QStringLiteral("layer_ref_path"),
        QStringLiteral("brush_name"),
    };
    if (node.isObject()) {
        const QJsonObject o = node.toObject();
        for (const QString &k : kPathKeys) {
            const QString p = o.value(k).toString();
            if (p.isEmpty()) continue;
            // Moho stores a relative path alongside an absolute one where it
            // can; only the relative one resolves against the project folder.
            if (p.contains("://") || QDir::isAbsolutePath(p)) continue;
            if (!out.contains(p)) out << p;
        }
        for (auto it = o.constBegin(); it != o.constEnd(); ++it)
            collectImages(it.value(), out);
    } else if (node.isArray()) {
        for (const QJsonValue &v : node.toArray())
            collectImages(v, out);
    }
}

void readLayer(const QJsonObject &o, int depth, Layer &layer) {
    layer.name     = o.value("name").toString();
    layer.type     = o.value("type").toString();
    layer.uuid     = o.value("uuid").toString();
    layer.visible  = o.value("visible").toBool(true);
    layer.depth    = depth;
    // -2 is "no bone", -1 is "flexible/region" binding, >=0 is a bone index.
    layer.parentBone = o.value("parent_bone").toInt(-2);
    layer.flexibleBoneSubset = o.value("flexi_bone_subset").toString();
    layer.imagePath = o.value("image_path").toString();
    const QString fill = o.value("fill_texture_path").toString();
    const QString line = o.value("line_texture_path").toString();
    layer.texturePath = !fill.isEmpty() ? fill : line;

    const QJsonArray kids = o.value("layers").toArray();
    for (const QJsonValue &c : kids) {
        if (!c.isObject()) continue;
        Layer child;
        readLayer(c.toObject(), depth + 1, child);
        layer.children.append(child);
    }
}

}  // namespace

// ---------------------------------------------------------------------------

Container detectContainer(const TFilePath &path) {
    QFile f(path.getQString());
    if (!f.open(QIODevice::ReadOnly)) return Container::Unknown;
    // 64 bytes: enough for the ZIP local header, a BOM plus the .anme signature,
    // and whitespace ahead of a '{'. The old peek of 8 could not see a signature
    // that arrived after leading whitespace or a BOM.
    const QByteArray head = f.peek(64);
    f.close();
    if (head.isEmpty()) return Container::Unknown;

    if (head.size() >= 4 && head[0] == 'P' && head[1] == 'K' &&
        (head[2] == 0x03 || head[2] == 0x05 || head[2] == 0x07))
        return Container::Zip;

    // The pre-11 .anme format is plain text beginning with this literal header.
    // Checking for it is what keeps a binary file from being diagnosed as a
    // legacy Moho project: "the first non-space byte is not '{'" is true of
    // every SWF, PNG and PDF, so inferring Legacy from it told a user holding a
    // Flash movie that they had an old Moho project and should re-save it from
    // Moho -- advice that cannot help them. So the header is required.
    static const char kLegacyHeader[] = "Anime Studio Project";
    const int legacyAt = head.indexOf(kLegacyHeader);
    if (legacyAt >= 0 && legacyAt < 16) {
        // Confined to the leading whitespace, so the phrase appearing later in
        // an unrelated binary does not classify that binary as legacy.
        for (int i = 0; i < legacyAt; ++i)
            if (!isspace(static_cast<unsigned char>(head.at(i))))
                return Container::Unknown;
        return Container::Legacy;
    }

    // Moho documents are JSON, so they begin with '{' once a UTF-8 BOM -- which
    // the JSON grammar allows, and which editors do emit -- and any whitespace
    // are skipped. The BOM was not skipped before, so a valid .mohoproj that
    // carried one was classified as legacy.
    for (int i = 0; i < head.size(); ++i) {
        const unsigned char ch = static_cast<unsigned char>(head.at(i));
        if (isspace(ch)) continue;
        if (ch == 0xEF && i + 2 < head.size() &&
            static_cast<unsigned char>(head.at(i + 1)) == 0xBB &&
            static_cast<unsigned char>(head.at(i + 2)) == 0xBF) {
            i += 2;
            continue;
        }
        if (ch == '{') return Container::RawJson;
        // Not JSON, not legacy, not a ZIP. Report that, rather than picking a
        // container for it and then failing inside that reader.
        return Container::Unknown;
    }
    // Nothing but whitespace: an empty document. Unknown, so the caller says the
    // file holds no project rather than that the format is unrecognised.
    return Container::Unknown;
}

bool read(const TFilePath &path, Document &doc) {
    doc = Document();
    doc.container = detectContainer(path);

    if (doc.container == Container::Legacy) {
        doc.error = "pre-11 .anme project: a plain-text format Moho itself only "
                    "reads. Re-save it from Moho as a .moho project.";
        return false;
    }
    if (doc.container == Container::Unknown) {
        doc.error = "not a Moho project (expected a ZIP around "
                    "Project.mohoproj, or a bare .mohoproj JSON document)";
        return false;
    }

    // ---- get the JSON document --------------------------------------------
    QByteArray raw;
    if (doc.container == Container::Zip) {
        // Reuse the shared, hardened ZIP extractor: it repairs stale trailers
        // and refuses member paths that escape the output directory.
        //
        // The source file's name is embedded, so it has to be reduced to
        // characters a directory name may actually contain. TSystem::mkDir
        // rejects more than the Windows-reserved set -- '+' among them -- and an
        // earlier version of this listed only the reserved characters, so
        // importing "foo+bar.moho" still threw: out of read(), past its
        // "returns false and fills doc.error" contract, and through a menu
        // handler with no catch around it. Allowlisting is the fix, because
        // there is then no rejected character to have forgotten.
        QString base = QFileInfo(path.getQString()).fileName();
        static const QString allowed =
            QStringLiteral("abcdefghijklmnopqrstuvwxyz"
                           "ABCDEFGHIJKLMNOPQRSTUVWXYZ"
                           "0123456789-_.");
        for (QChar &c : base)
            if (!allowed.contains(c))
                c = QLatin1Char('_');
        if (base.isEmpty())
            base = QStringLiteral("project");
        // A path component is length-limited, and a long source name would
        // otherwise push the whole temp path over it.
        if (base.size() > 48)
            base.truncate(48);

        // Uniqueness. The millisecond clock alone collides when the same file
        // is read twice inside one millisecond, and the second extraction would
        // then remove the first one's directory while it is being read.
        static QAtomicInteger<quint64> counter(0);
        const QString tmpName =
            QStringLiteral("moho_%1_%2_%3")
                .arg(base)
                .arg(QDateTime::currentMSecsSinceEpoch())
                .arg(counter.fetchAndAddOrdered(1));

        const TFilePath outDir =
            TSystem::getTempDir() + TFilePath(tmpName.toStdString());
        // Scoped so the extraction is removed on every path out, including the
        // early returns below. Without it, each import of a ZIP project left a
        // full extracted copy -- plus preview.jpg -- in %TEMP% permanently,
        // under a name that changed every run. Declared before the try so a
        // throw from mkDir cannot skip its own cleanup.
        const ScopedTempDir extraction(outDir);
        try {
            TSystem::mkDir(outDir);
        } catch (const std::exception &e) {
            doc.error = QStringLiteral("could not create a temporary directory "
                                       "to unpack the project into: %1")
                            .arg(QString::fromUtf8(e.what()));
            return false;
        }
        std::string detail;
        if (!FlareZip::extract(path, outDir, detail)) {
            doc.error = "could not read the project container: " +
                        QString::fromStdString(detail);
            return false;
        }
        // Find whichever of the two document names this build uses.
        QDir dir(outDir.getQString());
        for (const QString &name : {"Project.mohoproj", "Project.animeproj"}) {
            const QString f = dir.absoluteFilePath(name);
            if (!QFile::exists(f)) continue;
            QFile in(f);
            if (!in.open(QIODevice::ReadOnly)) continue;
            raw = in.readAll();
            doc.containerEntry = name;
            break;
        }
        if (raw.isEmpty()) {
            doc.error = "the archive contains no Project.mohoproj (or "
                        "Project.animeproj); this does not look like a Moho project";
            return false;
        }
    } else {
        QFile f(path.getQString());
        if (!f.open(QIODevice::ReadOnly)) {
            doc.error = "cannot open the project document";
            return false;
        }
        raw = f.readAll();
        doc.containerEntry = QFileInfo(path.getQString()).fileName();
    }

    // ---- parse -------------------------------------------------------------
    QJsonParseError perr{};
    const QJsonDocument jd = QJsonDocument::fromJson(raw, &perr);
    if (perr.error != QJsonParseError::NoError) {
        doc.error = "the project document is not valid JSON: " +
                    perr.errorString();
        return false;
    }
    if (!jd.isObject()) {
        // Distinct from the above: this parsed cleanly, it is just not a
        // document. Reporting it as a JSON error produced the nonsense message
        // "not valid JSON: no error occurred".
        doc.error = QStringLiteral(
            "the project document is a JSON %1, not a Moho document (which is "
            "a JSON object)")
            .arg(jd.isArray() ? QStringLiteral("array") : QStringLiteral("value"));
        return false;
    }
    const QJsonObject root = jd.object();

    doc.mimeType     = root.value("mime_type").toString();
    doc.version      = root.value("version").toInt();
    doc.majorVersion = root.value("major_version").toInt();

    // The mime type is the authoritative marker. Accept it when present, but
    // do not require it: some tools rewrite the file and drop it.
    if (!doc.mimeType.isEmpty() &&
        !doc.mimeType.contains("lm_mohodoc", Qt::CaseInsensitive)) {
        doc.error = "not a Moho document (mime_type is \"" + doc.mimeType + "\")";
        return false;
    }

    const QJsonObject pd = root.value("project_data").toObject();
    doc.width  = pd.value("width").toInt(0);
    doc.height = pd.value("height").toInt(0);
    doc.fps    = pd.value("fps").toDouble(0.0);
    doc.endFrame  = pd.value("end_frame").toInt(0);
    doc.startFrame = pd.value("start_frame").toInt(0);

    // ---- layers ------------------------------------------------------------
    int tracks = 0, keyframes = 0;
    int shapes = 0, points = 0, actions = 0;
    QMap<QString, int> types;

    // The channel census is a single walk of the whole document. Doing it per
    // layer instead would count a nested layer's channels twice, once with its
    // parent and once on its own.
    censusChannels(root, tracks, keyframes);

    // The layer walk, which is separate: it follows `layers` only, since that
    // is the sole nesting relation in a Moho document.
    std::function<void(const QJsonObject &, int)> visit =
        [&](const QJsonObject &o, int depth) {
            const QString t = o.value("type").toString();
            if (!t.isEmpty()) ++types[t];
            ++doc.totalLayers;
            // An action is a named entry in a layer's action list. The `pose`
            // it carries is a channel, which censusChannels already counted;
            // here we only want the registry of names, which is what turns a
            // bone name into a Smart Bone dial.
            for (const QJsonValue &a : o.value("actions").toArray()) {
                if (a.isObject() && !a.toObject().value("name").toString().isEmpty())
                    ++actions;
            }
            if (o.contains("mesh")) {
                const QJsonObject mesh = o.value("mesh").toObject();
                shapes += mesh.value("shapes").toArray().size();
                points += mesh.value("points").toArray().size();
            }
            // A switch layer names the active child by name, not index.
            if (t == "SwitchLayer") {
                Switch sw;
                sw.name = o.value("name").toString();
                for (const QJsonValue &c : o.value("layers").toArray()) {
                    if (c.isObject()) sw.alternatives << c.toObject().value("name").toString();
                }
                sw.keys = readChannel(o.value("switch_keys"));
                if (sw.keys.size() > 0) {
                    // Frame 0 is the rest pose in a Moho document, so the first
                    // key is the rest state; the last is where the timeline ends
                    // up. Reporting only one of them, under a name implying
                    // "whatever is showing now", would be misleading either way.
                    sw.childAtRest = sw.keys.values.first().toString();
                    sw.childAtEnd = sw.keys.values.last().toString();
                }
                doc.switches.append(sw);
            }
            // Bones live on the BoneLayer's skeleton, as a flat array with
            // parent indices - order is not guaranteed to be hierarchical.
            const QJsonObject sk = o.value("skeleton").toObject();
            for (const QJsonValue &b : sk.value("bones").toArray()) {
                if (!b.isObject()) continue;
                const QJsonObject bo = b.toObject();
                Bone bone;
                bone.name     = bo.value("name").toString();
                bone.parent   = bo.value("parent").toInt(-1);
                bone.length   = bo.value("length").toDouble(0.0);
                bone.strength = bo.value("strength").toDouble(0.0);
                bone.dynamic  = bo.value("bone_dynamics").toBool(false);
                doc.bones.append(bone);
            }
            for (const QJsonValue &c : o.value("layers").toArray())
                if (c.isObject()) visit(c.toObject(), depth + 1);
        };

    // Image paths can appear at any depth, including on bones and on actions,
    // so the scan starts at the document root rather than at the layer tree.
    collectImages(root, doc.referencedImages);

    for (const QJsonValue &l : root.value("layers").toArray()) {
        if (!l.isObject()) continue;
        Layer layer;
        readLayer(l.toObject(), 0, layer);
        doc.layers.append(layer);
        visit(l.toObject(), 0);
    }

    doc.layerTypes     = types;
    doc.keyframeTracks = tracks;
    doc.keyframes      = keyframes;
    doc.actions        = actions;
    doc.shapes         = shapes;
    doc.meshPoints     = points;
    doc.styles         = root.value("styles").toArray().size();

    // ---- resolve the referenced images ------------------------------------
    // They live next to the project, not inside it.
    const QString projDir = QFileInfo(path.getQString()).absolutePath();
    for (const QString &rel : doc.referencedImages) {
        const QString abs = QDir(projDir).absoluteFilePath(rel);
        if (QFileInfo::exists(abs)) ++doc.imagesFound;
        else ++doc.imagesMissing;
    }

    doc.valid = true;
    return true;
}

int writeManifest(const TFilePath &projectPath, const Document &doc,
                  const TFilePath &outDir) {
    if (!doc.valid) return 0;
    TSystem::mkDir(outDir);

    QJsonObject root;
    root["format"] = "moho-rig-manifest";
    root["format_version"] = 1;
    root["source_file"] = QFileInfo(projectPath.getQString()).fileName();
    // Which member of the container held the document. Written but never read
    // before, so it said nothing useful to anyone.
    root["container_entry"] = doc.containerEntry;
    root["mime_type"] = doc.mimeType;
    root["moho_version"] = doc.version;
    root["major_version"] = doc.majorVersion;
    root["width"] = doc.width;
    root["height"] = doc.height;
    root["fps"] = doc.fps;
    root["start_frame"] = doc.startFrame;
    root["end_frame"] = doc.endFrame;

    QJsonObject census;
    QJsonObject types;
    for (auto it = doc.layerTypes.constBegin(); it != doc.layerTypes.constEnd(); ++it)
        types[it.key()] = it.value();
    census["layer_types"] = types;
    census["total_layers"] = doc.totalLayers;
    census["keyframe_tracks"] = doc.keyframeTracks;
    census["keyframes"] = doc.keyframes;
    census["actions"] = doc.actions;
    census["shapes"] = doc.shapes;
    census["mesh_points"] = doc.meshPoints;
    census["bones"] = doc.bones.size();
    census["switch_layers"] = doc.switches.size();
    census["document_styles"] = doc.styles;
    root["census"] = census;

    QJsonArray bones;
    for (const Bone &b : doc.bones) {
        QJsonObject o;
        o["name"] = b.name;
        o["parent"] = b.parent;
        o["length"] = b.length;
        o["strength"] = b.strength;
        o["dynamic"] = b.dynamic;
        bones.append(o);
    }
    root["bones"] = bones;

    QJsonArray switches;
    for (const Switch &s : doc.switches) {
        QJsonObject o;
        o["name"] = s.name;
        o["alternatives"] = QJsonArray::fromStringList(QStringList(s.alternatives.begin(),
                                                    s.alternatives.end()));
        o["child_at_rest"] = s.childAtRest;
        o["child_at_end"] = s.childAtEnd;
        o["keys"] = s.keys.frames.size();
        switches.append(o);
    }
    root["switch_layers"] = switches;

    // Layers: name, type, hierarchy, visibility and bone binding. Draw order is
    // array order, back to front, so it is preserved implicitly.
    std::function<QJsonArray(const QVector<Layer> &)> dumpLayers =
        [&](const QVector<Layer> &ls) {
            QJsonArray arr;
            for (const Layer &l : ls) {
                QJsonObject o;
                o["name"] = l.name;
                o["type"] = l.type;
                o["uuid"] = l.uuid;
                o["visible"] = l.visible;
                o["depth"] = l.depth;
                o["parent_bone"] = l.parentBone;
                if (!l.flexibleBoneSubset.isEmpty())
                    o["flexible_bone_subset"] = l.flexibleBoneSubset;
                if (!l.imagePath.isEmpty()) o["image_path"] = l.imagePath;
                if (!l.texturePath.isEmpty()) o["texture_path"] = l.texturePath;
                if (!l.children.isEmpty())
                    o["layers"] = dumpLayers(l.children);
                arr.append(o);
            }
            return arr;
        };
    root["layers"] = dumpLayers(doc.layers);

    QJsonObject images;
    images["referenced"] = QJsonArray::fromStringList(QStringList(doc.referencedImages.begin(),
                                                    doc.referencedImages.end()));
    images["found"] = doc.imagesFound;
    images["missing"] = doc.imagesMissing;
    // Worth saying out loud: a .moho on its own is not a complete project.
    images["note"] =
        QStringLiteral("Moho keeps bitmaps outside the container, in a sibling "
                       "folder. This project referenced %1 image(s); %2 were "
                       "found next to it and %3 were not.")
            .arg(doc.referencedImages.size())
            .arg(doc.imagesFound)
            .arg(doc.imagesMissing);
    root["images"] = images;

    int written = 0;
    const QString manifest = outDir.getQString() + "/moho_manifest.json";
    QFile mf(manifest);
    if (mf.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        mf.write(QJsonDocument(root).toJson(QJsonDocument::Indented));
        mf.close();
        ++written;
    }

    // Copy the referenced images that actually exist, so the rig is usable.
    if (doc.imagesFound > 0) {
        const QString projDir = QFileInfo(projectPath.getQString()).absolutePath();
        const QString imgDir = outDir.getQString() + "/images";
        TSystem::mkDir(TFilePath(imgDir));
        for (const QString &rel : doc.referencedImages) {
            const QString abs = QDir(projDir).absoluteFilePath(rel);
            if (!QFileInfo::exists(abs)) continue;
            // Keep the sub-path so a rig with e.g. "images/char/arm.png"
            // does not flatten to one directory.
            const QString dst = imgDir + "/" + rel;
            TSystem::mkDir(TFilePath(QFileInfo(dst).absolutePath()));
            if (QFile::copy(abs, dst)) ++written;
        }
    }
    return written;
}

}  // namespace Moho
