// As3Bridge.cpp - AS3 support, calling the native Rust core directly.
// Copyright (c) 2026 Flare Project
//
// See As3Bridge.h. Every entry point returns a Result with an explanatory
// error instead of throwing.

#include "As3Bridge.h"

#include "flare_formats.h"
#include "tsystem.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>

namespace As3Bridge {

namespace {

// Owns a malloc'd string from the Rust side.
struct RustStr {
    char *p;
    explicit RustStr(char *s) : p(s) {}
    ~RustStr() { if (p) flare_as3_free(p); }
    RustStr(const RustStr &) = delete;
    RustStr &operator=(const RustStr &) = delete;
};

bool readFile(const TFilePath &fp, QByteArray &out, QString &err) {
    QFile f(fp.getQString());
    if (!f.open(QIODevice::ReadOnly)) {
        err = QObject::tr("cannot read %1").arg(fp.getQString());
        return false;
    }
    out = f.readAll();
    if (out.isEmpty()) {
        err = QObject::tr("%1 is empty").arg(fp.getQString());
        return false;
    }
    return true;
}

bool writeFile(const QString &path, const QByteArray &data) {
    QDir().mkpath(QFileInfo(path).absolutePath());
    QFile f(path);
    if (!f.open(QIODevice::WriteOnly | QIODevice::Truncate)) return false;
    return f.write(data) == data.size();
}

const uint8_t *u8(const QByteArray &b) {
    return reinterpret_cast<const uint8_t *>(b.constData());
}

// Handle of a patch call: take the output SWF and the JSON status.
bool finishPatch(FlareAs3Result *h, const TFilePath &outSwf, Result &r) {
    if (!h) {
        r.error = QObject::tr("the AS3 core rejected the request");
        return false;
    }
    const QJsonObject o = QJsonDocument::fromJson(flare_as3_result_json(h)).object();
    size_t n = 0;
    const uint8_t *bytes = flare_as3_result_bytes(h, &n);
    r.ok = o.value("ok").toBool(false) && bytes && n;
    if (!r.ok) {
        r.error = o.value("error").toString();
        if (r.error.isEmpty()) r.error = QObject::tr("patch failed");
    } else if (!writeFile(outSwf.getQString(),
                          QByteArray(reinterpret_cast<const char *>(bytes), (int)n))) {
        r.ok = false;
        r.error = QObject::tr("cannot write %1").arg(outSwf.getQString());
    } else {
        r.replaced = o.value("replaced").toInt();
        r.blocks = o.value("blocks").toInt();
    }
    flare_as3_result_free(h);
    return r.ok;
}

}  // namespace

bool isAvailable() {
    RustStr v(flare_as3_version());
    return v.p != nullptr;
}

QString version() {
    RustStr v(flare_as3_version());
    return v.p ? QString::fromUtf8(v.p) : QString();
}

QString unavailableReason() {
    return isAvailable() ? QString()
                         : QObject::tr("the native AS3 core did not respond");
}

Result decompile(const TFilePath &swf, const TFilePath &outDir) {
    Result r;
    QByteArray data;
    if (!readFile(swf, data, r.error)) return r;
    RustStr js(flare_as3_decompile(u8(data), (size_t)data.size()));
    if (!js.p) {
        r.error = QObject::tr("the AS3 core rejected the request");
        return r;
    }
    const QJsonObject o = QJsonDocument::fromJson(QByteArray(js.p)).object();
    if (!o.value("ok").toBool(false)) {
        r.error = o.value("error").toString();
        if (r.error.isEmpty()) r.error = QObject::tr("decompile failed");
        return r;
    }
    // No embedded AS3 is not a failure: ok with an empty class list.
    r.ok = true;
    const QString root = outDir.getQString();
    const QJsonArray blocks = o.value("blocks").toArray();
    r.blocks = blocks.size();
    for (const QJsonValue &bv : blocks) {
        const QJsonObject b = bv.toObject();
        // One folder per DoABC block: class names collide across blocks.
        const QString blockDir = QStringLiteral("block_%1").arg(b.value("index").toInt());
        for (const QJsonValue &cv : b.value("classes").toArray()) {
            const QJsonObject c = cv.toObject();
            if (!c.contains("source")) continue;  // per-class failure: skipped like before
            const QString pkg = c.value("package").toString();
            const QString name = c.value("name").toString();
            const QString rel = blockDir + "/" +
                                (pkg.isEmpty() ? QString() : QString(pkg).replace('.', '/') + "/") +
                                name + ".as";
            const QString src = c.value("source").toString();
            if (!writeFile(root + "/" + rel, src.toUtf8())) {
                r.ok = false;
                r.error = QObject::tr("cannot write %1").arg(root + "/" + rel);
                return r;
            }
            ActionScriptClass k;
            k.name = c.value("full_name").toString();
            k.block = blockDir;
            k.source = src;
            r.classes.append(k);
        }
    }
    return r;
}

Result patchStrings(const TFilePath &swf, const TFilePath &patchJson,
                    const TFilePath &outSwf) {
    Result r;
    QByteArray data, patch;
    if (!readFile(swf, data, r.error)) return r;
    QString perr;
    if (!readFile(patchJson, patch, perr)) {
        r.error = QObject::tr("could not read patch file: %1").arg(perr);
        return r;
    }
    const QByteArray pj = patch;
    finishPatch(flare_as3_patch_strings(u8(data), (size_t)data.size(), pj.constData()), outSwf, r);
    return r;
}

Result patchClassSource(const TFilePath &swf, int block, int classIndex,
                        const QString &editedSource, const TFilePath &outSwf) {
    Result r;
    QByteArray data;
    if (!readFile(swf, data, r.error)) return r;
    const QByteArray src = editedSource.toUtf8();
    finishPatch(flare_as3_patch_class(u8(data), (size_t)data.size(), block, classIndex,
                                      src.constData()),
                outSwf, r);
    return r;
}

Result compile(const TFilePath &sourceDir, const TFilePath &outSwf) {
    Result r;
    const QByteArray s = sourceDir.getQString().toUtf8();
    const QByteArray o = outSwf.getQString().toUtf8();
    RustStr js(flare_as3_compile(s.constData(), o.constData()));
    if (!js.p) {
        r.error = QObject::tr("the AS3 core rejected the request");
        return r;
    }
    const QJsonObject j = QJsonDocument::fromJson(QByteArray(js.p)).object();
    r.ok = j.value("ok").toBool(false);
    if (!r.ok) r.error = j.value("error").toString();
    return r;
}

}  // namespace As3Bridge
